// Palette de commandes modale (Ctrl+K).
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { check } from "@tauri-apps/plugin-updater";
import {
  CornerDownLeft,
  Download,
  MemoryStick,
  RefreshCw,
  ScrollText,
  Search,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Undo2,
} from "lucide-react";
import { fmt, useI18n } from "../i18n";
import { api, errorMessage } from "../lib/ipc";
import { formatBytes } from "../lib/format";
import { useApp } from "../store/app";
import { SECTIONS } from "./Sidebar";
import { useUndo } from "./TitleBar";
import { cx } from "./ui";

interface Command {
  id: string;
  group: string;
  label: string;
  icon: ReactNode;
  run: () => void | Promise<void>;
}

export function CommandPalette() {
  const { m, lang } = useI18n();
  const open = useApp((s) => s.paletteOpen);
  const setOpen = useApp((s) => s.setPaletteOpen);
  const setSection = useApp((s) => s.setSection);
  const expert = useApp((s) => s.expert);
  const setExpert = useApp((s) => s.setExpert);
  const setLogsOpen = useApp((s) => s.setLogsOpen);
  const toast = useApp((s) => s.toast);
  const undo = useUndo();
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(!useApp.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setOpen]);

  useEffect(() => {
    if (open) {
      setQuery("");
      setIndex(0);
      window.setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [open]);

  const commands = useMemo<Command[]>(() => {
    const safe = (fn: () => Promise<void>) => async () => {
      try {
        await fn();
      } catch (e) {
        toast("error", errorMessage(e));
      }
    };
    return [
      ...SECTIONS.map(({ id, icon: Icon }) => ({
        id: `nav-${id}`,
        group: m.palette.navigation,
        label: `${m.palette.go} ${m.nav[id]}`,
        icon: <Icon className="size-4" />,
        run: () => setSection(id),
      })),
      {
        id: "express",
        group: m.palette.actions,
        label: m.overview.expressRun,
        icon: <Sparkles className="size-4" />,
        run: () => {
          setSection("overview");
          window.dispatchEvent(new CustomEvent("aether:express"));
        },
      },
      {
        id: "ram",
        group: m.palette.actions,
        label: m.performance.ramRun,
        icon: <MemoryStick className="size-4" />,
        run: safe(async () => {
          const r = await api.ramPurge();
          toast("success", fmt(m.performance.ramResult, { size: formatBytes(r.freed, lang), count: r.trimmed }));
        }),
      },
      {
        id: "flush",
        group: m.palette.actions,
        label: m.network.flushDns,
        icon: <RefreshCw className="size-4" />,
        run: safe(async () => {
          await api.flushDns();
          toast("success", m.network.flushDone);
        }),
      },
      {
        id: "restore",
        group: m.palette.actions,
        label: m.overview.createPoint,
        icon: <ShieldCheck className="size-4" />,
        run: safe(async () => {
          await api.restoreCreate("Aether");
          toast("success", m.onboarding.restoreCreated);
        }),
      },
      { id: "undo", group: m.palette.actions, label: m.titlebar.undo, icon: <Undo2 className="size-4" />, run: undo },
      {
        id: "expert",
        group: m.palette.actions,
        label: m.common.advancedToggle,
        icon: <SlidersHorizontal className="size-4" />,
        run: () => setExpert(!expert),
      },
      { id: "logs", group: m.palette.actions, label: m.logs.title, icon: <ScrollText className="size-4" />, run: () => setLogsOpen(true) },
      {
        id: "update",
        group: m.palette.actions,
        label: m.palette.checkUpdates,
        icon: <Download className="size-4" />,
        run: safe(async () => {
          const update = await check();
          if (!update) {
            toast("info", m.palette.upToDate);
            return;
          }
          toast("info", fmt(m.palette.updateAvailable, { version: update.version }));
          await update.downloadAndInstall();
        }),
      },
    ];
  }, [m, lang, setSection, toast, undo, setExpert, expert, setLogsOpen]);

  const filtered = commands.filter((c) => c.label.toLowerCase().includes(query.trim().toLowerCase()));

  if (!open) return null;

  const runAt = (i: number) => {
    const cmd = filtered[i];
    if (!cmd) return;
    setOpen(false);
    void cmd.run();
  };

  let lastGroup = "";
  return (
    <div className="absolute inset-0 z-50 flex justify-center bg-black/60 pt-[12vh] animate-fade-in" onMouseDown={() => setOpen(false)}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={m.titlebar.search}
        className="glass-float h-fit w-[560px] max-w-[92%] overflow-hidden rounded-2xl animate-scale-in"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3 border-b border-white/[0.06] px-4">
          <Search className="size-4 text-slate-500" aria-hidden />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setIndex(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") setOpen(false);
              else if (e.key === "ArrowDown") {
                e.preventDefault();
                setIndex((i) => Math.min(i + 1, filtered.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => Math.max(i - 1, 0));
              } else if (e.key === "Enter") runAt(index);
            }}
            placeholder={m.palette.placeholder}
            className="h-12 flex-1 bg-transparent text-[14px] text-white outline-none placeholder:text-slate-500"
          />
          <kbd className="rounded border border-white/10 px-1.5 font-mono text-[10.5px] text-slate-500">Esc</kbd>
        </div>
        <div className="scroll-area max-h-[380px] p-1.5">
          {filtered.length === 0 && <div className="px-3 py-6 text-center text-[13px] text-[var(--text-3)]">{m.palette.empty}</div>}
          {filtered.map((c, i) => {
            const header = c.group !== lastGroup ? c.group : null;
            lastGroup = c.group;
            return (
              <div key={c.id}>
                {header && (
                  <div className="px-3 pt-2.5 pb-1 text-[10.5px] font-semibold uppercase tracking-[0.12em] text-slate-500">{header}</div>
                )}
                <button
                  type="button"
                  onMouseEnter={() => setIndex(i)}
                  onClick={() => runAt(i)}
                  className={cx(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-[13px] transition-colors",
                    i === index ? "bg-white/[0.07] text-white" : "text-slate-300",
                  )}
                >
                  <span className={i === index ? "text-cyan" : "text-slate-500"}>{c.icon}</span>
                  <span className="flex-1">{c.label}</span>
                  {i === index && <CornerDownLeft className="size-3.5 text-slate-500" aria-hidden />}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
