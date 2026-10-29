# Importer des bibliothèques d'interface

Fiche fonctionnelle UIGEN, étape 1 : installer des bibliothèques d'interface existantes dans un
projet de bibliothèque, puis capturer leurs composants (en commençant par le bouton) pour les
recréer sur le canvas avant de les recoder en web components Lit.

Maquettes : planches « Importer une bibliothèque », « Option C » et « Réglages du projet ».

## Objectif

Partir de composants qui existent déjà (MUI, shadcn/ui, Chakra, Material Web… n'importe quel paquet
npm) pour en reprendre le design et les animations, les comparer, et produire sa propre bibliothèque
UI/UX en Lit.

## Décisions prises

| Sujet | Décision |
|---|---|
| Base | Fork de FigCraft renommé UIGEN ; on ajoute les fonctionnalités dans son code |
| Entrée | Un paquet npm, n'importe quel framework (React, Vue, Svelte, web components…) |
| Capture | Rendu réel observé dans un navigateur invisible, jamais la lecture du code source |
| Variantes | Déduites des types TypeScript du paquet (`variant`, `size`, `color`, icônes…), ajustables |
| États | Par défaut, survol, focus clavier, appuyé, désactivé, chargement |
| Thèmes, tailles | Clair et sombre (modes de variables), toutes les tailles, icône à gauche et à droite |
| Résultat au canvas | Calques éditables : jeu de variantes Style × Taille × État, en auto layout |
| Valeurs | Chaque bibliothèque importée apporte un **thème de tokens** : ses variables (couleurs, rayons, espacements, ombres, typo) avec ses modes clair et sombre, nommées comme la lib les expose, sinon par rôle (`couleur/primaire`, `rayon/bouton`, `espace/padding-x`) |
| Origine des thèmes | Un thème vient d'une bibliothèque source, d'un fichier importé (DTCG, Tokens Studio, variables CSS) ou part de zéro ; chaque thème s'affiche avec un aperçu (couleurs, typo, rayons, espacements, ombres, et la lib rendue avec lui) avant d'être appliqué |
| Thèmes de tokens | Les thèmes sont les collections de la fenêtre Variables du projet (nouveau ou en cours) : « Importer des tokens » en ajoute, « Appliquer » choisit celui de la lib, et on peut en changer à tout moment |
| Animations | Transitions CSS relues entre états, plus les effets connus (ripple, spinner, pulsation) ; une animation inconnue est signalée, jamais inventée |
| Outils | Node.js et Chromium embarqués dans UIGEN |
| Projet | Un dossier : `uigen.json` (réglages, sources, versions), fichiers de design, sources installées dans `.uigen/sources/`, code Lit généré |
| Partage | Les bibliothèques installées appartiennent au projet et servent à tous ses fichiers |
| Rangement | Onglet « Composants » (ancien « Assets ») : sections « Importé · <lib> <version> » ; on glisse un composant importé sur le canvas pour le copier dans sa lib |
| Comparaison | Onglet « Comparer » : original et version Lit côte à côte, écart en pixels |
| Mise à jour d'une source | Recapture et liste des différences ; la version recodée ne change que si on applique |
| Retrait d'une source | Le paquet est désinstallé ; composants importés et captures restent, marqués « source retirée » |
| Réglages du projet | Bibliothèques sources, préfixe des balises, dossier de sortie Lit, tokens |
| Sortie finale | Fenêtre « Exporter la lib en Lit » (menu du fichier, Ctrl+Maj+E) : un web component Lit par page non vide, `tokens.css` (clair et sombre) depuis le thème appliqué, `index.ts`, `package.json`, `README.md`, dans le dossier de sortie du projet |

## Hors périmètre de cette étape

