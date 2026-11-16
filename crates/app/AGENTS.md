# Crate `app` (binaire `bimedit`)

Rôle : point d'entrée. `src/main.rs` (82 lignes) lit les arguments et aiguille : `--mcp` (`pilote::mcp::servir`), commandes de `collaboration::cli` et `pilote::cli`, modes console de `src/modes/`, sinon lancement de `interface::socle::fenetre::lancer`. `build.rs` et `assets/icone.ico` : icône Windows.

Dépendances internes autorisées : `noyau`, `modele`, `ifc_base`, `formats`, `echanges`, `collaboration`, `pilote`, `config`, `interface` (exactement celles-ci). Aucun code d'interface ici.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `main.rs` | `--mcp`, aiguillage des options, repli sur la fenêtre |
| `modes/modele.rs` | `--modele <fichier.rvt\|.rfa\|.ifc> [sortie.json]` : conversion, projet JSON et rapport |
| `modes/stats.rs`, `modes/rvt_stats.rs` | `--stats <fichier.ifc\|.rvt\|.rfa\|.rte>` (IFC, Revit) |
| `modes/rvt_parametres.rs`, `rvt_images.rs`, `rvt_decodage.rs`, `rvt_nomenclatures.rs` | `--parametres`, `--images <fichier> <dossier>`, `--decodage`, `--nomenclatures` |
| `tests/dependances.rs` | garde-fou : sens des dépendances entre crates, egui/wgpu hors des crates autorisées |

## Quand modifier ici

Seulement pour une option de ligne de commande ou un mode console (un fichier de `src/modes/`). Tout le reste va dans la crate concernée.

## Pièges

- `--mcp` doit rester le premier test de `main` : rien d'autre ne doit écrire sur la sortie standard avant le pont MCP.
- Les messages et les noms d'options des modes sont documentés dans `README.md`, `docs/MODELE.md` et utilisés par `.github/workflows/ci.yml` : ne pas les changer sans mettre ces fichiers à jour.
- Le garde-fou `tests/dependances.rs` contient le tableau du §2 de `docs/ARCHITECTURE.md` : modifier les deux ensemble.

## Vérifier

```
cargo build --workspace
cargo test -p bimedit --release     # garde-fou des dépendances
cargo run --release -p bimedit -- --stats samples/test_ifc4.ifc
```
