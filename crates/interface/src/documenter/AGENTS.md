# `interface/src/documenter/`

Rôle : documents de sortie.

| Fichier | Contenu |
|---|---|
| `feuilles.rs` | jeux de feuilles, vues placées, rendu (`rendu`), ouverture des cartouches et étiquettes dans l'éditeur de familles |
| `cartouche.rs` | peinture d'un rendu 2D de famille dans un cadre de vue (`peindre`) |
| `coupe.rs` | plan de coupe, dessin et export de la vue, boîte de sélection 3D, plage de vue |
| `gabarits_vues.rs` | éditeur des gabarits de vue |
| `editeur_hachure.rs` | éditeur de hachures dessinées (onglet : tuile répétée, bandeau, validation en familles de traits) |

Dépendances typiques : `outils/dessiner/dessin_plan/`, `outils/annoter/outils_texte.rs`, `familles/`, `zones/ruban/`.

Vérifier : `cargo test -p interface --release`.
