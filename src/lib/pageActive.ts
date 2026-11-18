// Indique à un composant si sa page est actuellement affichée (les pages masquées restent montées).
import { createContext, useContext } from "react";

export const PageActiveContext = createContext(true);

export function usePageActive(): boolean {
  return useContext(PageActiveContext);
}
