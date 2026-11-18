//! Programmes au lancement (`HKCU\...\Run`) avec export `.reg` automatique.
//!
//! La désactivation utilise `StartupApproved\Run` (même mécanisme que le Gestionnaire des
//! tâches) : la commande de lancement n'est jamais supprimée, l'opération est réversible.

use std::path::PathBuf;

use serde::Serialize;

use crate::core::command;
use crate::core::history::UndoAction;
use crate::core::logger::LogLevel;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};

/// Clé des programmes lancés à l'ouverture de session.
pub const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
/// Clé d'approbation (activé/désactivé) du Gestionnaire des tâches.
pub const APPROVED_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";

/// Programme au lancement.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
    /// Nom de la valeur.
    pub name: String,
    /// Commande exécutée.
    pub command: String,
    /// Lancement actif.
    pub enabled: bool,
}

/// Interprète un drapeau `StartupApproved` (octet 0 pair = activé).
pub fn approved_enabled(data: Option<&RegData>) -> bool {
    match data {
        Some(RegData::Binary(b)) => b.first().map(|x| x % 2 == 0).unwrap_or(true),
        _ => true,
    }
}

/// Liste les programmes au lancement de l'utilisateur.
///
/// Impact Windows : lecture seule.
pub fn list() -> AetherResult<Vec<StartupEntry>> {
    let approved = registry::list_values(Hive::Hkcu, APPROVED_KEY)?;
    let mut out: Vec<StartupEntry> = registry::list_values(Hive::Hkcu, RUN_KEY)?
        .into_iter()
        .filter_map(|(name, data)| match data {
            RegData::Sz(command) => {
                let flag = approved.iter().find(|(n, _)| n == &name).map(|(_, d)| d);
                Some(StartupEntry {
                    enabled: approved_enabled(flag),
                    name,
                    command,
                })
            }
            _ => None,
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    Ok(out)
}

fn export_backup(state: &AppState) -> AetherResult<PathBuf> {
    let file = state.data_dir.join("aether_startup_backup.reg");
    if !file.exists() {
        let target = file.display().to_string();
        command::run("reg.exe", &["export", &format!("HKCU\\{RUN_KEY}"), &target, "/y"])?;
        let approved = state.data_dir.join("aether_startup_approved_backup.reg");
        let _ = command::run_raw(
            "reg.exe",
            &[
                "export",
                &format!("HKCU\\{APPROVED_KEY}"),
                &approved.display().to_string(),
                "/y",
            ],
        );
    }
    Ok(file)
}

fn filetime_now() -> [u8; 8] {
    // Intervalles de 100 ns depuis le 1er janvier 1601.
    let unix_100ns = crate::core::logger::now_ms().saturating_mul(10_000);
    (unix_100ns + 116_444_736_000_000_000).to_le_bytes()
}

/// Active ou désactive un programme au lancement (idempotent, annulable).
///
/// Impact Windows : écrit `HKCU\...\StartupApproved\Run\<nom>` après export `.reg` de sauvegarde.
pub fn set_enabled(state: &AppState, name: &str, enabled: bool) -> AetherResult<bool> {
    let entry = list()?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| AetherError::NotFound(format!("programme au lancement « {name} »")))?;
    if entry.enabled == enabled {
        state.logger.log(
            LogLevel::Skip,
            "startup",
            format!("[OK - Déjà dans l'état demandé] {name}"),
        );
        return Ok(false);
    }
    if state.dry_run() {
        state.logger.log(
            LogLevel::Preview,
            "startup",
            format!(
                "[APERÇU] {} {name}",
                if enabled { "Activation" } else { "Désactivation" }
            ),
        );
        return Ok(false);
    }
    let backup = export_backup(state)?;
    let previous = registry::read(Hive::Hkcu, APPROVED_KEY, name)?;
    let mut flag = vec![if enabled { 0x02 } else { 0x03 }, 0, 0, 0];
    flag.extend_from_slice(&if enabled { [0u8; 8] } else { filetime_now() });
    registry::write(Hive::Hkcu, APPROVED_KEY, name, &RegData::Binary(flag))?;
    state.record(
        &format!("startup.{name}"),
        &format!("Lancement de {name}"),
        vec![UndoAction::Registry {
            hive: Hive::Hkcu,
            path: APPROVED_KEY.to_owned(),
            name: name.to_owned(),
            previous,
        }],
    )?;
    state.logger.success(
        "startup",
        format!(
            "[Appliqué] {name} {} (sauvegarde : {})",
            if enabled { "activé" } else { "désactivé" },
            backup.display()
        ),
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprets_startup_approved_flags() {
        assert!(approved_enabled(None));
        assert!(approved_enabled(Some(&RegData::Binary(vec![2, 0, 0]))));
        assert!(approved_enabled(Some(&RegData::Binary(vec![6, 0, 0]))));
        assert!(!approved_enabled(Some(&RegData::Binary(vec![3, 0, 0]))));
    }

    #[test]
    fn filetime_is_after_2020() {
        let ft = u64::from_le_bytes(filetime_now());
        assert!(ft > 132_223_104_000_000_000);
    }
}
