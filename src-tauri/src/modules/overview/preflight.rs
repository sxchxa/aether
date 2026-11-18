//! Pre-Flight Check : protection système (points de restauration) sur le lecteur système.

use serde::{Deserialize, Serialize};

use crate::core::command::{self, ps_quote};
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};

const SPP_CLIENTS: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SPP\\Clients";
const SPP_SYSTEM_RESTORE: &str = "{09F7EDC5-294E-4180-AF6A-FB0E6A0E9513}";
const SR_POLICY: &str = "SOFTWARE\\Policies\\Microsoft\\Windows NT\\SystemRestore";
/// Délai maximal des opérations de restauration système (création de point : jusqu'à plusieurs minutes).
const LONG: std::time::Duration = std::time::Duration::from_secs(600);

/// Dernier point de restauration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePoint {
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Date de création (ISO 8601).
    #[serde(default)]
    pub created: String,
}

/// État de la protection système.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStatus {
    /// Lettre du lecteur système.
    pub drive: String,
    /// Protection système active sur ce lecteur.
    pub enabled: bool,
    /// Désactivée par une stratégie de groupe.
    pub blocked_by_policy: bool,
    /// Dernier point de restauration connu.
    pub last_point: Option<RestorePoint>,
}

fn system_drive() -> String {
    std::env::var("SystemDrive")
        .ok()
        .filter(|d| d.len() == 2 && d.ends_with(':'))
        .unwrap_or_else(|| "C:".to_owned())
}

/// Indique si la protection système couvre `drive` d'après la liste SPP.
pub fn protection_covers(entries: &[String], drive: &str) -> bool {
    let marker = format!("({}%3A)", drive.trim_end_matches(':')).to_lowercase();
    entries.iter().any(|e| e.to_lowercase().contains(&marker))
}

/// Lit l'état de la protection système du lecteur système.
///
/// Impact Windows : lecture seule (Registre SPP, `Get-ComputerRestorePoint`).
pub fn status() -> AetherResult<RestoreStatus> {
    let drive = system_drive();
    let blocked = matches!(
        registry::read(Hive::Hklm, SR_POLICY, "DisableSR")?,
        Some(RegData::Dword(1))
    );
    let entries = registry::read_multi_sz(Hive::Hklm, SPP_CLIENTS, SPP_SYSTEM_RESTORE)?;
    let enabled = !blocked && protection_covers(&entries, &drive);
    let last_point = if enabled {
        command::powershell_json::<RestorePoint>(
            "Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Sort-Object SequenceNumber | Select-Object -Last 1 | \
             ForEach-Object { [pscustomobject]@{ description = [string]$_.Description; created = $_.ConvertToDateTime($_.CreationTime).ToString('o') } } | ConvertTo-Json -Compress",
        )
        .ok()
        .and_then(|v| v.into_iter().next())
    } else {
        None
    };
    Ok(RestoreStatus {
        drive,
        enabled,
        blocked_by_policy: blocked,
        last_point,
    })
}

/// Active la protection système sur le lecteur système (garde-fou : toujours exécuté).
///
/// Impact Windows : `Enable-ComputerRestore -Drive "C:\"`.
pub fn enable(state: &AppState) -> AetherResult<RestoreStatus> {
    let drive = system_drive();
    command::powershell_with_timeout(
        &format!("Enable-ComputerRestore -Drive {}", ps_quote(&format!("{drive}\\"))),
        LONG,
    )?;
    state
        .logger
        .success("restore", format!("[Appliqué] Protection système activée sur {drive}"));
    status()
}

/// Crée un point de restauration (garde-fou : toujours exécuté, même en Mode Aperçu).
///
/// Impact Windows : `Checkpoint-Computer`. La limite Windows d'un point par 24 h est levée
/// temporairement (`SystemRestorePointCreationFrequency = 0`) puis rétablie.
pub fn create_point(state: &AppState, description: &str) -> AetherResult<RestoreStatus> {
    let current = status()?;
    if !current.enabled {
        return Err(AetherError::Forbidden(
            "la protection système est désactivée sur le lecteur système".into(),
        ));
    }
    let desc: String = description.chars().filter(|c| !c.is_control()).take(120).collect();
    state.logger.info("restore", "Création du point de restauration…");
    command::powershell_with_timeout(&format!(
        "$k='HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore'; \
         $old=(Get-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue).SystemRestorePointCreationFrequency; \
         Set-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord; \
         try {{ Checkpoint-Computer -Description {} -RestorePointType MODIFY_SETTINGS }} \
         finally {{ if ($null -eq $old) {{ Remove-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue }} else {{ Set-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value $old -Type DWord }} }}",
        ps_quote(&desc)
    ), LONG)?;
    state
        .logger
        .success("restore", format!("[Appliqué] Point de restauration « {desc} » créé"));
    status()
}

/// Point de restauration de la liste complète.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePointEntry {
    /// Numéro de séquence (identifiant Windows du point).
    pub sequence: u32,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Date de création (ISO 8601).
    #[serde(default)]
    pub created: String,
    /// Type Windows (0 installation, 10 pilote, 12 modification des réglages, 13 annulation…).
    #[serde(default)]
    pub kind: u32,
}

/// Liste tous les points de restauration (du plus récent au plus ancien).
///
/// Impact Windows : lecture seule (`Get-ComputerRestorePoint`).
pub fn list_points() -> AetherResult<Vec<RestorePointEntry>> {
    command::powershell_json::<RestorePointEntry>(
        "Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Sort-Object SequenceNumber -Descending |          ForEach-Object { [pscustomobject]@{ sequence = [int]$_.SequenceNumber; description = [string]$_.Description;          created = $_.ConvertToDateTime($_.CreationTime).ToString('o'); kind = [int]$_.RestorePointType } } | ConvertTo-Json -Compress",
    )
}

/// Programme le retour du système à un point de restauration ; il s'effectue au redémarrage.
///
/// Impact Windows : WMI `SystemRestore.Restore(sequence)` (fichiers personnels conservés).
pub fn restore_to(state: &AppState, sequence: u32) -> AetherResult<()> {
    if !list_points()?.iter().any(|p| p.sequence == sequence) {
        return Err(AetherError::NotFound(format!("point de restauration n° {sequence}")));
    }
    let out = command::powershell_with_timeout(
        &format!(r"([wmiclass]'\\.\root\default:SystemRestore').Restore({sequence}).ReturnValue"),
        LONG,
    )?;
    if out.trim() != "0" {
        return Err(AetherError::Command {
            command: "SystemRestore.Restore".into(),
            code: out.trim().parse().ok(),
            stderr: format!("Windows a refusé la restauration (code {})", out.trim()),
        });
    }
    state.logger.success(
        "restore",
        format!("[Programmé] Retour au point de restauration n° {sequence} au prochain redémarrage"),
    );
    Ok(())
}

/// Redémarre le PC dans 10 secondes (pour appliquer une restauration du système).
pub fn restart_soon() -> AetherResult<()> {
    command::run(
        "shutdown.exe",
        &["/r", "/t", "10", "/c", "Aether : restauration du système"],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_protected_drive_in_spp_entries() {
        let entries = vec!["\\\\?\\Volume{abc}\\:(C%3A)".to_owned()];
        assert!(protection_covers(&entries, "C:"));
        assert!(!protection_covers(&entries, "D:"));
        assert!(!protection_covers(&[], "C:"));
    }
}
