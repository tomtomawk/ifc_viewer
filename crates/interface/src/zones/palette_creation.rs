//! Palette de création : le même ordre pour tous les objets.
//!
//! Le volet Propriétés d'un outil de création se lit toujours de haut en bas :
//! 1. contexte (phase, sous-projet, variante) : `volet_proprietes.rs`, commun ;
//! 2. sélecteur : type de l'objet (mur, dalle, garde-corps…) ;
//! 3. schéma : croquis ou composition de l'objet ;
//! 4. paramètres : « Paramètres — objet », une ligne par champ.
//!
//! `definition` classe chaque outil. Son `match` est exhaustif et sans cas par défaut : un
//! nouvel `Outil` oblige à choisir sa palette. Les emplacements sont des fonctions
//! `fn(&mut Application, &mut egui::Ui)` : `palette_creation` les appelle dans l'ordre.
use crate::*;
use crate::zones::ruban::{Menu, Outil};

type Emplacement = fn(&mut Application, &mut egui::Ui);

#[derive(Clone, Copy)]
pub(crate) enum Parametres {
    /// Les champs de l'outil (`champs` du ruban) : présentation commune.
    Champs,
    /// Paramètres propres à l'objet (niveau, pente, épaisseur du type…).
    Propres(Emplacement),
}

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub selecteur: Option<Emplacement>,
    pub schema: Option<Emplacement>,
    pub parametres: Parametres,
    /// Vrai : la palette garde le volet quand un élément est sélectionné (le dernier créé
    /// reste sélectionné pendant les créations successives).
    pub avec_selection: bool,
    /// Vrai : le volet continue après la palette (placement d'une ouverture).
    pub suite: bool,
}

pub(crate) enum Definition {
    Palette(Palette),
    /// Réglages seuls (déplacer, cote, texte…) : titre et champs, le reste du volet continue.
    Parametres,
    /// Panneau propre, pas encore passé en palette (`panneau_bim`, vues). À migrer.
    Heritage,
    /// Pas de panneau : action immédiate ou dessin sans réglage.
    Aucune,
}

pub(crate) fn definition(outil: Outil) -> Definition {
    use Outil::*;
    match outil {
        Mur => Definition::Palette(Palette { selecteur: Some(selecteur_mur), schema: Some(schema_mur), parametres: Parametres::Propres(parametres_mur), avec_selection: true, suite: false }),
        Surfaces | Dalle | Sol | Toiture | Plafond | TerrePlein => Definition::Palette(Palette { selecteur: Some(selecteur_surface), schema: Some(schema_surface), parametres: Parametres::Propres(parametres_surface), avec_selection: true, suite: false }),
        Escalier => Definition::Palette(Palette { selecteur: None, schema: Some(schema_escalier), parametres: Parametres::Champs, avec_selection: false, suite: false }),
        GardeCorps => Definition::Palette(Palette { selecteur: Some(selecteur_garde_corps), schema: None, parametres: Parametres::Champs, avec_selection: false, suite: false }),
        Ouverture => Definition::Palette(Palette { selecteur: Some(selecteur_ouverture), schema: None, parametres: Parametres::Champs, avec_selection: true, suite: true }),
        Quadrillage | Deplacer | Copier | Tourner | Ajuster | Cote | CoteAngulaire | Altimetrie | Texte | Etiquette | Symbole | Legende => Definition::Parametres,
        Niveau | Terrain | LimitePropriete | Poteau | Poutre | Composant | Piece | Arbre | Zone | VuePlan | VueCoupe => Definition::Heritage,
        Symetrie | Reseau | Echelle | Decaler | Scinder | Demolir | Reproduire | ChangerHote | Inverser | Aligner | ModifierContour | CopierNiveau | Jonctions | AttacherToiture | CreerIdentique | Supprimer | Cadrer | Isoler | Masquer | MasquerVue | Afficher | Verrouiller | Deverrouiller | CoteDiametre | Arc | Polyligne | Rectangle | Spline | Cercle | Feuilles | Nomenclatures | LienIfc => Definition::Aucune,
    }
}

