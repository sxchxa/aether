//! Section « Performance & Jeux » : services, RAM, énergie, gaming et TRIM.

pub mod extras;
pub mod gaming;
pub mod memory;
pub mod power;
pub mod ram;
pub mod services;
pub mod trim;
pub mod visual;

use tauri::State;

use crate::core::history::{StartKind, UndoAction};
use crate::core::logger::LogLevel;
use crate::core::tweak::Risk;
use crate::error::AetherResult;
use crate::modules::{blocking, Shared};

/// Options de services en arrière-plan.
pub fn service_tweaks() -> Vec<services::ServiceTweak> {
    vec![
        services::ServiceTweak {
            id: "svc.spooler",
            service: "Spooler",
            risk: Risk::Moderate,
            default_start: StartKind::Auto,
        },
        services::ServiceTweak {
            id: "svc.diagtrack",
            service: "DiagTrack",
            risk: Risk::Safe,
            default_start: StartKind::Auto,
        },
        services::ServiceTweak {
            id: "svc.wsearch",
            service: "WSearch",
            risk: Risk::Moderate,
            default_start: StartKind::DelayedAuto,
        },
        services::ServiceTweak {
            id: "svc.sysmain",
            service: "SysMain",
            risk: Risk::Moderate,
            default_start: StartKind::Auto,
        },
        services::ServiceTweak {
            id: "svc.mapsBroker",
            service: "MapsBroker",
            risk: Risk::Safe,
            default_start: StartKind::DelayedAuto,
        },
        services::ServiceTweak {
            id: "svc.fax",
            service: "Fax",
            risk: Risk::Safe,
            default_start: StartKind::Manual,
        },
    ]
}

/// Purge la RAM (action non persistante, exécutée même en Mode Aperçu).
#[tauri::command]
pub async fn ram_purge(state: State<'_, Shared>) -> AetherResult<ram::RamReport> {
    blocking(&state, |s| {
        let report = ram::purge()?;
        s.logger.success(
            "ram",
            format!(
                "[Appliqué] RAM purgée : {} Mo libérés ({} processus)",
                report.freed / 1_048_576,
                report.trimmed
            ),
        );
        Ok(report)
    })
    .await
}

/// Modes d'alimentation installés.
#[tauri::command]
pub async fn power_schemes() -> AetherResult<Vec<power::PowerScheme>> {
    tokio::task::spawn_blocking(power::list_schemes).await?
}

/// Active un mode d'alimentation (annulable).
#[tauri::command]
pub async fn power_set_active(state: State<'_, Shared>, guid: String) -> AetherResult<bool> {
    blocking(&state, move |s| {
        let current = power::active_guid()?;
        if current.eq_ignore_ascii_case(&guid) {
            s.logger
                .log(LogLevel::Skip, "power", "[OK - Déjà actif] Mode d'alimentation");
            return Ok(false);
        }
        if s.dry_run() {
            s.logger.log(
                LogLevel::Preview,
                "power",
                format!("[APERÇU] powercfg /setactive {guid}"),
            );
            return Ok(false);
        }
        power::set_active(&guid)?;
        s.record(
            "power.scheme",
            "Mode d'alimentation",
            vec![UndoAction::PowerScheme { guid: current }],
        )?;
        s.logger
            .success("power", format!("[Appliqué] Mode d'alimentation {guid}"));
        Ok(true)
    })
    .await
}

/// Fréquence de rafraîchissement de l'écran.
#[tauri::command]
pub async fn display_info() -> AetherResult<gaming::DisplayInfo> {
    tokio::task::spawn_blocking(gaming::display_info).await?
}

/// Lance un TRIM sur un SSD.
#[tauri::command]
pub async fn ssd_trim(state: State<'_, Shared>, letter: char) -> AetherResult<String> {
    blocking(&state, move |s| trim::retrim(s, letter)).await
}

/// Rétablit la configuration d'origine de tous les services.
#[tauri::command]
pub async fn services_restore_all(state: State<'_, Shared>) -> AetherResult<Vec<String>> {
    blocking(&state, services::restore_all).await
}
