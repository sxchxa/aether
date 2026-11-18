//! Diagnostics en lecture seule : santé des disques (SMART), impact des programmes au démarrage
//! et rapport de batterie. Aucune de ces fonctions ne modifie Windows.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::core::command;
use crate::error::{AetherError, AetherResult};
use crate::modules::{blocking, Shared};

// ───────────────────────── Santé des disques ─────────────────────────

/// Verdict de santé d'un disque.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiskVerdict {
    /// Aucun signe de faiblesse.
    Good,
    /// Usure avancée ou erreurs : sauvegardez vos données.
    Warning,
    /// Panne prévue par le disque : remplacez-le rapidement.
    Critical,
    /// Le disque ne communique pas son état (machine virtuelle, contrôleur RAID…).
    Unknown,
}

/// Données brutes renvoyées par `Get-PhysicalDisk` + `Get-StorageReliabilityCounter`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawDisk {
    name: Option<String>,
    media: Option<String>,
    bus: Option<String>,
    size: Option<u64>,
    health: Option<String>,
    temperature: Option<u32>,
    wear: Option<u32>,
    power_on_hours: Option<u64>,
    read_errors: Option<u64>,
    write_errors: Option<u64>,
}

/// Santé d'un disque physique.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskHealth {
    /// Nom du modèle.
    pub name: String,
    /// Type (SSD, HDD…).
    pub media: String,
    /// Interface (NVMe, SATA…).
    pub bus: String,
    /// Capacité (octets).
    pub size: u64,
    /// Verdict.
    pub verdict: DiskVerdict,
    /// Température (°C), si le disque la communique.
    pub temperature: Option<u32>,
    /// Usure en % (SSD), si disponible.
    pub wear: Option<u32>,
    /// Heures de fonctionnement, si disponibles.
    pub power_on_hours: Option<u64>,
    /// Erreurs de lecture/écriture non corrigées.
    pub uncorrected_errors: u64,
}

/// Établit le verdict à partir de l'état Windows, de l'usure et des erreurs.
pub fn verdict(health: Option<&str>, wear: Option<u32>, errors: u64) -> DiskVerdict {
    match health.map(str::to_ascii_lowercase).as_deref() {
        Some("unhealthy") => DiskVerdict::Critical,
        Some("warning") => DiskVerdict::Warning,
        _ if wear.is_some_and(|w| w >= 90) => DiskVerdict::Critical,
        _ if wear.is_some_and(|w| w >= 80) || errors > 0 => DiskVerdict::Warning,
        Some("healthy") => DiskVerdict::Good,
        _ => DiskVerdict::Unknown,
    }
}

const DISKS_SCRIPT: &str = r#"Get-PhysicalDisk | ForEach-Object {
  $r = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
  [pscustomobject]@{
    Name = $_.FriendlyName; Media = [string]$_.MediaType; Bus = [string]$_.BusType; Size = [uint64]$_.Size
    Health = [string]$_.HealthStatus; Temperature = $r.Temperature; Wear = $r.Wear; PowerOnHours = $r.PowerOnHours
    ReadErrors = $r.ReadErrorsUncorrected; WriteErrors = $r.WriteErrorsUncorrected
  }
} | ConvertTo-Json -Compress"#;

/// Santé de tous les disques physiques.
///
/// Impact Windows : lecture seule (`Get-PhysicalDisk`, `Get-StorageReliabilityCounter`).
pub fn disks() -> AetherResult<Vec<DiskHealth>> {
    let raw: Vec<RawDisk> = command::powershell_json(DISKS_SCRIPT)?;
    Ok(raw
        .into_iter()
        .map(|d| {
            let errors = d.read_errors.unwrap_or(0) + d.write_errors.unwrap_or(0);
            DiskHealth {
                name: d.name.unwrap_or_default(),
                media: d.media.unwrap_or_default(),
                bus: d.bus.unwrap_or_default(),
                size: d.size.unwrap_or(0),
                verdict: verdict(d.health.as_deref(), d.wear, errors),
                // 0 °C signifie « non communiqué » sur la plupart des contrôleurs.
                temperature: d.temperature.filter(|t| *t > 0),
                wear: d.wear,
                power_on_hours: d.power_on_hours.filter(|h| *h > 0),
                uncorrected_errors: errors,
            }
        })
        .collect())
}

