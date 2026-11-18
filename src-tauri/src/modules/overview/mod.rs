//! Section « Vue d'ensemble » : fiche système, monitoring, protection système,
//! profils guidés, Optimisation Express et export/import.

pub mod measure;
pub mod monitor;
pub mod preflight;
pub mod profiles;
pub mod report;
pub mod system;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::core::tweak::TweakReport;
use crate::error::AetherResult;
use crate::modules::{blocking, Shared};

/// Fiche système complète.
#[tauri::command]
pub async fn system_info() -> AetherResult<system::SystemInfo> {
    tokio::task::spawn_blocking(system::info).await?
}

/// Disques physiques et batterie (lent : WMI).
#[tauri::command]
pub async fn hardware_info() -> AetherResult<system::HardwareExtra> {
    Ok(tokio::task::spawn_blocking(system::hardware).await?)
}

/// Démarre le flux de métriques (remplace un flux existant).
#[tauri::command]
pub async fn monitor_start(
    state: State<'_, Shared>,
    on_frame: Channel<monitor::MetricsFrame>,
    fps: Option<u32>,
) -> AetherResult<()> {
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(previous) = state.monitor_stop.lock()?.replace(stop.clone()) {
        previous.store(true, Ordering::Relaxed);
    }
    monitor::spawn(on_frame, stop, fps.unwrap_or(60));
    Ok(())
}

/// Arrête le flux de métriques.
#[tauri::command]
pub async fn monitor_stop(state: State<'_, Shared>) -> AetherResult<()> {
    if let Some(stop) = state.monitor_stop.lock()?.take() {
        stop.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// État de la protection système.
#[tauri::command]
pub async fn restore_status() -> AetherResult<preflight::RestoreStatus> {
    tokio::task::spawn_blocking(preflight::status).await?
}

/// Active la protection système.
#[tauri::command]
pub async fn restore_enable(state: State<'_, Shared>) -> AetherResult<preflight::RestoreStatus> {
    blocking(&state, preflight::enable).await
}

/// Crée un point de restauration.
#[tauri::command]
pub async fn restore_create(state: State<'_, Shared>, description: String) -> AetherResult<preflight::RestoreStatus> {
    blocking(&state, move |s| preflight::create_point(s, &description)).await
}

/// Liste des points de restauration.
#[tauri::command]
pub async fn restore_points() -> AetherResult<Vec<preflight::RestorePointEntry>> {
    tokio::task::spawn_blocking(preflight::list_points).await?
}

/// Revient à un point de restauration (effectif au redémarrage).
#[tauri::command]
pub async fn restore_to_point(state: State<'_, Shared>, sequence: u32) -> AetherResult<()> {
    blocking(&state, move |s| preflight::restore_to(s, sequence)).await
}

/// Redémarre le PC dans 10 secondes.
#[tauri::command]
pub async fn system_restart() -> AetherResult<()> {
    tokio::task::spawn_blocking(preflight::restart_soon).await?
}

/// Optimisation Express.
#[tauri::command]
pub async fn express_optimize(state: State<'_, Shared>) -> AetherResult<profiles::ExpressReport> {
    blocking(&state, |s| Ok(profiles::express(s))).await
}

/// Applique un profil guidé.
#[tauri::command]
pub async fn profile_apply(
    state: State<'_, Shared>,
    profile: profiles::GuidedProfile,
) -> AetherResult<Vec<TweakReport>> {
    blocking(&state, move |s| Ok(profiles::apply_profile(s, profile))).await
}

/// Options recommandées par un profil guidé.
#[tauri::command]
pub fn profile_tweaks(profile: profiles::GuidedProfile) -> Vec<&'static str> {
    profiles::profile_tweaks(profile).to_vec()
}

/// Mesures « avant / après » enregistrées.
#[tauri::command]
pub async fn measures_get(state: State<'_, Shared>) -> AetherResult<measure::Measures> {
    blocking(&state, |s| Ok(measure::get(s))).await
}

/// Prend une nouvelle mesure.
#[tauri::command]
pub async fn measure_now(state: State<'_, Shared>) -> AetherResult<measure::Measures> {
    blocking(&state, measure::record).await
}

/// Fait de la dernière mesure la nouvelle référence.
#[tauri::command]
pub async fn measure_reset(state: State<'_, Shared>) -> AetherResult<measure::Measures> {
    blocking(&state, measure::reset_reference).await
}

/// Dernier entretien automatique.
#[tauri::command]
pub async fn maintenance_last(
    state: State<'_, Shared>,
) -> AetherResult<Option<crate::modules::storage::maintenance::LastRun>> {
    blocking(&state, |s| Ok(crate::modules::storage::maintenance::last_run(s))).await
}

/// Enregistre un rapport HTML généré par l'interface dans Documents\Aether.
///
/// Impact Windows : écrit un fichier `aether-rapport-<date>.html` (nom choisi par Aether).
#[tauri::command]
pub async fn report_save(state: State<'_, Shared>, html: String, pdf: Option<bool>) -> AetherResult<String> {
    blocking(&state, move |s| {
        if html.len() > 5 * 1024 * 1024 {
            return Err(crate::error::AetherError::InvalidInput(
                "rapport trop volumineux".into(),
            ));
        }
        let dir = profiles::export_dir(s);
        std::fs::create_dir_all(&dir)?;
        let stem = format!("aether-rapport-{}", crate::core::logger::now_ms());
        let html_path = dir.join(format!("{stem}.html"));
        std::fs::write(&html_path, html)?;
        if pdf.unwrap_or(false) {
            // PDF produit localement par Microsoft Edge (impression sans interface) ; à défaut, rapport HTML.
            if let Some(pdf_path) = report::html_to_pdf(&html_path, &dir.join(format!("{stem}.pdf"))) {
                let _ = std::fs::remove_file(&html_path);
                return Ok(pdf_path.display().to_string());
            }
        }
        Ok(html_path.display().to_string())
    })
    .await
}

/// Exporte le profil courant.
#[tauri::command]
pub async fn profile_export(state: State<'_, Shared>) -> AetherResult<String> {
    blocking(&state, profiles::export).await
}

/// Importe un profil `.json`.
#[tauri::command]
pub async fn profile_import(state: State<'_, Shared>, json: String) -> AetherResult<Vec<TweakReport>> {
    blocking(&state, move |s| profiles::import(s, &json)).await
}
