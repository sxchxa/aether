//! Exécution silencieuse des commandes système (aucune fenêtre console).
//!
//! Toutes les commandes (`sc.exe`, `powershell`, `ipconfig`, `powercfg`, `sfc`, `dism`,
//! `reg.exe`, `netsh`, `OneDriveSetup.exe`) passent par ce module, qui applique
//! systématiquement le drapeau `CREATE_NO_WINDOW` (`0x08000000`).

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::error::{AetherError, AetherResult};

/// Drapeau Win32 `CREATE_NO_WINDOW` : empêche l'apparition d'une console.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Résultat capturé d'une commande système.
#[derive(Debug, Clone, Default)]
pub struct CommandOutput {
    /// Code de sortie du processus (absent s'il a été tué).
    pub code: Option<i32>,
    /// Sortie standard décodée.
    pub stdout: String,
    /// Sortie d'erreur décodée.
    pub stderr: String,
}

impl CommandOutput {
    /// Indique si la commande s'est terminée avec le code 0.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Construit une `Command` silencieuse (`CREATE_NO_WINDOW`).
///
/// Impact Windows : aucun, se contente de préparer le processus enfant.
pub fn hidden(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Décode une sortie console Windows : UTF-16LE (cas de `sfc.exe`) ou UTF-8.
pub fn decode_console(bytes: &[u8]) -> String {
    let zeros = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
    let looks_utf16 = bytes.len() >= 4 && zeros * 3 > bytes.len() / 2;
    if looks_utf16 {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units).replace('\0', "")
    } else {
        match std::str::from_utf8(bytes) {
            Ok(s) => s.to_owned(),
            Err(_) => decode_oem(bytes),
        }
    }
}

/// Décode une sortie console en page de code OEM (ex. CP850 : `powercfg`, `sc`, `netsh`).
fn decode_oem(bytes: &[u8]) -> String {
    use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_OEMCP};
    let Ok(len) = i32::try_from(bytes.len()) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    // SAFETY : premier appel pour la taille, second dans un tampon exactement dimensionné.
    unsafe {
        let needed = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0);
        if needed <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; needed as usize];
        let written = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), len, wide.as_mut_ptr(), needed);
        String::from_utf16_lossy(&wide[..written.max(0) as usize])
    }
}

/// Délai maximal d'une commande système ponctuelle (hors SFC/DISM, diffusées en flux).
pub const COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Exécute un programme silencieusement et capture sa sortie, sans échouer sur un code non nul.
/// Le processus est arrêté s'il dépasse [`COMMAND_TIMEOUT`] : aucune action ne peut bloquer
/// l'interface indéfiniment.
///
/// Impact Windows : dépend du programme lancé.
pub fn run_raw(program: &str, args: &[&str]) -> AetherResult<CommandOutput> {
    run_with_timeout(program, args, COMMAND_TIMEOUT)
}

/// Variante de [`run_raw`] avec un délai explicite.
pub fn run_with_timeout(program: &str, args: &[&str], timeout: std::time::Duration) -> AetherResult<CommandOutput> {
    use std::time::Instant;
    let mut child = hidden(program).args(args).spawn()?;
    // Les flux sont lus dans des threads pour éviter tout interblocage sur un tampon plein.
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    match status {
        Some(status) => Ok(CommandOutput {
            code: status.code(),
            stdout: decode_console(&stdout),
            stderr: decode_console(&stderr),
        }),
        None => Err(AetherError::Command {
            command: format!("{program} {}", args.join(" ")).chars().take(200).collect(),
            code: None,
            stderr: format!("délai de {} s dépassé : opération interrompue", timeout.as_secs()),
        }),
    }
}

/// Exécute un programme silencieusement et renvoie une erreur si le code de sortie est non nul.
///
/// Impact Windows : dépend du programme lancé.
pub fn run(program: &str, args: &[&str]) -> AetherResult<CommandOutput> {
    let out = run_raw(program, args)?;
    if out.success() {
        Ok(out)
    } else {
        Err(AetherError::Command {
            command: format!("{program} {}", args.join(" ")),
            code: out.code,
            stderr: first_meaningful(&out.stderr, &out.stdout),
        })
    }
}

