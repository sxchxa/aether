//! Aether — optimiseur système, stockage, réseau et gaming pour Windows 10/11.
//!
//! Architecture :
//! - [`core`] : socle transverse (erreurs, commandes silencieuses, Registre, sauvegardes,
//!   historique d'annulation, journal, moteur idempotent) ;
//! - [`modules`] : une section de l'interface par module (`overview`, `storage`,
//!   `performance`, `network`, `system`).

// Aucune panique possible dans le code applicatif : les erreurs passent par `AetherError`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::dbg_macro,
        clippy::todo
    )
)]

pub mod core;
pub mod error;
pub mod modules;

use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent, Wry};

use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};
use crate::modules::Shared;

/// Événement émis vers l'interface après une action rapide de la zone de notification.
pub const TRAY_EVENT: &str = "aether://tray";

/// Entrées du menu de la zone de notification (pour la traduction dynamique).
pub struct TrayItems(Mutex<Vec<MenuItem<Wry>>>);

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn tray_action(app: &AppHandle, id: &str) {
    let state: Shared = Arc::clone(app.state::<Shared>().inner());
    let handle = app.clone();
    let id = id.to_owned();
    std::thread::spawn(move || {
        let result: AetherResult<String> = match id.as_str() {
            "purge" => modules::performance::ram::purge().map(|r| {
                state.logger.success(
                    "tray",
                    format!("[Appliqué] RAM purgée : {} Mo libérés", r.freed / 1_048_576),
                );
                "purge".to_owned()
            }),
            "performance" => modules::find("power.ultimate")
                .and_then(|t| crate::core::tweak::optimize(t, &state))
                .map(|_| "performance".to_owned()),
            _ => Ok(String::new()),
        };
        match result {
            Ok(action) if !action.is_empty() => {
                let _ = handle.emit(TRAY_EVENT, action);
            }
            Ok(_) => {}
            Err(e) => state.logger.error("tray", e.to_string()),
        }
    });
}

fn build_tray(app: &tauri::App) -> tauri::Result<TrayItems> {
    let purge = MenuItem::with_id(app, "purge", "Purger la RAM", true, None::<&str>)?;
    let perf = MenuItem::with_id(app, "performance", "Mode Performance", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Ouvrir Aether", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&purge, &perf, &separator, &open, &quit])?;
    let mut builder = TrayIconBuilder::with_id("aether-tray")
        .tooltip("Aether")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            other => tray_action(app, other),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(TrayItems(Mutex::new(vec![purge, perf, open, quit])))
}

/// Attribue à la fenêtre une petite et une grande icône chargées depuis la ressource de
/// l'exécutable, chacune à la taille exacte demandée par Windows.
///
/// Par défaut, Tauri ne fournit qu'une seule image (la grande) que Windows réduit lui-même pour
/// la barre de titre et le Gestionnaire des tâches, ce qui dégrade le logo en petite taille.
/// Impact Windows : `LoadImageW` + `WM_SETICON` sur la fenêtre principale (lecture seule ailleurs).
fn apply_crisp_window_icons(hwnd: *mut std::ffi::c_void) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, SM_CXICON,
        SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };
    // Identifiant de la ressource d'icône de l'application (tauri_utils::platform).
    const APP_ICON_ID: usize = 32512;
    // SAFETY : handle de fenêtre valide fourni par Tauri ; les icônes chargées restent valides
    // pendant toute la vie du processus (ressource partagée du module).
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        for (kind, cx, cy) in [(ICON_SMALL, SM_CXSMICON, SM_CYSMICON), (ICON_BIG, SM_CXICON, SM_CYICON)] {
            let icon = LoadImageW(
                module,
                APP_ICON_ID as *const u16,
                IMAGE_ICON,
                GetSystemMetrics(cx),
                GetSystemMetrics(cy),
                LR_DEFAULTCOLOR,
            );
            if !icon.is_null() {
                SendMessageW(hwnd, WM_SETICON, kind as usize, icon as isize);
            }
        }
    }
}

/// Traduit les entrées du menu de la zone de notification (ordre : purge, performance, ouvrir, quitter).
#[tauri::command]
async fn tray_localize(items: State<'_, TrayItems>, labels: Vec<String>) -> AetherResult<()> {
    let items = items.0.lock()?;
    if labels.len() != items.len() {
        return Err(AetherError::InvalidInput("nombre de libellés incorrect".into()));
    }
    for (item, label) in items.iter().zip(labels) {
        item.set_text(label)?;
    }
    Ok(())
}

