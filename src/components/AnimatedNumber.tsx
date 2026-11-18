// Valeur numérique affichée directement : pas de compteur animé (effet « tableau de bord » daté et
// coûteux — un rendu React par image). Les chiffres tabulaires évitent tout tremblement du texte.
export function AnimatedNumber({ value, format }: { value: number; format: (v: number) => string }) {
  return <>{format(value)}</>;
}
