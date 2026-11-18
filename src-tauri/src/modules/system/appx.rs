//! Applications préinstallées (UWP/Appx) : détection réelle et retrait.

use serde::{Deserialize, Serialize};

use crate::core::command::{self, ps_quote};
use crate::core::logger::LogLevel;
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};

/// Paquets retirables (identifiant interface, nom de paquet Appx).
pub const REMOVABLE: &[(&str, &str)] = &[
    ("copilot", "Microsoft.Copilot"),
    ("xboxGameBar", "Microsoft.XboxGamingOverlay"),
    ("weather", "Microsoft.BingWeather"),
    ("news", "Microsoft.BingNews"),
    ("solitaire", "Microsoft.MicrosoftSolitaireCollection"),
    ("mail", "microsoft.windowscommunicationsapps"),
    ("outlookNew", "Microsoft.OutlookForWindows"),
    ("getHelp", "Microsoft.GetHelp"),
    ("tips", "Microsoft.Getstarted"),
    ("officeHub", "Microsoft.MicrosoftOfficeHub"),
    ("people", "Microsoft.People"),
    ("moviesTv", "Microsoft.ZuneVideo"),
    ("phoneLink", "Microsoft.YourPhone"),
    ("feedbackHub", "Microsoft.WindowsFeedbackHub"),
    ("maps", "Microsoft.WindowsMaps"),
    ("cortana", "Microsoft.549981C3F5F10"),
    ("clipchamp", "Clipchamp.Clipchamp"),
    ("todo", "Microsoft.Todos"),
    ("powerAutomate", "Microsoft.PowerAutomateDesktop"),
    ("teams", "MSTeams"),
    ("mixedReality", "Microsoft.MixedReality.Portal"),
];

/// Fragments de noms **jamais** retirables (dépendances système et WebView2).
pub const PROTECTED_FRAGMENTS: &[&str] = &[
    "webview",
    "edge",
    "store",
    "vclibs",
    "ui.xaml",
    "net.native",
    "desktopappinstaller",
    "sechealthui",
    "windowsappruntime",
];

/// Application préinstallée détectée.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    /// Identifiant interface (`copilot`…).
    #[serde(default)]
    pub id: String,
    /// Nom du paquet Appx.
    #[serde(alias = "Name")]
    pub package: String,
    /// Version installée.
    #[serde(alias = "Version", default)]
    pub version: String,
}

fn is_protected(package: &str) -> bool {
    let lower = package.to_lowercase();
    PROTECTED_FRAGMENTS.iter().any(|f| lower.contains(f))
}

/// Liste les applications préinstallées réellement présentes.
///
/// Impact Windows : lecture seule (`Get-AppxPackage`).
pub fn installed() -> AetherResult<Vec<InstalledApp>> {
    let names = REMOVABLE.iter().map(|(_, p)| ps_quote(p)).collect::<Vec<_>>().join(",");
    let mut apps: Vec<InstalledApp> = command::powershell_json(&format!(
        "Get-AppxPackage | Where-Object {{ $_.Name -in @({names}) }} | Select-Object Name, @{{n='Version';e={{[string]$_.Version}}}} | Sort-Object Name -Unique | ConvertTo-Json -Compress"
    ))?;
    for app in &mut apps {
        if let Some((id, _)) = REMOVABLE.iter().find(|(_, p)| p.eq_ignore_ascii_case(&app.package)) {
            app.id = (*id).to_owned();
        }
    }
    Ok(apps)
}

/// Retire une application préinstallée (liste blanche stricte, WebView2 exclu).
///
/// Impact Windows : `Get-AppxPackage -Name <paquet> | Remove-AppxPackage` pour l'utilisateur
/// courant. Non annulable : l'application reste réinstallable depuis le Microsoft Store.
pub fn remove(state: &AppState, id: &str) -> AetherResult<bool> {
    let (_, package) = REMOVABLE
        .iter()
        .find(|(i, _)| *i == id)
        .ok_or_else(|| AetherError::Forbidden(format!("application « {id} » hors liste blanche")))?;
    if is_protected(package) {
        return Err(AetherError::Forbidden(format!("{package} est une dépendance protégée")));
    }
    if state.dry_run() {
        state.logger.log(
            LogLevel::Preview,
            "appx",
            format!("[APERÇU] Remove-AppxPackage {package}"),
        );
        return Ok(false);
    }
    command::powershell_with_timeout(
        &format!("Get-AppxPackage -Name {} | Remove-AppxPackage", ps_quote(package)),
        std::time::Duration::from_secs(300),
    )?;
    // Vérification : le paquet doit avoir disparu pour l'utilisateur courant.
    let remaining = command::powershell(&format!("@(Get-AppxPackage -Name {}).Count", ps_quote(package)))?;
    if remaining.trim().parse::<u32>().unwrap_or(0) > 0 {
        return Err(AetherError::Forbidden(format!(
            "{package} est toujours présent : fermez l'application si elle est ouverte, puis réessayez"
        )));
    }
    state.logger.success(
        "appx",
        format!("[Retiré] {package} (réinstallable depuis le Microsoft Store)"),
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_never_contains_protected_packages() {
        assert!(REMOVABLE.iter().all(|(_, p)| !is_protected(p)));
        assert!(is_protected("Microsoft.EdgeWebView2Runtime"));
        assert!(is_protected("Microsoft.WindowsStore"));
    }

    #[test]
    fn refuses_unknown_ids() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        assert!(remove(&state, "Microsoft.EdgeWebView2").is_err());
        Ok(())
    }
}