/// Point d'entrée de l'application Tauri.
pub fn run() {
    // Lancement par la tâche planifiée : entretien silencieux, sans fenêtre, puis fin.
    // Appelé par l'installateur : active ou retire le lancement au démarrage, sans fenêtre, puis fin.
    for (flag, on) in [("--autostart-on", true), ("--autostart-off", false)] {
        if std::env::args().any(|a| a == flag) {
            let _ = modules::autostart::set(on);
            return;
        }
    }
    if std::env::args().any(|a| a == modules::storage::maintenance::ARG) {
        modules::storage::maintenance::run_headless();
        return;
    }
    let result = tauri::Builder::default()
        // Une seule fenêtre d'Aether à la fois : un second lancement ramène simplement la fenêtre existante
        // (deux instances écraseraient mutuellement l'historique et les sauvegardes).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = AppState::new(&data_dir);
            state.logger.attach(app.handle().clone());
            state.logger.info(
                "aether",
                format!(
                    "Aether {} démarré — données : {}",
                    env!("CARGO_PKG_VERSION"),
                    data_dir.display()
                ),
            );
            // Session Boost interrompue (fermeture, plantage) : le mode d'alimentation d'origine est rétabli.
            modules::games::boost::recover_interrupted_session(&state);
            // Anciennes versions : réactive les mises à jour de sécurité d'Edge si Aether les avait bloquées.
            modules::system::edge::release_legacy_update_block(&state);
            let shared: Shared = Arc::new(state);
            // Mesure de référence au premier lancement (avant toute optimisation), sans bloquer l'ouverture.
            let for_measure = Arc::clone(&shared);
            std::thread::spawn(move || modules::overview::measure::ensure_reference(&for_measure));
            // Boost automatique des jeux lancés hors d’Aether (inactif tant que l’option est coupée).
            modules::games::autoboost::start_watcher(Arc::clone(&shared));
            app.manage::<Shared>(shared);
            let items = build_tray(app)?;
            app.manage(items);
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(hwnd) = window.hwnd() {
                    apply_crisp_window_icons(hwnd.0);
                }
                // Lancé à l’ouverture de session : Aether démarre réduit dans la zone de notification.
                if std::env::args().any(|a| a == modules::autostart::MINIMIZED_ARG) {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fermer la fenêtre la masque dans la zone de notification ; « Quitter » ferme Aether.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            tray_localize,
            modules::tweaks_status,
            modules::tweak_optimize,
            modules::tweak_restore,
            modules::tweaks_optimize_many,
            modules::power_max_ids,
            modules::history_list,
            modules::history_undo,
            modules::settings_get,
            modules::settings_set_dry_run,
            modules::logs_get,
            modules::logs_clear,
            modules::open_settings_page,
            modules::overview::system_info,
            modules::overview::hardware_info,
            modules::overview::monitor_start,
            modules::overview::monitor_stop,
            modules::overview::restore_status,
            modules::overview::restore_enable,
            modules::overview::restore_create,
            modules::overview::restore_points,
            modules::overview::restore_to_point,
            modules::overview::system_restart,
            modules::overview::express_optimize,
            modules::overview::profile_apply,
            modules::overview::profile_tweaks,
            modules::overview::profile_export,
            modules::overview::measures_get,
            modules::links::open_link,
            modules::feedback::feedback_available,
            modules::feedback::feedback_send,
            modules::tweaks_restore_all,
            modules::diagnostics::disks_health,
            modules::diagnostics::startup_impacts,
            modules::diagnostics::battery_health,
            modules::diagnostics::battery_report_open,
            modules::storage::duplicates::storage_duplicates,
            modules::autostart::autostart_get,
            modules::autostart::autostart_set,
            modules::overview::report_save,
            modules::overview::maintenance_last,
            modules::overview::measure_now,
            modules::overview::measure_reset,
            modules::overview::profile_import,
            modules::storage::storage_rules,
            modules::storage::storage_scan,
            modules::storage::storage_blockers,
            modules::storage::storage_close_apps,
            modules::storage::storage_clean,
            modules::storage::storage_analyze,
            modules::storage::storage_trash_file,
            modules::storage::storage_reveal,
            modules::performance::ram_purge,
            modules::performance::power_schemes,
            modules::performance::power_set_active,
            modules::performance::display_info,
            modules::performance::ssd_trim,
            modules::performance::services_restore_all,
            modules::network::network_adapter,
            modules::network::dns_current,
            modules::network::dns_benchmark,
            modules::network::dns_apply,
            modules::network::dns_reset,
            modules::network::net_speedtest,
            modules::network::net_flush_dns,
            modules::network::net_winsock_reset,
            modules::system::appx_list,
            modules::system::appx_remove,
            modules::system::onedrive_status,
            modules::system::onedrive_uninstall,
            modules::system::edge_status,
            modules::system::edge_uninstall,
            modules::system::startup_list,
            modules::system::startup_set,
            modules::system::restart_explorer,
            modules::system::repair_run,
            modules::system::win11_status,
            modules::system::win11_upgrade,
            modules::system::win11_upgrade_iso,
            modules::apps::apps_library,
            modules::apps::apps_install,
            modules::games::games_list,
            modules::games::game_image,
            modules::games::game_launch,
            modules::games::game_session,
            modules::games::game_session_stop,
            modules::games::autoboost::autoboost_get,
            modules::games::autoboost::autoboost_set,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("Aether n'a pas pu démarrer : {e}");
        std::process::exit(1);
    }
}
