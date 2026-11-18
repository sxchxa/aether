//! Bibliothèque d'applications courantes, installées via **winget** (gestionnaire officiel
//! de Microsoft, catalogue `winget` signé et vérifié).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;

use crate::core::command;
use crate::core::logger::LogLevel;
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};
use crate::modules::{blocking, Shared};

/// Catégorie d'application.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AppCategory {
    /// Navigateurs.
    Browsers,
    /// Communication.
    Chat,
    /// Jeux.
    Gaming,
    /// Multimédia.
    Media,
    /// Bureautique.
    Office,
    /// Développement.
    Dev,
    /// Outils.
    Tools,
}

/// Application du catalogue.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogApp {
    /// Identifiant winget exact (ex. `Mozilla.Firefox`).
    pub id: &'static str,
    /// Nom affiché.
    pub name: &'static str,
    /// Catégorie.
    pub category: AppCategory,
}

macro_rules! app {
    ($id:expr, $name:expr, $cat:ident) => {
        CatalogApp {
            id: $id,
            name: $name,
            category: AppCategory::$cat,
        }
    };
}

/// Classiques proposés (identifiants vérifiés dans le catalogue winget).
/// Spotify est volontairement absent : son installateur refuse les sessions administrateur.
pub const CATALOG: &[CatalogApp] = &[
    // Outils essentiels, du plus utile au plus spécialisé.
    app!("7zip.7zip", "7-Zip", Tools),
    app!("voidtools.Everything", "Everything", Tools),
    app!("Microsoft.PowerToys", "PowerToys", Tools),
    app!("Notepad++.Notepad++", "Notepad++", Tools),
    app!("Bitwarden.Bitwarden", "Bitwarden", Tools),
    app!("Malwarebytes.Malwarebytes", "Malwarebytes", Tools),
    app!("ShareX.ShareX", "ShareX", Tools),
    app!("qBittorrent.qBittorrent", "qBittorrent", Tools),
    app!("KeePassXCTeam.KeePassXC", "KeePassXC", Tools),
    app!("Klocman.BulkCrapUninstaller", "Bulk Crap Uninstaller", Tools),
    app!("WinDirStat.WinDirStat", "WinDirStat", Tools),
    app!("REALiX.HWiNFO", "HWiNFO", Tools),
    app!("CPUID.CPU-Z", "CPU-Z", Tools),
    app!("TechPowerUp.GPU-Z", "GPU-Z", Tools),
    app!("CrystalDewWorld.CrystalDiskInfo", "CrystalDiskInfo", Tools),
    app!("CrystalDewWorld.CrystalDiskMark", "CrystalDiskMark", Tools),
    app!(
        "Wagnardsoft.DisplayDriverUninstaller",
        "Display Driver Uninstaller",
        Tools
    ),
    app!("Rufus.Rufus", "Rufus", Tools),
    app!("Proton.ProtonVPN", "Proton VPN", Tools),
    app!("AutoHotkey.AutoHotkey", "AutoHotkey", Tools),
    // Navigateurs.
    app!("Google.Chrome", "Google Chrome", Browsers),
    app!("Mozilla.Firefox", "Firefox", Browsers),
    app!("Brave.Brave", "Brave", Browsers),
    app!("Opera.OperaGX", "Opera GX", Browsers),
    app!("Vivaldi.Vivaldi", "Vivaldi", Browsers),
    app!("Zen-Team.Zen-Browser", "Zen Browser", Browsers),
    app!("LibreWolf.LibreWolf", "LibreWolf", Browsers),
    // Communication.
    app!("Discord.Discord", "Discord", Chat),
    app!("Microsoft.Teams", "Microsoft Teams", Chat),
    app!("Zoom.Zoom", "Zoom", Chat),
    app!("Telegram.TelegramDesktop", "Telegram", Chat),
    app!("OpenWhisperSystems.Signal", "Signal", Chat),
    app!("SlackTechnologies.Slack", "Slack", Chat),
    // Multimédia.
    app!("VideoLAN.VLC", "VLC", Media),
    app!("OBSProject.OBSStudio", "OBS Studio", Media),
    app!("Audacity.Audacity", "Audacity", Media),
    app!("GIMP.GIMP.3", "GIMP", Media),
    app!("HandBrake.HandBrake", "HandBrake", Media),
    app!("clsid2.mpc-hc", "MPC-HC", Media),
    app!("KDE.Kdenlive", "Kdenlive", Media),
    app!("KDE.Krita", "Krita", Media),
    app!("Inkscape.Inkscape", "Inkscape", Media),
    app!("BlenderFoundation.Blender", "Blender", Media),
    app!("PeterPawlowski.foobar2000", "foobar2000", Media),
    // Jeux.
    app!("Valve.Steam", "Steam", Gaming),
    app!("EpicGames.EpicGamesLauncher", "Epic Games", Gaming),
    app!("Blizzard.BattleNet", "Battle.net", Gaming),
    app!("ElectronicArts.EADesktop", "EA app", Gaming),
    app!("Ubisoft.Connect", "Ubisoft Connect", Gaming),
    app!("GOG.Galaxy", "GOG Galaxy", Gaming),
    app!("Guru3D.Afterburner", "MSI Afterburner", Gaming),
    app!("Playnite.Playnite", "Playnite", Gaming),
    app!("HeroicGamesLauncher.HeroicGamesLauncher", "Heroic", Gaming),
    app!("PrismLauncher.PrismLauncher", "Prism Launcher", Gaming),
    // Bureautique.
    app!("TheDocumentFoundation.LibreOffice", "LibreOffice", Office),
    app!("Adobe.Acrobat.Reader.64-bit", "Adobe Acrobat Reader", Office),
    app!("SumatraPDF.SumatraPDF", "SumatraPDF", Office),
    app!("ONLYOFFICE.DesktopEditors", "ONLYOFFICE", Office),
    app!("Mozilla.Thunderbird", "Thunderbird", Office),
    app!("Notion.Notion", "Notion", Office),
    app!("Obsidian.Obsidian", "Obsidian", Office),
    // Développement.
    app!("Microsoft.VisualStudioCode", "Visual Studio Code", Dev),
    app!("Git.Git", "Git", Dev),
    app!("Microsoft.WindowsTerminal", "Windows Terminal", Dev),
    app!("Python.Python.3.13", "Python 3.13", Dev),
    app!("OpenJS.NodeJS.LTS", "Node.js LTS", Dev),
    app!("GitHub.GitHubDesktop", "GitHub Desktop", Dev),
    app!("Docker.DockerDesktop", "Docker Desktop", Dev),
    app!("JetBrains.Toolbox", "JetBrains Toolbox", Dev),
];

