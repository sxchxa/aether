//! Moteur d'optimisation idempotent.
//!
//! Chaque option est un [`Tweak`] capable de lire son état réel (`check_status`), de s'appliquer
//! et de revenir à l'état d'origine. Le moteur garantit qu'une option déjà dans son état cible
//! n'est **jamais** modifiée (aucune bascule aveugle) et journalise `[OK - Déjà optimisé]`.

use serde::Serialize;

use crate::core::history::UndoAction;
use crate::core::logger::LogLevel;
use crate::core::state::AppState;
use crate::error::AetherResult;

/// Niveau de risque affiché dans l'interface.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    /// 🟢 Sûr : aucun effet de bord attendu.
    Safe,
    /// 🟠 Modéré : peut désactiver une fonction utilisée par certains.
    Moderate,
    /// 🔴 Avancé : réservé aux utilisateurs expérimentés (mode Expert).
    Advanced,
}

/// Section de l'interface à laquelle appartient l'option.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Section {
    /// Performance & Jeux.
    Performance,
    /// Alimentation & Énergie.
    Power,
    /// Réseau & Latence.
    Network,
    /// Système & Réparation.
    System,
}

/// État réel lu sur le système.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    /// L'option est déjà dans son état optimisé.
    pub applied: bool,
    /// L'option est applicable sur ce PC (service installé, propriété NIC présente…).
    pub available: bool,
    /// Valeur actuelle lisible.
    pub current: String,
    /// Valeur cible lisible.
    pub target: String,
}

impl Probe {
    /// Option absente de ce PC.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            applied: false,
            available: false,
            current: reason.into(),
            target: String::new(),
        }
    }
}

/// Contrat commun à toutes les options activables d'Aether.
pub trait Tweak: Send + Sync {
    /// Identifiant stable (ex. `svc.spooler`), utilisé par l'interface et les profils.
    fn id(&self) -> &'static str;
    /// Section de rattachement.
    fn section(&self) -> Section;
    /// Groupe (carte) de rattachement dans la section.
    fn group(&self) -> &'static str;
    /// Niveau de risque.
    fn risk(&self) -> Risk;
    /// Cible technique exacte (clé de Registre / commande) affichée dans l'inspecteur.
    fn technical(&self) -> String;
    /// Indique si un redémarrage est conseillé après application.
    fn needs_reboot(&self) -> bool {
        false
    }
    /// Lit l'état réel sur le système (lecture seule).
    fn check_status(&self, state: &AppState) -> AetherResult<Probe>;
    /// Applique l'état optimisé. N'est appelé que si `check_status().applied == false`.
    /// Renvoie les actions permettant d'annuler.
    fn apply(&self, state: &AppState) -> AetherResult<Vec<UndoAction>>;
    /// Rétablit l'état d'origine (sauvegarde) ou, à défaut, la valeur par défaut de Windows.
    fn restore(&self, state: &AppState) -> AetherResult<Vec<UndoAction>>;
}

/// État complet d'une option, envoyé à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakStatus {
    /// Identifiant.
    pub id: &'static str,
    /// Section.
    pub section: Section,
    /// Groupe.
    pub group: &'static str,
    /// Risque.
    pub risk: Risk,
    /// Cible technique.
    pub technical: String,
    /// Redémarrage conseillé.
    pub needs_reboot: bool,
    /// État lu.
    pub probe: Probe,
    /// Erreur de lecture éventuelle.
    pub error: Option<String>,
}

/// Résultat d'une opération du moteur.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Modification appliquée.
    Applied,
    /// Déjà dans l'état demandé : rien n'a été fait.
    AlreadyDone,
    /// Mode Aperçu : simulation uniquement.
    Preview,
    /// Option non disponible sur ce PC.
    Unavailable,
}

/// Rapport d'opération par option.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakReport {
    /// Identifiant de l'option.
    pub id: String,
    /// Résultat (absent en cas d'erreur).
    pub outcome: Option<Outcome>,
    /// Message d'erreur éventuel.
    pub error: Option<String>,
    /// Précision lisible (ex. raison d'indisponibilité sur ce PC).
    pub detail: Option<String>,
}

