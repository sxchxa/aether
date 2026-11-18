//! Section « Jeux » : bibliothèque des jeux installés et lancement avec Boost.

pub mod autoboost;
pub mod boost;
pub mod library;

use std::sync::Mutex;

use tauri::ipc::Channel;
use tauri::State;

use crate::error::{AetherError, AetherResult};
use crate::modules::{blocking, Shared};
use library::Game;

static CACHE: Mutex<Option<Vec<Game>>> = Mutex::new(None);

fn games(force: bool) -> AetherResult<Vec<Game>> {
    let mut cache = CACHE.lock()?;
    if force || cache.is_none() {
        *cache = Some(library::all_games());
    }
    Ok(cache.clone().unwrap_or_default())
}

fn find(id: &str) -> AetherResult<Game> {
    games(false)?
        .into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| AetherError::NotFound(format!("jeu « {id} »")))
}

/// Jeux installés (Steam, Epic, GOG).
#[tauri::command]
pub async fn games_list(force: Option<bool>) -> AetherResult<Vec<Game>> {
    tokio::task::spawn_blocking(move || games(force.unwrap_or(false))).await?
}

/// Image de couverture d'un jeu (data URI), si disponible.
#[tauri::command]
pub async fn game_image(id: String) -> AetherResult<Option<String>> {
    tokio::task::spawn_blocking(move || {
        let game = find(&id)?;
        Ok(match game.image {
            Some(path) => {
                let bytes = std::fs::read(path)?;
                Some(format!("data:image/jpeg;base64,{}", library::base64(&bytes)))
            }
            None => None,
        })
    })
    .await?
}

/// Lance un jeu avec Boost ; la progression est diffusée en direct jusqu'à la fin de la partie.
#[tauri::command]
pub async fn game_launch(
    state: State<'_, Shared>,
    id: String,
    options: boost::BoostOptions,
    on_event: Channel<boost::BoostEvent>,
) -> AetherResult<()> {
    blocking(&state, move |s| {
        let game = find(&id)?;
        boost::launch(s, &game, options, |e| {
            let _ = on_event.send(e);
        })
    })
    .await
}

/// Jeu en cours de session Boost.
#[tauri::command]
pub async fn game_session() -> AetherResult<Option<String>> {
    Ok(boost::active_session())
}

/// Arrête le suivi de la session Boost (le jeu continue).
#[tauri::command]
pub async fn game_session_stop() -> AetherResult<()> {
    boost::stop_session();
    Ok(())
}
