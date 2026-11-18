//! Profils guidés, Optimisation Express (🟢 uniquement) et export/import de profil `.json`.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

use crate::core::logger::now_ms;
use crate::core::state::AppState;
use crate::core::tweak::{optimize_many, Outcome, Risk, Tweak, TweakReport};
use crate::error::{AetherError, AetherResult};
use crate::modules::catalog;
use crate::modules::storage::{self, cleaner, rules, CleanSummary};

/// Profil guidé proposé à l'utilisateur.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GuidedProfile {
    /// Usage Quotidien (Sûr).
    Daily,
    /// Joueur Compétitif (Ping & FPS).
    Gamer,
    /// PC Portable (Économie de batterie).
    Laptop,
    /// Silencieux : moins de tâches en arrière-plan, ventilateurs plus calmes.
    Silent,
    /// Compétitif : latence minimale (niveau Expérimenté).
    Competitive,
    /// Streaming : jeu et diffusion en même temps.
    Streaming,
    /// Bureautique : un Windows plus calme et plus clair pour travailler.
    Office,
    /// Création : montage, 3D, rendu (puissance soutenue).
    Creator,
    /// Intervention chez un client : réglages sûrs uniquement, aucun service touché, rien qui
    /// change les habitudes (apparence, menus) ni le réseau ou l'alimentation.
    Client,
}

/// Profil « Client » : confidentialité, réactivité de l'interface et notifications, uniquement.
/// Exclus volontairement : services Windows (impression, recherche, mises à jour…), carte réseau,
/// partages de fichiers, alimentation, apparence, Edge, tâches planifiées.
pub const CLIENT_TWEAKS: &[&str] = &[
    "gaming.xboxDvr",
    "net.deliveryOptimization",
    "privacy.advertisingId",
    "privacy.tips",
    "privacy.bingSearch",
    "privacy.startRecommendations",
    "privacy.feedback",
    "privacy.tailored",
    "privacy.telemetry",
    "privacy.recall",
    "ui.menuDelay",
    "ui.endTask",
    "ui.startupDelay",
    "wu.restartNotify",
];

