//! Lancement d'un jeu avec « Boost » et suivi de la session de jeu.
//!
//! Avant le lancement : purge RAM, purge DNS, mode « Performances optimales » (rétabli à la fin).
//! Pendant la partie : priorité « Haute » (jamais « Temps réel », déconseillé par Microsoft) et
//! désactivation du bridage d'efficacité (EcoQoS) sur chaque processus du jeu.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, ProcessPowerThrottling, SetPriorityClass, SetProcessInformation, HIGH_PRIORITY_CLASS,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
};

use crate::core::command;
use crate::core::logger::human_bytes;
use crate::core::state::{AppState, MISC_BACKUP};
use crate::error::{AetherError, AetherResult};
use crate::modules::games::library::Game;
use crate::modules::performance::{power, ram};

/// Actions de Boost choisies par l'utilisateur.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostOptions {
    /// Purger la RAM avant le lancement.
    pub purge_ram: bool,
    /// Purger le cache DNS.
    pub flush_dns: bool,
    /// Activer « Performances optimales » pendant la session.
    pub performance_plan: bool,
    /// Priorité « Haute » pour les processus du jeu.
    pub high_priority: bool,
    /// Désactiver le bridage d'efficacité (EcoQoS) des processus du jeu.
    pub no_throttling: bool,
}

/// État d'une étape.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    /// En cours.
    Running,
    /// Terminée.
    Done,
    /// Ignorée (option décochée, Mode Aperçu…).
    Skipped,
    /// En échec (non bloquant).
    Error,
}

/// Événement de progression envoyé à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostEvent {
    /// Étape (`ram`, `dns`, `plan`, `launch`, `detect`, `priority`, `session`, `restore`).
    pub step: &'static str,
    /// État.
    pub status: StepStatus,
    /// Détail lisible.
    pub detail: String,
}

static ACTIVE: Mutex<Option<(String, Arc<AtomicBool>)>> = Mutex::new(None);

/// Identifiant du jeu en cours de session Boost, s'il y en a un.
pub fn active_session() -> Option<String> {
    ACTIVE.lock().ok().and_then(|a| a.as_ref().map(|(id, _)| id.clone()))
}