- Appliquer ses propres variables CSS aux composants : étape suivante, juste après l'import.
- Publication du paquet sur un registre npm (l'export produit le dossier prêt à publier, sans le publier).
- Timeline d'animation à images clés.
- Import depuis une URL ou un Storybook.

## User stories

- En tant que concepteur de bibliothèque, je veux ajouter un paquet npm à mon projet afin que tous
  mes fichiers puissent piocher dans ses composants.
- En tant que concepteur, je veux choisir le bouton d'une bibliothèque et le retrouver sur le canvas
  avec toutes ses variantes, ses états et ses transitions afin de le recoder sans le redessiner.
- En tant que concepteur, je veux comparer ma version avec l'original afin de savoir ce qui diffère.

## Critères d'acceptation

- Étant donné un projet ouvert, quand j'ajoute `@mui/material` dans « Bibliothèques sources », alors
  le paquet s'installe dans `.uigen/sources/`, sa version est écrite dans `uigen.json`, et son
  framework détecté s'affiche.
- Étant donné un paquet installé, quand j'ouvre « Importer », alors la liste de ses composants
  s'affiche avec, pour chacun, les variantes lues dans ses types.
- Étant donné le bouton choisi avec les états de base, quand l'import se termine, alors l'onglet
  Composants montre une section « Importé · MUI 6.4.2 » contenant un jeu de variantes dont chaque
  combinaison Style × Taille × État est un calque éditable, et une capture de référence verrouillée.
- Étant donné un bouton importé, quand j'affiche ses transitions, alors chaque passage entre deux états
  porte la durée et la courbe lues dans son CSS ; un ripple de Material est reconnu comme effet.
- Étant donné un paquet qui ne s'installe pas ou un composant qui ne s'affiche pas, quand l'import
  échoue, alors un message dit ce qui a échoué (installation, rendu, composant introuvable) et rien
  n'est ajouté au fichier.
- Étant donné un projet partagé entre deux fichiers, quand j'ouvre le second, alors l'onglet
  Composants montre les mêmes bibliothèques importées.
- Étant donné un composant importé, quand je le glisse de l'onglet Composants sur le canvas de ma
  lib, alors il devient un composant de ma lib, copie indépendante de la source, dont chaque élément
  (cadre, texte, icône, spinner, remplissages, bordures, ombres, auto layout, variantes, propriétés,
  transitions) se modifie avec les outils existants de FigCraft ; la source n'est jamais modifiée,
  et le composant garde la mention de son origine pour la comparaison.
- Étant donné un projet de lib, quand il s'ouvre, alors chaque page est un composant de la lib et
  porte son nom : renommer la page renomme le composant, et le « + » de Pages ajoute un composant
  vide, « Composant N », dans une page blanche.
- Étant donné une page blanche, quand je la dessine de zéro avec les outils existants (cadre, formes,
  texte, auto layout, variantes), alors ce dessin est le composant de cette page, sans étape
  « Créer un composant » ; seules la capture de référence, les notes et les mesures restent hors
  composant et ne sont jamais générées.
- Étant donné une page blanche, quand j'y dépose un composant depuis l'onglet Composants, alors il
  devient le composant de la page et la page prend son nom.
- Étant donné une page, quand je la supprime, alors son composant quitte la lib, et un toast propose
  « Annuler ».
- Étant donné la page d'un composant, quand j'y dépose un autre composant, alors le nouveau remplace
  l'ancien sur la page, la page prend le nom du nouveau, l'ancien reste dans ma lib, et un toast
  « <ancien> remplacé par <nouveau> » propose « Annuler » ; un composant importé déposé devient une
  copie indépendante dans ma lib.
- Étant donné un import terminé, quand j'ouvre les réglages du projet, alors « Thèmes de tokens »
  liste le thème de la bibliothèque importée, avec ses variables et ses modes clair et sombre ;
  les calques importés sont liés à ces variables, jamais remplis de valeurs brutes.
- Étant donné plusieurs thèmes de tokens dans le projet, quand j'en choisis un comme thème appliqué,
  alors tous les composants de la lib prennent ses valeurs, sans modifier leurs calques, et je peux
  revenir au thème précédent.
- Étant donné le tableau des variables, quand il s'affiche, alors chaque token a un aperçu à côté
  de son nom selon son type (couleur : pastille ; rayon : courbe d'angle ; espacement : barre à
  l'échelle ; bordure : épaisseur ; ombre : carte ombrée ; typographie : « Aa » dans sa police ;
  opacité : pastille transparente), puis sa valeur en clair et en sombre ; un token sélectionné
  montre un grand aperçu, sa valeur, les composants qui l'utilisent et son origine dans la source.
- Étant donné une source retirée, quand je rouvre le projet, alors ses composants importés restent,
  marqués « source retirée ».

### Export de la lib en Lit

- Étant donné un projet de lib, quand j'ouvre « Exporter la lib en Lit », alors chaque page non vide
  est listée avec sa balise (`<préfixe><nom-en-kebab>`, par exemple `<ui-button>`) et cochée ; une
  page vide est grisée et jamais exportée.
- Étant donné le bouton exporté, quand j'ouvre `components/ui-button.ts`, alors c'est un
  `LitElement` dont chaque propriété de variante (Style, Taille) est une propriété réfléchie typée
  par ses valeurs, et dont les états (survol, focus, appuyé, désactivé, chargement) sont des
  sélecteurs CSS ou des propriétés booléennes, jamais des variantes figées.
- Étant donné un composant lié à des variables, quand il est exporté, alors son CSS cite les tokens
  par `var(--<préfixe>…)`, jamais leur valeur brute, et `tokens.css` déclare ces tokens pour le
  thème appliqué, en clair et en sombre.
- Étant donné un composant avec des transitions et un ripple, quand il est exporté, alors chaque
  transition devient une `transition` CSS (durée, courbe) et le ripple une animation CSS ; un effet
  non reproductible est signalé dans la fenêtre d'export, jamais omis en silence.
- Étant donné un composant exporté, quand l'export se termine, alors le composant Lit rendu dans
  Chromium est comparé à son calque dans chaque état, et l'écart en pixels s'affiche dans l'onglet
  Comparer.
- Étant donné un composant qui contient une capture de référence, des notes ou des mesures, quand
  il est exporté, alors aucune d'elles n'apparaît dans le code.
- Étant donné un dossier de sortie qui contient déjà un export, quand j'exporte à nouveau, alors
  seuls les fichiers générés sont remplacés ; un fichier ajouté à la main dans le dossier reste.
- Étant donné un nom de page qui ne donne pas une balise valide (vide, chiffre en tête, déjà pris),
  quand j'ouvre l'export, alors la page est signalée et l'export attend qu'elle soit renommée.

### Playground de la lib

Même interface de test que le constructeur de thèmes de HeroUI : barre du haut, modèle au centre,
barre de réglages en bas ; tout est monté avec les composants Lit de la lib.

- Étant donné une lib exportable, quand j'ouvre « Playground », alors un onglet s'ouvre sur un modèle
  (Composants, Tableau de bord, Mail, Chat, Finances, CRM) entièrement construit avec les composants
  Lit de la lib et le thème appliqué.
- Étant donné le playground ouvert, quand je déplace Accent (teinte, ou pipette) ou Base (gris
  neutres), ou que je change Police, Rayon (aucun, petit, moyen, grand, plein) ou Forme du rayon,
  alors les tokens correspondants du thème changent et le modèle se met à jour en direct.
- Étant donné le choix Thème, quand je prends un autre thème de tokens du projet, alors le modèle
  le prend, et les curseurs reflètent ses valeurs.
- Étant donné des réglages modifiés, quand j'utilise ↶ / ↷, alors le réglage précédent ou suivant
  revient ; ⟲ revient au thème appliqué ; ⤮ tire un accent, une base, une police et des rayons au
  hasard.
- Étant donné le sélecteur clair / sombre, quand je bascule, alors le modèle passe au mode sombre
  du thème.
- Étant donné des réglages qui me plaisent, quand j'enregistre, alors ils deviennent un nouveau
  thème de tokens dans la fenêtre Variables (ou mettent à jour le thème appliqué, au choix), jamais
  sans confirmation.
- Étant donné un composant qui manque à la lib pour un modèle (par exemple pas de graphique), quand
  le modèle s'affiche, alors l'emplacement est marqué « composant absent de la lib », jamais rempli
  par un composant externe.

## Questions encore ouvertes

- Licences : afficher la licence du paquet importé et prévenir si elle n'autorise pas la réutilisation.
- Composants qui exigent un contexte (thème, provider) : UIGEN les enveloppe automatiquement ; à
  confirmer sur MUI, Chakra et shadcn.
- Taille de UIGEN avec Node et Chromium embarqués (~300 Mo de plus) : à mesurer.
