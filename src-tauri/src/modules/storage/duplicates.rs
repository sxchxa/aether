//! Recherche de fichiers en double dans les dossiers personnels (Bureau, Documents, Téléchargements,
//! Images, Vidéos, Musique). Comparaison par contenu réel (SHA-256), en trois passes rapides :
//! taille identique → empreinte du début du fichier → empreinte complète.
//! Lecture seule : la suppression passe par la Corbeille, fichier par fichier, depuis l'interface.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;
use walkdir::WalkDir;

use crate::error::AetherResult;
use crate::modules::{blocking, Shared};

/// Taille minimale prise en compte (les petits fichiers libèrent peu de place).
pub const MIN_SIZE: u64 = 1024 * 1024;
/// Nombre maximal de fichiers examinés (garde-fou sur les très gros profils).
const MAX_FILES: usize = 300_000;
/// Nombre maximal de groupes renvoyés.
const MAX_GROUPS: usize = 200;

/// Fichier d'un groupe de doublons.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    /// Chemin complet.
    pub path: String,
    /// Date de modification (ms Unix).
    pub modified: u64,
}

/// Groupe de fichiers au contenu identique.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// Taille d'un fichier (octets).
    pub size: u64,
    /// Fichiers identiques (le plus récent en premier).
    pub files: Vec<DuplicateFile>,
    /// Place récupérable en ne gardant qu'un exemplaire.
    pub wasted: u64,
}

/// Dossiers personnels analysés.
pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };
    ["Desktop", "Documents", "Downloads", "Pictures", "Videos", "Music"]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| p.is_dir())
        .collect()
}

fn hash(path: &Path, limit: Option<u64>) -> Option<[u8; 32]> {
    let file = File::open(path).ok()?;
    let mut reader: Box<dyn Read> = match limit {
        Some(n) => Box::new(file.take(n)),
        None => Box::new(file),
    };
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = reader.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize().into())
}

fn modified_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Cherche les doublons sous les dossiers donnés (liens symboliques et jonctions ignorés).
pub fn find(roots: &[PathBuf], min_size: u64) -> Vec<DuplicateGroup> {
    // 1. Regroupement par taille.
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut seen = 0usize;
    for root in roots {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            seen += 1;
            if seen > MAX_FILES {
                break;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.len() >= min_size {
                by_size.entry(meta.len()).or_default().push(entry.into_path());
            }
        }
    }
    let mut groups = Vec::new();
    for (size, paths) in by_size.into_iter().filter(|(_, p)| p.len() > 1) {
        // 2. Empreinte des 64 premiers Ko (élimine vite les faux candidats).
        let mut by_head: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
        for p in paths {
            if let Some(h) = hash(&p, Some(64 * 1024)) {
                by_head.entry(h).or_default().push(p);
            }
        }
        // 3. Empreinte complète des candidats restants.
        for candidates in by_head.into_values().filter(|p| p.len() > 1) {
            let mut by_full: HashMap<[u8; 32], Vec<PathBuf>> = HashMap::new();
            for p in candidates {
                if let Some(h) = hash(&p, None) {
                    by_full.entry(h).or_default().push(p);
                }
            }
            for same in by_full.into_values().filter(|p| p.len() > 1) {
                let mut files: Vec<DuplicateFile> = same
                    .iter()
                    .map(|p| DuplicateFile {
                        path: p.display().to_string(),
                        modified: modified_ms(p),
                    })
                    .collect();
                files.sort_by_key(|f| std::cmp::Reverse(f.modified));
                let wasted = size * (files.len() as u64 - 1);
                groups.push(DuplicateGroup { size, files, wasted });
            }
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.wasted));
    groups.truncate(MAX_GROUPS);
    groups
}

/// Doublons dans les dossiers personnels.
///
/// Impact Windows : lecture seule.
#[tauri::command]
pub async fn storage_duplicates(state: State<'_, Shared>) -> AetherResult<Vec<DuplicateGroup>> {
    blocking(&state, |_| Ok(find(&default_roots(), MIN_SIZE))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_identical_content() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let a = vec![7u8; 200_000];
        let mut b = a.clone();
        // Même taille et même début que `a`, mais contenu différent à la fin : pas un doublon.
        if let Some(last) = b.last_mut() {
            *last = 8;
        }
        std::fs::create_dir_all(dir.path().join("sub"))?;
        std::fs::write(dir.path().join("photo.jpg"), &a)?;
        std::fs::write(dir.path().join("sub").join("photo (1).jpg"), &a)?;
        std::fs::write(dir.path().join("autre.jpg"), &b)?;
        std::fs::write(dir.path().join("petit.txt"), b"x")?;
        let groups = find(&[dir.path().to_path_buf()], 1024);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        assert_eq!(groups[0].wasted, 200_000);
        Ok(())
    }
}
