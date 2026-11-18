// Primitives d'interface « Dimensional Dark UI » : boutons en relief, interrupteur 3D,
// badges d'icônes lumineux, cartes à éclairage zénithal, en-têtes de page.
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Check, CircleHelp, LoaderCircle } from "lucide-react";
import type { Risk } from "../lib/types";
import { useI18n } from "../i18n";
import type { SectionId } from "../store/app";

export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

export type Tone = "sky" | "emerald" | "amber" | "violet" | "indigo" | "rose" | "slate";

type ButtonVariant = "primary" | "indigo" | "secondary" | "ghost" | "danger";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  loading?: boolean;
  icon?: ReactNode;
  size?: "sm" | "md" | "lg";
}

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "btn-3d-primary",
  indigo: "btn-3d-indigo",
  secondary: "btn-3d",
  ghost: "border border-transparent text-slate-300 hover:bg-white/[0.06] hover:text-white",
  danger: "btn-3d-danger",
};

const SIZES = {
  sm: "h-8 px-3.5 text-[12.5px] rounded-[9px]",
  md: "h-10 px-4 text-[13px] rounded-[10px]",
  lg: "h-12 px-6 text-[14.5px] rounded-[14px]",
};

export function Button({ variant = "secondary", loading, icon, size = "md", className, children, disabled, ...rest }: ButtonProps) {
  return (
    <button
      {...rest}
      disabled={disabled || loading}
      className={cx(
        "inline-flex items-center justify-center gap-2 font-semibold transition-[background-color,border-color,color,box-shadow,opacity,translate] duration-150 active:translate-y-px disabled:cursor-not-allowed disabled:opacity-55",
        SIZES[size],
        VARIANTS[variant],
        className,
      )}
    >
      {loading ? <LoaderCircle className="size-4 animate-spin" aria-hidden /> : icon}
      {children}
    </button>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  busy?: boolean;
  label: string;
}

export function Toggle({ checked, onChange, disabled, busy, label }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled || busy}
      onClick={() => onChange(!checked)}
      data-on={checked}
      className="toggle-track relative inline-flex h-[26px] w-[46px] shrink-0 items-center rounded-full transition-[background-color,border-color,opacity] duration-200 disabled:cursor-not-allowed disabled:opacity-50"
    >
      <span
        className={cx(
          "toggle-knob grid size-5 place-items-center rounded-full transition-transform duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]",
          checked ? "translate-x-[23px]" : "translate-x-[3px]",
        )}
      >
        {busy && <LoaderCircle className="size-3 animate-spin text-slate-500" aria-hidden />}
      </span>
    </button>
  );
}

/** Case à cocher visuelle (la ligne parente porte le rôle `checkbox` et l'interaction). */
export function CheckMark({ checked, disabled }: { checked: boolean; disabled?: boolean }) {
  return (
    <span
      aria-hidden
      className={cx(
        "grid size-[18px] shrink-0 place-items-center rounded-[6px] border transition-colors duration-150",
        checked ? "border-[var(--accent)] bg-[var(--accent)]" : "border-white/20 bg-white/[0.02]",
        disabled && "opacity-40",
      )}
    >
      {checked && <Check className="size-3 text-white" strokeWidth={3.2} />}
    </span>
  );
}

/** Icône posée dans un carré arrondi lumineux (badge 3D). */
export function IconBadge({ children, tone = "sky", size = "md" }: { children: ReactNode; tone?: Tone; size?: "sm" | "md" | "lg" }) {
  const dims = size === "sm" ? "size-8 rounded-[10px]" : size === "lg" ? "size-11 rounded-[10px]" : "size-9";
  return <span className={cx("icon-badge", `tone-${tone}`, dims)}>{children}</span>;
}

const RISK_STYLE: Record<Risk, string> = {
  safe: "text-emerald-300 bg-emerald/10 border-emerald/25",
  moderate: "text-amber-300 bg-amber/10 border-amber/25",
  advanced: "text-rose-300 bg-rose/10 border-rose/25",
};

const RISK_DOT: Record<Risk, string> = { safe: "bg-emerald", moderate: "bg-amber", advanced: "bg-rose" };

export function RiskBadge({ risk }: { risk: Risk }) {
  const { m } = useI18n();
  return (
    <span className={cx("inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[11px] font-medium", RISK_STYLE[risk])}>
      <span aria-hidden className={cx("size-1.5 rounded-full", RISK_DOT[risk])} />
      {m.risk[risk]}
    </span>
  );
}

export function Card({ children, className, hover, id }: { children: ReactNode; className?: string; hover?: boolean; id?: string }) {
  return (
    <section id={id} className={cx("glass anchor-card", hover && "lift", className)}>
      {children}
    </section>
  );
}

interface CardHeaderProps {
  icon?: ReactNode;
  tone?: Tone;
  title: string;
  description?: string;
  actions?: ReactNode;
  onHelp?: () => void;
}

export function CardHeader({ title, description, actions, onHelp }: CardHeaderProps) {
  const { m } = useI18n();
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-3 px-6 pt-5 pb-4">
      <div className="flex min-w-0 flex-1 basis-[260px] items-center gap-3.5">
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <h3 className="truncate text-[15.5px] font-normal tracking-[-0.005em] text-zinc-100" title={description ?? title}>
              {title}
            </h3>
            {onHelp && (
              <button
                type="button"
                onClick={onHelp}
                aria-label={m.common.whatIsThis}
                className="rounded-full text-slate-500 transition-colors hover:text-cyan"
              >
                <CircleHelp className="size-4" />
              </button>
            )}
          </div>
        </div>
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

interface PageHeaderProps {
  section: SectionId;
  title: string;
  subtitle: string;
  actions?: ReactNode;
}

/** En-tête de page condensé : icône, titre et une ligne de description. */
export function PageHeader({ title, subtitle, actions }: PageHeaderProps) {
  return (
    <header className="mb-10 animate-fade-in">
      <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-4">
        <div className="min-w-0">
          <h1 className="text-[32px] leading-tight font-medium tracking-[-0.02em] text-zinc-50">{title}</h1>
          <p className="mt-1.5 text-[14.5px] text-zinc-400">{subtitle}</p>
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
    </header>
  );
}

export function Skeleton({ className }: { className?: string }) {
  return <div className={cx("skeleton", className)} aria-hidden />;
}

export function Stat({ label, value, hint }: { label: string; value: ReactNode; hint?: ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--text-3)]">{label}</div>
      <div className="tabular mt-1 truncate text-[18px] font-semibold text-white">{value}</div>
      {hint && <div className="mt-0.5 truncate text-[12px] text-[var(--text-3)]">{hint}</div>}
    </div>
  );
}

export function Bar({ value, tone = "indigo" }: { value: number; tone?: "indigo" | "sky" | "emerald" | "amber" | "rose" }) {
  const colors = {
    indigo: "from-indigo-soft to-indigo",
    sky: "from-cyan to-sky",
    emerald: "from-emerald-300 to-emerald",
    amber: "from-amber-300 to-amber",
    rose: "from-rose-300 to-rose",
  };
  const pct = Math.max(0, Math.min(100, value));
  return (
    <div className="well h-2 w-full overflow-hidden rounded-full">
      <div className={cx("meter-fill h-full rounded-full bg-gradient-to-r", colors[tone])} style={{ transform: `scaleX(${pct / 100})` }} />
    </div>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="px-6 pb-5 text-[13px] text-[var(--text-3)]">{children}</div>;
}
