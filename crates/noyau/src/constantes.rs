//! Constantes partagées entre le calcul des scènes et le rendu.

/// Apparences de matériaux transmises au rendu (rang codé dans les couleurs de rendu).
/// Même valeur que `MAX_APPARENCES` dans `crates/moteur3d/src/scene.wgsl` (vérifié par un test de `moteur3d`).
pub const MAX_APPARENCES: usize = 32;

/// Familles de traits des motifs de surface d'une scène au plus (rendu 3D : tableau du
/// shader, trois `vec4` par famille).
pub const FAMILLES_MOTIFS_MAX: usize = 96;
