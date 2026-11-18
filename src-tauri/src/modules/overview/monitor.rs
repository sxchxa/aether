//! Monitoring CPU/RAM temps réel diffusé à 60 FPS via `tauri::ipc::Channel`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL};
use tauri::ipc::Channel;

use crate::core::logger::now_ms;

/// Image de métriques envoyée à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsFrame {
    /// Horodatage (ms Unix).
    pub ts: u64,
    /// Utilisation CPU globale (%).
    pub cpu: f32,
    /// Utilisation par cœur logique (%).
    pub cores: Vec<f32>,
    /// RAM utilisée (octets).
    pub ram_used: u64,
    /// RAM totale (octets).
    pub ram_total: u64,
}

/// Lance le flux de métriques dans un thread dédié jusqu'à ce que `stop` passe à `true`
/// ou que la fenêtre ferme le canal.
///
/// Impact Windows : lecture seule ; la mesure CPU est rafraîchie à l'intervalle minimal
/// supporté par `sysinfo`, la RAM à chaque image.
pub fn spawn(channel: Channel<MetricsFrame>, stop: Arc<AtomicBool>, fps: u32) {
    let frame = Duration::from_micros(1_000_000 / u64::from(fps.clamp(1, 120)));
    thread::spawn(move || {
        let Ok(mut sys) = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_usage())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        ) else {
            return;
        };
        let cpu_interval = MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250));
        let mut last_cpu = Instant::now();
        sys.refresh_cpu_usage();
        while !stop.load(Ordering::Relaxed) {
            let started = Instant::now();
            if last_cpu.elapsed() >= cpu_interval {
                sys.refresh_cpu_usage();
                last_cpu = Instant::now();
            }
            sys.refresh_memory();
            let payload = MetricsFrame {
                ts: now_ms(),
                cpu: sys.global_cpu_usage(),
                cores: sys.cpus().iter().map(|c| c.usage()).collect(),
                ram_used: sys.used_memory(),
                ram_total: sys.total_memory(),
            };
            if channel.send(payload).is_err() {
                break;
            }
            if let Some(rest) = frame.checked_sub(started.elapsed()) {
                thread::sleep(rest);
            }
        }
    });
}
