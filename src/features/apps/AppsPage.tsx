// Section « Applications » : grands classiques installés en un clic via winget.
import { useEffect, useMemo, useState } from "react";
import { Check, Download, LoaderCircle, RefreshCw, Search } from "lucide-react";
import { Button, Card, cx, Empty, PageHeader } from "../../components/ui";
import { fmt, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { AppCategory, LibraryApp } from "../../lib/types";
import { useApp } from "../../store/app";
import { appLogo } from "./logos";

const CATEGORIES: AppCategory[] = ["tools", "browsers", "chat", "media", "gaming", "office", "dev"];

/** Pastille avec le logo de marque, ou l'initiale si aucun logo n'existe. */
function AppIcon({ id, name }: { id: string; name: string }) {
  const logo = appLogo(id);
  if (!logo) {
    return (
      <span
        className="grid size-9 shrink-0 place-items-center rounded-[9px] bg-white/[0.05] text-[13px] font-semibold text-zinc-300"
        aria-hidden
      >
        {name.slice(0, 1)}
      </span>
    );
  }
  return (
    <span
      className={cx("grid size-9 shrink-0 place-items-center rounded-[9px]", logo.light ? "bg-zinc-200" : "bg-white/[0.04]")}
      aria-hidden
    >
      <img
        src={logo.src}
        alt=""
        width={24}
        height={24}
        className="size-6 object-contain"
        draggable={false}
        loading="lazy"
        decoding="async"
      />
    </span>
  );
}

interface Running {
  id: string;
  percent: number | null;
  status: string;
}

export function AppsPage() {
  const { m } = useI18n();
  const library = useApp((s) => s.library);
  const libraryError = useApp((s) => s.libraryError);
  const loadLibrary = useApp((s) => s.loadLibrary);
  const markInstalled = useApp((s) => s.markInstalled);
  const toast = useApp((s) => s.toast);
  const dryRun = useApp((s) => s.dryRun);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<AppCategory | "all">("all");
  const [running, setRunning] = useState<Running | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    void loadLibrary();
  }, [loadLibrary]);

  const refresh = async () => {
    setRefreshing(true);
    await loadLibrary(true);
    setRefreshing(false);
  };

  const install = async (app: LibraryApp, upgrade: boolean) => {
    setRunning({ id: app.id, percent: null, status: m.library.installing });
    try {
      const done = await api.appsInstall(app.id, upgrade, (e) =>
        setRunning((r) => (r ? (e.type === "progress" ? { ...r, percent: e.percent } : { ...r, status: e.text }) : r)),
      );
      if (dryRun) toast("warn", m.dryRun.simulated);
      else if (done) {
        markInstalled(app.id);
        toast("success", fmt(m.library.done, { name: app.name }));
      } else toast("info", fmt(m.library.upToDate, { name: app.name }));
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setRunning(null);
    }
  };

  const visible = useMemo(
    () =>
      (library ?? []).filter(
        (a) => (category === "all" || a.category === category) && a.name.toLowerCase().includes(query.trim().toLowerCase()),
      ),
    [library, category, query],
  );

  return (
    <div className="space-y-8">
      <PageHeader
        section="apps"
        title={m.library.title}
        subtitle={m.library.subtitle}
        actions={
          <Button
            size="sm"
            icon={<RefreshCw className={cx("size-3.5", refreshing && "animate-spin")} />}
            disabled={refreshing}
            onClick={() => void refresh()}
          >
            {m.games.refresh}
          </Button>
        }
      />

      <div className="flex flex-wrap items-center gap-3">
        <div className="flex h-9 min-w-[200px] flex-1 items-center gap-2 rounded-[8px] border border-white/[0.08] bg-white/[0.02] px-3">
          <Search className="size-4 text-zinc-500" aria-hidden />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={m.library.search}
            className="min-w-0 flex-1 bg-transparent text-[13px] text-white outline-none placeholder:text-zinc-500"
          />
        </div>
        <div className="flex flex-wrap gap-1" role="tablist">
          {(["all", ...CATEGORIES] as const).map((c) => (
            <button
              key={c}
              type="button"
              role="tab"
              aria-selected={category === c}
              onClick={() => setCategory(c)}
              className={cx(
                "h-8 rounded-[8px] px-3 text-[12.5px] transition-colors",
                category === c ? "bg-white/[0.08] text-white" : "text-zinc-400 hover:text-zinc-200",
              )}
            >
              {c === "all" ? m.library.all : m.library.categories[c]}
            </button>
          ))}
        </div>
      </div>

      {library === null && <Empty>{m.common.loading}</Empty>}
      {libraryError && <Empty>{m.library.wingetMissing}</Empty>}
      {/* Regroupement par catégorie, dans l'ordre d'utilité (le catalogue est déjà trié). */}
      {CATEGORIES.map((c) => ({ c, apps: visible.filter((a) => a.category === c) }))
        .filter((g) => g.apps.length > 0)
        .map((g) => (
          <section key={g.c} className="space-y-3">
            {category === "all" && <h2 className="px-1 text-[13px] font-medium text-zinc-400">{m.library.categories[g.c]}</h2>}
            <Card>
              <ul className="divide-y divide-white/[0.05]">
                {g.apps.map((a) => {
                  const busy = running?.id === a.id;
                  return (
                    <li key={a.id} className="cv-row flex min-w-0 items-center gap-4 px-5 py-3">
                      <AppIcon id={a.id} name={a.name} />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-[14px] text-zinc-100">{a.name}</div>
                        {busy && (
                          <div className="truncate text-[12px] text-zinc-500">
                            {running?.percent !== null ? `${running?.percent} %` : running?.status}
                          </div>
                        )}
                      </div>
                      {busy ? (
                        <LoaderCircle className="size-4 shrink-0 animate-spin text-zinc-400" aria-hidden />
                      ) : a.installed ? (
                        <div className="flex shrink-0 items-center gap-3">
                          <span className="flex items-center gap-1 text-[12.5px] text-emerald-300">
                            <Check className="size-3.5" aria-hidden />
                            {m.library.installed}
                          </span>
                          <Button size="sm" variant="ghost" disabled={running !== null} onClick={() => void install(a, true)}>
                            {m.library.update}
                          </Button>
                        </div>
                      ) : (
                        <Button
                          size="sm"
                          disabled={running !== null}
                          icon={<Download className="size-3.5" />}
                          onClick={() => void install(a, false)}
                        >
                          {m.library.install}
                        </Button>
                      )}
                    </li>
                  );
                })}
              </ul>
            </Card>
          </section>
        ))}
    </div>
  );
}
