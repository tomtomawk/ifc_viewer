# `interface/src/documenter/`

Rôle : documents de sortie.

| Fichier | Contenu |
|---|---|
| `feuilles.rs` | jeux de feuilles : sélection (`SelectionFeuille`, `ObjetFeuille`), état de la vue (`EtatFeuilles`), rendu (`rendu`), création et ordre des feuilles, Suppr, exports |
| `vue_jeu.rs` | vue du jeu ouvert : souris (sélection, glisser aimanté des vues, tableaux, cartouche et annotations), dépôts depuis l'arborescence, outils d'annotation, tracé |
| `annotations_feuille.rs` | outils d'annotation de feuille (`TraceFeuille`), accrochage, aperçu, barre d'outils, texte saisi en place |
| `proprietes_feuilles.rs`, `reglages_feuille.rs` | Propriétés du jeu, de la feuille, de la vue placée, du tableau ; réglages du cartouche (position, ancrage, suppression) et des annotations |
| `familles_feuilles.rs` | ouverture et enregistrement des familles de cartouche et d'étiquette dans l'éditeur de familles |
| `cartouche.rs` | peinture d'un rendu 2D de famille dans un cadre de vue (`peindre`) |
| `coupe.rs` | plan de coupe, dessin et export de la vue, boîte de sélection 3D, plage de vue |
| `gabarits_vues.rs` | éditeur des gabarits de vue |
| `editeur_hachure.rs` | édition graphique des motifs, dont ceux du gabarit : traits initiaux, carré répété, accrochage au cadre et validation en familles de traits |

Dépendances typiques : `outils/dessiner/dessin_plan/`, `outils/annoter/outils_texte.rs`, `familles/`, `zones/ruban/`.

Vérifier : `cargo test -p interface --release`.
