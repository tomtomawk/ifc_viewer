# `interface/src/saisie/`

Rôle : interaction commune aux outils de la vue et aux éditeurs.

| Fichier | Contenu |
|---|---|
| `accrochage.rs` | choix du point accroché, contraintes (Maj), marqueur ; `AccrocheMur` et `marqueur` importés par `lib.rs` |
| `reperage.rs` | guides temporaires (repérage polaire) |
| `saisie_trace.rs` | saisie clavier des points (distance, angle, coordonnées) |
| `saisie_plan.rs` | saisie en plan : création des murs, dalles et vues natives, aperçus, axes |
| `selection.rs` | clic, rectangle, cadrage |
| `poignees_annotation.rs` | poignées des annotations du projet |

Dépendances typiques : `outils/dessiner/dessin_plan/` (tracé 2D), `zones/ruban/` (`Outil`), `outils/modeliser/bim.rs`.

Vérifier : `cargo test -p interface --release`.
