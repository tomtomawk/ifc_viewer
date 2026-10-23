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
| `niveaux[]` | `id`, `nom`, `altitude`, `origine` | Triés par altitude |
| `types_murs[]`, `types_dalles[]` | `id`, `nom`, `couches[]` (`materiau`, `epaisseur`), `couches_detaillees` | Murs : couches de l'extérieur vers l'intérieur ; dalles : du haut vers le bas. `couches_detaillees` = false : une couche de l'épaisseur totale (détail non lu) |
| `types_ouvertures[]` | `id`, `nom`, `genre`, `largeur`, `hauteur` | Revit seulement en v0 (cotes du type) |
| `murs[]` | `id`, `nom`, `type_mur`, `niveau`, `decalage_bas`, `haut`, `axe`, `origine` | `haut` : `{contrainte: libre, hauteur}` ou `{contrainte: niveau, niveau, decalage}` |
| `axe` | `{forme: droit, debut, fin}` ou `{forme: arc, debut, fin, centre, direct}` | Ligne médiane de l'épaisseur, en plan (x, y) |
| `ouvertures[]` | `id`, `nom`, `genre` (porte, fenetre, baie), `mur`, `type_ouverture`, `abscisse`, `largeur`, `hauteur`, `allege`, `representation`, `origine` | Baie rectangulaire à travers le mur ; `abscisse` = milieu de la baie le long de l'axe ; `allege` au-dessus du niveau du mur ; `representation` = menuiserie lue (maillage) |
| `dalles[]` | `id`, `nom`, `type_dalle`, `niveau`, `decalage`, `contours`, `origine` | `decalage` = dessus au-dessus du niveau ; `contours` en plan, pair-impair (trous) |
| `importes[]` | `id`, `nom`, `categorie`, `niveau`, `maillage`, `raison`, `origine` | `maillage` : `sommets` [x, y, z], `triangles` [i, j, k] |
| `ignores` | catégorie → nombre | Éléments sans géométrie ou hors périmètre (pièces, quadrillages, espaces, ouvertures IFC vides, éléments 2D) |

Volumes déduits des paramètres (validation) : mur = longueur d'axe × épaisseur du type ×
hauteur − baies (bornées à la hauteur du mur) ; dalle = aire des contours × épaisseur du type.

## Conversion depuis Revit

Uniquement à partir des données déjà décodées par `rvt::maquette` (aucune lecture nouvelle du
format en v0).

| Élément | Paramètres | Limites |
|---|---|---|
| Niveaux | nom, altitude | |
| Murs `SWall`, `ArcWall` | type (épaisseur), niveau de base, décalage, niveau haut et décalage (ou hauteur libre), axe | Le tracé lu donne la droite ou le cercle porteur, pas les extrémités : extrémités et ligne médiane viennent de la géométrie projetée sur ce tracé (indépendant de la ligne d'implantation). Aux angles, la géométrie inclut la jonction. Couches du type non lues |
| Portes, fenêtres hébergées par un mur converti | largeur et hauteur du type, abscisse et allège du point d'insertion | Cotes du type ≠ réservation découpée dans le mur ; sans cotes de type : encombrement de la famille |
| Sols, plafonds prismatiques | contour du dessus, épaisseur du type, décalage | Sols en pente : importés |
| Murs-rideaux, toits, escaliers, garde-corps, terrain, autres familles, formes d'un `.rfa` | — | Importés |

Dans un document de famille de fenêtre ou de porte, le mur hôte du gabarit est percé par le
vide de la famille, pas par une instance : son écart de volume le laisse importé.

## Conversion depuis IFC

| Élément | Paramètres | Limites |
|---|---|---|
| `IfcBuildingStorey` | nom, élévation | |
| `IfcWall`, `IfcWallStandardCase`, `IfcWallElementedCase` | axe (représentation `Axis` : droite ou arc ; sinon plus longue arête horizontale), ligne médiane, base et hauteur lues dans la géométrie, couches de `IfcMaterialLayerSet(Usage)` du mur ou de son type | Axe en polyligne : importé. Ordre des couches = ordre du fichier |
| `IfcOpeningElement` (via `IfcRelVoidsElement`) | encombrement borné au mur ; genre selon `IfcRelFillsElement` (`IfcDoor`, `IfcWindow`) | Ouvertures non rectangulaires : approchées par leur encombrement |
| `IfcSlab` prismatique | contour du dessus, couches | Dalles en pente : importées |
| Autres produits avec géométrie | — | Importés, catégorie = classe IFC |

## Mesure (rapport)

Le rapport donne : part d'éléments paramétriques par catégorie ; pour les murs et les dalles
reconnus, l'écart de volume entre paramètres et géométrie lue (répartition ≤ 1 %, ≤ 5 %,
> 5 %, médiane, écart global) et les plus forts écarts ; les raisons des imports ; les
éléments non convertis.

Tests sur des fichiers hors dépôt (sautés s'ils sont absents) : `cargo test --release
depuis_rvt -- --nocapture` convertit `Projet1.rvt` (`RVT_ESSAIS`) et `Kerreut 2025.rvt`
(`RVT_CORPUS`), écrit leurs rapports dans `exports/` ; `RVT_FAMILLES=<dossier>` convertit
chaque `.rfa` du dossier.

## Hors v0 (prochaines itérations)

- Revit : couches des types de murs et de sols, contours d'esquisse des sols, extrémités du
  tracé des murs, ligne d'implantation, cotes de réservation des familles.
- Jonctions de murs, murs attachés aux toits, profils de murs non rectangulaires.
- Toits, escaliers, poteaux, poutres paramétriques ; familles paramétriques (portes,
  fenêtres) à la place des maillages de menuiserie.
- Affichage du modèle natif par le viewer (une seule chaîne d'affichage), export IFC.
- Maillages des éléments importés dans un fichier binaire à côté du JSON (taille).
