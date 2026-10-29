# Plan technique · import de bibliothèques

Réalise la fiche [import-bibliotheques.md](import-bibliotheques.md). Chaque lot se termine par
`cargo xtask ci` vert (fmt, clippy `-D warnings`, tests, i18n, layers, assets, wasm), un commit par
tâche, et l'état de la RAM sous 70 % (`-j 4`, une commande cargo à la fois).

## Architecture

```
uigen.exe (Rust, egui)
 ├─ figcraft-engine  (L5)  commandes project.*, import.*, page = composant, thèmes de tokens
 ├─ uigen-dom        (L4)  NOUVEAU : instantané DOM (JSON) → arbre de calques FigCraft
 ├─ uigen-project    (L3)  NOUVEAU : dossier de projet, uigen.json, verrou des versions
 ├─ uigen-capture    (L6)  NOUVEAU : pilote du service de capture (processus enfant, JSON-lines)
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

## Protocole du service de capture

Une requête par ligne, `{"id","method","params"}` ; réponse `{"id","ok","result"|"error"}`.

| Méthode | Rôle |
|---|---|
| `install {dir, package, version}` | installe dans `.uigen/sources/`, rend la version exacte, la licence, le framework détecté |
| `inspect {package}` | liste les composants exportés et leurs propriétés lues dans les types (`variant`, `size`…) |
| `render {component, props, state, theme}` | affiche une combinaison, force l'état, rend l'instantané DOM (boîtes, styles calculés, textes, SVG, transitions) et une capture PNG |
| `shutdown` | ferme Chromium et Node |

## Lots

### Lot 0 · Essai de faisabilité (risque principal d'abord)
Avant toute interface : le service capture le Button de MUI, shadcn/ui et Chakra en 2 états, et
`uigen-dom` le traduit en calques dans un fichier `.figcraft`. Critère : les trois boutons s'ouvrent
dans UIGEN, à moins de 2 % de pixels d'écart avec leur capture. Si l'essai échoue, on revoit
l'approche avant d'aller plus loin.

### Lot 1 · Projet de lib
1. `uigen-project` : lecture et écriture de `uigen.json` (sources, versions, préfixe des balises,
   dossier de sortie Lit, thème de tokens appliqué), dossier `.uigen/`.
2. Ouvrir un dossier de projet depuis l'accueil ; ses fichiers de design en sont les pages.
3. Règle page = composant : `page.add` crée « Composant N » ; renommer la page renomme le composant ;
   supprimer la page supprime le composant ; annotations (capture, notes, mesures) hors composant.
4. Toast avec action (« Annuler »), nouveau dans `app/frame.rs`.
5. Rail : « Ressources » devient « Composants » (22 langues).

### Lot 2 · Service de capture et installation
1. `sidecar/` : script Node (install, inspect, render) ; dans un premier temps Node et Chromium
   du poste en développement, embarqués au lot 6.
2. `uigen-capture` : démarrage, arrêt, délais, erreurs lisibles (installation, rendu, composant
   introuvable), sans bloquer l'interface (fil dédié).
3. Réglages du projet, section « Bibliothèques sources » : ajouter, changer de version, retirer
   (captures gardées, « source retirée »).

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
