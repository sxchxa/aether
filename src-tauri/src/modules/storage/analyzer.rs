//! Analyseur d'espace : fichiers de plus de 100 Mo (hors `C:\Windows`).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use sysinfo::Disks;
use walkdir::WalkDir;

use crate::core::command;
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};
use crate::modules::storage::rules::is_protected;

/// Seuil par défaut : 100 Mo.
pub const DEFAULT_MIN_BYTES: u64 = 100 * 1024 * 1024;
const MAX_RESULTS: usize = 300;
const SYSTEM_FILES: &[&str] = &["pagefile.sys", "hiberfil.sys", "swapfile.sys", "dumpstack.log"];
const SKIPPED_DIRS: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "$windows.~bt",
    "$windows.~ws",
];

/// Fichier volumineux détecté.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFile {
    /// Chemin complet.
    pub path: String,
    /// Taille (octets).
    pub size: u64,
    /// Date de modification (ms Unix).
    pub modified: u64,
}

/// Progression de l'analyse (diffusée via `tauri::ipc::Channel`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerProgress {
    /// Fichiers parcourus.
    pub files_scanned: u64,
    /// Fichiers volumineux trouvés.
    pub found: u64,
}

fn windir() -> PathBuf {
    std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
}

fn is_under(path: &Path, base: &Path) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    let b = base.to_string_lossy().to_lowercase();
    p == b || p.starts_with(&format!("{}\\", b.trim_end_matches('\\')))
}

