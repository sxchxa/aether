// Modèle de navigation : groupes, sections, sous-sections (ancres de cartes) et étapes de workflow.
import type { LucideIcon } from "lucide-react";
import {
  Activity,
  LifeBuoy,
  AppWindow,
  BatteryCharging,
  Brush,
  Cable,
  Cloud,
  Gamepad2,
  Gauge,
  Globe,
  HardDrive,
  HardDriveDownload,
  LayoutDashboard,
  ListTree,
  MemoryStick,
  Network,
  Power,
  Rocket,
  Server,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Stethoscope,
  UserLock,
  Users,
  Wrench,
  Zap,
} from "lucide-react";
import type { Tone } from "../components/ui";
import type { Messages } from "../i18n";
import type { SectionId } from "../store/app";

export const POWER_SETTINGS = [
  "power.throttling",
  "power.cpuMin",
  "power.usbSuspend",
  "power.pcieAspm",
  "power.diskIdle",
  "power.hibernate",
];

export const SECTION_TWEAKS: Record<SectionId, string[]> = {
  settings: [],
  help: [],
  overview: [],
  games: [],
  apps: [],
  storage: [],
  performance: [
    "svc.spooler",
    "svc.diagtrack",
    "svc.wsearch",
    "svc.sysmain",
    "svc.fax",
    "gaming.gameMode",
    "gaming.windowedOpt",
    "gaming.mouse",
    "gaming.xboxDvr",
    "gaming.stickyKeys",
    "gaming.hags",
    "gaming.mmcss",
    "mem.svchostSplit",
    "gaming.gameBarButton",
    "gaming.broadcast",
    "gaming.fsoOff",
    "svc.mapsBroker",
    "ui.menuDelay",
    "ui.endTask",
    "ui.fileExtensions",
    "ui.transparency",
    "ui.startupDelay",
    "ui.darkMode",
    "ui.aeroShake",
    "ui.hiddenFiles",
    "ui.launchThisPc",
    "ui.verboseStatus",
    "fs.storageSense",
    "fs.lastAccess",
    "fx.minMax",
    "fx.controls",
    "fx.menus",
    "fx.tooltips",
    "fx.combo",
    "fx.selectionFade",
    "fx.dragContents",
    "fx.windowShadow",
    "fx.cursorShadow",
    "fx.taskbarAnim",
    "fx.peek",
    "fx.iconShadow",
    "fx.selectionRect",
    "fx.thumbnails",
  ],
  power: ["power.ultimate", ...POWER_SETTINGS],
  network: [
    "nic.powerSaving",
    "nic.wakeMagic",
    "nic.wakePattern",
    "nic.eee",
    "nic.interruptModeration",
    "tcp.throttling",
    "net.deliveryOptimization",
    "net.doh",
    "net.llmnr",
    "net.smb1",
  ],
  system: [
    "ui.classicMenu",
    "edge.neutralize",
    "privacy.advertisingId",
    "privacy.tips",
    "privacy.widgets",
    "privacy.recall",
    "privacy.backgroundApps",
    "privacy.bingSearch",
    "privacy.copilot",
    "privacy.activity",
    "privacy.telemetry",
    "privacy.startRecommendations",
    "privacy.feedback",
    "privacy.tailored",
    "privacy.appTracking",
    "privacy.inking",
    "privacy.onlineSpeech",
    "wu.restartNotify",
    "wu.activeHours",
    "wu.noDrivers",
    "wu.deferFeature",
  ],
};

/** Réglages de goût personnel : proposés, jamais appliqués par « Tout corriger » ni comptés dans le score. */
export const PREFERENCE_TWEAKS: ReadonlySet<string> = new Set([
  "ui.endTask",
  "ui.darkMode",
  "ui.hiddenFiles",
  "ui.launchThisPc",
  "ui.aeroShake",
  "ui.verboseStatus",
  "privacy.onlineSpeech",
  "maint.weekly",
]);

/** Indique si une option doit rester un choix manuel (effets visuels et préférences). */
export function isPreference(t: { id: string; group: string }): boolean {
  return t.group === "fx" || PREFERENCE_TWEAKS.has(t.id);
}

export interface SubSection {
  anchor: string;
  icon: LucideIcon;
  label: (m: Messages) => string;
}

export interface SectionDef {
  id: SectionId;
  /** Groupe du menu (« settings » : hors menu, ouvert par la roue crantée). */
  group: keyof Messages["navGroups"] | "settings";
  icon: LucideIcon;
  tone: Tone;
  steps: (keyof Messages["workflow"])[];
  subs: SubSection[];
}

