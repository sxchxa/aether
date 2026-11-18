//! Tests de bout en bout sur le système réel (VM de test) : chaque option est appliquée,
//! vérifiée, puis restaurée à son état d'origine. Ignorés par défaut ; lancer avec :
//!
//! ```text
//! cargo test --test e2e -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Variables : `AETHER_E2E_ONLY=id1,id2` (filtre), `AETHER_E2E_NIC=1` (inclut les options de
//! carte réseau, qui coupent brièvement la connexion).

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use aether_lib::core::state::AppState;
use aether_lib::core::tweak::{optimize, restore, Outcome};
use aether_lib::modules::catalog;

const STEP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug)]
enum Step {
    Ok(Duration, String),
    Err(Duration, String),
    Hang,
}

fn timed<F>(f: F) -> Step
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(STEP_TIMEOUT) {
        Ok(Ok(s)) => Step::Ok(started.elapsed(), s),
        Ok(Err(e)) => Step::Err(started.elapsed(), e),
        Err(_) => Step::Hang,
    }
}

fn selected(id: &str) -> bool {
    let only = std::env::var("AETHER_E2E_ONLY").unwrap_or_default();
    if !only.is_empty() {
        return only.split(',').any(|o| o.trim() == id);
    }
    if id.starts_with("nic.") {
        return std::env::var("AETHER_E2E_NIC").is_ok();
    }
    true
}

