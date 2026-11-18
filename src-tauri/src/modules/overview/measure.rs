//! Mesures « avant / après » : une mesure de référence est prise au premier lancement (avant toute
//! optimisation), puis comparée aux mesures suivantes. Lecture seule : aucune modification du système.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sysinfo::{MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use crate::core::command;
use crate::core::logger::now_ms;
use crate::core::state::AppState;
use crate::error::AetherResult;
use crate::modules::system::startup;

const FILE: &str = "aether_measures.json";
/// Nombre de mesures conservées dans l'historique.
const KEEP: usize = 30;

/// Une mesure de l'état du PC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Measure {
    /// Horodatage (ms Unix).
    pub ts: u64,
    /// Durée du dernier démarrage de Windows en ms (journal Diagnostics-Performance), si disponible.
    pub boot_ms: Option<u64>,
    /// Mémoire utilisée (octets).
    pub ram_used: u64,
    /// Mémoire totale (octets).
    pub ram_total: u64,
    /// Nombre de processus.
    pub processes: u32,
    /// Services Windows en cours d'exécution.
    pub services_running: u32,
    /// Programmes lancés à l'ouverture de session.
    pub startup_items: u32,
}

/// Référence et dernière mesure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measures {
    /// Mesure de référence (avant optimisation).
    pub reference: Option<Measure>,
    /// Historique des mesures (la plus récente en dernier).
    pub history: Vec<Measure>,
}

fn path(state: &AppState) -> PathBuf {
    state.data_dir.join(FILE)
}

fn load(file: &Path) -> Measures {
    fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(file: &Path, m: &Measures) -> AetherResult<()> {
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(m)?)?;
    fs::rename(&tmp, file)?;
    Ok(())
}

/// Extrait la durée de démarrage (`BootTime`, ms) d'un évènement 100 au format XML.
pub fn parse_boot_time(xml: &str) -> Option<u64> {
    let start = xml
        .find("Name='BootTime'>")
        .map(|i| i + "Name='BootTime'>".len())
        .or_else(|| xml.find("Name=\"BootTime\">").map(|i| i + "Name=\"BootTime\">".len()))?;
    let digits: String = xml[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|ms| *ms > 0)
}

/// Durée du dernier démarrage de Windows (journal officiel `Diagnostics-Performance`).
///
/// Impact Windows : lecture seule (`wevtutil qe`). Absente si Windows ne l'a pas enregistrée
/// (machine virtuelle, démarrage rapide…).
pub fn last_boot_ms() -> Option<u64> {
    let out = command::run_raw(
        "wevtutil.exe",
        &[
            "qe",
            "Microsoft-Windows-Diagnostics-Performance/Operational",
            "/q:*[System[(EventID=100)]]",
            "/c:1",
            "/rd:true",
            "/f:xml",
        ],
    )
    .ok()?;
    parse_boot_time(&out.stdout)
}

/// Nombre de services Windows actifs (API native, indépendante de la langue du système).
fn services_running() -> u32 {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE,
        SERVICE_ACTIVE, SERVICE_WIN32,
    };
    // SAFETY : handle fermé en fin de fonction ; tampon dimensionné d'après la taille demandée par l'API.
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ENUMERATE_SERVICE);
        if scm.is_null() {
            return 0;
        }
        let (mut needed, mut count, mut resume) = (0u32, 0u32, 0u32);
        // Premier appel : taille nécessaire.
        EnumServicesStatusExW(
            scm,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_ACTIVE,
            std::ptr::null_mut(),
            0,
            &mut needed,
            &mut count,
            &mut resume,
            std::ptr::null(),
        );
        let mut buf = vec![0u8; needed as usize + 4096];
        resume = 0;
        let ok = EnumServicesStatusExW(
            scm,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_ACTIVE,
            buf.as_mut_ptr(),
            buf.len() as u32,
            &mut needed,
            &mut count,
            &mut resume,
            std::ptr::null(),
        );
        CloseServiceHandle(scm);
        if ok != 0 {
            count
        } else {
            0
        }
    }
}
/// Nombre de relevés par mesure.
const SAMPLES: usize = 3;

