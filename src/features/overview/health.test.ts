import { describe, expect, it } from "vitest";
import type { TweakStatus } from "../../lib/types";
import { computeHealth } from "./health";

function tweak(id: string, group: string, applied: boolean): TweakStatus {
  return {
    id,
    section: "performance",
    group,
    risk: "safe",
    technical: "",
    needsReboot: false,
    probe: { applied, available: true, current: "", target: "" },
    error: null,
  };
}

describe("score de santé", () => {
  const base = { info: null, ramPercent: 30, restoreEnabled: true };

  it("reste dans l'intervalle 0–100", () => {
    const score = computeHealth({ ...base, tweaks: [] });
    expect(score).toBeGreaterThanOrEqual(0);
    expect(score).toBeLessThanOrEqual(100);
  });

  it("monte quand les optimisations sont appliquées", () => {
    const none = computeHealth({ ...base, tweaks: [tweak("svc.fax", "services", false)] });
    const all = computeHealth({ ...base, tweaks: [tweak("svc.fax", "services", true)] });
    expect(all).toBeGreaterThan(none);
  });

  it("ignore les effets visuels et les préférences personnelles", () => {
    const optimized = [tweak("svc.fax", "services", true)];
    const withPreferences = [...optimized, tweak("fx.peek", "fx", false), tweak("ui.hiddenFiles", "visual", false)];
    expect(computeHealth({ ...base, tweaks: withPreferences })).toBe(computeHealth({ ...base, tweaks: optimized }));
  });

  it("pénalise une protection système désactivée", () => {
    const on = computeHealth({ ...base, tweaks: [] });
    const off = computeHealth({ ...base, restoreEnabled: false, tweaks: [] });
    expect(off).toBeLessThan(on);
  });
});
