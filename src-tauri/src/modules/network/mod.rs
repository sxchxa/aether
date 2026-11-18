//! Section « Réseau & Latence » : carte réseau, DNS, TCP gaming et maintenance.

pub mod dns;
pub mod nic;
pub mod speedtest;

use serde::Serialize;
use tauri::State;

use crate::core::command;
use crate::core::logger::LogLevel;
use crate::core::reg_tweak::{RegConst, RegTarget, RegistryTweak};
use crate::core::registry::Hive;
use crate::core::tweak::{Risk, Section};
use crate::error::AetherResult;
use crate::modules::{blocking, Shared};

const SYSTEM_PROFILE: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile";

/// Désactivation du Network Throttling Index et priorité maximale aux applications au premier plan.
pub static TCP_TARGETS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: SYSTEM_PROFILE,
        name: "NetworkThrottlingIndex",
        target: RegConst::Dword(0xffff_ffff),
        default: Some(RegConst::Dword(10)),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: SYSTEM_PROFILE,
        name: "SystemResponsiveness",
        target: RegConst::Dword(0),
        default: Some(RegConst::Dword(20)),
    },
];

/// Option « Optimisation TCP Gaming ».
pub fn tcp_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "tcp.throttling",
        section: Section::Network,
        group: "tcp",
        risk: Risk::Moderate,
        targets: TCP_TARGETS,
        reboot: true,
        refresh_mouse: false,
    }
}

/// Stratégie officielle Delivery Optimization : téléchargement HTTP uniquement, sans pair-à-pair.
pub static DELIVERY_TARGETS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Policies\\Microsoft\\Windows\\DeliveryOptimization",
    name: "DODownloadMode",
    target: RegConst::Dword(0),
    default: None,
}];

/// Option « Pas de partage pair-à-pair des mises à jour ».
pub fn delivery_tweak() -> RegistryTweak {
    RegistryTweak {
        id: "net.deliveryOptimization",
        section: Section::Network,
        group: "bandwidth",
        risk: Risk::Safe,
        targets: DELIVERY_TARGETS,
        reboot: false,
        refresh_mouse: false,
    }
}

/// DNS over HTTPS autorisé (stratégie « Configurer la résolution de noms DoH » = Autoriser).
pub static DOH: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Policies\\Microsoft\\Windows NT\\DNSClient",
    name: "DoHPolicy",
    target: RegConst::Dword(2),
    default: None,
}];

/// LLMNR désactivé (stratégie « Désactiver la résolution de noms multidiffusion »).
pub static LLMNR: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SOFTWARE\\Policies\\Microsoft\\Windows NT\\DNSClient",
    name: "EnableMulticast",
    target: RegConst::Dword(0),
    default: None,
}];

/// Serveur SMBv1 désactivé (recommandation Microsoft, protocole obsolète).
pub static SMB1: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: "SYSTEM\\CurrentControlSet\\Services\\LanmanServer\\Parameters",
    name: "SMB1",
    target: RegConst::Dword(0),
    default: None,
}];

fn net_policy(
    id: &'static str,
    group: &'static str,
    risk: Risk,
    targets: &'static [RegTarget],
    reboot: bool,
) -> RegistryTweak {
    RegistryTweak {
        id,
        section: Section::Network,
        group,
        risk,
        targets,
        reboot,
        refresh_mouse: false,
    }
}

/// Options réseau de sécurité et de confidentialité.
pub fn security_tweaks() -> Vec<RegistryTweak> {
    vec![
        net_policy("net.doh", "security", Risk::Safe, DOH, false),
        net_policy("net.llmnr", "security", Risk::Safe, LLMNR, false),
        net_policy("net.smb1", "security", Risk::Moderate, SMB1, true),
    ]
}

/// Options de carte réseau.
pub fn nic_tweaks() -> Vec<nic::NicTweak> {
    vec![
        nic::NicTweak {
            id: "nic.wakeMagic",
            keyword: "*WakeOnMagicPacket",
            risk: Risk::Safe,
        },
        nic::NicTweak {
            id: "nic.wakePattern",
            keyword: "*WakeOnPattern",
            risk: Risk::Safe,
        },
        nic::NicTweak {
            id: "nic.eee",
            keyword: "*EEE",
            risk: Risk::Moderate,
        },
        nic::NicTweak {
            id: "nic.interruptModeration",
            keyword: "*InterruptModeration",
            risk: Risk::Advanced,
        },
    ]
}

/// Résultat d'une commande de maintenance.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceResult {
    /// Exécutée réellement (faux en Mode Aperçu).
    pub executed: bool,
    /// Redémarrage nécessaire.
    pub needs_reboot: bool,
    /// Sortie de la commande.
    pub output: String,
}

