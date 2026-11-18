// Section « Réglages » : préférences générales, profils de configuration, communauté (Discord)
// et informations sur le logiciel.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check } from "@tauri-apps/plugin-updater";
import {
  Download,
  ExternalLink,
  Globe,
  Contrast,
  Keyboard,
  MessagesSquare,
  Palette,
  Power,
  Undo2,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Upload,
} from "lucide-react";
import { LevelSegmented } from "../../components/LevelPicker";
import { RestorePointsCard } from "./RestorePointsCard";
import { FeedbackCard } from "./FeedbackCard";
import { ConfirmDialog } from "../../components/Overlays";
import { Button, Card, CardHeader, cx, PageHeader, Toggle } from "../../components/ui";
import { fmt, LANGUAGES, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { LINKS } from "../../lib/links";
import { autoRestoreEnabled, setAutoRestoreEnabled } from "../../lib/safety";
import { getHighContrast, getThemePref, setHighContrast, setThemePref, type ThemePref } from "../../lib/theme";
import { useApp } from "../../store/app";

/** Ligne de réglage : icône discrète, titre, description secondaire et contrôle à droite. */
function Row({ icon, title, desc, children }: { icon: ReactNode; title: string; desc?: string; children?: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-3 px-6 py-4">
      <span className="text-[var(--steel)]">{icon}</span>
      <div className="min-w-0 flex-1 basis-[220px]">
        <div className="text-[14px] text-zinc-100">{title}</div>
        {desc && <div className="mt-0.5 text-[12.5px] text-zinc-500">{desc}</div>}
      </div>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </div>
  );
}

export function SettingsPage() {
  const { m, lang, setLanguage } = useI18n();
  const replayOnboarding = useApp((s) => s.replayOnboarding);
  const loadTweaks = useApp((s) => s.loadTweaks);
  const toast = useApp((s) => s.toast);
  const startupMs = useApp((s) => s.startupMs);
  const [busy, setBusy] = useState<string | null>(null);
  const [version, setVersion] = useState("");
  const [autostart, setAutostart] = useState(false);
  const [confirmRestore, setConfirmRestore] = useState(false);
  const loadTweaks2 = useApp((s) => s.loadTweaks);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const [autoRestore, setAutoRestore] = useState(autoRestoreEnabled);
  const [theme, setTheme] = useState<ThemePref>(getThemePref);
  const [contrast, setContrast] = useState(getHighContrast);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => undefined);
    void api
      .autostartGet()
      .then(setAutostart)
      .catch(() => undefined);
  }, []);

  const run = async (id: string, fn: () => Promise<void>) => {
    setBusy(id);
    try {
      await fn();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const open = (url: string) => void api.openLink(url).catch((e: unknown) => toast("error", errorMessage(e)));

  return (
    <div className="space-y-10">
      <PageHeader section="settings" title={m.settings.title} subtitle={m.settings.subtitle} />

      <Card id="set-general">
        <CardHeader title={m.settings.general} />
        <div className="divide-y divide-white/[0.05] border-t border-white/[0.05]">
          <Row icon={<Globe className="size-[18px]" strokeWidth={1.6} />} title={m.settings.language}>
            <div
              className="flex flex-wrap gap-1 rounded-[12px] border border-white/[0.06] bg-white/[0.02] p-1"
              role="radiogroup"
              aria-label={m.settings.language}
            >
              {LANGUAGES.map((l) => (
                <button
                  key={l.code}
                  type="button"
                  role="radio"
                  aria-checked={l.code === lang}
                  title={l.label}
                  onClick={() => setLanguage(l.code)}
                  className={cx(
                    "h-8 rounded-[9px] px-3 text-[12.5px] transition-colors",
                    l.code === lang ? "bg-white/[0.08] text-white" : "text-zinc-400 hover:text-zinc-100",
                  )}
                >
                  {l.short}
                </button>
              ))}
            </div>
          </Row>
          <Row icon={<SlidersHorizontal className="size-[18px]" strokeWidth={1.6} />} title={m.level.title} desc={m.level.settingsDesc}>
            <div className="w-[300px] max-w-full">
              <LevelSegmented />
            </div>
          </Row>
          <Row icon={<Power className="size-[18px]" strokeWidth={1.6} />} title={m.settings.autostart} desc={m.settings.autostartDesc}>
            <Toggle
              checked={autostart}
              busy={busy === "autostart"}
              label={m.settings.autostart}
              onChange={(v) => void run("autostart", async () => setAutostart(await api.autostartSet(v)))}
            />
          </Row>
          <Row
            icon={<ShieldCheck className="size-[18px]" strokeWidth={1.6} />}
            title={m.settings.autoRestore}
            desc={m.settings.autoRestoreDesc}
          >
            <Toggle
              checked={autoRestore}
              label={m.settings.autoRestore}
              onChange={(v) => {
                setAutoRestoreEnabled(v);
                setAutoRestore(v);
              }}
            />
          </Row>
          <Row icon={<Sparkles className="size-[18px]" strokeWidth={1.6} />} title={m.settings.onboarding} desc={m.settings.onboardingDesc}>
            <Button size="sm" onClick={replayOnboarding}>
              {m.settings.onboardingAction}
            </Button>
          </Row>
          <Row
            icon={<RefreshCw className="size-[18px]" strokeWidth={1.6} />}
            title={m.settings.updates}
            desc={version ? fmt(m.settings.versionLine, { version }) : undefined}
          >
            <Button
              size="sm"
              loading={busy === "update"}
              onClick={() =>
                void run("update", async () => {
                  const update = await check();
                  if (!update) {
                    toast("info", m.palette.upToDate);
                    return;
                  }
                  toast("info", fmt(m.palette.updateAvailable, { version: update.version }));
                  await update.downloadAndInstall();
                })
              }
            >
              {m.palette.checkUpdates}
            </Button>
          </Row>
        </div>
      </Card>

      <Card id="set-appearance">
        <CardHeader title={m.settings.appearance} />
        <div className="divide-y divide-white/[0.05] border-t border-white/[0.05]">
          <Row icon={<Palette className="size-[18px]" strokeWidth={1.6} />} title={m.settings.theme}>
            <div
              className="flex flex-wrap gap-1 rounded-[12px] border border-white/[0.06] bg-white/[0.02] p-1"
              role="radiogroup"
              aria-label={m.settings.theme}
            >
              {(["dark", "light", "system"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  role="radio"
                  aria-checked={theme === t}
                  onClick={() => {
                    setThemePref(t);
                    setTheme(t);
                  }}
                  className={cx(
                    "h-8 rounded-[9px] px-3 text-[12.5px] transition-colors",
                    theme === t ? "bg-white/[0.08] text-white" : "text-zinc-400 hover:text-zinc-100",
                  )}
                >
                  {m.settings.themes[t]}
                </button>
              ))}
            </div>
          </Row>
          <Row icon={<Contrast className="size-[18px]" strokeWidth={1.6} />} title={m.settings.contrast} desc={m.settings.contrastDesc}>
            <Toggle
              checked={contrast}
              label={m.settings.contrast}
              onChange={(v) => {
                setHighContrast(v);
                setContrast(v);
              }}
            />
          </Row>
          <Row icon={<Keyboard className="size-[18px]" strokeWidth={1.6} />} title={m.settings.shortcuts} desc={m.settings.shortcutsDesc} />
        </div>
      </Card>

      <Card id="set-profiles">
        <CardHeader title={m.settings.profiles} />
        <div className="divide-y divide-white/[0.05] border-t border-white/[0.05]">
          <Row icon={<Download className="size-[18px]" strokeWidth={1.6} />} title={m.settings.configTitle} desc={m.settings.configDesc}>
            <input
              ref={fileRef}
              type="file"
              accept=".json,application/json"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                e.target.value = "";
                if (!file) return;
                void run("import", async () => {
                  const reports = await api.profileImport(await file.text());
                  toast("success", fmt(m.toast.imported, { count: reports.filter((r) => r.outcome === "applied").length }));
                  await loadTweaks();
                });
              }}
            />
            <Button size="sm" icon={<Upload className="size-3.5" />} loading={busy === "import"} onClick={() => fileRef.current?.click()}>
              {m.overview.importProfile}
            </Button>
            <Button
              size="sm"
              icon={<Download className="size-3.5" />}
              loading={busy === "export"}
              onClick={() => void run("export", async () => toast("success", fmt(m.toast.exported, { path: await api.profileExport() })))}
            >
              {m.overview.exportProfile}
            </Button>
          </Row>
          <Row
            icon={<Sparkles className="size-[18px]" strokeWidth={1.6} />}
            title={m.settings.customProfiles}
            desc={m.settings.customProfilesDesc}
          >
            <span className="rounded-full border border-white/[0.08] px-2.5 py-1 text-[11.5px] text-zinc-400">{m.settings.soon}</span>
          </Row>
        </div>
      </Card>

      <RestorePointsCard />

      <FeedbackCard />

      <Card id="set-safety">
        <CardHeader title={m.settings.safety} />
        <div className="border-t border-white/[0.05]">
          <Row icon={<Undo2 className="size-[18px]" strokeWidth={1.6} />} title={m.settings.restoreAll} desc={m.settings.restoreAllDesc}>
            <Button size="sm" variant="danger" loading={busy === "restoreAll"} onClick={() => setConfirmRestore(true)}>
              {m.settings.restoreAllAction}
            </Button>
          </Row>
        </div>
      </Card>

      <ConfirmDialog
        open={confirmRestore}
        title={m.settings.restoreAll}
        body={m.settings.restoreAllConfirm}
        confirmLabel={m.settings.restoreAllAction}
        danger
        onClose={() => setConfirmRestore(false)}
        onConfirm={() =>
          void run("restoreAll", async () => {
            const reports = await api.tweaksRestoreAll();
            const done = reports.filter((r) => r.outcome === "applied").length;
            const failed = reports.filter((r) => r.error).length;
            toast("success", fmt(m.settings.restoreAllDone, { count: done }));
            if (failed > 0) toast("error", fmt(m.settings.restoreAllFailed, { failed }));
            await loadTweaks2();
            await refreshHistory();
          })
        }
      />

      <Card id="set-community">
        <CardHeader title={m.settings.community} />
        <div className="border-t border-white/[0.05]">
          <Row icon={<MessagesSquare className="size-[18px]" strokeWidth={1.6} />} title="Discord" desc={m.settings.discordDesc}>
            {LINKS.discord ? (
              <Button size="sm" icon={<ExternalLink className="size-3.5" />} onClick={() => open(LINKS.discord)}>
                {m.settings.discordAction}
              </Button>
            ) : (
              <span className="rounded-full border border-white/[0.08] px-2.5 py-1 text-[11.5px] text-zinc-400">{m.settings.soon}</span>
            )}
          </Row>
        </div>
      </Card>

      <p className="pb-2 text-center text-[12px] text-zinc-600">
        Aether {version && `v${version}`} · {m.settings.about}
        {startupMs !== null &&
          ` · ${fmt(m.settings.startupTime, { time: `${(startupMs / 1000).toLocaleString(lang, { maximumFractionDigits: 1 })} s` })}`}
      </p>
    </div>
  );
}
