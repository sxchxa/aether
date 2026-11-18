import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Configuration Vite pour Tauri v2 (port fixe, pas d'effacement de la console).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // Limite l'analyse des dépendances à Aether (ignore le dépôt de référence local).
  optimizeDeps: { entries: ["index.html"] },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    // Application de bureau chargée localement : un paquet unique est plus rapide à démarrer.
    chunkSizeWarningLimit: 900,
    // Logos d'applications servis en fichiers séparés (chargés à l'affichage) plutôt qu'intégrés au script principal.
    assetsInlineLimit: (file) => (file.replace(/\\/g, "/").includes("/assets/apps/") ? false : undefined),
  },
});
