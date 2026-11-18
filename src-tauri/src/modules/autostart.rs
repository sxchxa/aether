//! Lancement d'Aether à l'ouverture de session, réduit dans la zone de notification.
//!
//! Aether demande les droits administrateur : une entrée « Run » classique déclencherait une
//! invite UAC à chaque connexion. Une tâche planifiée « à l'ouverture de session » avec les
//! privilèges les plus élevés démarre Aether sans invite, comme le font les outils système.

use tauri::State;

use crate::core::command;
use crate::error::{AetherError, AetherResult};
use crate::modules::{blocking, Shared};

/// Nom de la tâche dans le Planificateur de tâches (dossier « Aether »).
pub const TASK: &str = "Aether\\Démarrage";
/// Argument : démarrer réduit dans la zone de notification.
pub const MINIMIZED_ARG: &str = "--minimized";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Définition XML de la tâche : à l'ouverture de session de l'utilisateur, 10 s de délai, sans limite
/// de durée (Aether reste ouvert dans la zone de notification), y compris sur batterie.
pub fn task_xml(exe: &str, user: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Aether : démarrage réduit dans la zone de notification.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId><Delay>PT10S</Delay></LogonTrigger></Triggers>
  <Principals>
    <Principal id="Author"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author"><Exec><Command>{exe}</Command><Arguments>{MINIMIZED_ARG}</Arguments></Exec></Actions>
</Task>"#,
        exe = xml_escape(exe),
        user = xml_escape(user),
    )
}

/// La tâche de démarrage existe.
///
/// Impact Windows : lecture seule (`schtasks /query`).
pub fn enabled() -> bool {
    command::run_raw("schtasks.exe", &["/query", "/tn", TASK]).is_ok_and(|o| o.code == Some(0))
}

/// Active ou désactive le lancement au démarrage.
///
/// Impact Windows : `schtasks /create /xml` (tâche « Aether\Démarrage ») ou `schtasks /delete`.
pub fn set(on: bool) -> AetherResult<()> {
    if !on {
        if enabled() {
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
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(task_xml(&exe, &user).encode_utf16().flat_map(u16::to_le_bytes));
    let file = std::env::temp_dir().join(format!("aether-autostart-{}.xml", std::process::id()));
    std::fs::write(&file, bytes)?;
    let res = command::run(
        "schtasks.exe",
        &["/create", "/tn", TASK, "/xml", &file.display().to_string(), "/f"],
    );
    let _ = std::fs::remove_file(&file);
    res.map(|_| ())
}

/// Lancement au démarrage actif.
#[tauri::command]
pub async fn autostart_get(state: State<'_, Shared>) -> AetherResult<bool> {
    blocking(&state, |_| Ok(enabled())).await
}

/// Active ou désactive le lancement au démarrage.
#[tauri::command]
pub async fn autostart_set(state: State<'_, Shared>, on: bool) -> AetherResult<bool> {
    blocking(&state, move |s| {
        set(on)?;
        s.logger.info(
            "autostart",
            if on {
                "Lancement au démarrage activé"
            } else {
                "Lancement au démarrage désactivé"
            },
        );
        Ok(enabled())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_starts_minimized_without_time_limit() {
        let xml = task_xml("C:\\Program Files\\Aether\\aether.exe", "PC\\Alex");
        assert!(xml.contains("<Arguments>--minimized</Arguments>"));
        assert!(xml.contains("<LogonTrigger>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
    }
}