/// Carte active et propriétés avancées.
#[tauri::command]
pub async fn network_adapter(force: Option<bool>) -> AetherResult<nic::NicSnapshot> {
    tokio::task::spawn_blocking(move || nic::snapshot(force.unwrap_or(false))).await?
}

/// Configuration DNS actuelle.
#[tauri::command]
pub async fn dns_current() -> AetherResult<dns::DnsConfig> {
    tokio::task::spawn_blocking(dns::current).await?
}

/// Benchmark DNS : 5 requêtes par fournisseur + DNS actuel, trié par latence.
#[tauri::command]
pub async fn dns_benchmark(state: State<'_, Shared>) -> AetherResult<Vec<dns::DnsBenchResult>> {
    state
        .logger
        .info("dns", "Benchmark DNS en cours (5 requêtes par serveur)…");
    // Uniquement les grands résolveurs publics de référence : le DNS de la box ou du fournisseur
    // d'accès n'est pas comparé (adresses inconnues, souvent plus lentes et sans chiffrement).
    let mut tasks = Vec::new();
    for p in dns::PROVIDERS {
        tasks.push(tokio::spawn(dns::bench_server(p.id, p.name, p.primary, p.secondary, 5)));
    }
    let mut results = Vec::new();
    for t in tasks {
        results.push(t.await?);
    }
    results.sort_by(|a, b| {
        a.avg_ms
            .unwrap_or(f64::MAX)
            .partial_cmp(&b.avg_ms.unwrap_or(f64::MAX))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some(best) = results.first() {
        state.logger.info(
            "dns",
            format!(
                "Benchmark terminé : {} le plus rapide ({:?} ms)",
                best.name, best.avg_ms
            ),
        );
    }
    Ok(results)
}

/// Applique un couple de serveurs DNS.
#[tauri::command]
pub async fn dns_apply(state: State<'_, Shared>, primary: String, secondary: String) -> AetherResult<bool> {
    blocking(&state, move |s| dns::apply(s, &primary, &secondary)).await
}

/// Rétablit le DNS automatique (DHCP).
#[tauri::command]
pub async fn dns_reset(state: State<'_, Shared>) -> AetherResult<bool> {
    blocking(&state, dns::reset_dhcp).await
}

/// Test de débit (latence, téléchargement, envoi) avec progression en direct.
#[tauri::command]
pub async fn net_speedtest(
    state: State<'_, Shared>,
    on_event: tauri::ipc::Channel<speedtest::SpeedEvent>,
) -> AetherResult<speedtest::SpeedResult> {
    state.logger.info("network", "Test de débit en cours…");
    let emit: speedtest::Emit = std::sync::Arc::new(move |e| {
        let _ = on_event.send(e);
    });
    let result = speedtest::run(emit).await?;
    state.logger.success(
        "network",
        format!(
            "[Test de débit] ↓ {:.0} Mbit/s · ↑ {:.0} Mbit/s · {:.0} ms",
            result.download_mbps, result.upload_mbps, result.ping_ms
        ),
    );
    Ok(result)
}

/// Purge le cache DNS (`ipconfig /flushdns`) — action non persistante, toujours exécutée.
#[tauri::command]
pub async fn net_flush_dns(state: State<'_, Shared>) -> AetherResult<MaintenanceResult> {
    blocking(&state, |s| {
        let out = command::run("ipconfig.exe", &["/flushdns"])?;
        s.logger.success("network", "[Appliqué] Cache DNS purgé");
        Ok(MaintenanceResult {
            executed: true,
            needs_reboot: false,
            output: out.stdout.trim().to_owned(),
        })
    })
    .await
}

/// Réinitialise le catalogue Winsock (`netsh winsock reset`) — redémarrage requis.
#[tauri::command]
pub async fn net_winsock_reset(state: State<'_, Shared>) -> AetherResult<MaintenanceResult> {
    blocking(&state, |s| {
        if s.dry_run() {
            s.logger
                .log(LogLevel::Preview, "network", "[APERÇU] netsh winsock reset");
            return Ok(MaintenanceResult {
                executed: false,
                needs_reboot: false,
                output: String::new(),
            });
        }
        let out = command::run("netsh.exe", &["winsock", "reset"])?;
        s.logger
            .success("network", "[Appliqué] Winsock réinitialisé (redémarrage requis)");
        Ok(MaintenanceResult {
            executed: true,
            needs_reboot: true,
            output: out.stdout.trim().to_owned(),
        })
    })
    .await
}
