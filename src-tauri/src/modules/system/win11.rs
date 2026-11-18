//! Passage de Windows 10 à Windows 11 (mise à niveau sur place : fichiers et applications conservés).
//!
//! - PC compatible : l'Assistant d'installation officiel de Microsoft est téléchargé puis lancé en
//!   mode automatique (`/SkipEULA /auto upgrade`).
//! - PC non compatible (TPM, processeur, Secure Boot) : Microsoft ne propose pas de mise à niveau
//!   automatique. Aether utilise l'image ISO officielle (téléchargée depuis microsoft.com), applique
//!   la clé documentée par Microsoft `AllowUpgradesWithUnsupportedTPMOrCPU`, puis lance le programme
//!   d'installation de l'image en mode automatique sans contrôle matériel (`/product server`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::core::command;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};
use crate::modules::system::context_menu::windows_build;

/// Premier numéro de build de Windows 11.
const WIN11_FIRST_BUILD: u32 = 22_000;
/// Assistant d'installation de Windows 11 (lien officiel Microsoft).
const ASSISTANT_URL: &str = "https://go.microsoft.com/fwlink/?linkid=2171764";
/// Page officielle de téléchargement de l'image ISO de Windows 11.
pub const ISO_PAGE: &str = "https://www.microsoft.com/software-download/windows11";
/// Espace libre minimal recommandé pour la mise à niveau.
const MIN_FREE_BYTES: u64 = 25 * 1024 * 1024 * 1024;
/// 4 Go (avec une marge pour la mémoire réservée par le matériel).
const MIN_RAM_BYTES: u64 = 4 * 1024 * 1024 * 1024 - 256 * 1024 * 1024;
const MOSETUP_KEY: &str = r"SYSTEM\Setup\MoSetup";

/// Compatibilité matérielle avec Windows 11.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Win11Status {
    /// Ce PC est sous Windows 10 (l'option n'est proposée que dans ce cas).
    pub is_windows10: bool,
    /// Numéro de build de Windows.
    pub build: u32,
    /// Version de la puce TPM (« 2.0 », « 1.2 ») ou `None` si absente.
    pub tpm: Option<String>,
    /// Démarrage UEFI avec Secure Boot activé.
    pub secure_boot: bool,
    /// Mémoire vive suffisante (4 Go).
    pub ram_ok: bool,
    /// Espace libre sur le disque système (octets).
    pub free_bytes: u64,
    /// Toutes les exigences vérifiables sont remplies : mise à niveau officielle automatique.
    pub compatible: bool,
    /// Image ISO de Windows 11 trouvée dans le dossier Téléchargements.
    pub iso: Option<String>,
}

fn tpm_version() -> Option<String> {
    let out = command::powershell(
        "(Get-CimInstance -Namespace root/cimv2/Security/MicrosoftTpm -ClassName Win32_Tpm -ErrorAction SilentlyContinue).SpecVersion",
    )
    .ok()?;
    // « 2.0, 0, 1.59 » : la première valeur est la version de la spécification.
    let first = out.trim().split(',').next()?.trim().to_owned();
    (!first.is_empty()).then_some(first)
}

fn secure_boot_enabled() -> bool {
    matches!(
        registry::read(
            Hive::Hklm,
            r"SYSTEM\CurrentControlSet\Control\SecureBoot\State",
            "UEFISecureBootEnabled"
        ),
        Ok(Some(RegData::Dword(1)))
    )
}

fn system_drive_free() -> u64 {
    let drive = std::env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".into())
        .to_uppercase();
    let Ok(list) = sysinfo::Disks::new_with_refreshed_list() else {
        return 0;
    };
    list.iter()
        .find(|d| d.mount_point().display().to_string().to_uppercase().starts_with(&drive))
        .map(|d| d.available_space())
        .unwrap_or(0)
}

fn total_ram() -> u64 {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};
    System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()))
        .map(|mut sys| {
            sys.refresh_memory();
            sys.total_memory()
        })
        .unwrap_or(0)
}

/// Image ISO de Windows 11 la plus récente d'un dossier (`Win11*.iso`).
pub fn find_iso(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.starts_with("win11") && name.ends_with(".iso")
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

fn downloads_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Downloads")
}

/// Lit la compatibilité de ce PC (lecture seule).
pub fn status() -> AetherResult<Win11Status> {
    let build = windows_build()?;
    let is_windows10 = build < WIN11_FIRST_BUILD;
    if !is_windows10 {
        return Ok(Win11Status {
            is_windows10,
            build,
            tpm: None,
            secure_boot: false,
            ram_ok: true,
            free_bytes: 0,
            compatible: false,
            iso: None,
        });
    }
    let tpm = tpm_version();
    let secure_boot = secure_boot_enabled();
    let ram_ok = total_ram() >= MIN_RAM_BYTES;
    let tpm2 = tpm.as_deref().is_some_and(|v| v.starts_with('2'));
    Ok(Win11Status {
        is_windows10,
        build,
        tpm,
        secure_boot,
        ram_ok,
        free_bytes: system_drive_free(),
        compatible: tpm2 && secure_boot && ram_ok,
        iso: find_iso(&downloads_dir()).map(|p| p.display().to_string()),
    })
}

