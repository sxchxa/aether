//! Règles de nettoyage Windows 10/11 : dossiers temporaires, caches des navigateurs et des pilotes graphiques.
//!
//! Seuls des dossiers de **cache** sont ciblés. Les données de navigation sensibles
//! (cookies, sessions, mots de passe, historique, favoris) sont protégées par une liste
//! blanche stricte appliquée à chaque fichier avant suppression.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::tweak::Risk;

/// Catégorie de nettoyage (cartes de la section « Stockage & Caches »).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CleanCategory {
    /// Résidus Système (`%TEMP%`, `C:\Windows\Temp`, Prefetch…).
    System,
    /// Données de Navigation (caches navigateurs uniquement).
    Browsers,
    /// Caches Graphiques & Jeux (shaders DirectX/NVIDIA/AMD/Intel, lanceurs).
    Graphics,
}

/// Règle de nettoyage résolue sur ce PC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanRule {
    /// Identifiant stable (ex. `sys.userTemp`).
    pub id: String,
    /// Catégorie.
    pub category: CleanCategory,
    /// Risque.
    pub risk: Risk,
    /// Dossiers racines existants à vider (jamais supprimés eux-mêmes).
    pub roots: Vec<PathBuf>,
    /// Préfixes de nom de fichier autorisés (vide = tous les fichiers).
    #[serde(skip)]
    pub file_prefixes: Vec<&'static str>,
    /// Âge minimal (minutes) : les fichiers plus récents sont ignorés.
    pub min_age_minutes: u64,
    /// Processus (`.exe`) qui doivent être fermés pour un nettoyage complet.
    pub processes: Vec<&'static str>,
    /// Sélectionnée par défaut (et incluse dans l'Optimisation Express si `Safe`).
    pub default_selected: bool,
}

/// Noms de fichiers/dossiers de navigateurs **jamais** supprimés (liste blanche stricte).
pub const PROTECTED_NAMES: &[&str] = &[
    "cookies",
    "cookies-journal",
    "login data",
    "login data-journal",
    "login data for account",
    "web data",
    "history",
    "bookmarks",
    "preferences",
    "secure preferences",
    "local state",
    "sessions",
    "current session",
    "current tabs",
    "last session",
    "last tabs",
    "local storage",
    "session storage",
    "indexeddb",
    "network",
    "extension cookies",
    "logins.json",
    "key4.db",
    "key3.db",
    "cert9.db",
    "cookies.sqlite",
    "places.sqlite",
    "formhistory.sqlite",
    "sessionstore.jsonlz4",
    "sessionstore-backups",
    "storage",
];

/// Indique si un chemin traverse un élément protégé (cookies, sessions, mots de passe…).
pub fn is_protected(path: &Path) -> bool {
    path.components().any(|c| {
        let name = c.as_os_str().to_string_lossy().to_lowercase();
        PROTECTED_NAMES.contains(&name.as_str())
    })
}

fn env_path(var: &str) -> Option<PathBuf> {
    env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute())
}

fn windir() -> PathBuf {
    env_path("WINDIR").unwrap_or_else(|| PathBuf::from("C:\\Windows"))
}

fn existing(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = paths.into_iter().filter(|p| p.is_dir()).collect();
    out.sort();
    out.dedup();
    out
}

fn join_all(base: Option<PathBuf>, subs: &[&str]) -> Vec<PathBuf> {
    match base {
        Some(b) => subs.iter().map(|s| b.join(s)).collect(),
        None => Vec::new(),
    }
}

/// Dossiers de cache par profil Chromium.
const CHROMIUM_PROFILE_CACHES: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnWebGPUCache",
    "DawnGraphiteCache",
    "Service Worker\\CacheStorage",
    "Service Worker\\ScriptCache",
    "Shared Dictionary\\cache",
];

/// Dossiers de cache à la racine `User Data` d'un navigateur Chromium.
const CHROMIUM_ROOT_CACHES: &[&str] = &[
    "ShaderCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "component_crx_cache",
];

