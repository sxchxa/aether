//! Moteur de scan et de nettoyage : suppression directe et parallèle des fichiers temporaires
//! et caches (comme le Nettoyage de disque de Windows) ; Corbeille disponible via `trash`.
//!
//! - Les fichiers verrouillés (`OS Error 32`) ou inaccessibles (`PermissionDenied`) sont ignorés.
//! - Les fichiers récents (âge < `min_age_minutes`) sont ignorés.
//! - Les données de navigation protégées ne sont jamais touchées.
//! - Les dossiers racines des règles ne sont jamais supprimés, seulement vidés.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use walkdir::WalkDir;

use crate::modules::storage::rules::{is_protected, CleanCategory, CleanRule};

/// Méthode d'élimination des fichiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposal {
    /// Envoi à la Corbeille Windows (récupérable).
    RecycleBin,
    /// Suppression directe (fichiers temporaires et caches).
    Permanent,
}

/// Résultat de scan d'une règle.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Identifiant de la règle.
    pub id: String,
    /// Catégorie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<CleanCategory>,
    /// Octets récupérables.
    pub bytes: u64,
    /// Nombre de fichiers récupérables.
    pub files: u64,
    /// Fichiers ignorés (verrouillés, récents, protégés, inaccessibles).
    pub skipped: u64,
}

/// Résultat de nettoyage d'une règle.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanResult {
    /// Identifiant de la règle.
    pub id: String,
    /// Octets libérés.
    pub freed: u64,
    /// Fichiers éliminés.
    pub removed: u64,
    /// Fichiers ignorés (en cours d'utilisation, accès refusé…).
    pub skipped: u64,
}

struct Candidate {
    path: PathBuf,
    size: u64,
}

fn eligible(path: &Path, meta: &fs::Metadata, rule: &CleanRule, cutoff: Option<SystemTime>) -> bool {
    if is_protected(path) {
        return false;
    }
    if !rule.file_prefixes.is_empty() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !rule.file_prefixes.iter().any(|p| name.starts_with(p)) {
            return false;
        }
    }
    if let Some(cutoff) = cutoff {
        if meta.modified().map(|m| m > cutoff).unwrap_or(true) {
            return false;
        }
    }
    true
}

fn collect(rule: &CleanRule) -> (Vec<Candidate>, u64) {
    let cutoff = (rule.min_age_minutes > 0)
        .then(|| SystemTime::now().checked_sub(Duration::from_secs(rule.min_age_minutes * 60)))
        .flatten();
    let mut out = Vec::new();
    let mut skipped = 0u64;
    for root in &rule.roots {
        if is_protected(root) {
            continue;
        }
        for entry in WalkDir::new(root).follow_links(false).min_depth(1) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    skipped += 1; // Accès refusé / fichier verrouillé : ignoré.
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            match entry.metadata() {
                Ok(meta) if eligible(entry.path(), &meta, rule, cutoff) => out.push(Candidate {
                    path: entry.into_path(),
                    size: meta.len(),
                }),
                _ => skipped += 1,
            }
        }
    }
    (out, skipped)
}

/// Analyse une règle sans rien modifier.
///
/// Impact Windows : lecture seule du système de fichiers.
pub fn scan_rule(rule: &CleanRule) -> ScanResult {
    let (candidates, skipped) = collect(rule);
    ScanResult {
        id: rule.id.clone(),
        category: Some(rule.category),
        bytes: candidates.iter().map(|c| c.size).sum(),
        files: candidates.len() as u64,
        skipped,
    }
}

fn dispose_one(path: &Path, disposal: Disposal) -> bool {
    match disposal {
        Disposal::Permanent => remove_file(path),
        Disposal::RecycleBin => trash::delete(path).is_ok(),
    }
}

fn dispose_batch(batch: &[Candidate], disposal: Disposal, result: &mut CleanResult) {
    if disposal == Disposal::RecycleBin {
        // Envoi groupé (rapide) ; en cas d'échec (fichier verrouillé), repli fichier par fichier.
        if trash::delete_all(batch.iter().map(|c| &c.path)).is_ok() {
            result.removed += batch.len() as u64;
            result.freed += batch.iter().map(|c| c.size).sum::<u64>();
            return;
        }
    }
    for c in batch {
        // Un envoi groupé partiellement réussi a pu déjà déplacer ce fichier.
        if !c.path.exists() || dispose_one(&c.path, disposal) {
            result.removed += 1;
            result.freed += c.size;
        } else {
            result.skipped += 1;
        }
    }
}

/// Supprime un fichier, y compris en lecture seule ; un fichier verrouillé reste en place.
fn remove_file(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => fs::metadata(path)
            .map(|m| {
                let mut perms = m.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                fs::set_permissions(path, perms).is_ok() && fs::remove_file(path).is_ok()
            })
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Suppression directe en parallèle (fichiers temporaires et caches : rien à récupérer).
/// Des dizaines de milliers de petits fichiers sont traités en quelques secondes, là où la
/// Corbeille de Windows demande ~8 ms par fichier.
fn remove_parallel(candidates: &[Candidate], result: &mut CleanResult) {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8);
    let chunk = candidates.len().div_ceil(workers).max(1);
    let partials: Vec<CleanResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = candidates
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    let mut r = CleanResult::default();
                    for c in part {
                        if remove_file(&c.path) {
                            r.removed += 1;
                            r.freed += c.size;
                        } else {
                            r.skipped += 1;
                        }
                    }
                    r
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    for p in partials {
        result.removed += p.removed;
        result.freed += p.freed;
        result.skipped += p.skipped;
    }
}