/// Application du catalogue avec son état sur ce PC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryApp {
    /// Entrée du catalogue.
    #[serde(flatten)]
    pub app: CatalogApp,
    /// Déjà installée.
    pub installed: bool,
}

/// Progression d'une installation (diffusée en direct).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum InstallEvent {
    /// Pourcentage de téléchargement / installation.
    Progress {
        /// 0–100.
        percent: u8,
    },
    /// Ligne d'état de winget.
    Line {
        /// Texte.
        text: String,
    },
}

#[derive(Deserialize)]
struct ExportFile {
    #[serde(rename = "Sources", default)]
    sources: Vec<ExportSource>,
}

#[derive(Deserialize)]
struct ExportSource {
    #[serde(rename = "Packages", default)]
    packages: Vec<ExportPackage>,
}

#[derive(Deserialize)]
struct ExportPackage {
    #[serde(rename = "PackageIdentifier")]
    id: String,
}

static BUSY: AtomicBool = AtomicBool::new(false);

/// Indique si winget est disponible.
pub fn winget_available() -> bool {
    // Deux essais : winget peut être momentanément occupé (mise à jour de ses sources, autre appel).
    (0..2).any(|attempt| {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        command::run_raw("winget.exe", &["--version"])
            .map(|o| o.success())
            .unwrap_or(false)
    })
}