fn ensure_can_upgrade(status: &Win11Status) -> AetherResult<()> {
    if !status.is_windows10 {
        return Err(AetherError::Forbidden("ce PC est déjà sous Windows 11".into()));
    }
    if status.free_bytes < MIN_FREE_BYTES {
        return Err(AetherError::Forbidden(
            "espace libre insuffisant sur le disque système : libérez au moins 25 Go (section Stockage)".into(),
        ));
    }
    Ok(())
}

/// Vérifie que l'exécutable est signé par Microsoft (signature Authenticode valide).
fn verify_microsoft_signature(path: &Path) -> AetherResult<()> {
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath {}; if ($s.Status -eq 'Valid' -and $s.SignerCertificate.Subject -match 'O=Microsoft Corporation') {{ 'OK' }} else {{ $s.Status }}",
        command::ps_quote(&path.display().to_string())
    );
    let out = command::powershell(&script)?;
    if out.trim() == "OK" {
        return Ok(());
    }
    Err(AetherError::Forbidden(format!(
        "signature Microsoft invalide pour {} ({})",
        path.display(),
        out.trim()
    )))
}

/// PC compatible : télécharge l'Assistant d'installation officiel et lance la mise à niveau.
pub async fn upgrade_with_assistant(state: &AppState) -> AetherResult<()> {
    let st = tokio::task::spawn_blocking(status).await??;
    ensure_can_upgrade(&st)?;
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    state.logger.info(
        "win11",
        "Téléchargement de l'Assistant d'installation de Windows 11 (Microsoft)…",
    );
    let network = |e: reqwest::Error| AetherError::Network(e.to_string());
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(network)?;
    let bytes = client
        .get(ASSISTANT_URL)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(network)?
        .bytes()
        .await
        .map_err(network)?;
    let exe = std::env::temp_dir().join("Windows11InstallationAssistant.exe");
    std::fs::write(&exe, &bytes)?;
    if let Err(e) = verify_microsoft_signature(&exe) {
        let _ = std::fs::remove_file(&exe);
        return Err(e);
    }
    // Fenêtre de progression de l'Assistant visible ; contrat accepté, mise à niveau automatique.
    let logs = std::env::temp_dir().display().to_string();
    std::process::Command::new(&exe)
        .args(["/SkipEULA", "/auto", "upgrade", "/copylogs", &logs])
        .spawn()?;
    state.logger.success(
        "win11",
        "[Lancé] Assistant d'installation de Windows 11 : mise à niveau en cours",
    );
    Ok(())
}

/// PC non compatible : installe Windows 11 depuis l'image ISO officielle trouvée dans Téléchargements.
pub fn upgrade_from_iso(state: &AppState) -> AetherResult<()> {
    let st = status()?;
    ensure_can_upgrade(&st)?;
    let iso = find_iso(&downloads_dir()).ok_or_else(|| {
        AetherError::NotFound("image ISO de Windows 11 (fichier « Win11….iso » dans le dossier Téléchargements)".into())
    })?;
    // Clé documentée par Microsoft pour les mises à niveau sur un PC non pris en charge.
    registry::write(
        Hive::Hklm,
        MOSETUP_KEY,
        "AllowUpgradesWithUnsupportedTPMOrCPU",
        &RegData::Dword(1),
    )?;
    state
        .logger
        .info("win11", format!("Montage de l'image {}", iso.display()));
    let letter = command::powershell(&format!(
        "$i = Mount-DiskImage -ImagePath {} -PassThru; ($i | Get-Volume).DriveLetter",
        command::ps_quote(&iso.display().to_string())
    ))?;
    let letter = letter
        .trim()
        .chars()
        .next()
        .filter(char::is_ascii_alphabetic)
        .ok_or_else(|| AetherError::NotFound("lettre de lecteur de l'image ISO montée".into()))?;
    let setup = PathBuf::from(format!("{letter}:\\setup.exe"));
    if !setup.exists() {
        return Err(AetherError::NotFound(format!(
            "{} (image ISO incomplète ?)",
            setup.display()
        )));
    }
    verify_microsoft_signature(&setup)?;
    // `/product server` : programme d'installation sans contrôle de compatibilité matérielle.
    std::process::Command::new(&setup)
        .args([
            "/product",
            "server",
            "/auto",
            "upgrade",
            "/eula",
            "accept",
            "/compat",
            "ignorewarning",
            "/dynamicupdate",
            "disable",
        ])
        .spawn()?;
    state
        .logger
        .success("win11", "[Lancé] Installation de Windows 11 depuis l'image officielle");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_newest_windows11_iso_only() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("ubuntu.iso"), b"x")?;
        std::fs::write(dir.path().join("Win10_22H2.iso"), b"x")?;
        assert!(find_iso(dir.path()).is_none());
        std::fs::write(dir.path().join("Win11_25H2_French_x64.iso"), b"x")?;
        let found = find_iso(dir.path()).and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        assert_eq!(found.as_deref(), Some("Win11_25H2_French_x64.iso"));
        Ok(())
    }

    #[test]
    fn reads_status_without_changing_anything() -> AetherResult<()> {
        let st = status()?;
        assert!(st.build > 10_000);
        Ok(())
    }
}
