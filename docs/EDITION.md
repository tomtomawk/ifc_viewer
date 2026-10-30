# Éditeur BIM natif

Le document `.modele.json` reste la source de vérité. Les ajouts de cette itération
utilisent le format v0. Aucun fichier RVT n'est écrit. Pendant l'alpha, le genre
d'ouverture libre est `libre` ; l'ancien nom `baie` n'est plus accepté.

## Parcours disponible

- **Début → Importer** convertit un IFC/RVT/RFA/RTE dans un nouveau document natif,
  conserve les documents ouverts et écrit le rapport `.rapport.md` à côté du JSON.
  Les murs courbes, ouvertures et maillages importés sont maintenant affichables.
- **Saisie validée** : un nom ou une valeur tapé dans un champ (propriétés, niveaux,
  matériaux, feuilles…) ou un nombre glissé n'est appliqué au modèle qu'à la fin de la
  saisie (Entrée, clic ailleurs, fin du glisser) : la scène est alors reconstruite et
  la modification forme une seule étape d'historique. Échap abandonne la saisie et
  rétablit l'ancienne valeur (sans quitter l'outil en cours).
- **Modéliser → Niveau** liste les niveaux (nom et altitude modifiables) ;
  « Ajouter un niveau » crée « Niveau n » au-dessus du plus haut, à la hauteur du
  dernier étage (3 m s'il n'y a qu'un niveau). **Quadrillage** crée des axes
  droits, visibles dans les plans natifs. Les dimensions du modèle sont saisies en
  centimètres par défaut ; Début → Options permet de choisir mm, cm ou m. Le JSON,
  les commandes CLI/MCP et les tailles sur papier restent en millimètres.
- **Modéliser → Poteau / Poutre** crée des structures rectangulaires en saisissant
  leurs dimensions et leur niveau. Placez un poteau par un clic dans la vue ;
  pour une poutre, cliquez son départ puis son arrivée. Le placement utilise les
  accrochages et Maj contraint la poutre aux axes du plan ; Échap ou clic droit
  annule un départ. La saisie par coordonnées reste disponible. Les poteaux sont
  verticaux et orientables ; les poutres sont horizontales et définies par leur axe.
  Après sélection, **Modifier → Propriétés structurelles** permet de les modifier ;
  les commandes de déplacement, rotation, copie, suppression et l'historique leur
  sont applicables. Leurs quantités et classes `IfcColumn` / `IfcBeam` sont exportées.
- **Ouverture** place une porte, une fenêtre ou une ouverture libre par clic sur un mur ou par
  abscisse depuis son départ. Une ouverture d'assemblage perce toutes ses couches. Une
  menuiserie importée conserve son maillage et suit son hôte ; ses dimensions sont
  verrouillées tant qu'elle n'est pas remplacée explicitement par un cadre natif.
- **Sélection** : sélectionner un élément (sans outil en cours) ouvre l'onglet
  Modifier ; tout désélectionner (clic dans le vide, Échap) rend l'onglet d'avant.
  Le panneau Propriétés commence par le sélecteur de type de l'élément : mur composé
  (couches reconstruites d'après le type choisi ; une couche sélectionnée montre le
  type de son mur), mur seul (types à une couche), élément horizontal, porte ou
  fenêtre (dimensions du type).
- **Modifier** propose déplacement, copie, rotation, alignement des axes et
  ajustement de l'extrémité d'un mur sur l'axe d'un autre. Ctrl+clic constitue une
  sélection multiple, surlignée dans la vue. On peut aussi
  tracer un rectangle en glissant depuis le vide ou en cliquant deux coins : de
  gauche à droite (bleu), seuls les éléments entièrement inclus sont retenus ; de
  droite à gauche (vert), tous les éléments touchés sont retenus. Ctrl ajoute à la
  sélection ; Échap ou clic droit annule le rectangle. Les objets visibles à travers
  d'autres sont inclus, tandis que les éléments masqués et les parties découpées
  sont exclus. Le glissement sur un objet conserve la navigation 3D. Copier un assemblage recrée les
  identifiants de ses couches et ouvertures. Les repères de coupe/élévation et les
  quadrillages se sélectionnent aussi en plan, ainsi que les annotations dans leur
  vue ; leurs propriétés sont accessibles dans Modifier. Les cotes liées gardent
  leurs références : leur décalage est éditable dans leurs propriétés. Supprimer (icône de
  Modifier, touche Suppr ou Retour arrière hors tracé) agit tout
  de suite, couches et ouvertures hébergées comprises ; Ctrl+Z rétablit. Un niveau
  encore utilisé ne peut pas être supprimé.
- Le panneau Propriétés ne montre que les informations de la sélection (type,
  paramètres, couches, sommets d'un élément horizontal). Les commandes sont ailleurs :
  Cadrer, Isoler et Masquer dans le groupe Visibilité de Modifier (touches F, I, H) ;
  dans la barre d'options en haut de la vue, les réglages de Déplacer, Copier,
  Tourner, Aligner et Ajuster, et les commandes propres à la sélection : dissocier
  une couche ou prendre son assemblage, associer un mur à un assemblage, jonctions
  et haut des murs (attache à une toiture, priorité d'une couche aux croisements),
  mise à jour depuis le type, trémie ou contour rectangulaire d'un élément
  horizontal. Modifier un contour réinitialise ses pentes par arête.
- **Annuler / Rétablir**, Ctrl+Z et Ctrl+Maj+Z, ont un historique propre à chaque
  document. Les flèches après Sortie annulent ou rétablissent une étape ; leurs
  flèches donnent accès à la liste pour remonter ou rétablir plusieurs étapes.
  Les intitulés décrivent les éléments modifiés lorsqu'ils peuvent être identifiés.
  Un glissement est regroupé en une seule modification. L'historique
  retient jusqu'à 100 instantanés, avec une cible de 64 Mio (au moins le dernier
  instantané est conservé, même s'il dépasse cette taille).
- **Enregistrer / Enregistrer sous** valident le modèle et remplacent le fichier
  atomiquement. Chaque modification appliquée écrit une copie `.recuperation.json`.
  À la réouverture, l'application propose de la restaurer. Enregistrer ou fermer
  explicitement sans enregistrer supprime cette copie de récupération.
- **Dessin 2D** (Documenter) : ligne, polyligne, spline, cercle et arc se tracent
  directement dans la vue native ouverte, à la manière de CADCraft ou d'AutoCAD.
  Clic : point suivant ; clic droit, Entrée ou Espace : terminer ; F puis Entrée :
  fermer la polyligne ou la spline ; Échap : effacer la saisie, puis abandonner le
  tracé en cours, puis quitter l'outil. La ligne enchaîne les segments ; le cercle
  se donne par centre et rayon, l'arc par départ, point de passage et fin ; la spline
  passe par ses points (Catmull-Rom centripète). Accrochages : extrémité, milieu,
  centre, quadrant, intersection, perpendiculaire et point proche, sur les dessins
  de la vue et, en plan, sur les contours et axes des murs, les contours des éléments
  horizontaux du niveau et les quadrillages. Sans accrochage, repérage polaire tous
  les 15° ; Maj force l'orthogonal. Au clavier, la ligne de commande en bas de la vue
  reçoit `x,y` ou `x;y` (absolu), `@dx,dy` (relatif), `d<angle` ou `@d<angle`
  (polaire depuis le point précédent) ou une distance seule (dans la direction du
  curseur), dans l'unité d'affichage. Chaque forme achevée devient une annotation
  de la vue. Le même moteur (`dessin2d.rs`) trace les murs (ligne, rectangle, arc,
  cercle ; voir le README) : un arc donne un mur courbe (`axe` en arc), un cercle deux
  demi-cercles, chaque forme une seule étape d'annulation.
  Les éléments horizontaux (dalle, sol, plafond, toiture) l'utilisent aussi :
  polygone, rectangle ou cercle sur le plan de leur altitude.
- **Documenter** crée aussi textes, zones rectangulaires, cotes et étiquettes
  dans la vue native ouverte. Avec un mur sélectionné, une cote référence ses
  extrémités ; sinon ses points sont fixes. Une référence supprimée est signalée
  en rouge, sans supprimer silencieusement la cote.
- **Feuilles** (Documenter, groupe Documents, ou l'arborescence) ouvre un jeu de
  feuilles dans un onglet : toutes ses feuilles l'une sous l'autre, comme les pages
  d'InDesign, avec cadre et cartouche (projet, nom, cartouche libre, échelles,
  numéro). Glisser un plan, une coupe ou une élévation depuis l'arborescence sur une
  feuille la place, centrée sous le curseur ; glisser une vue placée la déplace ;
  double-clic l'ouvre ; Suppr la retire (ou supprime la feuille choisie). La page en
  pointillés ajoute une feuille. Glisser le titre d'une feuille (au-dessus de la page)
  la remet à un autre rang du jeu (barre bleue d'insertion) ; dans l'arborescence,
  glisser une feuille au-dessus ou au-dessous d'une autre la déplace, la déposer sur
  le nom d'un jeu la met à la fin de ce jeu. Ctrl+Z annule. Molette : zoom ; glisser le fond : défiler. Les
  réglages sont dans Propriétés, sans boîte de dialogue : nom du jeu (Entrée pour
  renommer), Ajouter une feuille, Nouveau jeu, Exporter le jeu en PDF (un fichier, une
  page par feuille dans l'ordre du jeu, chacune à son format ; aussi Sortie → Jeu PDF
  et la commande du pilote `exporter_jeu <chemin.pdf> [jeu]`) ; feuille (numéro, nom, format A0 à A4,
  paysage/portrait, dimensions, cartouche, export PDF/SVG, suppression) ; vue placée
  (échelle, position, ouvrir, retirer). L'arborescence liste tous les jeux, toutes
  leurs feuilles et, sous chaque feuille, ses vues placées (clic : ouvrir la vue) ; clic
  sur une feuille pour l'amener à l'écran. La recherche garde un jeu entier si son nom
  correspond, une feuille si son titre ou une de ses vues correspond. Un fichier
  Revit converti (Début → Importer) apporte ses plans d'étage, coupes, élévations et
  feuilles (voir MODELE.md, conversion depuis Revit).
- **Nomenclatures** (Documenter ou l'arborescence) : quantités par catégorie,
  regroupées par type ou détaillées par élément, colonnes au choix (catégorie, type,
  élément, niveau, nombre, longueur, hauteur, largeur, épaisseur, surface, volume net ;
  regroupées, nombre, longueur, surface et volume sont totalisés), export CSV.
  Glisser une nomenclature de l'arborescence sur une feuille y pose son tableau
  (titre, en-tête, lignes ; coin haut gauche sous le curseur), tenu à jour avec le
  modèle, déplacé à la souris, retiré par Suppr ; il est dans les exports PDF/SVG.
  Sous chaque feuille, l'arborescence liste aussi ses nomenclatures. Les
  nomenclatures posées sur les feuilles Revit ne sont pas encore lues à l'import.
- **Sortie → IFC4** exporte niveaux, classes architecturales, compositions et
  relations d'ouverture avec une géométrie tessellée. Les GUID IFC valides de la
  source sont préservés ; les autres sont générés de manière stable.

L'exemple autonome `samples/maison_native.modele.json` contient deux niveaux, huit
murs, des ouvertures, deux planchers, une toiture, des murs attachés, des annotations,
une feuille A3 et une nomenclature. Les fichiers `samples/test_ifc4*.ifc` couvrent
également les conversions et les murs courbes.

## Géométrie et limites explicites

- Les angles droits/en L conservent les onglets existants. Aux croisements et aux
  raccords courbes, les volumes sont répartis par priorité : structure (30),
  isolation (20), finition (10), autre (0), puis identifiant pour départager. Une
  priorité explicite par mur/couche remplace cette valeur. Il ne s'agit pas d'un
  solveur de contraintes équivalent à Revit.
- Les toitures concaves à **pente uniforme sur toutes les arêtes** utilisent un
  squelette droit, y compris les noues. Les combinaisons concaves de pentes
  différentes ou d'arêtes sans pente sont refusées avec un message ; elles doivent
  être décomposées en plusieurs toitures. Les contours convexes gardent le calcul
  par arête existant.
- L'attachement requiert que le mur soit entièrement dans l'emprise de la toiture
  (prévoir un débord). Détacher retrouve la contrainte haute enregistrée. Supprimer
  la toiture détache ses murs sans les supprimer.
- Le modèle est reconstruit après chaque opération : la fluidité sur des maquettes
  volumineuses reste à optimiser. La copie de récupération n'est pas un journal
  incrémental ; un calcul qui n'a pas encore abouti n'y figure pas.
- L'export IFC utilise des corps tessellés, pas une restitution des familles Revit
  ni de tous les paramètres métier. Les objets importés deviennent des
  `IfcBuildingElementProxy` portant leur catégorie en `ObjectType`.
- Les textes PDF/SVG sont vectorisés en contours, pas en texte sélectionnable.
  Le PDF d'une feuille respecte son format exact ; l'export d'une vue seule ajoute
  une marge. Imprimer à **100 %**, sans « ajuster à la page ».
- Le cadre de porte/fenêtre natif est une représentation simple. Un configurateur
  de familles, les escaliers/garde-corps paramétriques, les symboles,
  légendes et altimétries, la fusion d'import et la comparaison de réimportations
  restent des itérations suivantes. Les sections circulaires et les poutres inclinées
  ne sont pas encore prises en charge. Les outils non raccordés sont désactivés.
- Les feuilles natives s'ouvrent dans le gestionnaire de documents avec un aperçu ;
  elles ne constituent pas encore des onglets de dessin éditables comme les vues.

## Raccourcis clavier

Sans champ de texte actif, taper deux lettres à la suite : `CS` créer à l'identique,
`MU` mur, `PO` poteau, `PP` poutre, `OU` ouverture, `MV` déplacer, `CO` copier,
`RO` tourner, `AL` aligner, `AJ` ajuster. `Ctrl+A` sélectionne les éléments visibles ;
Suppr demande confirmation de suppression. `CS` reprend les paramètres des murs,
éléments horizontaux à pente uniforme, poteaux, poutres, ouvertures natives simples,
quadrillages, coupes, élévations et annotations. Les menuiseries à géométrie importée
et les pentes différentes par arête ne sont pas encore proposées en création identique.
Les commandes Créer à l'identique et Supprimer sont aussi accessibles par leurs icônes dans l'onglet Modifier.
Le sous-projet de base est `A` et la variante par défaut est `Principale` ; le contexte
de la sélection est affiché dans Propriétés.

## Interfaces et commandes

Les commandes UI et MCP passent par les mêmes transactions. Une erreur de commande,
validation ou reconstruction conserve le modèle précédent.

`projet_natif` renvoie le document JSON et ses identifiants natifs. `editer <JSON>`
utilise ces identifiants, **pas** les identifiants numériques de la scène.

```text
editer {"action":"niveau","nom":"Étage","altitude":3000}
editer {"action":"barre","barre":{"id":"","nom":"Poteau","genre":"poteau","niveau":"rdc","debut":[0,0,0],"fin":[0,0,2800],"largeur":300,"hauteur":300,"rotation":0}}
editer {"action":"barre","barre":{"id":"","nom":"Poutre","genre":"poutre","niveau":"rdc","debut":[0,0,2600],"fin":[3000,0,2600],"largeur":200,"hauteur":400}}
editer {"action":"deplacer","ids":["rdc-m0"],"delta":[1000,0,0]}
editer {"action":"copier","ids":["rdc-m0"],"delta":[0,3000,0]}
editer {"action":"tourner","ids":["rdc-m0"],"centre":[0,0],"angle":90}
editer {"action":"ouverture","mur":"rdc-m0","genre":"porte","abscisse":2000,"largeur":900,"hauteur":2100,"allege":0}
editer {"action":"attacher","ids":["etage-m0"],"toiture":"toiture"}
editer {"action":"supprimer","ids":["porte"]}
annuler
retablir
enregistrer /chemin/projet.modele.json
```

Le type `modele::commandes::Commande` décrit aussi `modifier_niveau`, `quadrillage`,
`aligner`, `ajuster`, `actualiser_type`, `priorite_jonction`, `contour`, `tremie`,
`annoter`, `feuille` et `nomenclature`. Les trois derniers acceptent respectivement
un objet `annotation`, `feuille` ou `nomenclature` ; identifiant vide = création,
identifiant existant = modification.

Sans fenêtre, les mêmes documents peuvent être traités en ligne de commande :

```sh
cargo run -- --editer projet.modele.json commande.json sortie.modele.json
cargo run -- --export-ifc projet.modele.json sortie.ifc
cargo run -- --export-vue projet.modele.json plan-rdc plan.pdf
cargo run -- --export-feuille projet.modele.json A01 feuille.pdf
cargo run -- --quantites projet.modele.json quantites.csv Murs
```

La sortie de dessin accepte `.svg` ou `.pdf`. `--editer` remplace le fichier d'entrée
si la destination est omise. Les commandes renvoient un code non nul en cas d'erreur.

Collections v0 ajoutées : `quadrillages`, `annotations`, `feuilles`, `nomenclatures`.
Tables facultatives : `attaches_toitures` (id du mur/assemblage → id de toiture),
`priorites_jonction` (id du mur/couche → entier). Les annotations appartiennent à une
vue ; leurs formes : `texte`, `ligne`, `polyligne` (`points`, `fermee`), `spline`
(`points` de passage, `fermee`), `cercle` (`centre`, `rayon`), `arc` (`points` :
départ, passage, fin), `zone`, `cote`, `etiquette`, en mm dans le repère 2D de la vue ; les positions et tailles des feuilles sont en mm papier. Aucun historique
Undo/Redo n'est sérialisé dans le projet.

## Vérifications

```sh
cargo test --locked
cargo run -- --export-ifc samples/maison_native.modele.json target/maison.ifc
cargo run -- --export-feuille samples/maison_native.modele.json A01 target/maison.pdf
python tools/valider_ifc.py target/maison.ifc
```

Le dernier contrôle nécessite IfcOpenShell et pytest, seulement pour la validation
indépendante (`python -m pip install ifcopenshell pytest`). Les tests essentiels
utilisent les exemples du dépôt ; les tests du corpus RVT externe continuent à être
sautés lorsque ce corpus n'est pas disponible. Pour un serveur graphique de test
sans anticrénelage, `IFC_VIEWER_MSAA=0` sélectionne directement ce mode.
