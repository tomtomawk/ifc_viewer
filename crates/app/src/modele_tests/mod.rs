//! Tests des modules de la crate `modele` qui appellent du code situé au-dessus d'elle
//! (commandes, maillage, moteur2d). Un fichier par fichier d'origine ; ceux qui appelaient
//! `echanges` ou `collaboration` ont rejoint ces crates (lot 9). Restent 17 tests (§5.16 d).
mod documentation;
mod elements_arbre;
mod elements_composant;
mod elements_piece;
mod elements_site;
mod gabarits_vues;
mod murs_assemblage;
mod volumes_solides;
