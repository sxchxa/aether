//! Section « Système & Réparation » : applications préinstallées, OneDrive, Edge,
//! programmes au lancement, menu clic-droit et réparation Windows.

pub mod appx;
pub mod context_menu;
pub mod edge;
pub mod onedrive;
pub mod privacy;
pub mod repair;
pub mod startup;
pub mod win11;

use tauri::ipc::Channel;
use tauri::State;

use crate::error::AetherResult;
use crate::modules::{blocking, Shared};

/// Applications préinstallées présentes.
#[tauri::command]
pub async fn appx_list() -> AetherResult<Vec<appx::InstalledApp>> {
    tokio::task::spawn_blocking(appx::installed).await?
}

/// Retire une application préinstallée.
#[tauri::command]
pub async fn appx_remove(state: State<'_, Shared>, id: String) -> AetherResult<bool> {
    blocking(&state, move |s| appx::remove(s, &id)).await
}

/// État de OneDrive.
#[tauri::command]
pub async fn onedrive_status() -> AetherResult<onedrive::OneDriveStatus> {
    tokio::task::spawn_blocking(onedrive::status).await?
}

/// Désinstalle OneDrive.
#[tauri::command]
pub async fn onedrive_uninstall(state: State<'_, Shared>) -> AetherResult<bool> {
    blocking(&state, onedrive::uninstall).await
}

/// État d'Edge et de WebView2.
#[tauri::command]
pub async fn edge_status() -> AetherResult<edge::EdgeStatus> {
    tokio::task::spawn_blocking(edge::status).await?
}

/// Désinstalle le navigateur Edge (si autorisé, WebView2 conservé).
#[tauri::command]
pub async fn edge_uninstall(state: State<'_, Shared>) -> AetherResult<bool> {
    blocking(&state, edge::uninstall_browser).await
}

/// Programmes au lancement.
#[tauri::command]
pub async fn startup_list() -> AetherResult<Vec<startup::StartupEntry>> {
    tokio::task::spawn_blocking(startup::list).await?
}

/// Active/désactive un programme au lancement.
#[tauri::command]
pub async fn startup_set(state: State<'_, Shared>, name: String, enabled: bool) -> AetherResult<bool> {
    blocking(&state, move |s| startup::set_enabled(s, &name, enabled)).await
}

/// Redémarre l'Explorateur Windows.
#[tauri::command]
pub async fn restart_explorer(state: State<'_, Shared>) -> AetherResult<()> {
    blocking(&state, |s| {
        if s.dry_run() {
            s.logger.log(
                crate::core::logger::LogLevel::Preview,
                "explorer",
                "[APERÇU] Redémarrage de l'Explorateur",
            );
            return Ok(());
        }
        context_menu::restart_explorer()?;
        s.logger.success("explorer", "[Appliqué] Explorateur redémarré");
        Ok(())
    })
    .await
}

/// Lance SFC ou DISM avec sortie en direct.
#[tauri::command]
pub async fn repair_run(
    state: State<'_, Shared>,
    tool: repair::RepairTool,
    on_event: Channel<repair::RepairEvent>,
) -> AetherResult<Option<i32>> {
    blocking(&state, move |s| {
        repair::run(s, tool, |e| {
            let _ = on_event.send(e);
        })
    })
    .await
}

/// Compatibilité de ce PC avec Windows 11 (proposé uniquement sous Windows 10).
#[tauri::command]
pub async fn win11_status() -> AetherResult<win11::Win11Status> {
    tokio::task::spawn_blocking(win11::status).await?
}

/// Passage à Windows 11 avec l'Assistant d'installation officiel (PC compatible).
#[tauri::command]
pub async fn win11_upgrade(state: State<'_, Shared>) -> AetherResult<()> {
    let shared = std::sync::Arc::clone(state.inner());
    win11::upgrade_with_assistant(&shared).await
}

/// Passage à Windows 11 depuis l'image ISO officielle (PC non compatible).
#[tauri::command]
pub async fn win11_upgrade_iso(state: State<'_, Shared>) -> AetherResult<()> {
    blocking(&state, win11::upgrade_from_iso).await
}
