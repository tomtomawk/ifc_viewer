# Architecture du dépôt

Le dépôt est un workspace Cargo (`Cargo.toml` racine, `resolver = "3"`, `members = ["crates/*"]`) de 15 crates. Les dépendances externes sont déclarées une fois dans `[workspace.dependencies]` et reprises par `dep.workspace = true`. `Cargo.lock` et `target/` restent à la racine. À la racine aussi : `docs/`, `samples/`, `gabarits/`, `tools/`, `.github/`, `.mcp.json`, `README.md`, `AGENTS.md`.

Pour travailler sur une partie : lire l'`AGENTS.md` de la crate (≤ 40 lignes) et le §2 ci-dessous ; ne pas charger le reste. L'historique du découpage (lots 0 à 11b) est dans [`historique/ARCHITECTURE_CHANTIER_2026-11.md`](historique/ARCHITECTURE_CHANTIER_2026-11.md) : journal, pas à lire pour travailler.

## 1. Les 15 crates

Tailles : fichiers `.rs` et lignes de `src/` (état du lot 12). Ordre = ordre des dépendances.

| Crate | Rôle | Fichiers | Lignes |
|---|---|---|---|
| `noyau` | géométrie, booléens (CSG), texte, primitives 2D, arêtes, motifs, uuid, empreintes | 14 | 1 880 |
| `modele` | modèle natif : données, JSON, validation, murs, éléments, volumes, familles, documentation, gabarit | 40 | 14 541 |
| `ifc_base` | parseur STEP (`step`) et couche sémantique IFC (`semantique`) | 3 | 1 136 |
| `scene` | scène triangulée, navigateur de projet, picking | 1 | 384 |
| `formats` | lecture IFC (géométrie, assemblage de la scène) et Revit (maquette, vues, nomenclatures) | 23 | 12 537 |
| `maillage` | modèle natif -> scène (`depuis_natif`) | 5 | 899 |
| `commandes` | `Commande` sérialisable, exécution, historique, verrous, contextes ; par métier | 17 | 3 288 |
| `moteur2d` | coupe, poché, arêtes vues/cachées, DAO, rendu SVG/PDF des vues et feuilles | 8 | 2 713 |
| `moteur3d` | rendu wgpu, ciel, caméra, sélection à l'écran, shaders WGSL | 6 | 2 897 |
| `echanges` | import IFC/Revit -> `Projet`, export IFC, enregistrement atomique, rapport | 6 | 1 653 |
| `collaboration` | transactions partagées : dépôt, session, concurrence, `.bimpartage`, texte (`yrs`), CLI | 8 | 4 419 |
| `pilote` | serveur MCP, CLI non graphique, registre des instances, canal TCP, entrées egui | 6 | 1 410 |
| `config` | options utilisateur et raccourcis | 3 | 343 |
| `interface` | toute l'interface egui (`Application`) et la fenêtre native | 81 | 34 340 |
| `app` | binaire `bimedit` : arguments, modes console (`src/modes/`), icône Windows | 9 | 601 |

Package et binaire : `bimedit` (`crates/app`, `cargo run -p bimedit`).

## 2. Dépendances internes autorisées

Une flèche = « dépend de » ; une crate n'importe que des crates situées avant elle.

```
noyau <- modele <- ifc_base <- scene <- formats <- maillage <- commandes <- moteur2d <- moteur3d
      <- echanges <- collaboration <- pilote <- config <- interface <- app
```

