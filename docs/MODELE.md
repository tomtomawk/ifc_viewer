# Modèle natif v0

Format d'édition du projet, source de vérité de l’éditeur. Un fichier Revit ou IFC est
**converti** vers ce modèle : ce qui est reconnu devient paramétrique, le reste devient un
**élément importé** (géométrie figée). Rien n'est jeté sans être compté.

Code : `crates/modele/src/` (types : `lib.rs`) et `crates/echanges/src/` (conversions : `depuis_rvt.rs`, `depuis_ifc.rs` ;
lecture de paramètres dans une géométrie : `geometrie.rs` ; rapport : `rapport.rs`).

```
bimedit --modele <fichier.rvt|.rfa|.ifc> [sortie.json]
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
3. **Format** : JSON, en-tête `format` = `bimedit-modele` et `version` (0). Une version
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
| `profils[]` | `id`, `nom`, `document_famille` | Famille de catégorie `profil` (même JSON qu'un fichier `.famille.json`) ; la section est son contour fermé en plan (`section_profil` le désigne s'il y en a plusieurs), en mm. Contour fermé et non croisé. |
| `familles_cartouches[]` | `id`, `nom`, `largeur`, `hauteur` (mm papier), `document_famille` | Famille de catégorie `cartouche` ; seuls les éléments 2D de sa vue en plan sont rendus sur la feuille, en mm du papier depuis le coin inférieur gauche du cartouche. |
| `familles_etiquettes[]` | `id`, `nom`, `document_famille` | Famille de catégorie `etiquette_piece` ; seuls les éléments 2D de sa vue en plan sont rendus, en mm du papier autour du point de la pièce (textes centrés). |
| `gabarits_vues[]`, `gabarits_vue_affectes` | `id`, `nom`, `echelle`, `poche`, `phase`, `variante` ; correspondance vue → gabarit | Paramètres partagés, répercutés dans les vues et leurs contextes. La clé `3d` désigne la vue 3D par défaut ; ses valeurs conservées sont `echelle_vue_3d` et `poche_vue_3d`. |
| `terrains[]`, `types_terrains[]` | Terrain : `id`, `nom`, `contour` et `points` (cotes NGF), `type_terrain` ; type : `id`, `nom`, `epaisseur` (profondeur nominale), `materiau`, `couches[]` facultatives (`materiau`, `epaisseur`, `variable`) | Fond plat à la profondeur nominale sous le point le plus bas du terrain naturel, abaissé pour garder 1 m sous une fouille de terre-plein (profondeur effective, calculée). Couches de haut en bas : les fixes suivent la surface, la variable (une au plus, à défaut la dernière) comble jusqu'au fond ; celles qui la suivent reposent sur le fond |
| `niveaux[]` | `id`, `nom`, `altitude`, `origine` | Triés par altitude |
| `types_murs[]`, `types_dalles[]` | `id`, `nom`, `couches[]` (`materiau`, `epaisseur`, `fonction` : structure, isolation, finition, lame_air, autre — `lame_air` : couche vide, qui garde son épaisseur sans géométrie ni contour de coupe, `plume` facultative 1 à 12), `couches_detaillees`, `sorte` (types d'éléments horizontaux : dalle, sol, plafond, toiture, terre_plein ; champ ajouté le 20/11/2026, facultatif — absent, le type est rangé d'après les éléments qui l'utilisent) | Murs : couches de l'extérieur vers l'intérieur ; dalles : du haut vers le bas. `couches_detaillees` = false : une couche de l'épaisseur totale (détail non lu) |
| `materiaux[]` | `nom`, `couleur` [r, g, b], `surface_fond_motif`, `surface_motif`, `surface_couleur`, `motif`, `coupe_couleur`, `coupe_fond`, `coupe_fond_motif` | Graphismes comme dans Revit, sans ombrage à part. Motifs de surface : premier plan (`surface_motif`, `surface_couleur`) et arrière-plan (`surface_fond_motif`, uni par défaut, de couleur `couleur` : couleur des faces, aussi celle du style Réaliste). Motifs de coupe : premier plan (`motif`, `coupe_couleur`) et arrière-plan (`coupe_fond_motif`, uni par défaut, `coupe_fond`, blanc par défaut). Motif : `diagonales`, `croisees`, `uni`, `vide` (premier plan : aucun), `isolation` (coupe : ligne d'isolant en boucles au milieu de la couche) ou `{"perso": "<nom>"}` (motif de hachures nommé). L'arrière-plan est uni ou hachuré. Champs facultatifs omis : valeurs par défaut |
| `motifs[]` | `nom`, `lignes[]` (`angle` °, `pas`, `glissement`, `origine` [x, y], `tirets` : trait, blanc, trait… ; mm du papier) | Motifs de hachures du projet, comme les motifs de remplissage de Revit ; un motif du gabarit ou de base modifié y est copié. Motifs de base : Diagonales, Croisées |
| `plumes[]` | épaisseurs des plumes 1 à 12 (mm du papier) | Facultatif ; à défaut celles du gabarit (`plumes`), sinon 0,10 · 0,13 · 0,18 · 0,25 · 0,35 · 0,50 · 0,70 · 1,00 · 1,40 · 2,00 · 0,05 · 0,08. Le contour de coupe d'une couche prend l'épaisseur de sa plume ; sans plume : 5 (0,35) pour une structure, 3 (0,18) pour une isolation, 2 (0,13) sinon. Plume réglable par couche dans l'éditeur de type de mur ; table éditable dans Début > Options > « Épaisseurs de traits… » (une table identique à celle du gabarit n'est pas enregistrée : le projet suit le gabarit) |
| `assemblages[]` | `id`, `nom`, `type_mur`, `couches[]` (ids de murs), `origine` | Mur composé : **chaque couche est un mur** à part entière (type monocouche, niveaux, hauteur, phase, sous-projet propres), listées de l'extérieur vers l'intérieur. Une ouverture dont `mur` vaut l'id de l'assemblage traverse toutes ses couches |
| Fichier `.famille.json` | `format` (« famille-ouverture 3 »), `nom`, `largeur`, `hauteur`, `profondeur`, `famille` (comme `modele`) | Famille d'ouverture échangée entre projets (éditeur : « Enregistrer sous… », arborescence : « Charger une famille… ») |
| `escaliers[]` | `id`, `nom`, `niveau`, `niveau_haut` (facultatif), `decalage_bas`, `hauteur` (sans niveau d'arrivée), `trace` (ligne de foulée [x, y]), `largeur`, `hauteur_marche` (visée), `paillasse` (épaisseur verticale), `sous_face` (facultatif, `crantee` par défaut ou `lissee`) (`balance` facultatif : nombre de marches balancées de part et d'autre de chaque tournant, à la place des paliers) | Volées et paliers (un par sommet intermédiaire) ; contremarches = hauteur / hauteur visée arrondie |
| `barres[]` | `id`, `nom`, `genre` (poteau, poutre), `niveau`, `debut`, `fin` (mm, Z relatif au niveau), `largeur`, `hauteur`, `rotation`, `origine` (facultative : barre convertie d'un IFC) ; poutre : `type_poutre`, `justification` (gauche, axe, droite), `arase` (superieure, axe, inferieure), facultatifs | Poutre : la ligne tracée est sa face gauche, son axe ou sa face droite ; Z désigne son arase supérieure, son axe ou son arase inférieure. Sans type : section `largeur` × `hauteur` |
| `pieces[]` | `id`, `nom`, `numero`, `niveau`, `point` [x, y] (mm), `contour` (calculé) ; facultatifs : `etiquette`, `donnees`, `origine` | `contour` : faces intérieures des murs autour de `point`, recalculé après chaque modification. `donnees` : données lues dans la source (import IFC : jeux de l'`IfcSpace`, dont `Pset_SpaceCommon` et `Qto_SpaceBaseQuantities`), même forme et mêmes vérifications que les champs `proprietes`, `materiau`, `couleur` d'`importes[]`, omis si vide ; lecture seule, réécrites à l'export IFC (sauf `Qto_SpaceBaseQuantities`, recalculé). `origine` : `IfcSpace` d'origine ; son GUID est repris à l'export. Une pièce copiée perd son `origine` |
| `types_poutres[]` | `id`, `nom`, `profil` (famille de profil, facultatif), `largeur`, `hauteur`, `materiau` (facultatif) | Avec profil : contour de la famille mis à l'encombrement `largeur` × `hauteur` ; sans profil : section rectangulaire |
| `garde_corps[]` | `id`, `nom`, `niveau`, `decalage`, `trace`, `hauteur`, `remplissage` (barreaudage, plein, vitre), `escalier` (hôte, facultatif) | Main courante, lisse, montants, remplissage ; sur un escalier, la base suit les marches |
| `parametres_partages` | nom → valeur (mm) | Paramètres partagés du projet, repris par les familles qui les déclarent « partage » |
| `types_ouvertures[]` | `id`, `nom`, `genre`, `largeur`, `hauteur`, `famille` (facultatif) | Cotes du type (Revit, ou créé par l'éditeur de famille). `famille` (fenêtre) : `dormant`, `profondeur`, `nu` (`exterieur`, `milieu`, `interieur`), `retrait`, `ouvrant`, `vantaux[]` (`fixe`, `battant_gauche`, `battant_droite`, `oscillo_battant_gauche`, `oscillo_battant_droite`, `coulissant`, de gauche à droite vus de l'intérieur), `imposte`, `soubassement`, `vitrage`, `appui` (mm), `couleur` [r, g, b] ; elle remplace la menuiserie générique des fenêtres du type. `modele` (famille modélisée, prioritaire) : `genre` (porte / fenetre : catégorie du type), `parametres[]` (`nom`, `formule`, `portee` type / occurrence / partage, `genre` longueur / oui_non / materiau / texte : formule 0 = non, nom du matériau, ou texte non calculé ; un paramètre texte est de type), `plans[]` (`nom`, `axe` x/y/z, `position`, `fixe`, `hote` exterieur/interieur), `plans_tournes[]` (voir « Plans de référence tournés »), `contraintes[]` (`genre` distance : `a`, `b`, `valeur` ; egalite : `a`, `m`, `b` ; angle : `a`, `b`, `valeur` en degrés ; `priorite` 0–9), `volumes[]` (`nom`, `vide`, `materiau` menuiserie/vitrage/panneau/appui, `min` et `max` : trois faces `plan` + `decalage`, `visible` : paramètre oui/non, `visibilite` (voir « Visibilité des éléments de famille »), `materiau_parametre` : paramètre matériau), `extrusions[]` (`nom`, `vide`, `materiau`, `vue` elevation / plan / coupe, `support` facultatif, `profil[]` de sommets `h` et `v` : faces, `profondeur` : deux faces, `visible`, `visibilite`, `materiau_parametre`), `elements_2d[]` (voir « Éléments 2D des familles »), `couleur` ; repère de la baie : x le long du mur, y vers l'extérieur, z depuis l'allège |
| `murs[]` | `id`, `nom`, `type_mur`, `niveau`, `decalage_bas`, `haut`, `axe`, `origine`, `phase`, `sous_projet`, `variante` | `phase`, `sous_projet` et `variante` sont facultatifs (absents des anciens fichiers). `haut` : `{contrainte: libre, hauteur}` ou `{contrainte: niveau, niveau, decalage}` |
| `axe` | `{forme: droit, debut, fin}` ou `{forme: arc, debut, fin, centre, direct}` | Ligne médiane de l'épaisseur, en plan (x, y) |
| `ouvertures[]` | `id`, `nom`, `genre` (porte, fenetre, libre), `mur`, `type_ouverture`, `abscisse`, `largeur`, `hauteur`, `allege`, `representation`, `valeurs`, `retournee`, `inversee`, `origine` | Baie rectangulaire à travers le mur (ou vides de la famille du type) ; `abscisse` = milieu de la baie le long de l'axe ; `allege` au-dessus du niveau du mur ; `representation` = menuiserie lue (maillage) ; `valeurs` = paramètres d'occurrence de la famille (longueurs, oui/non) ; `materiaux` = paramètres matériau d'occurrence ; `retournee` / `inversee` = famille retournée (intérieur ↔ extérieur) / inversée (gauche ↔ droite) |
| `dalles[]` | `id`, `nom`, `sorte`, `type_dalle`, `niveau`, `decalage`, `contours`, `pentes`, `phase`, `sous_projet`, `variante`, `origine` | `sorte` : `dalle` (défaut, omis), `sol`, `plafond`, `toiture`. `decalage` = dessus (dalle, sol) ou sous-face (plafond, toiture ; à l'égout si en pente) au-dessus du niveau ; `contours` en plan, pair-impair (trous) ; `pentes[c][k]` = pente en degrés de l'arête du sommet k au suivant du contour c, `null` sans pente (omis : élément plan) |
| `positions_plan[]` | `nom`, `angle` (degrés, plan par rapport au nord du modèle) | Orientations enregistrées depuis le repère du plan (« Positions ») |
| `orientation_nord` | angle en radians par rapport à l'axe Y du projet | Orientation des points cardinaux ; 0 par défaut pour les anciens projets |
| `vues[]` | `id`, `nom`, `type_vue`, `echelle`, `niveau_detail` (`faible`, `moyen`, `eleve` ; absent : `moyen`), `genre` : `plan` (`niveau`, `coupe` mm au-dessus du niveau) ou `coupe` (`debut`, `fin`, `profondeur` mm, `elevation`) | Arborescence du projet ; une coupe ou une élévation regarde vers la gauche de sa ligne |
| `importes[]` | `id`, `nom`, `categorie`, `niveau`, `maillage`, `raison`, `origine` ; facultatifs : `proprietes`, `materiau`, `couleur` | `maillage` : `sommets` [x, y, z], `triangles` [i, j, k]. `proprietes[]` : jeux lus dans la source, `{nom, quantites, proprietes: [{nom, valeur, unite, logique}]}` (`quantites` vrai : jeu de quantités, omis sinon ; `valeur` en texte ; `unite` facultative, unités SI de base : m, m², m³, kg ; `logique` facultatif : `"booleen"` (IFC `IfcBoolean`, `valeur` « Vrai » ou « Faux ») ou `"logique"` (`IfcLogical`, « Vrai », « Faux » ou « Inconnu »), réécrit typé à l'export IFC, omis pour un texte ou un nombre). `materiau` : nom (plusieurs matériaux joints par « \| »). `couleur` : [r, g, b, a] de 0 à 255. Champs omis quand ils sont vides ; vérifiés par `Projet::verifier` : noms de jeux non vides et uniques, noms de propriétés non vides et uniques dans leur jeu, unité non vide si présente, valeur numérique finie, quantités numériques positives ou nulles, valeur admise par `logique` et pas de `logique` dans un jeu de quantités, matériau non vide |
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
sorte, types du projet `types_dalles` rangés par leur `sorte`, couches du haut vers le bas ;
sans type de cette sorte, le type par défaut du code, enregistré au premier élément),
niveau et décalage dans Propriétés. Modifier, Dupliquer et « + Nouveau type… » ouvrent
l'éditeur des types de murs. Le gabarit fournit les types d'un nouveau projet
seulement (`Projet::depuis_gabarit`). Contour par sommets
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
 "format": "bimedit.gabarit", "version": 1, "nom": "Agence",
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
`coupe_fond_motif`, et `apparence` (même forme que l'apparence d'un matériau du projet :
`herbe`, `rugosite`, `textures`…) donnée aux matériaux créés d'après la famille ; une
texture `gabarit:textures/<fichier>` désigne une texture livrée avec le logiciel
(`gabarits/textures`, embarquée). Le gabarit par défaut donne des brins d'herbe de 60 mm et
la texture de pelouse à « Terre naturelle » et « Pelouse / gazon / herbe », la texture de
feuilles découpée (masque) à « Feuillage » et une texture à « Écorce » ; un arbre d'un
projet sans matériau « Feuillage » ou « Écorce » prend ceux du gabarit (feuillage à la
couleur de son essence). Un matériau déjà enregistré dans un projet garde son apparence.
La porte intérieure du gabarit montre en plan le vantail ouvert (contour sans fond) et son
débattement en pointillés (segment de polyligne courbe, type `trait-pointilles`) ; le
vantail fermé et la quincaillerie sont cachés en plan (`visibilite.plan`). `motifs` : motifs de hachures proposés (Diagonales serrées,
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

## Liens IFC

`liens[]` (absent si vide) : fichiers IFC affichés dans le projet en lecture seule. Le
fichier n'est pas copié ; aucune entité liée n'entre dans les autres collections.

| Champ | Contenu |
|---|---|
| `id` | identifiant unique dans le projet (comme tout élément) |
| `nom` | nom affiché (nom du fichier à l'insertion) |
| `chemin` | chemin absolu du fichier `.ifc` (non vide) |
| `decalage` | `[x, y, z]` en mm, appliqué après la rotation ; défaut `[0, 0, 0]` |
| `rotation` | degrés autour de Z, sens trigonométrique, autour de l'origine du fichier lié ; défaut 0 |
| `visible` | défaut `true` |

Champ inconnu refusé ; `decalage` et `rotation` doivent être finis. Créé ou remplacé par
la commande `{"action":"lien","lien":{…}}` (identifiant vide : création), retiré par
`supprimer`. Collection partagée en collaboration (politique `Identifiee`).

## Conversion depuis IFC

| Élément | Paramètres | Limites |
|---|---|---|
| `IfcBuildingStorey` | nom, élévation | |
| `IfcWall`, `IfcWallStandardCase`, `IfcWallElementedCase` | axe (représentation `Axis` : droite ou arc ; sinon plus longue arête horizontale), ligne médiane, base et hauteur lues dans la géométrie, couches de `IfcMaterialLayerSet(Usage)` du mur ou de son type | Axe en polyligne : importé. Ordre des couches = ordre du fichier. Axe `Axis` droit et `IfcMaterialLayerSetUsage` (AXIS2, POSITIVE ou NEGATIVE) : assemblage, un mur par couche (axe retourné en POSITIVE pour avoir la première couche à gauche) |
| `IfcOpeningElement` (via `IfcRelVoidsElement`) | encombrement borné au mur ; genre selon `IfcRelFillsElement` (`IfcDoor`, `IfcWindow`) | Ouvertures non rectangulaires : approchées par leur encombrement |
| `IfcSlab` prismatique | contour du dessus, couches | Dalles en pente : importées |
| `IfcColumn`, `IfcBeam` (et `…StandardCase`) | barre (`barres[]`) : un seul `IfcExtrudedAreaSolid` d'un `IfcRectangleProfileDef` dans la représentation `Body`, sans ouverture ; poteau : extrusion verticale, section `XDim` × `YDim`, `rotation` = direction de `XDim` ; poutre : extrusion horizontale, axe au milieu de la section, côté horizontal = `largeur`, côté vertical = `hauteur`, sans type | Autre forme, poteau non vertical, poutre inclinée ou section tournée autour de son axe : importés avec la raison. Jeux de propriétés, matériau et couleur d'une barre convertie non gardés |
| `IfcSpace` | pièce (`pieces[]`) : numéro = `Name`, nom = `LongName` (sinon `Name`), niveau de l'étage, point = point intérieur de l'emprise au sol (faces du dessous de la géométrie, le plus loin du bord), contour détecté sur les murs convertis ; jeux et origine gardés | Sans niveau, sans contour, ou aire du contour à plus de 5 % de l'emprise ou de `NetFloorArea` : élément importé (bleu transparent si la source n'a pas de couleur), avec la raison |
| Autres produits avec géométrie | — | Importés, catégorie = classe IFC ; jeux de propriétés et de quantités, matériaux et couleur gardés (`proprietes`, `materiau`, `couleur`) |

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

Une feuille porte aussi des nomenclatures posées (`nomenclatures[]` : `nomenclature`,
`position` du coin haut gauche du tableau, mm du papier).
Les feuilles appartiennent à un jeu (`jeu`, « Plans » par défaut, y compris pour les
fichiers antérieurs) ; les feuilles d'un jeu se suivent dans l'ordre de `feuilles[]`.
Les collections et tables facultatives de l'éditeur (annotations, quadrillages,
feuilles, nomenclatures, attaches de toiture et priorités de jonction) sont détaillées
avec leurs commandes et leurs limites dans [EDITION.md](EDITION.md).

### Opérations des formes de famille

Le format courant des fichiers de famille est `famille-ouverture 3`. Il comporte une `profondeur` positive en millimètres : profondeur du composant ou épaisseur d'essai de l'hôte. Cette valeur est enregistrée et restaurée avec l'historique. Le format précédent et les documents de famille intégrés sans cette propriété sont incompatibles ; aucune conversion automatique n'est prévue. Les données utilisateur ne sont pas supprimées.

Un profil peut désigner sa section en plan par `famille.section_profil` : l'indice (à partir de 0) d'un élément de `elements_2d`. La référence doit désigner une polyligne fermée en plan ; une référence invalide est refusée au chargement. Sans désignation, un seul contour fermé en plan est accepté et utilisé automatiquement. Les autres vues et les autres éléments de dessin restent conservés. Cette propriété étend le schéma courant des familles ; aucune conversion d'anciens fichiers n'est ajoutée.

Chaque élément de `famille.extrusions` porte un objet `operation` obligatoire. `{"genre":"extrusion"}` décrit une extrusion, `{"genre":"revolution","revolution":{...}}` une révolution, et `{"genre":"guidage","guidage":{"genre":..., ...}}` une extrusion par chemin, un raccordement ou un raccordement par chemin. Le profil, le repère, le matériau et le caractère plein ou vide restent communs à la forme. Le format `famille-ouverture 3` est incompatible avec les anciens fichiers de famille ; ceux-ci sont à refaire. Les projets contenant des familles dans l'ancien schéma doivent également être refaits ou leurs familles recréées.

Champs facultatifs de `guidage` (absents : valeur par défaut, omis à l'écriture ; ajout du 18/11/2026 sans conversion) :

- `arcs` (`extrusion_chemin`, `raccordement_chemin`) : indices (à partir de 0) des points de `chemin` qui sont le passage d'un arc par trois points avec leurs deux voisins. Un passage doit avoir deux voisins (sur un chemin ouvert : ni le premier ni le dernier point), deux passages ne peuvent être ni confondus ni voisins, et les trois points ne doivent pas être alignés. L'arc est discrétisé en cordes de 5° au plus.
- `ferme` (`extrusion_chemin`, booléen) : le dernier point est relié au premier, sans capuchon ; au moins trois points, sans répéter le premier. Un chemin ouvert dont les extrémités se rejoignent est refusé.
- `ancrage_fin` (`raccordement`, `raccordement_chemin`) : profil d'arrivée en sommets `{ "h": {"plan", "decalage"}, "v": {...} }` verrouillés sur les plans de la vue d'esquisse, comme `profil` (au moins trois sommets, plans existants de l'axe voulu). Présent, il remplace `profil_fin`, recalculé à chaque résolution (la valeur écrite n'est qu'une copie).

`profil_fin` peut compter un nombre de sommets différent de `profil` : la correspondance aligne les centres de surface, prend pour premier sommet d'arrivée le plus proche du premier sommet de départ, puis rééchantillonne les deux contours par abscisse curviligne. À nombre égal, la correspondance reste sommet à sommet. Les deux profils doivent être dans le même sens. Aux sommets intérieurs du chemin, la section est en onglet exact (plan bissecteur) ; un virage de plus de 150° ou des onglets qui se recouvrent sont refusés ; sur un chemin fermé non plan, le vrillage résiduel du repère est réparti sur les tronçons.

### Plans de référence tournés

Un plan de référence est soit un plan d'axe (`plans[]` : `axe` x, y ou z et `position`), soit un plan tourné (`plans_tournes[]`, absent s'il est vide) : `{ "nom", "hote", "appui", "angle" }`. Le plan part de son plan `hote` et pivote de `angle` degrés (angle dessiné) autour de la droite d'appui, intersection de `hote` et de `appui`. `hote` et `appui` sont deux autres plans de référence, d'axe ou tournés, non parallèles : les plans tournés s'enchaînent, toutes les orientations sont atteignables, et un plan suit les plans qui le portent. Un cycle ou un hôte parallèle à l'appui est une erreur de résolution. Les noms sont uniques parmi tous les plans de référence. Un champ inconnu est refusé (`plans[]`, `plans_tournes[]`, la famille, les formes, `support`).

Une cote angulaire `{ "genre": "angle", "a": <hôte>, "b": <plan tourné>, "valeur", "priorite" }` pilote l'angle d'un plan tourné par rapport à son hôte ; `valeur` est une formule en degrés (paramètres admis). La plus prioritaire l'emporte, les autres sont signalées en conflit. Cotes (`distance`) et égalités (`egalite`) ne relient que des plans d'axe parallèles ; une cote angulaire relie un plan tourné à son hôte.

Le champ facultatif `support` d'une forme (`extrusions[]`) la pose hors de sa vue d'esquisse : `{ "genre": "plan", "plan": <nom> }` (plan de référence d'axe ou tourné : la forme suit ses contraintes et son angle) ou `{ "genre": "repere", "origine": [x, y, z], "horizontal": [...], "vertical": [...] }` (repère orthonormé figé, pris sur la face d'un solide). Avec un support, `profil` et `profondeur` sont des coordonnées locales : faces de `plan` vide, `decalage` en mm (h, v dans le plan, profondeur suivant la normale h × v). Sans support, les sommets sont verrouillés sur les plans d'axe de la vue `vue`. Le plan de dessin actif de l'éditeur n'est pas enregistré.

### Éléments 2D des familles

`famille.elements_2d[]` est la seule liste du dessin 2D d'une famille. Chaque élément porte `vue` (`elevation`, `plan`, `coupe`), `forme` (forme d'annotation du projet : `polyligne`, `arc` par trois points, `cercle`, `spline`, `ligne`, `texte`, `etiquette`…), `hauteur` (texte, mm papier) et un `ancrage` facultatif.

- Sans `ancrage`, les coordonnées de la forme sont fixes dans le repère de la vue.
- Avec `ancrage`, l'élément suit les paramètres : un sommet `{ "h": {"plan", "decalage"}, "v": {"plan", "decalage"} }` par point de la forme, dans l'ordre de ses points (polyligne, spline, zone : sommets ; arc : départ, passage, fin ; ligne : début, fin ; cercle : centre, le rayon reste fixe ; texte et libellé : point d'insertion). Les plans doivent exister et appartenir aux axes de la vue. Les coordonnées écrites dans `forme` sont recalculées à la résolution.
- Une polyligne a au moins deux sommets, trois si elle est fermée (`fermee`) ; seule une polyligne fermée peut porter un `remplissage`. Les autres formes ne sont pas ancrables.
- Une spline ou un cercle porte, comme dans le projet, `type_trait` (un type pour toute la courbe, absent pour le continu fin).
- Une polyligne porte, comme celles du projet, `traits[]` : l'identifiant d'un type de trait (`types_traits[]` du projet) par segment, vide pour le continu fin ; absent si tout est en continu fin. Un identifiant inconnu du projet où la famille est chargée est dessiné en continu fin (pas de refus) : les types du gabarit (`trait-continu-fin`, `trait-tirets`, `trait-axe`…) ont les mêmes identifiants dans tous les projets.

Les éléments de plan et d'élévation des portes, fenêtres et composants sont rendus dans les vues du projet et sur les feuilles (sens d'ouverture, débattement, régions remplies, types de trait avec leurs tirets à l'échelle de la vue). Un élément 2D porte aussi `visible` (paramètre oui/non, facultatif) et `visibilite`, comme les volumes et les formes.

### Visibilité des éléments de famille

Chaque volume (`volumes[]`), forme à profil (`extrusions[]`) et élément 2D (`elements_2d[]`) peut porter `visibilite` : `{ "faible", "moyen", "eleve", "plan", "coupe", "vue_3d" }`, booléens, chacun `true` par défaut ; l'objet entier est omis à l'écriture quand tout est coché, et un champ absent vaut `true` (les fichiers existants restent valides, sans conversion). Une clé inconnue est refusée. Un élément est montré dans une vue du projet si le niveau de détail de la vue (`vues[].niveau_detail`) et son type (plan d'étage : `plan` ; coupe, élévation : `coupe` ; vue 3D : `vue_3d`) sont cochés, et si son paramètre `visible` (« Visible si ») vaut oui. Les éléments 2D d'une vue d'esquisse `plan` relèvent du type plan, ceux d'`elevation` et de `coupe` du type coupe. Un vide perce le mur hôte quel que soit le niveau de détail ou la vue (seul `visible` le retire). L'export IFC prend les éléments d'une vue 3D de niveau moyen.

**Changement incompatible (17/11/2026)** : la liste `symboles[]` (`vue`, `points[]`, `arc`, `fermee`, `remplissage`) est retirée et refusée à la lecture ; un symbole devient un élément 2D ancré (polyligne ou arc). `section_profil` passe de `{"source", "indice"}` à un indice dans `elements_2d`. Aucune conversion des anciens fichiers : les familles du gabarit et des exemples ont été réécrites.

**Végétation et géométrie de rendu (20/11/2026).** `categorie` peut valoir `vegetation` : la famille se place comme un composant (`types_composants`). Un paramètre texte nommé `Rendu` porte la référence de la géométrie montrée dans le style Réaliste à la place de la géométrie de la famille (qui reste celle des plans, coupes, du style Graphique, de la sélection et de l'export) ; références connues : `arbre:feuillu`, `arbre:conifere`, `arbre:fastigie` (générateur intégré de branches et de feuillage, ajusté à l'encombrement de la géométrie de la famille). Référence vide ou inconnue : la géométrie de la famille est rendue. Les familles d'arbres par défaut (`modele::elements::vegetation`) : Largeur = diamètre du houppier, Hauteur = hauteur totale, houppier et tronc en révolution, matériaux par paramètres « Feuillage » et « Écorce », cercle du houppier en plan.

Le modèle de famille porte une `categorie` explicite et peut contenir des `elements_2d` associés à une vue, avec les formes d'annotation du projet et une hauteur de texte en millimètres papier. Les cartouches, étiquettes et profils chargés depuis ce modèle conservent le document complet ; leur rendu ou leur section est recalculé depuis la vue en plan. Un profil nécessite une seule polyligne fermée en plan. Les profils embarqués du gabarit sont définis par des plans contraints et des formules dans leur document de famille. L'ajout de `categorie` modifie le JSON des familles ; les anciens fichiers ne sont pas migrés.

**Changement incompatible (18/11/2026)** : `document_famille` devient obligatoire dans `profils[]`, `familles_cartouches[]` et `familles_etiquettes[]`. Les champs `parametres[]` et `contour[]` des profils, `traits[]`, `textes[]` et `formes[]` des cartouches et des étiquettes sont retirés (ignorés à la lecture) ; un projet dont un profil, un cartouche ou une étiquette n'a pas de `document_famille` est refusé. Les fichiers spécialisés `bimedit.profil` (`.profil.json`), `bimedit.cartouche` et `bimedit.etiquette` ne sont plus lus ni écrits : ces familles s'échangent en `.famille.json`. Aucune conversion : le gabarit et les exemples ont été réécrits (profils des exemples remplacés par ceux du gabarit, mêmes identifiants).