fn selecteur_mur(a: &mut Application, ui: &mut egui::Ui) {
    if let Some((p, _)) = &a.natif {
        a.ruban_edition.selecteur_type_creation_mur(ui, p);
    }
}
fn schema_mur(a: &mut Application, ui: &mut egui::Ui) {
    if let Some((p, _)) = &a.natif {
        a.ruban_edition.schema_mur(ui, p);
    }
}
fn parametres_mur(a: &mut Application, ui: &mut egui::Ui) {
    if let Some((p, _)) = &a.natif {
        a.ruban_edition.parametres_mur_creation(ui, p);
    }
}
fn selecteur_surface(a: &mut Application, ui: &mut egui::Ui) {
    a.ruban_edition.selecteur_surface(ui, a.natif.as_ref().map(|(p, _)| p));
}
fn schema_surface(a: &mut Application, ui: &mut egui::Ui) {
    a.ruban_edition.schema_surface(ui, a.natif.as_ref().map(|(p, _)| p));
}
fn parametres_surface(a: &mut Application, ui: &mut egui::Ui) {
    a.ruban_edition.parametres_surface(ui, a.natif.as_ref().map(|(p, _)| p));
}
fn schema_escalier(a: &mut Application, ui: &mut egui::Ui) {
    a.schema_escalier(ui);
}
fn selecteur_garde_corps(a: &mut Application, ui: &mut egui::Ui) {
    a.selecteur_garde_corps(ui, &[]);
}
/// Type à placer (et éditeur de familles ou chargement d'une famille), comme dans Revit.
fn selecteur_ouverture(a: &mut Application, ui: &mut egui::Ui) {
    if !a.ruban_edition.proprietes_ouverture(ui, a.natif.as_ref().map(|(p, _)| p)) {
        return;
    }
    if std::mem::take(&mut a.ruban_edition.demande_charger_famille)
        && let Err(e) = a.charger_famille_dialogue(true)
    {
        a.error = Some(e);
    }
    if let Some(t) = a.ruban_edition.demande_famille.take() {
        let genre = if a.ruban_edition.texte_option(Outil::Ouverture, 0) == "Porte" { modele::GenreOuverture::Porte } else { modele::GenreOuverture::Fenetre };
        a.ouvrir_editeur_famille_type(t, genre);
    }
}

impl Application {
    /// Affiche la palette de l'outil actif ; vrai si elle occupe tout le volet.
    pub(crate) fn palette_creation(&mut self, ui: &mut egui::Ui) -> bool {
        if self.natif.is_none() {
            return false;
        }
        let menu = match self.onglet_ruban {
            OngletRuban::Modeliser => Menu::Modeliser,
            OngletRuban::Modifier => Menu::Modifier,
            OngletRuban::Annoter => Menu::Annoter,
            _ => return false,
        };
        let Some(outil) = self.ruban_edition.outil() else { return false };
        match definition(outil) {
            Definition::Palette(p) => {
                // pendant un tracé de Modéliser, l'onglet Modifier reste ouvert
                if !matches!(menu, Menu::Modeliser | Menu::Modifier) || (!p.avec_selection && !self.selection_bim().is_empty()) {
                    return false;
                }
                egui::ScrollArea::vertical().id_salt(("palette_creation", outil)).auto_shrink([false, true]).show(ui, |ui| {
                    if let Some(selecteur) = p.selecteur {
                        selecteur(self, ui);
                    }
                    if let Some(schema) = p.schema {
                        schema(self, ui);
                    }
                    match p.parametres {
                        Parametres::Champs => self.ruban_edition.section_parametres(ui, outil, self.natif.as_ref().map(|(p, _)| p)),
                        Parametres::Propres(parametres) => parametres(self, ui),
                    }
                });
                !p.suite
            }
            Definition::Parametres => {
                if self.ruban_edition.outil_actif_dans(menu).is_some() {
                    self.ruban_edition.section_parametres(ui, outil, self.natif.as_ref().map(|(p, _)| p));
                }
                false
            }
            Definition::Heritage | Definition::Aucune => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un outil qui a des champs doit les montrer : palette ou réglages seuls. Seuls les outils
    /// dont le panneau propre porte déjà ces réglages (vues) ou qui n'ont pas de bouton (zone)
    /// peuvent rester hérités.
    #[test]
    fn tout_outil_avec_champs_montre_ses_parametres() {
        for &outil in Outil::TOUS {
            if !outil.a_des_champs() {
                continue;
            }
            let ok = match definition(outil) {
                Definition::Palette(_) | Definition::Parametres => true,
                Definition::Heritage => matches!(outil, Outil::VuePlan | Outil::VueCoupe | Outil::Zone),
                Definition::Aucune => false,
            };
            assert!(ok, "{} a des champs mais aucune palette ne les affiche", outil.nom());
        }
    }

    /// Les cinq sortes d'éléments horizontaux ont la même palette.
    #[test]
    fn surfaces_une_seule_palette() {
        for o in [Outil::Dalle, Outil::Sol, Outil::Toiture, Outil::Plafond, Outil::TerrePlein] {
            assert!(matches!(definition(o), Definition::Palette(_)), "{}", o.nom());
        }
    }
}
