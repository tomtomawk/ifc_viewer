# `interface/src/documenter/`

Rôle : documents de sortie.

| Fichier | Contenu |
|---|---|
| `feuilles.rs` | jeux de feuilles, vues placées, rendu (`rendu`), édition des cartouches et étiquettes |
| `cartouche.rs` | édition graphique du cartouche (`peindre`) |
| `coupe.rs` | plan de coupe, dessin et export de la vue, boîte de sélection 3D, plage de vue |
| `gabarits_vues.rs` | éditeur des gabarits de vue |

Dépendances typiques : `outils/dessiner/dessin_plan/`, `outils/annoter/outils_texte.rs`, `familles/`, `zones/ruban/`.

Vérifier : `cargo test -p interface --release`.
