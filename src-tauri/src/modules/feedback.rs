//! Signalement de bug : le message de l'utilisateur est transmis par un service de formulaires
//! (Web3Forms). L'adresse de destination n'apparaît nulle part : elle est liée à une clé d'accès
//! fournie au moment de la compilation (`AETHER_REPORT_KEY`, secret du dépôt GitHub), jamais au code.

use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};
use crate::modules::Shared;

const ENDPOINT: &str = "https://api.web3forms.com/submit";
/// Clé d'accès injectée à la compilation ; absente dans une compilation locale.
const ACCESS_KEY: Option<&str> = option_env!("AETHER_REPORT_KEY");
const MAX_MESSAGE: usize = 5_000;
const LOG_LINES: usize = 60;

/// Le signalement direct est disponible dans cette compilation.
#[tauri::command]
pub fn feedback_available() -> bool {
    ACCESS_KEY.is_some_and(|k| !k.trim().is_empty())
}

/// Retire de la ligne tout ce qui pourrait identifier l'utilisateur (nom de session, chemins).
pub fn anonymize(line: &str, user: &str) -> String {
    if user.len() < 2 {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len());
    let lower = line.to_lowercase();
    let needle = user.to_lowercase();
    let mut rest = 0;
    for (i, _) in lower.match_indices(&needle) {
        out.push_str(&line[rest..i]);
        out.push_str("<utilisateur>");
        rest = i + needle.len();
    }
    out.push_str(&line[rest..]);
    out
}

fn technical_info(state: &AppState) -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    let build = crate::modules::system::context_menu::windows_build().unwrap_or(0);
    let logs = state.logger.snapshot();
    let tail: Vec<String> = logs
        .iter()
        .rev()
        .take(LOG_LINES)
        .rev()
        .map(|e| anonymize(&format!("[{:?}] {} : {}", e.level, e.module, e.message), &user))
        .collect();
    format!(
        "Aether {}\nWindows build {build}\n\nJournal (anonymisé, {} dernières lignes) :\n{}",
        env!("CARGO_PKG_VERSION"),
        tail.len(),
        tail.join("\n")
    )
}

#[derive(Serialize)]
struct Payload<'a> {
    access_key: &'a str,
    subject: String,
    from_name: &'a str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replyto: Option<String>,
}

/// Envoie un signalement. `reply_to` (facultatif) permet de recontacter l'utilisateur.
#[tauri::command]
pub async fn feedback_send(
    state: State<'_, Shared>,
    message: String,
    include_info: bool,
    reply_to: Option<String>,
) -> AetherResult<()> {
    let key = ACCESS_KEY
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| AetherError::NotFound("service de signalement (compilation sans clé)".into()))?;
    let message = message.trim();
    if message.is_empty() {
        return Err(AetherError::InvalidInput("message vide".into()));
    }
    let body: String = message.chars().take(MAX_MESSAGE).collect();
    let reply_to = reply_to
        .map(|r| r.trim().to_owned())
        .filter(|r| r.contains('@') && r.len() <= 200 && !r.contains(char::is_whitespace));
    let full = if include_info {
        format!("{body}\n\n---\n{}", technical_info(&state))
    } else {
        body
    };
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let network = |e: reqwest::Error| AetherError::Network(e.to_string());
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(network)?;
    client
        .post(ENDPOINT)
        .json(&Payload {
            access_key: key,
            subject: format!("Aether {} — signalement", env!("CARGO_PKG_VERSION")),
            from_name: "Aether",
            message: full,
            replyto: reply_to,
        })
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(network)?;
    state
        .logger
        .success("feedback", "[Envoyé] Signalement transmis, merci !");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymizes_user_name_case_insensitively() {
        assert_eq!(
            anonymize(r"Lecture de C:\Users\Jean\AppData et JEAN", "jean"),
            r"Lecture de C:\Users\<utilisateur>\AppData et <utilisateur>"
        );
        assert_eq!(anonymize("rien à masquer", "x"), "rien à masquer");
    }
}
