// Couche IPC typée : une fonction par commande Tauri du backend Rust.
import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AetherError,
  AnalyzerProgress,
  BatteryHealth,
  BoostEvent,
  BoostOptions,
  CleanRule,
  CleanSummary,
  DiskHealth,
  DisplayInfo,
  DnsBenchResult,
  DnsConfig,
  DuplicateGroup,
  EdgeStatus,
  ExpressReport,
  Game,
  GuidedProfile,
  HistoryEntry,
  InstalledApp,
  InstallEvent,
  LargeFile,
  LastRun,
  LibraryApp,
  LogEntry,
  MaintenanceResult,
  Measures,
  MetricsFrame,
  NicSnapshot,
  OneDriveStatus,
  Outcome,
  PowerScheme,
  RamReport,
  RepairEvent,
  RepairTool,
  RestorePointEntry,
  RestoreStatus,
  RunningApp,
  ScanResult,
  Settings,
  SpeedEvent,
  SpeedResult,
  Win11Status,
  StartupEntry,
  StartupImpact,
  SystemInfo,
  TweakReport,
  TweakStatus,
} from "./types";

export function isAetherError(value: unknown): value is AetherError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof (value as { message: unknown }).message === "string"
  );
}

export function errorMessage(value: unknown): string {
  if (isAetherError(value)) return value.message;
  if (value instanceof Error) return value.message;
  return String(value);
}

