# Modèle natif v0

Format d'édition du projet, source de vérité de l’éditeur. Un fichier Revit ou IFC est
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
Les murs courbes, ouvertures et éléments importés sont également affichés. Les
références et les maillages sont validés avant remplacement du document ouvert.
Voir [EDITION.md](EDITION.md) pour les commandes, la documentation et les exports.

Un RVT déjà ouvert peut aussi être enregistré en v0 avec **Sortie → Enregistrer**
ou Ctrl+S. Une destination est demandée la première fois et mémorisée par document.
La conversion complète est effectuée en arrière-plan depuis le fichier source,
dont la taille et la date sont vérifiées par rapport à son ouverture. Elle utilise
les mêmes règles de reconnaissance et de conservation en éléments importés que
la conversion en ligne de commande. Le JSON est validé avant remplacement du fichier
de destination ; un rapport `.modele.rapport.md` est écrit à côté. L'application
garde le RVT et ses vues ouverts, sans changer automatiquement de document (Début → Importer ouvre le document natif).

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
5. **Échange** : l'IFC reste le format d'échange (import et export IFC4 tessellé). Pas d'écriture
   `.rvt`.

## Schéma

| Collection | Champs | Notes |
|---|---|---|
| `sources[]` | `id` (« s1 »), `fichier`, `format` (RVT, IFC), `octets`, `modifie` (s depuis 1970) | Fichiers d'origine |
| `phases[]`, `sous_projets[]` | Noms | Choix de création ; listes vides si absentes des anciens fichiers. À défaut : Projet et Sous-projet 1 au premier mur. Les listes Revit sont reprises lors de la conversion. |
| `variantes[]` | `id`, `nom`, `jeu` | Options Revit reprises à la conversion, distinguées par jeu ; une variante absente sur un mur désigne le modèle principal. |
| `niveaux[]` | `id`, `nom`, `altitude`, `origine` | Triés par altitude |
| `types_murs[]`, `types_dalles[]` | `id`, `nom`, `couches[]` (`materiau`, `epaisseur`, `fonction` : structure, isolation, finition, autre, `plume` facultative 1 à 12), `couches_detaillees` | Murs : couches de l'extérieur vers l'intérieur ; dalles : du haut vers le bas. `couches_detaillees` = false : une couche de l'épaisseur totale (détail non lu) |
| `materiaux[]` | `nom`, `couleur` [r, g, b], `surface_fond_motif`, `surface_motif`, `surface_couleur`, `motif`, `coupe_couleur`, `coupe_fond`, `coupe_fond_motif` | Graphismes comme dans Revit, sans ombrage à part. Motifs de surface : premier plan (`surface_motif`, `surface_couleur`) et arrière-plan (`surface_fond_motif`, uni par défaut, de couleur `couleur` : couleur des faces, aussi celle du style Réaliste). Motifs de coupe : premier plan (`motif`, `coupe_couleur`) et arrière-plan (`coupe_fond_motif`, uni par défaut, `coupe_fond`, blanc par défaut). Motif : `diagonales`, `croisees`, `uni`, `vide` (premier plan : aucun), `isolation` (coupe : ligne d'isolant en boucles au milieu de la couche) ou `{"perso": "<nom>"}` (motif de hachures nommé). L'arrière-plan est uni ou hachuré. Champs facultatifs omis : valeurs par défaut |
| `motifs[]` | `nom`, `lignes[]` (`angle` °, `pas`, `glissement`, `origine` [x, y], `tirets` : trait, blanc, trait… ; mm du papier) | Motifs de hachures du projet, comme les motifs de remplissage de Revit ; un motif du gabarit ou de base modifié y est copié. Motifs de base : Diagonales, Croisées |
| `plumes[]` | épaisseurs des plumes 1 à 12 (mm du papier) | Facultatif ; à défaut celles du gabarit (`plumes`), sinon 0,10 · 0,13 · 0,18 · 0,25 · 0,35 · 0,50 · 0,70 · 1,00 · 1,40 · 2,00 · 0,05 · 0,08. Le contour de coupe d'une couche prend l'épaisseur de sa plume ; sans plume : 5 (0,35) pour une structure, 3 (0,18) pour une isolation, 2 (0,13) sinon. Plume réglable par couche dans l'éditeur de type de mur ; table éditable dans Début > Options > « Épaisseurs de traits… » (une table identique à celle du gabarit n'est pas enregistrée : le projet suit le gabarit) |
| `assemblages[]` | `id`, `nom`, `type_mur`, `couches[]` (ids de murs), `origine` | Mur composé : **chaque couche est un mur** à part entière (type monocouche, niveaux, hauteur, phase, sous-projet propres), listées de l'extérieur vers l'intérieur. Une ouverture dont `mur` vaut l'id de l'assemblage traverse toutes ses couches |
| `types_ouvertures[]` | `id`, `nom`, `genre`, `largeur`, `hauteur` | Revit seulement en v0 (cotes du type) |
| `murs[]` | `id`, `nom`, `type_mur`, `niveau`, `decalage_bas`, `haut`, `axe`, `origine`, `phase`, `sous_projet`, `variante` | `phase`, `sous_projet` et `variante` sont facultatifs (absents des anciens fichiers). `haut` : `{contrainte: libre, hauteur}` ou `{contrainte: niveau, niveau, decalage}` |
| `axe` | `{forme: droit, debut, fin}` ou `{forme: arc, debut, fin, centre, direct}` | Ligne médiane de l'épaisseur, en plan (x, y) |
| `ouvertures[]` | `id`, `nom`, `genre` (porte, fenetre, libre), `mur`, `type_ouverture`, `abscisse`, `largeur`, `hauteur`, `allege`, `representation`, `origine` | Baie rectangulaire à travers le mur ; `abscisse` = milieu de la baie le long de l'axe ; `allege` au-dessus du niveau du mur ; `representation` = menuiserie lue (maillage) |
| `dalles[]` | `id`, `nom`, `sorte`, `type_dalle`, `niveau`, `decalage`, `contours`, `pentes`, `phase`, `sous_projet`, `variante`, `origine` | `sorte` : `dalle` (défaut, omis), `sol`, `plafond`, `toiture`. `decalage` = dessus (dalle, sol) ou sous-face (plafond, toiture ; à l'égout si en pente) au-dessus du niveau ; `contours` en plan, pair-impair (trous) ; `pentes[c][k]` = pente en degrés de l'arête du sommet k au suivant du contour c, `null` sans pente (omis : élément plan) |
| `positions_plan[]` | `nom`, `angle` (degrés, plan par rapport au nord du modèle) | Orientations enregistrées depuis le repère du plan (« Positions ») |
| `vues[]` | `id`, `nom`, `type_vue`, `echelle`, `genre` : `plan` (`niveau`, `coupe` mm au-dessus du niveau) ou `coupe` (`debut`, `fin`, `profondeur` mm, `elevation`) | Arborescence du projet ; une coupe ou une élévation regarde vers la gauche de sa ligne |
| `importes[]` | `id`, `nom`, `categorie`, `niveau`, `maillage`, `raison`, `origine` | `maillage` : `sommets` [x, y, z], `triangles` [i, j, k] |
| `ignores` | catégorie → nombre | Éléments sans géométrie ou hors périmètre (pièces, quadrillages, espaces, ouvertures IFC vides, éléments 2D) |

Volumes déduits des paramètres (validation) : mur = longueur d'axe × épaisseur du type ×
hauteur − baies (bornées à la hauteur du mur) ; dalle = aire des contours × épaisseur du type (en pente : épaisseur normale aux versants, intégrée sur chaque versant).

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

## Éléments horizontaux natifs (dalle, sol, plafond, toiture)

Un seul élément, quatre sortes qui ne diffèrent que par leur nom, leurs types et leur
référence d'altitude : dessus pour la dalle et le sol, sous-face pour le plafond
(hauteur sous plafond) et la toiture. Type choisi dans le sélecteur de type (onglet par
sorte, types du gabarit, couches du haut vers le bas, enregistré dans `types_dalles` au
premier élément de ce type), niveau et décalage dans Propriétés. Contour par sommets
(recliquer le premier ou Entrée pour fermer, Retour arrière enlève le dernier) ou
rectangle par deux coins dans les axes du plan ; accrochage aux extrémités, milieux et
angles des murs (contours raccordés) et aux sommets des dalles. Contour validé : au
moins trois sommets, 0,01 m² au moins, pas d'arête qui en croise une autre, orienté
dans le sens trigonométrique. Affichage : une partie par couche (couleur du matériau),
poché par couche en coupe. Classes IFC de la scène : `IfcSlab` (dalle), `IfcCovering`
(sol, plafond), `IfcRoof` (toiture) ; familles de l'arborescence : Sols, Plafonds, Toits.

### Pente par arête

Comme une toiture par tracé de Revit : une arête qui « définit la pente » porte un
versant qui monte de son arête vers l'intérieur du contour, avec sa pente en degrés ;
la surface est le plus bas des versants (croupes, noues, faîtages à leurs
intersections). Une arête sans pente ne porte rien : deux versants opposés et deux
pignons font une toiture à deux pans ; aucune arête en pente, un élément plan.

- Création : outil Toiture, case « Toutes les arêtes la définissent » et pente (défaut
  35°) ; décochée, toiture plate.
- Modification (toiture, plafond rampant) : liste des arêtes dans Propriétés (case et
  pente), ou clic sur les repères des arêtes dans la vue quand l'élément est
  sélectionné (triangle plein et pente : arête en pente ; rond : sans pente).
- Couches : épaisseur normale aux versants (verticale : épaisseur / cos pente), empilées
  depuis la sous-face. Coupe d'égout verticale. Une couche de coupe par versant.
- Contour convexe : enveloppe des plans par arête. Contour concave (L, T) :
  squelette droit pour une pente uniforme sur toutes les arêtes ; les autres
  combinaisons sont refusées explicitement (voir EDITION.md).

## Gabarit

Matériaux (couleur et motif de coupe par mot du nom), types de murs, types d'éléments
horizontaux et types de coupes / élévations viennent d'un gabarit JSON, pas du code.
Le gabarit par défaut est `gabarits/defaut.gabarit.json` (embarqué dans l'exécutable).
Début > Options : choisir un autre fichier (`.gabarit.json`, mémorisé dans les options),
revenir au défaut, ou enregistrer une copie pour en faire un gabarit d'agence.

```json
{
 "format": "ifc_viewer.gabarit", "version": 1, "nom": "Agence",
 "materiaux": [{"mots": ["béton"], "couleur": [176, 176, 170], "motif": "diagonales"}],
 "types_murs": [{"nom": "Voile BA 200", "couches": [{"materiau": "Béton armé", "epaisseur": 200.0, "fonction": "structure"}]}],
 "types_surfaces": [{"sorte": "toiture", "nom": "Bac acier", "couches": [{"materiau": "Bac acier", "epaisseur": 40.0}]}],
 "types_vues": [{"elevation": false, "nom": "Coupe de bâtiment", "aide": "", "echelle": 100.0, "profondeur": 15000.0}]
}
```

Motifs : `diagonales`, `croisees`, `uni`, `vide`. Il faut au moins un type de coupe et un
type d'élévation. `niveaux` (`nom`, `altitude` en mm) : niveaux d'un nouveau projet (RDC
à 0 dans le gabarit par défaut). Les familles de `materiaux` acceptent aussi
`surface_fond_motif`, `surface_motif`, `surface_couleur`, `coupe_couleur`, `coupe_fond`,
`coupe_fond_motif`. `motifs` : motifs de hachures proposés (Diagonales serrées,
Horizontales, Béton, Brique, Bois (fil), Terre dans le gabarit par défaut).

Éditeur de hachures (Graphismes d'un matériau > Motifs de hachures…) : liste des motifs
avec aperçu, Nouveau, Dupliquer, Supprimer (motif du projet non utilisé) ou Rétablir (motif
du gabarit modifié) ; nom (renommer met à jour les matériaux), familles de traits (angle,
pas, glissement, origine, tirets) et aperçu de 3 × 3 cm du papier. En 3D, les motifs de
surface sont tracés en traits continus (deux familles au plus). Un nouveau projet reprend les types de murs du gabarit ; dans un projet
existant, ceux qui manquent sont proposés en bas du sélecteur de type de mur et copiés
dans le projet au choix.

## Jonctions et édition des murs

- Angle (L) en onglet entre murs de même épaisseur ; entre épaisseurs différentes,
  bout à bout (pas d'onglet) : le plus épais va jusqu'à la face extérieure de l'autre,
  qui s'arrête sur sa face, noyau prolongé jusqu'à son noyau (comme une arrivée en T).
  Arrivée (T) arrêtée sur la face du mur traversé, deux murs alignés
  bout à bout continus : un troisième mur au même nœud s'arrête sur leur face.
- Angle en L (même épaisseur) joint par les noyaux : les noyaux (couches porteuses) se coupent en onglet
  sur leurs propres faces ; chaque autre couche rejoint, sur l'autre mur, la ligne située
  à la même profondeur depuis le noyau (côté intérieur ou extérieur de l'angle) : à
  composition égale, onglet couche à couche ; à compositions différentes, noyaux alignés
  et finitions qui s'enveloppent.
- Jonction des noyaux (comme Revit) : à une arrivée en T, les couches porteuses
  (fonction structure) du mur arrivant vont jusqu'à la face du noyau du mur rejoint ; ses
  couches de finition s'arrêtent sur sa face, et celles du mur rejoint sont interrompues
  au passage. En coupe, le trait commun à deux éléments du même matériau disparaît, même
  partiel (trait court posé sur un trait long).
- Arêtes : celles des éléments d'un même matériau sont calculées ensemble ; là où deux
  éléments se touchent (faces collées), les faces qui se prolongent ne portent pas
  d'arête. Les angles réels restent, et deux matériaux différents gardent leur limite.
- Mur ou assemblage sélectionné : axe en tireté et poignées à ses extrémités ; glisser
  une poignée (accrochages du tracé, Maj : orthogonal) déplace l'extrémité ; les couches
  d'un assemblage gardent leur écart à l'axe.

## Vues et arborescence

Un projet natif a une arborescence comme un fichier Revit : vue 3D, plans d'étage,
coupes (enregistrés dans `vues`), types de murs et de sols, matériaux. Onglet
Documenter : **Vue en plan** (un bouton Créer par niveau, hauteur du plan de coupe ;
la vue s'ouvre) et **Coupe / élévation** : un seul outil, le sélecteur de type propose
les coupes (de bâtiment, de mur, de détail) et les élévations (extérieure, intérieure),
chacune avec son échelle et sa profondeur. Deux clics dans un plan ; la vue regarde
vers la gauche de la ligne. Une coupe tranche le modèle par le plan vertical de sa
ligne (poché par couche) ; une élévation montre le modèle depuis sa ligne, sans
poché. Le modèle est limité à la profondeur et à la longueur de la ligne. Repères
dans les plans (coupe : trait mixte et têtes ; élévation : cercle et flèche).
Propriétés d'une vue ouverte : nom, échelle, hauteur de coupe (plan) ou profondeur,
Retourner, Supprimer la vue.

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
- Les croisements et raccords multiples/courbes sont complétés par soustraction
  volumétrique selon la priorité des couches (voir EDITION.md).
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
| Plans d'étage | niveau, hauteur du plan de coupe (plage de vue), nom, type, échelle | Orientation d'un plan pivoté non reprise (plan au nord du modèle, compté dans le rapport) |
| Coupes, élévations verticales cadrées | ligne dans le plan proche du cadrage, sur sa largeur ; profondeur du cadrage (limite de vue), sinon 50 m ; même côté de regard | Coupes inclinées ou sans cadrage, vues 3D, plafonds, dessin, légendes, nomenclatures : ignorées (comptées) ; éléments 2D (cotes, textes) non repris |
| Nomenclatures | nom, catégorie (Sols → Dalle, Toits → Toiture, Plafonds → Plafond, Poteaux, Ossature → Poutres, sinon le nom Revit), détail ou regroupement par type, champs que le modèle natif calcule (type, niveau, nombre, longueur, hauteur, largeur, épaisseur, surface, volume) | Autres champs (paramètres partagés, formules, marques…), filtres et tris : comptés non repris ; nomenclatures de clés ignorées |
| Feuilles | numéro, nom, format (étendue du cartouche, sinon A1), vues placées converties à la même position et la même échelle, jeu « Plans » | Le cartouche Revit n'est pas repris (cartouche natif) ; une vue placée non convertie est comptée dans le rapport |

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
- Jonctions : choix explicite onglet / aboutement et profils de murs arbitraires.
  Les croisements, priorités de couches et attaches aux toitures sont disponibles.
- Assemblages courbes à la conversion (arc Revit ou IFC) et fonction des couches
  Revit (structure, isolation…). La mise à jour depuis un type natif est disponible.
- Toits concaves à pentes différentes ; escaliers et garde-corps paramétriques ;
  familles paramétriques (portes, fenêtres) à la place des maillages de menuiserie.
- Export de géométrie IFC paramétrique plus riche que les corps tessellés actuels.
- Maillages des éléments importés dans un fichier binaire à côté du JSON (taille).

## Édition et documentation additionnelles

Les feuilles appartiennent à un jeu (`jeu`, « Plans » par défaut, y compris pour les
fichiers antérieurs) ; les feuilles d'un jeu se suivent dans l'ordre de `feuilles[]`.
Les collections et tables facultatives de l'éditeur (annotations, quadrillages,
feuilles, nomenclatures, attaches de toiture et priorités de jonction) sont détaillées
avec leurs commandes et leurs limites dans [EDITION.md](EDITION.md).