/// Résout les caches d'un navigateur Chromium (tous profils : `Default`, `Profile N`…).
pub fn chromium_caches(user_data: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = CHROMIUM_ROOT_CACHES.iter().map(|c| user_data.join(c)).collect();
    let mut profiles = vec![user_data.to_path_buf()];
    if let Ok(entries) = fs::read_dir(user_data) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "Default" || name.starts_with("Profile ") || name == "Guest Profile" {
                profiles.push(entry.path());
            }
        }
    }
    for profile in profiles {
        out.extend(CHROMIUM_PROFILE_CACHES.iter().map(|c| profile.join(c)));
    }
    existing(out)
}

fn firefox_caches() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(profiles) = env_path("LOCALAPPDATA").map(|p| p.join("Mozilla\\Firefox\\Profiles")) {
        if let Ok(entries) = fs::read_dir(profiles) {
            for entry in entries.flatten() {
                out.push(entry.path().join("cache2"));
                out.push(entry.path().join("startupCache"));
                out.push(entry.path().join("thumbnails"));
            }
        }
    }
    existing(out)
}

struct BrowserDef {
    id: &'static str,
    base_var: &'static str,
    user_data: &'static str,
    process: &'static str,
}

const BROWSERS: &[BrowserDef] = &[
    BrowserDef {
        id: "chrome",
        base_var: "LOCALAPPDATA",
        user_data: "Google\\Chrome\\User Data",
        process: "chrome.exe",
    },
    BrowserDef {
        id: "edge",
        base_var: "LOCALAPPDATA",
        user_data: "Microsoft\\Edge\\User Data",
        process: "msedge.exe",
    },
    BrowserDef {
        id: "brave",
        base_var: "LOCALAPPDATA",
        user_data: "BraveSoftware\\Brave-Browser\\User Data",
        process: "brave.exe",
    },
    BrowserDef {
        id: "vivaldi",
        base_var: "LOCALAPPDATA",
        user_data: "Vivaldi\\User Data",
        process: "vivaldi.exe",
    },
    BrowserDef {
        id: "opera",
        base_var: "APPDATA",
        user_data: "Opera Software\\Opera Stable",
        process: "opera.exe",
    },
    BrowserDef {
        id: "operaGx",
        base_var: "APPDATA",
        user_data: "Opera Software\\Opera GX Stable",
        process: "opera.exe",
    },
];

fn rule(
    id: &str,
    category: CleanCategory,
    risk: Risk,
    roots: Vec<PathBuf>,
    min_age_minutes: u64,
    processes: Vec<&'static str>,
    default_selected: bool,
) -> CleanRule {
    CleanRule {
        id: id.to_owned(),
        category,
        risk,
        roots: existing(roots),
        file_prefixes: Vec::new(),
        min_age_minutes,
        processes,
        default_selected,
    }
}

/// Vérifie qu'un dossier temporaire est sûr à vider : nom « Temp »/« Tmp », au moins trois
/// niveaux de profondeur (ex. `C:\Users\Nom\AppData\Local\Temp`) et situé sous AppData ou Windows.
pub fn is_safe_temp_dir(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let depth = p
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .count();
    let lower = p.display().to_string().to_lowercase();
    let windows_temp = depth == 2 && lower.ends_with(":\\windows\\temp");
    (name == "temp" || name == "tmp") && (windows_temp || (depth >= 3 && lower.contains("\\appdata\\")))
}

