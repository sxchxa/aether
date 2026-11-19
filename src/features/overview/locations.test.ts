import { describe, expect, it } from "vitest";
import { SECTION_TWEAKS } from "../../lib/navigation";
import { tweakLocation } from "./locations";

describe("tweakLocation", () => {
  it("situe chaque réglage des menus dans sa propre section", () => {
    for (const [section, ids] of Object.entries(SECTION_TWEAKS)) {
      for (const id of ids) {
        const where = tweakLocation(id);
        expect(where, id).not.toBeNull();
        expect(where?.section, id).toBe(section);
        expect(where?.anchor, id).not.toBe("");
      }
    }
    expect(tweakLocation("maint.weekly")).toEqual({ section: "storage", anchor: "st-maintenance" });
  });

  it("ne situe pas un réglage absent des menus", () => {
    expect(tweakLocation("inconnu.option")).toBeNull();
  });
});
