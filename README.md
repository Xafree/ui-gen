# UI-GEN

Studio de bureau (Windows) pour concevoir des composants d'interface Lit, avec leur code synchronisé
dans les deux sens avec le canvas. Electron, Lit et TypeScript.

## Prérequis

- Node.js 22.12 ou plus récent
- Docker Desktop, pour l'analyse SonarQube seulement

## Installer

```sh
npm install
```

Le binaire d'Electron se télécharge au premier lancement ; pour le télécharger tout de suite :
`npx install-electron --no`.

## Lancer

```sh
npm run dev
```

## Construire

```sh
npm run build
```

Le résultat est dans `out/` (`main/`, `preload/`, `renderer/`).

## Tester

| Commande | Ce qu'elle fait |
|---|---|
| `npm run typecheck` | vérifie les types (TypeScript `strict`) |
| `npm run lint` | ESLint, typescript-eslint `strictTypeChecked` et `stylisticTypeChecked`, eslint-plugin-lit |
| `npm run test` | tests unitaires Vitest de `tests/`, avec couverture (rapport lcov dans `coverage/`) |
| `npm run test:e2e` | construit l'application puis lance les tests Playwright de `tests/e2e/` sur Electron |
| `npm run test:mutation` | tests de mutation StrykerJS sur `domaine/` et `application/` (rapport dans `reports/`) |
| `npm run verifier-structure` | 5 fichiers au plus par dossier de `src/` et `tests/`, sens des imports |
| `npm run verifier-fct` | chaque critère `FCT-NNNN` est cité par un test, et inversement |
| `npm run adr-lint` | forme des ADR |

Les trois dernières commandes lancent les scripts du dossier parent (`../scripts/`), qui lisent
la documentation fonctionnelle et les ADR rangés hors de ce dépôt.

## Analyse SonarQube

```sh
npm run sonar:up      # démarre SonarQube sur http://127.0.0.1:9100
npm run test          # produit coverage/lcov.info
npm run sonar         # analyse ; le jeton est lu dans la variable d'environnement SONAR_TOKEN
npm run sonar:down    # arrête le conteneur, les données restent dans les volumes Docker
```

## Organisation

| Chemin | Contenu |
|---|---|
| `src/main/` | processus main d'Electron |
| `src/preload/` | preload : API exposée au renderer par `contextBridge` |
| `src/renderer/` | interface du studio en Lit |
| `src/modules/<module>/` | code métier en architecture hexagonale : `domaine/`, `application/`, `ports/`, `adaptateurs/` |
| `tests/` | tests unitaires, même arborescence que `src/` |
| `tests/e2e/` | tests de bout en bout Playwright |
| `docker/compose.yaml` | services de développement (SonarQube) |
