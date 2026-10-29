# Plan technique · import de bibliothèques

Réalise la fiche [import-bibliotheques.md](import-bibliotheques.md). Chaque lot se termine par
`cargo xtask ci` vert (fmt, clippy `-D warnings`, tests, i18n, layers, assets, wasm), un commit par
tâche, et l'état de la RAM sous 70 % (`-j 4`, une commande cargo à la fois).

## Architecture

```
uigen.exe (Rust, egui)
 ├─ figcraft-engine  (L5)  commandes project.*, capture.status, import.*, page = composant, thèmes de tokens ;
 │                         port de capture (`CaptureBackend`, `Session::pump_capture`)
 ├─ uigen-dom        (L4)  NOUVEAU : instantané DOM (JSON) → arbre de calques FigCraft
 ├─ uigen-project    (L3)  NOUVEAU : dossier de projet, uigen.json, verrou des versions
 ├─ figcraft-capture (L6)  NOUVEAU : adaptateur du port de capture, pilote le service (processus enfant, JSON-lines)
 └─ ui-egui          (L6)  onglet Composants, réglages du projet, onglet Comparer, toast Annuler

service de capture (processus enfant, hors du code Rust)
 ├─ Node.js embarqué        installe le paquet (npm, versions verrouillées) et le compile (esbuild)
 └─ Chromium embarqué       affiche chaque combinaison, force les états, relève DOM + styles calculés
```

- **Pourquoi un processus à part** : installer un paquet npm et afficher un composant React, Vue ou
  Svelte exige Node et un vrai navigateur. Rust pilote le service par stdio en JSON-lines, sur le
  modèle du serveur MCP existant (`crates/mcp`, `MAX_LINE`, délais).
- **Écart à la règle « Rust only »** du dépôt : le script de capture est en JavaScript, isolé dans
  `sidecar/`, embarqué dans l'exécutable. L'exception s'écrit dans `CLAUDE.md`.
- **Le traducteur est en Rust pur** (`uigen-dom`, dépend seulement de `doc` et `svg`) : il se teste
  sans navigateur, sur des instantanés enregistrés.
- **Web (wasm)** : l'import n'existe que sur le bureau (`cfg(not(target_arch = "wasm32"))`).

## Service de capture