/// Construit la liste des règles applicables sur ce PC (dossiers existants uniquement).
///
/// Impact Windows : lecture seule (vérification d'existence des dossiers).
pub fn all_rules() -> Vec<CleanRule> {
    let local = env_path("LOCALAPPDATA");
    let roaming = env_path("APPDATA");
    let program_data = env_path("PROGRAMDATA");
    let win = windir();
    let mut rules = Vec::new();

    // --- Résidus Système ---
    let mut user_temp = join_all(local.clone(), &["Temp"]);
    // %TEMP% n'est retenu que s'il s'agit vraiment d'un dossier « Temp » (jamais une racine de lecteur,
    // le profil ou un dossier de données mal configuré).
    user_temp.extend(env_path("TEMP").filter(|p| is_safe_temp_dir(p)));
    rules.push(rule(
        "sys.userTemp",
        CleanCategory::System,
        Risk::Safe,
        user_temp,
        60,
        vec![],
        true,
    ));
    rules.push(rule(
        "sys.windowsTemp",
        CleanCategory::System,
        Risk::Safe,
        vec![win.join("Temp"), win.join("SystemTemp")],
        60,
        vec![],
        true,
    ));
    rules.push(rule(
        "sys.prefetch",
        CleanCategory::System,
        Risk::Moderate,
        vec![win.join("Prefetch")],
        0,
        vec![],
        false,
    ));
    rules.push(rule(
        "sys.updateCache",
        CleanCategory::System,
        Risk::Moderate,
        vec![win.join("SoftwareDistribution\\Download")],
        1440,
        vec![],
        false,
    ));
    let mut dumps = join_all(local.clone(), &["CrashDumps", "Microsoft\\Windows\\WER"]);
    dumps.extend(join_all(program_data.clone(), &["Microsoft\\Windows\\WER"]));
    dumps.push(win.join("Minidump"));
    dumps.push(win.join("LiveKernelReports"));
    rules.push(rule(
        "sys.crashDumps",
        CleanCategory::System,
        Risk::Safe,
        dumps,
        60,
        vec![],
        true,
    ));
    let mut thumbs = rule(
        "sys.thumbnails",
        CleanCategory::System,
        Risk::Safe,
        join_all(local.clone(), &["Microsoft\\Windows\\Explorer"]),
        0,
        vec![],
        false,
    );
    thumbs.file_prefixes = vec!["thumbcache_", "iconcache_"];
    rules.push(thumbs);
    rules.push(rule(
        "sys.deliveryOptimization",
        CleanCategory::System,
        Risk::Safe,
        vec![win
            .join("ServiceProfiles\\NetworkService\\AppData\\Local\\Microsoft\\Windows\\DeliveryOptimization\\Cache")],
        60,
        vec![],
        true,
    ));
    rules.push(rule(
        "sys.inetCache",
        CleanCategory::System,
        Risk::Safe,
        join_all(local.clone(), &["Microsoft\\Windows\\INetCache"]),
        60,
        vec![],
        true,
    ));

    // --- Données de Navigation (caches uniquement) ---
    for b in BROWSERS {
        let base = env_path(b.base_var).map(|p| p.join(b.user_data));
        let roots = base.as_deref().map(chromium_caches).unwrap_or_default();
        rules.push(rule(
            &format!("browser.{}", b.id),
            CleanCategory::Browsers,
            Risk::Safe,
            roots,
            0,
            vec![b.process],
            true,
        ));
    }
    rules.push(rule(
        "browser.firefox",
        CleanCategory::Browsers,
        Risk::Safe,
        firefox_caches(),
        0,
        vec!["firefox.exe"],
        true,
    ));
    rules.push(rule(
        "browser.discord",
        CleanCategory::Browsers,
        Risk::Safe,
        join_all(
            roaming.clone(),
            &["discord\\Cache", "discord\\Code Cache", "discord\\GPUCache"],
        ),
        0,
        vec!["Discord.exe"],
        true,
    ));

    // --- Caches Graphiques & Jeux ---
    rules.push(rule(
        "gpu.directx",
        CleanCategory::Graphics,
        Risk::Moderate,
        join_all(local.clone(), &["D3DSCache"]),
        0,
        vec![],
        false,
    ));
    let mut nvidia = join_all(
        local.clone(),
        &["NVIDIA\\DXCache", "NVIDIA\\GLCache", "NVIDIA Corporation\\NV_Cache"],
    );
    nvidia.extend(join_all(program_data.clone(), &["NVIDIA Corporation\\NV_Cache"]));
    nvidia.extend(join_all(roaming.clone(), &["NVIDIA\\ComputeCache"]));
    rules.push(rule(
        "gpu.nvidia",
        CleanCategory::Graphics,
        Risk::Moderate,
        nvidia,
        0,
        vec![],
        false,
    ));
    rules.push(rule(
        "gpu.amd",
        CleanCategory::Graphics,
        Risk::Moderate,
        join_all(
            local.clone(),
            &["AMD\\DxCache", "AMD\\DxcCache", "AMD\\GLCache", "AMD\\VkCache"],
        ),
        0,
        vec![],
        false,
    ));
    rules.push(rule(
        "gpu.intel",
        CleanCategory::Graphics,
        Risk::Moderate,
        join_all(local.clone(), &["Intel\\ShaderCache"]),
        0,
        vec![],
        false,
    ));
    let steam_base = env_path("ProgramFiles(x86)").map(|p| p.join("Steam"));
    rules.push(rule(
        "games.steam",
        CleanCategory::Graphics,
        Risk::Safe,
        join_all(steam_base, &["appcache\\httpcache", "logs", "dumps"]),
        60,
        vec!["steam.exe"],
        true,
    ));
    rules.push(rule(
        "games.epic",
        CleanCategory::Graphics,
        Risk::Safe,
        join_all(
            local,
            &["EpicGamesLauncher\\Saved\\webcache", "EpicGamesLauncher\\Saved\\Logs"],
        ),
        60,
        vec!["EpicGamesLauncher.exe"],
        true,
    ));

    rules
}

