//! Commande TRIM pour SSD (`Optimize-Volume -ReTrim`).

use crate::core::command;
use crate::core::state::AppState;
use crate::error::{AetherError, AetherResult};
use crate::modules::overview::system::disks;

/// Lance un TRIM manuel sur le lecteur `letter` (uniquement s'il s'agit d'un SSD).
///
/// Impact Windows : `Optimize-Volume -DriveLetter <X> -ReTrim -Verbose` — informe le SSD
/// des blocs libérés ; sans risque pour les données.
pub fn retrim(state: &AppState, letter: char) -> AetherResult<String> {
    if !letter.is_ascii_alphabetic() {
        return Err(AetherError::InvalidInput(format!("lecteur invalide : {letter}")));
    }
    let letter = letter.to_ascii_uppercase();
    let is_ssd = disks()
        .iter()
        .any(|d| d.mount.to_ascii_uppercase().starts_with(letter) && d.kind == "ssd");
    if !is_ssd {
        return Err(AetherError::Forbidden(format!(
            "le lecteur {letter}: n'est pas détecté comme SSD : TRIM ignoré"
        )));
    }
    if state.dry_run() {
        state.logger.log(
            crate::core::logger::LogLevel::Preview,
            "trim",
            format!("[APERÇU] Optimize-Volume -DriveLetter {letter} -ReTrim"),
        );
        return Ok("Mode Aperçu : aucune commande exécutée.".into());
    }
    state
        .logger
        .info("trim", format!("TRIM du lecteur {letter}: en cours…"));
    let out = command::powershell_with_timeout(
        &format!("Optimize-Volume -DriveLetter {letter} -ReTrim -Verbose 4>&1 | ForEach-Object {{ $_.ToString() }}"),
        std::time::Duration::from_secs(600),
    )?;
    state
        .logger
        .success("trim", format!("TRIM du lecteur {letter}: terminé"));
    Ok(out.trim().to_owned())
}
