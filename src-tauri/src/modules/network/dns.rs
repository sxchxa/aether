//! DNS : lecture de la configuration, benchmark UDP asynchrone et application.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::core::command::{self, ps_quote};
use crate::core::history::UndoAction;
use crate::core::registry::{self, Hive, RegData};
use crate::core::state::{AppState, MISC_BACKUP};
use crate::error::{AetherError, AetherResult};
use crate::modules::network::nic;

/// Fournisseur DNS public testé.
#[derive(Debug, Clone, Copy)]
pub struct Provider {
    /// Identifiant.
    pub id: &'static str,
    /// Nom affiché.
    pub name: &'static str,
    /// Serveur primaire.
    pub primary: &'static str,
    /// Serveur secondaire.
    pub secondary: &'static str,
}

/// Fournisseurs comparés : les trois résolveurs publics les plus utilisés et les plus fiables.
pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "cloudflare",
        name: "Cloudflare",
        primary: "1.1.1.1",
        secondary: "1.0.0.1",
    },
    Provider {
        id: "google",
        name: "Google",
        primary: "8.8.8.8",
        secondary: "8.8.4.4",
    },
    Provider {
        id: "quad9",
        name: "Quad9",
        primary: "9.9.9.9",
        secondary: "149.112.112.112",
    },
];

const TEST_DOMAINS: &[&str] = &[
    "www.google.com",
    "www.cloudflare.com",
    "www.microsoft.com",
    "www.wikipedia.org",
    "www.amazon.com",
];

/// Configuration DNS de la carte active.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsConfig {
    /// Index d'interface.
    pub interface_index: u32,
    /// Nom de la carte.
    pub interface_name: String,
    /// Serveurs IPv4 actuellement utilisés.
    pub servers: Vec<String>,
    /// Serveurs fournis automatiquement par DHCP (aucun serveur statique).
    pub dhcp: bool,
}

/// Résultat de benchmark d'un serveur.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsBenchResult {
    /// Identifiant (`cloudflare`, `current`…).
    pub id: String,
    /// Nom affiché.
    pub name: String,
    /// Serveur primaire testé.
    pub primary: String,
    /// Serveur secondaire.
    pub secondary: String,
    /// Latence moyenne (ms) sur les requêtes réussies.
    pub avg_ms: Option<f64>,
    /// Meilleure latence (ms).
    pub min_ms: Option<f64>,
    /// Requêtes réussies.
    pub success: u32,
    /// Requêtes envoyées.
    pub attempts: u32,
}

