//! Sauvegardes « première écriture uniquement » (`aether_*_backup.json`).
//!
//! L'état d'origine d'un paramètre n'est enregistré que la première fois qu'Aether le modifie :
//! une seconde optimisation n'écrase jamais la valeur d'origine de l'utilisateur.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::AetherResult;

/// Fichier de sauvegarde JSON clé → état d'origine.
/// Verrou global des fichiers de sauvegarde : deux actions simultanées ne peuvent pas lire puis
/// réécrire le même fichier en même temps (aucune valeur d'origine perdue).
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
pub struct BackupStore {
    path: PathBuf,
}

impl BackupStore {
    /// Ouvre (sans le créer) le fichier de sauvegarde `file_name` dans `dir`.
    pub fn new(dir: &Path, file_name: &str) -> Self {
        Self {
            path: dir.join(file_name),
        }
    }

    /// Chemin du fichier de sauvegarde.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load_raw(&self) -> AetherResult<BTreeMap<String, serde_json::Value>> {
        match fs::read_to_string(&self.path) {
            Ok(text) if !text.trim().is_empty() => Ok(serde_json::from_str(&text)?),
            Ok(_) => Ok(BTreeMap::new()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_raw(&self, map: &BTreeMap<String, serde_json::Value>) -> AetherResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(map)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Enregistre `value` pour `key` uniquement si aucune sauvegarde n'existe déjà.
    /// Renvoie `true` si la valeur a été enregistrée.
    ///
    /// Impact Windows : écrit un fichier JSON dans le dossier de données d'Aether.
    pub fn record_first<T: Serialize>(&self, key: &str, value: &T) -> AetherResult<bool> {
        let _guard = LOCK.lock()?;
        let mut map = self.load_raw()?;
        if map.contains_key(key) {
            return Ok(false);
        }
        map.insert(key.to_owned(), serde_json::to_value(value)?);
        self.save_raw(&map)?;
        Ok(true)
    }

    /// Lit la sauvegarde associée à `key`.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> AetherResult<Option<T>> {
        match self.load_raw()?.get(key) {
            Some(v) => Ok(Some(serde_json::from_value(v.clone())?)),
            None => Ok(None),
        }
    }

    /// Indique si une sauvegarde existe pour `key`.
    pub fn contains(&self, key: &str) -> AetherResult<bool> {
        Ok(self.load_raw()?.contains_key(key))
    }

    /// Supprime la sauvegarde de `key` (après restauration de l'état d'origine).
    pub fn remove(&self, key: &str) -> AetherResult<()> {
        let _guard = LOCK.lock()?;
        let mut map = self.load_raw()?;
        if map.remove(key).is_some() {
            self.save_raw(&map)?;
        }
        Ok(())
    }

    /// Liste toutes les clés sauvegardées.
    pub fn keys(&self) -> AetherResult<Vec<String>> {
        Ok(self.load_raw()?.into_keys().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_first_state_is_recorded() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let store = BackupStore::new(dir.path(), "aether_services_backup.json");
        assert!(store.record_first("Spooler", &"auto")?);
        assert!(!store.record_first("Spooler", &"demand")?);
        assert_eq!(store.get::<String>("Spooler")?, Some("auto".to_owned()));
        assert!(store.contains("Spooler")?);
        store.remove("Spooler")?;
        assert_eq!(store.get::<String>("Spooler")?, None);
        assert!(store.keys()?.is_empty());
        Ok(())
    }
}
