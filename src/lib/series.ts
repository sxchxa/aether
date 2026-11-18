// Traitement des séries de mesures (graphiques du tableau de bord).

/** Moyenne glissante sur 5 points : courbe calme, sans à-coups. */
export function smooth(values: number[]): number[] {
  return values.map((_, i) => {
    const part = values.slice(Math.max(0, i - 2), i + 3);
    return part.reduce((a, b) => a + b, 0) / part.length;
  });
}

/** Réduit une série à `max` points (moyenne par paquets) pour un tracé léger. */
export function downsample(values: number[], max: number): number[] {
  if (values.length <= max) return values;
  const size = values.length / max;
  return Array.from({ length: max }, (_, i) => {
    const part = values.slice(Math.floor(i * size), Math.floor((i + 1) * size));
    return part.reduce((a, b) => a + b, 0) / Math.max(1, part.length);
  });
}
