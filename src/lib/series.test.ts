import { describe, expect, it } from "vitest";
import { downsample, smooth } from "./series";

describe("séries des graphiques", () => {
  it("lisse une valeur isolée sans changer la longueur", () => {
    const out = smooth([0, 0, 100, 0, 0]);
    expect(out).toHaveLength(5);
    expect(Math.max(...out)).toBeLessThan(100);
    expect(out[2]).toBeCloseTo(20);
  });

  it("garde une série constante intacte", () => {
    expect(smooth([42, 42, 42, 42])).toEqual([42, 42, 42, 42]);
  });

  it("réduit une longue série en conservant la moyenne", () => {
    const values = Array.from({ length: 1800 }, (_, i) => (i % 2 === 0 ? 0 : 100));
    const out = downsample(values, 240);
    expect(out).toHaveLength(240);
    const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
    expect(mean(out)).toBeCloseTo(mean(values), 0);
  });

  it("ne touche pas une série déjà courte", () => {
    expect(downsample([1, 2, 3], 240)).toEqual([1, 2, 3]);
  });
});