/// Options recommandées par profil.
pub fn profile_tweaks(profile: GuidedProfile) -> &'static [&'static str] {
    match profile {
        GuidedProfile::Daily => &[
            "svc.diagtrack",
            "svc.fax",
            "gaming.xboxDvr",
            "nic.wakeMagic",
            "nic.wakePattern",
            "net.deliveryOptimization",
            "privacy.advertisingId",
            "privacy.tips",
            "privacy.recall",
            "privacy.bingSearch",
            "privacy.startRecommendations",
            "ui.menuDelay",
            "ui.endTask",
        ],
        GuidedProfile::Gamer => &[
            "svc.diagtrack",
            "svc.fax",
            "gaming.xboxDvr",
            "gaming.mouse",
            "gaming.gameMode",
            "gaming.windowedOpt",
            "gaming.stickyKeys",
            "privacy.tips",
            "power.ultimate",
            "power.throttling",
            "power.usbSuspend",
            "power.pcieAspm",
            "power.cpuMin",
            "tcp.throttling",
            "nic.wakeMagic",
            "nic.wakePattern",
            "nic.eee",
            "nic.interruptModeration",
            "net.deliveryOptimization",
            "privacy.advertisingId",
            "ui.menuDelay",
            "gaming.mmcss",
            "mem.svchostSplit",
            "gaming.gameBarButton",
        ],
        GuidedProfile::Client => CLIENT_TWEAKS,
        GuidedProfile::Silent => &[
            "svc.diagtrack",
            "net.deliveryOptimization",
            "privacy.tips",
            "privacy.widgets",
            "privacy.backgroundApps",
            "privacy.feedback",
            "gaming.xboxDvr",
            "ui.startupDelay",
        ],
        GuidedProfile::Competitive => &[
            "svc.diagtrack",
            "gaming.xboxDvr",
            "gaming.mouse",
            "gaming.gameMode",
            "gaming.windowedOpt",
            "gaming.hags",
            "gaming.stickyKeys",
            "power.ultimate",
            "power.throttling",
            "power.usbSuspend",
            "power.pcieAspm",
            "power.cpuMin",
            "tcp.throttling",
            "nic.powerSaving",
            "nic.eee",
            "nic.interruptModeration",
            "net.deliveryOptimization",
            "privacy.tips",
            "ui.menuDelay",
            "gaming.mmcss",
            "mem.svchostSplit",
            "gaming.gameBarButton",
        ],
        GuidedProfile::Streaming => &[
            "svc.diagtrack",
            "gaming.gameMode",
            "gaming.hags",
            "gaming.windowedOpt",
            "gaming.stickyKeys",
            "power.ultimate",
            "power.throttling",
            "tcp.throttling",
            "net.deliveryOptimization",
            "privacy.tips",
        ],
        GuidedProfile::Office => &[
            "svc.diagtrack",
            "net.deliveryOptimization",
            "privacy.advertisingId",
            "privacy.tips",
            "privacy.bingSearch",
            "privacy.startRecommendations",
            "ui.fileExtensions",
            "ui.menuDelay",
            "ui.startupDelay",
            "wu.restartNotify",
            "wu.activeHours",
        ],
        GuidedProfile::Creator => &[
            "svc.diagtrack",
            "power.ultimate",
            "power.throttling",
            "power.diskIdle",
            "gaming.hags",
            "ui.fileExtensions",
            "net.deliveryOptimization",
            "wu.restartNotify",
            "wu.activeHours",
        ],
        GuidedProfile::Laptop => &[
            "svc.diagtrack",
            "svc.fax",
            "gaming.xboxDvr",
            "nic.wakeMagic",
            "nic.wakePattern",
            "net.deliveryOptimization",
            "privacy.advertisingId",
            "privacy.tips",
            "privacy.widgets",
            "ui.transparency",
            "privacy.bingSearch",
            "ui.menuDelay",
        ],
    }
}

/// Options incluses dans l'Optimisation Express (risque 🟢 uniquement).
pub const EXPRESS_TWEAKS: &[&str] = &[
    "svc.diagtrack",
    "svc.fax",
    "gaming.xboxDvr",
    "nic.wakeMagic",
    "nic.wakePattern",
    "net.deliveryOptimization",
    "privacy.advertisingId",
    "privacy.tips",
    "gaming.gameMode",
    "privacy.bingSearch",
    "ui.menuDelay",
];

/// Instantané « Avant / Après ».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// RAM utilisée (octets).
    pub ram_used: u64,
    /// Fichiers inutiles récupérables (octets).
    pub junk_bytes: u64,
    /// Options Express déjà optimisées.
    pub optimized: u32,
    /// Options Express évaluées.
    pub total: u32,
}

/// Rapport d'Optimisation Express.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpressReport {
    /// État avant.
    pub before: Snapshot,
    /// État après.
    pub after: Snapshot,
    /// Détail des options.
    pub tweaks: Vec<TweakReport>,
    /// Nettoyage effectué.
    pub cleaning: CleanSummary,
    /// Mode Aperçu.
    pub preview: bool,
}

fn safe_rule_ids() -> Vec<String> {
    rules::all_rules()
        .into_iter()
        .filter(|r| r.risk == Risk::Safe && r.default_selected && r.processes.is_empty())
        .map(|r| r.id)
        .collect()
}

fn express_tweaks() -> Vec<&'static dyn Tweak> {
    catalog()
        .iter()
        .map(|b| b.as_ref())
        .filter(|t| EXPRESS_TWEAKS.contains(&t.id()) && t.risk() == Risk::Safe)
        .collect()
}

fn ram_used() -> u64 {
    System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()))
        .map(|mut sys| {
            sys.refresh_memory();
            sys.used_memory()
        })
        .unwrap_or(0)
}

