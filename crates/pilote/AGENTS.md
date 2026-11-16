# Crate `pilote`

Rôle : pilotage du viewer par un autre programme : serveur MCP stdio (`bimedit --mcp`), canal TCP local avec jeton, registre des instances, commandes en ligne non graphiques, entrées egui validées.

Dépendances internes autorisées : `noyau`, `modele`, `maillage`, `commandes`, `moteur2d`, `echanges`, `collaboration` (exactement celles-ci). Interdit : `config`, `interface`. `eframe` n'est là que pour `egui::Event` (`saisie.rs`) et `egui::Context` (`lib.rs`) : ne pas y ajouter d'autre usage d'egui ni de wgpu.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `lib.rs` | écoute TCP (`demarrer`, `Requete`), helpers JSON, `documentation` |
| `mcp.rs` | serveur MCP (607 lignes : ne pas agrandir) |
| `cli.rs` | `--editer`, `--export-*`, `--quantites`, `--mesurer-scene` |
| `edition.rs`, `instances.rs`, `saisie.rs` | transactions du pilote, registre des instances, entrées egui |

Les commandes reçues sont exécutées par l'interface (`crates/interface/src/socle/commandes_pilote.rs`, `socle/pilotage.rs`), pas ici. Les `include_str!` visent `docs/PILOTE.md`, `docs/EDITION.md`, `docs/MODELE.md` et `crates/commandes/src/socle/commande.rs` (chemins relatifs à `src/`).

## Quand modifier ici

Outil MCP, commande en ligne non graphique (`--editer`, `--export-*`, `--quantites`), registre des instances, canal TCP. Documentation à tenir à jour : `docs/PILOTE.md`.

## Vérifier

```
cargo test -p pilote --release
cargo tree -p pilote   # aucune crate interne hors noyau, modele, maillage, commandes, moteur2d, echanges, collaboration
```
