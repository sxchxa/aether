// Assistant de premier lancement : 1. langue · 2. niveau · 3. point de restauration.
import { useEffect, useState } from "react";
import { Check, ShieldCheck } from "lucide-react";
import { LevelCards } from "../../components/LevelPicker";
import { Logo } from "../../components/Logo";
import { Button, cx } from "../../components/ui";
import { fmt, LANGUAGES, useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { RestoreStatus } from "../../lib/types";
import { useApp } from "../../store/app";

export function Onboarding() {
  const { m, lang, setLanguage } = useI18n();
  const setOnboarded = useApp((s) => s.setOnboarded);
  const toast = useApp((s) => s.toast);
  const [step, setStep] = useState(1);
  const [restore, setRestore] = useState<RestoreStatus | null>(null);
  const [created, setCreated] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (step === 3)
      void api
        .restoreStatus()
        .then(setRestore)
        .catch(() => setRestore(null));
  }, [step]);

  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="aether-backdrop absolute inset-0 z-[60] grid place-items-center animate-fade-in">
      <div className="glass-float w-[620px] max-w-[92%] rounded-3xl p-8 animate-scale-in">
        <div className="flex items-center gap-3">
          <Logo size={34} />
          <div>
            <h1 className="text-[20px] font-semibold tracking-tight text-white">{m.onboarding.welcome}</h1>
            <p className="text-[12.5px] text-[var(--text-3)]">{fmt(m.onboarding.step, { n: step })}</p>
          </div>
        </div>
        <div className="mt-5 flex gap-1.5" aria-hidden>
          {[1, 2, 3].map((n) => (
            <div
              key={n}
              className={cx(
                "h-1 flex-1 rounded-full transition-colors duration-500",
                n <= step ? "bg-gradient-to-r from-indigo to-cyan" : "bg-white/[0.08]",
              )}
            />
          ))}
        </div>

        <div key={step} className="mt-7 min-h-[250px]">
          {step === 1 && (
            <>
              <h2 className="text-[15px] font-semibold text-white">{m.onboarding.languageTitle}</h2>
              <div className="mt-4 grid grid-cols-2 gap-2">
                {LANGUAGES.map((l) => (
                  <button
                    key={l.code}
                    type="button"
                    onClick={() => setLanguage(l.code)}
                    className={cx(
                      "flex items-center justify-between rounded-xl border px-4 py-3 text-left text-[13.5px] transition",
                      l.code === lang
                        ? "border-indigo/50 bg-indigo/10 text-white"
                        : "border-white/[0.07] bg-white/[0.02] text-slate-300 hover:border-white/15",
                    )}
                  >
                    {l.label}
                    {l.code === lang && <Check className="size-4 text-cyan" aria-hidden />}
                  </button>
                ))}
              </div>
            </>
          )}
          {step === 2 && (
            <>
              <h2 className="text-[15px] font-semibold text-white">{m.level.title}</h2>
              <p className="mt-1 text-[12.5px] text-[var(--text-3)]">{m.level.subtitle}</p>
              <div className="mt-4">
                <LevelCards />
              </div>
            </>
          )}
          {step === 3 && (
            <>
              <h2 className="text-[15px] font-semibold text-white">{m.onboarding.restoreTitle}</h2>
              <p className="mt-2 text-[13px] leading-relaxed text-[var(--text-2)]">{m.onboarding.restoreDesc}</p>
              <div className="mt-5 flex items-center gap-3">
                {restore && !restore.enabled ? (
                  <>
                    <Button variant="primary" loading={busy} onClick={() => void act(async () => setRestore(await api.restoreEnable()))}>
                      {m.overview.enableProtection}
                    </Button>
                    <span className="text-[12px] text-amber-300">{m.onboarding.restoreDisabled}</span>
                  </>
                ) : (
                  <Button
                    variant="primary"
                    loading={busy}
                    disabled={created || !restore}
                    icon={created ? <Check className="size-4" /> : <ShieldCheck className="size-4" />}
                    onClick={() =>
                      void act(async () => {
                        setRestore(await api.restoreCreate("Aether — avant optimisation"));
                        setCreated(true);
                      })
                    }
                  >
                    {created ? m.onboarding.restoreCreated : m.onboarding.restoreCreate}
                  </Button>
                )}
              </div>
            </>
          )}
        </div>

        <div className="mt-6 flex items-center justify-between">
          <Button variant="ghost" onClick={() => (step === 1 ? setOnboarded() : setStep(step - 1))}>
            {step === 1 ? m.common.skip : m.common.back}
          </Button>
          <Button variant="primary" onClick={() => (step === 3 ? setOnboarded() : setStep(step + 1))}>
            {step === 3 ? m.common.finish : m.common.next}
          </Button>
        </div>
      </div>
    </div>
  );
}
