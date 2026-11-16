//! Socle de l'application : documents ouverts, vues et caméra (soleil, piéton), viewport, fenêtre native (seul module public : `fenetre`), paramètres, aides communes, pilotage (MCP), volets flottants. `ViewerApp` et la boucle restent dans `lib.rs`.
pub(crate) mod documents;
pub(crate) mod vues;
pub(crate) mod viewport;
pub mod fenetre;
pub(crate) mod parametres;
pub(crate) mod aides;
pub(crate) mod commandes_pilote;
pub(crate) mod pilotage;
pub(crate) mod volets;
pub(crate) mod soleil;
