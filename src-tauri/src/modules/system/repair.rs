//! Réparation Windows : `sfc /scannow` et `DISM /RestoreHealth` avec sortie en direct.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::core::command;
use crate::core::logger::LogLevel;
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Outil de réparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RepairTool {
    /// Vérificateur des fichiers système.
    Sfc,
    /// Réparation de l'image Windows (DISM).
    Dism,
}

/// Événement diffusé pendant la réparation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum RepairEvent {
    /// Ligne de sortie.
    Line {
        /// Texte.
        text: String,
    },
    /// Fin de l'exécution.
    Done {
        /// Code de sortie.
        code: Option<i32>,
    },
}

/// Commande exacte exécutée (affichée dans l'inspecteur).
pub fn command_line(tool: RepairTool) -> &'static str {
    match tool {
        RepairTool::Sfc => "sfc /scannow",
        RepairTool::Dism => "DISM /Online /Cleanup-Image /RestoreHealth",
    }
}

struct RunGuard;

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

/// Lance l'outil de réparation et transmet chaque ligne à `emit`.
///
/// Impact Windows : vérifie et répare les fichiers système / le magasin de composants.
/// Peut durer de 5 à 30 minutes ; une seule réparation à la fois.
pub fn run<F: FnMut(RepairEvent)>(state: &AppState, tool: RepairTool, mut emit: F) -> AetherResult<Option<i32>> {
    if state.dry_run() {
        state
            .logger
            .log(LogLevel::Preview, "repair", format!("[APERÇU] {}", command_line(tool)));
        emit(RepairEvent::Line {
            text: format!("Mode Aperçu : « {} » n'a pas été exécuté.", command_line(tool)),
        });
        emit(RepairEvent::Done { code: None });
        return Ok(None);
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AetherError::Forbidden("une réparation est déjà en cours".into()));
    }
    let _guard = RunGuard;
    state
        .logger
        .info("repair", format!("Démarrage : {}", command_line(tool)));
    let mut forward = |line: &str| emit(RepairEvent::Line { text: line.to_owned() });
    let code = match tool {
        RepairTool::Sfc => command::stream("sfc.exe", &["/scannow"], &mut forward)?,
        RepairTool::Dism => command::stream(
            "cmd.exe",
            &["/C", "chcp 65001 >nul & DISM.exe /Online /Cleanup-Image /RestoreHealth"],
            &mut forward,
        )?,
    };
    emit(RepairEvent::Done { code });
    if code == Some(0) {
        state
            .logger
            .success("repair", format!("[Terminé] {}", command_line(tool)));
    } else {
        state.logger.warn(
            "repair",
            format!("{} terminé avec le code {code:?}", command_line(tool)),
        );
    }
    Ok(code)
}
