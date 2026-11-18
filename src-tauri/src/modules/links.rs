//! Ouverture de liens externes (communauté Discord, site du projet) dans le navigateur par défaut.

use crate::error::{AetherError, AetherResult};

/// Ouvre un lien externe (boutique, Discord) dans le navigateur par défaut.
///
/// Impact Windows : `explorer.exe <url>` (le navigateur s'ouvre avec les droits de l'utilisateur).
/// Seules les adresses `https://` sans espace ni guillemet sont acceptées.
#[tauri::command]
pub async fn open_link(url: String) -> AetherResult<()> {
    let ok = url.starts_with("https://")
        && url.len() < 300
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c == '"' || c == '\'' || c == '^' || c == '&');
    if !ok {
        return Err(AetherError::Forbidden(format!("lien refusé : {url}")));
    }
    crate::core::command::hidden("explorer.exe").arg(&url).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_unsafe_links() {
        assert!(open_link("http://exemple.com".into()).await.is_err());
        assert!(open_link("https://exemple.com/a b".into()).await.is_err());
        assert!(open_link("https://exemple.com/\"&calc".into()).await.is_err());
    }
}
