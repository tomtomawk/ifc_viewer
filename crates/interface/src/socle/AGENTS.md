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
| `messages.rs` | messages centralisés sous le bandeau de dessin : `informer`, `signaler` (information, avertissement, erreur), `afficher_messages`, `consigne_vue`, `origine_consigne` ; journal (`Journal`, `suivre_messages` une fois par image, `bouton_journal`, commande `journal` du pilote). `error` et `status` restent écrits directement partout : le journal les relève, ne pas les remplacer par des appels |
| `aides.rs` | aides communes (icônes, libellés, réticule) ; importé en glob par `lib.rs` |
| `commandes_pilote.rs`, `pilotage.rs` | exécution des requêtes du pilote MCP (`piloter`) |
| `volets.rs` | palettes flottantes natives |
| `taches.rs` | enregistrement natif et exports en arrière-plan (`Tache`, `Controle` : étape et annulation), suivi par image (`suivre_tache`), barre de progression (`barre_progression`) ; lien IFC et import IFC dans le projet ouvert (`importer_dans` : conversion puis `Fusionner`). Les commandes du pilote restent synchrones |
| `document_bim.rs` | transactions du document : `executer_bim(_suite)`, historique, enregistrer sous, import natif, verrous et cadenas, `elements_natifs` |

Dépendances typiques : `zones/` (ruban, panneaux), `outils/modeliser/bim/` (`EtatBim`), `documenter/feuilles.rs`.

Vérifier : `cargo test -p interface --release`.
