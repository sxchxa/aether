//! Détection des jeux installés (Steam, Epic Games, GOG) sans lancer les lanceurs.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::registry::{self, Hive};

/// Plateforme d'origine du jeu.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GameSource {
    /// Steam (Valve).
    Steam,
    /// Epic Games Store.
    Epic,
    /// GOG Galaxy.
    Gog,
}

/// Jeu installé.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    /// Identifiant unique (`steam:730`, `epic:Fortnite`…).
    pub id: String,
    /// Nom affiché.
    pub name: String,
    /// Plateforme.
    pub source: GameSource,
    /// Dossier d'installation (sert à reconnaître les processus du jeu).
    pub install_dir: String,
    /// Cible de lancement (URI du lanceur ou exécutable).
    #[serde(skip)]
    pub launch: String,
    /// Image de couverture locale (chemin), si disponible.
    #[serde(skip)]
    pub image: Option<PathBuf>,
    /// Une image est disponible.
    pub has_image: bool,
}

/// Outils Steam qui ne sont pas des jeux.
const STEAM_EXCLUDED: &[&str] = &["228980", "1070560", "1391110", "1493710", "250820"];

/// Extrait la valeur d'une clé `"cle"  "valeur"` (formats VDF/ACF de Steam).
pub fn vdf_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(&format!("\"{key}\""))?;
    let value = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some(value.replace("\\\\", "\\"))
}

fn steam_root() -> Option<PathBuf> {
    let from_reg = |hive, path: &str, name: &str| -> Option<PathBuf> {
        registry::read_text(hive, path, name)
            .ok()
            .flatten()
            .map(|p| PathBuf::from(p.replace('/', "\\")))
    };
    from_reg(Hive::Hkcu, "Software\\Valve\\Steam", "SteamPath")
        .or_else(|| from_reg(Hive::Hklm, "SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"))
        .filter(|p| p.is_dir())
}