fn snapshot(state: &AppState, ids: &[String]) -> Snapshot {
    let ram_used = ram_used();
    let junk_bytes = rules::all_rules()
        .iter()
        .filter(|r| ids.contains(&r.id))
        .map(|r| cleaner::scan_rule(r).bytes)
        .sum();
    // Seules les options présentes sur ce PC comptent (ex. pas de service Fax installé).
    let probes: Vec<_> = express_tweaks()
        .iter()
        .filter_map(|t| t.check_status(state).ok())
        .filter(|p| p.available)
        .collect();
    Snapshot {
        ram_used,
        junk_bytes,
        optimized: probes.iter().filter(|p| p.applied).count() as u32,
        total: probes.len() as u32,
    }
}

/// Optimisation Express : options 🟢 idempotentes + nettoyage des résidus sûrs.
///
/// Impact Windows : n'applique que les options sûres non encore optimisées ; envoie les
/// résidus temporaires à la Corbeille. Respecte le Mode Aperçu.
pub fn express(state: &AppState) -> ExpressReport {
    state.logger.info("express", "Optimisation Express : démarrage");
    let ids = safe_rule_ids();
    let before = snapshot(state, &ids);
    let tweaks = optimize_many(&express_tweaks(), state);
    let cleaning = storage::clean(state, &ids);
    // Bilan « Après » déduit des résultats : évite un nouveau parcours disque complet.
    let applied = tweaks.iter().filter(|r| r.outcome == Some(Outcome::Applied)).count() as u32;
    let after = Snapshot {
        ram_used: ram_used(),
        junk_bytes: if cleaning.preview {
            before.junk_bytes
        } else {
            before.junk_bytes.saturating_sub(cleaning.freed)
        },
        optimized: (before.optimized + applied).min(before.total),
        total: before.total,
    };
    state.logger.info("express", "Optimisation Express : terminée");
    ExpressReport {
        before,
        after,
        tweaks,
        cleaning,
        preview: state.dry_run(),
    }
}

/// Applique un profil guidé (idempotent).
///
/// Impact Windows : applique uniquement les options recommandées qui ne sont pas déjà optimisées.
pub fn apply_profile(state: &AppState, profile: GuidedProfile) -> Vec<TweakReport> {
    let wanted = profile_tweaks(profile);
    let tweaks: Vec<&dyn Tweak> = catalog()
        .iter()
        .map(|b| b.as_ref())
        .filter(|t| wanted.contains(&t.id()))
        // PC portable : pas de processeur à 100 % en continu ni de PCIe sans économie d'énergie.
        .filter(|t| !crate::modules::performance::power::skip_on_this_pc(t.id()))
        // Profil client : double garde-fou, uniquement des options sûres et aucun service.
        .filter(|t| profile != GuidedProfile::Client || (t.risk() == Risk::Safe && !t.id().starts_with("svc.")))
        .collect();
    state
        .logger
        .info("profile", format!("Application du profil {profile:?}"));
    let mut reports = optimize_many(&tweaks, state);
    if profile == GuidedProfile::Client {
        // Nettoyage des fichiers temporaires sûrs (navigateurs fermés uniquement : rien n'est forcé).
        let cleaning = storage::clean(state, &safe_rule_ids());
        reports.push(TweakReport {
            id: "clean.safe".into(),
            outcome: Some(if cleaning.preview {
                Outcome::Preview
            } else {
                Outcome::Applied
            }),
            error: None,
            detail: Some(format!("{} fichier(s), {} octet(s)", cleaning.removed, cleaning.freed)),
        });
    }
    reports
}

/// Fichier de profil exporté.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFile {
    /// Signature de format (`aether-profile`).
    pub format: String,
    /// Version du format.
    pub version: u32,
    /// Date d'export (ms Unix).
    pub exported_at: u64,
    /// Options optimisées.
    pub optimized: Vec<String>,
    /// Version d'Aether ayant exporté le fichier (absente des anciens exports).
    #[serde(default)]
    pub app_version: String,
    /// Nom du PC d'origine (information, pour s'y retrouver entre plusieurs machines).
    #[serde(default)]
    pub machine: String,
}

