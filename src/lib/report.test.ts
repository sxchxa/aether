import { describe, expect, it } from "vitest";
import fr from "../i18n/locales/fr";
import { buildReport } from "./report";

describe("rapport HTML", () => {
  it("échappe tout contenu (aucune injection de code possible)", () => {
    const html = buildReport(fr, "fr-FR", {
      info: null,
      tweaks: {},
      history: [{ id: 1, ts: Date.now(), target: "x", label: "<script>alert(1)</script>" }],
      measures: null,
    });
    expect(html).not.toContain("<script>alert(1)</script>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("produit un document complet et autonome", () => {
    const html = buildReport(fr, "fr-FR", { info: null, tweaks: {}, history: [], measures: null });
    expect(html.startsWith("<!doctype html>")).toBe(true);
    expect(html).toContain(fr.report.title);
    // Aucune ressource externe : le rapport s'ouvre hors ligne.
    expect(html).not.toMatch(/<(script|link)\b/);
  });
});
