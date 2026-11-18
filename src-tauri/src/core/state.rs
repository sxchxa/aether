//! État global partagé entre les commandes Tauri.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::core::backup::BackupStore;
use crate::core::history::{History, UndoAction};
use crate::core::logger::Logger;
use crate::error::AetherResult;

/// Fichier de sauvegarde des services.
pub const SERVICES_BACKUP: &str = "aether_services_backup.json";
/// Fichier de sauvegarde des propriétés de carte réseau.
pub const NIC_BACKUP: &str = "aether_nic_backup.json";
/// Fichier de sauvegarde des valeurs de Registre.
pub const REGISTRY_BACKUP: &str = "aether_registry_backup.json";
/// Fichier de sauvegarde DNS / énergie / démarrage.
pub const MISC_BACKUP: &str = "aether_misc_backup.json";
/// Liste persistante de tout ce qu'Aether a déjà modifié (utilisée par « Tout rétablir »).
const TOUCHED_FILE: &str = "aether_touched.json";

/// Préférences persistées d'Aether.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Mode Aperçu : aucune modification réelle. Retiré de l'interface : toujours désactivé au démarrage.
    pub dry_run: bool,
}

/// État applicatif partagé.
pub struct AppState {
    /// Dossier des données d'Aether (sauvegardes, historique, réglages).
    pub data_dir: PathBuf,
    /// Journal d'activité.
    pub logger: Logger,
    /// Pile d'annulation.
    pub history: Mutex<History>,
    settings: Mutex<Settings>,
    /// Drapeau d'arrêt du flux de monitoring en cours.
    pub monitor_stop: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    /// Initialise l'état à partir du dossier de données (créé si besoin).
    pub fn new(data_dir: &Path) -> Self {
        let _ = fs::create_dir_all(data_dir);
        // Le Mode Aperçu a été retiré : une préférence enregistrée par une ancienne version est ignorée.
        let settings = Settings::default();
        Self {
            data_dir: data_dir.to_path_buf(),
            logger: Logger::default(),
            history: Mutex::new(History::load(data_dir)),
            settings: Mutex::new(settings),
            monitor_stop: Mutex::new(None),
        }
    }

    /// Indique si le Mode Aperçu (Dry-Run) est actif.
    pub fn dry_run(&self) -> bool {
        self.settings.lock().map(|s| s.dry_run).unwrap_or(false)
    }

    /// Copie des préférences.
    pub fn settings(&self) -> AetherResult<Settings> {
        Ok(self.settings.lock()?.clone())
    }

    /// Active/désactive le Mode Aperçu et persiste le choix.
    pub fn set_dry_run(&self, value: bool) -> AetherResult<Settings> {
        let mut s = self.settings.lock()?;
        s.dry_run = value;
        fs::write(
            self.data_dir.join("aether_settings.json"),
            serde_json::to_string_pretty(&*s)?,
        )?;
        Ok(s.clone())
    }

    /// Accès à un fichier de sauvegarde « première écriture ».
    pub fn backups(&self, file: &str) -> BackupStore {
        BackupStore::new(&self.data_dir, file)
    }

    /// Ajoute une modification à la pile d'annulation.
    pub fn record(&self, target: &str, label: &str, actions: Vec<UndoAction>) -> AetherResult<()> {
        if !actions.is_empty() {
            self.mark_touched(target);
        }
        self.history.lock()?.push(target, label, actions)
    }

    /// Tout ce qu'Aether a modifié un jour : liste persistante complétée par l'historique
    /// (qui, lui, est limité aux dernières modifications).
    pub fn touched(&self) -> Vec<String> {
        let mut ids: Vec<String> = fs::read_to_string(self.data_dir.join(TOUCHED_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if let Ok(h) = self.history.lock() {
            for e in h.list() {
                if !ids.contains(&e.target) {
                    ids.push(e.target);
                }
            }
        }
        ids
    }

    fn mark_touched(&self, target: &str) {
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock();
        let mut ids = self.touched();
        if ids.iter().any(|i| i == target) {
            return;
        }
        ids.push(target.to_owned());
        if let Ok(json) = serde_json::to_string_pretty(&ids) {
            let _ = fs::write(self.data_dir.join(TOUCHED_FILE), json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_is_always_off_at_startup() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        assert!(!state.dry_run());
        // Une ancienne préférence « activé » est ignorée au redémarrage.
        state.set_dry_run(true)?;
        assert!(!AppState::new(dir.path()).dry_run());
        Ok(())
    }
}
