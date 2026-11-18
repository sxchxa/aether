# Contribuer à Aether

Merci de votre intérêt ! Aether modifie des réglages système : la **sécurité** et la **réversibilité** passent avant tout.

## Règles du projet

1. **Réglages sûrs et documentés uniquement.** Chaque option doit correspondre à un interrupteur des Paramètres Windows ou à une stratégie documentée par Microsoft. Indiquez la source dans votre pull request.
2. **Idempotent et réversible.** Toute option lit l'état réel (`check_status`), ne touche à rien si elle est déjà appliquée, et sauvegarde l'état d'origine avant la première écriture.
3. **WebView2 n'est jamais modifié ni supprimé.**
4. **Pas de `.unwrap()` ni de `.expect()` dans le code applicatif.** Les erreurs passent par `AetherError`.
5. **TypeScript strict, sans `any`.** Les textes vont dans les 5 langues (`src/i18n/locales/*.ts`).
6. **Les commandes système** s'exécutent sans fenêtre (`CREATE_NO_WINDOW`) et ne reçoivent jamais de texte non validé.

## Avant d'ouvrir une pull request

```powershell
npm run format             # met en forme l'interface (Prettier)
npm run lint
npm run typecheck
npm test
npm run check:i18n
cd src-tauri
cargo fmt                  # met en forme le code Rust (rustfmt)
cargo clippy --all-targets -- -D warnings
cargo test
```

Pour une nouvelle option, lancez aussi le test de bout en bout **sur une machine de test** :

```powershell
$env:AETHER_E2E_ONLY = "id.de.votre.option"
cargo test --test e2e every_tweak_applies_and_restores -- --ignored --nocapture --test-threads=1
```

Il doit montrer que l'option s'applique, qu'une seconde application ne change rien, et que la restauration rétablit la valeur d'origine **à l'identique**.

## Signaler un problème

Utilisez les modèles d'[issues](../../issues/new/choose). Pour une faille de sécurité, suivez [SECURITY.md](SECURITY.md) et ne la publiez pas dans une issue.
