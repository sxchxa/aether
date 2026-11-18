// Aide : problèmes fréquents et solutions pas à pas, avec recherche.
import { useMemo, useState } from "react";
import { ChevronRight, Search } from "lucide-react";
import { Card, cx, PageHeader } from "../../components/ui";
import { useI18n } from "../../i18n";
import { useApp } from "../../store/app";

/** Raccourcis vers l'endroit de l'application qui règle le problème. */
const SHORTCUTS: Partial<Record<string, { section: "settings" | "network" | "performance"; anchor: string }>> = {
  restorePoint: { section: "settings", anchor: "set-restore-points" },
  undoAll: { section: "settings", anchor: "set-safety" },
  noInternet: { section: "network", anchor: "nw-dns" },
  printer: { section: "performance", anchor: "pf-services" },
  search: { section: "performance", anchor: "pf-services" },
};

export function HelpPage() {
  const { m } = useI18n();
  const goTo = useApp((s) => s.goTo);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<string | null>("restorePoint");

  const items = useMemo(() => {
    const all = Object.entries(m.help.items);
    const q = query.trim().toLowerCase();
    return q ? all.filter(([, it]) => `${it.q} ${it.a}`.toLowerCase().includes(q)) : all;
  }, [m, query]);

  const jump = (key: string) => {
    const target = SHORTCUTS[key];
    if (!target) return;
    goTo(target.section);
    window.setTimeout(() => document.getElementById(target.anchor)?.scrollIntoView({ behavior: "smooth", block: "start" }), 120);
  };

  return (
    <div>
      <PageHeader section="help" title={m.help.title} subtitle={m.help.subtitle} />
      <label className="surface-2 mb-6 flex items-center gap-3 rounded-[12px] px-4 py-3">
        <Search className="size-4 shrink-0 text-[var(--text-3)]" aria-hidden />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={m.help.search}
          aria-label={m.help.search}
          className="min-w-0 flex-1 bg-transparent text-[14px] text-white outline-none placeholder:text-[var(--text-3)]"
        />
      </label>
      <Card>
        {items.length === 0 ? (
          <p className="px-6 py-5 text-[13px] text-[var(--text-3)]">{m.help.empty}</p>
        ) : (
          <ul className="divide-y divide-white/[0.05]">
            {items.map(([key, it]) => {
              const expanded = open === key || query.trim() !== "";
              return (
                <li key={key}>
                  <button
                    type="button"
                    aria-expanded={expanded}
                    onClick={() => setOpen(open === key ? null : key)}
                    className="flex w-full items-center gap-3 px-6 py-4 text-left hover:bg-white/[0.02]"
                  >
                    <ChevronRight
                      className={cx("size-4 shrink-0 text-[var(--steel)] transition-transform", expanded && "rotate-90")}
                      aria-hidden
                    />
                    <span className="min-w-0 flex-1 text-[14px] font-medium text-zinc-100">{it.q}</span>
                  </button>
                  {expanded && (
                    <div className="px-6 pb-5 pl-[52px]">
                      <p className="text-[13.5px] leading-relaxed whitespace-pre-line text-[var(--text-2)]">{it.a}</p>
                      {SHORTCUTS[key] && (
                        <button
                          type="button"
                          onClick={() => jump(key)}
                          className="mt-3 text-[13px] font-medium text-[var(--accent-soft)] hover:underline"
                        >
                          {m.help.goThere} →
                        </button>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </Card>
    </div>
  );
}