// ───────────────────────── Impact au démarrage ─────────────────────────

/// Ralentissement du démarrage causé par un programme (journal Diagnostics-Performance, évènement 101).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StartupImpact {
    /// Nom du fichier exécutable (minuscules), ex. `onedrive.exe`.
    pub file: String,
    /// Retard maximal observé (ms).
    pub degradation_ms: u64,
}

fn data_value<'a>(event: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("Name='{name}'>");
    let start = event.find(&key)? + key.len();
    let end = event[start..].find('<')? + start;
    Some(event[start..end].trim())
}

/// Extrait les retards par programme d'une liste d'évènements XML (le plus grand retard est conservé).
pub fn parse_startup_events(xml: &str) -> Vec<StartupImpact> {
    let mut worst: HashMap<String, u64> = HashMap::new();
    for event in xml.split("<Event ").skip(1) {
        let Some(file) = data_value(event, "FileName").filter(|f| !f.is_empty()) else {
            continue;
        };
        let ms = data_value(event, "DegradationTime")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let entry = worst.entry(file.to_ascii_lowercase()).or_insert(0);
        *entry = (*entry).max(ms);
    }
    let mut out: Vec<StartupImpact> = worst
        .into_iter()
        .map(|(file, degradation_ms)| StartupImpact { file, degradation_ms })
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(i.degradation_ms));
    out
}

/// Programmes ayant ralenti le démarrage de Windows d'après le journal officiel.
///
/// Impact Windows : lecture seule (`wevtutil qe`, 200 évènements les plus récents).
pub fn startup_impact() -> Vec<StartupImpact> {
    command::run_raw(
        "wevtutil.exe",
        &[
            "qe",
            "Microsoft-Windows-Diagnostics-Performance/Operational",
            "/q:*[System[(EventID=101)]]",
            "/c:200",
            "/rd:true",
            "/f:xml",
        ],
    )
    .map(|o| parse_startup_events(&o.stdout))
    .unwrap_or_default()
}

// ───────────────────────── Batterie ─────────────────────────

/// État de la batterie (rapport officiel `powercfg /batteryreport`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryHealth {
    /// Capacité d'origine (mWh).
    pub design_mwh: u64,
    /// Capacité actuelle à pleine charge (mWh).
    pub full_mwh: u64,
    /// Nombre de cycles de charge, si communiqué.
    pub cycles: Option<u64>,
    /// Usure (% de capacité perdue).
    pub wear_percent: f64,
}

fn tag_value(xml: &str, tag: &str) -> Option<u64> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find('<')? + start;
    xml[start..end].trim().parse().ok()
}

/// Lit la première batterie d'un rapport XML ; `None` s'il n'y en a pas.
pub fn parse_battery_report(xml: &str) -> Option<BatteryHealth> {
    let battery = &xml[xml.find("<Battery>")?..];
    let design = tag_value(battery, "DesignCapacity").filter(|v| *v > 0)?;
    let full = tag_value(battery, "FullChargeCapacity")?;
    let wear = (1.0 - full as f64 / design as f64) * 100.0;
    Some(BatteryHealth {
        design_mwh: design,
        full_mwh: full,
        cycles: tag_value(battery, "CycleCount").filter(|c| *c > 0),
        wear_percent: (wear.max(0.0) * 10.0).round() / 10.0,
    })
}

/// État de la batterie, ou `None` sur un PC sans batterie.
///
/// Impact Windows : génère un rapport XML temporaire (`powercfg /batteryreport /xml`), puis le supprime.
pub fn battery() -> AetherResult<Option<BatteryHealth>> {
    if !crate::modules::performance::power::has_battery() {
        return Ok(None);
    }
    let file = std::env::temp_dir().join(format!("aether-battery-{}.xml", std::process::id()));
    let path = file.display().to_string();
    command::run_with_timeout(
        "powercfg.exe",
        &["/batteryreport", "/xml", "/output", &path],
        std::time::Duration::from_secs(60),
    )?;
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    Ok(parse_battery_report(&text))
}

