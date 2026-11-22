//! Projet partagé : parcours Collaborer (module local `collaboration`, qui masque la crate du même nom : écrire `::collaboration::…` pour la crate) et gestion des phases, sous-projets et variantes.
pub(crate) mod collaboration;
pub(crate) mod collaboration_pilote;
mod conflits;
pub(crate) mod gestion_contextes;
