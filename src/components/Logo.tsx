// Monogramme Aether : « A » aux traits épais et arrondis, lame courbe en bleu acier
// (même géométrie que app-icon.svg et que la maquette de référence).
import { useId } from "react";

export function Logo({ size = 22, tile = false }: { size?: number; tile?: boolean }) {
  const id = useId();
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden>
      <defs>
        <linearGradient id={`${id}a`} x1="512" y1="170" x2="512" y2="880" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#6E9DEB" />
          <stop offset="1" stopColor="#2F5FC4" />
        </linearGradient>
        <linearGradient id={`${id}b`} x1="250" y1="620" x2="600" y2="860" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#3D6ED2" />
          <stop offset="1" stopColor="#2853B2" />
        </linearGradient>
      </defs>
      {tile && <path fill="#0C1018" d="M512 32C843 32 992 181 992 512S843 992 512 992 32 843 32 512 181 32 512 32Z" />}
      <path
        d="M222 806L512 236L802 806"
        fill="none"
        stroke={`url(#${id}a)`}
        strokeWidth={124}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        fill={`url(#${id}b)`}
        d="M206 784C276 668 420 612 572 618C626 620 638 650 612 670C510 716 384 764 302 818C288 830 276 841 266 850L222 806Z"
      />
    </svg>
  );
}