/// Identifiants des paquets installés connus de winget.
///
/// Impact Windows : lecture seule (`winget export`).
pub fn installed_ids(tmp_dir: &std::path::Path) -> AetherResult<HashSet<String>> {
    let file = tmp_dir.join("aether-winget-export.json");
    let path = file.display().to_string();
    command::run_raw(
        "winget.exe",
        &[
            "export",
            "-o",
            &path,
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
    )?;
    let text = std::fs::read_to_string(&file)?;
    let _ = std::fs::remove_file(&file);
    let export: ExportFile = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    Ok(export
        .sources
        .into_iter()
        .flat_map(|s| s.packages)
        .map(|p| p.id.to_lowercase())
        .collect())
}

/// Extrait un pourcentage d'une ligne de progression winget (ex. `██▒▒  42%` ou `12 MB / 30 MB`).
pub fn parse_percent(line: &str) -> Option<u8> {
    if let Some(i) = line.find('%') {
        let digits: String = line[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let value: String = digits.chars().rev().collect();
        return value.parse::<u8>().ok().filter(|v| *v <= 100);
    }
    let (done, total) = line.split_once(" / ")?;
    let parse = |s: &str| s.replace(',', ".").parse::<f64>().ok();
    // « ██████  12.0 MB » : le nombre est l'avant-dernier mot ; « 48.0 MB » : le premier.
    let d = parse(done.split_whitespace().rev().nth(1)?)?;
    let t = parse(total.split_whitespace().next()?)?;
    (t > 0.0).then(|| ((d / t) * 100.0).clamp(0.0, 100.0) as u8)
}

fn find(id: &str) -> AetherResult<&'static CatalogApp> {
    CATALOG
        .iter()
        .find(|a| a.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| AetherError::Forbidden(format!("« {id} » ne fait pas partie de la bibliothèque")))
}

/// Installe (ou met à jour) une application du catalogue en silence.
///
/// Impact Windows : `winget install|upgrade --id <id> -e --silent` depuis la source officielle.
pub fn install<F: FnMut(InstallEvent)>(state: &AppState, id: &str, upgrade: bool, mut emit: F) -> AetherResult<bool> {
    let app = find(id)?;
    if state.dry_run() {
        state.logger.log(
            LogLevel::Preview,
            "apps",
            format!(
                "[APERÇU] winget {} {}",
                if upgrade { "upgrade" } else { "install" },
                app.id
            ),
        );
        return Ok(false);
    }
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err(AetherError::Forbidden("une installation est déjà en cours".into()));
    }
    let verb = if upgrade { "upgrade" } else { "install" };
    state.logger.info("apps", format!("{} : {}", app.name, verb));
    let mut last = 0u8;
    // 20 minutes au plus : un installateur bloqué n'immobilise jamais la bibliothèque.
    let result = command::stream_with_timeout(
        "winget.exe",
        &[
            verb,
            "--id",
            app.id,
            "-e",
            "--silent",
            "--source",
            "winget",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
        Some(std::time::Duration::from_secs(20 * 60)),
        |line| match parse_percent(line) {
            Some(p) if p != last => {
                last = p;
                emit(InstallEvent::Progress { percent: p });
            }
            Some(_) => {}
            None => emit(InstallEvent::Line { text: line.to_owned() }),
        },
    );
    BUSY.store(false, Ordering::SeqCst);
    let code = result?;
    // 0 = succès ; 0x8A15002B = aucune mise à jour applicable (déjà à jour).
    const NO_UPDATE: i32 = 0x8A15_002B_u32 as i32;
    match code {
        Some(0) => {
            state.logger.success("apps", format!("[Installé] {}", app.name));
            Ok(true)
        }
        Some(NO_UPDATE) if upgrade => {
            state
                .logger
                .log(LogLevel::Skip, "apps", format!("[OK - Déjà à jour] {}", app.name));
            Ok(false)
        }
        other => Err(AetherError::Command {
            command: format!("winget {verb} {}", app.id),
            code: other,
            stderr: "voir le journal d'activité".into(),
        }),
    }
}

/// Catalogue avec l'état d'installation de chaque application.
#[tauri::command]
pub async fn apps_library(state: State<'_, Shared>) -> AetherResult<Vec<LibraryApp>> {
    blocking(&state, |s| {
        if !winget_available() {
            return Err(AetherError::NotFound(
                "winget (Programme d'installation d'application)".into(),
            ));
        }
        // Si la liste des applications installées est indisponible, le catalogue reste affiché.
        let installed = installed_ids(&s.data_dir).unwrap_or_else(|e| {
            s.logger
                .error("apps", format!("Liste des applications installées indisponible : {e}"));
            HashSet::new()
        });
        Ok(CATALOG
            .iter()
            .map(|a| LibraryApp {
                app: a.clone(),
                installed: installed.contains(&a.id.to_lowercase()),
            })
            .collect())
    })
    .await
}

/// Installe ou met à jour une application, avec progression en direct.
#[tauri::command]
pub async fn apps_install(
    state: State<'_, Shared>,
    id: String,
    upgrade: bool,
    on_event: Channel<InstallEvent>,
) -> AetherResult<bool> {
    blocking(&state, move |s| {
        install(s, &id, upgrade, |e| {
            let _ = on_event.send(e);
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_winget_progress() {
        assert_eq!(parse_percent("  ██████████████▒▒▒▒▒▒▒▒▒▒  57%"), Some(57));
        assert_eq!(parse_percent("  ██████  12.0 MB / 48.0 MB"), Some(25));
        assert_eq!(parse_percent("Installation réussie"), None);
    }

    #[test]
    fn catalog_ids_are_unique_and_whitelisted() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|a| a.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert!(find("Mozilla.Firefox").is_ok());
        assert!(find("Malware.Inconnu").is_err());
    }
}
