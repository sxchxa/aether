// Logos officiels des applications (icônes publiées par chaque éditeur), intégrés à l'application :
// aucun téléchargement à l'exécution. Fichier nommé d'après l'identifiant winget (caractères
// non alphanumériques remplacés par « _ »).
const FILES = import.meta.glob<string>("../../assets/apps/*.{png,svg}", { eager: true, query: "?url", import: "default" });

const LOGOS = new Map<string, string>();
for (const [path, url] of Object.entries(FILES)) {
  const name = path
    .split("/")
    .pop()
    ?.replace(/\.(png|svg)$/, "");
  if (name) LOGOS.set(name, url);
}

/** Logos sombres, posés sur une pastille claire pour rester lisibles sur le thème noir. */
const DARK = new Set(["Ubisoft_Connect"]);

export interface AppLogo {
  src: string;
  light: boolean;
}

/** Logo officiel d'une application ; `null` si aucun n'est disponible (l'initiale est affichée). */
export function appLogo(id: string): AppLogo | null {
  const key = id.replace(/[^A-Za-z0-9]/g, "_");
  const src = LOGOS.get(key);
  return src ? { src, light: DARK.has(key) } : null;
}
