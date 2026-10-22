# IFC Viewer (Rust)

Visionneuse IFC native, écrite de zéro : parseur STEP, géométrie et rendu OpenGL. Aucune dépendance IFC externe.
Ouvre aussi les fichiers Revit (`.rvt`) : murs, sols, toits, escaliers, garde-corps, portes, fenêtres, mobilier et autres familles, pièces et grilles lus directement dans le fichier, sans export IFC.

## Utilisation

```
cargo run --release -- chemin\vers\fichier.ifc
cargo run --release -- --stats chemin\vers\fichier.ifc   # mode console : comptages, volumes, entités ignorées
cargo run --release -- chemin\vers\projet.rvt
cargo run --release -- --stats chemin\vers\projet.rvt    # métadonnées, niveaux, éléments par catégorie, volumes, causes d'échec
cargo run --release -- --parametres chemin\vers\projet.rvt   # valeurs de paramètres décodées (élément, paramètre, stockage, valeur brute)
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
- Revit (`.rvt`, `.rte`, formats 2016 à 2027) : murs, sols, plafonds, toits, escaliers, garde-corps, pièces et instances de familles (portes, fenêtres, mobilier…) avec la géométrie calculée par Revit (jonctions et ouvertures comprises ; familles en vue 3D, détail fin), colorés par matériau, rangés par niveau, avec type, contraintes, hauteurs, décalages et volume ; portes et fenêtres avec catégorie, hôte, repère et cotes du panneau ; grilles dans l'arborescence ; métadonnées (version, informations projet, références externes, vignette) dans la « Fiche Revit ». Familles (`.rfa`) : formes (extrusions, raccordements, révolutions, balayages, vides). Vues, feuilles, nomenclatures et éléments 2D (lignes, textes, cotes, zones remplies avec leurs hachures), motifs de surface des matériaux sur les faces ; arborescence du projet façon Revit ; paramètres (intégrés, de famille, partagés, de projet) rangés par groupe comme la palette de Revit ; GlobalId = IfcGUID de Revit. Format : [docs/LISEZMOI-RVT.md](docs/LISEZMOI-RVT.md).

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
| `src/app.rs` | Interface egui (panneaux, arborescence, propriétés, interaction 3D), commandes du pilote |
| `src/pilote.rs` | Canal de commande local (TCP 127.0.0.1:47811) du serveur MCP |
| `src/rvt.rs` | Revit : conteneur OLE, métadonnées |
| `src/rvt/objets.rs` | Revit : retrait des blocs intercalés, décompression, cadrage des partitions, table des éléments |
| `src/rvt/schema.rs` | Revit : schéma de classes `Formats/Latest` |
| `src/rvt/champs.rs` | Revit : lecteur générique des objets, piloté par le schéma |
| `src/rvt/geometrie.rs` | Revit : B-rep `GElement` → triangles (faces planes, cylindriques, réglées et de révolution, trous, faces courbes densifiées ; vides écartés) ; familles (`GInstance`) : filtres de vue, repère, géométrie des types partagée |
| `src/rvt/vues.rs` | Revit : vues, éléments 2D, hachures et motifs de surface |
| `src/rvt/parametres.rs` | Revit : définitions et valeurs des paramètres, mise en forme, UniqueId et IfcGUID ; tables relevées dans Revit dans `src/rvt/donnees/` |
| `src/rvt/maquette.rs` | Revit : niveaux, types, familles, matériaux et éléments, traduits en modèle IFC synthétique pour l'affichage |
| `tools/viewer-mcp/serveur.py` | Serveur MCP (stdio) qui pilote le viewer : ouverture, vues, caméra, propriétés, captures |

## Pilotage par MCP

Le viewer écoute sur `127.0.0.1:47811` (variable `IFC_VIEWER_PORT`) des commandes d'une
ligne. `tools/viewer-mcp/serveur.py` (Python 3, sans dépendance) les expose en serveur MCP,
déclaré dans `.mcp.json` sous le nom `ifc-viewer` : un agent ouvre un fichier, choisit une
vue Revit, règle la caméra, lit les propriétés, désigne l'objet sous un point et reçoit des
captures PNG. La vue 3D est rendue hors écran (tampon de trame), à la taille demandée, même
fenêtre réduite ; la capture de la fenêtre entière exige qu'elle ne soit pas réduite. Si
aucun viewer n'écoute, le serveur lance `target/release/ifc_viewer.exe` (ou
`IFC_VIEWER_EXE`). Liste des commandes : description de l'outil `viewer_commande`.

## Limites connues

- Les surfaces NURBS des IfcAdvancedBrep sont approchées par leurs arêtes : les faces fortement courbes sont facettisées grossièrement.
- La CSG est sautée pour les hôtes de plus de 6 000 triangles ou les ouvertures de plus de 3 000 triangles (performance) ; l'élément est alors affiché plein.
- Revit : surfaces `HermiteSurf`, `ConeSurf`, `OffsetSurf` et courbes `GEllipse` non décodées ; un solide qui en porte est écarté (le reste de l'élément est affiché) et compté dans les entités ignorées. Contenu des nomenclatures et paramètres calculés par Revit (surfaces, volumes) non lus. GlobalId : IfcGUID de Revit pour les formats 2024 et suivants, synthétiques (`RVT` + ElementId) avant.
- Non pris en charge : IfcSurfaceCurveSweptAreaSolid, IfcFixedReferenceSweptAreaSolid, IfcSectionedSolid, alignements IFC4X3, textures. Les entités ignorées sont listées dans la barre d'état et dans `--stats`.

## Tests

```
cargo test
cargo run --release -- --stats samples/test_ifc4.ifc
```

`samples/test_ifc4.ifc` couvre mur percé, dalle à profil avec arc indexé, item mappé coloré, faces triangulées et découpe par demi-espace ; les volumes attendus sont en commentaire dans le fichier.

Les tests Revit (`src/rvt/maquette.rs`) lisent des fichiers hors dépôt, avec des valeurs relevées dans Revit 2026 : `Essais-format\2026\E04_mur.rvt` (6,711 562 m³, boîte 12 345,6 × 200 × 2 718,2 mm), `Essais-format\2026\E12_grilles.rvt` (3 grilles), `Archives\Projet1.rvt` (59 murs, matériaux, sols, escaliers, garde-corps, pièces, instances et géométrie des familles, cotes d'une porte) et `Kerreut 2025.rvt` du corpus d'agence (10 toits, escalier assemblé, familles). Dossiers par défaut `C:\Users\theba\Desktop\Revit` et `C:\Users\theba\Documents\Thomas\ARCHI PERSO`, remplaçables par les variables `RVT_ESSAIS` et `RVT_CORPUS` ; fichier absent : test sauté avec un message.

Le décodage RVT est fait dans ce dépôt ; [rvt-rs](https://github.com/DrunkOnJava/rvt-rs) (Apache-2.0) n'a été consulté que partiellement, comme référence de comparaison, sans reprise de code (détail : [docs/LISEZMOI-RVT.md](docs/LISEZMOI-RVT.md) § 10).
