# Crate `noyau`

Rôle : briques géométriques et utilitaires sans dépendance au modèle : booléens de maillages, texte vectoriel, primitives 2D, arêtes, motifs, empreintes.

Dépendances internes autorisées : aucune (docs/ARCHITECTURE.md §2). Externes : `earcutr`, `epaint_default_fonts`, `glam`, `serde`, `serde_json`, `sha2`, `skrifa`, `uuid`. Interdit : toute crate du workspace, egui, eframe, wgpu.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `csg.rs`, `csg_memoire.rs` | `boolean`, `signed_volume`, `fix_orientation` sur triangles |
| `geometrie.rs`, `trace2d.rs`, `forme2d.rs` | triangulation, tracés 2D, `Forme2D` et points d'arcs/splines |
| `texte.rs` | texte vectoriel (polices embarquées) |
| `aretes.rs`, `motif.rs` | `Aretes`, constantes de trait ; `Motif`, `GrilleMotif`, `hachures_plan` |
| `constantes.rs`, `nombres.rs`, `hash.rs`, `uuid.rs`, `chrono.rs` | `MAX_APPARENCES`, `format_real`, empreintes, uuid, chronomètre |
| `lib.rs` | `Id = u64` (identifiant neutre) |

## Quand modifier ici

Une fonction pure de géométrie 2D/3D ou un utilitaire dont `modele`, `scene`, `ifc_base` ou plusieurs crates ont besoin. Sinon, plus haut.

## Pièges

- `MAX_APPARENCES` (`constantes.rs`) doit rester égale à la constante en dur de `crates/moteur3d/src/scene.wgsl`.
- Pas de ré-export de compatibilité : les appelants écrivent `noyau::<module>::…`.

## Vérifier

```
cargo test -p noyau --release
```
