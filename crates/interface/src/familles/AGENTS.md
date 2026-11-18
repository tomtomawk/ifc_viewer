# `interface/src/familles/`

Rôle : édition des familles.

| Fichier | Contenu |
|---|---|
| `editeur_famille/` | éditeur des familles modélisées (ancrage des éléments 2D, canevas, catalogue d'outils, documents, formes, panneau ; onglet Modifier : `transformer`, `reseau`, `aligner` ; `construction` : renommer, supprimer, détacher le plan de construction actif) |
| `familles_planes.rs` | onglets des familles 2D |
| `document_famille.rs` | état de document commun aux familles planes et modélisées |
| `profils.rs` | familles de profils plans |
| `selection_famille.rs` | sélection des familles du projet et accès aux éditeurs |

Dépendances typiques : `saisie/` (accrochage, saisie_trace), `documenter/` (cartouche, feuilles), `outils/annoter/outils_texte.rs`, `zones/communes.rs`.

Vérifier : `cargo test -p interface --release`.
