//! Exécution des actions d'annulation (« Annuler la dernière modification »).

use crate::core::history::{HistoryEntry, UndoAction};
use crate::core::logger::LogLevel;
use crate::core::registry;
use crate::core::state::AppState;
use crate::error::AetherResult;
use crate::modules::network::{dns, nic};
use crate::modules::performance::{gaming, power, services, visual};

/// Rejoue une action d'annulation élémentaire.
///
/// Impact Windows : restaure une valeur de Registre, un service, une propriété réseau,
/// un DNS ou un mode d'alimentation.
pub fn execute(action: &UndoAction) -> AetherResult<()> {
    match action {
        UndoAction::Registry {
            hive,
            path,
            name,
            previous,
        } => match previous {
            Some(data) => registry::write(*hive, path, name, data),
            None => registry::delete_value(*hive, path, name),
        },
        UndoAction::DeleteKey { hive, path } => registry::delete_tree(*hive, path),
        UndoAction::Service {
            name,
            start,
            was_running,
        } => services::set_service_state(name, *start, *was_running),
        UndoAction::NicProperty {
            adapter,
            keyword,
            value,
        } => nic::set_property(adapter, keyword, value),
        UndoAction::Dns {
            interface_index,
            servers,
        } => dns::set_servers(*interface_index, servers),
        UndoAction::PowerScheme { guid } => power::set_active(guid),
        UndoAction::PowerSetting {
            scheme,
            subgroup,
            setting,
            value,
        } => power::set_ac_value(scheme, subgroup, setting, *value),
        UndoAction::Hibernate { enabled } => power::set_hibernate(*enabled),
        UndoAction::RefreshMouse => gaming::refresh_mouse(),
        UndoAction::VisualEffect { id, enabled } => visual::set_by_id(id, *enabled),
        UndoAction::MaintenanceTask { enabled } => crate::modules::storage::maintenance::set(*enabled),
    }
}

/// Annule la dernière modification enregistrée dans l'historique.
///
/// En Mode Aperçu, l'annulation est seulement simulée et l'entrée reste dans la pile.
pub fn undo_last(state: &AppState) -> AetherResult<Option<HistoryEntry>> {
    let entry = {
        let mut history = state.history.lock()?;
        if state.dry_run() {
            if let Some(e) = history.peek() {
                state.logger.log(
                    LogLevel::Preview,
                    "history",
                    format!("[APERÇU] Annulation de « {} »", e.label),
                );
            }
            return Ok(None);
        }
        match history.pop()? {
            Some(e) => e,
            None => return Ok(None),
        }
    };
    for action in entry.actions.iter().rev() {
        if let Err(err) = execute(action) {
            state
                .logger
                .error("history", format!("[Échec] Annulation de « {} » : {err}", entry.label));
            state.history.lock()?.restore_entry(entry)?;
            return Err(err);
        }
    }
    state.logger.success("history", format!("[Annulé] {}", entry.label));
    Ok(Some(entry))
}