fn excluded_dir(path: &Path, win: &Path) -> bool {
    if is_under(path, win) {
        return true;
    }
    path.file_name()
        .map(|n| SKIPPED_DIRS.contains(&n.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Analyse `roots` à la recherche de fichiers ≥ `min_bytes`, hors dossier Windows.
///
/// Impact Windows : lecture seule ; les dossiers inaccessibles sont ignorés.
pub fn scan<F: FnMut(AnalyzerProgress)>(roots: &[PathBuf], min_bytes: u64, mut on_progress: F) -> Vec<LargeFile> {
    let win = windir();
    let mut found: Vec<LargeFile> = Vec::new();
    let mut scanned = 0u64;
    for root in roots {
        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !(e.file_type().is_dir() && excluded_dir(e.path(), &win)));
        for entry in walker.filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            scanned += 1;
            if scanned % 5000 == 0 {
                on_progress(AnalyzerProgress {
                    files_scanned: scanned,
                    found: found.len() as u64,
                });
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if SYSTEM_FILES.contains(&name.as_str()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.len() >= min_bytes {
                found.push(LargeFile {
                    path: entry.path().to_string_lossy().into_owned(),
                    size: meta.len(),
                    modified: meta
                        .modified()
                        .ok()
                        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                });
            }
        }
    }
    on_progress(AnalyzerProgress {
        files_scanned: scanned,
        found: found.len() as u64,
    });
    found.sort_by_key(|f| std::cmp::Reverse(f.size));
    found.truncate(MAX_RESULTS);
    found
}

/// Lecteurs fixes du PC (racines d'analyse par défaut).
pub fn fixed_drives() -> Vec<PathBuf> {
    Disks::new_with_refreshed_list()
        .map(|list| {
            list.iter()
                .filter(|d| !d.is_removable())
                .map(|d| d.mount_point().to_path_buf())
                .collect()
        })
        .unwrap_or_default()
}

/// Dossiers où l'envoi à la Corbeille est toujours refusé (sur n'importe quel lecteur).
const PROTECTED_DIRS: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "system volume information",
    "$recycle.bin",
    "recovery",
    "boot",
];

/// Vrai si le chemin (canonique) se trouve dans un dossier système, quel que soit le lecteur.
fn in_protected_dir(p: &Path) -> bool {
    // Composants après le préfixe de lecteur (\\?\C:) et la racine.
    let parts: Vec<String> = p
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    parts
        .first()
        .is_some_and(|first| PROTECTED_DIRS.contains(&first.as_str()))
}

fn validate_user_file(path: &str) -> AetherResult<PathBuf> {
    let raw = PathBuf::from(path);
    if !raw.is_absolute() {
        return Err(AetherError::NotFound(path.to_owned()));
    }
    // Chemin réel : résout .., noms courts 8.3, jonctions et liens symboliques avant tout contrôle.
    let p = std::fs::canonicalize(&raw).map_err(|_| AetherError::NotFound(path.to_owned()))?;
    let meta = std::fs::symlink_metadata(&p).map_err(|_| AetherError::NotFound(path.to_owned()))?;
    if !meta.is_file() {
        return Err(AetherError::NotFound(path.to_owned()));
    }
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_SYSTEM (0x4) : fichier géré par Windows.
        if meta.file_attributes() & 0x4 != 0 {
            return Err(AetherError::Forbidden(format!("{path} est un fichier système protégé")));
        }
    }
    if in_protected_dir(&p) || is_under(&p, &windir()) || is_protected(&p) {
        return Err(AetherError::Forbidden(format!("{path} est un fichier système protégé")));
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if SYSTEM_FILES.contains(&name.as_str()) {
        return Err(AetherError::Forbidden(format!("{path} est géré par Windows")));
    }
    Ok(p)
}

/// Envoie un fichier volumineux à la Corbeille (Mode Aperçu respecté).
///
/// Impact Windows : déplace le fichier vers la Corbeille (récupérable).
pub fn trash_file(state: &AppState, path: &str) -> AetherResult<bool> {
    let p = validate_user_file(path)?;
    if state.dry_run() {
        state.logger.log(
            crate::core::logger::LogLevel::Preview,
            "analyzer",
            format!("[APERÇU] Corbeille : {path}"),
        );
        return Ok(false);
    }
    // Le fichier validé (chemin réel) est celui qui part à la Corbeille.
    trash::delete(&p).map_err(|e| AetherError::Io(std::io::Error::other(e.to_string())))?;
    state.logger.success("analyzer", format!("[Corbeille] {path}"));
    Ok(true)
}

/// Ouvre l'Explorateur sur le fichier sélectionné.
///
/// Impact Windows : lance `explorer.exe /select,<chemin>` (lecture seule).
pub fn reveal(path: &str) -> AetherResult<()> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(AetherError::NotFound(path.to_owned()));
    }
    command::hidden("explorer.exe")
        .arg(format!("/select,{}", p.display()))
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_only_large_files_in_sandbox() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("petit.bin"), vec![0u8; 10])?;
        let big = fs::File::create(dir.path().join("gros.iso"))?;
        big.set_len(2048)?;
        let mut progress_calls = 0;
        let res = scan(&[dir.path().to_path_buf()], 1024, |_| progress_calls += 1);
        assert_eq!(res.len(), 1);
        assert!(res[0].path.ends_with("gros.iso"));
        assert!(progress_calls >= 1);
        Ok(())
    }

    #[test]
    fn windows_folder_is_excluded() {
        let win = windir();
        assert!(excluded_dir(&win.join("System32"), &win));
        assert!(!excluded_dir(Path::new("C:\\Users"), &win));
        assert!(validate_user_file(&win.join("notepad.exe").to_string_lossy()).is_err());
    }

    #[test]
    fn blocks_system_folders_on_every_drive() {
        assert!(in_protected_dir(Path::new("C:\\Windows\\System32\\drivers\\x.sys")));
        assert!(in_protected_dir(Path::new("D:\\Windows\\explorer.exe")));
        assert!(in_protected_dir(Path::new("C:\\Program Files\\App\\big.bin")));
        assert!(in_protected_dir(Path::new("\\\\?\\C:\\ProgramData\\cache.db")));
        assert!(!in_protected_dir(Path::new("C:\\Users\\Alex\\Videos\\film.mkv")));
        assert!(!in_protected_dir(Path::new("D:\\Archives\\backup.iso")));
    }

    #[test]
    fn refuses_traversal_to_system_files() {
        // `..` vers Windows : le chemin réel est résolu puis refusé.
        let sneaky = format!(
            "{}\\..\\Windows\\notepad.exe",
            std::env::var("USERPROFILE").unwrap_or_default()
        );
        assert!(validate_user_file(&sneaky).is_err());
        assert!(validate_user_file("C:\\Windows\\notepad.exe").is_err());
    }
}
