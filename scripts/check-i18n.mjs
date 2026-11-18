// Vérifie les 5 langues : aucun texte vide, mêmes variables {{…}} que la langue source (français),
// aucune clé manquante ou en trop. Node lit directement les fichiers TypeScript (suppression des types).
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src", "i18n", "locales");
const LANGS = ["fr", "en", "es", "de", "it"];

async function load(code) {
  const mod = await import(pathToFileURL(path.join(dir, `${code}.ts`)).href);
  return mod.default;
}

function flatten(obj, prefix = "", out = new Map()) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object" && !Array.isArray(v)) flatten(v, key, out);
    else out.set(key, v);
  }
  return out;
}

const vars = (s) => [...String(s).matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).sort().join(",");

const source = flatten(await load("fr"));
let errors = 0;
for (const code of LANGS) {
  const dict = flatten(await load(code));
  for (const [key, value] of dict) {
    if (typeof value === "string" && value.trim() === "") {
      console.error(`✗ ${code} : texte vide → ${key}`);
      errors++;
    }
    if (!source.has(key)) {
      console.error(`✗ ${code} : clé inconnue en français → ${key}`);
      errors++;
    } else if (typeof value === "string" && vars(value) !== vars(source.get(key))) {
      console.error(`✗ ${code} : variables différentes du français → ${key} (« ${vars(value)} » au lieu de « ${vars(source.get(key))} »)`);
      errors++;
    }
  }
  for (const key of source.keys()) {
    if (!dict.has(key)) {
      console.error(`✗ ${code} : clé manquante → ${key}`);
      errors++;
    }
  }
  console.log(`${errors === 0 ? "✓" : "·"} ${code} : ${dict.size} textes`);
}
if (errors > 0) {
  console.error(`${errors} problème(s) de traduction.`);
  process.exit(1);
}
console.log("Traductions complètes et cohérentes.");