/// Lit l'état complet d'une option (ne modifie rien).
pub fn status_of(tweak: &dyn Tweak, state: &AppState) -> TweakStatus {
    let (probe, error) = match tweak.check_status(state) {
        Ok(p) => (p, None),
        Err(e) => (Probe::unavailable(""), Some(e.to_string())),
    };
    TweakStatus {
        id: tweak.id(),
        section: tweak.section(),
        group: tweak.group(),
        risk: tweak.risk(),
        technical: tweak.technical(),
        needs_reboot: tweak.needs_reboot(),
        probe,
        error,
    }
}

/// Optimise une option de façon idempotente.
///
/// Impact Windows : modifie le système uniquement si l'option n'est pas déjà optimisée
/// et si le Mode Aperçu est désactivé.
pub fn optimize(tweak: &dyn Tweak, state: &AppState) -> AetherResult<Outcome> {
    let id = tweak.id();
    let probe = tweak.check_status(state)?;
    if !probe.available {
        state
            .logger
            .log(LogLevel::Info, id, format!("[Indisponible] {id} : {}", probe.current));
        return Ok(Outcome::Unavailable);
    }
    if probe.applied {
        state.logger.log(
            LogLevel::Skip,
            id,
            format!("[OK - Déjà optimisé] {id} ({})", probe.current),
        );
        return Ok(Outcome::AlreadyDone);
    }
    if state.dry_run() {
        state.logger.log(
            LogLevel::Preview,
            id,
            format!("[APERÇU] {id} : {} → {}", probe.current, probe.target),
        );
        return Ok(Outcome::Preview);
    }
    let actions = tweak.apply(state)?;
    state.record(id, &format!("Optimisation {id}"), actions)?;
    state
        .logger
        .success(id, format!("[Appliqué] {id} : {} → {}", probe.current, probe.target));
    Ok(Outcome::Applied)
}

/// Rétablit l'état d'origine d'une option, uniquement si elle est actuellement optimisée.
///
/// Impact Windows : restaure la sauvegarde d'origine ou la valeur par défaut de Windows.
pub fn restore(tweak: &dyn Tweak, state: &AppState) -> AetherResult<Outcome> {
    let id = tweak.id();
    let probe = tweak.check_status(state)?;
    if !probe.available {
        return Ok(Outcome::Unavailable);
    }
    if !probe.applied {
        state
            .logger
            .log(LogLevel::Skip, id, format!("[OK - Déjà à l'état d'origine] {id}"));
        return Ok(Outcome::AlreadyDone);
    }
    if state.dry_run() {
        state
            .logger
            .log(LogLevel::Preview, id, format!("[APERÇU] Restauration de {id}"));
        return Ok(Outcome::Preview);
    }
    let actions = tweak.restore(state)?;
    state.record(id, &format!("Restauration {id}"), actions)?;
    state.logger.success(id, format!("[Restauré] {id}"));
    Ok(Outcome::Applied)
}

/// Voie d'exécution d'une option : les options d'une même voie s'enchaînent dans l'ordre
/// (carte réseau redémarrée, mode d'alimentation actif…), les voies avancent en parallèle.
fn lane(id: &str) -> String {
    match id.split('.').next().unwrap_or_default() {
        "nic" | "tcp" | "net" | "dns" => "network".into(),
        "power" => "power".into(),
        // Chaque service attend son propre démarrage ou arrêt : aucun lien entre eux.
        "svc" => id.to_owned(),
        _ => "misc".into(),
    }
}