export const SECTION_DEFS: SectionDef[] = [
  {
    id: "overview",
    group: "main",
    icon: LayoutDashboard,
    tone: "sky",
    steps: ["measure", "protect", "apply"],
    subs: [
      { anchor: "ov-health", icon: Activity, label: (m) => m.overview.health },
      { anchor: "ov-express", icon: Sparkles, label: (m) => m.scan.title },
      { anchor: "ov-live", icon: Gauge, label: (m) => m.overview.cpu },
      { anchor: "ov-protection", icon: ShieldCheck, label: (m) => m.overview.protection },
      { anchor: "ov-profiles", icon: Users, label: (m) => m.overview.profiles },
    ],
  },
  {
    id: "games",
    group: "main",
    icon: Gamepad2,
    tone: "violet",
    steps: ["scan", "apply", "measure"],
    subs: [
      { anchor: "gm-boost", icon: Rocket, label: (m) => m.games.boost },
      { anchor: "gm-library", icon: Gamepad2, label: (m) => m.games.library },
    ],
  },
  {
    id: "apps",
    group: "main",
    icon: AppWindow,
    tone: "sky",
    steps: ["scan", "apply", "measure"],
    subs: [],
  },
  {
    id: "storage",
    group: "optimize",
    icon: HardDrive,
    tone: "violet",
    steps: ["scan", "review", "apply"],
    subs: [
      { anchor: "st-system", icon: Brush, label: (m) => m.storage.categories.system },
      { anchor: "st-browsers", icon: Globe, label: (m) => m.storage.categories.browsers },
      { anchor: "st-graphics", icon: Gamepad2, label: (m) => m.storage.categories.graphics },
      { anchor: "st-analyzer", icon: HardDriveDownload, label: (m) => m.storage.analyzer },
    ],
  },
  {
    id: "performance",
    group: "optimize",
    icon: Gauge,
    tone: "indigo",
    steps: ["measure", "review", "apply"],
    subs: [
      { anchor: "pf-services", icon: Server, label: (m) => m.performance.services },
      { anchor: "pf-gaming", icon: Gamepad2, label: (m) => m.performance.gaming },
      { anchor: "pf-ram", icon: MemoryStick, label: (m) => m.performance.ram },
      { anchor: "pf-trim", icon: HardDriveDownload, label: (m) => m.performance.trim },
    ],
  },
  {
    id: "power",
    group: "optimize",
    icon: BatteryCharging,
    tone: "amber",
    steps: ["measure", "review", "apply"],
    subs: [
      { anchor: "pw-max", icon: Zap, label: (m) => m.power.maxTitle },
      { anchor: "pw-plans", icon: Power, label: (m) => m.power.plans },
      { anchor: "pw-settings", icon: SlidersHorizontal, label: (m) => m.power.settings },
    ],
  },
  {
    id: "network",
    group: "optimize",
    icon: Network,
    tone: "emerald",
    steps: ["measure", "review", "apply"],
    subs: [
      { anchor: "nw-nic", icon: Cable, label: (m) => m.network.nic },
      { anchor: "nw-tcp", icon: Gauge, label: (m) => m.network.tcp },
      { anchor: "nw-maintenance", icon: Wrench, label: (m) => m.network.maintenance },
      { anchor: "nw-dns", icon: Rocket, label: (m) => m.network.dns },
    ],
  },
  {
    id: "system",
    group: "maintain",
    icon: Wrench,
    tone: "rose",
    steps: ["review", "protect", "repair"],
    subs: [
      { anchor: "sy-apps", icon: AppWindow, label: (m) => m.system.apps },
      { anchor: "sy-startup", icon: Power, label: (m) => m.system.startup },
      { anchor: "sy-privacy", icon: UserLock, label: (m) => m.system.privacy },
      { anchor: "sy-menu", icon: ListTree, label: (m) => m.system.contextMenu },
      { anchor: "sy-onedrive", icon: Cloud, label: (m) => m.system.onedrive },
      { anchor: "sy-edge", icon: Globe, label: (m) => m.system.edge },
      { anchor: "sy-repair", icon: Stethoscope, label: (m) => m.system.repair },
    ],
  },
];

SECTION_DEFS.push({ id: "settings", group: "settings", icon: Settings, tone: "slate", steps: ["review"], subs: [] });
SECTION_DEFS.push({ id: "help", group: "settings", icon: LifeBuoy, tone: "slate", steps: ["review"], subs: [] });

export function sectionDef(id: SectionId): SectionDef {
  const def = SECTION_DEFS.find((s) => s.id === id);
  if (!def) throw new Error(`section inconnue : ${id}`);
  return def;
}