Ce tableau est celui que vérifie `crates/app/tests/dependances.rs` (constantes `AUTORISEES` et `DEV_SEULEMENT`) : modifier les deux ensemble. « Réel » = dépendances déclarées dans les `Cargo.toml` ; « dev » = `[dev-dependencies]` (tests d'intégration), autorisées séparément.

| Crate | Autorisées (production et dev) | Réel | Dev seulement |
|---|---|---|---|
| noyau | aucune | aucune | |
| modele | noyau | noyau | |
| ifc_base | noyau | noyau | |
| scene | noyau, modele, ifc_base | les trois | |
| formats | noyau, ifc_base, scene | les trois | |
| maillage | noyau, modele, ifc_base, scene, formats | les cinq | |
| commandes | noyau, modele | noyau, modele | maillage |
| moteur2d | noyau, modele, scene, maillage | les quatre | commandes |
| moteur3d | noyau, modele, scene | les trois | maillage |
| echanges | noyau, modele, ifc_base, formats | les quatre | commandes, maillage, moteur2d |
| collaboration | noyau, modele, maillage, commandes, echanges | les cinq |  |
| pilote | noyau, modele, maillage, commandes, moteur2d, echanges, collaboration | les sept |  |
| config | noyau, modele, echanges | les trois |  |
| interface | toutes jusqu'à `config` | les 13 |  |
| app | noyau, modele, ifc_base, formats, echanges, collaboration, pilote, config, interface | les neuf |  |

Le tableau est resserré aux dépendances réellement utilisées (aucune autorisation inutilisée). Ajouter une dépendance interne se décide : modifier ce tableau et le test ensemble, en respectant l'ordre.

Externes : `eframe` (egui, wgpu) et `wgpu` seulement dans `moteur3d`, `pilote`, `config`, `interface`, `app` ; `egui`/`wgpu` jamais dans `noyau`, `ifc_base`, `modele`, `scene`, `formats`, `maillage`, `commandes`, `moteur2d`, `echanges`, `collaboration` (vérifié par le test). `eframe` est la seule déclaration d'egui : `moteur3d` (rendu), `pilote` (`egui::Event`, `egui::Context`), `config` (`DragValue`, `ThemePreference`, `Key`), `interface` (tout).

## 3. Règles

1. Une crate n'importe que des crates situées avant elle ; aucun cycle ; le sens est vérifié par `cargo test -p bimedit --release --test dependances`.
2. Aucun ré-export de compatibilité : un symbole déplacé est appelé par son nouveau chemin partout (`noyau::…`, `modele::unite::…`, `ifc_base::step::…`).
3. Plafond d'environ 600 lignes par fichier pour tout nouveau code ; un fichier au-delà ne s'agrandit pas (§5).
4. Un `AGENTS.md` court (≤ ~40 lignes : rôle, dépendances autorisées, points d'entrée, quand modifier ici, `cargo test -p <crate>`, pièges) dans chaque crate ; le mettre à jour avec le code. L'`AGENTS.md` racine porte la carte du dépôt.
5. `pub(crate)` ne devient `pub` que si la compilation l'exige.
6. Ce qui reste à faire s'écrit dans `docs/ROADMAP.md`, ce qui est fait ou abandonné dans `docs/SUIVI.md`, dans le même commit que le code, et nulle part ailleurs ; les autres documents décrivent le fonctionnement.
7. Aucun changement de format sans mise à jour de la documentation (`docs/MODELE.md`, `docs/EDITION.md`) ; pas de migration des formats natifs : `docs/AGENTS.md`.
8. Commits : un message préfixé du contexte (« Lot 12 : … », ou le sujet) qui décrit le changement ; les lignes `Co-Authored-By` et `Claude-Session` en fin de message pour le travail fait avec un agent. `cargo check --workspace --tests` avant chaque commit.
9. Les tests de `samples/` se lisent par `concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/…")` ; les `include_str!` de `docs/` et `gabarits/` sont relatifs à `crates/<crate>/src/`.

## 4. Où est quoi

| Je veux… | Aller à |
|---|---|
| ajouter ou changer une commande d'édition | `crates/commandes/src/<métier>/` (+ `socle/commande.rs`), `docs/EDITION.md` ; l'outil à la souris dans `crates/interface/src/outils/<métier>/` |
| changer le format JSON du modèle | `crates/modele/src/lib.rs` (et module concerné), `docs/MODELE.md` |
| créer un mur, une dalle, une vue (paramètres) | `crates/modele/src/creation.rs` |
| géométrie d'un élément (volume) | `crates/modele/src/volumes/`, `elements/`, `murs/` |
| rendu 3D, shaders, ciel, caméra | `crates/moteur3d/src/` |
| ce qu'on voit d'un élément natif dans la 3D | `crates/maillage/src/` |
| coupe, plan, poché, hachures, arêtes, SVG/PDF | `crates/moteur2d/src/` |
| import IFC | `crates/echanges/src/depuis_ifc.rs`, `crates/formats/src/ifc/`, `crates/ifc_base/` |
| import Revit | `crates/echanges/src/depuis_rvt.rs`, `crates/formats/src/rvt/` |
| export IFC, enregistrement du projet | `crates/echanges/src/` |
| ruban, barre haute, panneaux | `crates/interface/src/zones/` (`ruban/`) |
| outil de dessin 2D, cotation, textes | `crates/interface/src/outils/dessiner/`, `outils/annoter/` |
| éditeur de familles | `crates/interface/src/familles/` ; données : `crates/modele/src/famille/` |
| feuilles, cartouche, coupes | `crates/interface/src/documenter/` ; rendu : `moteur2d/src/feuilles.rs` |
| MCP, commandes `--editer`, `--export-*`, `--quantites` | `crates/pilote/src/`, `docs/PILOTE.md` |
| collaboration (partage, verrous, concurrence) | `crates/collaboration/src/`, `docs/COLLABORATION.md` |
| options utilisateur, raccourcis | `crates/config/src/options/` |
| option de ligne de commande, mode console | `crates/app/src/main.rs`, `crates/app/src/modes/` |
| dépendances entre crates | §2 et `crates/app/tests/dependances.rs` |
| CI et publication | `.github/workflows/ci.yml`, `release.yml` |

## 5. Dette technique connue

- **43 fichiers `.rs` au-delà de 600 lignes** (`find crates -name '*.rs' | xargs wc -l | awk '$1>600'`) : `interface` 22 (dont `familles/editeur_famille/canevas.rs` 1 219, `outils/dessiner/dessin_plan/mod.rs` 1 174, `lib.rs` 1 143, `saisie/saisie_plan.rs` 1 121, `documenter/feuilles.rs` 1 060), `formats` 9, `modele` 6 (`lib.rs` 1 603, `famille/modelee.rs` 1 414, `documentation/mod.rs` 1 200), `collaboration` 3 (`tests.rs` 1 384, `stockage.rs` 626, `session.rs` 604), `moteur3d` 1 (`gpu.rs`), `moteur2d` 1 (`dao.rs` 795), `pilote` 1 (`mcp.rs` 607). Seul `bim.rs` a été découpé (voir ci-dessous ; reste `bim/panneau.rs` 646, une seule fonction `panneau_bim`) ; le découpage de ceux d'`interface` n'est pas planifié.
- **`Renderer` / `moteur3d/src/gpu.rs`** (1 416 lignes) : un seul `impl Renderer` ; non découpé.
- **`interface/src/outils/modeliser/bim/`** : découpé en `mod.rs` (`EtatBim`, placements, Créer à l'identique), `panneau.rs` (`panneau_bim`, 646 lignes, à redécouper par outil si on y touche), `widgets.rs` ; les transactions du document (`executer_bim`, historique, enregistrer sous, import, verrous, `elements_natifs`) sont dans `socle/document_bim.rs`.
- **`interface/src/saisie/saisie_plan.rs`** : contient aussi la création des murs, dalles et vues natives ; classement mixte.
- **`collaboration/src/stockage.rs` et `session.rs`** : au plafond, ne pas agrandir.
- **Dev-dependencies** (cycles de tests évités par des tests d'intégration en aval) : `echanges` -> `commandes`, `maillage`, `moteur2d` ; `commandes` -> `maillage` ; `moteur2d` -> `commandes` ; `moteur3d` -> `maillage`.
- **`eframe` hors de l'interface** : dans `moteur3d` (rendu wgpu), `pilote` (`egui::Event`), `config` (`DragValue`, `ThemePreference`, `Key` : adaptateurs d'interface dans une crate de configuration, à valider).
- **`interface::projet::collaboration`** masque la crate `collaboration` dans `projet/` : écrire `::collaboration::…` pour la crate.
- **Modes console dans `app`** : `--modele`, `--stats`, `--images`, `--decodage`, `--nomenclatures`, `--parametres` appellent `formats`, `ifc_base`, `echanges` ; ils sont rangés dans `crates/app/src/modes/` (un fichier par mode, `rvt_stats.rs` 272 lignes) mais restent dans le binaire.
- **`moteur3d/src/scene.wgsl`** : `const MAX_APPARENCES: i32 = 32;` en dur ; l'égalité avec `noyau::constantes::MAX_APPARENCES` est vérifiée par le test `max_apparences_du_shader_egal_a_noyau` (`moteur3d`).
- **Garde-fou** : le §2 est codé en dur dans le test, sans lecture du document ; les deux se modifient ensemble. `[dependencies.x]` (tableau TOML) n'est pas géré (le test échoue s'il en trouve).

Cette section décrit la dette ; son traitement (priorité, fait, reste à faire) est suivi dans [ROADMAP.md](ROADMAP.md) §6.

## 6. Comment vérifier

```
cargo build --workspace
cargo test --workspace --release          # référence : voir ci-dessous
cargo test -p <crate> --release           # une crate seule
cargo test -p bimedit --release --test dependances   # sens des dépendances, egui/wgpu
cargo run --release -p bimedit -- --stats samples/test_ifc4.ifc
```

État courant des tests : [SUIVI.md](SUIVI.md) §1. Référence de tests (lot 12) : 395 réussis et 2 ignorés (somme des crates ; +1 depuis le lot 12 : test `MAX_APPARENCES` de `moteur3d`). Répartition : `modele` 101 (+1 ignoré), `formats` 52, `interface` 49, `collaboration` 43, `moteur2d` 40 (+1 ignoré), `commandes` 29, `echanges` 28, `pilote` 12, `moteur3d` 13, `noyau` 10, `maillage` 8, `config` 4, `bimedit` 4, `ifc_base` 2, `scene` 0. La somme brute des lignes « test result » est supérieure de 4 : `collaboration` relance son binaire de test en sous-processus (lignes « 1 passed »). Sans affichage, le binaire échoue au lancement de la fenêtre (`winit … DISPLAY is set`, code 1) ; les modes console et `--mcp` fonctionnent sans affichage.

## 7. Documents voisins

`docs/MODELE.md` (format du modèle), `docs/EDITION.md` (commandes), `docs/MODIFICATION.md`, `docs/PILOTE.md` (MCP), `docs/COLLABORATION.md`, `docs/AGENTS.md` (politique des formats : à respecter). `docs/ROADMAP.md` : ce qui reste à faire, objectif des familles (§5) et études des évolutions (§10) ; `docs/SUIVI.md` : ce qui est fait ou abandonné. `docs/historique/` : archives (chantier du découpage, audit de factorisation du 11/11/2026, avancement des familles).
