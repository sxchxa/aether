// Signal « l'essentiel est affiché » : les lectures système lourdes (WMI, winget) attendent ce
// signal pour ne pas ralentir le démarrage sur les PC à peu de cœurs.
let release: () => void = () => undefined;

export const startupDone: Promise<void> = new Promise((resolve) => {
  release = resolve;
});

export function markStartupDone(): void {
  release();
}
