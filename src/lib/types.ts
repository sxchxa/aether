// Types miroirs des structures Rust sérialisées (camelCase).

export type Risk = "safe" | "moderate" | "advanced";

/** Niveau de l'utilisateur (réglages et profils proposés). */
export type UserLevel = "beginner" | "gamer" | "expert";
export type Section = "performance" | "network" | "system";
export type Outcome = "applied" | "alreadyDone" | "preview" | "unavailable";
export type LogLevel = "info" | "success" | "skip" | "preview" | "warn" | "error";
export type GuidedProfile = "daily" | "gamer" | "laptop" | "client" | "silent" | "competitive" | "streaming" | "office" | "creator";
export type CleanCategory = "system" | "browsers" | "graphics";
export type RepairTool = "sfc" | "dism";

export interface AetherError {
  kind: string;
  message: string;
}

export interface Probe {
  applied: boolean;
  available: boolean;
  current: string;
  target: string;
}

export interface TweakStatus {
  id: string;
  section: Section;
  group: string;
  risk: Risk;
  technical: string;
  needsReboot: boolean;
  probe: Probe;
  error: string | null;
}

export interface TweakReport {
  id: string;
  outcome: Outcome | null;
  error: string | null;
  detail: string | null;
}

export interface LogEntry {
  ts: number;
  level: LogLevel;
  module: string;
  message: string;
}

export interface Settings {
  dryRun: boolean;
}

export interface HistoryEntry {
  id: number;
  ts: number;
  target: string;
  label: string;
}

export interface DiskInfo {
  mount: string;
  name: string;
  kind: "ssd" | "hdd" | "unknown";
  fileSystem: string;
  total: number;
  available: number;
  removable: boolean;
}

export interface PhysicalDisk {
  name: string;
  media: string;
  bus: string;
  size: number;
}

export interface BatteryInfo {
  charge: number;
  full: number;
  design: number;
  wearPercent: number | null;
}

export interface DisplayInfo {
  width: number;
  height: number;
  currentHz: number;
  maxHz: number;
  upgradeAvailable: boolean;
}

export interface SystemInfo {
  osName: string;
  osVersion: string;
  hostname: string;
  cpu: string;
  threads: number;
  totalMemory: number;
  uptime: number;
  disks: DiskInfo[];
  physicalDisks: PhysicalDisk[];
  battery: BatteryInfo | null;
  display: DisplayInfo | null;
}

export interface RestorePointEntry {
  sequence: number;
  description: string;
  created: string;
  kind: number;
}

export interface Win11Status {
  isWindows10: boolean;
  build: number;
  tpm: string | null;
  secureBoot: boolean;
  ramOk: boolean;
  freeBytes: number;
  compatible: boolean;
  iso: string | null;
}

export type SpeedPhase = "ping" | "download" | "upload" | "done";

export interface SpeedEvent {
  phase: SpeedPhase;
  mbps: number | null;
}

export interface SpeedResult {
  pingMs: number;
  jitterMs: number;
  downloadMbps: number;
  uploadMbps: number;
}

export interface MetricsFrame {
  ts: number;
  cpu: number;
  cores: number[];
  ramUsed: number;
  ramTotal: number;
}

export interface RestorePoint {
  description: string;
  created: string;
}

export interface RestoreStatus {
  drive: string;
  enabled: boolean;
  blockedByPolicy: boolean;
  lastPoint: RestorePoint | null;
}

export interface Snapshot {
  ramUsed: number;
  junkBytes: number;
  optimized: number;
  total: number;
}

export interface CleanResult {
  id: string;
  freed: number;
  removed: number;
  skipped: number;
}

export interface CleanSummary {
  results: CleanResult[];
  freed: number;
  removed: number;
  skipped: number;
  preview: boolean;
}

export interface ExpressReport {
  before: Snapshot;
  after: Snapshot;
  tweaks: TweakReport[];
  cleaning: CleanSummary;
  preview: boolean;
}