fn first_meaningful(stderr: &str, stdout: &str) -> String {
    let source = if stderr.trim().is_empty() { stdout } else { stderr };
    source.trim().chars().take(600).collect()
}

/// Préambule PowerShell : sortie UTF-8 et erreurs bloquantes.
const PS_PRELUDE: &str = "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;";

/// Exécute un script PowerShell silencieux (`-NoProfile -NonInteractive`) et renvoie stdout.
///
/// Impact Windows : dépend du script ; utilisé pour les cmdlets réseau, Appx et restauration.
pub fn powershell(script: &str) -> AetherResult<String> {
    powershell_with_timeout(script, COMMAND_TIMEOUT)
}

/// Variante de [`powershell`] pour les opérations longues (point de restauration, TRIM…).
pub fn powershell_with_timeout(script: &str, timeout: std::time::Duration) -> AetherResult<String> {
    let full = format!("{PS_PRELUDE}{script}");
    let args = [
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        full.as_str(),
    ];
    let out = run_with_timeout("powershell.exe", &args, timeout)?;
    if !out.success() {
        return Err(AetherError::Command {
            command: "powershell.exe".into(),
            code: out.code,
            stderr: first_meaningful(&out.stderr, &out.stdout),
        });
    }
    Ok(out.stdout)
}

/// Exécute un script PowerShell qui produit du JSON (`ConvertTo-Json`) et le désérialise.
///
/// Gère le cas PowerShell où un tableau d'un seul élément est émis comme un objet.
pub fn powershell_json<T: serde::de::DeserializeOwned>(script: &str) -> AetherResult<Vec<T>> {
    let raw = powershell(script)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        Ok(serde_json::from_str(trimmed)?)
    } else {
        Ok(vec![serde_json::from_str(trimmed)?])
    }
}

