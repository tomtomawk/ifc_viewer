//! Identifiants numériques des éléments natifs dans la scène et l'arborescence.

/// Identifiant numérique stable d'un élément natif : genre (« mur », « dalle », « vue »…)
/// et identifiant du modèle.
pub fn id_natif(genre: &str, id: &str) -> u64 {
    genre.bytes().chain([0]).chain(id.bytes()).fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Identifiant numérique stable d'une vue native dans l'arborescence (« 3d » : la vue 3D).
pub fn numero_vue(id: &str) -> i64 {
    (id_natif("vue", id) >> 1) as i64
}
