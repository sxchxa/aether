//! Test de débit (latence, téléchargement, envoi) sur les serveurs de test publics de Cloudflare
//! (`speed.cloudflare.com`, ceux du site officiel de mesure). Durée totale bornée (~15 s), résultats
//! envoyés en direct à l'interface.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::error::{AetherError, AetherResult};

const DOWN_URL: &str = "https://speed.cloudflare.com/__down";
const UP_URL: &str = "https://speed.cloudflare.com/__up";
const PING_SAMPLES: usize = 8;
const PING_TIMEOUT: Duration = Duration::from_secs(2);
const DOWN_STREAMS: usize = 4;
const UP_STREAMS: usize = 3;
const DOWN_DURATION: Duration = Duration::from_secs(6);
const UP_DURATION: Duration = Duration::from_secs(5);
/// Taille demandée par requête de téléchargement (la mesure s'arrête à la fin de la durée).
const DOWN_CHUNK_BYTES: u64 = 50_000_000;
/// Taille de chaque envoi (plusieurs envois successifs par flux).
const UP_CHUNK_BYTES: usize = 2_000_000;
const TICK: Duration = Duration::from_millis(250);

/// Destinataire des étapes du test (canal de l'interface, ou journal dans les tests).
pub type Emit = Arc<dyn Fn(SpeedEvent) + Send + Sync>;

/// Étape en cours.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeedPhase {
    /// Mesure de la latence.
    Ping,
    /// Téléchargement.
    Download,
    /// Envoi.
    Upload,
    /// Terminé.
    Done,
}

/// Progression envoyée à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedEvent {
    /// Étape en cours.
    pub phase: SpeedPhase,
    /// Débit instantané de l'étape (Mbit/s).
    pub mbps: Option<f64>,
}

/// Résultat final.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    /// Latence médiane (ms).
    pub ping_ms: f64,
    /// Variation de la latence (ms).
    pub jitter_ms: f64,
    /// Débit descendant (Mbit/s).
    pub download_mbps: f64,
    /// Débit montant (Mbit/s).
    pub upload_mbps: f64,
}

fn net(e: reqwest::Error) -> AetherError {
    if e.status() == Some(reqwest::StatusCode::TOO_MANY_REQUESTS) {
        return AetherError::Network(
            "le serveur de test limite les mesures répétées : réessayez dans quelques minutes".into(),
        );
    }
    AetherError::Network(e.to_string())
}

/// Réponse acceptée uniquement si le serveur a répondu « 2xx » (sinon : erreur explicite).
fn ok(resp: reqwest::Response) -> AetherResult<reqwest::Response> {
    resp.error_for_status().map_err(net)
}

