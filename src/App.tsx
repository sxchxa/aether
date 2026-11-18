// Coquille de l'application : barre de titre, navigation, section active et superpositions.
import { memo, useEffect, useLayoutEffect, useRef, useState } from "react";
import { PageActiveContext } from "./lib/pageActive";
import { hideSplash, preloadAll } from "./lib/preload";
import { listen } from "@tauri-apps/api/event";
import { CommandPalette } from "./components/CommandPalette";
import { Inspector } from "./components/Inspector";
import { LogDrawer, Toasts } from "./components/Overlays";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { UpdatePrompt } from "./components/UpdatePrompt";
import { AppsPage } from "./features/apps/AppsPage";
import { GamesPage } from "./features/games/GamesPage";
import { NetworkPage } from "./features/network/NetworkPage";
import { Onboarding } from "./features/onboarding/Onboarding";
import { OverviewPage } from "./features/overview/OverviewPage";
import { PerformancePage } from "./features/performance/PerformancePage";
import { PowerPage } from "./features/power/PowerPage";
import { StoragePage } from "./features/storage/StoragePage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { HelpPage } from "./features/help/HelpPage";
import { SystemPage } from "./features/system/SystemPage";
import { useI18n } from "./i18n";
import { api } from "./lib/ipc";
import type { LogEntry } from "./lib/types";
import { useApp, type SectionId } from "./store/app";
import { useMetrics } from "./store/metrics";

const SECTION_ORDER: SectionId[] = [
  "overview",
  "games",
  "apps",
  "storage",
  "performance",
  "power",
  "network",
  "system",
  "settings",
  "help",
];

// Pages mémorisées : changer de menu ne redessine pas les pages masquées.
const PAGES: Record<SectionId, React.MemoExoticComponent<() => React.JSX.Element>> = {
  overview: memo(OverviewPage),
  games: memo(GamesPage),
  apps: memo(AppsPage),
  storage: memo(StoragePage),
  performance: memo(PerformancePage),
  power: memo(PowerPage),
  network: memo(NetworkPage),
  system: memo(SystemPage),
  settings: memo(SettingsPage),
  help: memo(HelpPage),
};

export default function App() {
  const { m, lang } = useI18n();
  const section = useApp((s) => s.section);
  const onboarded = useApp((s) => s.onboarded);
  const inspector = useApp((s) => s.inspector);

  useEffect(() => {
    const s = useApp.getState();
    // Préchargement unique pendant l'écran de chargement, partagé ensuite par toutes les pages.
    void preloadAll().then(hideSplash);
    useMetrics.getState().start();
    void api
      .logsGet()
      .then(s.setLogs)
      .catch(() => undefined);
    const unlisteners: Promise<() => void>[] = [
      listen<LogEntry>("aether://log", (e) => useApp.getState().pushLog(e.payload)),
      listen<string>("aether://tray", (e) => {
        const st = useApp.getState();
        void st.refreshHistory();
        void st.loadTweaks(["power.ultimate"]);
        const msg = messagesRef.current;
        if (msg) st.toast("success", e.payload === "purge" ? msg.toast.trayPurge : msg.toast.trayPerformance);
      }),
    ];
    return () => {
      for (const u of unlisteners) void u.then((fn) => fn());
      useMetrics.getState().stop();
    };
  }, []);

  // Garde les messages courants accessibles aux écouteurs d'événements.
  messagesRef.current = m;

  useEffect(() => {
    document.documentElement.lang = lang;
    void api.trayLocalize([m.tray.purge, m.tray.performance, m.tray.open, m.tray.quit]).catch(() => undefined);
  }, [lang, m]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && useApp.getState().inspector) useApp.getState().inspect(null);
      // Navigation au clavier : Alt+1…8 pour les sections, Ctrl+, pour les Réglages.
      if (e.altKey && !e.ctrlKey && /^[1-8]$/.test(e.key)) {
        const target = SECTION_ORDER[Number(e.key) - 1];
        if (target) {
          e.preventDefault();
          useApp.getState().goTo(target);
        }
      }
      if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        useApp.getState().goTo("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Pages conservées en mémoire après leur première visite : changer de menu ne reconstruit
  // plus rien, la page est simplement réaffichée (fondu d'opacité court, sans recalcul).
  const [visited, setVisited] = useState<SectionId[]>([section]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const scrollMemory = useRef<Partial<Record<SectionId, number>>>({});
  const previous = useRef<SectionId>(section);

  useLayoutEffect(() => {
    setVisited((v) => (v.includes(section) ? v : [...v, section]));
    const el = scrollRef.current;
    if (el && previous.current !== section) {
      scrollMemory.current[previous.current] = el.scrollTop;
      el.scrollTop = scrollMemory.current[section] ?? 0;
      previous.current = section;
    }
  }, [section]);

  return (
    <div className="aether-backdrop flex h-full flex-col">
      <div className="relative flex min-h-0 flex-1">
        {!onboarded && <Onboarding />}
        {onboarded && <UpdatePrompt />}
        <Sidebar />
        <main className="relative min-w-0 flex-1 overflow-hidden">
          <svg
            className="pointer-events-none absolute inset-x-0 top-0 h-[560px] w-full"
            viewBox="0 0 1600 560"
            preserveAspectRatio="none"
            aria-hidden
          >
            <defs>
              <linearGradient id="aether-sheen" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0" stopColor="#9fb4d4" stopOpacity="0.075" />
                <stop offset="1" stopColor="#9fb4d4" stopOpacity="0" />
              </linearGradient>
            </defs>
            <path d="M640 0C860 90 940 250 1160 330C1330 392 1480 400 1600 420L1600 0Z" fill="url(#aether-sheen)" />
            <path d="M900 0C1040 70 1100 170 1260 230C1390 280 1500 290 1600 300L1600 0Z" fill="url(#aether-sheen)" />
          </svg>
          <TitleBar />
          <div
            id="aether-scroll"
            ref={scrollRef}
            className={`scroll-area scroll-fade-top h-full px-6 pt-14 pb-24 md:px-12 xl:pt-16 ${inspector ? "lg:pr-[380px]" : ""}`}
          >
            <div className="mx-auto max-w-[1180px]">
              {SECTION_ORDER.filter((id) => visited.includes(id)).map((id) => {
                const Page = PAGES[id];
                const active = id === section;
                return (
                  <PageActiveContext.Provider key={id} value={active}>
                    <div hidden={!active} className={active ? "page-enter" : undefined}>
                      <Page />
                    </div>
                  </PageActiveContext.Provider>
                );
              })}
            </div>
          </div>
          <Inspector />
          <LogDrawer />
        </main>
      </div>
      <CommandPalette />
      <Toasts />
    </div>
  );
}

const messagesRef: { current: ReturnType<typeof useI18n>["m"] | null } = { current: null };