#[test]
#[ignore = "modifie réellement le système : lancer explicitement sur la VM de test"]
fn every_action_responds() {
    use aether_lib::modules::{network, overview, performance, storage, system};
    let dir = std::env::temp_dir().join(format!("aether-e2e-a-{}", std::process::id()));
    let state = Arc::new(AppState::new(&dir));
    let mut failures: Vec<String> = Vec::new();

    macro_rules! check {
        ($name:expr, $body:expr) => {{
            let s = Arc::clone(&state);
            let _ = &s;
            match timed(move || {
                let s = s;
                let _ = &s;
                $body(&*s).map_err(|e: aether_lib::error::AetherError| e.to_string())
            }) {
                Step::Ok(d, msg) => println!(
                    "✓ {:<26} {:>6} ms  {}",
                    $name,
                    d.as_millis(),
                    msg.chars().take(110).collect::<String>()
                ),
                Step::Err(d, e) => {
                    println!("✗ {:<26} {:>6} ms  {}", $name, d.as_millis(), e);
                    failures.push(format!("{}: {e}", $name));
                }
                Step::Hang => {
                    println!("✗ {:<26} BLOQUÉ", $name);
                    failures.push(format!("{}: bloqué", $name));
                }
            }
        }};
    }

    check!("system_info", |_: &AppState| overview::system::info().map(|i| format!(
        "{} · {} disques",
        i.cpu,
        i.disks.len()
    )));
    check!("hardware_info", |_: &AppState| Ok(format!(
        "{:?}",
        overview::system::hardware()
            .disks
            .iter()
            .map(|d| d.bus.clone())
            .collect::<Vec<_>>()
    )));
    check!("restore_status", |_: &AppState| overview::preflight::status().map(
        |r| format!(
            "activée={} dernier={:?}",
            r.enabled,
            r.last_point.map(|p| p.description)
        )
    ));
    check!("power_schemes", |_: &AppState| performance::power::list_schemes()
        .map(|l| l.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(", ")));
    check!("display_info", |_: &AppState| performance::gaming::display_info()
        .map(|d| format!("{} Hz / max {} Hz", d.current_hz, d.max_hz)));
    check!("ram_purge", |_: &AppState| performance::ram::purge().map(|r| format!(
        "{} Mo libérés, {} processus",
        r.freed / 1_048_576,
        r.trimmed
    )));
    check!("storage_scan", |_: &AppState| Ok(format!(
        "{} règles, {} Mo",
        storage::rules::all_rules().len(),
        storage::rules::all_rules()
            .iter()
            .map(|r| storage::cleaner::scan_rule(r).bytes)
            .sum::<u64>()
            / 1_048_576
    )));
    check!("storage_clean (aperçu)", |s: &AppState| {
        s.set_dry_run(true)?;
        let ids: Vec<String> = storage::rules::all_rules().into_iter().map(|r| r.id).collect();
        let r = storage::clean(s, &ids);
        Ok(format!("aperçu={} {} fichiers", r.preview, r.removed))
    });
    check!("analyzer (C:\\Users)", |_: &AppState| Ok(format!(
        "{} fichiers ≥ 100 Mo",
        storage::analyzer::scan(
            &[std::path::PathBuf::from("C:\\Users")],
            storage::analyzer::DEFAULT_MIN_BYTES,
            |_| {}
        )
        .len()
    )));
    check!("blockers", |_: &AppState| Ok(format!(
        "{:?}",
        storage::processes::running_among(&storage::rules::known_processes())
    )));
    check!("network_adapter", |_: &AppState| network::nic::snapshot(true)
        .map(|n| format!("{:?}", n.adapter.map(|a| a.name))));
    check!("dns_current", |_: &AppState| network::dns::current()
        .map(|d| format!("{:?} dhcp={}", d.servers, d.dhcp)));
    check!("dns_benchmark", |_: &AppState| {
        let rt = tokio::runtime::Runtime::new()?;
        let r = rt.block_on(async {
            let mut out = Vec::new();
            for p in network::dns::PROVIDERS {
                out.push(network::dns::bench_server(p.id, p.name, p.primary, p.secondary, 5).await);
            }
            out
        });
        Ok(r.iter()
            .map(|b| format!("{}={:?}", b.name, b.avg_ms))
            .collect::<Vec<_>>()
            .join(" "))
    });
    check!("flush_dns", |_: &AppState| aether_lib::core::command::run(
        "ipconfig.exe",
        &["/flushdns"]
    )
    .map(|_| "ok".to_owned()));
    check!("startup_list", |_: &AppState| system::startup::list()
        .map(|l| format!("{} programmes", l.len())));
    check!("appx_list", |_: &AppState| system::appx::installed().map(|l| l
        .iter()
        .map(|a| a.id.clone())
        .collect::<Vec<_>>()
        .join(", ")));
    check!("onedrive_status", |_: &AppState| system::onedrive::status()
        .map(|o| format!("installé={}", o.installed)));
    check!("edge_status", |_: &AppState| system::edge::status().map(|e| format!(
        "webview2={:?} désinstallable={}",
        e.webview2_version, e.uninstall_allowed
    )));
    check!("express (aperçu)", |s: &AppState| {
        s.set_dry_run(true)?;
        let r = overview::profiles::express(s);
        Ok(format!(
            "aperçu={} optimisées {}/{}",
            r.preview, r.after.optimized, r.after.total
        ))
    });
    check!("profil export+import", |s: &AppState| {
        let path = overview::profiles::export(s)?;
        let json = std::fs::read_to_string(&path)?;
        let _ = std::fs::remove_file(&path);
        overview::profiles::import(s, &json).map(|r| format!("{} options relues", r.len()))
    });
    check!("annuler (historique)", |s: &AppState| {
        s.set_dry_run(false)?;
        let t = aether_lib::modules::find("ui.classicMenu")?;
        let was = t.check_status(s)?.applied;
        if was {
            aether_lib::core::tweak::restore(t, s)?;
        } else {
            aether_lib::core::tweak::optimize(t, s)?;
        }
        let undone = aether_lib::core::undo::undo_last(s)?;
        let now = t.check_status(s)?.applied;
        if now != was {
            return Err(aether_lib::error::AetherError::InvalidInput(
                "l'annulation n'a pas rétabli l'état".into(),
            ));
        }
        Ok(format!("annulé : {:?}", undone.map(|e| e.label)))
    });

    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "échecs : {failures:#?}");
}

