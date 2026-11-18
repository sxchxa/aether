//! Pile d'historique des modifications (bouton « Annuler la dernière modification »).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::logger::now_ms;
use crate::core::registry::{Hive, RegData};
use crate::error::AetherResult;

const MAX_ENTRIES: usize = 100;

/// Type de démarrage d'un service Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartKind {
    /// Démarrage automatique.
    Auto,
    /// Démarrage automatique différé.
    DelayedAuto,
    /// Démarrage manuel (à la demande).
    Manual,
    /// Désactivé.
    Disabled,
    /// Démarrage noyau / système (non modifiable par Aether).
    System,
}

/// Action élémentaire permettant d'annuler une modification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UndoAction {
    /// Restaure (ou supprime si `previous` est `None`) une valeur de Registre.
    #[serde(rename_all = "camelCase")]
    Registry {
        hive: Hive,
        path: String,
        name: String,
        previous: Option<RegData>,
    },
    /// Supprime une clé créée par Aether.
    #[serde(rename_all = "camelCase")]
    DeleteKey { hive: Hive, path: String },
    /// Rétablit le type de démarrage et l'état d'un service.
    #[serde(rename_all = "camelCase")]
    Service {
        name: String,
        start: StartKind,
        was_running: bool,
    },
    /// Rétablit une propriété avancée de carte réseau.
    #[serde(rename_all = "camelCase")]
    NicProperty {
        adapter: String,
        keyword: String,
        value: String,
    },
    /// Rétablit les serveurs DNS d'une interface (liste vide = DHCP).
    #[serde(rename_all = "camelCase")]
    Dns { interface_index: u32, servers: Vec<String> },
    /// Réactive un mode de gestion de l'alimentation.
    #[serde(rename_all = "camelCase")]
    PowerScheme { guid: String },
    /// Rétablit la valeur « sur secteur » d'un paramètre d'alimentation d'un mode.
    #[serde(rename_all = "camelCase")]
    PowerSetting {
        scheme: String,
        subgroup: String,
        setting: String,
        value: u32,
    },
    /// Réactive ou désactive l'hibernation.
    #[serde(rename_all = "camelCase")]
    Hibernate { enabled: bool },
    /// Réapplique à chaud les paramètres souris lus dans le Registre.
    RefreshMouse,
    /// Rétablit un effet visuel de Windows (SystemParametersInfo).
    #[serde(rename_all = "camelCase")]
    VisualEffect { id: String, enabled: bool },
    /// Crée ou supprime la tâche d'entretien hebdomadaire.
    #[serde(rename_all = "camelCase")]
    MaintenanceTask { enabled: bool },
}

/// Entrée de l'historique : une modification utilisateur et ses actions d'annulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Identifiant unique croissant.
    pub id: u64,
    /// Horodatage Unix en millisecondes.
    pub ts: u64,
    /// Identifiant de l'option concernée (ex. `svc.spooler`).
    pub target: String,
    /// Libellé lisible.
    pub label: String,
    /// Actions à rejouer en ordre inverse pour annuler.
    pub actions: Vec<UndoAction>,
}

/// Pile d'historique persistée dans `aether_history.json`.
#[derive(Debug)]
pub struct History {
    path: PathBuf,
    entries: Vec<HistoryEntry>,
}

impl History {
    /// Charge l'historique depuis `dir` (vide si absent ou illisible).
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("aether_history.json");
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    fn persist(&self) -> AetherResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Écriture atomique : un arrêt brutal ne laisse jamais un historique à moitié écrit.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&self.entries)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Empile une modification (ignorée si elle ne contient aucune action).
    pub fn push(&mut self, target: &str, label: &str, actions: Vec<UndoAction>) -> AetherResult<()> {
        if actions.is_empty() {
            return Ok(());
        }
        let id = self.entries.last().map(|e| e.id + 1).unwrap_or(1);
        self.entries.push(HistoryEntry {
            id,
            ts: now_ms(),
            target: target.to_owned(),
            label: label.to_owned(),
            actions,
        });
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(0..excess);
        }
        self.persist()
    }

    /// Retire et renvoie la dernière modification.
    pub fn pop(&mut self) -> AetherResult<Option<HistoryEntry>> {
        let entry = self.entries.pop();
        self.persist()?;
        Ok(entry)
    }

    /// Remet une entrée sur la pile (annulation échouée).
    pub fn restore_entry(&mut self, entry: HistoryEntry) -> AetherResult<()> {
        self.entries.push(entry);
        self.persist()
    }

    /// Dernière modification (sans la retirer).
    pub fn peek(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    /// Toutes les entrées, de la plus récente à la plus ancienne.
    pub fn list(&self) -> Vec<HistoryEntry> {
        self.entries.iter().rev().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_persists() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let mut h = History::load(dir.path());
        h.push("svc.fax", "Fax", vec![UndoAction::RefreshMouse])?;
        h.push("empty", "Rien", Vec::new())?;
        assert_eq!(h.list().len(), 1);
        let reloaded = History::load(dir.path());
        assert_eq!(reloaded.peek().map(|e| e.target.clone()), Some("svc.fax".to_owned()));
        let popped = h.pop()?;
        assert_eq!(popped.map(|e| e.id), Some(1));
        assert!(History::load(dir.path()).peek().is_none());
        Ok(())
    }
}
