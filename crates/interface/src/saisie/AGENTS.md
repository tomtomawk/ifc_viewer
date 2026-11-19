# `interface/src/saisie/`

Rôle : interaction commune aux outils de la vue et aux éditeurs.

| Fichier | Contenu |
|---|---|
| `moteur_accrochage.rs` | moteur unique d'accrochage et de repérage des tracés 2D (projet, familles, plan de dessin actif en 3D) : `SourceAccrochage` (tracés, droites), `viser`, `viser_simple`, `peindre_marqueur`, distance tapée |
| `accrochage.rs` | choix du point accroché, contraintes (Maj), marqueur ; `AccrocheMur` et `marqueur` importés par `lib.rs` |
| `reperage.rs` | guides temporaires (repérage polaire) |
| `saisie_trace.rs` | saisie clavier des points (distance, angle, coordonnées) |
| `saisie_plan.rs` | saisie en plan : création des murs, dalles et vues natives, aperçus, axes |
| `visee.rs` | visée des éléments 2D d'une vue native (annotations, pièces, niveaux, limites) au trait, à `TOLERANCE` px ; prise par la fenêtre de sélection sur les traits |
| `selection.rs` | clic, rectangle, cadrage, sélecteur de type d'un élément seul |
| `selection_multiple.rs` | sélecteur de type de plusieurs éléments : un par sorte d'élément sélectionnée ; `pareils` : élément principal et éléments de même sorte pour le report |
| `poignees_annotation.rs` | poignées des annotations du projet |

Dépendances typiques : `outils/dessiner/dessin_plan/` (tracé 2D), `zones/ruban/` (`Outil`), `outils/modeliser/bim/`, `socle/document_bim.rs`.

Vérifier : `cargo test -p interface --release`.