Crate `figcraft-capture` (`crates/capture`, L6) : il implémente le port `CaptureBackend` du moteur en
lançant `node sidecar/service.mjs`. Les applications de bureau l'installent dans la `Session`
(`set_capture_backend`) ; la version web n'en installe aucun et les commandes répondent « capture service
unavailable in the browser ». Un fil dédié exécute un travail à la fois ; l'interface n'attend jamais :
`Session::pump_capture()` applique les travaux terminés à chaque image (boucle d'attente pour le CLI).

**Protocole, version 1.** Une ligne JSON UTF-8 par message sur stdin / stdout de Node ; stderr est un
journal libre, jamais lu comme protocole.

- **Salut** : le service écrit d'abord `{"protocol":1,"service":"uigen-sidecar","node":"22.16.0"}`. Le client
  refuse un autre numéro (erreur `Protocol` : « capture service speaks protocol N, UIGEN expects 1 »).
- **Requête** : `{"id": n, "method", "params"}`, `id` entier ≥ 1. **Réponse** : `{"id","ok":true,"result"}` ou
  `{"id","ok":false,"error":{"kind","message"}}`.
- **Genres d'erreur** : `invalidRequest`, `install`, `uninstall`, `notInstalled`, `componentNotFound`,
  `frameworkUnsupported`, `render`, `tooLarge`, `internal`. Côté Rust s'y ajoutent `Unavailable`, `Start`
  (Node ou script introuvable, message qui nomme `UIGEN_NODE` et `UIGEN_SIDECAR_DIR`), `Timeout`,
  `ServiceExited` et `Protocol`.

| Méthode | Paramètres | Résultat |
|---|---|---|
| `install` | `dir`, `package`, `version` (exacte ou `latest`) | `{package, version, license, framework}` |
| `uninstall` | `dir`, `package` | `{package}` |
| `inspect` | `dir`, `package` | `{package, version, framework, components: [{name, props}]}` |
| `render` | `dir`, `package`, `component`, `props`, `state`, `theme` | `{snapshot (instantané v1 de `figcraft-dom`), png (chemin relatif à `dir`)}` |
| `shutdown` | `{}` | `{}` |

`state` ∈ `default`, `hover`, `focus`, `pressed`, `disabled` ; `theme` ∈ `light`, `dark`. `framework` ∈ `react`,
`vue`, `svelte`, `web-components`, `unknown` ; `render` n'accepte que `react` et `web-components`
(`frameworkUnsupported` sinon).

**Délais** (client, `Timeouts::default()`) : salut 15 s, `install` et `uninstall` 10 min, `inspect` 60 s, `render`
30 s, `shutdown` 5 s puis arrêt forcé de l'arbre de processus. Un délai dépassé rend `Timeout`, le processus
est tué et relancé à la requête suivante ; il en va de même quand le service sort ou sort du protocole.

**Tailles** : requête ≤ 1 Mio (au-delà `tooLarge`, la ligne est ignorée et le service continue) ; réponse ≤ 32 Mio
(au-delà `Protocol`) ; instantané dans les limites de `figcraft-dom` ; la capture PNG est écrite sur le disque, la
réponse ne porte que son chemin.

**Sécurité** : le service n'ouvre aucun port (ni HTTP, ni WebSocket) ; Chromium est lancé par Playwright en mode
pipe, sans port de débogage. Le nom du paquet et la version sont validés côté Rust puis côté Node (nom de
registre npm, semver exacte ou `latest` ; ni chemin, ni URL, ni `git+`, ni alias, ni plage). `npm` tourne
toujours avec `--ignore-scripts` (et `npm_config_ignore_scripts=true` dans son environnement, qui l'emporte sur un
`.npmrc` du projet), `--save-exact`, `--no-audit`, `--no-fund`.

**Dossiers** : les paquets vivent dans `<projet>/.uigen/sources/` (`node_modules/` et un `package.json` privé), les
captures dans `<projet>/.uigen/captures/<paquet>/<version>/`. Rien n'est commité ni embarqué. Retirer une
source lance `npm uninstall` et garde les captures ; `uigen.json` n'enregistre une version qu'après une
installation réussie.

**Trouver Node et le script** : `UIGEN_NODE` sinon `node` dans le `PATH` (Node 22 ou plus) ; `UIGEN_SIDECAR_DIR`
sinon `<dossier de l'exécutable>/sidecar`, sinon le `sidecar/` du dépôt en build de débogage. Chromium est celui de
Playwright, déjà installé pour le lot 0.

**Essai réel, à la main.** Les tests du crate n'utilisent jamais le réseau : ils jouent un faux service. Deux
tests `#[ignore]` (`crates/capture/src/real_tests.rs`) pilotent le vrai service et **téléchargent depuis npm**,
donc à lancer en connaissance de cause :

```
npm --prefix sidecar ci
cargo test -p figcraft-capture -- --ignored --test-threads=1
```

- E2-8.1 : `install @mui/material 7.3.11` (`framework: "react"`), `inspect` liste `Button`, `render Button
  {variant: "contained"} default light` rend un instantané que `figcraft_dom::parse_snapshot` accepte et un PNG,
  `uninstall` retire le paquet, `shutdown` rend la main avant son délai de 5 s. Limite : le crate n'expose pas le pid
  du fils (et le client tue l'arbre de processus en se libérant), donc l'absence de processus `node` ou Chromium
  restants se vérifie à l'œil dans le gestionnaire de tâches, pas par le test.
- E2-8.2 : `install @uigen-test/absent-404 1.0.0` rend `Install` avec un message lisible ; `render` d'un composant
  `Nope` rend `ComponentNotFound`.

## Lots

### Lot 0 · Essai de faisabilité (risque principal d'abord)
Avant toute interface : le service capture le Button de MUI, shadcn/ui et Chakra en 2 états, et
`uigen-dom` le traduit en calques dans un fichier `.figcraft`. Critère : les trois boutons s'ouvrent
dans UIGEN. Critère : les formes s'écartent de moins de 1 % de pixels de leur capture ; le texte ne
se juge pas au pixel (les glyphes diffèrent entre DirectWrite et le moteur de texte d'UIGEN) mais sur
la police (famille, installée dans UIGEN), la taille et la graisse identiques à celles de Chromium, la
position (x, y) de la couche à 1 px près de sa boîte capturée et sa largeur, mesurée par le moteur
de texte d'UIGEN, à 1 px près de la largeur de cette boîte, l'écart pixel du texte restant affiché à
titre d'information. Si l'essai échoue, on revoit l'approche avant d'aller plus loin.

### Lot 1 · Projet de lib
1. `uigen-project` : lecture et écriture de `uigen.json` (sources, versions, préfixe des balises,
   dossier de sortie Lit, thème de tokens appliqué), dossier `.uigen/`.
2. Ouvrir un dossier de projet depuis l'accueil ; ses fichiers de design en sont les pages.
3. Règle page = composant : `page.add` crée « Composant N » ; renommer la page renomme le composant ;
   supprimer la page supprime le composant ; annotations (capture, notes, mesures) hors composant.
4. Toast avec action (« Annuler »), nouveau dans `app/frame.rs`.
5. Rail : « Ressources » devient « Composants » (22 langues).

### Lot 2 · Service de capture et installation
- [x] 1. `sidecar/` : script Node (install, inspect, render) ; dans un premier temps Node et Chromium
   du poste en développement, embarqués au lot 8.
- [x] 2. `figcraft-capture` : démarrage, arrêt, délais, erreurs lisibles (installation, rendu, composant
   introuvable), sans bloquer l'interface (fil dédié) ; port `CaptureBackend` dans le moteur, commandes
   `project.addSource`, `project.setSourceVersion`, `project.removeSource`, `capture.status`.
- [x] 3. Réglages du projet (`project.settings`), section « Bibliothèques sources » : ajouter, changer de
   version, retirer (captures gardées, « source retirée »).
- [ ] 4. Essai réel (`crates/capture/src/real_tests.rs`, E2-8.1 et E2-8.2) : écrit et compilé, à lancer à la
   main (voir « Service de capture »).

### Lot 3 · Traduction en calques
1. `uigen-dom` : boîtes → cadres, flex → auto layout, textes, SVG (`figcraft_svg::import_svg`),
   fonds, bordures, ombres, rayons, opacité, découpe ; nettoyage des enveloppes vides ;
   ce qui ne se traduit pas devient une note sur le calque, jamais une invention.
2. Jeu de variantes Style × Taille × État (`ComponentSet`, noms `Style=…, Size=…, State=…`), mêmes
   noms de calques entre variantes, mise en grille.
3. Thème de tokens : variables de la lib (noms exposés, sinon par rôle), modes clair et sombre,
   calques liés (`variable.bind`).
4. Transitions → interactions de prototype `ChangeTo` (durée, courbe) ; ripple et spinner reconnus.
5. Un import = un seul pas d'annulation (`Session::grouped`).

### Lot 4 · Onglet Composants et glisser-déposer
1. Sections « Importé · <lib> <version> » dans le panneau (`assets_panel.rs`, `build_listing`).
2. Fenêtre « Importer un composant » (source, composants, états, thèmes, tailles, animations).
3. Glisser sur une page : copie indépendante, la page prend le nom ; sur une page occupée,
   remplacement, l'ancien reste dans la lib, toast « Annuler ».

### Lot 5 · Thèmes de tokens et Comparer
1. Réglages du projet, section « Thèmes de tokens » : choisir le thème appliqué à la lib.
2. Onglet « Comparer » (`RightTab::Compare`) : original et version de la lib côte à côte par état,
   écart en pixels (repart de `dev_compare.rs`).
3. Mise à jour d'une source : recapture et liste des différences, appliquées seulement sur demande.

### Lot 6 · Export de la lib en Lit
1. `uigen-lit` (L4, nouveau, dépend de `doc`) : composant (jeu de variantes) → modèle intermédiaire
   (propriétés de variante, états, styles, slots), en Rust pur, testé sur des fichiers `.figcraft`
   enregistrés.
2. Générateur : modèle → `components/<balise>.ts` (LitElement, `@property` réfléchies, `static
   styles` avec `var(--…)`, états en sélecteurs `:hover`, `:focus-visible`, `:active`,
   `[disabled]`, `[loading]`), transitions et ripple en CSS.
3. `tokens.css` depuis le thème appliqué (clair sur `:root`, sombre sur
   `@media (prefers-color-scheme: dark)` et `[data-theme="dark"]`), `index.ts`, `package.json`,
   `README.md` ; noms de balises validés.
4. Écriture dans le dossier de sortie : seuls les fichiers générés sont remplacés (manifeste
   `.uigen/export.json`).
5. Fenêtre « Exporter la lib en Lit » (menu du fichier, Ctrl+Maj+E) : composants cochés, réglages,
   arborescence produite, aperçu du code, notes d'export.
6. Vérification : le service de capture rend chaque composant exporté, l'écart en pixels avec le
   calque s'affiche dans Comparer.

### Lot 7 · Playground
1. Onglet « Playground » : vue web (Chromium du service de capture) qui charge l'export Lit en
   cours et un modèle.
2. Modèles Composants, Tableau de bord, Mail, Chat, Finances, CRM, écrits avec les balises de la
   lib ; un composant absent est marqué, jamais remplacé.
3. Barre de réglages (Accent, Base, Police, Rayon, Forme du rayon, Thème) : chaque réglage écrit
   des tokens du thème en mémoire et les pousse au modèle sans rechargement.
4. Barre du haut : annuler, rétablir, revenir, hasard, clair / sombre, Claude, partager, code.
5. Enregistrer les réglages en thème de tokens (fenêtre Variables).

### Lot 8 · Livraison
Node et Chromium embarqués dans l'installeur Windows, mesure du poids, licences affichées.

## Hors plan

Appliquer ses propres variables CSS, publication du paquet sur un registre npm : fiches suivantes.
