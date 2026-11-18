//! Conversion du rapport HTML en PDF avec Microsoft Edge en mode sans interface
//! (`--headless --print-to-pdf`), entièrement en local.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::command;

fn edge_exe() -> Option<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .map(|base| PathBuf::from(base).join(r"Microsoft\Edge\Application\msedge.exe"))
        .find(|p| p.is_file())
}

/// Convertit `html` en `pdf` ; `None` si Edge est absent ou si la conversion échoue.
pub fn html_to_pdf(html: &Path, pdf: &Path) -> Option<PathBuf> {
    let edge = edge_exe()?;
    let url = format!("file:///{}", html.display().to_string().replace('\\', "/"));
    let profile = std::env::temp_dir().join("aether-pdf-profile");
    let out = format!("--print-to-pdf={}", pdf.display());
    let user_data = format!("--user-data-dir={}", profile.display());
    let args = [
        "--headless",
        "--disable-gpu",
        "--no-first-run",
        "--no-pdf-header-footer",
        user_data.as_str(),
        out.as_str(),
        url.as_str(),
    ];
    let edge = edge.display().to_string();
    let _ = command::run_with_timeout(&edge, &args, Duration::from_secs(60));
    // Le lanceur d'Edge rend la main avant la fin de l'écriture : attente du fichier complet.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last = 0;
    while std::time::Instant::now() < deadline {
        let size = std::fs::metadata(pdf).map(|m| m.len()).unwrap_or(0);
        if size > 0 && size == last {
            return Some(pdf.to_path_buf());
        }
        last = size;
        std::thread::sleep(Duration::from_millis(400));
    }
    None
}
