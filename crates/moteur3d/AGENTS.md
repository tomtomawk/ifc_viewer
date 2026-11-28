# Crate `moteur3d`

Rôle : vue 3D : rendu wgpu de la scène (`Renderer`), ciel physique, caméra, repère d'orientation des plans, sélection à l'écran par rectangle, shaders WGSL.

Dépendances internes autorisées : `noyau`, `modele`, `scene` (`maillage` en dev-dependency seulement). Interdit : `config`, `echanges`, `collaboration`, `pilote`, `interface`. `egui` et `wgpu` ne sont permis que dans cette crate et au-delà, via `eframe`.

## Où travailler

| Fichier (`src/`) | Contenu |
|---|---|
| `gpu.rs` | `Renderer`, `FrameParams`, passes, uploads, `paint`, `capture` |
| `ciel.rs` | ciel physique (passes de tables, `couleur_soleil`) |
| `camera.rs` | `Camera` |
| `profondeur.rs` | matrices des passes pour wgpu : profondeur inversée de la vue (1 proche, 0 loin), directe des cartes d'ombres |
| `profondeur_faces.rs` | copie de la profondeur des faces opaques (multiéchantillonnée) dans une texture lue par les traits et les reflets du verre |
| `apparences.rs` | images des apparences envoyées au GPU (`couches_images`) |
| `tuiles_motifs.rs` | masques périodiques des segments dessinés, en couches indexées par code de motif |
| `placement_surface.rs` | attributs GPU du placement partagé entre le motif de surface et l’apparence du matériau |
| `fond.rs` | couleur de fond des vues (seule source : projet, familles, captures) |
| `orientation_plan.rs` | repère d'orientation des plans (egui) |
| `selection_rectangle.rs` | sélection par rectangle (egui) |
| `scene.wgsl`, `commun.wgsl`, `ao.wgsl` | shaders, lus par `include_str!` ; `scene.wgsl` et `commun.wgsl` sont concaténés |

Profondeur de la vue inversée : effacer à `LOIN_INVERSE`, comparer avec `COMPARAISON_INVERSEE`. Aucun décalage de profondeur sur les faces. Chaque dessin (vue, fenêtre de vue) a deux passes : faces opaques, copie de leur profondeur (`profondeur_faces`), puis traits et transparents. Les traits ne passent pas par le test du matériel : `trait_cache` (`scene.wgsl`) compare leur profondeur, avancée par `avancer`, à celle des faces lue au centre du trait et prolongée par leur pente. Les annotations sont avancées par `avancer` et testées par le matériel.

Apparences (`scene.wgsl`) : `effet_apparence` reçoit une `Projection` ; les faces verticales courbes passent par `effet_courbe` (projections sur les plans verticaux des axes). Herbe : `fs_herbe` / `brin_herbe`, `COUCHES_HERBE` couches dessinées par instances, trois réseaux de cellules tournés (pas de rangs).

`MAX_APPARENCES` est dans `noyau::constantes` et en dur dans `scene.wgsl` : les deux valeurs doivent rester égales. Les tests `shader_*_valide` valident les shaders (naga) sans GPU. Le rendu lui-même ne se teste pas sans affichage. `gpu.rs` dépasse 600 lignes (`Renderer` à un seul `impl`) : ne pas l'agrandir. `TAILLE_OMBRES` vaut 2048 en wasm32 (4096 sur poste) : la mémoire GPU d'un onglet est comptée.

## Quand modifier ici

Rendu 3D, caméra, ciel, shaders, sélection à l'écran. La disposition de la fenêtre et l'appel de `Renderer` sont dans `interface/src/socle/viewport.rs`.

## Vérifier

```
cargo test -p moteur3d --release
cargo tree -p moteur3d   # aucune crate interne hors noyau, modele, scene (+ maillage en dev)
```
