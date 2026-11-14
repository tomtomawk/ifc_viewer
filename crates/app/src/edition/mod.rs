//! Édition du modèle : commandes, historique, verrous, gestion des phases, sous-projets
//! et variantes.
pub mod commandes;
pub mod contextes;
pub mod historique;
pub mod verrous;
pub use commandes::{Execution, Modification};
pub use contextes::Contextes;
pub use verrous::Verrous;
