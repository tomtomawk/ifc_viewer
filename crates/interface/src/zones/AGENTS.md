# `interface/src/zones/`

Rôle : zones d'écran autour de la vue.

| Fichier | Contenu |
|---|---|
| `ruban/` | ruban d’édition (`Outil`, `Menu`, `RubanEdition`), sélecteurs de type, icônes, propriétés des murs et ouvertures ; éditeur de type composé commun aux murs et aux terrains (`editeur_type.rs`) ; tests dans `ruban/tests.rs` |
| `barre_haute.rs` | barre de titre, ruban, bords de fenêtre (`top_bar`) ; `include_bytes!("../../assets/icone.png")` |
| `barre_vue.rs` | barre du bas de la vue (`barre_vue` : aiguille vers la famille ou le projet ; sections échelle/détail, apparence, cadrage, visibilité ; styles, fond de plan, visibilité par catégorie) |
| `palette_creation.rs` | palette de création : `definition(outil)` (match exhaustif sur `Outil`) classe chaque outil ; `palette_creation` appelle sélecteur, schéma, paramètres dans cet ordre. Nouvel objet : remplir `definition`, pas de panneau ad hoc ; emplacements dans `outils/modeliser/bim/palettes.rs`, `poutres.rs`, `poteau.rs`, `zones/ruban/` (mur, surfaces, vues), `outils/dessiner/dessin_plan/terrain_palette.rs` |
| `fiche_selection.rs` | fiche de l'objet sélectionné (Modifier) : même schéma que la palette (sélecteur, puis paramètres), `fiche(genre)` exhaustif |
| `voilage_vue.rs` | ce qui se dessine par-dessus la vue : cube d'orientation et projection (3D), repère d'orientation du plan (2D). Bandeau de dessin : `outils/bandeau.rs` ; messages : `socle/messages.rs` |
| `panneaux.rs` | panneaux ancrables (`panneau`) : contenu, en-tête, ancrage |
| `volet_proprietes.rs` | volet Propriétés (`volet_proprietes`), visibilité de la sélection |
| `volet_arborescence.rs` | volet Arborescence (`volet_arborescence`) ; contenu des sections dans `navigateur.rs` |
| `navigateur.rs` | navigateur de projet, matériaux, arborescence IFC |
| `proprietes.rs`, `communes.rs` | propriétés individuelles, terrains et dessins 2D |
| `proprietes_type.rs` | section « Propriétés du type » (`proprietes_type`) : grisée, déverrouillée par double-clic (`type_deverrouille`), brouillon pendant la saisie (`type_brouillon`), écriture puis propagation (`repercuter` : `ActualiserType` par assemblage, dimensions des ouvertures) |
| `proprietes_multiples.rs`, `proprietes_multiples/` | champs communs et par catégorie, comparaison « Varié », application explicite sur les occurrences sélectionnées |
| `donnees_importees.rs` | propriétés, matériau et couleur lus d'un élément importé (lecture seule, appelé depuis `proprietes.rs`) |
| `menu_vue.rs` | clic droit dans la vue, poignée de milieu de mur |
| `vue_native.rs`, `vue_revit.rs` | propriétés des vues natives et sélecteur de gabarit ; vues Revit, nomenclatures |
| `graphismes.rs` | fenêtres matériau, plumes |
| `motifs.rs` | motifs de hachures : arborescence, propriétés du motif et ouverture de la vue de dessin |

Dépendances typiques : `socle/`, `outils/` (barres d'options), `familles/` (sélection des familles), `documenter/` (feuilles).

Vérifier : `cargo test -p interface --release` (les tests du ruban : `zones::ruban::tests`).
