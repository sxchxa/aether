//! Modules fonctionnels d'Aether (architecture « plug-and-play »).
//!
//! Ajouter une option = implémenter [`Tweak`] dans le module concerné puis l'enregistrer dans
//! [`catalog`] : elle apparaît automatiquement dans l'interface, les profils et l'historique.

pub mod apps;
pub mod autostart;
pub mod diagnostics;
pub mod feedback;
pub mod games;
pub mod links;
pub mod network;
pub mod overview;
pub mod performance;
pub mod storage;
pub mod system;

use std::sync::{Arc, OnceLock};

use tauri::State;

use crate::core::history::HistoryEntry;
use crate::core::logger::LogEntry;
use crate::core::state::{AppState, Settings};
use crate::core::tweak::{self, Outcome, Tweak, TweakStatus};
use crate::core::undo;
use crate::error::{AetherError, AetherResult};

/// État partagé entre commandes.
pub type Shared = Arc<AppState>;

/// Exécute un traitement bloquant (E/S, PowerShell, Registre) hors du thread asynchrone.
pub async fn blocking<T, F>(state: &State<'_, Shared>, f: F) -> AetherResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> AetherResult<T> + Send + 'static,
{
    let shared: Shared = Arc::clone(state.inner());
    tokio::task::spawn_blocking(move || f(&shared)).await?
}

/// Catalogue de toutes les options activables.
pub fn catalog() -> &'static [Box<dyn Tweak>] {
    static CATALOG: OnceLock<Vec<Box<dyn Tweak>>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut list: Vec<Box<dyn Tweak>> = Vec::new();
        for t in performance::service_tweaks() {
            list.push(Box::new(t));
        }
        list.push(Box::new(performance::gaming::mouse_tweak()));
        list.push(Box::new(performance::gaming::xbox_dvr_tweak()));
        list.push(Box::new(performance::extras::WindowedGamesTweak));
        list.push(Box::new(performance::memory::SvcHostSplitTweak));
        for t in performance::visual::tweaks() {
            list.push(Box::new(t));
        }
        for t in performance::extras::registry_extras() {
            list.push(Box::new(t));
        }
        // Alimentation & Énergie
        list.push(Box::new(performance::power::UltimatePowerTweak));
        list.push(Box::new(performance::power::throttling_tweak()));
        for t in performance::power::powercfg_tweaks() {
            list.push(Box::new(t));
        }
        list.push(Box::new(performance::power::HibernateTweak));
        // Réseau
        for t in network::nic_tweaks() {
            list.push(Box::new(t));
        }
        list.push(Box::new(network::tcp_tweak()));
        list.push(Box::new(network::delivery_tweak()));
        list.push(Box::new(network::nic::NicPowerTweak));
        list.push(Box::new(storage::maintenance::MaintenanceTweak));
        for t in network::security_tweaks() {
            list.push(Box::new(t));
        }
        // Système
        list.push(Box::new(system::context_menu::ClassicMenuTweak));
        list.push(Box::new(system::edge::neutralize_tweak()));
        list.push(Box::new(system::privacy::advertising_tweak()));
        list.push(Box::new(system::privacy::background_apps_tweak()));
        list
    })
}

/// Recherche une option par identifiant.
pub fn find(id: &str) -> AetherResult<&'static dyn Tweak> {
    catalog()
        .iter()
        .find(|t| t.id() == id)
        .map(|b| b.as_ref())
        .ok_or_else(|| AetherError::NotFound(format!("option « {id} »")))
}

