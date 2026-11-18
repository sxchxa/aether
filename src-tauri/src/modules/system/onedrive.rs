//! Détection et désinstallation propre de OneDrive.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::command;
use crate::core::logger::LogLevel;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};

const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OneDriveSetup.exe";

/// État de OneDrive.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveStatus {
    /// OneDrive est installé.
    pub installed: bool,
    /// OneDrive est en cours d'exécution.
    pub running: bool,
    /// Programme de désinstallation trouvé.
    pub uninstaller: Option<String>,
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn system_setup() -> Option<PathBuf> {
    let win = env_dir("WINDIR")?;
    [
        win.join("SysWOW64\\OneDriveSetup.exe"),
        win.join("System32\\OneDriveSetup.exe"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Lit l'état de OneDrive.
///
/// Impact Windows : lecture seule.
pub fn status() -> AetherResult<OneDriveStatus> {
    let exe_candidates = [
        env_dir("LOCALAPPDATA").map(|p| p.join("Microsoft\\OneDrive\\OneDrive.exe")),
        env_dir("ProgramFiles").map(|p| p.join("Microsoft OneDrive\\OneDrive.exe")),
        env_dir("ProgramFiles(x86)").map(|p| p.join("Microsoft OneDrive\\OneDrive.exe")),
    ];
    let exe_present = exe_candidates.iter().flatten().any(|p| p.is_file());
    let registered = registry::key_exists(Hive::Hkcu, UNINSTALL_KEY);
    let running = !crate::modules::storage::processes::running_among(&["OneDrive.exe"]).is_empty();
    let uninstaller = system_setup().map(|p| p.display().to_string()).or(
        match registry::read(Hive::Hkcu, UNINSTALL_KEY, "UninstallString")? {
            Some(RegData::Sz(s)) => Some(s),
            _ => None,
        },
    );
    Ok(OneDriveStatus {
        installed: exe_present || registered,
        running,
        uninstaller,
    })
}

/// Désinstalle OneDrive (arrêt du processus puis `OneDriveSetup.exe /uninstall`).
///
/// Impact Windows : supprime l'application OneDrive. Les fichiers locaux du dossier OneDrive
/// sont conservés. Réinstallable depuis microsoft.com.
pub fn uninstall(state: &AppState) -> AetherResult<bool> {
    let st = status()?;
    if !st.installed {
        state.logger.log(
            LogLevel::Skip,
            "onedrive",
            "[OK - Déjà optimisé] OneDrive n'est pas installé",
        );
        return Ok(false);
    }
    if state.dry_run() {
        state
            .logger
            .log(LogLevel::Preview, "onedrive", "[APERÇU] OneDriveSetup.exe /uninstall");
        return Ok(false);
    }
    // Bureau, Documents ou Images synchronisés dans OneDrive : désinstaller rendrait ces fichiers
    // (souvent stockés seulement en ligne) inaccessibles. On refuse et on explique.
    if let Some(folder) = redirected_folder() {
        return Err(AetherError::Forbidden(format!(
            "votre dossier « {folder} » est synchronisé dans OneDrive. Désactivez d'abord la sauvegarde des dossiers dans OneDrive (Paramètres > Synchroniser et sauvegarder) puis réessayez"
        )));
    }
    let _ = command::run_raw("taskkill.exe", &["/F", "/IM", "OneDrive.exe"]);
    if let Some(setup) = system_setup() {
        command::run_with_timeout(
            &setup.display().to_string(),
            &["/uninstall"],
            std::time::Duration::from_secs(300),
        )?;
    } else {
        let cmdline = st
            .uninstaller
            .ok_or_else(|| AetherError::NotFound("programme de désinstallation OneDrive".into()))?;
        let (exe, args) = split_command_line(&cmdline);
        // La commande vient de HKCU (modifiable sans droits administrateur) : seul un OneDriveSetup.exe
        // situé dans un dossier d'installation officiel de OneDrive est exécuté.
        if !exe.to_lowercase().ends_with("onedrivesetup.exe") || !is_official_onedrive_path(Path::new(&exe)) {
            return Err(AetherError::Forbidden(format!("désinstalleur inattendu : {exe}")));
        }
        let args: Vec<&str> = args.split_whitespace().collect();
        command::run_with_timeout(&exe, &args, std::time::Duration::from_secs(300))?;
    }
    state.logger.success("onedrive", "[Désinstallé] OneDrive");
    Ok(true)
}

/// Dossier personnel (Bureau, Documents, Images) redirigé vers OneDrive, s'il y en a un.
///
/// Impact Windows : lecture seule (Registre `User Shell Folders`).
pub fn redirected_folder() -> Option<&'static str> {
    const KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders";
    [
        ("Desktop", "Bureau"),
        ("Personal", "Documents"),
        ("My Pictures", "Images"),
    ]
    .into_iter()
    .find(|(value, _)| {
        registry::read_text(Hive::Hkcu, KEY, value)
            .ok()
            .flatten()
            .is_some_and(|p| p.to_lowercase().contains("onedrive"))
    })
    .map(|(_, label)| label)
}

/// Vrai si l'exécutable se trouve réellement dans un dossier d'installation officiel de OneDrive.
fn is_official_onedrive_path(exe: &Path) -> bool {
    let Ok(real) = std::fs::canonicalize(exe) else {
        return false;
    };
    let roots = [
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Microsoft\\OneDrive")),
        std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("Microsoft OneDrive")),
        std::env::var_os("ProgramFiles(x86)").map(|p| PathBuf::from(p).join("Microsoft OneDrive")),
        std::env::var_os("WINDIR").map(|p| PathBuf::from(p).join("System32")),
        std::env::var_os("WINDIR").map(|p| PathBuf::from(p).join("SysWOW64")),
    ];
    roots
        .into_iter()
        .flatten()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .any(|r| real.starts_with(r))
}
/// Sépare `"C:\chemin\prog.exe" --args` en (exécutable, arguments).
pub fn split_command_line(line: &str) -> (String, String) {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_owned(), rest[end + 1..].trim().to_owned());
        }
    }
    match line.find(".exe") {
        Some(i) => (line[..i + 4].to_owned(), line[i + 4..].trim().to_owned()),
        None => (line.to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_and_unquoted_command_lines() {
        assert_eq!(
            split_command_line("\"C:\\A B\\setup.exe\" --uninstall --msedge"),
            ("C:\\A B\\setup.exe".to_owned(), "--uninstall --msedge".to_owned())
        );
        assert_eq!(
            split_command_line("C:\\x\\OneDriveSetup.exe  /uninstall /allusers"),
            ("C:\\x\\OneDriveSetup.exe".to_owned(), "/uninstall /allusers".to_owned())
        );
    }
}
