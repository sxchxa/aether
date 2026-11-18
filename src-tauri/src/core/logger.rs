//! Journal d'activité temps réel (tiroir « Logs » de l'interface).

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Nom de l'événement Tauri émis à chaque nouvelle entrée.
pub const LOG_EVENT: &str = "aether://log";
const CAPACITY: usize = 800;

/// Niveau d'une entrée de journal.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    /// Information neutre.
    Info,
    /// Action réussie.
    Success,
    /// Paramètre déjà dans son état cible : aucune action.
    Skip,
    /// Mode Aperçu : action simulée, rien n'est modifié.
    Preview,
    /// Avertissement non bloquant.
    Warn,
    /// Échec.
    Error,
}

/// Entrée du journal d'activité.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Horodatage Unix en millisecondes.
    pub ts: u64,
    /// Niveau.
    pub level: LogLevel,
    /// Module émetteur (ex. `services`, `storage`).
    pub module: String,
    /// Message lisible.
    pub message: String,
}

/// Journal en mémoire (anneau de 800 entrées) diffusé à l'interface.
#[derive(Default)]
pub struct Logger {
    entries: Mutex<VecDeque<LogEntry>>,
    app: OnceLock<AppHandle>,
}

/// Taille lisible (ex. `7,8 Go`) pour les messages du journal.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} o")
    } else {
        format!("{value:.1} {}", UNITS[unit]).replace('.', ",")
    }
}

/// Horodatage Unix courant en millisecondes.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Logger {
    /// Attache le handle Tauri pour diffuser les entrées en temps réel.
    pub fn attach(&self, app: AppHandle) {
        let _ = self.app.set(app);
    }

    /// Ajoute une entrée et la diffuse à l'interface.
    pub fn log(&self, level: LogLevel, module: &str, message: impl Into<String>) {
        let entry = LogEntry {
            ts: now_ms(),
            level,
            module: module.to_owned(),
            message: message.into(),
        };
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= CAPACITY {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }
        if let Some(app) = self.app.get() {
            let _ = app.emit(LOG_EVENT, &entry);
        }
    }

    /// Raccourci niveau `Info`.
    pub fn info(&self, module: &str, message: impl Into<String>) {
        self.log(LogLevel::Info, module, message);
    }

    /// Raccourci niveau `Success`.
    pub fn success(&self, module: &str, message: impl Into<String>) {
        self.log(LogLevel::Success, module, message);
    }

    /// Raccourci niveau `Warn`.
    pub fn warn(&self, module: &str, message: impl Into<String>) {
        self.log(LogLevel::Warn, module, message);
    }

    /// Raccourci niveau `Error`.
    pub fn error(&self, module: &str, message: impl Into<String>) {
        self.log(LogLevel::Error, module, message);
    }

    /// Copie des entrées actuelles (les plus anciennes en premier).
    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.entries
            .lock()
            .map(|e| e.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Vide le journal.
    pub fn clear(&self) {
        if let Ok(mut e) = self.entries.lock() {
            e.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_human_sizes() {
        assert_eq!(human_bytes(512), "512 o");
        assert_eq!(human_bytes(8_332_255_623), "7,8 Go");
    }

    #[test]
    fn ring_buffer_is_bounded() {
        let logger = Logger::default();
        for i in 0..(CAPACITY + 10) {
            logger.info("test", format!("ligne {i}"));
        }
        let snap = logger.snapshot();
        assert_eq!(snap.len(), CAPACITY);
        assert_eq!(snap[0].message, "ligne 10");
    }
}
