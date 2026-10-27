# Modèle natif v0

Format d'édition du projet, source de vérité d'un futur éditeur. Un fichier Revit ou IFC est
**converti** vers ce modèle : ce qui est reconnu devient paramétrique, le reste devient un
**élément importé** (géométrie figée). Rien n'est jeté sans être compté.

Code : `src/modele/` (types : `mod.rs` ; conversions : `depuis_rvt.rs`, `depuis_ifc.rs` ;
lecture de paramètres dans une géométrie : `geometrie.rs` ; rapport : `rapport.rs`).

```
ifc_viewer --modele <fichier.rvt|.rfa|.ifc> [sortie.json]
```

écrit le projet (`fichier.modele.json` par défaut), le rapport de conversion
(`fichier.modele.rapport.md`) et affiche ce rapport.

## Principes

Dans l'interface, **Début → Nouveau…** (Ctrl+N) crée un fichier `.modele.json`
nommé d'après le fichier choisi et l'ouvre dans une vue 3D vide, en conservant les
autres documents. Il utilise `Projet::nouveau` : en-tête v0, collections vides,
aucun niveau, type ou élément ajouté implicitement. **Sortie → Enregistrer**
(Ctrl+S) enregistre ce document. **Modéliser → Mur** crée des murs droits par deux
clics, avec un aperçu et une palette Propriétés dédiée. La phase, le sous-projet et la variante
se choisissent sur une ligne commune en tête du panneau Propriétés,
indépendamment de l’outil actif. Les trois listes restent
dans la largeur du panneau ; les noms longs sont abrégés et lisibles au survol. Chaque liste contient une entrée « + Ajouter » pour sa catégorie :
phase, sous-projet ou variante (nom et jeu). Cet ajout sélectionne la
nouvelle valeur et la conserve dans le document. Les noms vides et doublons sont
refusés. Ces choix s’appliquent aux prochaines créations. Dans la palette
Propriétés, un croquis du mur en perspective est placé à gauche. Deux traits de rappel
relient ses extrémités aux réglages à droite : niveau haut et décalage en haut,
niveau bas et décalage en dessous. La hauteur utile figure sous le dessin.
Les contrôles restent dans la largeur du panneau. La limite haute est un niveau ou une
hauteur libre (le décalage haut s'ajoute alors à cette hauteur). La hauteur
résultante doit être positive. L'aperçu utilise les mêmes contraintes que le mur
créé. L'épaisseur, l'implantation et le chaînage restent réglables. Un niveau 0 est créé si nécessaire et les types à une couche de
même épaisseur sont réutilisés. L'axe enregistré
reste la ligne médiane de l'épaisseur ; les variantes d'implantation décalent cet axe.
Les niveaux et murs droits natifs sont affichés par la chaîne de rendu commune,
avec sélection et propriétés, et peuvent être rouverts après enregistrement.
L'affichage natif des autres éléments et des murs courbes reste à implémenter :
les fichiers qui en contiennent sont refusés explicitement, sans être modifiés.

Un RVT déjà ouvert peut aussi être enregistré en v0 avec **Sortie → Enregistrer**
ou Ctrl+S. Une destination est demandée la première fois et mémorisée par document.
La conversion complète est effectuée en arrière-plan depuis le fichier source,
dont la taille et la date sont vérifiées par rapport à son ouverture. Elle utilise
les mêmes règles de reconnaissance et de conservation en éléments importés que
la conversion en ligne de commande. Le JSON est validé avant remplacement du fichier
de destination ; un rapport `.modele.rapport.md` est écrit à côté. L'application
garde le RVT et ses vues ouverts, sans basculer vers le rendu natif limité.

1. **Deux niveaux** : paramétrique (niveaux, murs, ouvertures, dalles) ou importé (maillage +
   catégorie + raison). Un élément paramétrique n'est retenu que si ses paramètres reproduisent
   sa géométrie lue à 5 % près en volume (`ECART_MAX`) ; sinon il redevient un élément importé,
   avec la raison.
2. **Traçabilité** : chaque élément garde son `origine` (source, ElementId Revit ou `#n` STEP,
   IfcGUID, classe). Les identifiants du modèle sont **stables** : même élément source, même
   identifiant d'une conversion à l'autre (hachage de l'UniqueId Revit ou du GlobalId IFC),
   ce qui permettra de comparer une réimportation à la précédente.
3. **Format** : JSON, en-tête `format` = `ifc_viewer-modele` et `version` (0). Une version
   plus récente que celle du programme est refusée à la lecture ; toute évolution
   incompatible incrémente la version (migrations).
4. **Unités** : millimètres, repère du fichier source (Revit : origine interne ; IFC : repère
   du projet). Longueurs arrondies au micromètre.
5. **Échange** : l'IFC reste le format d'échange (import ; export à venir). Pas d'écriture
   `.rvt`.

## Schéma

| Collection | Champs | Notes |
|---|---|---|
| `sources[]` | `id` (« s1 »), `fichier`, `format` (RVT, IFC), `octets`, `modifie` (s depuis 1970) | Fichiers d'origine |
| `phases[]`, `sous_projets[]` | Noms | Choix de création ; listes vides si absentes des anciens fichiers. À défaut : Projet et Sous-projet 1 au premier mur. Les listes Revit sont reprises lors de la conversion. |
| `variantes[]` | `id`, `nom`, `jeu` | Options Revit reprises à la conversion, distinguées par jeu ; une variante absente sur un mur désigne le modèle principal. |
| `niveaux[]` | `id`, `nom`, `altitude`, `origine` | Triés par altitude |
| `types_murs[]`, `types_dalles[]` | `id`, `nom`, `couches[]` (`materiau`, `epaisseur`, `fonction` : structure, isolation, finition, autre), `couches_detaillees` | Murs : couches de l'extérieur vers l'intérieur ; dalles : du haut vers le bas. `couches_detaillees` = false : une couche de l'épaisseur totale (détail non lu) |
| `materiaux[]` | `nom`, `couleur` [r, g, b], `motif` (diagonales, croisees, uni, vide) | Matériaux nommés par les couches : couleur 3D et poché de coupe |
| `assemblages[]` | `id`, `nom`, `type_mur`, `couches[]` (ids de murs), `origine` | Mur composé : **chaque couche est un mur** à part entière (type monocouche, niveaux, hauteur, phase, sous-projet propres), listées de l'extérieur vers l'intérieur. Une ouverture dont `mur` vaut l'id de l'assemblage traverse toutes ses couches |
| `types_ouvertures[]` | `id`, `nom`, `genre`, `largeur`, `hauteur` | Revit seulement en v0 (cotes du type) |
| `murs[]` | `id`, `nom`, `type_mur`, `niveau`, `decalage_bas`, `haut`, `axe`, `origine`, `phase`, `sous_projet`, `variante` | `phase`, `sous_projet` et `variante` sont facultatifs (absents des anciens fichiers). `haut` : `{contrainte: libre, hauteur}` ou `{contrainte: niveau, niveau, decalage}` |
| `axe` | `{forme: droit, debut, fin}` ou `{forme: arc, debut, fin, centre, direct}` | Ligne médiane de l'épaisseur, en plan (x, y) |
| `ouvertures[]` | `id`, `nom`, `genre` (porte, fenetre, baie), `mur`, `type_ouverture`, `abscisse`, `largeur`, `hauteur`, `allege`, `representation`, `origine` | Baie rectangulaire à travers le mur ; `abscisse` = milieu de la baie le long de l'axe ; `allege` au-dessus du niveau du mur ; `representation` = menuiserie lue (maillage) |
| `dalles[]` | `id`, `nom`, `type_dalle`, `niveau`, `decalage`, `contours`, `phase`, `sous_projet`, `variante`, `origine` | `decalage` = dessus au-dessus du niveau ; `contours` en plan, pair-impair (trous) |
| `vues[]` | `id`, `nom`, `echelle`, `genre` : `plan` (`niveau`, `coupe` mm au-dessus du niveau) ou `coupe` (`debut`, `fin`, `profondeur` mm) | Arborescence du projet ; une coupe regarde vers la gauche de sa ligne |
| `importes[]` | `id`, `nom`, `categorie`, `niveau`, `maillage`, `raison`, `origine` | `maillage` : `sommets` [x, y, z], `triangles` [i, j, k] |
| `ignores` | catégorie → nombre | Éléments sans géométrie ou hors périmètre (pièces, quadrillages, espaces, ouvertures IFC vides, éléments 2D) |

Volumes déduits des paramètres (validation) : mur = longueur d'axe × épaisseur du type ×
hauteur − baies (bornées à la hauteur du mur) ; dalle = aire des contours × épaisseur du type.

## Murs composés (assemblages)

Contrairement à Revit, une couche n'est pas une tranche d'un mur unique : c'est un mur.
Un assemblage regroupe ces murs pour les tracer, les sélectionner et les percer
ensemble ; chacun garde ses propres contraintes (isolant monté jusqu'à l'acrotère,
doublage d'une autre phase…). Conventions :

- **Côté extérieur** : à gauche de l'axe parcouru du début vers la fin (normale
  (−dy, dx)), comme un mur Revit tracé dans le sens horaire.
- **Tracé** (outil Mur avec un type à plusieurs couches) : un mur par couche, axes
  décalés depuis la ligne tracée selon l'implantation (axe, face extérieure, face
  intérieure, axe de la première couche de structure). L'assemblage prend l'identifiant
  que le mur aurait eu.
- **Dissocier** : la couche quitte l'assemblage et reste un mur indépendant, en place.
  **Associer** : un mur droit du même niveau rejoint un assemblage, rangé selon sa
  position le long de la normale.
- **Type** : le type composé reste celui de l'assemblage ; le modifier n'affecte que les
  prochains tracés (les murs déjà tracés gardent leurs couches).
- **Volume** : celui de chaque couche, baies de son assemblage déduites.

## Dalles natives

Outil Dalle (ou Sol) : type choisi dans le sélecteur de type (couches du haut vers le
bas, enregistré dans `types_dalles` à la première dalle de ce type), niveau et décalage
du dessus dans Propriétés. Contour par sommets (recliquer le premier ou Entrée pour
fermer, Retour arrière enlève le dernier) ou rectangle par deux coins dans les axes du
plan ; accrochage aux extrémités, milieux et angles des murs (contours raccordés) et
aux sommets des dalles. Contour validé : au moins trois sommets, 0,01 m² au moins,
pas d'arête qui en croise une autre, orienté dans le sens trigonométrique. Affichage :
une partie par couche (couleur du matériau), poché par couche en coupe.

## Vues et arborescence

Un projet natif a une arborescence comme un fichier Revit : vue 3D, plans d'étage,
coupes (enregistrés dans `vues`), types de murs et de sols, matériaux. Onglet
Documenter : **Vue en plan** (un bouton Créer par niveau, hauteur du plan de coupe ;
la vue s'ouvre) et **Coupe** (deux clics dans un plan ; la coupe regarde vers la
gauche de la ligne, profondeur dessinée en tireté). Une coupe ouverte coupe le modèle
par le plan vertical de sa ligne (poché par couche) ; ses repères sont dessinés dans
les vues en plan. La profondeur n'est pas encore appliquée (le modèle au-delà reste
visible).

