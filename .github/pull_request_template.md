## Description

<!-- Ce que change cette pull request et pourquoi. -->

## Pour une nouvelle option d'optimisation

- Source officielle (Microsoft / Paramètres Windows) :
- [ ] Test de bout en bout réussi sur une machine de test (application, idempotence, restauration à l'identique)

## Vérifications

- [ ] `npm run typecheck`, `npm test` et `npm run check:i18n`
- [ ] `cargo clippy --all-targets -- -D warnings` et `cargo test`
- [ ] Textes ajoutés dans les 5 langues
- [ ] WebView2 n'est pas concerné
