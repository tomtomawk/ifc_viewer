# IFC Viewer (Rust)

Visionneuse IFC native, écrite de zéro : parseur STEP, géométrie et rendu OpenGL. Aucune dépendance IFC externe.
Ouvre aussi les fichiers Revit (`.rvt`) : murs, sols, toits, escaliers, garde-corps, portes, fenêtres, mobilier et autres familles, pièces et grilles lus directement dans le fichier, sans export IFC ; feuilles affichées avec leur cartouche, leurs vues placées, leurs titres, textes et cotes.

## Édition BIM native

Le parcours d'édition est décrit dans [docs/EDITION.md](docs/EDITION.md) : import
IFC/Revit en document natif, murs courbes et ouvertures, modifications réversibles,
niveaux, quadrillages, trémies, jonctions, attachements aux toitures, annotations,
feuilles et nomenclatures, exports IFC4/SVG/PDF/CSV.

Pour essayer un projet complet :

```sh
cargo run --release -- samples/maison_native.modele.json
```

## Utilisation

```
cargo run --release -- chemin\vers\fichier.ifc
cargo run --release -- --stats chemin\vers\fichier.ifc   # mode console : comptages, volumes, entités ignorées
cargo run --release -- chemin\vers\projet.rvt
cargo run --release -- --stats chemin\vers\projet.rvt    # métadonnées, niveaux, éléments par catégorie, volumes, causes d'échec
cargo run --release -- --parametres chemin\vers\projet.rvt   # valeurs de paramètres décodées (élément, paramètre, stockage, valeur brute)
cargo run --release -- --modele chemin\vers\projet.rvt     # conversion vers le modèle natif v0 (JSON) et rapport de conversion
```

Ouverture aussi possible par Ctrl+O ou par glisser-déposer.

**Début → Nouveau…** (Ctrl+N) crée et ouvre un projet natif v0 vide
(`.modele.json`), selon [le schéma du modèle](docs/MODELE.md).
**Sortie → Enregistrer** (Ctrl+S) l'enregistre. Les niveaux et murs droits natifs
sont affichés et peuvent être enregistrés puis rouverts.

Avec un fichier Revit ouvert, **Sortie → Enregistrer** (Ctrl+S) propose un fichier
`.modele.json` au premier enregistrement, puis réutilise cette destination pour
ce document. La conversion v0 s'exécute en arrière-plan, écrit aussi le rapport
`.modele.rapport.md` et conserve le RVT et ses vues ouverts. Un RVT modifié sur
disque doit être rouvert avant conversion. Le fichier RVT source reste intact.