#[test]
#[ignore = "modifie réellement le système : lancer explicitement sur la VM de test"]
fn game_boost_full_session() {
    use aether_lib::modules::games::boost::{launch_with, BoostOptions, StepStatus};
    use aether_lib::modules::games::library::{Game, GameSource};
    use aether_lib::modules::performance::power;
    use std::os::windows::process::CommandExt;

    // Faux jeu : copie de ping.exe dans un dossier temporaire, lancée sans fenêtre (~12 s).
    let dir = std::env::temp_dir().join(format!("aether-fake-game-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dossier");
    let exe = dir.join("FakeGame.exe");
    std::fs::copy("C:\\Windows\\System32\\PING.EXE", &exe).expect("copie");
    let game = Game {
        id: "test:fake".into(),
        name: "Faux jeu".into(),
        source: GameSource::Gog,
        install_dir: dir.display().to_string(),
        launch: exe.display().to_string(),
        image: None,
        has_image: false,
    };
    let state_dir = std::env::temp_dir().join(format!("aether-e2e-g-{}", std::process::id()));
    let state = AppState::new(&state_dir);
    state.set_dry_run(false).expect("mode réel");
    let plan_before = power::active_guid().expect("plan");
    // On part d'un plan différent pour vérifier la bascule puis la restauration.
    power::set_active(power::BALANCED).expect("plan équilibré");

    let mut events = Vec::new();
    let started = Instant::now();
    let exe_for_start = exe.clone();
    let res = launch_with(
        &state,
        &game,
        BoostOptions {
            purge_ram: true,
            flush_dns: true,
            performance_plan: true,
            high_priority: true,
            no_throttling: true,
        },
        |e| {
            println!("  {:<9} {:?} {}", e.step, e.status, e.detail);
            events.push((e.step, e.status));
        },
        move || {
            std::process::Command::new(&exe_for_start)
                .args(["-n", "12", "127.0.0.1"])
                .creation_flags(0x0800_0000)
                .spawn()
                .map(|_| ())
                .map_err(aether_lib::error::AetherError::from)
        },
    );
    println!("session complète en {} s", started.elapsed().as_secs());
    let plan_after = power::active_guid().expect("plan");
    let _ = power::set_active(&plan_before);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state_dir);

    assert!(res.is_ok(), "{res:?}");
    for step in [
        "ram", "dns", "plan", "launch", "detect", "priority", "session", "restore",
    ] {
        assert!(
            events.iter().any(|(s, st)| *s == step && *st == StepStatus::Done),
            "étape {step} non terminée"
        );
    }
    assert!(
        plan_after.eq_ignore_ascii_case(power::BALANCED),
        "le plan d'origine n'a pas été rétabli"
    );
}

