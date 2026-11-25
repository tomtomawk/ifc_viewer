//! Palette de création : le même ordre pour tous les objets.
//!
//! Le volet Propriétés d'un outil de création se lit toujours de haut en bas :
//! 1. contexte (phase, sous-projet, variante) : `volet_proprietes.rs`, commun ;
//! 2. sélecteur : type de l'objet (mur, dalle, garde-corps…) ;
//! 3. schéma : croquis ou composition de l'objet ;
//! 4. paramètres : « Paramètres — objet », une ligne par champ.
//!
//! Tout objet a un sélecteur (`Palette::selecteur` n'est pas optionnel) : son type quand il en
//! a un, sinon l'en-tête « objet · Générique » (`entete_fixe`).
//!
//! `definition` classe chaque outil. Son `match` est exhaustif et sans cas par défaut : un
//! nouvel `Outil` oblige à choisir sa palette. Les emplacements sont des fonctions
//! `fn(&mut Application, &mut egui::Ui)` : `palette_creation` les appelle dans l'ordre.
use crate::*;
use crate::zones::ruban::{selecteur, Menu, Outil};

type Emplacement = fn(&mut Application, &mut egui::Ui);

#[derive(Clone, Copy)]
pub(crate) enum Parametres {
    /// Les champs de l'outil (`champs` du ruban) : présentation commune.
    Champs,
    /// Paramètres propres à l'objet (niveau, pente, épaisseur du type…).
    Propres(Emplacement),
    /// L'objet n'a aucun paramètre.
    Aucun,
}

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub selecteur: Emplacement,
    pub schema: Option<Emplacement>,
    pub parametres: Parametres,
    /// Onglets du ruban où la palette s'affiche (Modifier : pendant un tracé de Modéliser).
    pub menus: &'static [Menu],
    /// Vrai : la palette garde le volet quand un élément est sélectionné (le dernier créé
    /// reste sélectionné pendant les créations successives).
    pub avec_selection: bool,
}

pub(crate) enum Definition {
    Palette(Palette),
    /// Réglages seuls (déplacer, cote, texte…) : titre et champs, le reste du volet continue.
    Parametres,
    /// Pas de panneau : action immédiate ou dessin sans réglage.
    Aucune,
}

const MODELISER: &[Menu] = &[Menu::Modeliser, Menu::Modifier];
const ANNOTER: &[Menu] = &[Menu::Annoter];

const fn palette(menus: &'static [Menu], selecteur: Emplacement, schema: Option<Emplacement>, parametres: Parametres, avec_selection: bool) -> Definition {
    Definition::Palette(Palette { selecteur, schema, parametres, menus, avec_selection })
}

pub(crate) fn definition(outil: Outil) -> Definition {
    use Outil::*;
    use Parametres::{Aucun, Champs, Propres};
    match outil {
        Mur => palette(MODELISER, selecteur_mur, Some(schema_mur), Propres(parametres_mur), true),
        Surfaces | Dalle | Sol | Toiture | Plafond | TerrePlein => palette(MODELISER, selecteur_surface, Some(schema_surface), Propres(parametres_surface), true),
        Escalier => palette(MODELISER, selecteur_escalier, Some(schema_escalier), Champs, false),
        GardeCorps => palette(MODELISER, selecteur_garde_corps, None, Champs, false),
        Modenature => palette(MODELISER, Application::selecteur_modenature, None, Propres(Application::parametres_modenature), true),
        Ouverture => palette(MODELISER, selecteur_ouverture, None, Propres(Application::parametres_ouverture), true),
        Poutre => palette(MODELISER, Application::selecteur_poutre, Some(Application::schema_poutre), Propres(Application::parametres_poutre), true),
        Poteau => palette(MODELISER, selecteur_poteau, Some(Application::schema_poteau), Propres(Application::parametres_poteau), true),
        Composant => palette(MODELISER, Application::selecteur_composant, None, Propres(Application::parametres_composant), true),
        Arbre => palette(MODELISER, Application::selecteur_arbre, None, Propres(Application::parametres_arbre), true),
        Piece => palette(MODELISER, Application::selecteur_piece, None, Propres(Application::parametres_piece), true),
        Niveau => palette(MODELISER, selecteur_niveau, None, Propres(Application::parametres_niveau), true),
        Quadrillage => palette(MODELISER, selecteur_quadrillage, None, Propres(Application::parametres_quadrillage), true),
        Terrain => palette(MODELISER, Application::selecteur_terrain, None, Propres(Application::parametres_terrain), true),
        LimitePropriete => palette(MODELISER, selecteur_limite, None, Aucun, true),
        VueCoupe => palette(ANNOTER, selecteur_coupe, None, Propres(parametres_coupe), true),
        VuePlan => palette(ANNOTER, selecteur_plan, None, Propres(parametres_plan), true),
        Texte => palette(ANNOTER, selecteur_texte, None, Propres(Application::parametres_texte), true),
        Cartouche => palette(ANNOTER, Application::selecteur_cartouche_pose, None, Aucun, true),
        Etiquette => palette(ANNOTER, selecteur_etiquette, None, Propres(Application::parametres_etiquette), true),
        Polyligne => palette(ANNOTER, Application::selecteur_trait_creation, None, Aucun, true),
        Spline => palette(ANNOTER, Application::selecteur_trait_creation, None, Aucun, true),
        Cercle => palette(ANNOTER, Application::selecteur_trait_creation, None, Aucun, true),
        Zone => palette(ANNOTER, selecteur_zone, None, Propres(Application::parametres_zone), true),
        Cote => palette(ANNOTER, selecteur_cote, None, Champs, true),
        CoteAngulaire => palette(ANNOTER, selecteur_cote_angulaire, None, Champs, true),
        CoteDiametre => palette(ANNOTER, selecteur_cote_diametre, None, Aucun, true),
        Altimetrie => palette(ANNOTER, selecteur_altimetrie, None, Champs, true),
        Symbole => palette(ANNOTER, selecteur_symbole, None, Champs, true),
        Legende => palette(ANNOTER, selecteur_legende, None, Champs, true),
        Deplacer | Copier | Tourner | Ajuster => Definition::Parametres,
        Symetrie | Reseau | Echelle | Decaler | Scinder | Demolir | Reproduire | ChangerHote | Inverser | Aligner | ModifierContour | CopierNiveau | Jonctions | AttacherToiture | CreerIdentique | Supprimer | Cadrer | Isoler | Masquer | MasquerVue | Afficher | Verrouiller | Deverrouiller | Rectangle | Feuilles | Nomenclatures | LienIfc | VueDessin => Definition::Aucune,
    }
}

