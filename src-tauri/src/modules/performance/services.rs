//! Services Windows en arrière-plan (Spouleur, Télémétrie, Indexation, SysMain, Fax).

use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use windows_service::service::{ServiceAccess, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use crate::core::command;
use crate::core::history::{StartKind, UndoAction};
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, SERVICES_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::{AetherError, AetherResult};

/// État d'origine d'un service, enregistré dans `aether_services_backup.json`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceBackup {
    /// Type de démarrage d'origine.
    pub start: StartKind,
    /// Le service était en cours d'exécution.
    pub was_running: bool,
}

fn service_key(name: &str) -> String {
    format!("SYSTEM\\CurrentControlSet\\Services\\{name}")
}

/// Indique si le service est installé sur ce PC.
///
/// Impact Windows : lecture seule du Registre.
pub fn is_installed(name: &str) -> bool {
    registry::key_exists(Hive::Hklm, &service_key(name))
}

/// Lit le type de démarrage réel d'un service depuis le Registre.
///
/// Impact Windows : lecture seule.
pub fn read_start(name: &str) -> AetherResult<StartKind> {
    let key = service_key(name);
    let start = match registry::read(Hive::Hklm, &key, "Start")? {
        Some(RegData::Dword(v)) => v,
        _ => return Err(AetherError::NotFound(format!("type de démarrage du service {name}"))),
    };
    let delayed = matches!(
        registry::read(Hive::Hklm, &key, "DelayedAutostart")?,
        Some(RegData::Dword(1))
    );
    Ok(match start {
        0 | 1 => StartKind::System,
        2 if delayed => StartKind::DelayedAuto,
        2 => StartKind::Auto,
        3 => StartKind::Manual,
        _ => StartKind::Disabled,
    })
}

/// Indique si le service est en cours d'exécution (ou en démarrage).
///
/// Impact Windows : interroge le Gestionnaire de contrôle des services (lecture seule).
pub fn is_running(name: &str) -> AetherResult<bool> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(name, ServiceAccess::QUERY_STATUS)?;
    let status = service.query_status()?;
    Ok(!matches!(status.current_state, ServiceState::Stopped))
}

fn wait_for(name: &str, running: bool) -> AetherResult<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(name, ServiceAccess::QUERY_STATUS)?;
        let state = service.query_status()?.current_state;
        let ok = if running {
            state == ServiceState::Running
        } else {
            state == ServiceState::Stopped
        };
        if ok {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(AetherError::Service {
        name: name.to_owned(),
        message: "délai dépassé en attendant le changement d'état".into(),
    })
}

fn sc_start_arg(kind: StartKind) -> AetherResult<&'static str> {
    Ok(match kind {
        StartKind::Auto => "auto",
        StartKind::DelayedAuto => "delayed-auto",
        StartKind::Manual => "demand",
        StartKind::Disabled => "disabled",
        StartKind::System => {
            return Err(AetherError::Forbidden(
                "les services de démarrage système ne sont jamais modifiés".into(),
            ))
        }
    })
}

/// Définit le type de démarrage d'un service puis le démarre ou l'arrête.
///
/// Impact Windows : `sc.exe config <service> start= …` puis arrêt/démarrage via le SCM.
pub fn set_service_state(name: &str, start: StartKind, running: bool) -> AetherResult<()> {
    if read_start(name)? != start {
        command::run("sc.exe", &["config", name, "start=", sc_start_arg(start)?])?;
    }
    let currently = is_running(name)?;
    if currently == running {
        return Ok(());
    }
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(
        name,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::START,
    )?;
    if running {
        service.start::<&str>(&[])?;
    } else {
        service.stop()?;
    }
    wait_for(name, running)
}

fn describe(start: StartKind, running: bool) -> String {
    let s = match start {
        StartKind::Auto => "Automatique",
        StartKind::DelayedAuto => "Automatique (différé)",
        StartKind::Manual => "Manuel",
        StartKind::Disabled => "Désactivé",
        StartKind::System => "Système",
    };
    format!("{s} · {}", if running { "en cours d'exécution" } else { "arrêté" })
}

/// Option « passer en Manuel et arrêter » un service.
pub struct ServiceTweak {
    /// Identifiant de l'option.
    pub id: &'static str,
    /// Nom court du service Windows.
    pub service: &'static str,
    /// Risque.
    pub risk: Risk,
    /// Type de démarrage par défaut de Windows (utilisé sans sauvegarde).
    pub default_start: StartKind,
}

