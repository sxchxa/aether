import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { detectLanguage, loadLanguage } from "./i18n";
import { applyTheme, watchSystemTheme } from "./lib/theme";
import "./index.css";
import App from "./App";

// Thème appliqué avant le premier rendu : aucun flash de couleur au démarrage.
applyTheme();
watchSystemTheme();

const root = document.getElementById("root");
if (root) {
  // Dictionnaire de la langue active chargé avant le premier rendu (l'écran de chargement est visible).
  void loadLanguage(detectLanguage())
    .catch(() => undefined)
    .finally(() =>
      createRoot(root).render(
        <StrictMode>
          <App />
        </StrictMode>,
      ),
    );
}