/// Échappe une chaîne pour l'insérer entre apostrophes dans un script PowerShell.
pub fn ps_quote(value: &str) -> String {
    // PowerShell considère aussi ‘ ’ ‚ ‛ comme des apostrophes : chacune est doublée.
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for c in value.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// Lance une commande longue et transmet chaque ligne produite au rappel `on_line`.
///
/// Les retours chariot (`\r`) des barres de progression de `sfc`/`DISM` sont traités comme
/// des fins de ligne. Impact Windows : dépend du programme lancé.
pub fn stream<F: FnMut(&str)>(program: &str, args: &[&str], on_line: F) -> AetherResult<Option<i32>> {
    stream_with_timeout(program, args, None, on_line)
}

/// Comme [`stream`], avec une durée maximale : passé ce délai, le processus et ses enfants sont
/// arrêtés (`taskkill /T`) et une erreur est renvoyée (aucune attente infinie).
pub fn stream_with_timeout<F: FnMut(&str)>(
    program: &str,
    args: &[&str],
    limit: Option<Duration>,
    mut on_line: F,
) -> AetherResult<Option<i32>> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let mut child = hidden(program).args(args).stderr(Stdio::null()).spawn()?;
    let finished = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    if let Some(limit) = limit {
        let (finished, timed_out, pid) = (Arc::clone(&finished), Arc::clone(&timed_out), child.id());
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while !finished.load(Ordering::SeqCst) {
                if start.elapsed() > limit {
                    timed_out.store(true, Ordering::SeqCst);
                    let pid = pid.to_string();
                    let _ = run_raw("taskkill.exe", &["/F", "/T", "/PID", &pid]);
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AetherError::NotFound("flux de sortie du processus".into()))?;
    // L'encodage (UTF-16LE pour sfc.exe, UTF-8 sinon) est déterminé sur le premier bloc lu.
    let mut utf16: Option<bool> = None;
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = stdout.read(&mut buf)?;
        if n == 0 {
            break;
        }
        pending.extend_from_slice(&buf[..n]);
        let is_utf16 = *utf16.get_or_insert_with(|| {
            let zeros = pending.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
            pending.len() >= 4 && zeros * 3 > pending.len() / 2
        });
        let (text, consumed) = decode_complete(&pending, is_utf16);
        if let Some(last_break) = text.rfind(['\n', '\r']) {
            let (complete, rest) = text.split_at(last_break + 1);
            emit_lines(complete, &mut on_line);
            let mut remaining = encode_back(rest, is_utf16);
            remaining.extend_from_slice(&pending[consumed..]);
            pending = remaining;
        }
    }
    let (tail, _) = decode_complete(&pending, utf16.unwrap_or(false));
    emit_lines(&tail, &mut on_line);
    let status = child.wait()?;
    finished.store(true, Ordering::SeqCst);
    if timed_out.load(Ordering::SeqCst) {
        return Err(AetherError::Command {
            command: program.to_owned(),
            code: None,
            stderr: "délai maximal dépassé : opération arrêtée".into(),
        });
    }
    Ok(status.code())
}

fn decode_complete(bytes: &[u8], utf16: bool) -> (String, usize) {
    if utf16 {
        let even = bytes.len() - bytes.len() % 2;
        let units: Vec<u16> = bytes[..even]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        (String::from_utf16_lossy(&units).replace('\0', ""), even)
    } else {
        (String::from_utf8_lossy(bytes).into_owned(), bytes.len())
    }
}

fn encode_back(text: &str, utf16: bool) -> Vec<u8> {
    if utf16 {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    } else {
        text.as_bytes().to_vec()
    }
}

fn emit_lines<F: FnMut(&str)>(text: &str, on_line: &mut F) {
    for line in text.split(['\n', '\r']) {
        let line = line.trim();
        if !line.is_empty() {
            on_line(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16_console_output() {
        let bytes: Vec<u8> = "Vérification 42 %".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_console(&bytes), "Vérification 42 %");
    }

    #[test]
    fn decodes_utf8_console_output() {
        assert_eq!(decode_console("Opération réussie".as_bytes()), "Opération réussie");
    }

    #[test]
    fn decodes_oem_console_output() {
        // `é` en CP850 = 0x82 (octet invalide en UTF-8) : décodé via la page OEM du système.
        let out = run("powercfg.exe", &["/list"]).map(|o| o.stdout).unwrap_or_default();
        assert!(!out.contains('\u{FFFD}'), "caractère de remplacement dans : {out}");
    }

    #[test]
    fn long_commands_are_interrupted() {
        let started = std::time::Instant::now();
        let res = run_with_timeout(
            "ping.exe",
            &["-n", "30", "127.0.0.1"],
            std::time::Duration::from_millis(800),
        );
        assert!(res.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn quotes_powershell_strings() {
        assert_eq!(ps_quote("Wi-Fi d'Ethernet"), "'Wi-Fi d''Ethernet'");
    }

    #[test]
    fn runs_hidden_command() {
        let out = run("cmd.exe", &["/C", "echo aether"]);
        assert!(out.map(|o| o.stdout.contains("aether")).unwrap_or(false));
    }

    #[test]
    fn streams_lines() {
        let mut lines = Vec::new();
        let code = stream("cmd.exe", &["/C", "echo un& echo deux"], |l| lines.push(l.to_owned()));
        assert_eq!(code.ok().flatten(), Some(0));
        assert_eq!(lines, vec!["un", "deux"]);
    }

    #[test]
    fn ps_quote_escapes_typographic_quotes() {
        assert_eq!(ps_quote("a'b"), "'a''b'");
        assert_eq!(ps_quote("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
        assert_eq!(
            ps_quote("x\u{2018};calc;\u{2018}"),
            "'x\u{2018}\u{2018};calc;\u{2018}\u{2018}'"
        );
    }

    #[test]
    fn stream_timeout_stops_a_hung_process() {
        let start = std::time::Instant::now();
        // ping de 30 s arrêté au bout d'1 s.
        let res = stream_with_timeout(
            "ping.exe",
            &["-n", "30", "127.0.0.1"],
            Some(Duration::from_secs(1)),
            |_| {},
        );
        assert!(res.is_err(), "le délai dépassé doit être signalé");
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "le processus aurait dû être arrêté"
        );
    }
}