fn client() -> AetherResult<reqwest::Client> {
    // Fournisseur cryptographique de rustls (le même que celui du module de mise à jour).
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .user_agent(concat!("Aether/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(net)
}

fn mbps(bytes: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64().max(0.001);
    (bytes as f64 * 8.0) / secs / 1_000_000.0
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    match values.len() {
        0 => 0.0,
        n if n % 2 == 1 => values[n / 2],
        n => (values[n / 2 - 1] + values[n / 2]) / 2.0,
    }
}

/// Latence : temps de réponse d'une requête vide (connexion déjà ouverte), médiane et gigue.
async fn ping(client: &reqwest::Client) -> AetherResult<(f64, f64)> {
    // Première requête : ouverture de la connexion (non comptée).
    ok(client.get(format!("{DOWN_URL}?bytes=0")).send().await.map_err(net)?)?;
    let mut samples = Vec::with_capacity(PING_SAMPLES);
    for _ in 0..PING_SAMPLES {
        let start = Instant::now();
        // Une réponse anormalement lente (> 2 s) est ignorée plutôt que de fausser la mesure.
        let request = async {
            client
                .get(format!("{DOWN_URL}?bytes=0"))
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await
        };
        if let Ok(Ok(_)) = tokio::time::timeout(PING_TIMEOUT, request).await {
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    if samples.is_empty() {
        return Err(AetherError::Network("aucune réponse du serveur de test".into()));
    }
    Ok(latency_stats(&samples))
}

/// Latence médiane et gigue (écart moyen entre mesures successives), valeurs aberrantes exclues
/// (plus de trois fois la médiane : paquet perdu ou retransmis, pas la latence réelle).
fn latency_stats(samples: &[f64]) -> (f64, f64) {
    let med = median(&mut samples.to_vec());
    let kept: Vec<f64> = samples.iter().copied().filter(|v| *v <= med * 3.0).collect();
    let jitter = if kept.len() < 2 {
        0.0
    } else {
        kept.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (kept.len() - 1) as f64
    };
    (med, jitter)
}

/// Publie le débit courant toutes les 250 ms jusqu'à la fin de l'étape.
async fn report_progress(
    phase: SpeedPhase,
    bytes: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
    start: Instant,
    events: Emit,
) {
    while !done.load(Ordering::Relaxed) {
        tokio::time::sleep(TICK).await;
        events(SpeedEvent {
            phase,
            mbps: Some(mbps(bytes.load(Ordering::Relaxed), start.elapsed())),
        });
    }
}

async fn download(client: &reqwest::Client, events: &Emit) -> AetherResult<f64> {
    let bytes = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let deadline = start + DOWN_DURATION;
    let progress = tokio::spawn(report_progress(
        SpeedPhase::Download,
        bytes.clone(),
        done.clone(),
        start,
        events.clone(),
    ));
    let mut streams = Vec::new();
    for _ in 0..DOWN_STREAMS {
        let (client, bytes) = (client.clone(), bytes.clone());
        streams.push(tokio::spawn(async move {
            // Chaque attente est bornée par l'échéance de l'étape.
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                let Ok(resp) =
                    tokio::time::timeout(left, client.get(format!("{DOWN_URL}?bytes={DOWN_CHUNK_BYTES}")).send()).await
                else {
                    break;
                };
                let mut resp = ok(resp.map_err(net)?)?;
                while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                    match tokio::time::timeout(left, resp.chunk()).await {
                        Ok(Ok(Some(chunk))) => {
                            bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                        }
                        Ok(Ok(None)) | Err(_) => break,
                        Ok(Err(e)) => return Err(net(e)),
                    }
                }
            }
            Ok::<(), AetherError>(())
        }));
    }
    let mut first_error = None;
    for s in streams {
        if let Ok(Err(e)) = s.await {
            first_error.get_or_insert(e);
        }
    }
    done.store(true, Ordering::Relaxed);
    let _ = progress.await;
    let total = bytes.load(Ordering::Relaxed);
    match first_error {
        Some(e) if total == 0 => Err(e),
        _ => Ok(mbps(total, start.elapsed().min(DOWN_DURATION))),
    }
}

async fn upload(client: &reqwest::Client, events: &Emit) -> AetherResult<f64> {
    let payload = bytes::Bytes::from(vec![0u8; UP_CHUNK_BYTES]);
    let sent = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let deadline = start + UP_DURATION;
    let progress = tokio::spawn(report_progress(
        SpeedPhase::Upload,
        sent.clone(),
        done.clone(),
        start,
        events.clone(),
    ));
    let mut streams = Vec::new();
    for _ in 0..UP_STREAMS {
        let (client, sent, payload) = (client.clone(), sent.clone(), payload.clone());
        streams.push(tokio::spawn(async move {
            // Chaque envoi est borné par l'échéance : un envoi inachevé n'est pas compté.
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                match tokio::time::timeout(left, client.post(UP_URL).body(payload.clone()).send()).await {
                    Ok(resp) => {
                        ok(resp.map_err(net)?)?;
                        sent.fetch_add(UP_CHUNK_BYTES as u64, Ordering::Relaxed);
                    }
                    Err(_) => break,
                }
            }
            Ok::<(), AetherError>(())
        }));
    }
    let mut first_error = None;
    for s in streams {
        if let Ok(Err(e)) = s.await {
            first_error.get_or_insert(e);
        }
    }
    done.store(true, Ordering::Relaxed);
    let _ = progress.await;
    let total = sent.load(Ordering::Relaxed);
    match first_error {
        Some(e) if total == 0 => Err(e),
        _ => Ok(mbps(total, start.elapsed().min(UP_DURATION))),
    }
}

/// Lance le test complet : latence, puis téléchargement, puis envoi.
pub async fn run(events: Emit) -> AetherResult<SpeedResult> {
    let client = client()?;
    events(SpeedEvent {
        phase: SpeedPhase::Ping,
        mbps: None,
    });
    let (ping_ms, jitter_ms) = ping(&client).await?;
    let download_mbps = download(&client, &events).await?;
    let upload_mbps = upload(&client, &events).await?;
    events(SpeedEvent {
        phase: SpeedPhase::Done,
        mbps: None,
    });
    Ok(SpeedResult {
        ping_ms,
        jitter_ms,
        download_mbps,
        upload_mbps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_bytes_to_megabits_per_second() {
        assert!((mbps(12_500_000, Duration::from_secs(1)) - 100.0).abs() < 1e-9);
        assert!((mbps(0, Duration::from_secs(3))).abs() < 1e-9);
    }

    #[test]
    fn latency_ignores_outliers() {
        let (med, jitter) = latency_stats(&[20.0, 22.0, 900.0, 21.0, 23.0]);
        assert!((med - 22.0).abs() < 1e-9);
        assert!(jitter < 3.0, "gigue {jitter}");
    }

    #[test]
    fn median_of_even_and_odd_sets() {
        assert!((median(&mut [3.0, 1.0, 2.0]) - 2.0).abs() < 1e-9);
        assert!((median(&mut [4.0, 1.0, 3.0, 2.0]) - 2.5).abs() < 1e-9);
        assert!(median(&mut []).abs() < 1e-9);
    }
}
