// Visualisations lumineuses : jauge circulaire SVG animée avec halo, courbe temps réel avec
// dégradé, point « pulse » au bout de la ligne et info-bulle au survol.
import { useId, useMemo, useState, type ReactNode } from "react";

interface GaugeProps {
  /** Valeur 0–100. */
  value: number;
  size?: number;
  stroke?: number;
  /** Couleur explicite ; sinon couleur de statut selon `invert`. */
  color?: string;
  /** Vrai si une valeur haute est mauvaise (occupation disque, RAM). */
  invert?: boolean;
  children?: ReactNode;
  label: string;
}

/** Couleurs de statut réservées (bon / attention / critique). */
export function statusColor(value: number, invert = false): string {
  const v = invert ? 100 - value : value;
  if (v >= 70) return "#10B981";
  if (v >= 40) return "#F59E0B";
  return "#F43F5E";
}

export function CircularGauge({ value, size = 168, stroke = 12, color, invert, children, label }: GaugeProps) {
  const id = useId();
  const pct = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  const pad = 14; // marge pour le halo lumineux
  const r = (size - stroke) / 2 - pad;
  const c = 2 * Math.PI * r;
  const tone = color ?? statusColor(pct, invert);
  const center = size / 2;
  return (
    <div
      className="relative grid place-items-center"
      style={{ width: size, height: size }}
      role="img"
      aria-label={`${label} : ${Math.round(pct)} %`}
    >
      <svg width={size} height={size} className="-rotate-90 overflow-visible">
        <defs>
          <linearGradient id={`g${id}`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor={tone} stopOpacity="0.7" />
            <stop offset="100%" stopColor={tone} />
          </linearGradient>
        </defs>
        {/* Rainure creusée */}
        <circle cx={center} cy={center} r={r} fill="none" stroke="transparent" strokeWidth={stroke + 4} />
        <circle cx={center} cy={center} r={r} fill="none" stroke="#1d1e24" strokeWidth={stroke} />
        <circle
          className="glow-arc"
          cx={center}
          cy={center}
          r={r}
          fill="none"
          stroke={`url(#g${id})`}
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c - (pct / 100) * c}
          style={{ color: tone, transition: "stroke-dashoffset 1100ms cubic-bezier(0.16,1,0.3,1), color 400ms ease" }}
        />
      </svg>
      <div className="absolute inset-0 grid place-items-center text-center">{children}</div>
    </div>
  );
}

interface SparklineProps {
  values: number[];
  /** Maximum de l'axe (100 pour un pourcentage). */
  max?: number;
  color: string;
  height?: number;
  label: string;
  format: (v: number) => string;
}

/** Courbe temps réel (une seule série) : dégradé lumineux, point pulsé, réticule au survol. */
export function Sparkline({ values, max = 100, color, height = 64, label, format }: SparklineProps) {
  const id = useId();
  const [hover, setHover] = useState<number | null>(null);
  const width = 320;
  const points = useMemo(() => {
    const n = values.length;
    if (n < 2) return [] as [number, number][];
    return values.map((v, i): [number, number] => [
      (i / (n - 1)) * width,
      height - 3 - (Math.max(0, Math.min(max, v)) / max) * (height - 8),
    ]);
  }, [values, max, height]);
  const line = points.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  const area = points.length ? `${line} L${width},${height} L0,${height} Z` : "";
  const hovered = hover !== null ? points[hover] : undefined;
  const hoveredValue = hover !== null ? values[hover] : undefined;
  const last = points[points.length - 1];

  return (
    <div className="relative" onMouseLeave={() => setHover(null)}>
      <svg
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        className="block w-full overflow-visible"
        style={{ height }}
        role="img"
        aria-label={label}
        onMouseMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const ratio = (e.clientX - rect.left) / rect.width;
          setHover(Math.round(ratio * (values.length - 1)));
        }}
      >
        <defs>
          <linearGradient id={`a${id}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={color} stopOpacity="0.28" />
            <stop offset="60%" stopColor={color} stopOpacity="0.12" />
            <stop offset="100%" stopColor={color} stopOpacity="0" />
          </linearGradient>
        </defs>
        {[0.25, 0.5, 0.75].map((f) => (
          <line
            key={f}
            x1="0"
            x2={width}
            y1={height * f}
            y2={height * f}
            stroke="rgba(255,255,255,0.035)"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {area && <path d={area} fill={`url(#a${id})`} />}
        {line && (
          <path
            d={line}
            fill="none"
            stroke={color}
            strokeWidth="2.25"
            strokeLinejoin="round"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        )}
        {hovered && (
          <line
            x1={hovered[0]}
            x2={hovered[0]}
            y1="0"
            y2={height}
            stroke="rgba(255,255,255,0.25)"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
        )}
      </svg>
      {last && (
        <span
          className="pointer-events-none absolute size-2.5 -translate-1/2"
          style={{ left: "100%", top: `${(last[1] / height) * 100}%` }}
        >
          <span className="absolute inset-0 rounded-full border-2 border-[#0f1013]" style={{ background: color }} />
        </span>
      )}
      {hovered && hoveredValue !== undefined && (
        <div
          className="glass-float pointer-events-none absolute -top-8 rounded-[8px] px-2 py-0.5 text-[11px] tabular text-slate-100"
          style={{ left: `clamp(0px, calc(${(hovered[0] / width) * 100}% - 24px), calc(100% - 56px))` }}
        >
          {format(hoveredValue)}
        </div>
      )}
    </div>
  );
}