/// Retire les dossiers vides **anciens** : un dossier récent (même vide) peut être en cours
/// d'utilisation par un programme (installateur, verrou…) et n'est jamais touché.
fn prune_empty_dirs(root: &Path, min_age_minutes: u64) {
    let cutoff = SystemTime::now().checked_sub(Duration::from_secs(min_age_minutes.max(60) * 60));
    let dirs: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .map(|e| e.into_path())
        .collect();
    for dir in dirs {
        let recent = match (cutoff, fs::metadata(&dir).and_then(|m| m.modified())) {
            (Some(cutoff), Ok(modified)) => modified > cutoff,
            _ => true,
        };
        if !recent && !is_protected(&dir) {
            // Échoue silencieusement si le dossier n'est pas vide : comportement voulu.
            let _ = fs::remove_dir(&dir);
        }
    }
}

/// Nettoie une règle : envoie les fichiers éligibles à la Corbeille (ou les supprime en test).
///
/// Impact Windows : déplace des fichiers de cache/temporaires vers la Corbeille ; les fichiers
/// en cours d'utilisation sont ignorés sans erreur.
pub fn clean_rule(rule: &CleanRule, disposal: Disposal) -> CleanResult {
    let (candidates, skipped) = collect(rule);
    let mut result = CleanResult {
        id: rule.id.clone(),
        skipped,
        ..CleanResult::default()
    };
    match disposal {
        Disposal::Permanent => remove_parallel(&candidates, &mut result),
        Disposal::RecycleBin => {
            for batch in candidates.chunks(400) {
                dispose_batch(batch, disposal, &mut result);
            }
        }
    }
    for root in &rule.roots {
        prune_empty_dirs(root, rule.min_age_minutes);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tweak::Risk;
    use std::io::Write;

    fn sandbox_rule(root: &Path, min_age: u64) -> CleanRule {
        CleanRule {
            id: "test.sandbox".into(),
            category: CleanCategory::System,
            risk: Risk::Safe,
            roots: vec![root.to_path_buf()],
            file_prefixes: Vec::new(),
            min_age_minutes: min_age,
            processes: Vec::new(),
            default_selected: true,
        }
    }

    fn write_file(path: &Path, size: usize) -> std::io::Result<()> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::File::create(path)?.write_all(&vec![0u8; size])
    }

    #[test]
    fn scan_counts_files_and_skips_protected() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.tmp"), 100)?;
        write_file(&dir.path().join("sub\\b.tmp"), 50)?;
        write_file(&dir.path().join("Default\\Cookies"), 999)?;
        write_file(&dir.path().join("Default\\Local Storage\\leveldb\\x.ldb"), 999)?;
        let res = scan_rule(&sandbox_rule(dir.path(), 0));
        assert_eq!(res.files, 2);
        assert_eq!(res.bytes, 150);
        assert_eq!(res.skipped, 2);
        Ok(())
    }

    #[test]
    fn recent_files_are_ignored() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("fresh.tmp"), 10)?;
        let res = scan_rule(&sandbox_rule(dir.path(), 60));
        assert_eq!(res.files, 0);
        assert_eq!(res.skipped, 1);
        Ok(())
    }

    #[test]
    fn clean_removes_files_but_keeps_root_and_protected() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.tmp"), 100)?;
        write_file(&dir.path().join("nested\\deep\\b.tmp"), 20)?;
        write_file(&dir.path().join("Default\\Login Data"), 5)?;
        let res = clean_rule(&sandbox_rule(dir.path(), 0), Disposal::Permanent);
        assert_eq!(res.removed, 2);
        assert_eq!(res.freed, 120);
        assert!(dir.path().exists());
        // Dossiers vides tout juste créés : conservés (un programme peut être en train de s'en servir).
        assert!(dir.path().join("nested\\deep").exists());
        assert!(dir.path().join("Default\\Login Data").exists());
        Ok(())
    }

    #[test]
    fn only_old_empty_folders_are_pruned() -> std::io::Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir()?;
        let old = dir.path().join("old");
        let fresh = dir.path().join("fresh");
        fs::create_dir_all(&old)?;
        fs::create_dir_all(&fresh)?;
        // Vieillit le dossier « old » de deux heures (FILE_FLAG_BACKUP_SEMANTICS pour ouvrir un dossier).
        let handle = fs::OpenOptions::new()
            .write(true)
            .custom_flags(0x0200_0000)
            .open(&old)?;
        handle.set_modified(SystemTime::now() - Duration::from_secs(2 * 3600))?;
        drop(handle);
        prune_empty_dirs(dir.path(), 60);
        assert!(!old.exists(), "un dossier vide ancien doit être retiré");
        assert!(fresh.exists(), "un dossier vide récent doit être conservé");
        Ok(())
    }

    #[test]
    fn locked_files_are_skipped_without_error() -> std::io::Result<()> {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir()?;
        let locked = dir.path().join("locked.tmp");
        write_file(&locked, 10)?;
        write_file(&dir.path().join("free.tmp"), 10)?;
        // share_mode(0) : verrou exclusif → OS Error 32 lors de la suppression.
        let _guard = fs::OpenOptions::new().read(true).share_mode(0).open(&locked)?;
        let res = clean_rule(&sandbox_rule(dir.path(), 0), Disposal::Permanent);
        assert_eq!(res.removed, 1);
        assert!(res.skipped >= 1);
        assert!(locked.exists());
        Ok(())
    }

    #[test]
    fn prefix_filter_limits_targets() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("thumbcache_256.db"), 10)?;
        write_file(&dir.path().join("other.db"), 10)?;
        let mut rule = sandbox_rule(dir.path(), 0);
        rule.file_prefixes = vec!["thumbcache_"];
        assert_eq!(scan_rule(&rule).files, 1);
        Ok(())
    }
}
