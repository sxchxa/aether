//! Microsoft Edge : neutralisation propre ou désinstallation (si autorisée par le DMA).
//!
//! **Garde-fou critique** : Aether dépend de Microsoft Edge WebView2. Le runtime
//! `Microsoft.EdgeWebView2` (client EdgeUpdate `{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`) n'est
//! jamais supprimé ni modifié. Les tâches `MicrosoftEdgeUpdate*` ne sont **pas** désactivées,
//! car elles mettent aussi à jour WebView2 : seule la stratégie de mise à jour propre au
//! navigateur Edge Stable est coupée.

use serde::Serialize;

use crate::core::command;
use crate::core::logger::LogLevel;
use crate::core::reg_tweak::{RegConst, RegTarget, RegistryTweak};
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, REGISTRY_BACKUP};
use crate::core::tweak::{Risk, Section};
use crate::error::{AetherError, AetherResult};
use crate::modules::system::onedrive::split_command_line;

/// GUID EdgeUpdate du runtime WebView2 (intouchable).
pub const WEBVIEW2_CLIENT: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
/// GUID EdgeUpdate du navigateur Edge Stable.
pub const EDGE_STABLE_CLIENT: &str = "{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}";
const EDGE_UNINSTALL: &str = "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Microsoft Edge";

/// Stratégies de neutralisation (préchargement, démarrage rapide, arrière-plan). Les mises à jour de
/// sécurité d'Edge ne sont jamais bloquées : le navigateur reste installé et doit rester à jour.
pub static EDGE_TARGETS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: "SOFTWARE\\Policies\\Microsoft\\Edge",
        name: "StartupBoostEnabled",
        target: RegConst::Dword(0),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: "SOFTWARE\\Policies\\Microsoft\\Edge",
        name: "BackgroundModeEnabled",
        target: RegConst::Dword(0),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: "SOFTWARE\\Policies\\Microsoft\\MicrosoftEdge\\Main",
        name: "AllowPrelaunch",
        target: RegConst::Dword(0),
        default: None,
    },
    RegTarget {
        hive: Hive::Hklm,
        path: "SOFTWARE\\Policies\\Microsoft\\MicrosoftEdge\\TabPreloader",
        name: "AllowTabPreloading",
        target: RegConst::Dword(0),
        default: None,
    },
];

/// Stratégie de blocage des mises à jour d'Edge posée par les anciennes versions d'Aether.
const LEGACY_UPDATE_BLOCK: (&str, &str) = (
    "SOFTWARE\\Policies\\Microsoft\\EdgeUpdate",
    "Update{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}",
);

/// Lève le blocage des mises à jour de sécurité d'Edge si Aether l'avait posé (version antérieure).
///
/// Impact Windows : rétablit la valeur d'origine sauvegardée de la stratégie EdgeUpdate, uniquement
/// lorsqu'une sauvegarde prouve qu'Aether l'a écrite (une stratégie d'entreprise n'est jamais modifiée).
pub fn release_legacy_update_block(state: &AppState) {
    let (path, name) = LEGACY_UPDATE_BLOCK;
    let key = format!("{}\\{path}\\{name}", Hive::Hklm.label());
    let backups = state.backups(REGISTRY_BACKUP);
    let Ok(Some(original)) = backups.get::<Option<RegData>>(&key) else {
        return;
    };
    let res = match &original {
        Some(data) => registry::write(Hive::Hklm, path, name, data),
        None => registry::delete_value(Hive::Hklm, path, name),
    };
    if res.is_ok() {
        let _ = backups.remove(&key);
        state
            .logger
            .info("edge", "Mises à jour de sécurité de Microsoft Edge réactivées");
    }
}

/// Option « Neutraliser Microsoft Edge » (WebView2 isolé).
pub fn neutralize_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "edge.neutralize",
        section: Section::System,
        group: "edge",
        risk: Risk::Advanced,
        targets: EDGE_TARGETS,
        reboot: false,
        refresh_mouse: false,
    }
}

/// État du navigateur Edge et du runtime WebView2.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeStatus {
    /// Navigateur Edge installé.
    pub browser_installed: bool,
    /// Désinstallation autorisée par Windows (Espace économique européen / DMA).
    pub uninstall_allowed: bool,
    /// Runtime WebView2 présent (requis par Aether).
    pub webview2_installed: bool,
    /// Version du runtime WebView2.
    pub webview2_version: Option<String>,
}

