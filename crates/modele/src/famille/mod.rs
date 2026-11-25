//! Familles : familles modélisées de l'éditeur (`modelee`), édition des types qui les
//! portent (`edition`), familles de profils (`profil`). Toute menuiserie (porte,
//! fenêtre, ouverture libre) est une famille modélisée.
pub mod edition;
pub mod modelee;
pub mod profil;

/// Rôle d'une pièce de menuiserie : couleur et transparence au rendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Dormant,
    Ouvrant,
    Panneau,
    Vitrage,
    Appui,
}
