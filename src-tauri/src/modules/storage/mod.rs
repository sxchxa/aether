//! Section « Stockage & Caches » : Résidus Système, Données de Navigation,
//! Caches Graphiques & Jeux et Analyseur d'Espace.

pub mod analyzer;
pub mod cleaner;
pub mod duplicates;
pub mod maintenance;
pub mod processes;
pub mod rules;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::core::logger::{human_bytes, LogLevel};
use crate::core::state::AppState;
use crate::error::AetherResult;
use crate::modules::{blocking, Shared};
use analyzer::{AnalyzerProgress, LargeFile};
use cleaner::{CleanResult, Disposal, ScanResult};
use processes::RunningApp;
use rules::CleanRule;

/// Bilan d'un nettoyage.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanSummary {
    /// Détail par règle.
    pub results: Vec<CleanResult>,
    /// Total libéré (octets).
    pub freed: u64,
    /// Fichiers éliminés.
    pub removed: u64,
    /// Fichiers ignorés.
    pub skipped: u64,
    /// Mode Aperçu : rien n'a été supprimé (`freed` = estimation).
    pub preview: bool,
}

fn selected_rules(ids: &[String]) -> Vec<CleanRule> {
    rules::all_rules()
        .into_iter()
        .filter(|r| ids.iter().any(|id| id == &r.id))
        .collect()
}

/// Nettoie les règles demandées (Mode Aperçu respecté).
///
/// Impact Windows : supprime les fichiers de cache/temporaires (verrouillés ou récents ignorés).
pub fn clean(state: &AppState, ids: &[String]) -> CleanSummary {
    let preview = state.dry_run();
    let mut summary = CleanSummary {
        preview,
        ..CleanSummary::default()
    };
    for rule in selected_rules(ids) {
        let result = if preview {
            let scan = cleaner::scan_rule(&rule);
            state.logger.log(
                LogLevel::Preview,
                "storage",
                format!(
                    "[APERÇU] {} : {} fichier(s), {}",
                    rule.id,
                    scan.files,
                    human_bytes(scan.bytes)
                ),
            );
            CleanResult {
                id: rule.id.clone(),
                freed: scan.bytes,
                removed: scan.files,
                skipped: scan.skipped,
            }
        } else {
            let r = cleaner::clean_rule(&rule, Disposal::Permanent);
            state.logger.success(
                "storage",
                format!(
                    "[Nettoyé] {} : {} fichier(s) supprimé(s), {} ignoré(s)",
                    rule.id, r.removed, r.skipped
                ),
            );
            r
        };
        summary.freed += result.freed;
        summary.removed += result.removed;
        summary.skipped += result.skipped;
        summary.results.push(result);
    }
    summary
}

/// Règles de nettoyage disponibles sur ce PC.
#[tauri::command]
pub async fn storage_rules() -> AetherResult<Vec<CleanRule>> {
    Ok(tokio::task::spawn_blocking(rules::all_rules).await?)
}

/// Analyse toutes les règles (lecture seule).
#[tauri::command]
pub async fn storage_scan(state: State<'_, Shared>) -> AetherResult<Vec<ScanResult>> {
    blocking(&state, |s| {
        s.logger.info("storage", "Analyse des caches et résidus…");
        // Chaque règle parcourt ses propres dossiers : analyse en parallèle, ordre conservé.
        let all = rules::all_rules();
        let results: Vec<ScanResult> = std::thread::scope(|scope| {
            let handles: Vec<_> = all.iter().map(|r| scope.spawn(move || cleaner::scan_rule(r))).collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        let total: u64 = results.iter().map(|r| r.bytes).sum();
        s.logger.info(
            "storage",
            format!("Analyse terminée : {} récupérables", human_bytes(total)),
        );
        Ok(results)
    })
    .await
}

/// Applications ouvertes qui bloquent le nettoyage des règles demandées.
#[tauri::command]
pub async fn storage_blockers(ids: Vec<String>) -> AetherResult<Vec<RunningApp>> {
    Ok(tokio::task::spawn_blocking(move || {
        let mut names: Vec<&'static str> = selected_rules(&ids).iter().flat_map(|r| r.processes.clone()).collect();
        names.sort_unstable();
        names.dedup();
        processes::running_among(&names)
    })
    .await?)
}

/// Ferme les applications bloquantes (après confirmation dans la modale).
#[tauri::command]
pub async fn storage_close_apps(state: State<'_, Shared>, processes: Vec<String>) -> AetherResult<u32> {
    blocking(&state, move |s| {
        let killed = processes::close(&processes)?;
        s.logger.info(
            "storage",
            format!("{killed} processus fermé(s) : {}", processes.join(", ")),
        );
        std::thread::sleep(std::time::Duration::from_millis(800));
        Ok(killed)
    })
    .await
}

/// Nettoie les règles sélectionnées.
#[tauri::command]
pub async fn storage_clean(state: State<'_, Shared>, ids: Vec<String>) -> AetherResult<CleanSummary> {
    blocking(&state, move |s| Ok(clean(s, &ids))).await
}

/// Analyse les fichiers de plus de 100 Mo sur les lecteurs fixes (progression en flux).
#[tauri::command]
pub async fn storage_analyze(
    state: State<'_, Shared>,
    min_bytes: Option<u64>,
    on_progress: Channel<AnalyzerProgress>,
) -> AetherResult<Vec<LargeFile>> {
    blocking(&state, move |s| {
        s.logger.info("analyzer", "Recherche des fichiers volumineux…");
        let found = analyzer::scan(
            &analyzer::fixed_drives(),
            min_bytes.unwrap_or(analyzer::DEFAULT_MIN_BYTES),
            |p| {
                let _ = on_progress.send(p);
            },
        );
        s.logger
            .info("analyzer", format!("{} fichier(s) volumineux trouvé(s)", found.len()));
        Ok(found)
    })
    .await
}

/// Envoie un fichier volumineux à la Corbeille.
#[tauri::command]
pub async fn storage_trash_file(state: State<'_, Shared>, path: String) -> AetherResult<bool> {
    blocking(&state, move |s| analyzer::trash_file(s, &path)).await
}

/// Affiche un fichier dans l'Explorateur.
#[tauri::command]
pub async fn storage_reveal(path: String) -> AetherResult<()> {
    analyzer::reveal(&path)
}