/// Relevé médian (mémoire utilisée, mémoire totale, processus), triés par mémoire utilisée.
pub fn median_sample(samples: &mut [(u64, u64, u32)]) -> (u64, u64, u32) {
    if samples.is_empty() {
        return (0, 0, 0);
    }
    samples.sort_by_key(|s| s.0);
    let mid = samples[samples.len() / 2];
    let mut procs: Vec<u32> = samples.iter().map(|s| s.2).collect();
    procs.sort_unstable();
    (mid.0, mid.1, procs[procs.len() / 2])
}

/// Prend une mesure de l'état actuel du PC.
///
/// Impact Windows : lecture seule (mémoire, processus, services, programmes de démarrage, journal).
pub fn take() -> Measure {
    // Trois relevés à une seconde d'intervalle, valeur médiane : un pic passager ne fausse pas la mesure.
    let mut samples: Vec<(u64, u64, u32)> = Vec::with_capacity(SAMPLES);
    if let Ok(mut sys) =
        System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()))
    {
        for i in 0..SAMPLES {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            sys.refresh_memory();
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            samples.push((sys.used_memory(), sys.total_memory(), sys.processes().len() as u32));
        }
    }
    let (ram_used, ram_total, processes) = median_sample(&mut samples);
    Measure {
        ts: now_ms(),
        boot_ms: last_boot_ms(),
        ram_used,
        ram_total,
        processes,
        services_running: services_running(),
        startup_items: startup::list()
            .map(|l| l.iter().filter(|e| e.enabled).count() as u32)
            .unwrap_or(0),
    }
}

/// Mesures enregistrées.
pub fn get(state: &AppState) -> Measures {
    load(&path(state))
}

/// Prend une nouvelle mesure et l'enregistre (elle devient la référence s'il n'y en a pas encore).
pub fn record(state: &AppState) -> AetherResult<Measures> {
    let file = path(state);
    let mut m = load(&file);
    let now = take();
    if m.reference.is_none() {
        m.reference = Some(now.clone());
    }
    m.history.push(now);
    if m.history.len() > KEEP {
        let excess = m.history.len() - KEEP;
        m.history.drain(..excess);
    }
    save(&file, &m)?;
    Ok(m)
}

/// Fait de la dernière mesure la nouvelle référence.
pub fn reset_reference(state: &AppState) -> AetherResult<Measures> {
    let file = path(state);
    let mut m = load(&file);
    m.reference = m.history.last().cloned().or_else(|| Some(take()));
    save(&file, &m)?;
    Ok(m)
}

/// Mesure de référence prise au tout premier lancement (avant optimisation), en arrière-plan.
pub fn ensure_reference(state: &AppState) {
    if load(&path(state)).reference.is_none() {
        let _ = record(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_ignores_a_single_spike() {
        let mut s = vec![(4_000, 16_000, 120), (9_000, 16_000, 180), (4_200, 16_000, 122)];
        assert_eq!(median_sample(&mut s), (4_200, 16_000, 122));
        assert_eq!(median_sample(&mut []), (0, 0, 0));
    }

    #[test]
    fn parses_boot_time_from_event_xml() {
        let xml = "<Event><EventData><Data Name='BootTsVersion'>2</Data><Data Name='BootTime'>31427</Data></EventData></Event>";
        assert_eq!(parse_boot_time(xml), Some(31_427));
        assert_eq!(parse_boot_time("<Data Name=\"BootTime\">0</Data>"), None);
        assert_eq!(parse_boot_time(""), None);
    }

    #[test]
    fn first_measure_becomes_reference_and_history_is_capped() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        let first = record(&state)?;
        let reference = first.reference.clone();
        assert!(reference.is_some());
        assert!(first.history[0].ram_total > 0 && first.history[0].processes > 0);
        assert!(first.history[0].services_running > 10, "services en cours non comptés");
        let mut m = first;
        m.history = vec![m.history[0].clone(); KEEP + 5];
        save(&path(&state), &m)?;
        let after = record(&state)?;
        assert_eq!(after.history.len(), KEEP);
        assert_eq!(after.reference, reference, "la référence ne change pas d'elle-même");
        Ok(())
    }
}