/// Dossier des exports (Documents\Aether, ou le dossier de données à défaut).
pub fn export_dir(state: &AppState) -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Documents"))
        .filter(|p| p.is_dir())
        .map(|p| p.join("Aether"))
        .unwrap_or_else(|| state.data_dir.join("profiles"))
}

/// Exporte la liste des options actuellement optimisées dans un fichier `.json`.
///
/// Impact Windows : écrit `Documents\Aether\aether-profile-<horodatage>.json`.
pub fn export(state: &AppState) -> AetherResult<String> {
    // Lecture parallèle de toutes les options (bien plus rapide qu'une lecture une à une).
    let optimized = crate::modules::all_status(state, None)
        .into_iter()
        .filter(|t| t.probe.available && t.probe.applied)
        .map(|t| t.id.to_owned())
        .collect();
    let file = ProfileFile {
        format: "aether-profile".into(),
        version: 1,
        exported_at: now_ms(),
        optimized,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        machine: std::env::var("COMPUTERNAME").unwrap_or_default(),
    };
    let dir = export_dir(state);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("aether-profile-{}.json", file.exported_at));
    fs::write(&path, serde_json::to_string_pretty(&file)?)?;
    state.logger.success("profile", format!("[Exporté] {}", path.display()));
    Ok(path.display().to_string())
}

/// Importe un profil `.json` et applique ses options (idempotent, options inconnues ignorées).
///
/// Impact Windows : applique les options listées non encore optimisées.
pub fn import(state: &AppState, json: &str) -> AetherResult<Vec<TweakReport>> {
    let file: ProfileFile = serde_json::from_str(json)?;
    if file.format != "aether-profile" || file.version != 1 {
        return Err(AetherError::InvalidInput("fichier de profil Aether non reconnu".into()));
    }
    let tweaks: Vec<&dyn Tweak> = catalog()
        .iter()
        .map(|b| b.as_ref())
        .filter(|t| file.optimized.iter().any(|id| id == t.id()))
        .collect();
    state
        .logger
        .info("profile", format!("Import de profil : {} option(s)", tweaks.len()));
    Ok(optimize_many(&tweaks, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn express_contains_only_safe_known_tweaks() {
        for id in EXPRESS_TWEAKS {
            let t = catalog().iter().find(|t| t.id() == *id);
            assert!(t.is_some_and(|t| t.risk() == Risk::Safe), "{id}");
        }
    }

    #[test]
    fn profiles_reference_existing_tweaks() {
        for p in [GuidedProfile::Daily, GuidedProfile::Gamer, GuidedProfile::Laptop] {
            for id in profile_tweaks(p) {
                assert!(catalog().iter().any(|t| t.id() == *id), "{id}");
            }
        }
        assert!(!profile_tweaks(GuidedProfile::Laptop).contains(&"power.ultimate"));
    }

    #[test]
    fn rejects_foreign_profile_files() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        assert!(import(
            &state,
            r#"{"format":"autre","version":1,"exportedAt":0,"optimized":[]}"#
        )
        .is_err());
        assert!(import(&state, "pas du json").is_err());
        Ok(())
    }

    #[test]
    fn client_profile_is_safe_only_and_never_touches_services() {
        for id in profile_tweaks(GuidedProfile::Client) {
            assert!(!id.starts_with("svc."), "{id} : service interdit dans le profil client");
            let t = catalog()
                .iter()
                .find(|t| t.id() == *id)
                .unwrap_or_else(|| panic!("{id} absent du catalogue"));
            assert_eq!(t.risk(), Risk::Safe, "{id} doit être sûr");
            assert!(!crate::modules::performance::power::skip_on_this_pc(id));
        }
    }

    #[test]
    fn every_profile_option_exists() {
        use GuidedProfile::*;
        for p in [
            Daily,
            Gamer,
            Laptop,
            Silent,
            Competitive,
            Streaming,
            Office,
            Creator,
            Client,
        ] {
            for id in profile_tweaks(p) {
                assert!(
                    catalog().iter().any(|t| t.id() == *id),
                    "{p:?} : {id} absent du catalogue"
                );
            }
        }
    }
}
