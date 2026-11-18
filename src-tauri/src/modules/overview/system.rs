//! Informations matérielles : OS, CPU, RAM, disques (SSD/NVMe/HDD) et batterie.

use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, DiskKind, Disks, RefreshKind, System};

use crate::core::command;
use crate::error::AetherResult;
use crate::modules::performance::gaming::{self, DisplayInfo};

/// Volume logique.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    /// Point de montage (ex. `C:\`).
    pub mount: String,
    /// Nom du volume.
    pub name: String,
    /// `ssd`, `hdd` ou `unknown`.
    pub kind: String,
    /// Système de fichiers.
    pub file_system: String,
    /// Capacité totale (octets).
    pub total: u64,
    /// Espace libre (octets).
    pub available: u64,
    /// Lecteur amovible.
    pub removable: bool,
}

/// Disque physique.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalDisk {
    /// Nom commercial.
    #[serde(default)]
    pub name: String,
    /// Type de média (`SSD`, `HDD`, `Unspecified`).
    #[serde(default)]
    pub media: String,
    /// Bus (`NVMe`, `SATA`, `USB`…).
    #[serde(default)]
    pub bus: String,
    /// Taille (octets).
    #[serde(default)]
    pub size: u64,
}

/// État de la batterie (PC portable).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryInfo {
    /// Charge restante (%).
    #[serde(default)]
    pub charge: u32,
    /// Capacité à pleine charge actuelle (mWh).
    #[serde(default)]
    pub full: u64,
    /// Capacité d'origine (mWh).
    #[serde(default)]
    pub design: u64,
    /// Usure estimée (%), absente si non mesurable.
    #[serde(default)]
    pub wear_percent: Option<f64>,
}

/// Fiche système complète.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// Nom de l'OS.
    pub os_name: String,
    /// Version / build.
    pub os_version: String,
    /// Nom du PC.
    pub hostname: String,
    /// Processeur.
    pub cpu: String,
    /// Nombre de threads logiques.
    pub threads: usize,
    /// RAM totale (octets).
    pub total_memory: u64,
    /// Temps de fonctionnement (s).
    pub uptime: u64,
    /// Volumes.
    pub disks: Vec<DiskInfo>,
    /// Disques physiques.
    pub physical_disks: Vec<PhysicalDisk>,
    /// Batterie (absente sur un PC fixe).
    pub battery: Option<BatteryInfo>,
    /// Écran principal.
    pub display: Option<DisplayInfo>,
}

/// Informations matérielles lentes à obtenir (WMI), chargées séparément.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardwareExtra {
    /// Disques physiques.
    #[serde(default)]
    pub disks: Vec<PhysicalDisk>,
    /// Batterie (absente sur un PC fixe).
    #[serde(default)]
    pub battery: Option<BatteryInfo>,
}

const HARDWARE_SCRIPT: &str = "$d = @(Get-PhysicalDisk -ErrorAction SilentlyContinue | Select-Object @{n='name';e={[string]$_.FriendlyName}}, @{n='media';e={[string]$_.MediaType}}, @{n='bus';e={[string]$_.BusType}}, @{n='size';e={[uint64]$_.Size}}); \
$b = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue | Select-Object -First 1; $bat = $null; \
if ($b) { $f = Get-CimInstance -Namespace root/wmi -ClassName BatteryFullChargedCapacity -ErrorAction SilentlyContinue | Select-Object -First 1; \
$s = Get-CimInstance -Namespace root/wmi -ClassName BatteryStaticData -ErrorAction SilentlyContinue | Select-Object -First 1; \
$bat = [pscustomobject]@{ charge = [int]$b.EstimatedChargeRemaining; full = [uint64]$f.FullChargedCapacity; design = [uint64]$s.DesignedCapacity } } \
[pscustomobject]@{ disks = $d; battery = $bat } | ConvertTo-Json -Depth 4 -Compress";

/// Volumes logiques (rapide, via `sysinfo`).
///
/// Impact Windows : lecture seule.
pub fn disks() -> Vec<DiskInfo> {
    let Ok(list) = Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    list.iter()
        .map(|d| DiskInfo {
            mount: d.mount_point().display().to_string(),
            name: d.name().to_string_lossy().into_owned(),
            kind: match d.kind() {
                DiskKind::SSD => "ssd",
                DiskKind::HDD => "hdd",
                _ => "unknown",
            }
            .to_owned(),
            file_system: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            available: d.available_space(),
            removable: d.is_removable(),
        })
        .collect()
}

/// Calcule l'usure de la batterie en pourcentage.
pub fn wear_percent(full: u64, design: u64) -> Option<f64> {
    if full == 0 || design == 0 {
        return None;
    }
    let wear = 100.0 - (full as f64 / design as f64 * 100.0);
    Some((wear.clamp(0.0, 100.0) * 10.0).round() / 10.0)
}

/// Disques physiques (SSD/NVMe/HDD) et usure de la batterie.
///
/// Impact Windows : lecture seule (`Get-PhysicalDisk`, WMI batterie) — lent, appelé une fois.
pub fn hardware() -> HardwareExtra {
    let mut extra = command::powershell(HARDWARE_SCRIPT)
        .ok()
        .and_then(|raw| serde_json::from_str::<HardwareExtra>(raw.trim()).ok())
        .unwrap_or_default();
    if let Some(b) = extra.battery.as_mut() {
        b.wear_percent = wear_percent(b.full, b.design);
    }
    extra
}

/// Collecte la fiche système (rapide : sans WMI ni PowerShell).
///
/// Impact Windows : lecture seule (sysinfo, `EnumDisplaySettingsW`).
pub fn info() -> AetherResult<SystemInfo> {
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram()),
    )?;
    Ok(SystemInfo {
        os_name: System::long_os_version().unwrap_or_else(|_| "Windows".into()),
        os_version: System::kernel_version().unwrap_or_default(),
        hostname: System::host_name().unwrap_or_default(),
        cpu: sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_owned())
            .unwrap_or_default(),
        threads: sys.cpus().len(),
        total_memory: sys.total_memory(),
        uptime: System::uptime().unwrap_or(0),
        disks: disks(),
        physical_disks: Vec::new(),
        battery: None,
        display: gaming::display_info().ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_battery_wear() {
        assert_eq!(wear_percent(45_000, 50_000), Some(10.0));
        assert_eq!(wear_percent(0, 50_000), None);
        assert_eq!(wear_percent(60_000, 50_000), Some(0.0));
    }

    #[test]
    fn lists_at_least_the_system_volume() {
        assert!(disks().iter().any(|d| d.mount.to_uppercase().starts_with("C:")));
    }

    #[test]
    fn parses_hardware_json() -> AetherResult<()> {
        let raw = r#"{"disks":[{"name":"Samsung 990","media":"SSD","bus":"NVMe","size":1000}],"battery":null}"#;
        let extra: HardwareExtra = serde_json::from_str(raw)?;
        assert_eq!(extra.disks[0].bus, "NVMe");
        assert!(extra.battery.is_none());
        Ok(())
    }
}
