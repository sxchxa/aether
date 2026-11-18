//! Script de build : embarque le manifeste Windows d'Aether (élévation UAC
//! `requireAdministrator` + Common-Controls v6) dans **tous** les exécutables liés
//! (application et binaires de test), via l'éditeur de liens MSVC.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let manifest = manifest_dir.join("aether.exe.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    // Ressources Windows (icône de l'exécutable : barre des tâches, Gestionnaire des tâches,
    // Explorateur) : recompilées dès que l'icône ou la configuration change.
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("icons").join("icon.ico").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("tauri.conf.json").display()
    );

    // Le manifeste est fourni par l'éditeur de liens : on désactive celui de tauri-build
    // pour éviter une ressource RT_MANIFEST en double.
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))?;

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        // Le niveau d'exécution vient du manifeste (requireAdministrator), pas du linker.
        println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
    }
    Ok(())
}
