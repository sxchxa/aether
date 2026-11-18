//! Boost automatique : quand un jeu de la bibliothèque est lancé **en dehors d'Aether** (Steam,
//! raccourci…), ses processus reçoivent la priorité haute et la pleine puissance du processeur.
//! Aucun réglage système n'est modifié : seule la priorité des processus du jeu change, et elle
//! disparaît avec eux à la fermeture du jeu.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};
use tauri::State;

use crate::core::state::AppState;
use crate::error::AetherResult;
use crate::modules::games::boost::{is_specific_install_dir, session_active, tune_process};
use crate::modules::games::library::{all_games, Game};
use crate::modules::{blocking, Shared};

const FILE: &str = "aether_autoboost.json";
/// Intervalle de surveillance des processus.
const POLL: Duration = Duration::from_secs(5);
/// Rafraîchissement de la liste des jeux installés.
const LIBRARY_REFRESH: Duration = Duration::from_secs(300);

#[derive(Debug, Default, Serialize, Deserialize)]
struct Setting {
    enabled: bool,
}

fn path(state: &AppState) -> PathBuf {
    state.data_dir.join(FILE)
}

/// Le Boost automatique est activé (désactivé par défaut).
pub fn enabled(state: &AppState) -> bool {
    fs::read_to_string(path(state))
        .ok()
        .and_then(|t| serde_json::from_str::<Setting>(&t).ok())
        .is_some_and(|s| s.enabled)
}

/// Active ou désactive le Boost automatique.
pub fn set_enabled(state: &AppState, on: bool) -> AetherResult<()> {
    fs::write(path(state), serde_json::to_string(&Setting { enabled: on })?)?;
    Ok(())
}

/// Jeu auquel appartient un exécutable (dossier d'installation suffisamment précis uniquement).
pub fn game_for<'a>(games: &'a [Game], exe: &Path) -> Option<&'a Game> {
    games.iter().find(|g| {
        let dir = Path::new(&g.install_dir);
        is_specific_install_dir(dir) && exe.starts_with(dir)
    })
}

/// Surveille en arrière-plan le lancement des jeux de la bibliothèque (thread dédié, très léger).
pub fn start_watcher(state: Arc<AppState>) {
    std::thread::spawn(move || {
        let Ok(mut sys) = System::new_with_specifics(RefreshKind::nothing()) else {
            return;
        };
        let mut games: Vec<Game> = Vec::new();
        let mut loaded_at: Option<Instant> = None;
        let mut tuned: HashSet<Pid> = HashSet::new();
        loop {
            std::thread::sleep(POLL);
            // Inactif, ou une session Boost lancée depuis Aether s'en occupe déjà.
            if !enabled(&state) || session_active() {
                continue;
            }
            if loaded_at.is_none_or(|t| t.elapsed() > LIBRARY_REFRESH) {
                games = all_games();
                loaded_at = Some(Instant::now());
            }
            if games.is_empty() {
                continue;
            }
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
            );
            let alive: HashSet<Pid> = sys.processes().keys().copied().collect();
            tuned.retain(|pid| alive.contains(pid));
            for (pid, process) in sys.processes() {
                if tuned.contains(pid) {
                    continue;
                }
                let Some(game) = process.exe().and_then(|exe| game_for(&games, exe)) else {
                    continue;
                };
                tuned.insert(*pid);
                if tune_process(pid.as_u32(), true, true) {
                    state.logger.success(
                        "games",
                        format!("Boost automatique : {} (PID {})", game.name, pid.as_u32()),
                    );
                }
            }
        }
    });
}

/// Boost automatique activé.
#[tauri::command]
pub async fn autoboost_get(state: State<'_, Shared>) -> AetherResult<bool> {
    blocking(&state, |s| Ok(enabled(s))).await
}

/// Active ou désactive le Boost automatique.
#[tauri::command]
pub async fn autoboost_set(state: State<'_, Shared>, on: bool) -> AetherResult<bool> {
    blocking(&state, move |s| {
        set_enabled(s, on)?;
        Ok(enabled(s))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::games::library::GameSource;

    fn game(dir: &str) -> Game {
        Game {
            id: "t".into(),
            name: "Jeu".into(),
            source: GameSource::Steam,
            install_dir: dir.into(),
            launch: String::new(),
            image: None,
            has_image: false,
        }
    }

    #[test]
    fn matches_only_specific_game_folders() {
        let games = vec![game("D:\\SteamLibrary\\steamapps\\common\\Hades"), game("C:\\")];
        assert!(game_for(
            &games,
            Path::new("D:\\SteamLibrary\\steamapps\\common\\Hades\\x64\\Hades.exe")
        )
        .is_some());
        // Un dossier trop général (« C:\ ») ne capture jamais les autres programmes.
        assert!(game_for(&games, Path::new("C:\\Windows\\explorer.exe")).is_none());
    }

    #[test]
    fn setting_is_off_by_default_and_persists() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        assert!(!enabled(&state));
        set_enabled(&state, true)?;
        assert!(enabled(&state));
        Ok(())
    }
}
