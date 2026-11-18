//! Regroupement des services Windows selon la mémoire installée (`SvcHostSplitThresholdInKB`).
//!
//! Microsoft (« Changes to Service Host grouping in Windows 10 ») : au-dessus de ce seuil, chaque
//! service tourne dans son propre processus `svchost.exe`. En le portant à la quantité de mémoire
//! du PC, les services sont regroupés dans moins de processus : moins de mémoire consommée et
//! moins de processus à planifier. La valeur est calculée pour chaque PC (8, 16, 32 Go…).

use crate::core::history::UndoAction;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, REGISTRY_BACKUP};
use crate::core::tweak::{Probe, Risk, Section, Tweak};
use crate::error::AetherResult;

const KEY: &str = r"SYSTEM\CurrentControlSet\Control";
const NAME: &str = "SvcHostSplitThresholdInKB";
/// Valeur par défaut de Windows (3,5 Go).
const WINDOWS_DEFAULT_KB: u32 = 0x0038_0000;

/// Mémoire installée en Ko (bornée à la capacité d'un DWORD).
pub fn installed_kb() -> u32 {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    let bytes = System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()))
        .map(|mut sys| {
            sys.refresh_memory();
            sys.total_memory()
        })
        .unwrap_or(0);
    u32::try_from(bytes / 1024).unwrap_or(u32::MAX)
}

/// Valeur cible pour une mémoire donnée : la mémoire installée, arrondie au Go supérieur.
pub fn target_kb(installed_kb: u32) -> u32 {
    const GIB_KB: u32 = 1024 * 1024;
    installed_kb
        .div_ceil(GIB_KB)
        .saturating_mul(GIB_KB)
        .max(WINDOWS_DEFAULT_KB)
}

fn gib(kb: u32) -> String {
    format!("{:.0} Go", f64::from(kb) / 1024.0 / 1024.0)
}

/// Option « services regroupés selon la mémoire ».
pub struct SvcHostSplitTweak;

impl SvcHostSplitTweak {
    fn backup_key() -> String {
        format!("{}\\{KEY}\\{NAME}", Hive::Hklm.label())
    }
}

impl Tweak for SvcHostSplitTweak {
    fn id(&self) -> &'static str {
        "mem.svchostSplit"
    }
    fn section(&self) -> Section {
        Section::Performance
    }
    fn group(&self) -> &'static str {
        "gaming"
    }
    fn risk(&self) -> Risk {
        Risk::Moderate
    }
    fn technical(&self) -> String {
        format!(
            "HKLM\\{KEY}\\{NAME} = mémoire installée en Ko ({} sur ce PC)",
            target_kb(installed_kb())
        )
    }
    fn needs_reboot(&self) -> bool {
        true
    }
    fn check_status(&self, _state: &AppState) -> AetherResult<Probe> {
        let target = target_kb(installed_kb());
        let current = match registry::read(Hive::Hklm, KEY, NAME)? {
            Some(RegData::Dword(v)) => v,
            _ => WINDOWS_DEFAULT_KB,
        };
        Ok(Probe {
            applied: current >= target,
            available: true,
            current: format!("{} ({current} Ko)", gib(current)),
            target: format!("{} ({target} Ko)", gib(target)),
        })
    }
    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let previous = registry::read(Hive::Hklm, KEY, NAME)?;
        state
            .backups(REGISTRY_BACKUP)
            .record_first(&Self::backup_key(), &previous)?;
        registry::write(Hive::Hklm, KEY, NAME, &RegData::Dword(target_kb(installed_kb())))?;
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hklm,
            path: KEY.to_owned(),
            name: NAME.to_owned(),
            previous,
        }])
    }
    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>> {
        let backups = state.backups(REGISTRY_BACKUP);
        let current = registry::read(Hive::Hklm, KEY, NAME)?;
        let original = backups
            .get::<Option<RegData>>(&Self::backup_key())?
            .unwrap_or(Some(RegData::Dword(WINDOWS_DEFAULT_KB)));
        match &original {
            Some(data) => registry::write(Hive::Hklm, KEY, NAME, data)?,
            None => registry::delete_value(Hive::Hklm, KEY, NAME)?,
        }
        backups.remove(&Self::backup_key())?;
        Ok(vec![UndoAction::Registry {
            hive: Hive::Hklm,
            path: KEY.to_owned(),
            name: NAME.to_owned(),
            previous: current,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_follows_installed_memory() {
        const GIB: u32 = 1024 * 1024;
        // 16 Go affichés par Windows (un peu moins en réalité) → 16 Go.
        assert_eq!(target_kb(16 * GIB - 300_000), 16 * GIB);
        assert_eq!(target_kb(8 * GIB), 8 * GIB);
        assert_eq!(target_kb(31 * GIB + 10), 32 * GIB);
        // Jamais en dessous de la valeur par défaut de Windows.
        assert_eq!(target_kb(2 * GIB), WINDOWS_DEFAULT_KB);
    }
}
