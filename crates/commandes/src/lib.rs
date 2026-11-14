//! Édition du modèle : commandes sérialisables (`Commande`) et leur exécution, rangées par
//! métier (`modeliser`, `modifier`, `annoter`, `documenter`), sur un socle commun
//! (`socle` : exécution, historique, verrous, contextes). Voir `AGENTS.md`.
mod annoter;
mod documenter;
mod modeliser;
mod modifier;
mod socle;
pub use annoter::cotes::piloter_cote;
pub use modifier::composees::{Modification, decaler_polygone};
pub use socle::commande::Commande;
pub use socle::contextes::Contextes;
pub use socle::execution::Execution;
pub use socle::verrous::Verrous;
pub use socle::{contextes, historique, objets, verrous};
