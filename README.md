# IFC Viewer (Rust)

Visionneuse IFC native, écrite de zéro : parseur STEP, géométrie et rendu OpenGL. Aucune dépendance IFC externe.

## Utilisation

```
cargo run --release -- chemin\vers\fichier.ifc
cargo run --release -- --stats chemin\vers\fichier.ifc   # mode console : comptages, volumes, entités ignorées
```

Ouverture aussi possible par Ctrl+O ou par glisser-déposer.

| Action | Commande |
|---|---|
| Orbite (autour du point visé) | clic gauche + glisser |
| Panoramique | clic droit / milieu, ou Maj + clic gauche |
| Zoom vers le curseur | molette |
| Sélection | clic |
| Cadrer l'objet / tout | double-clic / F |
| Masquer / isoler la sélection | H / I (Échap annule l'isolement) |

## Fonctions

- IFC2X3, IFC4, IFC4X3 (fichiers `.ifc` texte ; pas `.ifczip` ni `.ifcXML`).
- Arborescence spatiale (projet > site > bâtiment > niveaux, éléments groupés par classe), visibilité par niveau et par classe, recherche par nom, classe ou GlobalId.
- Propriétés : attributs, type, niveau, matériaux avec épaisseurs de couches, Psets et Qto (y compris ceux du type), unités converties en m / m² / m³.
- Couleurs : IfcStyledItem, puis couleur du matériau, puis couleur par défaut de la classe. Transparence gérée.
- Géométrie : IfcExtrudedAreaSolid, IfcRevolvedAreaSolid, IfcFacetedBrep, IfcAdvancedBrep (facettisé), IfcTriangulatedFaceSet, IfcPolygonalFaceSet, IfcShellBased/FaceBasedSurfaceModel, IfcMappedItem, IfcBooleanResult / IfcBooleanClippingResult (y compris IfcHalfSpaceSolid et IfcPolygonalBoundedHalfSpace), IfcCsgSolid, IfcBlock, IfcRightCircularCylinder, IfcSphere, IfcSweptDiskSolid.
- Profils : rectangle (creux), cercle (creux), ellipse, I, L, U, T, C, Z, arbitraire avec ou sans vides, dérivé, composite. Courbes : polyligne, IfcIndexedPolyCurve (arcs), composite, cercle/ellipse rognés, B-spline (rationnelle).
- Ouvertures (IfcRelVoidsElement) soustraites par CSG BSP : les portes et fenêtres sont visibles dans les murs.
- Grandes coordonnées (géoréférencement) : le rendu est recentré pour éviter les tremblements en f32.

## Architecture

| Fichier | Rôle |
|---|---|
| `src/step.rs` | Parseur ISO 10303-21 (découpage séquentiel, analyse parallèle avec rayon, décodage `\X2\` pour les accents) |
| `src/model.rs` | Unités, relations (agrégation, contenu, propriétés, type, matériaux, ouvertures), styles |
| `src/geom.rs` | Placements, profils, courbes, solides → triangles en mètres |
| `src/csg.rs` | Booléens sur maillages par arbre BSP (portage de csg.js) |
| `src/scene.rs` | Chargement complet, tampons de sommets, picking CPU par rayon |
| `src/render.rs` | Shaders et dessin OpenGL (glow) via un PaintCallback egui |
| `src/camera.rs` | Caméra orbitale Z vers le haut, perspective / orthographique |
| `src/app.rs` | Interface egui (panneaux, arborescence, propriétés, interaction 3D) |

## Limites connues

- Les surfaces NURBS des IfcAdvancedBrep sont approchées par leurs arêtes : les faces fortement courbes sont facettisées grossièrement.
- La CSG est sautée pour les hôtes de plus de 6 000 triangles ou les ouvertures de plus de 3 000 triangles (performance) ; l'élément est alors affiché plein.
- Non pris en charge : IfcSurfaceCurveSweptAreaSolid, IfcFixedReferenceSweptAreaSolid, IfcSectionedSolid, alignements IFC4X3, textures. Les entités ignorées sont listées dans la barre d'état et dans `--stats`.

## Tests

```
cargo test
cargo run --release -- --stats samples/test_ifc4.ifc
```

`samples/test_ifc4.ifc` couvre mur percé, dalle à profil avec arc indexé, item mappé coloré, faces triangulées et découpe par demi-espace ; les volumes attendus sont en commentaire dans le fichier.