/// Clé de comparaison d'un dossier : Steam écrit le même chemin en minuscules avec des « / »
/// dans le Registre, et avec la casse d'origine et des « \ » dans `libraryfolders.vdf`.
fn path_key(path: &Path) -> String {
    let real = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    real.display()
        .to_string()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn steam_libraries(root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![root.to_path_buf()];
    if let Ok(text) = fs::read_to_string(root.join("steamapps\\libraryfolders.vdf")) {
        libs.extend(text.lines().filter_map(|l| vdf_value(l, "path")).map(PathBuf::from));
    }
    // Une même bibliothèque peut être listée deux fois sous deux écritures différentes.
    let mut seen = std::collections::HashSet::new();
    libs.into_iter()
        .filter(|p| p.join("steamapps").is_dir())
        .filter(|p| seen.insert(path_key(p)))
        .collect()
}

fn steam_image(root: &Path, appid: &str) -> Option<PathBuf> {
    let cache = root.join("appcache\\librarycache");
    [
        cache.join(format!("{appid}_header.jpg")),
        cache.join(appid).join("header.jpg"),
        cache.join(format!("{appid}_library_hero.jpg")),
        cache.join(appid).join("library_hero.jpg"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Jeux Steam installés (manifestes `appmanifest_*.acf`).
pub fn steam_games() -> Vec<Game> {
    steam_root().map(|root| steam_games_in(&root)).unwrap_or_default()
}

/// Jeux Steam d'une installation donnée (dossier racine de Steam).
pub fn steam_games_in(root: &Path) -> Vec<Game> {
    let root = root.to_path_buf();
    let mut games = Vec::new();
    for lib in steam_libraries(&root) {
        let apps = lib.join("steamapps");
        let Ok(entries) = fs::read_dir(&apps) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let get = |k: &str| text.lines().find_map(|l| vdf_value(l, k));
            let (Some(appid), Some(title), Some(dir)) = (get("appid"), get("name"), get("installdir")) else {
                continue;
            };
            if STEAM_EXCLUDED.contains(&appid.as_str()) || title.to_lowercase().contains("redistributable") {
                continue;
            }
            let install = apps.join("common").join(&dir);
            if !install.is_dir() {
                continue;
            }
            let image = steam_image(&root, &appid);
            games.push(Game {
                id: format!("steam:{appid}"),
                name: title,
                source: GameSource::Steam,
                install_dir: install.display().to_string(),
                launch: format!("steam://rungameid/{appid}"),
                has_image: image.is_some(),
                image,
            });
        }
    }
    games
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EpicManifest {
    display_name: String,
    install_location: String,
    app_name: String,
    // Les manifestes Epic écrivent ces deux champs en camelCase (« bIsIncompleteInstall »).
    #[serde(default, rename = "bIsIncompleteInstall")]
    b_is_incomplete_install: bool,
    #[serde(default, rename = "bIsApplication")]
    b_is_application: Option<bool>,
}

/// Jeux Epic Games installés (manifestes `*.item`).
pub fn epic_games() -> Vec<Game> {
    let dir = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"))
        .join("Epic\\EpicGamesLauncher\\Data\\Manifests");
    epic_games_in(&dir)
}

/// Jeux Epic Games décrits par les manifestes d'un dossier.
pub fn epic_games_in(dir: &Path) -> Vec<Game> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str::<EpicManifest>(&t).ok())
        .filter(|m| {
            !m.b_is_incomplete_install && m.b_is_application != Some(false) && Path::new(&m.install_location).is_dir()
        })
        .map(|m| Game {
            id: format!("epic:{}", m.app_name),
            name: m.display_name,
            source: GameSource::Epic,
            install_dir: m.install_location,
            launch: format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", m.app_name),
            image: None,
            has_image: false,
        })
        .collect()
}

/// Jeux GOG installés (Registre `GOG.com\Games`).
pub fn gog_games() -> Vec<Game> {
    gog_games_from(Hive::Hklm, "SOFTWARE\\WOW6432Node\\GOG.com\\Games")
}

/// Jeux GOG déclarés sous une clé de Registre donnée.
pub fn gog_games_from(hive: Hive, base: &str) -> Vec<Game> {
    registry::list_subkeys(hive, base)
        .into_iter()
        .filter_map(|id| {
            let key = format!("{base}\\{id}");
            let name = registry::read_text(hive, &key, "gameName").ok().flatten()?;
            let path = registry::read_text(hive, &key, "path").ok().flatten()?;
            let exe = registry::read_text(hive, &key, "exe").ok().flatten()?;
            Path::new(&exe).is_file().then(|| Game {
                id: format!("gog:{id}"),
                name,
                source: GameSource::Gog,
                install_dir: path,
                launch: exe,
                image: None,
                has_image: false,
            })
        })
        .collect()
}

/// Tous les jeux détectés, triés par nom.
///
/// Impact Windows : lecture seule (Registre et fichiers de manifeste des lanceurs).
pub fn all_games() -> Vec<Game> {
    let mut games = steam_games();
    games.extend(epic_games());
    games.extend(gog_games());
    // Filet de sécurité : un même jeu n'apparaît jamais deux fois.
    let mut seen = std::collections::HashSet::new();
    games.retain(|g| seen.insert(g.id.clone()));
    games.sort_by_key(|g| g.name.to_lowercase());
    games
}

/// Encode des octets en Base64 (pour les images de couverture).
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vdf_lines() {
        assert_eq!(
            vdf_value("\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"", "path").as_deref(),
            Some("D:\\SteamLibrary")
        );
        assert_eq!(
            vdf_value("\t\"name\"\t\t\"Counter-Strike 2\"", "name").as_deref(),
            Some("Counter-Strike 2")
        );
        assert_eq!(vdf_value("\"appid\" \"730\"", "name"), None);
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }

    #[test]
    fn detection_never_fails() {
        // Lecture seule : doit fonctionner même sans aucun lanceur installé.
        let games = all_games();
        assert!(games.iter().all(|g| !g.id.is_empty()));
    }

    /// Fausse installation Steam : bibliothèque principale + bibliothèque secondaire déclarée dans
    /// `libraryfolders.vdf`, un jeu valide dans chacune, un redistribuable et un jeu non installé.
    #[test]
    fn detects_steam_games_across_libraries() -> crate::error::AetherResult<()> {
        let root = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        let apps = root.path().join("steamapps");
        let apps2 = second.path().join("steamapps");
        fs::create_dir_all(apps.join("common\\Portal 2"))?;
        fs::create_dir_all(apps2.join("common\\Hades"))?;
        // Chemins écrits comme Steam le fait (barres obliques inverses doublées). Le dossier
        // principal y figure aussi, en majuscules et avec des « / » : il ne doit compter qu'une fois.
        let escaped = second.path().display().to_string().replace('\\', "\\\\");
        let root_variant = root.path().display().to_string().to_uppercase().replace('\\', "/");
        fs::write(
            apps.join("libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{root_variant}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{escaped}\"\n\t}}\n}}\n"
            ),
        )?;
        let manifest = |appid: &str, name: &str, dir: &str| {
            format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"name\"\t\t\"{name}\"\n\t\"installdir\"\t\t\"{dir}\"\n}}\n")
        };
        fs::write(
            apps.join("appmanifest_620.acf"),
            manifest("620", "Portal 2", "Portal 2"),
        )?;
        fs::write(
            apps2.join("appmanifest_1145360.acf"),
            manifest("1145360", "Hades", "Hades"),
        )?;
        // Outil Steam exclu, puis jeu dont le dossier n'existe plus : tous deux ignorés.
        fs::write(
            apps.join("appmanifest_228980.acf"),
            manifest("228980", "Steamworks Common Redistributables", "Steamworks Shared"),
        )?;
        fs::write(apps.join("appmanifest_999.acf"), manifest("999", "Désinstallé", "Gone"))?;

        let mut games = steam_games_in(root.path());
        games.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Hades", "Portal 2"]);
        assert!(games.iter().all(|g| g.source == GameSource::Steam));
        assert_eq!(games[1].launch, "steam://rungameid/620");
        assert!(Path::new(&games[0].install_dir).ends_with("common\\Hades"));
        Ok(())
    }

    /// Faux manifestes Epic : un jeu valide, une installation incomplète, un module non jeu,
    /// un dossier supprimé et un fichier corrompu.
    #[test]
    fn detects_epic_games_and_skips_invalid_manifests() -> crate::error::AetherResult<()> {
        let manifests = tempfile::tempdir()?;
        let install = tempfile::tempdir()?;
        let loc = install.path().display().to_string();
        let item = |name: &str, app: &str, location: &str, extra: &str| {
            serde_json::json!({ "DisplayName": name, "AppName": app, "InstallLocation": location })
                .to_string()
                .replace('}', &format!("{extra}}}"))
        };
        fs::write(manifests.path().join("a.item"), item("Fortnite", "Fortnite", &loc, ""))?;
        fs::write(
            manifests.path().join("b.item"),
            item("Partiel", "Part", &loc, ",\"bIsIncompleteInstall\":true"),
        )?;
        fs::write(
            manifests.path().join("c.item"),
            item("Plugin", "Plug", &loc, ",\"bIsApplication\":false"),
        )?;
        fs::write(
            manifests.path().join("d.item"),
            item("Supprimé", "Gone", "Z:\\nexiste\\pas", ""),
        )?;
        fs::write(manifests.path().join("e.item"), "{ pas du json")?;
        fs::write(manifests.path().join("notes.txt"), "ignoré")?;

        let games = epic_games_in(manifests.path());
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Fortnite");
        assert_eq!(games[0].source, GameSource::Epic);
        assert!(games[0].launch.starts_with("com.epicgames.launcher://apps/Fortnite"));
        // Dossier de manifestes absent : liste vide, jamais d'erreur.
        assert!(epic_games_in(&manifests.path().join("absent")).is_empty());
        Ok(())
    }

    /// Fausses entrées GOG dans une clé de test (HKCU) : seules celles dont l'exécutable existe sont gardées.
    #[test]
    fn detects_gog_games_from_registry() -> crate::error::AetherResult<()> {
        use crate::core::registry::RegData;
        let base = "Software\\AetherTests\\GOG.com\\Games";
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("witcher3.exe");
        fs::write(&exe, b"MZ")?;
        let put = |id: &str, name: &str, exe: &str| -> crate::error::AetherResult<()> {
            let key = format!("{base}\\{id}");
            registry::write(Hive::Hkcu, &key, "gameName", &RegData::Sz(name.into()))?;
            registry::write(Hive::Hkcu, &key, "path", &RegData::Sz(dir.path().display().to_string()))?;
            registry::write(Hive::Hkcu, &key, "exe", &RegData::Sz(exe.into()))
        };
        put("1207664663", "The Witcher 3", &exe.display().to_string())?;
        put("1", "Fantôme", "Z:\\absent\\game.exe")?;
        let games = gog_games_from(Hive::Hkcu, base);
        registry::delete_tree(Hive::Hkcu, "Software\\AetherTests\\GOG.com")?;

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, "gog:1207664663");
        assert_eq!(games[0].name, "The Witcher 3");
        assert_eq!(games[0].launch, exe.display().to_string());
        Ok(())
    }
}
