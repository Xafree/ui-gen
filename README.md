# UI-GEN

Desktop studio (Windows) for designing Lit UI components, with their code kept in two-way sync with
the canvas. Electron, Lit and TypeScript.

## Prerequisites

- Node.js 22.12 or later
- Docker Desktop, for the SonarQube analysis only

## Install

```sh
npm install
```

The Electron binary is downloaded on first launch; to download it right away:
`npx install-electron --no`.

The `browser` test project needs the Playwright Chromium: `npx playwright install chromium`.

## Run

```sh
npm run dev
```

## Build

```sh
npm run build
```

The output is in `out/` (`main/`, `preload/`, `renderer/`).

## Test

| Command | What it does |
|---|---|
| `npm run typecheck` | checks types (TypeScript `strict`) |
| `npm run lint` | ESLint, typescript-eslint `strictTypeChecked` and `stylisticTypeChecked`, eslint-plugin-lit |
| `npm run test` | Vitest: `node` project (`tests/modules/`, `tests/main/`) and `browser` project (`tests/renderer/`, Lit components in headless Chromium from Playwright), merged coverage with the thresholds of `vitest.config.ts` (lcov report in `coverage/`) |
| `npm run test:e2e` | builds the app, then runs the Playwright tests from `tests/e2e/` on Electron |
| `npm run test:mutation` | StrykerJS mutation tests on `domain/` and `application/` (report in `reports/`) |
| `npm run check-structure` | at most 5 files per folder of `src/` and `tests/`, import direction |
| `npm run check-studio-styles` | `src/renderer/styles/tokens.css` and `studio.css` are byte-for-byte copies of the mockup stylesheets |
| `npm run check-fct` | every `FCT-NNNN` criterion is cited by a test (`// covers: FCT-NNNN`), and the other way round |
| `npm run adr-lint` | ADR format |

The last four commands run the scripts of the parent folder (`../scripts/`), which read the
functional documentation, the ADRs and the mockups kept outside this repository.

## SonarQube analysis

```sh
npm run sonar:up      # starts SonarQube on http://127.0.0.1:9100
npm run test          # produces coverage/lcov.info
npm run sonar         # analysis; the token is read from the SONAR_TOKEN environment variable
npm run sonar:down    # stops the container; data stays in the Docker volumes
```

## Layout

| Path | Content |
|---|---|
| `src/main/` | Electron main process |
| `src/utility/` | utility process entry (`compute-host`): reads and highlights component text sent by the main process, no disk access |
| `src/preload/` | preload: API exposed to the renderer through `contextBridge` |
| `src/renderer/` | studio UI in Lit |
| `src/modules/<module>/` | business code in hexagonal architecture: `domain/`, `application/`, `ports/`, `adapters/` |
| `tests/` | unit tests, same tree as `src/` |
| `tests/e2e/` | Playwright end-to-end tests |
| `docker/compose.yaml` | development services (SonarQube) |
