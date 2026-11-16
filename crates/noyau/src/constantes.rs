//! Constantes partagées entre le calcul des scènes et le rendu.

/// Apparences de matériaux transmises au rendu (rang codé dans les couleurs de rendu).
/// Même valeur que `MAX_APPARENCES` dans `crates/moteur3d/src/scene.wgsl` (vérifié par un test de `moteur3d`).
pub const MAX_APPARENCES: usize = 32;
