//! Entretien automatique hebdomadaire : une tâche planifiée Windows lance Aether en mode silencieux
//! (`--maintenance`) pour nettoyer les fichiers temporaires sûrs (envoyés à la Corbeille).

use std::path::PathBuf;

use crate::core::command;
use crate::core::history::UndoAction;
use crate::core::state::AppState;
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::{AetherError, AetherResult};
use crate::modules::storage::{self, rules};

/// Nom de la tâche dans le Planificateur de tâches (dossier « Aether »).
pub const TASK: &str = "Aether\\Entretien hebdomadaire";
/// Argument de ligne de commande du mode silencieux.
pub const ARG: &str = "--maintenance";

/// Règles nettoyées par l'entretien automatique : sûres, cochées par défaut, sans application à fermer.
pub fn safe_rule_ids() -> Vec<String> {
    rules::all_rules()
        .into_iter()
        .filter(|r| r.risk == Risk::Safe && r.default_selected && r.processes.is_empty())
        .map(|r| r.id)
        .collect()
}

/// La tâche planifiée existe.
///
/// Impact Windows : lecture seule (`schtasks /query`).
pub fn task_exists() -> bool {
    command::run_raw("schtasks.exe", &["/query", "/tn", TASK]).is_ok_and(|o| o.code == Some(0))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Définition XML de la tâche : chaque dimanche à midi, rattrapée au prochain démarrage si le PC
/// était éteint, jamais sur batterie, limitée à 30 minutes, avec les droits de l'utilisateur.
pub fn task_xml(exe: &str, user: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Aether : nettoyage hebdomadaire des fichiers temporaires (envoyés à la Corbeille).</Description></RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-01-04T12:00:00</StartBoundary>
      <Enabled>true</Enabled>
      <ScheduleByWeek><DaysOfWeek><Sunday /></DaysOfWeek><WeeksInterval>1</WeeksInterval></ScheduleByWeek>
    </CalendarTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>true</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT30M</ExecutionTimeLimit>
    <Priority>7</Priority>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>{ARG}</Arguments></Exec></Actions>
</Task>"#,
        exe = xml_escape(exe),
        user = xml_escape(user),
    )
}

/// Crée ou supprime la tâche planifiée.
///
/// Impact Windows : `schtasks /create /xml` (tâche « Aether\Entretien hebdomadaire ») ou `schtasks /delete`.
pub fn set(enabled: bool) -> AetherResult<()> {
    if !enabled {
        if task_exists() {
            command::run("schtasks.exe", &["/delete", "/tn", TASK, "/f"])?;
        }
        return Ok(());
    }
    let exe = std::env::current_exe()?.display().to_string();
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").map_err(|_| AetherError::NotFound("nom d'utilisateur".into()))?
    );
    // Le Planificateur attend un fichier UTF-16 (avec BOM) pour une déclaration « UTF-16 ».
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(task_xml(&exe, &user).encode_utf16().flat_map(u16::to_le_bytes));
    let file = std::env::temp_dir().join(format!("aether-task-{}.xml", std::process::id()));
    std::fs::write(&file, bytes)?;
    let res = command::run(
        "schtasks.exe",
        &["/create", "/tn", TASK, "/xml", &file.display().to_string(), "/f"],
    );
    let _ = std::fs::remove_file(&file);
    res.map(|_| ())
}

/// Dossier des données d'Aether hors interface (identique à celui de l'application).
fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("app.aether.optimizer"))
}

/// Exécute l'entretien sans interface (appelé par la tâche planifiée), puis l'inscrit à l'historique.
///
/// Impact Windows : envoie à la Corbeille les fichiers temporaires des règles sûres.
pub fn run_headless() {
    let Some(dir) = data_dir() else { return };
    let state = AppState::new(&dir);
    let summary = storage::clean(&state, &safe_rule_ids());
    let last = LastRun {
        ts: crate::core::logger::now_ms(),
        freed: summary.freed,
        removed: summary.removed,
    };
    if let Ok(json) = serde_json::to_string_pretty(&last) {
        let _ = std::fs::write(dir.join(LAST_RUN_FILE), json);
    }
}

const LAST_RUN_FILE: &str = "aether_maintenance.json";

/// Résultat du dernier entretien automatique.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    /// Horodatage (ms Unix).
    pub ts: u64,
    /// Octets libérés.
    pub freed: u64,
    /// Fichiers envoyés à la Corbeille.
    pub removed: u64,
}

/// Dernier entretien automatique effectué, s'il y en a eu un.
pub fn last_run(state: &AppState) -> Option<LastRun> {
    std::fs::read_to_string(state.data_dir.join(LAST_RUN_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// Option « entretien automatique hebdomadaire ».
pub struct MaintenanceTweak;

impl Tweak for MaintenanceTweak {
    fn id(&self) -> &'static str {
        "maint.weekly"
    }
    fn section(&self) -> Section {
        Section::System
    }
    fn group(&self) -> &'static str {
        "maintenance"
    }
    fn risk(&self) -> Risk {
        Risk::Safe
    }
    fn technical(&self) -> String {
        format!("schtasks /create /tn \"{TASK}\" (dimanche 12 h, rattrapée au démarrage) → aether.exe {ARG}")
    }
    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        let on = task_exists();
        Ok(Probe {
            applied: on,
            available: true,
            current: if on { "planifié" } else { "non planifié" }.into(),
            target: "chaque semaine".into(),
        })
    }
    fn apply(&self, _: &AppState) -> AetherResult<Vec<UndoAction>> {
        set(true)?;
        Ok(vec![UndoAction::MaintenanceTask { enabled: false }])
    }
    fn restore(&self, _: &AppState) -> AetherResult<Vec<UndoAction>> {
        set(false)?;
        Ok(vec![UndoAction::MaintenanceTask { enabled: true }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_only_uses_safe_rules() {
        let ids = safe_rule_ids();
        assert!(ids.iter().any(|i| i == "sys.userTemp"));
        let all = rules::all_rules();
        for id in &ids {
            let r = all.iter().find(|r| &r.id == id).expect("règle");
            assert_eq!(r.risk, Risk::Safe);
            assert!(r.processes.is_empty(), "{id} exigerait de fermer une application");
        }
    }

    #[test]
    fn task_xml_is_escaped_and_complete() {
        let xml = task_xml("C:\\A & B\\aether.exe", "PC\\Alex");
        assert!(xml.contains("C:\\A &amp; B\\aether.exe"));
        assert!(xml.contains("<Arguments>--maintenance</Arguments>"));
        assert!(xml.contains("<StartWhenAvailable>true</StartWhenAvailable>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>"));
    }
}
