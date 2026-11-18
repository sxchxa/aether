//! Purge de la mémoire vive (vidage des working sets via `K32EmptyWorkingSet`).

use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA};

use crate::error::AetherResult;

/// Résultat d'une purge RAM.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RamReport {
    /// Mémoire utilisée avant (octets).
    pub used_before: u64,
    /// Mémoire utilisée après (octets).
    pub used_after: u64,
    /// Mémoire libérée (octets).
    pub freed: u64,
    /// Nombre de processus dont le working set a été vidé.
    pub trimmed: u32,
    /// Mémoire totale (octets).
    pub total: u64,
}

fn empty_working_set(pid: u32) -> bool {
    // SAFETY : handle ouvert puis systématiquement fermé ; aucun pointeur partagé.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, 0, pid);
        if handle.is_null() {
            return false;
        }
        let ok = K32EmptyWorkingSet(handle) != 0;
        CloseHandle(handle);
        ok
    }
}

/// Vide le working set de tous les processus accessibles.
///
/// Impact Windows : les pages inactives sont renvoyées dans la liste de veille ; les
/// processus les rechargent à la demande. Aucun effet persistant.
pub fn purge() -> AetherResult<RamReport> {
    let mut sys = System::new_with_specifics(RefreshKind::nothing())?;
    sys.refresh_memory();
    let used_before = sys.used_memory();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let mut trimmed = 0u32;
    for pid in sys.processes().keys() {
        let raw = pid.as_u32();
        if raw > 4 && empty_working_set(raw) {
            trimmed += 1;
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    sys.refresh_memory();
    let used_after = sys.used_memory();
    Ok(RamReport {
        used_before,
        used_after,
        freed: used_before.saturating_sub(used_after),
        trimmed,
        total: sys.total_memory(),
    })
}
