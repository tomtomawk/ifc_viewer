# Crate `scene`

Rôle : scène triangulée commune aux rendus : `Scene`, `SceneObject`, `Vertex`, `Range`, navigateur de projet (`Navigateur`, `NavVue`, `NavFamille`…), picking (`Scene::pick`), phases (`etat_de_phase`).

Dépendances internes autorisées : `noyau`, `modele`, `ifc_base`. Externe : `glam`. Interdit : `formats`, `maillage` et toute crate après, egui, wgpu.

## Où travailler

Un seul fichier : `src/lib.rs` (384 lignes) : structures de la scène, `Navigateur`, `ReperePartage`, `Scene::pick`, `etat_de_phase`, `decode_step_string`. Pas de test dans cette crate.

## Quand modifier ici

Un champ de scène ou de navigateur lu par plusieurs crates. Les constructeurs de scène sont ailleurs : `formats::ifc::assemblage` (IFC), `formats::rvt::maquette::vers_scene` (Revit), `maillage::depuis_natif` (modèle natif).

## Pièges

- `Scene` contient `Vec<modele::Apparence>`, un `ifc_base::IfcModel` et un `IdMap` : ajouter un champ oblige à mettre à jour les trois constructeurs.
- `moteur3d` et `moteur2d` lisent `Vertex` tel quel (`repr(C)`, normales en `i8`) : ne pas changer la disposition sans toucher aux shaders.

## Vérifier

```
cargo check -p scene
cargo test --workspace --release   # les constructeurs de scène sont testés dans formats, maillage, moteur2d
```
