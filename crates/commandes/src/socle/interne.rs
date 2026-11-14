//! Imports communs aux fichiers d'exécution des commandes (socle et métiers) : chacun
//! commence par `use crate::socle::interne::*;`.
pub(crate) use super::commande::Commande;
pub(crate) use super::execution::{Execution, ExecutionInterne, identite};
pub(crate) use super::verrous::VerrousInterne;
pub(crate) use crate::modifier::composees::ModificationInterne;
pub(crate) use crate::modifier::transformations::Transformation;
pub(crate) use glam::DVec2;
pub(crate) use modele::creation::croisement;
pub(crate) use modele::*;
pub(crate) use modele::{murs::assemblage, volumes::{geometrie, solides}};
pub(crate) use std::collections::{HashMap, HashSet};
