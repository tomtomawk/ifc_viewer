# Instructions du projet

Lire et appliquer la politique des formats et des migrations dans [docs/AGENTS.md](docs/AGENTS.md).

**Travailler sur une partie seulement** : lire d'abord l'`AGENTS.md` de la crate concernée et `docs/ARCHITECTURE.md` §2 (dépendances autorisées) ; ne pas charger le reste. `docs/historique/` est un journal, pas à lire. Avant de commiter : `cargo check --workspace --tests`.

## Carte du dépôt (`crates/`, dans l'ordre des dépendances)

| Crate | Rôle |
|---|---|
| `noyau` | géométrie, booléens, texte, primitives 2D, uuid, empreintes |
| `modele` | modèle natif : données, JSON, validation, murs, éléments, volumes, familles |
| `ifc_base` | parseur STEP et couche sémantique IFC |
| `scene` | scène triangulée, navigateur de projet, picking |
| `formats` | lecture IFC (géométrie) et Revit |
| `maillage` | modèle natif -> scène |
| `commandes` | commandes d'édition, historique, verrous, contextes |
| `moteur2d` | coupe, plan, poché, DAO, rendu SVG/PDF |
| `moteur3d` | rendu wgpu, ciel, caméra, shaders |
| `echanges` | import IFC/Revit, export IFC, enregistrement |
| `collaboration` | transactions partagées entre postes |
| `pilote` | MCP, CLI non graphique, registre des instances |
| `config` | options utilisateur, raccourcis |
| `interface` | interface egui (`ViewerApp`) et fenêtre |
| `app` | binaire `bimedit`, modes console, test des dépendances |

## Je veux modifier… -> où aller

| Sujet | Emplacement |
|---|---|
| une commande Modifier / Modéliser / Annoter / Documenter | `crates/commandes/src/<métier>/` + outil dans `crates/interface/src/outils/<métier>/` (documenter : `interface/src/documenter/`) |
| format JSON du modèle | `crates/modele/` + `docs/MODELE.md` |
| rendu 3D, shaders, ciel | `crates/moteur3d/` |
| apparence d'un élément natif dans la 3D | `crates/maillage/` |
| coupe, plan 2D, SVG/PDF | `crates/moteur2d/` |
| import IFC | `crates/echanges/src/depuis_ifc.rs`, `crates/formats/src/ifc/`, `crates/ifc_base/` |
| import Revit | `crates/echanges/src/depuis_rvt.rs`, `crates/formats/src/rvt/` |
| ruban | `crates/interface/src/zones/ruban/` |
| éditeur de familles | `crates/interface/src/familles/` + `crates/modele/src/famille/` |
| MCP, `--editer`, `--export-*`, `--quantites` | `crates/pilote/` + `docs/PILOTE.md` |
| collaboration | `crates/collaboration/` + `docs/COLLABORATION.md` ; écran : `crates/interface/src/projet/` |
| options, raccourcis | `crates/config/` |
| option de ligne de commande | `crates/app/src/main.rs`, `crates/app/src/modes/` |
| dépendances entre crates | `docs/ARCHITECTURE.md` §2 + `crates/app/tests/dependances.rs` |

Règles : pas de ré-export de compatibilité ; fichiers ≤ ~600 lignes pour tout nouveau code ; un `AGENTS.md` court par crate, à tenir à jour avec le code.