/// Tous les processus connus des règles (seuls ceux-ci peuvent être fermés par Aether).
pub fn known_processes() -> Vec<&'static str> {
    let mut list: Vec<&'static str> = BROWSERS.iter().map(|b| b.process).collect();
    list.extend(["firefox.exe", "Discord.exe", "steam.exe", "EpicGamesLauncher.exe"]);
    list.sort_unstable();
    list.dedup();
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_sensitive_browser_data() {
        assert!(is_protected(Path::new(
            "C:\\U\\Chrome\\User Data\\Default\\Network\\Cookies"
        )));
        assert!(is_protected(Path::new("C:\\U\\Edge\\User Data\\Default\\Login Data")));
        assert!(is_protected(Path::new("C:\\U\\Firefox\\Profiles\\x\\key4.db")));
        assert!(!is_protected(Path::new(
            "C:\\U\\Chrome\\User Data\\Default\\Cache\\Cache_Data\\f_0001"
        )));
    }

    #[test]
    fn chromium_profiles_are_resolved_in_sandbox() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let ud = dir.path();
        fs::create_dir_all(ud.join("Default\\Cache"))?;
        fs::create_dir_all(ud.join("Profile 2\\Code Cache"))?;
        fs::create_dir_all(ud.join("ShaderCache"))?;
        fs::create_dir_all(ud.join("Default\\Local Storage"))?;
        let caches = chromium_caches(ud);
        assert_eq!(caches.len(), 3);
        assert!(caches.iter().all(|p| !is_protected(p)));
        Ok(())
    }

    #[test]
    fn rules_never_target_windows_root() {
        let win = windir();
        for r in all_rules() {
            assert!(r.roots.iter().all(|p| p != &win), "{} cible C:\\Windows", r.id);
        }
    }

    #[test]
    fn temp_root_must_really_be_a_temp_folder() {
        assert!(is_safe_temp_dir(Path::new("C:\\Users\\Alex\\AppData\\Local\\Temp")));
        assert!(is_safe_temp_dir(Path::new("C:\\Windows\\Temp")));
        assert!(!is_safe_temp_dir(Path::new("D:\\")));
        assert!(!is_safe_temp_dir(Path::new("C:\\Users\\Alex")));
        assert!(!is_safe_temp_dir(Path::new("D:\\Temp")));
        assert!(!is_safe_temp_dir(Path::new("C:\\Users\\Alex\\Documents")));
    }
}
