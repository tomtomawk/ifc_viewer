# Crate `web` (binaire `bimedit-web`, navigateur)

Rôle : BIMedit dans le navigateur (ROADMAP §10.3). `src/main.rs` crée `interface::Application` dans le canevas `#bimedit` de `index.html` par `eframe::WebRunner` (wgpu : WebGPU, ou WebGL2 en repli). Visionneuse : un projet `.bem` déposé sur la page s'ouvre (`interface/src/socle/depot.rs`).

Dépendances internes autorisées : `interface` seule, sans sa fonctionnalité `poste` (`default-features = false`) : ni pilote MCP ni registre des instances.

## Construire et servir

```
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cd crates/web && trunk build --release   # sortie dans crates/web/dist (non suivie)
trunk serve --release                   # http://127.0.0.1:8080
```

Sur poste, `cargo run -p web` ne fait qu'afficher ce rappel : le binaire n'a de sens qu'en wasm32.

## Quand modifier ici

Le démarrage dans la page, `index.html`. L'adaptation des comportements au navigateur va dans les modules d'adaptation (`noyau::fil`, `interface::socle::{dialogue, depot, fenetre}`), pas ici.

## Pièges

- Pas de fil d'exécution, de chemin local ni de dialogue synchrone en wasm32 ; `std::time::Instant` panique (utiliser `web_time`).
- Module de 22,6 Mo après `wasm-opt` (8,1 Mo compressé en gzip), mesuré le 28/11/2026.

## Vérifier

```
cargo check -p web --target wasm32-unknown-unknown
cargo test -p bimedit --test dependances
```