/// Titre de l'emplacement « paramètres », le même pour tous les objets.
pub(crate) fn titre_parametres(ui: &mut egui::Ui, objet: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(format!("Paramètres — {objet}")).strong());
}

/// Sélecteur des objets sans type : en-tête « objet · Générique », au même endroit que le
/// sélecteur de type des autres objets.
macro_rules! selecteurs_generiques {
    ($($nom:ident => $titre:expr),* $(,)?) => {
        $(fn $nom(_: &mut Application, ui: &mut egui::Ui) {
            selecteur::entete_fixe(ui, &format!("{} · Générique", $titre), "Cet objet n'a pas de type à choisir.");
        })*
    };
}
selecteurs_generiques! {
    selecteur_escalier => "Escalier",
    selecteur_niveau => "Niveau",
    selecteur_quadrillage => "Quadrillage",
    selecteur_limite => "Limite de propriété",
    selecteur_plan => "Vue en plan",
    selecteur_texte => "Texte",
    selecteur_etiquette => "Étiquette",
    selecteur_zone => "Zone remplie",
    selecteur_cote => "Cote",
    selecteur_cote_angulaire => "Cote angulaire",
    selecteur_cote_diametre => "Cote de diamètre",
    selecteur_altimetrie => "Altimétrie",
    selecteur_symbole => "Symbole",
    selecteur_legende => "Légende",
}
/// Poteau : types de poteaux (sélecteur commun avec les poutres).
fn selecteur_poteau(a: &mut Application, ui: &mut egui::Ui) {
    a.selecteur_section(ui, modele::GenreBarre::Poteau);
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
fn selecteur_coupe(a: &mut Application, ui: &mut egui::Ui) {
    a.ruban_edition.selecteur_coupe(ui);
}
fn parametres_coupe(a: &mut Application, ui: &mut egui::Ui) {
    a.ruban_edition.parametres_coupe(ui);
}
fn parametres_plan(a: &mut Application, ui: &mut egui::Ui) {
    if let Some((niveau, hauteur)) = a.ruban_edition.parametres_plan(ui, a.natif.as_ref().map(|(p, _)| p)) {
        a.creer_vue(|p| p.ajouter_vue_plan(&niveau, hauteur), true);
    }
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
        self.oublier_annotation_identique();
        let menu = match self.onglet_ruban {
            OngletRuban::Modeliser => Menu::Modeliser,
            OngletRuban::Modifier => Menu::Modifier,
            OngletRuban::Annoter => Menu::Annoter,
            _ => return false,
        };
        let Some(outil) = self.ruban_edition.outil() else { return false };
        match definition(outil) {
            Definition::Palette(p) => {
                if !p.menus.contains(&menu) || (!p.avec_selection && !self.selection_bim().is_empty()) {
                    return false;
                }
                egui::ScrollArea::vertical().id_salt(("palette_creation", outil)).auto_shrink([false, true]).show(ui, |ui| {
                    (p.selecteur)(self, ui);
                    if let Some(schema) = p.schema {
                        schema(self, ui);
                    }
                    match p.parametres {
                        Parametres::Aucun => {}
                        Parametres::Champs => self.ruban_edition.section_parametres(ui, outil, self.natif.as_ref().map(|(p, _)| p)),
                        Parametres::Propres(parametres) => parametres(self, ui),
                    }
                });
                true
            }
            Definition::Parametres => {
                if self.ruban_edition.outil_actif_dans(menu).is_some() {
                    self.ruban_edition.section_parametres(ui, outil, self.natif.as_ref().map(|(p, _)| p));
                }
                false
            }
            Definition::Aucune => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un outil qui a des champs doit les montrer : palette ou réglages seuls. 
    #[test]
    fn tout_outil_avec_champs_montre_ses_parametres() {
        for &outil in Outil::TOUS {
            if !outil.a_des_champs() {
                continue;
            }
            let ok = match definition(outil) {
                Definition::Palette(_) | Definition::Parametres => true,
                // Zone remplie : aucun bouton dans le ruban (régions = polylignes fermées)
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
