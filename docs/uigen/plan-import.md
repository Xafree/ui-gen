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
| `inspect` | `dir`, `package` | `{package, version, framework, components: [{name, props: [{name, type, values}]}], notes}` (propriétés typées depuis le lot 3, voir « Instantané et `inspect` enrichis ») |
| `render` | `dir`, `package`, `component`, `props`, `state`, `theme`, `sampleText` (facultatif) | `{snapshot (instantané v1 de `figcraft-dom`), png (chemin relatif à `dir`), note (facultatif)}` |
| `shutdown` | `{}` | `{}` |

`sampleText` est le libellé d'exemple du composant (≤ 256 caractères, `invalidRequest` sinon) ; le moteur y met le
nom du composant (`Button`), pour tous les états, `Loading` compris. Le service l'injecte sans jamais l'écrire dans
du code : en contenu enfant du composant React (`createElement(Composant, props, texte)`), en nœud texte enfant de
l'élément pour un web component (le `<slot>` le reprend). Un composant qui refuse ses enfants (élément vide : React
lève « void element ») fait échouer ce rendu ; le service réessaie **une fois** sans texte et, si cela réussit,
ajoute `note` = `sampleText ignored: the component takes no content`. Un composant qui n'affiche simplement pas ses
enfants ne donne aucune note. Les deux essais tiennent dans les 25 s du service. Le moteur remonte la `note` dans
les `warnings` de l'import, une seule fois.

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

Deux autres tests `#[ignore]` (`crates/capture/src/real_import_tests.rs`, lot 3) font tourner une vraie `Session` du moteur
sur le vrai service, dans un projet temporaire (`project.open {create: true}`, `project.addSource @mui/material
7.3.11`) ; ils téléchargent aussi depuis npm, lancent Chromium et durent environ 100 s chacun :

- E3-8.1 : `uigen.importComponent {component: "Button", styles: ["contained", "outlined"], sizes: ["small",
  "medium"], states: ["Default", "Hover", "Pressed", "Disabled"]}` (thèmes par défaut) aboutit à 16 variantes dans
  un `ComponentSet`, la collection `@mui/material 7.3.11` aux modes `Light` et `Dark`, un calque
  `uigen.effect = "ripple"` et une interaction `OnHover` de durée non nulle ; un seul `edit.undo` vide la page.
- E3-8.2 : le même import avec `component: "Nope"` échoue en `ComponentNotFound` (le message nomme `inspect` et le
  composant) et laisse le document et l'historique d'annulation inchangés.

Lancés à la main le 29 octobre 2026 : `cargo test -p figcraft-capture real_import -- --ignored --test-threads=1`, verts.

Un troisième test (lot 4, `real_staged_import_lands_in_the_import_page_and_placing_it_makes_the_page_a_button_then_archives_it`,
environ 240 s) suit le même chemin avec `stage: true` :

- E4-7.1 : `uigen.listComponents` rend `Button` avec `variant` et `size` (`enum`) puis, au second appel,
  `{cached: true}` ; `uigen.importComponent {…, stage: true}` aboutit à 16 variantes dans la page
  `- Imported · @mui/material 7.3.11` sans toucher à la page courante, et un `Chip` s'y range à côté ;
  `uigen.placeImported` du `Button` sur la page blanche la rend page `Button` de 16 variantes
  (`replaced: null`) ; celui du `Chip` sur cette page archive le premier dans une page `Button` insérée juste
  après (l'ancien nom est gardé tel quel parce qu'il diffère du nouveau ; il serait `Button 2` pour deux
  jeux de même nom) ; deux `edit.undo` rendent l'état d'avant les dépôts (page blanche, import intact).

Lancé à la main le 30 octobre 2026, vert. Le worktree du lot n'a pas de `sidecar/node_modules` : le test se
lance avec `UIGEN_SIDECAR_DIR` sur un `sidecar/` dont les dépendances sont installées.

### Instantané et `inspect` enrichis (lot 3)

Le protocole reste en **version 1** : le lot 3 n'ajoute que des champs facultatifs, aucun n'est retiré, et un
instantané du lot 0 se relit à l'identique.

