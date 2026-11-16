# `interface/src/zones/`

Rôle : zones d'écran autour de la vue.

| Fichier | Contenu |
|---|---|
| `ruban/` | ruban d'édition (`Outil`, `Menu`, `RubanEdition`), sélecteurs de type, icônes, propriétés des murs et ouvertures ; tests dans `ruban/tests.rs` |
| `barre_haute.rs` | barre de titre et de vue (`top_bar`, `barre_vue`) ; `include_bytes!("../../assets/icone.png")` |
| `panneaux.rs`, `panneau_droit.rs` | panneaux ancrables, panneau droit (`panneau`, `right_panel`) |
| `navigateur.rs` | navigateur de projet, matériaux, arborescence IFC |
| `proprietes.rs`, `communes.rs` | propriétés de la sélection native ; propriétés communes d'une sélection multiple |
| `menu_vue.rs` | clic droit dans la vue, poignée de milieu de mur |
| `vue_native.rs`, `vue_revit.rs` | propriétés des vues natives ; vues Revit, nomenclatures |
| `graphismes.rs` | fenêtres matériau, plumes, motifs |

Dépendances typiques : `socle/`, `outils/` (barres d'options), `familles/` (sélection des familles), `documenter/` (feuilles).

Vérifier : `cargo test -p interface --release` (les tests du ruban : `zones::ruban::tests`).
