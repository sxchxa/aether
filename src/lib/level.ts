// Niveau de l'utilisateur : détermine les réglages et profils proposés.
// - Débutant : uniquement les réglages 🟢 sûrs ;
// - Joueur : réglages sûrs et modérés (jeu, latence, mémoire) ;
// - Expérimenté : tout, y compris les réglages avancés.
import type { Risk, UserLevel } from "./types";

export const LEVELS: readonly UserLevel[] = ["beginner", "gamer", "expert"];

const RANK: Record<UserLevel, number> = { beginner: 0, gamer: 1, expert: 2 };
const RISK_MIN_LEVEL: Record<Risk, UserLevel> = { safe: "beginner", moderate: "gamer", advanced: "expert" };

/** Le niveau `level` est au moins `min`. */
export function atLeast(level: UserLevel, min: UserLevel): boolean {
  return RANK[level] >= RANK[min];
}

/** Un réglage de ce niveau de risque est proposé à ce niveau d'utilisateur. */
export function riskAllowed(level: UserLevel, risk: Risk | undefined): boolean {
  return risk === undefined || atLeast(level, RISK_MIN_LEVEL[risk]);
}

export function isLevel(value: unknown): value is UserLevel {
  return value === "beginner" || value === "gamer" || value === "expert";
}