- **`inspect`** lit les propriétés typées de chaque composant : `props: [{name, type, values}]`, `type` ∈ `enum`
  (union de littéraux chaîne ; `values` dans l'ordre de déclaration), `boolean`, `string`, `number`, `other`
  (`values: null`). Les types viennent des `.d.ts` du paquet installé, lus avec l'API du compilateur
  `typescript` ; le code du paquet n'est jamais exécuté dans Node. Sont écartés `children`, `className`, `style`,
  `ref`, `key`, les `on[A-Z]…`, les props de type fonction et celles qui ne viennent que de React ou du DOM.
  Au plus 64 props par composant et 64 valeurs par prop. Bornes de lecture : 20 s, 4 000 fichiers, 64 Mio, 512 Mio de
  mémoire, confinée à `.uigen/sources/node_modules/` (un lien qui sort du paquet est tenu pour inexistant) ; la lecture
  tourne dans un `worker_thread` interrompu à l'échéance, et la taille d'un fichier est lue avant le fichier. Une borne
  atteinte ou un paquet sans types rend `props: []` et une phrase dans `notes`, jamais une erreur. Composant web :
  `custom-elements.json`.
- **`style`** d'un élément gagne les clés de mise en page `flexDirection`, `flexWrap`, `justifyContent`,
  `alignItems`, `alignContent`, `alignSelf`, `flexGrow`, `flexShrink`, `rowGap`, `columnGap`, `paddingTop`,
  `paddingRight`, `paddingBottom`, `paddingLeft`, `position`, `boxSizing` et `gridTemplateColumns`.
- **`className`** : l'attribut `class` (256 caractères au plus). **`vars`** : pour chaque clé de `style` dont la
  déclaration d'auteur gagnante est un `var(--nom…)` nu, `{"<clé>": "--nom"}` (64 entrées au plus, nom
  `--[A-Za-z0-9_-]{1,128}`). **`animations`** : les animations CSS en cours (`element.getAnimations()`, 16 au
  plus) `{name, durationMs, easing, iterations (nombre ou "infinite"), properties}` ; un nœud `svg` porte les mêmes
  `animations` (un spinner est souvent un `<svg>` qui tourne, par `transform` ou par la propriété `rotate`).
- `parse_snapshot` (`figcraft-dom`) valide ces champs à la frontière et refuse l'instantané en nommant le nœud :
  `vars` au plus `MAX_VARS` (64) entrées dont le nom suit `MAX_VAR_NAME_CHARS` (128) ; `animations` au plus
  `MAX_ANIMATIONS` (16) par élément ou `svg` ; `className` au plus 256 caractères ; `name` et `easing` d'une animation
  au plus 256 caractères ; `properties` d'une animation au plus 64 entrées de 128 caractères. Le service coupe ces
  textes avant d'écrire l'instantané ; seul un instantané qui ne vient pas de lui est refusé.

### Traduction et assemblage (lot 3)

- **`figcraft_dom::translate_snapshot(doc, snapshot, parent, x, y, &TranslateOptions)`** (L4, pur) traduit
  **un** instantané : boîtes en cadres (fonds, bordures, ombres, rayons, opacité, découpe), flex en auto layout quand
  le moteur de mise en page retrouve les boîtes du navigateur à 1 px près (sinon les enfants restent où ils ont été
  capturés et une note le dit), textes dans la police installée la plus proche, SVG par `usvg`, enveloppes vides
  retirées, frères homonymes suffixés ` 2`, ` 3`. Il rend `Translated {root, warnings, layers}` ; chaque
  `LayerFacts` donne le chemin du calque, les valeurs liables en tokens (`TokenFact`, avec le nom `--…` exposé
  quand `vars` en porte un), la transition lue (`TransitionFact`) et l'effet reconnu (`Effect::Ripple` ou
  `Spinner`). Ce qui ne se traduit pas devient une annotation Dev Mode « UIGEN import: … » sur le calque.
