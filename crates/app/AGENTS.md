# Crate `app` (binaire `bimedit`)

Rôle : point d'entrée. `main.rs` lit les arguments : `--mcp` (`pilote::mcp::servir`), commandes de `collaboration::cli` et `pilote::cli`, modes console (`--modele`, `--stats`, `--images`, `--decodage`, `--nomenclatures`, `--parametres`), sinon lancement de `interface::socle::fenetre::lancer`. `build.rs` et `assets/icone.ico` : icône Windows.

Dépendances internes : `noyau`, `modele`, `ifc_base`, `formats`, `echanges`, `collaboration`, `pilote`, `config`, `interface`. Pas de code d'interface ici ; pas de test.

Les modes console de `main.rs` (~500 lignes) restent à alléger au lot 12 (docs/ARCHITECTURE.md §5.18 e).

## Vérifier

```
cargo build --workspace
cargo run --release -p bimedit -- --stats samples/test_ifc4.ifc
```
