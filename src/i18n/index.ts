// Système i18n : i18next gère la langue active, les dictionnaires typés fournissent les textes.
import i18n from "i18next";
import { initReactI18next, useTranslation } from "react-i18next";
import fr, { type Messages } from "./locales/fr";

export const LANGUAGES = [
  { code: "fr-FR", label: "Français", short: "FR" },
  { code: "en-US", label: "English", short: "EN" },
  { code: "es-ES", label: "Español", short: "ES" },
  { code: "de-DE", label: "Deutsch", short: "DE" },
  { code: "it-IT", label: "Italiano", short: "IT" },
] as const;

export type LanguageCode = (typeof LANGUAGES)[number]["code"];

// Le français (langue de secours) est intégré ; les autres langues sont chargées à la demande :
// seul le dictionnaire de la langue active est lu au démarrage.
const BUNDLES: Partial<Record<LanguageCode, Messages>> = { "fr-FR": fr };

const LOADERS: Record<Exclude<LanguageCode, "fr-FR">, () => Promise<{ default: Messages }>> = {
  "en-US": () => import("./locales/en"),
  "es-ES": () => import("./locales/es"),
  "de-DE": () => import("./locales/de"),
  "it-IT": () => import("./locales/it"),
};

/** Charge (une seule fois) le dictionnaire d'une langue. */
export async function loadLanguage(code: LanguageCode): Promise<void> {
  if (BUNDLES[code] || code === "fr-FR") return;
  const messages = (await LOADERS[code]()).default;
  BUNDLES[code] = messages;
  i18n.addResourceBundle(code, "translation", messages, true, true);
}

const STORAGE_KEY = "aether.language";

export function isLanguage(value: string | null | undefined): value is LanguageCode {
  return LANGUAGES.some((l) => l.code === value);
}

function safeStorageGet(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeStorageSet(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    /* stockage indisponible : préférence non mémorisée */
  }
}

export function detectLanguage(): LanguageCode {
  const stored = safeStorageGet(STORAGE_KEY);
  if (isLanguage(stored)) return stored;
  const prefix = navigator.language.slice(0, 2).toLowerCase();
  return LANGUAGES.find((l) => l.code.startsWith(prefix))?.code ?? "fr-FR";
}

void i18n.use(initReactI18next).init({
  resources: { "fr-FR": { translation: fr } },
  lng: detectLanguage(),
  fallbackLng: "fr-FR",
  interpolation: { escapeValue: false },
});

/** Remplace les `{{variables}}` d'un gabarit. */
export function fmt(template: string, vars: Record<string, string | number> = {}): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key: string) => {
    const value = vars[key];
    return value === undefined ? "" : String(value);
  });
}

export function useI18n() {
  const { i18n: instance } = useTranslation();
  const lang: LanguageCode = isLanguage(instance.language) ? instance.language : "fr-FR";
  const setLanguage = (code: LanguageCode) => {
    safeStorageSet(STORAGE_KEY, code);
    document.documentElement.lang = code;
    void loadLanguage(code).then(() => instance.changeLanguage(code));
  };
  return { m: BUNDLES[lang] ?? fr, lang, setLanguage };
}

export function messagesFor(code: LanguageCode): Messages {
  return BUNDLES[code] ?? fr;
}

export type { Messages };
export default i18n;
