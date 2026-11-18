# `interface/src/familles/`

Rôle : édition des familles.

| Fichier | Contenu |
|---|---|
| `editeur_famille/` | éditeur commun des familles, toutes catégories (ancrage des éléments 2D, canevas (superpositions de l'éditeur), `rendu` : famille en scène pour le `Renderer` du projet (toutes les vues), `navigation` : caméra des vues planes, gestes, barre de vue (échelle, niveau de détail, aperçu, Cadrer) ; `apercu` : visibilité des éléments (tous montrés, ceux qui seraient cachés signalés dans l'arborescence, retirés en aperçu « comme dans le projet »), réglages de visibilité d'un élément, commandes `famille_vue` et `famille_visibilite` ; catalogue d’outils, documents, formes, panneau ; onglet Modifier : `transformer`, `reseau`, `aligner` ; `construction` : renommer, supprimer, détacher le plan de construction actif) |
| `document_famille.rs` | état de document (historique, fichier, projet d'origine) d'une famille ouverte |
| `selection_famille.rs` | sélection des familles du projet, accès à l'éditeur, ouverture et enregistrement des profils |

Toutes les catégories (porte, fenêtre, composant, profil, cartouche, étiquette de pièce) passent par `editeur_famille/` ; cartouches et étiquettes s'ouvrent depuis `documenter/feuilles.rs` (`editer_cartouche`, `editer_famille_etiquette`).

Accrochage : pas de moteur propre ; `canevas.rs` construit la source de géométrie de la famille (plans de référence en droites, contours, éléments 2D ; repère du plan de construction) pour `saisie/moteur_accrochage.rs`.

Dépendances typiques : `saisie/` (moteur_accrochage, saisie_trace), `documenter/` (feuilles), `socle/projection.rs` (monde ↔ écran, molette), `outils/annoter/outils_texte.rs`, `zones/communes.rs`.

Vérifier : `cargo test -p interface --release`.