#[test]
#[ignore = "installe réellement une application (7-Zip) puis la désinstalle"]
fn app_library_installs_for_real() {
    use aether_lib::modules::apps::{install, installed_ids, InstallEvent};
    let dir = std::env::temp_dir().join(format!("aether-e2e-apps-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dossier");
    let state = AppState::new(&dir);
    state.set_dry_run(false).expect("mode réel");

    let started = Instant::now();
    let before = installed_ids(&dir).expect("export winget");
    println!(
        "{} paquets détectés en {} ms",
        before.len(),
        started.elapsed().as_millis()
    );
    assert!(
        !before.contains("7zip.7zip"),
        "7-Zip déjà installé : test non concluant"
    );

    let started = Instant::now();
    let mut percents = Vec::new();
    let ok = install(&state, "7zip.7zip", false, |e| match e {
        InstallEvent::Progress { percent } => percents.push(percent),
        InstallEvent::Line { text } => println!("  winget: {text}"),
    })
    .expect("installation");
    println!(
        "installé={ok} en {} s, progression : {:?}",
        started.elapsed().as_secs(),
        percents.last()
    );
    assert!(ok);
    assert!(
        installed_ids(&dir).expect("export").contains("7zip.7zip"),
        "7-Zip non détecté après installation"
    );

    // Remise en l'état de la VM.
    let out = aether_lib::core::command::run_raw(
        "winget.exe",
        &[
            "uninstall",
            "--id",
            "7zip.7zip",
            "-e",
            "--silent",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
    )
    .expect("désinstallation");
    println!("désinstallé (code {:?})", out.code);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "redémarre réellement l'Explorateur"]
fn explorer_restarts_unelevated() {
    let started = Instant::now();
    aether_lib::modules::system::context_menu::restart_explorer().expect("redémarrage");
    println!("Explorateur relancé en {} ms", started.elapsed().as_millis());
    assert!(!aether_lib::modules::storage::processes::running_among(&["explorer.exe"]).is_empty());
}

#[test]
#[ignore = "diagnostic sur le système réel (lecture seule)"]
fn profile_coverage_read_only() {
    use aether_lib::modules::overview::profiles::{profile_tweaks, GuidedProfile};
    let dir = std::env::temp_dir().join(format!("aether-e2e-c-{}", std::process::id()));
    let state = AppState::new(&dir);
    for p in [GuidedProfile::Daily, GuidedProfile::Gamer, GuidedProfile::Laptop] {
        println!("── {p:?}");
        for id in profile_tweaks(p) {
            match aether_lib::modules::find(id).and_then(|t| t.check_status(&state)) {
                Ok(pr) => println!(
                    "  {:<26} {}  {}",
                    id,
                    if !pr.available {
                        "INDISPONIBLE"
                    } else if pr.applied {
                        "actif      "
                    } else {
                        "inactif    "
                    },
                    pr.current
                ),
                Err(e) => println!("  {id:<26} ERREUR {e}"),
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "modifie réellement le système : lancer explicitement sur la VM de test"]
fn profile_buttons_complete_quickly() {
    use aether_lib::modules::overview::profiles::{apply_profile, GuidedProfile};
    let dir = std::env::temp_dir().join(format!("aether-e2e-p-{}", std::process::id()));
    let state = Arc::new(AppState::new(&dir));
    state.set_dry_run(false).expect("mode réel");
    for profile in [GuidedProfile::Daily, GuidedProfile::Laptop] {
        let s = Arc::clone(&state);
        match timed(move || Ok(format!("{:?}", apply_profile(&s, profile)))) {
            Step::Ok(d, r) => println!("profil {profile:?} : {} ms\n{r}", d.as_millis()),
            other => panic!("profil {profile:?} : {other:?}"),
        }
    }
    let s = Arc::clone(&state);
    match timed(move || Ok(aether_lib::modules::all_status(&s, None).len().to_string())) {
        Step::Ok(d, n) => println!("lecture de {n} options : {} ms", d.as_millis()),
        other => panic!("lecture des options : {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "modifie réellement le système : lancer explicitement sur la VM de test"]
fn every_tweak_applies_and_restores() {
    let dir = std::env::temp_dir().join(format!("aether-e2e-{}", std::process::id()));
    let state = Arc::new(AppState::new(&dir));
    state.set_dry_run(false).expect("mode réel");
    let mut failures = Vec::new();

    for boxed in catalog() {
        let t = boxed.as_ref();
        let id = t.id();
        if !selected(id) {
            continue;
        }
        let before = match t.check_status(&state) {
            Ok(p) => p,
            Err(e) => {
                println!("✗ {id:<28} lecture impossible : {e}");
                failures.push(format!("{id}: lecture {e}"));
                continue;
            }
        };
        if !before.available {
            println!("· {id:<28} indisponible ({})", before.current);
            continue;
        }

        // 1) Optimiser
        let s = Arc::clone(&state);
        let id_owned = id.to_owned();
        let step = timed(move || {
            let t = aether_lib::modules::find(&id_owned).map_err(|e| e.to_string())?;
            optimize(t, &s).map(|o| format!("{o:?}")).map_err(|e| e.to_string())
        });
        let applied_ok = match &step {
            Step::Ok(d, o) => {
                println!("✓ {id:<28} optimiser  {:>6} ms  {o}", d.as_millis());
                true
            }
            Step::Err(d, e) => {
                println!("✗ {id:<28} optimiser  {:>6} ms  ERREUR {e}", d.as_millis());
                failures.push(format!("{id}: optimiser {e}"));
                false
            }
            Step::Hang => {
                println!("✗ {id:<28} optimiser  BLOQUÉ > {} s", STEP_TIMEOUT.as_secs());
                failures.push(format!("{id}: optimiser bloqué"));
                false
            }
        };
        if !applied_ok {
            continue;
        }
        match t.check_status(&state) {
            Ok(p) if p.applied => {}
            Ok(p) => {
                println!("✗ {id:<28} état non optimisé après application ({})", p.current);
                failures.push(format!("{id}: non appliqué"));
            }
            Err(e) => failures.push(format!("{id}: relecture {e}")),
        }
        // Idempotence : une seconde optimisation ne doit rien faire.
        match optimize(t, &state) {
            Ok(Outcome::AlreadyDone) => {}
            other => failures.push(format!("{id}: second passage {other:?}")),
        }

        // 2) Restaurer l'état d'origine. Si l'option était déjà active avant le test, on teste
        //    l'aller-retour inverse (restaurer puis réappliquer) pour laisser l'état de l'utilisateur intact.
        if before.applied {
            let s = Arc::clone(&state);
            let id_owned = id.to_owned();
            let round_trip = timed(move || {
                let t = aether_lib::modules::find(&id_owned).map_err(|e| e.to_string())?;
                let r = restore(t, &s).map_err(|e| format!("restaurer : {e}"))?;
                let back = t.check_status(&s).map_err(|e| e.to_string())?;
                let o = optimize(t, &s).map_err(|e| format!("réappliquer : {e}"))?;
                Ok(format!("restaurer {r:?} (→ {}) puis réappliquer {o:?}", back.current))
            });
            match round_trip {
                Step::Ok(d, msg) => println!("  {id:<28} aller-retour {:>6} ms  {msg}", d.as_millis()),
                Step::Err(_, e) => {
                    println!("✗ {id:<28} aller-retour ERREUR {e}");
                    failures.push(format!("{id}: aller-retour {e}"));
                }
                Step::Hang => failures.push(format!("{id}: aller-retour bloqué")),
            }
            match t.check_status(&state) {
                Ok(p) if p.applied && p.current == before.current => {}
                Ok(p) => failures.push(format!(
                    "{id}: état utilisateur non rétabli ({} au lieu de {})",
                    p.current, before.current
                )),
                Err(e) => failures.push(format!("{id}: relecture {e}")),
            }
            continue;
        }
        let s = Arc::clone(&state);
        let id_owned = id.to_owned();
        match timed(move || {
            let t = aether_lib::modules::find(&id_owned).map_err(|e| e.to_string())?;
            restore(t, &s).map(|o| format!("{o:?}")).map_err(|e| e.to_string())
        }) {
            Step::Ok(d, o) => println!("  {id:<28} restaurer  {:>6} ms  {o}", d.as_millis()),
            Step::Err(d, e) => {
                println!("✗ {id:<28} restaurer  {:>6} ms  ERREUR {e}", d.as_millis());
                failures.push(format!("{id}: restaurer {e}"));
            }
            Step::Hang => {
                println!("✗ {id:<28} restaurer  BLOQUÉ");
                failures.push(format!("{id}: restaurer bloqué"));
            }
        }
        match t.check_status(&state) {
            Ok(p) if p.applied => {
                println!("✗ {id:<28} toujours optimisé après restauration ({})", p.current);
                failures.push(format!("{id}: restauration incomplète"));
            }
            Ok(p) if p.current != before.current => {
                // Garantie de sécurité : l'état exact d'avant le test doit être rétabli.
                println!(
                    "✗ {id:<28} état restauré : « {} » (avant : « {} »)",
                    p.current, before.current
                );
                failures.push(format!("{id}: état différent après restauration"));
            }
            _ => {}
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "échecs : {failures:#?}");
}

/// Faux jeu commun aux tests Boost : copie de ping.exe lancée sans fenêtre.
fn fake_game(tag: &str) -> (std::path::PathBuf, aether_lib::modules::games::library::Game) {
    use aether_lib::modules::games::library::{Game, GameSource};
    let dir = std::env::temp_dir().join(format!("aether-fake-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dossier");
    let exe = dir.join("FakeGame.exe");
    std::fs::copy("C:\\Windows\\System32\\PING.EXE", &exe).expect("copie");
    let game = Game {
        id: format!("test:{tag}"),
        name: format!("Faux jeu {tag}"),
        source: GameSource::Gog,
        install_dir: dir.display().to_string(),
        launch: exe.display().to_string(),
        image: None,
        has_image: false,
    };
    (dir, game)
}

#[test]
#[ignore = "modifie réellement le mode d'alimentation : lancer explicitement sur la VM de test"]
fn game_boost_launch_failure_restores_power_plan() {
    use aether_lib::modules::games::boost::{launch_with, BoostOptions};
    use aether_lib::modules::performance::power;
    let (dir, game) = fake_game("fail");
    let state_dir = std::env::temp_dir().join(format!("aether-e2e-gf-{}", std::process::id()));
    let state = AppState::new(&state_dir);
    let plan_before = power::active_guid().expect("plan");
    power::set_active(power::BALANCED).expect("plan équilibré");
    let res = launch_with(
        &state,
        &game,
        BoostOptions {
            purge_ram: false,
            flush_dns: false,
            performance_plan: true,
            high_priority: true,
            no_throttling: true,
        },
        |e| println!("  {:<9} {:?} {}", e.step, e.status, e.detail),
        || {
            Err(aether_lib::error::AetherError::NotFound(
                "lanceur introuvable (simulé)".into(),
            ))
        },
    );
    let plan_after = power::active_guid().expect("plan");
    let pending = state
        .backups(aether_lib::core::state::MISC_BACKUP)
        .contains("boost.previousPlan")
        .expect("lecture");
    let _ = power::set_active(&plan_before);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state_dir);
    assert!(res.is_err(), "l'échec du lancement doit être signalé");
    assert!(
        plan_after.eq_ignore_ascii_case(power::BALANCED),
        "plan d'origine non rétabli après échec"
    );
    assert!(!pending, "la sauvegarde du plan doit être effacée une fois rétabli");
}

#[test]
#[ignore = "modifie réellement le mode d'alimentation : lancer explicitement sur la VM de test"]
fn game_boost_interrupted_session_is_recovered_at_startup() {
    use aether_lib::modules::games::boost::recover_interrupted_session;
    use aether_lib::modules::performance::power;
    let state_dir = std::env::temp_dir().join(format!("aether-e2e-gr-{}", std::process::id()));
    let state = AppState::new(&state_dir);
    let plan_before = power::active_guid().expect("plan");
    // Simule une session coupée net : plan « Performances optimales » actif et plan d'origine mémorisé.
    let ultimate = power::ensure_ultimate().expect("plan ultime");
    state
        .backups(aether_lib::core::state::MISC_BACKUP)
        .record_first("boost.previousPlan", &power::BALANCED.to_owned())
        .expect("sauvegarde");
    power::set_active(&ultimate).expect("bascule");
    // Redémarrage d'Aether.
    recover_interrupted_session(&state);
    let plan_after = power::active_guid().expect("plan");
    let pending = state
        .backups(aether_lib::core::state::MISC_BACKUP)
        .contains("boost.previousPlan")
        .expect("lecture");
    let _ = power::set_active(&plan_before);
    let _ = std::fs::remove_dir_all(&state_dir);
    assert!(
        plan_after.eq_ignore_ascii_case(power::BALANCED),
        "plan non rétabli au démarrage : {plan_after}"
    );
    assert!(!pending);
}

#[test]
#[ignore = "lance réellement un faux jeu (~10 s)"]
fn game_boost_without_options_changes_nothing() {
    use aether_lib::modules::games::boost::{launch_with, BoostOptions, StepStatus};
    use aether_lib::modules::performance::power;
    use std::os::windows::process::CommandExt;
    let (dir, game) = fake_game("plain");
    let state_dir = std::env::temp_dir().join(format!("aether-e2e-gp-{}", std::process::id()));
    let state = AppState::new(&state_dir);
    let plan_before = power::active_guid().expect("plan");
    let exe = std::path::PathBuf::from(&game.launch);
    let mut events = Vec::new();
    let res = launch_with(
        &state,
        &game,
        BoostOptions {
            purge_ram: false,
            flush_dns: false,
            performance_plan: false,
            high_priority: false,
            no_throttling: false,
        },
        |e| events.push((e.step, e.status)),
        move || {
            std::process::Command::new(&exe)
                .args(["-n", "6", "127.0.0.1"])
                .creation_flags(0x0800_0000)
                .spawn()
                .map(|_| ())
                .map_err(aether_lib::error::AetherError::from)
        },
    );
    let plan_after = power::active_guid().expect("plan");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state_dir);
    assert!(res.is_ok(), "{res:?}");
    assert!(
        plan_after.eq_ignore_ascii_case(&plan_before),
        "le plan ne doit pas changer sans l'option"
    );
    for step in ["ram", "dns", "plan"] {
        assert!(
            events.iter().any(|(s, st)| *s == step && *st == StepStatus::Skipped),
            "{step} aurait dû être ignorée"
        );
    }
    assert!(
        !events.iter().any(|(s, _)| *s == "priority"),
        "aucune priorité ne doit être modifiée"
    );
    assert!(events.iter().any(|(s, st)| *s == "session" && *st == StepStatus::Done));
}

#[test]
#[ignore = "crée puis supprime réellement une tâche planifiée"]
fn autostart_task_roundtrip() {
    use aether_lib::modules::autostart;
    let before = autostart::enabled();
    autostart::set(true).expect("création");
    assert!(autostart::enabled(), "tâche absente après création");
    autostart::set(false).expect("suppression");
    assert!(!autostart::enabled(), "tâche encore présente après suppression");
    if before {
        autostart::set(true).expect("état d'origine");
    }
}

#[test]
#[ignore = "diagnostic sur le système réel (lecture seule)"]
fn diagnostics_read_real_system() {
    use aether_lib::modules::diagnostics;
    let disks = diagnostics::disks().expect("disques");
    for d in &disks {
        println!(
            "disque : {} · {} {} · {:?} · temp {:?} · usure {:?} · erreurs {}",
            d.name, d.media, d.bus, d.verdict, d.temperature, d.wear, d.uncorrected_errors
        );
    }
    assert!(!disks.is_empty(), "au moins un disque attendu");
    let impacts = diagnostics::startup_impact();
    println!("programmes ayant ralenti le démarrage : {}", impacts.len());
    for i in impacts.iter().take(5) {
        println!("  {} : {} ms", i.file, i.degradation_ms);
    }
    println!("batterie : {:?}", diagnostics::battery().expect("batterie"));
    let groups = aether_lib::modules::storage::duplicates::find(
        &aether_lib::modules::storage::duplicates::default_roots(),
        aether_lib::modules::storage::duplicates::MIN_SIZE,
    );
    println!("groupes de doublons : {}", groups.len());
}

#[test]
#[ignore = "modifie réellement une option puis la rétablit"]
fn restore_all_reverts_aether_changes_only() {
    use aether_lib::core::tweak::{optimize, Outcome};
    let dir = std::env::temp_dir().join(format!("aether-e2e-ra-{}", std::process::id()));
    let state = AppState::new(&dir);
    // Première option de confidentialité non active sur ce PC (réglage utilisateur, sans effet système lourd).
    let t = aether_lib::modules::catalog()
        .iter()
        .map(|b| b.as_ref())
        .find(|t| {
            [
                "privacy.",
                "ui.launchThisPc",
                "ui.aeroShake",
                "ui.verboseStatus",
                "fx.selectionRect",
                "fx.iconShadow",
            ]
            .iter()
            .any(|p| t.id().starts_with(p))
                && t.check_status(&state)
                    .map(|p| p.available && !p.applied)
                    .unwrap_or(false)
        })
        .expect("au moins une option inactive");
    println!("option testée : {}", t.id());
    let before = false;
    assert_eq!(optimize(t, &state).expect("optimiser"), Outcome::Applied);
    let reports = aether_lib::modules::restore_all(&state);
    println!(
        "rétablissements : {:?}",
        reports.iter().map(|r| (&r.id, &r.outcome)).collect::<Vec<_>>()
    );
    let after = t.check_status(&state).expect("lecture").applied;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(after, before, "l'état d'origine doit être rétabli");
}

#[test]
#[ignore]
fn speedtest_measures_real_connection() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let t0 = Instant::now();
    let last = std::sync::Mutex::new(String::new());
    let emit: aether_lib::modules::network::speedtest::Emit = Arc::new(move |e| {
        let p = format!("{:?}", e.phase);
        if let Ok(mut l) = last.lock() {
            if *l != p {
                println!("{:>6.1}s {p}", t0.elapsed().as_secs_f64());
                *l = p;
            }
        }
    });
    let r = rt
        .block_on(aether_lib::modules::network::speedtest::run(emit))
        .expect("test de débit");
    println!(
        "ping {:.0} ms (gigue {:.1}) · ↓ {:.0} Mbit/s · ↑ {:.0} Mbit/s",
        r.ping_ms, r.jitter_ms, r.download_mbps, r.upload_mbps
    );
    assert!(r.download_mbps > 0.0 && r.upload_mbps > 0.0 && r.ping_ms > 0.0);
}

#[test]
#[ignore]
fn lists_restore_points_read_only() {
    let points = aether_lib::modules::overview::preflight::list_points().expect("liste");
    println!(
        "{} point(s) : {:?}",
        points.len(),
        points
            .iter()
            .take(3)
            .map(|p| (p.sequence, &p.description, &p.created))
            .collect::<Vec<_>>()
    );
}

#[test]
#[ignore]
fn converts_report_to_pdf_with_edge() {
    let dir = std::env::temp_dir().join(format!("aether-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dossier");
    let html = dir.join("r.html");
    std::fs::write(&html, "<html><body><h1>Rapport Aether</h1><p>Test</p></body></html>").expect("html");
    let t0 = Instant::now();
    let pdf = aether_lib::modules::overview::report::html_to_pdf(&html, &dir.join("r.pdf"));
    println!(
        "PDF {:?} en {:?} ({} o)",
        pdf,
        t0.elapsed(),
        pdf.as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0)
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(pdf.is_some());
}
