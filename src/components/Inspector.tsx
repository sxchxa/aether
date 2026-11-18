// Panneau Inspecteur Pédagogique : rôle, gain attendu, mise en garde et cible technique.
import { useState } from "react";
import { ChevronRight, Lightbulb, ShieldAlert, Sparkles, Terminal, X } from "lucide-react";
import { useI18n } from "../i18n";
import { tweakIcon } from "../lib/catalog";
import { useApp } from "../store/app";
import { cx, RiskBadge } from "./ui";

interface Explain {
  title: string;
  role: string;
  gain: string;
  caution: string;
}

export function Inspector() {
  const { m } = useI18n();
  const target = useApp((s) => s.inspector);
  const close = useApp((s) => s.inspect);
  // Seule l'option inspectée est suivie (aucun rendu pour les autres changements).
  const inspected = useApp((s) => (s.inspector?.kind === "tweak" ? s.tweaks[s.inspector.id] : undefined));
  const [showTech, setShowTech] = useState(true);

  if (!target) return null;

  const status = inspected;
  const text: Explain | undefined =
    target.kind === "tweak" ? (m.tweaks as Record<string, Explain | undefined>)[target.id] : m.actions[target.id];
  const technical = target.kind === "tweak" ? (status?.technical ?? "") : target.technical;
  const Icon = target.kind === "tweak" ? tweakIcon(target.id) : Sparkles;

  return (
    <aside
      key={`${target.kind}:${target.id}`}
      className="glass-float absolute top-14 right-3 bottom-3 z-30 flex w-[340px] flex-col rounded-2xl animate-slide-in-right"
      aria-label={m.common.whatIsThis}
    >
      <div className="flex items-start justify-between gap-3 border-b border-white/[0.06] p-5">
        <div className="flex items-start gap-3">
          <div className="grid size-10 place-items-center rounded-xl border border-indigo/30 bg-indigo/10 text-indigo-soft">
            <Icon className="size-5" aria-hidden />
          </div>
          <div>
            <div className="text-[12px] text-zinc-500">{m.common.whatIsThis}</div>
            <h2 className="mt-0.5 text-[15px] font-semibold text-white">{text?.title ?? target.id}</h2>
            {status && (
              <div className="mt-1.5">
                <RiskBadge risk={status.risk} />
              </div>
            )}
          </div>
        </div>
        <button
          type="button"
          onClick={() => close(null)}
          aria-label={m.common.close}
          className="rounded-lg p-1 text-slate-400 hover:bg-white/5 hover:text-white"
        >
          <X className="size-4" />
        </button>
      </div>

      <div className="scroll-area flex-1 space-y-4 p-5">
        {text && (
          <>
            <Block icon={<Lightbulb className="size-4 text-cyan" />} title={m.common.role} body={text.role} />
            <Block icon={<Sparkles className="size-4 text-emerald" />} title={m.common.gain} body={text.gain} />
            <Block icon={<ShieldAlert className="size-4 text-amber" />} title={m.common.caution} body={text.caution} tone="warn" />
          </>
        )}

        {status && status.probe.available && (
          <div className="grid grid-cols-2 gap-2 text-[12px]">
            <div className="surface-2 rounded-[10px] p-3">
              <div className="text-[var(--text-3)]">{m.common.current}</div>
              <div className="selectable mt-1 break-words text-slate-200">{status.probe.current}</div>
            </div>
            <div className="surface-2 rounded-[10px] p-3">
              <div className="text-[var(--text-3)]">{m.common.target}</div>
              <div className="selectable mt-1 break-words text-emerald-300">{status.probe.target}</div>
            </div>
          </div>
        )}

        {technical && (
          <div className="well rounded-[10px]">
            <button
              type="button"
              onClick={() => setShowTech((v) => !v)}
              aria-expanded={showTech}
              className="flex w-full items-center gap-2 px-3 py-2.5 text-[12.5px] font-medium text-slate-300 hover:text-white"
            >
              <ChevronRight className={cx("size-4 transition-transform", showTech && "rotate-90")} aria-hidden />
              <Terminal className="size-3.5" aria-hidden />
              {m.common.technical}
            </button>
            {showTech && (
              <pre className="selectable overflow-x-auto border-t border-white/[0.06] px-3 py-2.5 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap break-all text-cyan/90">
                {technical}
              </pre>
            )}
          </div>
        )}
      </div>
    </aside>
  );
}

function Block({ icon, title, body, tone }: { icon: React.ReactNode; title: string; body: string; tone?: "warn" }) {
  return (
    <div className={cx("rounded-[10px] p-3.5", tone === "warn" ? "border border-amber/25 bg-amber/[0.06]" : "surface-2")}>
      <div className="flex items-center gap-2 text-[12px] font-semibold tracking-wide text-slate-200">
        {icon}
        {title}
      </div>
      <p className="mt-1.5 text-[13px] leading-relaxed text-[var(--text-2)]">{body}</p>
    </div>
  );
}