fn webview2_version() -> AetherResult<Option<String>> {
    for (hive, path) in [
        (
            Hive::Hklm,
            format!("SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{WEBVIEW2_CLIENT}"),
        ),
        (
            Hive::Hklm,
            format!("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{WEBVIEW2_CLIENT}"),
        ),
        (
            Hive::Hkcu,
            format!("Software\\Microsoft\\EdgeUpdate\\Clients\\{WEBVIEW2_CLIENT}"),
        ),
    ] {
        if let Some(RegData::Sz(v)) = registry::read(hive, &path, "pv")? {
            if !v.is_empty() && v != "0.0.0.0" {
                return Ok(Some(v));
            }
        }
    }
    Ok(None)
}

/// Lit l'état d'Edge et de WebView2.
///
/// Impact Windows : lecture seule du Registre.
pub fn status() -> AetherResult<EdgeStatus> {
    let browser_installed = registry::key_exists(Hive::Hklm, EDGE_UNINSTALL);
    let no_remove = matches!(
        registry::read(Hive::Hklm, EDGE_UNINSTALL, "NoRemove")?,
        Some(RegData::Dword(1))
    );
    let version = webview2_version()?;
    Ok(EdgeStatus {
        browser_installed,
        uninstall_allowed: browser_installed && !no_remove,
        webview2_installed: version.is_some(),
        webview2_version: version,
    })
}

/// Vérifie qu'une ligne de commande de désinstallation cible le navigateur et jamais WebView2.
pub fn is_browser_only_uninstall(args: &str) -> bool {
    let lower = args.to_lowercase();
    lower.contains("--msedge") && !lower.contains("webview") && !lower.contains(&WEBVIEW2_CLIENT.to_lowercase())
}

/// Désinstalle le navigateur Edge via son propre désinstalleur, uniquement si Windows l'autorise.
///
/// Impact Windows : supprime le navigateur Microsoft Edge ; le runtime WebView2 est conservé.
pub fn uninstall_browser(state: &AppState) -> AetherResult<bool> {
    let st = status()?;
    if !st.browser_installed {
        return Ok(false);
    }
    if !st.uninstall_allowed {
        return Err(AetherError::Forbidden(
            "Windows n'autorise pas la désinstallation d'Edge dans cette région (hors DMA)".into(),
        ));
    }
    let Some(RegData::Sz(cmdline)) = registry::read(Hive::Hklm, EDGE_UNINSTALL, "UninstallString")? else {
        return Err(AetherError::NotFound("désinstalleur Edge".into()));
    };
    let (exe, args) = split_command_line(&cmdline);
    if !is_browser_only_uninstall(&args) || !exe.to_lowercase().ends_with("setup.exe") {
        return Err(AetherError::Forbidden(
            "commande de désinstallation inattendue : WebView2 doit rester intact".into(),
        ));
    }
    if state.dry_run() {
        state
            .logger
            .log(LogLevel::Preview, "edge", format!("[APERÇU] {exe} {args}"));
        return Ok(false);
    }
    let args: Vec<&str> = args.split_whitespace().collect();
    command::run_with_timeout(&exe, &args, std::time::Duration::from_secs(600))?;
    if webview2_version()?.is_none() {
        state.logger.error(
            "edge",
            "WebView2 introuvable après désinstallation : réinstallez le runtime.",
        );
    }
    state
        .logger
        .success("edge", "[Désinstallé] Navigateur Microsoft Edge (WebView2 conservé)");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutralization_never_touches_webview2() {
        for t in EDGE_TARGETS {
            let all = format!("{}{}", t.path, t.name).to_lowercase();
            assert!(!all.contains("webview"));
            assert!(!all.contains(&WEBVIEW2_CLIENT.to_lowercase()));
        }
    }

    #[test]
    fn uninstall_guard_rejects_webview2() {
        assert!(is_browser_only_uninstall(
            "--uninstall --msedge --channel=stable --system-level"
        ));
        assert!(!is_browser_only_uninstall("--uninstall --msedgewebview --system-level"));
        assert!(!is_browser_only_uninstall("--uninstall --system-level"));
    }
}
