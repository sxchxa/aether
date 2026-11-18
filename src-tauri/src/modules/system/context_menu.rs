//! Menu clic-droit classique (style Windows 10) sur Windows 11.

use std::time::Duration;

use crate::core::command;
use crate::core::history::UndoAction;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, REGISTRY_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::{AetherError, AetherResult};

/// Clé CLSID dont la présence (avec `InprocServer32` vide) réactive le menu classique.
pub const CLSID_KEY: &str = "Software\\Classes\\CLSID\\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}";
const INPROC: &str = "Software\\Classes\\CLSID\\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\\InprocServer32";

/// Numéro de build Windows (ex. 22631).
///
/// Impact Windows : lecture seule.
pub fn windows_build() -> AetherResult<u32> {
    Ok(
        match registry::read(
            Hive::Hklm,
            "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "CurrentBuildNumber",
        )? {
            Some(RegData::Sz(s)) => s.trim().parse().unwrap_or(0),
            _ => 0,
        },
    )
}

/// Option « Menu clic-droit classique Windows 10 ».
pub struct ClassicMenuTweak;

impl Tweak for ClassicMenuTweak {
    fn id(&self) -> &'static str {
        "ui.classicMenu"
    }
    fn section(&self) -> Section {
        Section::System
    }
    fn group(&self) -> &'static str {
        "contextMenu"
    }
    fn risk(&self) -> Risk {
        Risk::Safe
    }
    fn technical(&self) -> String {
        format!("HKCU\\{INPROC}\\(par défaut) = \"\"\n(redémarrage de l'Explorateur requis)")
    }

    fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
        if windows_build()? < 22000 {
            return Ok(Probe::unavailable("Windows 10 : le menu classique est déjà utilisé"));
        }
        let applied = registry::read(Hive::Hkcu, INPROC, "")? == Some(RegData::Sz(String::new()));
        Ok(Probe {
            applied,
            available: true,
            current: if applied { "Menu classique" } else { "Menu Windows 11" }.into(),
            target: "Menu classique".into(),
        })
    }

    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let existed = registry::key_exists(Hive::Hkcu, CLSID_KEY);
        let previous = registry::read(Hive::Hkcu, INPROC, "")?;
        state
            .backups(REGISTRY_BACKUP)
            .record_first("ui.classicMenu.keyExisted", &existed)?;
        registry::write(Hive::Hkcu, INPROC, "", &RegData::Sz(String::new()))?;
        Ok(if existed {
            vec![UndoAction::Registry {
                hive: Hive::Hkcu,
                path: INPROC.to_owned(),
                name: String::new(),
                previous,
            }]
        } else {
            vec![UndoAction::DeleteKey {
                hive: Hive::Hkcu,
                path: CLSID_KEY.to_owned(),
            }]
        })
    }

    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        registry::delete_tree(Hive::Hkcu, CLSID_KEY)?;
        state.backups(REGISTRY_BACKUP).remove("ui.classicMenu.keyExisted")?;
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hkcu,
            path: INPROC.to_owned(),
            name: String::new(),
            previous: Some(RegData::Sz(String::new())),
        }])
    }
}

/// Vrai si une fenêtre de progression de copie/déplacement de l'Explorateur est ouverte.
///
/// Impact Windows : lecture seule (`FindWindowW`, classe `OperationStatusWindow`).
pub fn file_operation_in_progress() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let class: Vec<u16> = "OperationStatusWindow"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY : chaîne UTF-16 terminée par zéro, nom de fenêtre nul (recherche par classe seule).
    let hwnd = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    !hwnd.is_null()
}
/// Redémarre l'Explorateur Windows pour appliquer les changements d'interface.
///
/// Impact Windows : ferme `explorer.exe`. Si Windows ne le relance pas de lui-même (cas des
/// versions récentes), Aether le relance **sans élévation** (`runas /trustlevel:0x20000`) :
/// un Explorateur administrateur hériterait sinon des droits d'Aether.
pub fn restart_explorer() -> AetherResult<()> {
    // Copie ou déplacement de fichiers en cours : fermer l'Explorateur interromprait l'opération.
    if file_operation_in_progress() {
        return Err(AetherError::Forbidden(
            "une copie de fichiers est en cours : l'Explorateur sera redémarré plus tard (ou à la prochaine connexion)"
                .into(),
        ));
    }
    let explorer_running = || !crate::modules::storage::processes::running_among(&["explorer.exe"]).is_empty();
    let _ = command::run_raw("taskkill.exe", &["/F", "/IM", "explorer.exe"]);
    for _ in 0..8 {
        std::thread::sleep(Duration::from_millis(500));
        if explorer_running() {
            return Ok(());
        }
    }
    command::hidden("runas.exe")
        .args(["/trustlevel:0x20000", "explorer.exe"])
        .spawn()?;
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(500));
        if explorer_running() {
            return Ok(());
        }
    }
    Err(crate::error::AetherError::NotFound(
        "l'Explorateur ne s'est pas relancé".into(),
    ))
}