/// Lit l'état réel de toutes les options (introspection `check_status`, en parallèle).
pub fn all_status(state: &AppState, ids: Option<&[String]>) -> Vec<TweakStatus> {
    let selected: Vec<&dyn Tweak> = catalog()
        .iter()
        .map(|b| b.as_ref())
        .filter(|t| ids.is_none_or(|ids| ids.iter().any(|i| i == t.id())))
        .collect();
    if selected.iter().any(|t| t.id().starts_with("nic.")) {
        // Préchauffe le cache réseau pour éviter des appels PowerShell concurrents.
        let _ = network::nic::snapshot(false);
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = selected
            .iter()
            .map(|t| scope.spawn(move || tweak::status_of(*t, state)))
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// État réel des options (toutes ou celles listées).
#[tauri::command]
pub async fn tweaks_status(state: State<'_, Shared>, ids: Option<Vec<String>>) -> AetherResult<Vec<TweakStatus>> {
    blocking(&state, move |s| Ok(all_status(s, ids.as_deref()))).await
}

/// Optimise une option (idempotent).
#[tauri::command]
pub async fn tweak_optimize(state: State<'_, Shared>, id: String) -> AetherResult<Outcome> {
    blocking(&state, move |s| tweak::optimize(find(&id)?, s)).await
}

/// Optimise plusieurs options d'un coup (idempotent, ex. « Performance maximale »).
#[tauri::command]
pub async fn tweaks_optimize_many(state: State<'_, Shared>, ids: Vec<String>) -> AetherResult<Vec<tweak::TweakReport>> {
    blocking(&state, move |s| {
        let list: Vec<&dyn Tweak> = ids.iter().filter_map(|id| find(id).ok()).collect();
        Ok(tweak::optimize_many(&list, s))
    })
    .await
}

/// Identifiants du profil « Performance maximale (PC fixe) ».
#[tauri::command]
pub async fn power_max_ids() -> AetherResult<Vec<&'static str>> {
    Ok(performance::power::POWER_MAX
        .iter()
        .copied()
        .filter(|id| !performance::power::skip_on_this_pc(id))
        .collect())
}

/// Rétablit l'état d'origine d'une option.
#[tauri::command]
pub async fn tweak_restore(state: State<'_, Shared>, id: String) -> AetherResult<Outcome> {
    blocking(&state, move |s| tweak::restore(find(&id)?, s)).await
}

/// Rétablit l’état d’origine de toutes les options modifiées par Aether (d’après l’historique).
///
/// Impact Windows : restaure chaque option à partir de sa sauvegarde « première écriture ».
/// Les réglages faits avant Aether (jamais modifiés par lui) ne sont pas touchés.
pub fn restore_all(state: &AppState) -> Vec<tweak::TweakReport> {
    let touched = state.touched();
    state.logger.info(
        "restore",
        format!(
            "Rétablissement de tout ce qu'Aether a modifié ({} élément(s))",
            touched.len()
        ),
    );
    let report = |id: &str, r: AetherResult<Outcome>| match r {
        Ok(outcome) => tweak::TweakReport {
            id: id.to_owned(),
            outcome: Some(outcome),
            error: None,
            detail: None,
        },
        Err(e) => {
            state.logger.error(id, format!("[Échec] {id} : {e}"));
            tweak::TweakReport {
                id: id.to_owned(),
                outcome: None,
                error: Some(e.to_string()),
                detail: None,
            }
        }
    };
    let mut reports: Vec<tweak::TweakReport> = Vec::new();

    // 1. Options du catalogue : chacune revient à sa sauvegarde d'origine (ou au défaut de Windows).
    let list: Vec<&dyn Tweak> = catalog()
        .iter()
        .map(|t| t.as_ref())
        .filter(|t| touched.iter().any(|id| id == t.id()))
        .collect();
    reports.extend(tweak::run_in_lanes(&list, |t| report(t.id(), tweak::restore(t, state))));

    // 2. Serveurs DNS : la configuration d'avant le premier changement (liste vide = automatique).
    if touched.iter().any(|id| id.starts_with("dns.")) {
        reports.push(report("dns.apply", restore_dns(state)));
    }

    // 3. Programmes au lancement : état d'avant la première modification par Aether.
    for id in touched.iter().filter(|id| id.starts_with("startup.")) {
        reports.push(report(id, replay_oldest(state, id)));
    }

    // 4. Services restants modifiés hors catalogue.
    if !state.dry_run() {
        match performance::services::restore_all(state) {
            Ok(names) => reports.extend(
                names
                    .into_iter()
                    .map(|n| report(&format!("svc:{n}"), Ok(Outcome::Applied))),
            ),
            Err(e) => reports.push(report("services", Err(e))),
        }
    }
    let done = reports.iter().filter(|r| r.outcome == Some(Outcome::Applied)).count();
    state
        .logger
        .success("restore", format!("[Terminé] {done} élément(s) rétabli(s)"));
    reports
}

/// Rétablit les serveurs DNS sauvegardés avant la première modification.
fn restore_dns(state: &AppState) -> AetherResult<Outcome> {
    let backups = state.backups(crate::core::state::MISC_BACKUP);
    let mut changed = false;
    for key in backups.keys()?.into_iter().filter(|k| k.starts_with("dns.")) {
        let (Ok(index), Some(servers)) = (key["dns.".len()..].parse::<u32>(), backups.get::<Vec<String>>(&key)?) else {
            continue;
        };
        if state.dry_run() {
            return Ok(Outcome::Preview);
        }
        let current = network::dns::current().ok().filter(|c| c.interface_index == index);
        if let Some(c) = &current {
            if (servers.is_empty() && c.dhcp) || (!c.dhcp && c.servers == servers) {
                backups.remove(&key)?;
                continue;
            }
        }
        let previous = current
            .map(|c| if c.dhcp { Vec::new() } else { c.servers })
            .unwrap_or_default();
        network::dns::set_servers(index, &servers)?;
        state.record(
            "dns.restore",
            "DNS d'origine",
            vec![crate::core::history::UndoAction::Dns {
                interface_index: index,
                servers: previous,
            }],
        )?;
        backups.remove(&key)?;
        changed = true;
    }
    Ok(if changed {
        Outcome::Applied
    } else {
        Outcome::AlreadyDone
    })
}

/// Rejoue les actions d'annulation de la plus ancienne modification connue d'une cible.
fn replay_oldest(state: &AppState, target: &str) -> AetherResult<Outcome> {
    let oldest = state
        .history
        .lock()?
        .list()
        .into_iter()
        .filter(|e| e.target == target)
        .min_by_key(|e| e.id);
    let Some(entry) = oldest else {
        return Ok(Outcome::Unavailable);
    };
    if state.dry_run() {
        return Ok(Outcome::Preview);
    }
    for action in entry.actions.iter().rev() {
        undo::execute(action)?;
    }
    state.record(
        &format!("{target}.restore"),
        &format!("{} (origine)", entry.label),
        Vec::new(),
    )?;
    state.logger.success("restore", format!("[Restauré] {}", entry.label));
    Ok(Outcome::Applied)
}

/// Rétablit toutes les options modifiées par Aether.
#[tauri::command]
pub async fn tweaks_restore_all(state: State<'_, Shared>) -> AetherResult<Vec<tweak::TweakReport>> {
    blocking(&state, |s| Ok(restore_all(s))).await
}

/// Historique des modifications (plus récentes en premier).
#[tauri::command]
pub async fn history_list(state: State<'_, Shared>) -> AetherResult<Vec<HistoryEntry>> {
    Ok(state.history.lock()?.list())
}

/// Annule la dernière modification.
#[tauri::command]
pub async fn history_undo(state: State<'_, Shared>) -> AetherResult<Option<HistoryEntry>> {
    blocking(&state, undo::undo_last).await
}

/// Préférences.
#[tauri::command]
pub async fn settings_get(state: State<'_, Shared>) -> AetherResult<Settings> {
    state.settings()
}

/// Active/désactive le Mode Aperçu.
#[tauri::command]
pub async fn settings_set_dry_run(state: State<'_, Shared>, value: bool) -> AetherResult<Settings> {
    let s = state.set_dry_run(value)?;
    state.logger.info(
        "settings",
        if value {
            "Mode Aperçu activé : aucune modification réelle"
        } else {
            "Mode Aperçu désactivé : les modifications seront appliquées"
        },
    );
    Ok(s)
}

/// Journal d'activité.
#[tauri::command]
pub async fn logs_get(state: State<'_, Shared>) -> AetherResult<Vec<LogEntry>> {
    Ok(state.logger.snapshot())
}

/// Vide le journal.
#[tauri::command]
pub async fn logs_clear(state: State<'_, Shared>) -> AetherResult<()> {
    state.logger.clear();
    Ok(())
}

/// Pages des Paramètres Windows autorisées à l'ouverture.
const ALLOWED_URIS: &[&str] = &[
    "ms-settings:display-advanced",
    "ms-settings:appsfeatures",
    "ms-settings:startupapps",
    "ms-settings:powersleep",
    "ms-settings:network-status",
];

/// Ouvre une page des Paramètres Windows (liste blanche).
#[tauri::command]
pub async fn open_settings_page(uri: String) -> AetherResult<()> {
    if !ALLOWED_URIS.contains(&uri.as_str()) {
        return Err(AetherError::Forbidden(format!("page non autorisée : {uri}")));
    }
    crate::core::command::hidden("explorer.exe").arg(&uri).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<&str> = catalog().iter().map(|t| t.id()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert!(find("svc.spooler").is_ok());
        assert!(find("inconnu").is_err());
    }

    #[test]
    fn status_introspection_is_read_only_and_complete() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        let ids = vec![
            "svc.fax".to_owned(),
            "gaming.mouse".to_owned(),
            "tcp.throttling".to_owned(),
        ];
        let status = all_status(&state, Some(&ids));
        assert_eq!(status.len(), 3);
        assert!(state.history.lock()?.list().is_empty());
        Ok(())
    }
}
