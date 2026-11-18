//! Erreur unifiée d'Aether, sérialisable vers le frontend.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Erreur métier d'Aether. Toutes les commandes Tauri renvoient `Result<T, AetherError>`,
/// sérialisée en `{ kind, message }` côté interface.
#[derive(Debug, thiserror::Error)]
pub enum AetherError {
    /// Erreur d'entrée/sortie (fichier, dossier, processus).
    #[error("Erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
    /// Erreur de (dé)sérialisation JSON.
    #[error("Erreur JSON : {0}")]
    Json(#[from] serde_json::Error),
    /// Erreur d'accès au Registre Windows.
    #[error("Registre ({path}) : {message}")]
    Registry { path: String, message: String },
    /// Commande système en échec (code de sortie non nul).
    #[error("La commande « {command} » a échoué (code {code:?}) : {stderr}")]
    Command {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    /// Erreur du Gestionnaire de contrôle des services Windows.
    #[error("Service {name} : {message}")]
    Service { name: String, message: String },
    /// Erreur renvoyée par une API Win32.
    #[error("API Win32 {api} : code {code}")]
    Win32 { api: &'static str, code: u32 },
    /// Erreur réseau (DNS, sockets).
    #[error("Réseau : {0}")]
    Network(String),
    /// Ressource introuvable.
    #[error("Introuvable : {0}")]
    NotFound(String),
    /// Paramètre invalide transmis par l'interface.
    #[error("Paramètre invalide : {0}")]
    InvalidInput(String),
    /// Opération interdite par un garde-fou de sécurité.
    #[error("Opération bloquée par un garde-fou : {0}")]
    Forbidden(String),
    /// Verrou interne empoisonné (thread paniqué).
    #[error("État interne indisponible (verrou empoisonné)")]
    Poisoned,
    /// Erreur du runtime Tauri.
    #[error("Tauri : {0}")]
    Tauri(String),
}

impl AetherError {
    /// Code court stable utilisé par le frontend pour choisir un message traduit.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::Registry { .. } => "registry",
            Self::Command { .. } => "command",
            Self::Service { .. } => "service",
            Self::Win32 { .. } => "win32",
            Self::Network(_) => "network",
            Self::NotFound(_) => "notFound",
            Self::InvalidInput(_) => "invalidInput",
            Self::Forbidden(_) => "forbidden",
            Self::Poisoned => "poisoned",
            Self::Tauri(_) => "tauri",
        }
    }

    /// Construit une erreur de Registre à partir d'une erreur d'E/S `winreg`.
    pub fn registry(path: impl Into<String>, err: &std::io::Error) -> Self {
        Self::Registry {
            path: path.into(),
            message: err.to_string(),
        }
    }
}

impl Serialize for AetherError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AetherError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl From<tauri::Error> for AetherError {
    fn from(value: tauri::Error) -> Self {
        Self::Tauri(value.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for AetherError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::Poisoned
    }
}

impl From<windows_service::Error> for AetherError {
    fn from(value: windows_service::Error) -> Self {
        Self::Service {
            name: "SCM".into(),
            message: value.to_string(),
        }
    }
}

impl From<sysinfo::Error> for AetherError {
    fn from(value: sysinfo::Error) -> Self {
        Self::Win32 {
            api: "sysinfo",
            code: match value {
                sysinfo::Error::Io(e) => e.raw_os_error().unwrap_or(0) as u32,
                _ => 0,
            },
        }
    }
}

impl From<tokio::task::JoinError> for AetherError {
    fn from(value: tokio::task::JoinError) -> Self {
        Self::Tauri(format!("tâche asynchrone interrompue : {value}"))
    }
}

/// Alias de résultat utilisé dans tout le backend.
pub type AetherResult<T> = Result<T, AetherError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let err = AetherError::NotFound("adaptateur".into());
        let json = serde_json::to_value(&err).unwrap_or_default();
        assert_eq!(json["kind"], "notFound");
        assert_eq!(json["message"], "Introuvable : adaptateur");
    }
}