export const api = {
  // Options (moteur idempotent)
  tweaksStatus: (ids?: string[]) => invoke<TweakStatus[]>("tweaks_status", { ids: ids ?? null }),
  tweakOptimize: (id: string) => invoke<Outcome>("tweak_optimize", { id }),
  tweakRestore: (id: string) => invoke<Outcome>("tweak_restore", { id }),
  tweaksOptimizeMany: (ids: string[]) => invoke<TweakReport[]>("tweaks_optimize_many", { ids }),
  powerMaxIds: () => invoke<string[]>("power_max_ids"),
  hardwareInfo: () => invoke<{ disks: SystemInfo["physicalDisks"]; battery: SystemInfo["battery"] }>("hardware_info"),
  historyList: () => invoke<HistoryEntry[]>("history_list"),
  historyUndo: () => invoke<HistoryEntry | null>("history_undo"),
  settingsGet: () => invoke<Settings>("settings_get"),
  setDryRun: (value: boolean) => invoke<Settings>("settings_set_dry_run", { value }),
  logsGet: () => invoke<LogEntry[]>("logs_get"),
  logsClear: () => invoke<void>("logs_clear"),
  openSettingsPage: (uri: string) => invoke<void>("open_settings_page", { uri }),
  trayLocalize: (labels: string[]) => invoke<void>("tray_localize", { labels }),

  // Vue d'ensemble
  systemInfo: () => invoke<SystemInfo>("system_info"),
  monitorStart: (onFrame: (f: MetricsFrame) => void, fps = 60) => {
    const channel = new Channel<MetricsFrame>();
    channel.onmessage = onFrame;
    return invoke<void>("monitor_start", { onFrame: channel, fps });
  },
  monitorStop: () => invoke<void>("monitor_stop"),
  netSpeedtest: (onEvent: (e: SpeedEvent) => void) => {
    const channel = new Channel<SpeedEvent>();
    channel.onmessage = onEvent;
    return invoke<SpeedResult>("net_speedtest", { onEvent: channel });
  },
  restoreStatus: () => invoke<RestoreStatus>("restore_status"),
  restorePoints: () => invoke<RestorePointEntry[]>("restore_points"),
  restoreToPoint: (sequence: number) => invoke<void>("restore_to_point", { sequence }),
  systemRestart: () => invoke<void>("system_restart"),
  restoreEnable: () => invoke<RestoreStatus>("restore_enable"),
  restoreCreate: (description: string) => invoke<RestoreStatus>("restore_create", { description }),
  expressOptimize: () => invoke<ExpressReport>("express_optimize"),
  profileApply: (profile: GuidedProfile) => invoke<TweakReport[]>("profile_apply", { profile }),
  profileTweaks: (profile: GuidedProfile) => invoke<string[]>("profile_tweaks", { profile }),
  profileExport: () => invoke<string>("profile_export"),
  measuresGet: () => invoke<Measures>("measures_get"),
  openLink: (url: string) => invoke<void>("open_link", { url }),
  feedbackAvailable: () => invoke<boolean>("feedback_available"),
  feedbackSend: (message: string, includeInfo: boolean, replyTo: string | null) =>
    invoke<void>("feedback_send", { message, includeInfo, replyTo }),
  win11Status: () => invoke<Win11Status>("win11_status"),
  win11Upgrade: () => invoke<void>("win11_upgrade"),
  win11UpgradeIso: () => invoke<void>("win11_upgrade_iso"),
  autostartGet: () => invoke<boolean>("autostart_get"),
  tweaksRestoreAll: () => invoke<TweakReport[]>("tweaks_restore_all"),
  disksHealth: () => invoke<DiskHealth[]>("disks_health"),
  startupImpacts: () => invoke<StartupImpact[]>("startup_impacts"),
  batteryHealth: () => invoke<BatteryHealth | null>("battery_health"),
  batteryReportOpen: () => invoke<void>("battery_report_open"),
  storageDuplicates: () => invoke<DuplicateGroup[]>("storage_duplicates"),
  autostartSet: (on: boolean) => invoke<boolean>("autostart_set", { on }),
  autoboostGet: () => invoke<boolean>("autoboost_get"),
  autoboostSet: (on: boolean) => invoke<boolean>("autoboost_set", { on }),
  reportSave: (html: string, pdf = false) => invoke<string>("report_save", { html, pdf }),
  maintenanceLast: () => invoke<LastRun | null>("maintenance_last"),
  measureNow: () => invoke<Measures>("measure_now"),
  measureReset: () => invoke<Measures>("measure_reset"),
  profileImport: (json: string) => invoke<TweakReport[]>("profile_import", { json }),

  // Stockage
  storageRules: () => invoke<CleanRule[]>("storage_rules"),
  storageScan: () => invoke<ScanResult[]>("storage_scan"),
  storageBlockers: (ids: string[]) => invoke<RunningApp[]>("storage_blockers", { ids }),
  storageCloseApps: (processes: string[]) => invoke<number>("storage_close_apps", { processes }),
  storageClean: (ids: string[]) => invoke<CleanSummary>("storage_clean", { ids }),
  storageAnalyze: (onProgress: (p: AnalyzerProgress) => void) => {
    const channel = new Channel<AnalyzerProgress>();
    channel.onmessage = onProgress;
    return invoke<LargeFile[]>("storage_analyze", { minBytes: null, onProgress: channel });
  },
  storageTrashFile: (path: string) => invoke<boolean>("storage_trash_file", { path }),
  storageReveal: (path: string) => invoke<void>("storage_reveal", { path }),

  // Performance
  ramPurge: () => invoke<RamReport>("ram_purge"),
  powerSchemes: () => invoke<PowerScheme[]>("power_schemes"),
  powerSetActive: (guid: string) => invoke<boolean>("power_set_active", { guid }),
  displayInfo: () => invoke<DisplayInfo>("display_info"),
  ssdTrim: (letter: string) => invoke<string>("ssd_trim", { letter }),
  servicesRestoreAll: () => invoke<string[]>("services_restore_all"),

  // Réseau
  networkAdapter: (force = false) => invoke<NicSnapshot>("network_adapter", { force }),
  dnsCurrent: () => invoke<DnsConfig>("dns_current"),
  dnsBenchmark: () => invoke<DnsBenchResult[]>("dns_benchmark"),
  dnsApply: (primary: string, secondary: string) => invoke<boolean>("dns_apply", { primary, secondary }),
  dnsReset: () => invoke<boolean>("dns_reset"),
  flushDns: () => invoke<MaintenanceResult>("net_flush_dns"),
  winsockReset: () => invoke<MaintenanceResult>("net_winsock_reset"),

  // Bibliothèque d'applications (winget)
  appsLibrary: () => invoke<LibraryApp[]>("apps_library"),
  appsInstall: (id: string, upgrade: boolean, onEvent: (e: InstallEvent) => void) => {
    const channel = new Channel<InstallEvent>();
    channel.onmessage = onEvent;
    return invoke<boolean>("apps_install", { id, upgrade, onEvent: channel });
  },

  // Jeux
  gamesList: (force = false) => invoke<Game[]>("games_list", { force }),
  gameImage: (id: string) => invoke<string | null>("game_image", { id }),
  gameLaunch: (id: string, options: BoostOptions, onEvent: (e: BoostEvent) => void) => {
    const channel = new Channel<BoostEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("game_launch", { id, options, onEvent: channel });
  },
  gameSession: () => invoke<string | null>("game_session"),
  gameSessionStop: () => invoke<void>("game_session_stop"),

  // Système
  appxList: () => invoke<InstalledApp[]>("appx_list"),
  appxRemove: (id: string) => invoke<boolean>("appx_remove", { id }),
  onedriveStatus: () => invoke<OneDriveStatus>("onedrive_status"),
  onedriveUninstall: () => invoke<boolean>("onedrive_uninstall"),
  edgeStatus: () => invoke<EdgeStatus>("edge_status"),
  edgeUninstall: () => invoke<boolean>("edge_uninstall"),
  startupList: () => invoke<StartupEntry[]>("startup_list"),
  startupSet: (name: string, enabled: boolean) => invoke<boolean>("startup_set", { name, enabled }),
  restartExplorer: () => invoke<void>("restart_explorer"),
  repairRun: (tool: RepairTool, onEvent: (e: RepairEvent) => void) => {
    const channel = new Channel<RepairEvent>();
    channel.onmessage = onEvent;
    return invoke<number | null>("repair_run", { tool, onEvent: channel });
  },
};