/// Exécute `f` sur chaque option, voies en parallèle, résultats renvoyés dans l'ordre d'entrée.
pub fn run_in_lanes<F>(tweaks: &[&dyn Tweak], f: F) -> Vec<TweakReport>
where
    F: Fn(&dyn Tweak) -> TweakReport + Sync,
{
    let mut lanes: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, t) in tweaks.iter().enumerate() {
        let key = lane(t.id());
        match lanes.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(i),
            None => lanes.push((key, vec![i])),
        }
    }
    let mut out: Vec<Option<TweakReport>> = (0..tweaks.len()).map(|_| None).collect();
    let f = &f;
    std::thread::scope(|scope| {
        let handles: Vec<_> = lanes
            .iter()
            .map(|(_, idx)| scope.spawn(move || idx.iter().map(|&i| (i, f(tweaks[i]))).collect::<Vec<_>>()))
            .collect();
        for h in handles {
            if let Ok(done) = h.join() {
                for (i, r) in done {
                    out[i] = Some(r);
                }
            }
        }
    });
    out.into_iter()
        .zip(tweaks)
        .map(|(r, t)| {
            r.unwrap_or_else(|| TweakReport {
                id: t.id().to_owned(),
                outcome: None,
                error: Some("interrompu".into()),
                detail: None,
            })
        })
        .collect()
}

/// Optimise une liste d'options en continuant malgré les erreurs individuelles
/// (voies indépendantes traitées en parallèle).
pub fn optimize_many(tweaks: &[&dyn Tweak], state: &AppState) -> Vec<TweakReport> {
    run_in_lanes(tweaks, |t| match optimize(t, state) {
        Ok(o) => TweakReport {
            id: t.id().to_owned(),
            outcome: Some(o),
            error: None,
            // Raison lisible quand l'option n'existe pas sur ce PC.
            detail: (o == Outcome::Unavailable)
                .then(|| t.check_status(state).ok().map(|p| p.current))
                .flatten(),
        },
        Err(e) => {
            state.logger.error(t.id(), format!("[Échec] {} : {e}", t.id()));
            TweakReport {
                id: t.id().to_owned(),
                outcome: None,
                error: Some(e.to_string()),
                detail: None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct FakeTweak {
        applied: AtomicBool,
        apply_calls: AtomicUsize,
    }

    impl Tweak for FakeTweak {
        fn id(&self) -> &'static str {
            "test.fake"
        }
        fn section(&self) -> Section {
            Section::System
        }
        fn group(&self) -> &'static str {
            "test"
        }
        fn risk(&self) -> Risk {
            Risk::Safe
        }
        fn technical(&self) -> String {
            "aucune".into()
        }
        fn check_status(&self, _: &AppState) -> AetherResult<Probe> {
            Ok(Probe {
                applied: self.applied.load(Ordering::SeqCst),
                available: true,
                current: "x".into(),
                target: "y".into(),
            })
        }
        fn apply(&self, _: &AppState) -> AetherResult<Vec<UndoAction>> {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            self.applied.store(true, Ordering::SeqCst);
            Ok(vec![UndoAction::RefreshMouse])
        }
        fn restore(&self, _: &AppState) -> AetherResult<Vec<UndoAction>> {
            self.applied.store(false, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    #[test]
    fn engine_is_idempotent_and_respects_dry_run() -> AetherResult<()> {
        let dir = tempfile::tempdir()?;
        let state = AppState::new(dir.path());
        let t = FakeTweak {
            applied: AtomicBool::new(false),
            apply_calls: AtomicUsize::new(0),
        };
        // Mode Aperçu (interne, plus exposé) : rien n'est appliqué.
        state.set_dry_run(true)?;
        assert_eq!(optimize(&t, &state)?, Outcome::Preview);
        assert_eq!(t.apply_calls.load(Ordering::SeqCst), 0);

        state.set_dry_run(false)?;
        assert_eq!(optimize(&t, &state)?, Outcome::Applied);
        // Seconde optimisation : aucune bascule, aucune réapplication.
        assert_eq!(optimize(&t, &state)?, Outcome::AlreadyDone);
        assert_eq!(t.apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(state.history.lock()?.list().len(), 1);

        assert_eq!(restore(&t, &state)?, Outcome::Applied);
        assert_eq!(restore(&t, &state)?, Outcome::AlreadyDone);
        Ok(())
    }
}