Le ruban regroupe les commandes dans Début, Modéliser, Modifier, Documenter et Sortie.
**Début → Options → Navigation IFC** (désactivée par défaut) affiche, pour les modèles Revit et natifs, l'arborescence spatiale IFC, la liste des classes et les attributs IFC des propriétés (classe, Id STEP, GlobalId, bouton Copier GlobalId). Le même menu choisit le gabarit. Les fichiers IFC gardent toujours cette navigation, qui est leur seule arborescence. L'option est enregistrée dans `%APPDATA%\ifc_viewer\options.json` (Windows) ou `~/.config/ifc_viewer/options.json`.
Les commandes fréquentes (Mur, Éléments horizontaux, Ouverture, Déplacer, Copier, Aligner, Cote et
Texte) disposent de grands boutons lorsque la largeur le permet ; les autres
restent empilées avec de petites icônes. Les grands boutons se replient selon leur
priorité quand la fenêtre rétrécit. Le bouton Éléments horizontaux superpose sol, plafond et toiture dans une seule icône ; la sorte (Dalle, Sol, Plafond, Toiture) et son type se choisissent dans le sélecteur de type en tête de Propriétés : un onglet par sorte, puis des types prédéfinis (dalles BA ou bois, sols carrelage, parquet, PVC, résine, plafonds plaque de plâtre ou dalles acoustiques, toitures tuiles, terrasse, bac acier, végétalisée) avec leur composition ; l'épaisseur est celle du type. **Dalle**, **Sol**, **Plafond** et **Toiture** sont opérationnels dans les projets natifs : contour par sommets (recliquer le premier ou Entrée pour fermer) ou rectangle, accroché aux angles des murs ; une couleur par couche. Le plafond se cale par sa hauteur sous plafond, la toiture par sa sous-face ; la pente se donne arête par arête (croupes, deux pans, plat), dans les propriétés ou en cliquant les repères des arêtes dans la vue. Les types, matériaux et types de vues viennent du gabarit (`gabarits/defaut.gabarit.json`, autre gabarit au choix dans Début → Options) ; détails dans `docs/MODELE.md`.
Les projets natifs ont une arborescence (vue 3D, plans d'étage, coupes, types, matériaux) ; l'onglet **Documenter** crée les vues en plan (une par niveau) et, avec un même outil, les coupes et les élévations (type choisi dans le sélecteur, deux clics dans un plan), enregistrées dans le projet ; une vue ouverte se renomme, se retourne ou se supprime dans Propriétés. Son sélecteur se trouve dans un encart en tête de
Propriétés, avec une vignette et une liste déroulante distinguant Dalle (structure),
Sol (revêtement), Plafond et Toiture. Les paramètres sont juste en dessous et
restent mémorisés séparément pour chaque catégorie. Les vignettes distinguent la
dalle épaisse, le sol carrelé mince, le plafond suspendu et la toiture à deux pans.
L'icône Mur présente une coupe en trois couches avec une face lisse.
Modéliser comprend aussi Poteau, Escalier et Garde-corps.
Sur une fenêtre étroite, le bouton « ... » à droite
de la colonne de l'outil actif donne accès à ses réglages ; Éléments horizontaux conserve son
sélecteur et ses paramètres dans Propriétés à toutes les largeurs.
Les onglets d'édition présentent leurs groupes d'outils et les options de l'outil
sélectionné directement à droite de son icône (Ouverture : Porte/Fenêtre ; Cote : Alignée/Orthogonale ; Texte :
avec/sans flèche). **Mur** est opérationnel dans les projets natifs : régler
hauteur, épaisseur, implantation et niveau de base, puis cliquer le départ et
l'extrémité, ou taper la longueur au clavier puis Entrée (direction du curseur).
L'aperçu montre le mur, sa longueur et son angle. Accrochages : extrémités (carré)
et milieux (triangle) des murs du niveau, angles multiples de 15° à 3° près (ligne
guide) ; Maj contraint le tracé horizontalement ou verticalement. L'option Chaîner
permet de continuer depuis le dernier point ; revenir au premier point de la chaîne
l'arrête. Échap efface la saisie, puis annule le segment en cours, puis quitte
l'outil ; le clic droit arrête le segment. Le niveau 0 est créé au premier
mur si le projet est vide. Les outils complémentaires disponibles et leurs limites sont décrits dans [Édition BIM](docs/EDITION.md).
**Murs composés** : choisir un type dans Propriétés (ou en créer un avec « + » :
couches, fonction, matériau, couleur, motif de coupe, épaisseur ; glisser la poignée ≡ d'une
couche pour la déplacer). Chaque couche est
tracée comme un mur indépendant, regroupé en assemblage : un clic sélectionne
l'assemblage, un second clic (ou Tab) la couche, dont la hauteur, les niveaux, la
phase et le sous-projet se règlent seuls ; « Dissocier » la sort de l'assemblage.
Les murs droits se raccordent automatiquement : angles en onglet (couche à couche pour
deux assemblages de même composition), arrivées en T arrêtées sur la face du mur.
Un astérisque signale les modifications et la fermeture propose de les enregistrer. Une seconde
barre, au-dessus de la zone de dessin, affiche les vues ouvertes : vue générale du
modèle, plans, coupes, vues 3D, feuilles et nomenclatures. Cliquer sur un onglet
retrouve son cadrage et ses réglages d'affichage ; la croix au survol ou le clic du milieu
ferme la vue. Une vue fermée peut être rouverte depuis l'arborescence du projet.
L'ouverture d'un autre fichier conserve les documents précédents et leurs onglets.
Les onglets affichent uniquement le nom de la vue ; le nom du fichier reste dans
l'infobulle et chaque fichier garde sa couleur discrète. Les deux petites flèches
affichent la liste des onglets ouverts ; la barre d'onglets ne défile pas.

Exécutables prêts à l'emploi, sans installation : page [Releases](https://github.com/tomtomawk/ifc_viewer/releases) (Windows x64 et ARM64, Linux x64 et ARM64, macOS ARM64). Une release est publiée par GitHub Actions (`.github/workflows/release.yml`) quand on pousse une étiquette `v*` :

```
git tag v0.2.0
git push origin v0.2.0
```

Fenêtre : barre dessinée par l'application ; sous Windows 10 (build antérieur à 22000),
cadre du système (barre de titre et bords natifs), la fenêtre sans décoration y décalant
l'affichage par rapport aux clics. `IFC_VIEWER_CADRE=natif` ou `IFC_VIEWER_CADRE=perso`
force l'un ou l'autre.

| Action | Commande |
|---|---|
| Orbite (autour du point visé, vue 3D) | clic gauche + glisser |
| Navigation en vol (vue 3D, comme la vue Scène d'Unity) | clic droit maintenu : la souris oriente le regard ; Z/S avancer/reculer, Q/D gauche/droite, A/E descendre/monter (W/S, A/D, Q/E en QWERTY : touches lues par position) ; Maj : vitesse ×3 ; molette : vitesse |
| Panoramique | clic milieu ou Maj + clic gauche ; clic gauche et droit aussi dans une vue 2D (plan, coupe, élévation, feuille) |
| Zoom vers le curseur | molette |
| Sélection | clic |
| Cadrer l'objet / tout | double-clic / F |
| Masquer / isoler la sélection | H / I (Échap annule l'isolement) |
| Orientation (vue 3D) | cube d'orientation, coin haut droit : clic sur une face, une arête ou un coin ; glisser pour tourner. Boussole autour : clic sur N, E, S, O ; glisser l'anneau pour tourner autour de la verticale |
| Orientation (vue en plan) | repère plat en haut à droite : glisser l'anneau pour tourner (aimanté tous les 15°, Maj : libre), N/E/S/O pour placer une direction en haut, −90°/+90°, angle saisi en degrés ; « PLAN » retrouve l’orientation initiale de la vue ; « Aligner sur… » puis clic sur un élément : sa direction principale devient horizontale (Échap annule). L'outil Mur suit les axes du plan tourné (Maj, angles ronds). Zoom et cadrage conservés. Disponible aussi en vue orthographique de dessus/dessous du modèle. |
| Barre sous la vue | dans l'ordre : échelle (1 : n), niveau de détail (lu dans la vue Revit ; sans effet sur l'affichage pour l'instant), style visuel, cadrer, visibilité / tout afficher ; puis Ortho (vue 3D : projection orthographique ou perspective) |
| Visibilité par catégorie (modèle, annotation) | bouton « Visibilité » sous la vue, ou V |
| Déplacer Propriétés / Arborescence | glisser le titre ; une zone magnétique prévisualise l'ancrage au bord gauche ou droit, relâcher pour ancrer |
| Style visuel | barre sous la vue : Graphique (fond blanc, faces colorées et éclairées, arêtes ; cases « Couleurs des faces », décochée = dessin au trait, et « Éclairage », décochée = aplats) et Réaliste (couleurs de rendu des matériaux, ciel et soleil, sans arêtes) ; dans les deux, plans et coupes sont coupés (poché hachuré en Graphique, couleur des matériaux en Réaliste) et les cases ombres portées (calées sur la vue) et ombre ambiante s'ajoutent ; traits, cotes et textes jamais plus fins qu'un pixel (stables au zoom) |
| Zone de coupe 3D | bouton « Zone de coupe » sous la vue : boîte qui découpe le modèle dans les vues 3D ; poignées au milieu des faces pour la régler en 3D, en plan (côtés) ou en élévation (dessus, dessous) ; bornes dans les propriétés |
| Plan de coupe | propriétés, « Plage de vue » (style Lignes cachées) : coupe, niveau, décalage, poché |
| Export vectoriel du dessin | ruban, onglet « Exporter » : « Dessin SVG… » (mode dessin, vue orthographique) |

## Fonctions

- Compositions Revit : couches des types de murs et de sols, avec épaisseur, matériau et ordre, affichées dans les propriétés et conservées dans le v0. Sur les murs droits dont le côté extérieur peut être vérifié par les matériaux des faces, les coupes et la section box distinguent les couches, leurs couleurs et les motifs de coupe des matériaux. Les compositions variables et les orientations ambiguës gardent un rendu global.

- Zone de coupe 3D (section box) : les six faces coupées sont refermées avec la couleur de l'élément ; en style Lignes cachées, elles utilisent le poché et les hachures du dessin. Les cavités restent vides et le remplissage respecte les bornes de la boîte.

- IFC2X3, IFC4, IFC4X3 (fichiers `.ifc` texte ; pas `.ifczip` ni `.ifcXML`).
- Arborescence en onglets, choisis par une barre d'icônes : pour un fichier Revit, tout le navigateur de projet ou une seule de ses sections (vues, feuilles, nomenclatures, familles, groupes, liens), matériaux (avec les éléments qui les portent : un clic les isole), modèle et classes ; pour un IFC, modèle, classes et matériaux (`IfcMaterial`).
- Arborescence spatiale (projet > site > bâtiment > niveaux, éléments groupés par classe), visibilité par niveau et par classe, recherche par nom, classe ou GlobalId.
- Propriétés : attributs, type, niveau, matériaux avec épaisseurs de couches, Psets et Qto (y compris ceux du type), unités converties en m / m² / m³.
- Couleurs : IfcStyledItem, puis couleur du matériau, puis couleur par défaut de la classe. Transparence gérée.
- Géométrie : IfcExtrudedAreaSolid, IfcRevolvedAreaSolid, IfcFacetedBrep, IfcAdvancedBrep (facettisé), IfcTriangulatedFaceSet, IfcPolygonalFaceSet, IfcShellBased/FaceBasedSurfaceModel, IfcMappedItem, IfcBooleanResult / IfcBooleanClippingResult (y compris IfcHalfSpaceSolid et IfcPolygonalBoundedHalfSpace), IfcCsgSolid, IfcBlock, IfcRightCircularCylinder, IfcSphere, IfcSweptDiskSolid.
- Profils : rectangle (creux), cercle (creux), ellipse, I, L, U, T, C, Z, arbitraire avec ou sans vides, dérivé, composite. Courbes : polyligne, IfcIndexedPolyCurve (arcs), composite, cercle/ellipse rognés, B-spline (rationnelle).
- Ouvertures (IfcRelVoidsElement) soustraites par CSG BSP : les portes et fenêtres sont visibles dans les murs.
- Style Graphique, faces blanches (ancien « Lignes cachées ») : dessin 2D des plans et élévations, à la manière d'un logiciel de DAO ([CADCraft](https://github.com/storytold/cadcraft)) : fond blanc, faces blanches, arêtes vives et silhouettes des surfaces courbes en trait fin, parties cachées éliminées. Coupe horizontale à une hauteur au-dessus d'un niveau (1,20 m par défaut) : parties coupées en poché (hachuré, plein ou vide) et contour en trait fort (0,35 mm), parties au-dessus retirées. Épaisseur et pas des hachures en mm du papier à l'échelle choisie (celle de la vue Revit ouverte, sinon 1:100). Activé d'office à l'ouverture d'un plan d'étage (coupe sur son niveau), d'un plan de plafond, d'une coupe ou d'une élévation Revit. Export SVG (mm du papier, un groupe par rôle : poché, hachures, vu, coupe, annotations) : coupe, arêtes vues sans leurs parties cachées (tampon de profondeur calculé à l'export, 4 096 pixels sur le grand côté), éléments 2D de la vue Revit ouverte.
- Grandes coordonnées (géoréférencement) : le rendu est recentré pour éviter les tremblements en f32.
- Revit (`.rvt`, `.rte`, formats 2016 à 2027) : murs, sols, plafonds, toits, escaliers, garde-corps, pièces et instances de familles (portes, fenêtres, mobilier…) avec la géométrie calculée par Revit (jonctions et ouvertures comprises ; familles en vue 3D, détail fin), colorés par matériau, rangés par niveau, avec type, contraintes, hauteurs, décalages et volume ; portes et fenêtres avec catégorie, hôte, repère et cotes du panneau ; grilles dans l'arborescence ; métadonnées (version, informations projet, références externes, vignette) dans la « Fiche Revit ». Familles (`.rfa`) : formes (extrusions, raccordements, révolutions, balayages, vides). Vues, feuilles, nomenclatures et éléments 2D (lignes, textes, cotes, zones remplies avec leurs hachures), motifs de surface des matériaux sur les faces ; arborescence du projet façon Revit ; paramètres (intégrés, de famille, partagés, de projet) rangés par groupe comme la palette de Revit ; GlobalId = IfcGUID de Revit ; coordonnées (emplacements, point de base, nord géographique, lieu), coordonnées partagées du point cliqué. Format : `LISEZMOI-RVT.md` du dossier de décodage (hors dépôt).

## Architecture

| Fichier | Rôle |
|---|---|
| `src/step.rs` | Parseur ISO 10303-21 (découpage séquentiel, analyse parallèle avec rayon, décodage `\X2\` pour les accents) |
| `src/model.rs` | Unités, relations (agrégation, contenu, propriétés, type, matériaux, ouvertures), styles |
| `src/geom.rs` | Placements, profils, courbes, solides → triangles en mètres |
| `src/csg.rs` | Booléens sur maillages par arbre BSP (portage de csg.js) |
| `src/scene.rs` | Chargement complet, tampons de sommets, picking CPU par rayon |
| `src/render.rs` | Shaders et dessin OpenGL (glow) via un PaintCallback egui |
| `src/trace2d.rs` | Primitives 2D vectorielles (polylignes, segments, surfaces, triangles ; style en mm du papier, rôle, élément d'origine), maillage pour le GPU, export SVG |
| `src/dessin.rs` | Dessin 2D : arêtes vives et douces (silhouettes) des maillages, coupe par un plan (poché, hachures, contours en trait fort), arêtes vues sans parties cachées pour l'export |
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
| `src/modele/` | Modèle natif v0 (format d'édition) et conversions depuis Revit et IFC, rapport de conversion : [docs/MODELE.md](docs/MODELE.md) |
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
- Dessin 2D : les coupes natives verticales sont prises en charge ; les limites de fidélité des vues Revit restent distinctes ; ni éléments au-dessus de la coupe en pointillés, ni épaisseurs de trait par catégorie, ni export DXF (export PDF désormais disponible) ; traits fins d'un pixel à l'écran ; export SVG/PDF des vues orthographiques et des feuilles natives ; les feuilles Revit ne sont pas exportées. Textes, cotes et cartouches Revit passent dans l'export en triangles (pas en texte SVG). Courbes en polylignes (pas d'arcs). En OpenGL ES (Raspberry Pi), les parties au-dessus de la coupe ne sont pas retirées, seuls les objets entièrement au-dessus le sont.
- Non pris en charge : IfcSurfaceCurveSweptAreaSolid, IfcFixedReferenceSweptAreaSolid, IfcSectionedSolid, alignements IFC4X3, textures. Les entités ignorées sont listées dans la barre d'état et dans `--stats`.

## Tests

```
cargo test
cargo run --release -- --stats samples/test_ifc4.ifc
```

`samples/test_ifc4_murs.ifc` couvre la conversion vers le modèle natif : mur à axe et jeu de couches avec une porte, mur courbe, dalle à couches (volumes attendus en commentaire).

`samples/test_ifc4.ifc` couvre mur percé, dalle à profil avec arc indexé, item mappé coloré, faces triangulées et découpe par demi-espace ; les volumes attendus sont en commentaire dans le fichier.

Les tests Revit (`src/rvt/maquette.rs`) lisent des fichiers hors dépôt, avec des valeurs relevées dans Revit 2026 : `essais\2026\E04_mur.rvt` (6,711 562 m³, boîte 12 345,6 × 200 × 2 718,2 mm), `essais\2026\E12_grilles.rvt` (3 grilles), `fichiers\Projet1.rvt` (59 murs, matériaux, sols, escaliers, garde-corps, pièces, instances et géométrie des familles, cotes d'une porte) et `fichiers\Kerreut 2025.rvt`, copie d'un projet du corpus d'agence (10 toits, escalier assemblé, familles). Ces fichiers sont dans le dossier du décodage `C:\Users\theba\Projets code\Decodage RVT` (essais, fichiers de référence, sorties de l'API de Revit, scripts RiveTT ; provenance de chaque fichier dans son `SOURCES.md`), remplaçable par la variable `RVT_ESSAIS` ; fichier absent : test sauté avec un message.

Le décodage RVT est fait dans ce dépôt ; [rvt-rs](https://github.com/DrunkOnJava/rvt-rs) (Apache-2.0) n'a été consulté que partiellement, comme référence de comparaison, sans reprise de code (détail : `LISEZMOI-RVT.md` § 10, dossier de décodage hors dépôt).