- **`uigen.importDom`** garde la traduction du lot 0 par défaut ; `full: true` appelle `translate_snapshot`.
- **`uigen.importComponent`** (moteur, `crates/engine/src/uigen_import/`) : `plan` lit `inspect` et fixe les axes
  (Style, Taille, États) et la grille ; `pending` enchaîne les travaux (`inspect`, puis un `render` par variante et
  par thème) et rend un seul évènement ; `assemble` traduit chaque variante, en fait un composant, les combine en
  `ComponentSet`, les place en grille (pas de 20 px) et marque le jeu `plugin_data["uigen.source"]` ; `tokens`
  crée ou réutilise la collection `<paquet> <version>` (modes `Light` et `Dark`) et lie les calques ; `motion`
  pose les interactions `ChangeTo` et les effets (`plugin_data["uigen.effect"]`). Le tout s'écrit dans un seul
  `Session::grouped("Import component")` : un pas d'annulation. La commande est décrite dans
  [`docs/mcp.md`](../mcp.md#component-import).

### Limites connues (lot 3)

- La cascade de `vars` ignore `@layer` et le CSS imbriqué : une déclaration posée dans l'un d'eux n'est pas
  comptée. Seul le nom exposé (`--nom`) est perdu ; la valeur du calque reste celle que Chromium a calculée.
- Les animations terminées avant la capture ne sont pas relevées (`getAnimations()` ne rend que celles en cours) ;
  le ripple est reconnu par sa classe (`ripple`, casse ignorée), pas par son animation.
- Les chemins de calques écrivent un `/` d'un nom `\/` et un `\` d'un nom `\\`, pour que deux calques distincts
  n'aient jamais le même chemin ; les noms affichés ne changent pas.

### Playground (lot 7)

L'onglet « Playground » montre une page web vivante, construite avec l'export Lit du projet, dans le
Chromium du service de capture. **La vue est faite d'images relayées** : le service garde la page
ouverte, l'interface en affiche les images (JPEG) et lui relaie la souris, la molette et le clavier.
Aucun port, aucune fenêtre tierce, aucune dépendance Rust nouvelle.

| Rôle | Ce qu'il fait |
|---|---|
| Moteur (L5, `crates/engine/src/assets/playground/`, commandes `cmd/playground.rs` et `cmd/playground_settings.rs`) | Possède l'état : export en cours (instantané du document de l'onglet actif à l'ouverture), modèle, réglages, historique, cibles des réglages. Compose le modèle en balises de la lib, écrit le dossier de staging, dit au service quoi pousser. |
| Interface (L6, `crates/ui-egui/src/panels/playground*.rs`) | Demande une image toutes les 33 ms (250 ms quand le service est occupé), l'affiche, traduit la souris, la molette et le clavier en évènements `playground.input`. Barre du haut, barre de réglages, fenêtres « Enregistrer le thème » et « Code ». |
| Service (Node, `sidecar/playground.mjs`, `sidecar/lit-bundle.mjs`) | Bundle le paquet Lit exporté (esbuild en mémoire, `lit` pris dans `sidecar/node_modules`, jamais dans le projet), tient **une seule** page, pousse les feuilles de style sans recharger, rend les images, applique les entrées. Ne connaît ni le document ni les noms de composants : il lit des fichiers de staging. |

**Staging** : `<projet>/.uigen/playground/` contient `lib/` (le paquet Lit exporté du thème appliqué, par
`lit_export::plan` puis `write(force)` ; le dossier de sortie Lit du projet n'est jamais touché) et
`template.html` (le modèle composé). Le moteur ne l'écrit que par `file_access` : le dossier doit être dans les
racines d'écriture de l'automatisation. Le thème appliqué est `appliedTheme` de `uigen.json`, à défaut la première
collection du document ; sans collection, `playground.open` refuse.

**Protocole du service**, toujours en **version 1** (méthodes nouvelles, aucune méthode ni champ existant ne
change ; un service plus ancien répond `invalidRequest` « unknown method "playground.open" »). Les appels du
playground ne sont jamais listés par `capture.status`.

| Méthode | Paramètres | Résultat |
|---|---|---|
| `playground.open` | `dir`, `width` (320 à 3840), `height` (240 à 2160), `scale` (1 à 2), `theme` (`light` / `dark`), `tags` (au plus 256) | `{defined, missing, errors, shape}` (`shape` : Chromium connaît `corner-shape`) |
| `playground.setTemplate` | `dir` | `{}` : remplace le `<body>` par `template.html`, garde les définitions et les feuilles |
| `playground.apply` | `tokens?` (≤ 512 Kio), `overrides?` (≤ 256 Kio), `theme?`, `shape?` (`round` / `squircle` / `bevel`) | `{}` : remplace le texte des `<style id="uigen-tokens">` et `<style id="uigen-overrides">`, sans recharger la page |
| `playground.resize` | `width`, `height`, `scale` | `{}` |
| `playground.frame` | `after`, `format?` (`jpeg` par défaut, `png`) | `{seq, changed: false}` ou `{seq, changed: true, format, width, height, data}` (base64) ; attend au plus 40 ms une image plus récente |
| `playground.input` | `events` (1 à 64) | `{}` |
| `playground.close` | `{}` | `{}` (idempotent) |

Évènements d'entrée : `move`, `down` / `up` (boutons `left`, `middle`, `right`), `wheel` (|dx|, |dy| ≤ 4000),
`key` (liste fermée : Enter, Tab, Escape, Backspace, Delete, flèches, Home, End, PageUp, PageDown, Space ; modificateurs
shift, ctrl, alt), `text` (≤ 256 caractères, sans caractère de contrôle). Une image dépasse au plus 8 Mio (sinon
`tooLarge`). Délais côté client : `open` 45 s, `frame` 5 s, les autres 10 s ; un délai dépassé tue le service et le
playground passe à `Failed`. La page est isolée comme celle de `render` : contexte hors ligne, requêtes distantes
bloquées, fenêtres surgissantes fermées, `alert` / `confirm` refusés, clic sur un lien hors de la page sans effet ;
le module n'est exécuté que dans Chromium, jamais dans Node.

**Commandes du moteur** (aucune n'est annulable sauf `saveTheme`) : `playground.open {template?, width?, height?,
scale?}` et `playground.refresh` (rendent `{job}`, dernier évènement `playground.open` ou `playground.refresh`),
`playground.close`, `playground.template {name}`, `playground.resize`, `playground.state`, `playground.input`,
`playground.set`, `playground.undo`, `playground.redo`, `playground.revert`, `playground.random {seed?}`,
`playground.saveTheme {name, update?, apply?}` et `playground.code`. Détail : [`docs/mcp.md`](../mcp.md#playground).

**Réglages** : Accent, Base, Police, Rayon, Forme du rayon, Thème, mode clair / sombre. Ils ne changent pas le
document : ils produisent une feuille de surcharges (propriétés personnalisées sur `:root[data-theme="light"]` et
`:root[data-theme="dark"]`) poussée par `playground.apply`, avec une pile d'historique propre au playground (100 étapes,
distincte de `edit.undo`). Chaque réglage vise des **cibles** détectées par règles : Accent, les couleurs dont le nom
contient `primary`, `primaire` ou `accent` (sans `contrast`, `text`, `foreground`, `on`…) ; Base, les couleurs neutres
(chroma OKLCH ≤ 0,04, luminosité de 0,1 à 0,985) ; Police, les chaînes de portée `FONT_FAMILY` ou nommées `font` /
`police` avec `family` / `famille` ; Rayon, les nombres de rôle `Radius`. Un réglage sans cible est grisé avec sa
raison, jamais inventé. La forme du rayon (`corner-shape`) n'agit que dans l'aperçu : elle ne passe ni dans l'export Lit ni
dans le thème enregistré. `playground.saveTheme` écrit les surcharges dans une nouvelle collection (chaque mode) ou
réécrit le thème choisi, en un seul pas d'annulation, après une confirmation dans l'interface.

**Modèles** : fragments HTML de `crates/engine/src/assets/playground/templates/` dont les emplacements
`<uigen-use name="Button" …>` deviennent les balises de la lib ; « Composants » est généré (une instance par variante
de chaque composant). Un composant absent donne `<div data-uigen-missing="Chart" role="note">Composant absent de la lib :
Chart</div>`, jamais un élément natif ni un composant externe.

**Limites connues (lot 7)**

- La page n'est pas un widget d'UIGEN : **pas de presse-papiers** (ni copier, ni coller depuis ou vers la page),
  **pas de sélection de texte** dans la page, **pas d'arbre d'accessibilité de la page** (les lecteurs d'écran ne
  voient que l'image et son libellé), pas de composition IME (le texte arrive par évènements `text`).
- Un seul playground par `Session` ; il vise le document de l'onglet actif à l'ouverture, passe à `stale` quand
  ce document change (bouton « Actualiser » = `playground.refresh`) et se ferme avec son onglet.
- Pendant une installation, une inspection ou un rendu, le service ne sert pas le playground : la vue garde la
  dernière image et le dit.
- **Le contenu placé dans une balise de la lib n'apparaît que si le composant a un `<slot>` par défaut.** Les modèles
  Tableau de bord et Finances imbriquent du contenu (dont `Chart`) dans `Card`. L'export Lit donne donc un `<slot></slot>`
  par défaut, après ses propres calques, à la racine de tout composant qui peut contenir du contenu : un cadre (ou un
  composant) en auto layout, ou qui a au moins un calque qui n'est pas une simple forme (rectangle, ellipse, polygone,
  étoile, ligne, vecteur, opération booléenne). Une icône faite de formes, ou un cadre vide sans auto layout (un champ
  sans enfant), n'en a pas ; les slots nommés et les échanges d'instance gardent leur `<slot name>`. Un composant sans
  slot par défaut ne dessine pas ce contenu, et le marqueur « composant absent » qu'il porte non plus ; un composant
  **absent** ne cache pas ses enfants.
- `playground.close` ferme la page ; le Chromium du service, partagé avec les rendus, ne s'arrête qu'avec le service.
- Web (wasm) : `playground.open` répond « capture service unavailable in the browser » et l'onglet n'existe pas.

**Essai réel, à la main** (`crates/capture/src/real_playground_tests.rs`, Node et Chromium du poste, aucun réseau) :

```
cargo test --offline -p figcraft-capture real_playground -- --ignored --test-threads=1 --nocapture
```

- E7-8.1 : sur un projet temporaire à trois composants synthétiques dont le fond est lié à des jetons,
  `playground.open {template: "components"}` aboutit, une image JPEG arrive, `playground.set {accent}` change des pixels
  de l'image suivante sans nouvel évènement `playground.open`, `playground.set {mode: "dark"}` fonce le fond, et il ne
  reste aucun processus Node, esbuild ni Chromium sous le test une fois le service arrêté.
- E7-8.2 : le temps entre `playground.set` et l'image qui le montre, sur 20 réglages. Mesuré le 30 octobre 2026 :
  **médiane 34 ms, minimum 30 ms, maximum 56 ms** (le test dit si la médiane dépasse 150 ms ; ce n'est pas le cas, le
  repli « fenêtre Chromium séparée » n'est pas nécessaire).
- E7-8.3 : un `open` sur le modèle Tableau de bord d'une lib sans `Chart` liste `Chart` dans `missing`, le modèle de
  staging porte son marqueur, et l'image change quand `Chart` est ajouté puis `playground.refresh` lancé (le marqueur
  était donc visible).

## Onglet Composants et glisser-déposer (lot 4)

Le moteur (L5) garde toute la logique ; l'interface (L6) affiche et relaie, chaque geste passe par
`app.run`. `figcraft-dom` et `figcraft-project` ne changent pas. Les commandes sont décrites dans
[`docs/mcp.md`](../mcp.md#component-import).

- **Pages d'import.** `uigen.importComponent {stage: true}` écrit le jeu de variantes dans la page
  `- Imported · <paquet> <version>` du fichier, créée dans le même pas d'annulation que le jeu et
  retrouvée par `plugin_data["uigen.importPage"]` = `{package, version}`, jamais par son nom (modifiable).
  Son nom commence par un tiret : elle n'est donc jamais un composant de la lib (ni exportée en Lit, ni
  comptée par `project.info`). Un réimport du même composant remplace le jeu précédent à la même place ;
  un autre composant se range à côté ; une nouvelle version de la source a sa propre page. Sans `stage`,
  le comportement du lot 3 est inchangé (CLI, MCP, scripts). `animations: false` laisse de côté les
  interactions `ChangeTo` et les marques `uigen.effect`.
- **Cache d'inspection.** `uigen.listComponents {package, refresh?}` lance `inspect` et garde la réponse
  validée et bornée **en mémoire de la session**, par projet, paquet et version (pas de cache disque : une
  version donnée ne change pas). Un second appel rend `{cached: true, …}` sans travail ;
  `Session::inspected(package)` la relit ; l'invalidation suit `project.setSourceVersion` et
  `project.removeSource` terminés, et `refresh: true` la force. La fenêtre « Import a component » s'en sert
  pour ne pas relancer `inspect` à chaque ouverture ; « Try again » appelle `refresh: true`.
- **Commandes.** `uigen.listComponents` (travail de capture), `uigen.importWindow {package?}` (demande
  `dialog.importComponent` à l'interface, jamais ouverte par un appel programmatique), `uigen.imported`
  (lecture : une section par source du projet non retirée, puis les pages d'import d'une ancienne version
  ou d'une source retirée), `uigen.placeImported {set, page?}` (un pas d'annulation).
- **Copie indépendante.** `placeImported` duplique le jeu entier sur la page cible
  (`Document::duplicate_all_into` : nouveaux identifiants, transitions qui visent les variantes de la
  copie, liaisons de variables, `uigen.source` et `uigen.effect` gardés) ; la page d'import n'est pas
  touchée. La page prend le nom du jeu (`Nom 2` quand un **autre** composant le porte ; son ancien nom à
  elle ne compte pas). Sur une page occupée (`component_roots` non vide), **tout** son contenu (captures
  de référence, notes, mesures, flux compris) part dans une nouvelle page insérée juste après, qui garde
  l'ancien nom (`Nom 2` quand l'ancien nom est le nouveau, casse ignorée) ; le résultat porte
  `replaced: {page, name}`, la copie est posée en haut à gauche de l'ancien contenu et sélectionnée.
- **Panneau Composants.** Les sections « Importé · <lib> <version> » viennent après « Local styles » et
  avant les bibliothèques activées : une par source du projet (vide, avec « Nothing imported from this
  source yet. », tant que rien n'en est importé), puis les anciennes versions et les sources retirées
  (« source removed »). Les jeux importés quittent « Local components ». Hors projet, le panneau est
  inchangé. Chaque section se replie, la recherche ne garde que les jeux qui correspondent. La vignette
  est la variante par défaut du jeu, rendue dans le document de l'onglet. Le menu contextuel d'une tuile
  propose « Copy to the current page » et « Show in the import page » ; le bouton « Import a component… »
  (ligne de recherche et titre de chaque section) lance `uigen.importWindow`, bureau seulement.
- **Glisser.** Une tuile importée se lâche sur le canvas de la page ouverte : « Drop to make it the
  component of this page » sur une page blanche, « Drop to replace {name} on this page » sur une page
  occupée, rien hors du canvas, jamais de cadre ou d'instance mis en évidence (Alt sans effet). Le lâcher
  lance `uigen.placeImported` ; le toast « {old} replaced by {new} » ou « Copied {name} into your library »
  porte l'action « Undo », qui défait l'archivage et le nom en un pas. Une confirmation avant remplacement
  n'existe pas (elle casserait le geste) : l'ancien contenu reste dans la lib et le remplacement s'annule.
- **Fenêtre « Import a component ».** Menu Fichier, palette (« Actions ») et bouton du panneau ; Échap la
  ferme. Elle lit la liste des composants par `Session::inspected` et traite chargement, erreur (« Try
  again »), vide et succès ; sans source, la page « Source libraries » s'ouvre à la place. Mêmes états de succès : (« Show in Components », « Import
  another »). Choisir un composant règle les options depuis ses props ; une ligne « {variants} variants ·
  {renders} renders » suit les cases (`render_count`), et l'import est refusé au-delà de `MAX_RENDERS`
  (128). Elle envoie toujours `stage: true`.
- **Écart à la fiche, validé par l'utilisateur.** « Un projet partagé entre deux fichiers » : les sources
  (`uigen.json`) sont celles du projet et les mêmes dans chaque fichier, donc les deux fichiers montrent
  les mêmes sections ; le **contenu** importé reste propre à chaque fichier (la page d'import vit dans le
  fichier), et un import se refait dans l'autre fichier. Partager aussi le contenu demanderait un second
  format de fichier de projet et une copie entre documents : lot à part.
- **Limites.** La page d'import apparaît dans la liste des Pages ; la supprimer retire les originaux (les
  copies de la lib restent) et « Undo » la rend. Un fichier ouvert sans elle n'a plus de section remplie,
  sans erreur. « Publier ce fichier comme bibliothèque » listerait les jeux d'import comme composants
  locaux.
- **Essai réel** (`#[ignore]`, `crates/capture/src/real_import_tests.rs`, E4-7.1) : voir « Service de
  capture ».

## Thèmes, Comparer et mise à jour (lot 5)

**Références des originaux.** Chaque variante importée porte `plugin_data["uigen.reference"]`
(`ThemeReference` par thème : chemin du PNG sous `.uigen/captures/`, largeur et hauteur de la boîte racine
arrondies, `distinct`, textes avec `box`, `line`, `family`, `size`, `weight`) ; l'ensemble porte
`uigen.importOptions` (styles, tailles, états, thèmes, animations de l'import, rejoués par la mise à jour). Le PNG
reste sur le disque ; le fichier ne garde qu'une référence légère (16 Kio au plus). **Il n'y a pas de couche de capture
verrouillée** : le PNG n'est pas dans le document. `distinct` dit si le rendu sombre diffère du rendu clair **là où
il peint** : les couleurs qui ne peignent rien (bordure sans trait, `caret-color`, `color` d'un élément sans texte)
ne comptent pas.

**Règle d'écart** (`uigen.compare`, `crates/engine/src/uigen_compare/`). Le rendu de la variante est mesuré **sur
sa seule boîte** (`world_bounds`, sans l'ombre qui déborde) contre le PNG, au pixel : seuil de couleur 0,1 (YIQ,
anti-crénelage écarté comme `pixelmatch`). Deux verdicts :

| Verdict | Règle |
|---|---|
| Formes | pixels hors des boîtes de texte : **moins de 1 %** d'écart pour `ok` (le seuil ne s'assouplit jamais) ; les pixels du texte sont une information |
| Texte | par ordre, même famille (casse, ponctuation et suffixes de style ignorés), même taille, même graisse, position et largeur mesurée à 1 px près, police installée, calque à taille automatique |

**Le rendu de la bibliothèque se mesure sur le fond de page de la capture.** Le service de capture photographie
l'élément sur une page opaque (`#ffffff` en clair, `#1e1e1e` en sombre, `BACKGROUNDS` de `sidecar/render.mjs`) : les PNG
sont en RGB sans alpha. Le rendu du moteur est transparent là où la variante ne peint rien (bouton `outlined`) ; avant la
mesure, `uigen_compare::library_raster` le compose sur le même fond (`page_background`, un test lit `render.mjs` pour
garder les deux valeurs égales). Les images des lignes, Superposition et Différence montrent ce même rendu opaque.
`pixel_diff`, `difference_image`, le seuil de 1 % et la tolérance de 1 px ne changent pas.

**La taille d'une variante se compare à 1 px près** (`GEOMETRY_TOLERANCE_PX`, décision de l'utilisateur) : largeur
et hauteur diffèrent chacune d'au plus 1 px, la ligne n'est pas `sizeMismatch`, les deux images sont rognées à leur
intersection avant `pixel_diff` (la colonne en trop n'est pas comptée) et la ligne rend quand même les deux tailles
réelles dans `original` et `library`. Au-delà de 1 px sur l'un des côtés, rien ne change : `sizeMismatch`, mesure sur
l'intersection, pixels hors intersection comptés comme différents. `pixel_diff` et le seuil de 1 % ne changent pas. Le
détail et les images de l'onglet Comparer suivent la même règle : « Size differs » seulement au-delà de la tolérance,
mode Différence rogné à l'intersection.

Statuts d'une ligne : `ok`, `ko`, `sizeMismatch` (taille des deux images différente de plus de 1 px en largeur ou en
hauteur), `missingReference`
(PNG absent, illisible, hors de `.uigen/captures`, de plus de 8 Mio ou de 4096 × 4096), `notComparable` (rendu sombre égal au
clair). **Les lectures sont confinées à `<projet>/.uigen/captures`** même sans racines d'automatisation : un lien ou un
`..` qui en sort donne `missingReference`.

**Le thème s'applique par son nom** : `uigen.applyTheme {theme}` déplace chaque liaison de variable des composants de la
lib vers la variable de même nom et de même type du thème choisi, puis écrit `appliedTheme` dans `uigen.json`.
Références et thème appliqué ont été **validés par l'utilisateur**.

| Commande | Rôle |
|---|---|
| `uigen.themes` | thèmes du fichier (collections locales), origine, calques liés, pastilles |
| `uigen.applyTheme` | applique un thème (un pas d'annulation) |
| `uigen.compare` | une ligne par variante : statut, tailles, écart des formes, verdict du texte |
| `uigen.updateSource` | recapture les composants importés d'une source (travaux de capture, la lib ne change pas) |
| `uigen.sourceDiff` | différences entre la copie et la dernière capture (`clean`, `modified`, `unknown`), calques appariés par leur nom anglais stable (`uigen.stableName`), le nom localisé servant à l'affichage (`label`) |
| `uigen.applyDifferences` | prend des différences dans la copie, sur demande seulement |
| `panel.compare` | demande à l'interface d'ouvrir l'onglet Comparer |

**Essai réel** (`real_import_tests.rs`, E5-8.1, `@mui/material` 7.3.11, Button `contained` `medium`, états Default et
Hover), lancé à la main le 30 octobre 2026. Avant correction, `render_node` rendait l'ombre : bibliothèque 96 × 52 et
111 × 67 pour un original de 82 × 37 (`sizeMismatch`, 67,8 % et 88,0 % d'écart). Après : 81 × 37. Le rendu sombre est
`notComparable` ; effacer les PNG donne `missingReference`, `uigen.updateSource` les restaure ; `uigen.themes` liste
`@mui/material 7.3.11` ; un thème « Brand » d'une variable (`variant-containedBg/default`) ne déplace que cette liaison.

### Limites connues (lot 5)

- **MUI à 1 px de largeur près** : original 82 × 37 (Chromium arrondit la boîte de 81,39 px vers le haut), bibliothèque
  81 × 37 (le texte mesuré par le moteur fait 49 px, Chromium 49,39 px, pour la même face). Avant la tolérance, cette
  seule colonne donnait `sizeMismatch` et des formes à 2,30 %. Avec la tolérance de 1 px (essai réel du 30 octobre 2026),
  les lignes Default et Hover sont `ok` : formes à 0,229 % sur l'aire commune, texte `ok` (`maxDx` 0, `maxDy` 0,75,
  `maxDwidth` −0,39). L'essai réel n'affirme le verdict du texte et le statut que si la police n'est pas en cause
  (« police absente »).
- Une différence de taille de plus de 1 px reste un `sizeMismatch`.
- **Essai réel `outlined`** (`real_compare_of_every_style_size_and_state_of_the_mui_button_matches_its_original`, 8 variantes
  `contained`/`outlined` × `small`/`medium` × Default/Hover, lancé à la main le 30 octobre 2026). Avant la composition sur
  le fond, les quatre lignes `outlined` étaient `ko` (97,57 %, 81,17 %, 98,63 % et 86,95 % d'écart) : le rendu
  transparent s'opposait à un PNG opaque. Après : `contained` 0,0 % (quatre lignes `ok`), `outlined` Default `small` et
  `medium` 0,0 % (`ok`), `outlined` Hover `medium` 0,572 % (`ok`) et `outlined` Hover `small` **1,012 %**
  (10 pixels sur 988) : `ko`. Ces 10 pixels sont les anti-crénelages des quatre coins arrondis (rayon 4) de la bordure,
  pleine `rgb(25, 118, 210)` au survol : le moteur y couvre plus que Chromium (par exemple (2, 0) : 96 contre 140 sur le
  canal rouge ; Default a le même écart, mais la bordure à 50 % le laisse sous le seuil de couleur). Écart de
  rastérisation des coins, sans rapport avec le fond ; rien n'est assoupli, la ligne reste `ko`. Le texte est `ok` sur
  les huit lignes.
- Les états Hover, Pressed ne sont pas forcés sur la bibliothèque : la comparaison mesure le rendu de la variante.
- `uigen.updateSource` recapture dans la version courante de la source ; une version qui change se règle par
  `project.setSourceVersion` d'abord.

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
- [x] 4. Essai réel (`crates/capture/src/real_tests.rs`, E2-8.1 et E2-8.2) : lancé à la main le 29 octobre
   2026, vert (voir « Service de capture »).

### Lot 3 · Traduction en calques
- [x] 1. `figcraft-dom` : boîtes → cadres, flex → auto layout, textes, SVG (`figcraft_svg::import_svg`),
   fonds, bordures, ombres, rayons, opacité, découpe ; nettoyage des enveloppes vides ; ce qui ne se
   traduit pas devient une note sur le calque, jamais une invention (`translate_snapshot`,
   `uigen.importDom {full: true}`).
- [x] 2. Jeu de variantes Style × Taille × État (`ComponentSet`, noms `Style=…, Size=…, State=…`), mêmes
   noms de calques entre variantes, mise en grille (`uigen.importComponent`).
- [x] 3. Thème de tokens : variables de la lib (noms exposés, sinon par rôle), modes clair et sombre,
   calques liés (`variable.bind`).
- [x] 4. Transitions → interactions de prototype `ChangeTo` (durée, courbe) ; ripple et spinner reconnus.
- [x] 5. Un import = un seul pas d'annulation (`Session::grouped`).
- [x] 6. Essai réel (`crates/capture/src/real_import_tests.rs`, E3-8.1 et E3-8.2) : voir « Service de
   capture ».

### Lot 4 · Onglet Composants et glisser-déposer
- [x] 1. Sections « Importé · <lib> <version> » dans le panneau (`assets_panel.rs`, `build_listing`,
   sous-module `assets_panel/imported.rs`).
- [x] 2. Fenêtre « Importer un composant » (source, composants, états, thèmes, tailles, animations ;
   `dialogs/import_component.rs`).
- [x] 3. Glisser sur une page : copie indépendante, la page prend le nom ; sur une page occupée,
   remplacement, l'ancien reste dans la lib, toast « Annuler » (`uigen.placeImported`).
- [x] 4. Essai réel (`crates/capture/src/real_import_tests.rs`, E4-7.1) : import posé dans la page
   d'import, copie sur une page blanche, second dépôt qui archive le premier, deux annulations.

### Lot 5 · Thèmes de tokens et Comparer
- [x] 1. Références des originaux sur chaque variante (`uigen.reference`, `uigen.importOptions`), PNG sur le disque.
- [x] 2. Mesure `uigen.compare` (formes sous 1 %, verdict du texte), `pixel_diff` et `difference_image`.
- [x] 3. Réglages du projet, section « Thèmes de tokens » : `uigen.themes`, `uigen.applyTheme`.
- [x] 4. Onglet « Comparer » (`RightTab::Compare`) : une ligne par état, fenêtre de détail (curseur, différence),
   `panel.compare` (repart de `dev_compare.rs`).
- [x] 5. Mise à jour d'une source : `uigen.updateSource`, `uigen.sourceDiff`, `uigen.applyDifferences`, appliquées
   seulement sur demande (section « Mises à jour » de Comparer).
- [x] 6. Essai réel (`crates/capture/src/real_import_tests.rs`, E5-8.1) : voir « Thèmes, Comparer et mise à jour (lot 5) ».

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
- [x] 1. Onglet « Playground » : vue web (Chromium du service de capture) qui charge l'export Lit en
   cours et un modèle, par images relayées (voir « Playground (lot 7) »).
- [x] 2. Modèles Composants, Tableau de bord, Mail, Chat, Finances, CRM, écrits avec les balises de la
   lib ; un composant absent est marqué, jamais remplacé.
- [x] 3. Barre de réglages (Accent, Base, Police, Rayon, Forme du rayon, Thème) : chaque réglage écrit
   des tokens du thème en mémoire et les pousse au modèle sans rechargement.
- [x] 4. Barre du haut : annuler, rétablir, revenir, hasard, clair / sombre, code. « Claude » et
   « Partager » n'en font pas partie : aucun bouton, aucune commande.
- [x] 5. Enregistrer les réglages en thème de tokens (fenêtre « Enregistrer le thème » : nouvelle collection ou
   mise à jour du thème choisi, jamais sans confirmation).
- [x] 6. Essai réel et mesure de latence (`crates/capture/src/real_playground_tests.rs`, E7-8.1 à E7-8.3) :
   médiane de 34 ms de `playground.set` à l'image, sous le seuil de 150 ms.

### Lot 8 · Livraison
Node et Chromium embarqués dans l'installeur Windows, mesure du poids, licences affichées.

## Hors plan

Appliquer ses propres variables CSS, publication du paquet sur un registre npm : fiches suivantes.
