// Tests unitaires de l’interface (fonctions pures : score, séries, rapport).
// Configuration séparée de vite.config.ts : seuls les tests de src/ sont exécutés
// (aucun autre dossier du projet).
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: { include: ["src/**/*.test.ts"], environment: "node" },
});