/// Génère le rapport de batterie complet de Windows (HTML) et l'ouvre dans le navigateur.
///
/// Impact Windows : `powercfg /batteryreport` vers le dossier de données d'Aether, puis ouverture du fichier.
pub fn open_battery_report(data_dir: &std::path::Path) -> AetherResult<()> {
    let file = data_dir.join("rapport-batterie.html");
    let path = file.display().to_string();
    command::run_with_timeout(
        "powercfg.exe",
        &["/batteryreport", "/output", &path],
        std::time::Duration::from_secs(60),
    )?;
    if !file.is_file() {
        return Err(AetherError::NotFound("rapport de batterie".into()));
    }
    command::hidden("explorer.exe").arg(&path).spawn()?;
    Ok(())
}

// ───────────────────────── Commandes ─────────────────────────

/// Santé des disques physiques.
#[tauri::command]
pub async fn disks_health(state: State<'_, Shared>) -> AetherResult<Vec<DiskHealth>> {
    blocking(&state, |_| disks()).await
}

/// Ralentissements du démarrage par programme.
#[tauri::command]
pub async fn startup_impacts(state: State<'_, Shared>) -> AetherResult<Vec<StartupImpact>> {
    blocking(&state, |_| Ok(startup_impact())).await
}

/// État de la batterie.
#[tauri::command]
pub async fn battery_health(state: State<'_, Shared>) -> AetherResult<Option<BatteryHealth>> {
    blocking(&state, |_| battery()).await
}

/// Ouvre le rapport de batterie complet.
#[tauri::command]
pub async fn battery_report_open(state: State<'_, Shared>) -> AetherResult<()> {
    blocking(&state, |s| open_battery_report(&s.data_dir)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_verdicts() {
        assert_eq!(verdict(Some("Healthy"), Some(5), 0), DiskVerdict::Good);
        assert_eq!(verdict(Some("Healthy"), Some(85), 0), DiskVerdict::Warning);
        assert_eq!(verdict(Some("Healthy"), Some(95), 0), DiskVerdict::Critical);
        assert_eq!(verdict(Some("Healthy"), None, 3), DiskVerdict::Warning);
        assert_eq!(verdict(Some("Unhealthy"), None, 0), DiskVerdict::Critical);
        assert_eq!(verdict(None, None, 0), DiskVerdict::Unknown);
    }

    #[test]
    fn parses_startup_degradation_events() {
        let xml = "<Events><Event xmlns='x'><EventData><Data Name='FileName'>OneDrive.exe</Data><Data Name='DegradationTime'>1830</Data></EventData></Event>\
                   <Event xmlns='x'><EventData><Data Name='FileName'>OneDrive.exe</Data><Data Name='DegradationTime'>2400</Data></EventData></Event>\
                   <Event xmlns='x'><EventData><Data Name='FileName'>Discord.exe</Data><Data Name='DegradationTime'>600</Data></EventData></Event></Events>";
        let out = parse_startup_events(xml);
        assert_eq!(
            out[0],
            StartupImpact {
                file: "onedrive.exe".into(),
                degradation_ms: 2400
            }
        );
        assert_eq!(out[1].file, "discord.exe");
        assert!(parse_startup_events("").is_empty());
    }

    #[test]
    fn parses_battery_report() {
        let xml = "<BatteryReport><Batteries><Battery><Id>X</Id><DesignCapacity>50000</DesignCapacity><FullChargeCapacity>42500</FullChargeCapacity><CycleCount>312</CycleCount></Battery></Batteries></BatteryReport>";
        let b = parse_battery_report(xml).expect("batterie");
        assert_eq!((b.design_mwh, b.full_mwh, b.cycles), (50000, 42500, Some(312)));
        assert!((b.wear_percent - 15.0).abs() < 0.01);
        assert!(parse_battery_report("<BatteryReport><Batteries></Batteries></BatteryReport>").is_none());
    }
}
