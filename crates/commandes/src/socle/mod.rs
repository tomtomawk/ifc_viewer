//! Socle commun à tous les métiers : le type `Commande`, son exécution, l'historique,
//! les verrous, les contextes (phases, sous-projets, variantes) et la table d'objets.
pub mod commande;
pub mod contextes;
pub mod execution;
pub mod historique;
pub(crate) mod interne;
pub mod objets;
pub mod verrous;
