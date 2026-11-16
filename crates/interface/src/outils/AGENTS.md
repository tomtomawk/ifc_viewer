# `interface/src/outils/`

Rôle : outils de la vue, rangés par métier comme `crates/commandes/src/`. L'exécution des commandes est dans la crate `commandes` ; ici, la saisie à la souris et les barres d'options.

| Dossier | Fichiers |
|---|---|
| `modeliser/` | `bim.rs` (transactions du document BIM `executer_bim`, historique, import, verrous, placement d'éléments, panneau des outils de création `panneau_bim`), `murs_courbes.rs`, `garde_corps.rs` (sélecteur et éditeur de types), `escalier.rs` (création d'escaliers et garde-corps) |
| `modifier/` | `transformer.rs` (Déplacer, Copier, Tourner, Symétrie, Réseau, Échelle, Aligner, Décaler, Scinder), `ajuster.rs`, `options_modifier.rs` (barre d'options de Modifier, jonctions, annotations natives, export IFC/PDF, fenêtre documentation) |
| `dessiner/` | `dessin_plan/` : DAO 2D commune (tracés, accrochages, contours, courbures, options de tracé) |
| `annoter/` | `cotation.rs`, `cotes_aretes.rs`, `outils_texte.rs`, `types_traits.rs` |

Dépendances typiques : `saisie/`, `zones/ruban/` (`Outil`), `socle/`. `annoter/` et `modeliser/escalier.rs` utilisent des éléments `pub(crate)` de `dessiner/dessin_plan/mod.rs` (`EtatDessin2D`, `vers_ecran`, `champ_valeur`, `HautEscalier`).

Vérifier : `cargo test -p interface --release`.