/// Construit une requête DNS de type A (RFC 1035).
pub fn build_query(id: u16, domain: &str) -> Vec<u8> {
    let mut packet = Vec::with_capacity(32 + domain.len());
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&[0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    for label in domain.split('.').filter(|l| !l.is_empty()) {
        let bytes = label.as_bytes();
        packet.push(bytes.len().min(63) as u8);
        packet.extend_from_slice(&bytes[..bytes.len().min(63)]);
    }
    packet.extend_from_slice(&[0x00, 0x00, 0x01, 0x00, 0x01]);
    packet
}

/// Vérifie qu'un paquet est une réponse valide à la requête `id`.
pub fn is_response_to(packet: &[u8], id: u16) -> bool {
    packet.len() >= 12 && packet[0..2] == id.to_be_bytes() && packet[2] & 0x80 != 0
}

async fn measure(server: IpAddr, domain: &str, id: u16) -> Option<f64> {
    let bind: SocketAddr = if server.is_ipv4() {
        "0.0.0.0:0".parse().ok()?
    } else {
        "[::]:0".parse().ok()?
    };
    let socket = UdpSocket::bind(bind).await.ok()?;
    socket.connect(SocketAddr::new(server, 53)).await.ok()?;
    let query = build_query(id, domain);
    let start = Instant::now();
    socket.send(&query).await.ok()?;
    let mut buf = [0u8; 512];
    let deadline = Duration::from_millis(1500);
    loop {
        let remaining = deadline.checked_sub(start.elapsed())?;
        let n = timeout(remaining, socket.recv(&mut buf)).await.ok()?.ok()?;
        if is_response_to(&buf[..n], id) {
            return Some(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

/// Mesure la latence d'un serveur sur `attempts` requêtes (domaines variés).
///
/// Impact Windows : envoie des requêtes DNS UDP ; aucune modification système.
pub async fn bench_server(id: &str, name: &str, primary: &str, secondary: &str, attempts: u32) -> DnsBenchResult {
    let mut samples = Vec::new();
    if let Ok(ip) = primary.parse::<IpAddr>() {
        for i in 0..attempts {
            let domain = TEST_DOMAINS[i as usize % TEST_DOMAINS.len()];
            let qid = 0xAE00u16.wrapping_add(i as u16);
            if let Some(ms) = measure(ip, domain, qid).await {
                samples.push(ms);
            }
        }
    }
    let avg = (!samples.is_empty()).then(|| samples.iter().sum::<f64>() / samples.len() as f64);
    let min = samples
        .iter()
        .copied()
        .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.min(v))));
    DnsBenchResult {
        id: id.to_owned(),
        name: name.to_owned(),
        primary: primary.to_owned(),
        secondary: secondary.to_owned(),
        avg_ms: avg.map(|v| (v * 10.0).round() / 10.0),
        min_ms: min.map(|v| (v * 10.0).round() / 10.0),
        success: samples.len() as u32,
        attempts,
    }
}

/// Lit la configuration DNS IPv4 de la carte active.
///
/// Impact Windows : lecture seule (`Get-DnsClientServerAddress` + Registre Tcpip).
pub fn current() -> AetherResult<DnsConfig> {
    let adapter = nic::snapshot(false)?
        .adapter
        .ok_or_else(|| AetherError::NotFound("carte réseau active".into()))?;
    // Lecture native : serveurs statiques (`NameServer`) sinon ceux fournis par DHCP.
    let key = format!(
        "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{}",
        adapter.guid
    );
    let split = |s: String| -> Vec<String> {
        s.split([',', ' '])
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let static_servers = match registry::read(Hive::Hklm, &key, "NameServer")? {
        Some(RegData::Sz(s)) => split(s),
        _ => Vec::new(),
    };
    let dhcp = static_servers.is_empty();
    let servers = if dhcp {
        match registry::read(Hive::Hklm, &key, "DhcpNameServer")? {
            Some(RegData::Sz(s)) => split(s),
            _ => Vec::new(),
        }
    } else {
        static_servers
    };
    Ok(DnsConfig {
        interface_index: adapter.index,
        interface_name: adapter.name,
        servers,
        dhcp,
    })
}

/// Définit les serveurs DNS IPv4 d'une interface (liste vide = retour au DHCP).
///
/// Impact Windows : `Set-DnsClientServerAddress` (`-ResetServerAddresses` pour le DHCP).
pub fn set_servers(interface_index: u32, servers: &[String]) -> AetherResult<()> {
    for s in servers {
        s.parse::<IpAddr>()
            .map_err(|_| AetherError::InvalidInput(format!("adresse DNS invalide : {s}")))?;
    }
    let script = if servers.is_empty() {
        format!("Set-DnsClientServerAddress -InterfaceIndex {interface_index} -ResetServerAddresses")
    } else {
        let list = servers.iter().map(|s| ps_quote(s)).collect::<Vec<_>>().join(",");
        format!("Set-DnsClientServerAddress -InterfaceIndex {interface_index} -ServerAddresses @({list})")
    };
    command::powershell(&script)?;
    let _ = command::run_raw("ipconfig.exe", &["/flushdns"]);
    Ok(())
}

fn previous_servers(cfg: &DnsConfig) -> Vec<String> {
    if cfg.dhcp {
        Vec::new()
    } else {
        cfg.servers.clone()
    }
}

/// PC membre d'un domaine d'entreprise (suffixe DNS principal défini).
///
/// Impact Windows : lecture seule (Registre Tcpip).
fn domain_joined() -> bool {
    registry::read_text(
        Hive::Hklm,
        "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters",
        "Domain",
    )
    .ok()
    .flatten()
    .is_some_and(|d| !d.trim().is_empty())
}

/// La carte active utilise une adresse IP fixe (pas de DHCP pour l'adresse).
fn static_ip(interface_index: u32) -> bool {
    let Ok(Some(adapter)) = crate::modules::network::nic::active_adapter() else {
        return false;
    };
    if adapter.index != interface_index {
        return false;
    }
    let key = format!(
        "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{}",
        adapter.guid
    );
    matches!(
        registry::read(Hive::Hklm, &key, "EnableDHCP"),
        Ok(Some(RegData::Dword(0)))
    )
}

/// Applique un couple de serveurs DNS sur la carte active (sauvegarde + annulation).
///
/// Impact Windows : modifie les DNS IPv4 de la carte active.
pub fn apply(state: &AppState, primary: &str, secondary: &str) -> AetherResult<bool> {
    // Réseau d'entreprise : un DNS public casserait l'ouverture de session et les ressources internes.
    if domain_joined() {
        return Err(AetherError::Forbidden(
            "ce PC est membre d'un domaine d'entreprise : le DNS est géré par votre service informatique".into(),
        ));
    }
    let cfg = current()?;
    let wanted: Vec<String> = [primary, secondary]
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| (*s).to_owned())
        .collect();
    if !cfg.dhcp && cfg.servers == wanted {
        state.logger.log(
            crate::core::logger::LogLevel::Skip,
            "dns",
            format!("[OK - Déjà optimisé] DNS {}", wanted.join(", ")),
        );
        return Ok(false);
    }
    if state.dry_run() {
        state.logger.log(
            crate::core::logger::LogLevel::Preview,
            "dns",
            format!("[APERÇU] DNS {} → {}", cfg.servers.join(", "), wanted.join(", ")),
        );
        return Ok(false);
    }
    let previous = previous_servers(&cfg);
    state
        .backups(MISC_BACKUP)
        .record_first(&format!("dns.{}", cfg.interface_index), &previous)?;
    set_servers(cfg.interface_index, &wanted)?;
    state.record(
        "dns.apply",
        &format!("DNS {}", wanted.join(", ")),
        vec![UndoAction::Dns {
            interface_index: cfg.interface_index,
            servers: previous,
        }],
    )?;
    state
        .logger
        .success("dns", format!("[Appliqué] DNS {}", wanted.join(", ")));
    Ok(true)
}

/// Rétablit le DNS automatique (DHCP) sur la carte active.
///
/// Impact Windows : `Set-DnsClientServerAddress -ResetServerAddresses`.
pub fn reset_dhcp(state: &AppState) -> AetherResult<bool> {
    let cfg = current()?;
    if cfg.dhcp {
        state.logger.log(
            crate::core::logger::LogLevel::Skip,
            "dns",
            "[OK - Déjà en DHCP] DNS automatique",
        );
        return Ok(false);
    }
    // Adresse IP fixe sans DHCP : le DNS « automatique » laisserait la carte sans aucun DNS.
    if static_ip(cfg.interface_index) {
        return Err(AetherError::Forbidden("cette carte utilise une adresse IP fixe : le DNS automatique la laisserait sans DNS. Choisissez plutôt un serveur DNS".into()));
    }
    if state.dry_run() {
        state.logger.log(
            crate::core::logger::LogLevel::Preview,
            "dns",
            "[APERÇU] Retour au DNS automatique (DHCP)",
        );
        return Ok(false);
    }
    set_servers(cfg.interface_index, &[])?;
    state.record(
        "dns.reset",
        "DNS automatique (DHCP)",
        vec![UndoAction::Dns {
            interface_index: cfg.interface_index,
            servers: cfg.servers,
        }],
    )?;
    state.logger.success("dns", "[Appliqué] DNS automatique (DHCP)");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_rfc1035_query() {
        let q = build_query(0x1234, "www.google.com");
        assert_eq!(&q[0..2], &[0x12, 0x34]);
        assert_eq!(q[12], 3);
        assert_eq!(&q[13..16], b"www");
        assert_eq!(&q[q.len() - 4..], &[0x00, 0x01, 0x00, 0x01]);
    }

    #[test]
    fn validates_responses() {
        let mut resp = build_query(7, "a.b");
        assert!(!is_response_to(&resp, 7));
        resp[2] |= 0x80;
        assert!(is_response_to(&resp, 7));
        assert!(!is_response_to(&resp, 8));
    }

    #[test]
    fn rejects_invalid_dns_addresses() {
        assert!(set_servers(1, &["1.1.1.1; Remove-Item C:\\".to_owned()]).is_err());
    }

    #[tokio::test]
    async fn unreachable_server_reports_no_latency() {
        // 192.0.2.0/24 (TEST-NET-1) n'est jamais routé : la requête expire proprement.
        let r = bench_server("test", "Test", "192.0.2.1", "", 1).await;
        assert_eq!(r.success, 0);
        assert!(r.avg_ms.is_none());
    }
}
