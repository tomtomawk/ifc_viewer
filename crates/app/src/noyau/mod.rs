//! Briques géométriques sans dépendance au modèle : booléens, textes, primitives 2D.
pub mod geometrie;
pub mod csg;
pub mod csg_memoire;
pub mod texte;
pub mod trace2d;
pub mod uuid;
pub mod hash;

/// Identifiant d'entité neutre (numéro d'instance STEP, identifiant d'objet de scène).
pub type Id = u64;
