# `interface/src/socle/`

Rôle : état et services de l'application autour de `ViewerApp` (défini avec la boucle `update` dans `../lib.rs`).

| Fichier | Contenu |
|---|---|
| `documents.rs` | ouverture, chargement, enregistrement, onglets de documents (`open`, `install_scene`) |
| `vues.rs` | mémorisation, onglets et activation des vues (`onglets_vues`, `activer_vue`) |
| `viewport.rs` | zone de vue : souris, rendu, superpositions (`viewport`) |
| `soleil.rs` | lieu, date et heure du soleil, caméra de vue, mode piéton |
| `fenetre.rs` | seul module public : `lancer` (appelé par `app`), `arriere_plan`, `cadre_natif` ; `include_bytes!("../../assets/icone.png")` |
| `parametres.rs` | paramètres personnels, raccourcis clavier |
| `aides.rs` | aides communes (icônes, libellés, réticule) ; importé en glob par `lib.rs` |
| `commandes_pilote.rs`, `pilotage.rs` | exécution des requêtes du pilote MCP (`piloter`) |
| `volets.rs` | palettes flottantes natives |

Dépendances typiques : `zones/` (ruban, panneaux), `outils/modeliser/bim.rs` (`executer_bim`), `documenter/feuilles.rs`.

Vérifier : `cargo test -p interface --release`.