export interface CleanRule {
  id: string;
  category: CleanCategory;
  risk: Risk;
  roots: string[];
  minAgeMinutes: number;
  processes: string[];
  defaultSelected: boolean;
}

export interface ScanResult {
  id: string;
  category?: CleanCategory;
  bytes: number;
  files: number;
  skipped: number;
}

export interface RunningApp {
  process: string;
  instances: number;
}

export interface LargeFile {
  path: string;
  size: number;
  modified: number;
}

export interface AnalyzerProgress {
  filesScanned: number;
  found: number;
}

export interface RamReport {
  usedBefore: number;
  usedAfter: number;
  freed: number;
  trimmed: number;
  total: number;
}

export interface PowerScheme {
  guid: string;
  name: string;
  active: boolean;
}

export interface Adapter {
  name: string;
  description: string;
  index: number;
  linkSpeed: string;
  mac: string;
  guid: string;
  media: string;
}

export interface NicProperty {
  keyword: string;
  value: string;
  displayName: string;
  displayValue: string;
}

export interface NicSnapshot {
  adapter: Adapter | null;
  properties: NicProperty[];
}

export interface DnsConfig {
  interfaceIndex: number;
  interfaceName: string;
  servers: string[];
  dhcp: boolean;
}

export interface DnsBenchResult {
  id: string;
  name: string;
  primary: string;
  secondary: string;
  avgMs: number | null;
  minMs: number | null;
  success: number;
  attempts: number;
}

export interface MaintenanceResult {
  executed: boolean;
  needsReboot: boolean;
  output: string;
}

export interface InstalledApp {
  id: string;
  package: string;
  version: string;
}

export interface OneDriveStatus {
  installed: boolean;
  running: boolean;
  uninstaller: string | null;
}

export interface EdgeStatus {
  browserInstalled: boolean;
  uninstallAllowed: boolean;
  webview2Installed: boolean;
  webview2Version: string | null;
}

export interface StartupEntry {
  name: string;
  command: string;
  enabled: boolean;
}

export type AppCategory = "tools" | "browsers" | "chat" | "media" | "gaming" | "office" | "dev";

export interface LibraryApp {
  id: string;
  name: string;
  category: AppCategory;
  installed: boolean;
}

export type InstallEvent = { type: "progress"; percent: number } | { type: "line"; text: string };

export interface Game {
  id: string;
  name: string;
  source: "steam" | "epic" | "gog";
  installDir: string;
  hasImage: boolean;
}

export interface BoostOptions {
  purgeRam: boolean;
  flushDns: boolean;
  performancePlan: boolean;
  highPriority: boolean;
  noThrottling: boolean;
}

export type BoostStep = "ram" | "dns" | "plan" | "launch" | "detect" | "priority" | "session" | "restore";

export interface BoostEvent {
  step: BoostStep;
  status: "running" | "done" | "skipped" | "error";
  detail: string;
}

export type RepairEvent = { type: "line"; text: string } | { type: "done"; code: number | null };

export interface Measure {
  ts: number;
  bootMs: number | null;
  ramUsed: number;
  ramTotal: number;
  processes: number;
  servicesRunning: number;
  startupItems: number;
}

export interface Measures {
  reference: Measure | null;
  history: Measure[];
}

export interface LastRun {
  ts: number;
  freed: number;
  removed: number;
}

export type DiskVerdict = "good" | "warning" | "critical" | "unknown";

export interface DiskHealth {
  name: string;
  media: string;
  bus: string;
  size: number;
  verdict: DiskVerdict;
  temperature: number | null;
  wear: number | null;
  powerOnHours: number | null;
  uncorrectedErrors: number;
}

export interface StartupImpact {
  file: string;
  degradationMs: number;
}

export interface BatteryHealth {
  designMwh: number;
  fullMwh: number;
  cycles: number | null;
  wearPercent: number;
}

export interface DuplicateGroup {
  size: number;
  files: { path: string; modified: number }[];
  wasted: number;
}