## Jonctions

Calculées à l'affichage à partir des axes (rien n'est enregistré ; `modele/jonctions.rs`),
pour les murs droits superposés en hauteur, un assemblage comptant pour un seul mur :

- **Angle (L)** : deux extrémités proches de l'intersection des axes (à l'épaisseur des
  deux murs près) sont coupées en onglet, par la droite qui joint les intersections des
  faces intérieures et des faces extérieures. Toutes les couches d'un assemblage
  prennent la même coupe : deux assemblages de même composition, tracés dans le même
  sens, se raccordent couche à couche.
- **Arrivée (T)** : une extrémité proche d'un mur, loin de ses bouts, s'arrête sur sa
  face (toutes les couches de l'arrivant).
- Plus de deux murs au même angle : extrémités laissées droites.
- En coupe, le trait commun à deux couches du même matériau est retiré (régions
  fondues, comme le nettoyage des jonctions de Revit).

## Conversion depuis Revit

Uniquement à partir des données déjà décodées par `rvt::maquette` (aucune lecture nouvelle du
format en v0).

| Élément | Paramètres | Limites |
|---|---|---|
| Niveaux | nom, altitude | |
| Murs `SWall`, `ArcWall` | type (épaisseur), niveau de base, décalage, niveau haut et décalage (ou hauteur libre), axe | Le tracé lu donne la droite ou le cercle porteur, pas les extrémités : extrémités et ligne médiane viennent de la géométrie projetée sur ce tracé (indépendant de la ligne d'implantation). Aux angles, la géométrie inclut la jonction. Couches du type lues (épaisseur et matériau, ordre extérieur → intérieur) |
| Portes, fenêtres hébergées par un mur converti | largeur et hauteur du type, abscisse et allège du point d'insertion | Cotes du type ≠ réservation découpée dans le mur ; sans cotes de type : encombrement de la famille |
| Sols, plafonds prismatiques | contour du dessus, épaisseur du type, décalage | Sols en pente : importés |
| Murs-rideaux, toits, escaliers, garde-corps, terrain, autres familles, formes d'un `.rfa` | — | Importés |

Dans un document de famille de fenêtre ou de porte, le mur hôte du gabarit est percé par le
vide de la famille, pas par une instance : son écart de volume le laisse importé.

## Conversion depuis IFC

| Élément | Paramètres | Limites |
|---|---|---|
| `IfcBuildingStorey` | nom, élévation | |
| `IfcWall`, `IfcWallStandardCase`, `IfcWallElementedCase` | axe (représentation `Axis` : droite ou arc ; sinon plus longue arête horizontale), ligne médiane, base et hauteur lues dans la géométrie, couches de `IfcMaterialLayerSet(Usage)` du mur ou de son type | Axe en polyligne : importé. Ordre des couches = ordre du fichier. Axe `Axis` droit et `IfcMaterialLayerSetUsage` (AXIS2, POSITIVE ou NEGATIVE) : assemblage, un mur par couche (axe retourné en POSITIVE pour avoir la première couche à gauche) |
| `IfcOpeningElement` (via `IfcRelVoidsElement`) | encombrement borné au mur ; genre selon `IfcRelFillsElement` (`IfcDoor`, `IfcWindow`) | Ouvertures non rectangulaires : approchées par leur encombrement |
| `IfcSlab` prismatique | contour du dessus, couches | Dalles en pente : importées |
| Autres produits avec géométrie | — | Importés, catégorie = classe IFC |

## Mesure (rapport)

Le rapport donne : part d'éléments paramétriques par catégorie ; pour les murs et les dalles
reconnus, l'écart de volume entre paramètres et géométrie lue (répartition ≤ 1 %, ≤ 5 %,
> 5 %, médiane, écart global) et les plus forts écarts ; les raisons des imports ; les
éléments non convertis.

Tests sur des fichiers hors dépôt (sautés s'ils sont absents) : `cargo test --release
depuis_rvt -- --nocapture` convertit `Projet1.rvt` et `Kerreut 2025.rvt` (copies dans
`fichiers` du dossier `RVT_ESSAIS`, par défaut `C:\Users\theba\Projets code\Decodage RVT`), écrit leurs rapports dans `exports/` ; `RVT_FAMILLES=<dossier>` convertit
chaque `.rfa` du dossier.

## Hors v0 (prochaines itérations)

- Revit : contours d'esquisse des sols, extrémités du
  tracé des murs, ligne d'implantation, cotes de réservation des familles.
- Jonctions : murs courbes, croisements (X) et angles à plus de deux murs, choix
  onglet / aboutement et priorité des couches comme dans Revit ; murs attachés aux
  toits, profils de murs non rectangulaires.
- Assemblages courbes à la conversion (arc Revit ou IFC), fonction des couches Revit
  (structure, isolation…), mise à jour des assemblages existants depuis leur type.
- Toits, escaliers, poteaux, poutres paramétriques ; familles paramétriques (portes,
  fenêtres) à la place des maillages de menuiserie.
- Affichage des autres éléments natifs (hors niveaux et murs droits), export IFC.
- Maillages des éléments importés dans un fichier binaire à côté du JSON (taille).
