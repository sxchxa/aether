// Emplacement manuel de chaque réglage : permet à « Tout optimiser » d'indiquer où
// chaque action peut être faite à la main.
import { NETWORK_TWEAKS } from "../network/NetworkPage";
import { PERFORMANCE_TWEAKS } from "../performance/PerformancePage";
import { SYSTEM_TWEAKS } from "../system/SystemPage";
import { POWER_SETTINGS } from "../../lib/navigation";
import type { SectionId } from "../../store/app";

export interface Location {
  section: SectionId;
  anchor: string;
}

const GROUPS: [SectionId, Record<string, readonly string[]>, Record<string, string>][] = [
  ["performance", PERFORMANCE_TWEAKS, { services: "pf-services", gaming: "pf-gaming", visual: "pf-visual", fs: "pf-fs", fx: "pf-fx" }],
  ["network", NETWORK_TWEAKS, { nic: "nw-nic", tcp: "nw-tcp", bandwidth: "nw-tcp", security: "nw-security" }],
  ["system", SYSTEM_TWEAKS, { update: "sy-update", privacy: "sy-privacy", contextMenu: "sy-menu", edge: "sy-edge" }],
  ["power", { plans: ["power.ultimate"], settings: POWER_SETTINGS }, { plans: "pw-plans", settings: "pw-settings" }],
  ["storage", { maintenance: ["maint.weekly"] }, { maintenance: "st-maintenance" }],
];

/** Carte de la page où le réglage `id` s'active manuellement (null s'il n'est affiché nulle part). */
export function tweakLocation(id: string): Location | null {
  for (const [section, groups, anchors] of GROUPS) {
    for (const [group, ids] of Object.entries(groups)) {
      if (ids.includes(id)) return { section, anchor: anchors[group] ?? "" };
    }
  }
  return null;
}