impl Tweak for ServiceTweak {
    fn id(&self) -> &'static str {
        self.id
    }
    fn section(&self) -> Section {
        Section::Performance
    }
    fn group(&self) -> &'static str {
        "services"
    }
    fn risk(&self) -> Risk {
        self.risk
    }
    fn technical(&self) -> String {
        format!(
            "sc.exe config {0} start= demand && sc.exe stop {0}\nHKLM\\{1}\\Start",
            self.service,
            service_key(self.service)
        )
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        if !is_installed(self.service) {
            return Ok(Probe::unavailable("Service non installé sur ce PC"));
        }
        let start = read_start(self.service)?;
        let running = is_running(self.service)?;
        Ok(Probe {
            applied: matches!(start, StartKind::Manual | StartKind::Disabled) && !running,
            available: start != StartKind::System,
            current: describe(start, running),
            target: describe(StartKind::Manual, false),
        })
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let start = read_start(self.service)?;
        let running = is_running(self.service)?;
        state.backups(SERVICES_BACKUP).record_first(
            self.service,
            &ServiceBackup {
                start,
                was_running: running,
            },
        )?;
        // Un service déjà désactivé par l'utilisateur reste désactivé.
        let target = if start == StartKind::Disabled {
            StartKind::Disabled
        } else {
            StartKind::Manual
        };
        set_service_state(self.service, target, false)?;
        Ok(vec![UndoAction::Service {
            name: self.service.to_owned(),
            start,
            was_running: running,
        }])
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let backups = state.backups(SERVICES_BACKUP);
        let original = backups.get::<ServiceBackup>(self.service)?.unwrap_or(ServiceBackup {
            start: self.default_start,
            was_running: matches!(self.default_start, StartKind::Auto | StartKind::DelayedAuto),
        });
        let start = read_start(self.service)?;
        let running = is_running(self.service)?;
        set_service_state(self.service, original.start, original.was_running)?;
        backups.remove(self.service)?;
        Ok(vec![UndoAction::Service {
            name: self.service.to_owned(),
            start,
            was_running: running,
        }])
    }
}

/// Rétablit la configuration d'origine de tous les services sauvegardés.
///
/// Impact Windows : restaure type de démarrage et état de chaque service listé dans
/// `aether_services_backup.json`, puis supprime les entrées restaurées.
pub fn restore_all(state: &AppState) -> AetherResult<Vec<String>> {
    let backups = state.backups(SERVICES_BACKUP);
    let mut restored = Vec::new();
    let mut actions = Vec::new();
    for name in backups.keys()? {
        let Some(original) = backups.get::<ServiceBackup>(&name)? else {
            continue;
        };
        if state.dry_run() {
            state.logger.log(
                crate::core::logger::LogLevel::Preview,
                "services",
                format!("[APERÇU] Restauration de {name}"),
            );
            continue;
        }
        let start = read_start(&name)?;
        let running = is_running(&name)?;
        set_service_state(&name, original.start, original.was_running)?;
        actions.push(UndoAction::Service {
            name: name.clone(),
            start,
            was_running: running,
        });
        backups.remove(&name)?;
        state.logger.success("services", format!("[Restauré] {name}"));
        restored.push(name);
    }
    state.record("services.restoreAll", "Configuration d'origine des services", actions)?;
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_real_state_of_a_core_service() -> AetherResult<()> {
        // Le service « EventLog » existe sur tout Windows : lecture seule, aucune modification.
        assert!(is_installed("EventLog"));
        let start = read_start("EventLog")?;
        assert!(matches!(start, StartKind::Auto | StartKind::DelayedAuto));
        assert!(is_running("EventLog")?);
        Ok(())
    }

    #[test]
    fn missing_service_is_unavailable() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        let t = ServiceTweak {
            id: "svc.test",
            service: "AetherServiceQuiNExistePas",
            risk: Risk::Safe,
            default_start: StartKind::Manual,
        };
        assert!(!t.check_status(&state)?.available);
        Ok(())
    }

    #[test]
    fn system_services_are_never_reconfigured() {
        assert!(sc_start_arg(StartKind::System).is_err());
    }
}