/// Arrête le suivi de la session en cours (le jeu n'est pas fermé).
pub fn stop_session() {
    if let Ok(mut a) = ACTIVE.lock() {
        if let Some((_, stop)) = a.take() {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

fn under(dir: &Path, exe: &Path) -> bool {
    let d = dir.to_string_lossy().to_lowercase();
    let e = exe.to_string_lossy().to_lowercase();
    e.starts_with(&format!("{}\\", d.trim_end_matches('\\')))
}

fn game_pids(sys: &mut System, dir: &Path) -> Vec<Pid> {
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    sys.processes()
        .iter()
        .filter(|(_, p)| p.exe().is_some_and(|e| under(dir, e)))
        .map(|(pid, _)| *pid)
        .collect()
}

/// Applique priorité haute et/ou désactivation d'EcoQoS à un processus.
///
/// Impact Windows : `SetPriorityClass(HIGH_PRIORITY_CLASS)` et
/// `SetProcessInformation(ProcessPowerThrottling, ControlMask=EXECUTION_SPEED, StateMask=0)`.
/// Effet limité à la durée de vie du processus.
pub fn tune_process(pid: u32, high_priority: bool, no_throttling: bool) -> bool {
    // SAFETY : handle ouvert puis systématiquement fermé ; structure POD valide pendant l'appel.
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut ok = true;
        if high_priority {
            ok &= SetPriorityClass(handle, HIGH_PRIORITY_CLASS) != 0;
        }
        if no_throttling {
            let state = PROCESS_POWER_THROTTLING_STATE {
                Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                StateMask: 0,
            };
            ok &= SetProcessInformation(
                handle,
                ProcessPowerThrottling,
                (&state as *const PROCESS_POWER_THROTTLING_STATE).cast(),
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            ) != 0;
        }
        CloseHandle(handle);
        ok
    }
}

fn open_target(target: &str) -> AetherResult<()> {
    // Lancement via l'Explorateur : le jeu démarre avec les droits de l'utilisateur (non élevés),
    // ce qui évite les problèmes d'anti-triche et d'overlay.
    command::hidden("explorer.exe").arg(target).spawn()?;
    Ok(())
}

/// Lance un jeu avec Boost puis suit la session jusqu'à sa fermeture.
///
/// Impact Windows : voir la documentation du module. Le mode d'alimentation d'origine est
/// rétabli automatiquement à la fin de la partie.
pub fn launch<F: FnMut(BoostEvent)>(state: &AppState, game: &Game, opts: BoostOptions, emit: F) -> AetherResult<()> {
    let target = game.launch.clone();
    launch_with(state, game, opts, emit, move || open_target(&target))
}
/// Vrai si le dossier d'un jeu est assez précis pour n'englober que ses propres processus.
pub fn is_specific_install_dir(dir: &Path) -> bool {
    let parts: Vec<String> = dir
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    const TOO_BROAD: &[&str] = &[
        "program files",
        "program files (x86)",
        "windows",
        "users",
        "games",
        "programdata",
    ];
    match parts.as_slice() {
        [] => false,
        [only] => !TOO_BROAD.contains(&only.as_str()),
        _ => !(parts.len() == 2 && parts[0] == "users"),
    }
}

/// Variante de [`launch`] dont l'étape de démarrage est fournie par l'appelant (tests de bout en bout).
/// Une session Boost lancée depuis Aether est en cours.
pub fn session_active() -> bool {
    ACTIVE.lock().map(|a| a.is_some()).unwrap_or(false)
}

pub fn launch_with<F, L>(state: &AppState, game: &Game, opts: BoostOptions, mut emit: F, starter: L) -> AetherResult<()>
where
    F: FnMut(BoostEvent),
    L: FnOnce() -> AetherResult<()>,
{
    // Dossier trop large (racine de lecteur, Program Files…) : la détection viserait d'autres programmes.
    if !is_specific_install_dir(Path::new(&game.install_dir)) {
        return Err(AetherError::Forbidden(format!(
            "dossier d'installation trop général : {}",
            game.install_dir
        )));
    }
    {
        let mut active = ACTIVE.lock()?;
        if let Some((id, _)) = active.as_ref() {
            return Err(AetherError::Forbidden(format!(
                "une session Boost est déjà en cours ({id})"
            )));
        }
        *active = Some((game.id.clone(), Arc::new(AtomicBool::new(false))));
    }
    let stop = ACTIVE
        .lock()?
        .as_ref()
        .map(|(_, s)| Arc::clone(s))
        .ok_or(AetherError::Poisoned)?;
    let result = run_session(state, game, opts, &stop, &mut emit, starter);
    if let Ok(mut a) = ACTIVE.lock() {
        *a = None;
    }
    result
}

fn run_session<F, L>(
    state: &AppState,
    game: &Game,
    opts: BoostOptions,
    stop: &AtomicBool,
    emit: &mut F,
    starter: L,
) -> AetherResult<()>
where
    F: FnMut(BoostEvent),
    L: FnOnce() -> AetherResult<()>,
{
    let mut step = |step: &'static str, status: StepStatus, detail: String| emit(BoostEvent { step, status, detail });
    state
        .logger
        .info("games", format!("Lancement de {} avec Boost", game.name));

    // 1. RAM
    if opts.purge_ram {
        step("ram", StepStatus::Running, String::new());
        match ram::purge() {
            Ok(r) => step("ram", StepStatus::Done, human_bytes(r.freed)),
            Err(e) => step("ram", StepStatus::Error, e.to_string()),
        }
    } else {
        step("ram", StepStatus::Skipped, String::new());
    }

    // 2. DNS
    if opts.flush_dns {
        match command::run("ipconfig.exe", &["/flushdns"]) {
            Ok(_) => step("dns", StepStatus::Done, String::new()),
            Err(e) => step("dns", StepStatus::Error, e.to_string()),
        }
    } else {
        step("dns", StepStatus::Skipped, String::new());
    }

    // 3. Mode d'alimentation (rétabli en fin de session)
    let mut previous_plan: Option<String> = None;
    if opts.performance_plan && state.dry_run() {
        step("plan", StepStatus::Skipped, "Mode Aperçu".into());
    } else if opts.performance_plan {
        step("plan", StepStatus::Running, String::new());
        let res = power::active_guid().and_then(|current| {
            let ultimate = power::ensure_ultimate()?;
            if !current.eq_ignore_ascii_case(&ultimate) {
                // Mémorisé sur disque AVANT la bascule : rétabli au prochain démarrage si Aether
                // est fermé ou si le PC s'arrête pendant la session.
                state.backups(MISC_BACKUP).record_first(PENDING_PLAN, &current)?;
                power::set_active(&ultimate)?;
                previous_plan = Some(current);
            }
            Ok(())
        });
        match res {
            Ok(()) => step("plan", StepStatus::Done, String::new()),
            Err(e) => step("plan", StepStatus::Error, e.to_string()),
        }
    } else {
        step("plan", StepStatus::Skipped, String::new());
    }

    // 4. Lancement
    step("launch", StepStatus::Running, game.name.clone());
    if let Err(e) = starter() {
        step("launch", StepStatus::Error, e.to_string());
        restore_plan(state, previous_plan, &mut step);
        return Err(e);
    }
    step("launch", StepStatus::Done, String::new());

    // 5. Détection du processus du jeu (jusqu'à 3 minutes : mises à jour, lanceur…)
    step("detect", StepStatus::Running, String::new());
    let dir = Path::new(&game.install_dir);
    let mut sys = System::new_with_specifics(RefreshKind::nothing())?;
    let started = Instant::now();
    let mut tuned: HashSet<Pid> = HashSet::new();
    let mut found = false;
    while started.elapsed() < Duration::from_secs(180) && !stop.load(Ordering::Relaxed) {
        if !game_pids(&mut sys, dir).is_empty() {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
    if !found {
        step("detect", StepStatus::Error, "processus du jeu non détecté".into());
        restore_plan(state, previous_plan, &mut step);
        return Ok(());
    }
    step("detect", StepStatus::Done, String::new());

    // 6. Session : réglage de chaque processus du jeu, jusqu'à sa fermeture.
    step("session", StepStatus::Running, String::new());
    let mut absent_since: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        let pids = game_pids(&mut sys, dir);
        if pids.is_empty() {
            let since = *absent_since.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_secs(8) {
                break;
            }
        } else {
            absent_since = None;
            for pid in pids {
                if tuned.insert(pid) && (opts.high_priority || opts.no_throttling) {
                    let ok = tune_process(pid.as_u32(), opts.high_priority, opts.no_throttling);
                    step(
                        "priority",
                        if ok { StepStatus::Done } else { StepStatus::Error },
                        format!("PID {}", pid.as_u32()),
                    );
                }
            }
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    step("session", StepStatus::Done, String::new());
    restore_plan(state, previous_plan, &mut step);
    state
        .logger
        .success("games", format!("Session terminée : {}", game.name));
    Ok(())
}

/// Clé de sauvegarde du mode d'alimentation à rétablir après une session Boost.
const PENDING_PLAN: &str = "boost.previousPlan";

/// Rétablit le mode d'alimentation d'une session Boost interrompue (Aether fermé, arrêt du PC…).
///
/// Impact Windows : `powercfg /setactive` vers le mode d'origine, uniquement si une session
/// n'a pas pu le rétablir elle-même.
pub fn recover_interrupted_session(state: &AppState) {
    let backups = state.backups(MISC_BACKUP);
    let Ok(Some(guid)) = backups.get::<String>(PENDING_PLAN) else {
        return;
    };
    match power::set_active(&guid) {
        Ok(()) => {
            state.logger.info(
                "games",
                "Mode d'alimentation d'origine rétabli après une session Boost interrompue",
            );
            let _ = backups.remove(PENDING_PLAN);
        }
        Err(e) => state.logger.error(
            "games",
            format!("Rétablissement du mode d'alimentation impossible : {e}"),
        ),
    }
}

fn restore_plan<F: FnMut(&'static str, StepStatus, String)>(state: &AppState, previous: Option<String>, step: &mut F) {
    if let Some(guid) = previous {
        match power::set_active(&guid) {
            Ok(()) => {
                // Sauvegarde retirée seulement une fois le mode réellement rétabli.
                let _ = state.backups(MISC_BACKUP).remove(PENDING_PLAN);
                step("restore", StepStatus::Done, String::new());
            }
            Err(e) => {
                state
                    .logger
                    .error("games", format!("Mode d'alimentation non rétabli : {e}"));
                step("restore", StepStatus::Error, e.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_overly_broad_install_dirs() {
        assert!(!is_specific_install_dir(Path::new("C:\\")));
        assert!(!is_specific_install_dir(Path::new("C:\\Program Files")));
        assert!(!is_specific_install_dir(Path::new("D:\\Games")));
        assert!(!is_specific_install_dir(Path::new("C:\\Users\\Alex")));
        assert!(is_specific_install_dir(Path::new("D:\\Games\\Hades")));
        assert!(is_specific_install_dir(Path::new("C:\\GOG Games\\The Witcher 3")));
        assert!(is_specific_install_dir(Path::new(
            "D:\\SteamLibrary\\steamapps\\common\\Portal 2"
        )));
    }

    #[test]
    fn matches_processes_inside_install_dir_only() {
        let dir = Path::new("D:\\Games\\Steam\\steamapps\\common\\Game");
        assert!(under(
            dir,
            Path::new("D:\\Games\\Steam\\steamapps\\common\\Game\\bin\\game.exe")
        ));
        assert!(!under(
            dir,
            Path::new("D:\\Games\\Steam\\steamapps\\common\\GameTwo\\game.exe")
        ));
    }

    #[test]
    fn tunes_a_real_child_process() {
        // Processus de test lancé par nous-mêmes (ping), réglé puis arrêté.
        let child = std::process::Command::new("ping.exe")
            .args(["-n", "5", "127.0.0.1"])
            .spawn();
        if let Ok(mut c) = child {
            assert!(tune_process(c.id(), true, true));
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
