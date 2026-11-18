//! Détection (via `sysinfo`) des navigateurs/applications ouverts avant nettoyage.

use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use crate::error::{AetherError, AetherResult};
use crate::modules::storage::rules::known_processes;

/// Application ouverte qui bloque un nettoyage complet.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    /// Nom de l'exécutable (ex. `chrome.exe`).
    pub process: String,
    /// Nombre d'instances.
    pub instances: u32,
}

fn snapshot() -> AetherResult<System> {
    let mut sys = System::new_with_specifics(RefreshKind::nothing())?;
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    Ok(sys)
}

/// Liste, parmi `names`, les processus actuellement ouverts.
///
/// Impact Windows : lecture seule de la liste des processus.
pub fn running_among(names: &[&str]) -> Vec<RunningApp> {
    let Ok(sys) = snapshot() else {
        return Vec::new();
    };
    names
        .iter()
        .filter_map(|wanted| {
            let count = sys
                .processes()
                .values()
                .filter(|p| p.name().to_string_lossy().eq_ignore_ascii_case(wanted))
                .count() as u32;
            (count > 0).then(|| RunningApp {
                process: (*wanted).to_owned(),
                instances: count,
            })
        })
        .collect()
}

/// Ferme les applications demandées (uniquement celles connues des règles de nettoyage).
///
/// Impact Windows : termine les processus ciblés ; les onglets non enregistrés peuvent être perdus
/// (l'interface le signale avant confirmation).
pub fn close(names: &[String]) -> AetherResult<u32> {
    let allowed = known_processes();
    for n in names {
        if !allowed.iter().any(|a| a.eq_ignore_ascii_case(n)) {
            return Err(AetherError::Forbidden(format!(
                "fermeture de « {n} » non autorisée (hors liste des applications nettoyables)"
            )));
        }
    }
    let sys = snapshot()?;
    let mut killed = 0u32;
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy();
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) && process.kill() {
            killed += 1;
        }
    }
    Ok(killed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_to_close_unknown_processes() {
        assert!(close(&["explorer.exe".to_owned()]).is_err());
        assert!(close(&["lsass.exe".to_owned()]).is_err());
    }

    #[test]
    fn detects_a_running_system_process() {
        let found = running_among(&["svchost.exe", "aether-introuvable.exe"]);
        assert_eq!(found.len(), 1);
        assert!(found[0].instances > 0);
    }
}
