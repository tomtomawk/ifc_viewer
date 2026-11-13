//! Éditeur de familles d'ouverture, à la manière de Revit : un mur générique, des plans
//! de référence, des cotes et égalités (EQ) libellées par des paramètres et résolues par
//! priorité, des volumes pleins et des vides dont les faces sont verrouillées sur les
//! plans. Les vides percent le mur une fois la famille chargée dans la maquette.
//!
//! Vues : élévation intérieure (largeur × hauteur), plan (largeur × épaisseur, extérieur
//! en haut), coupe (épaisseur × hauteur, extérieur à droite), ouvertes depuis
//! l'arborescence. Sélection par défaut (Échap y revient), plan de référence par deux
//! points, volume, vide, cote, égalité. Annuler / Rétablir propres à la famille. Les
//! valeurs d'essai (largeur, hauteur, épaisseur du mur) font fléchir la famille avant
//! enregistrement ; rien n'est écrit dans le projet avant « Enregistrer le type » ou
//! « Nouveau type ».

mod documents;
pub(super) mod catalogue;
mod panneau;
use panneau::*;
mod canevas;
use canevas::*;
mod formes;
mod transformer;
use super::*;
use super::accrochage;
use crate::modele::famille::modelee::{AxePlan, PlanConstruction, Contrainte, CoteMesure, FormeProfil, OperationForme, Face, FamilleModelee, GenreParametre, Hote, Materiau, Parametre, PlanRef, PointEsquisse, Portee, Resolution, Symbole, VueEsquisse, Volume, EPAISSEUR, HAUTEUR, LARGEUR};
use crate::modele::famille::modelee::FichierFamille;
use crate::modele::documentation::GenreCote;
use crate::modele::GenreOuverture;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum VueFamille {
    Elevation,
    Plan,
    Coupe,
}

impl VueFamille {
    pub(super) const TOUTES: [Self; 3] = [Self::Elevation, Self::Plan, Self::Coupe];
    /// Axes horizontal et vertical de la vue.
    fn axes(self) -> (AxePlan, AxePlan) {
        match self {
            Self::Elevation => (AxePlan::X, AxePlan::Z),
            Self::Plan => (AxePlan::X, AxePlan::Y),
            Self::Coupe => (AxePlan::Y, AxePlan::Z),
        }
    }
    fn esquisse(self) -> VueEsquisse {
        match self {
            Self::Elevation => VueEsquisse::Elevation,
            Self::Plan => VueEsquisse::Plan,
            Self::Coupe => VueEsquisse::Coupe,
        }
    }
    pub(super) fn nom(self) -> &'static str {
        match self {
            Self::Elevation => "Élévation intérieure",
            Self::Plan => "Plan",
            Self::Coupe => "Coupe",
        }
    }
}

pub(super) fn choisir_vue_famille(ui: &mut egui::Ui, actuelle: Option<VueFamille>, nom: impl Fn(VueFamille) -> &'static str) -> Option<VueFamille> {
    let mut choix = None;
    egui::CollapsingHeader::new(RichText::new("Vues").strong()).default_open(true).show(ui, |ui| {
        for vue in VueFamille::TOUTES {
            if ui.selectable_label(actuelle == Some(vue), nom(vue)).clicked() { choix = Some(vue); }
        }
    });
    choix
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OutilFamille {
    /// Outil par défaut, comme dans le projet : Échap y revient.
    Selection,
    /// Plan de référence tracé par deux points ; vertical ou horizontal selon le
    /// tracé.
    PlanReference,
    PlanConstruction,
    Volume,
    Vide,
    Cote,
    Egalite,
    /// Profil quelconque cliqué sommet par sommet, extrudé (plein ou vide).
    Extrusion,
    ExtrusionVide,
    Revolution,
    RevolutionVide,
    ExtrusionChemin,
    Raccordement,
    RaccordementChemin,
    /// Symbole 2D : ligne brisée, ou arc par trois points.
    Ligne,
    Arc,
    Spline,
    Cercle,
    Rectangle,
    Texte,
    Libelle,
    /// Cotes de mesure (non pilotantes) : angle, diamètre, altimétrie.
    CoteAngle,
    CoteDiametre,
    Altimetrie,
    /// Onglet Modifier : transformations de la sélection par points cliqués.
    Deplacer,
    Copier,
    Tourner,
    Symetrie,
}

impl OutilFamille {
    fn est_revolution(self) -> bool { matches!(self, Self::Revolution | Self::RevolutionVide) }
    pub(super) fn est_transformation(self) -> bool { matches!(self, Self::Deplacer | Self::Copier | Self::Tourner | Self::Symetrie) }
    fn outil_dessin(self) -> Option<crate::dessin::dao::OutilDessin> {
        use crate::dessin::dao::OutilDessin as D;
        match self { Self::Ligne => Some(D::Polyligne), Self::Arc => Some(D::Arc), Self::Spline => Some(D::Spline), Self::Cercle => Some(D::Cercle), Self::Rectangle => Some(D::Rectangle), _ => None }
    }
    /// Genre de cote de mesure saisi par l'outil.
    fn cote_mesure(self) -> Option<GenreCote> {
        match self {
            Self::CoteAngle => Some(GenreCote::Angulaire),
            Self::CoteDiametre => Some(GenreCote::Diametre),
            Self::Altimetrie => Some(GenreCote::Altimetrie),
            _ => None,
        }
    }
}

/// Poignée déplacée dans la vue : sommet d'un profil ou d'un symbole, face d'un volume.
#[derive(Clone, Copy, PartialEq)]
enum Poignee {
    Sommet { extrusion: bool, k: usize, i: usize },
    Face { k: usize, axe: usize, max: bool },
    Element2D { k: usize, i: usize },
}

#[derive(Clone, PartialEq)]
enum SelectionFamille {
    Aucune,
    Plan(String),
    Volume(usize),
    Contrainte(usize),
    Extrusion(usize),
    Symbole(usize),
    Element2D(usize),
    CoteMesure(usize),
}

/// Action demandée depuis les panneaux de l'éditeur, traitée après leur tracé.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionFamille {
    /// Charger dans le projet : le type édité (false) ou un nouveau type (true).
    Charger(bool),
    Enregistrer,
    EnregistrerSous,
    Ouvrir,
}

struct DeplacementFamille {
    selection: Vec<SelectionFamille>,
    precedent: egui::Pos2,
    avant: FamilleModelee,
}

pub(super) struct EditeurFamille {
    pub(super) document_famille: super::document_famille::DocumentFamille<FichierFamille>,
    /// Fenêtre d'où l'éditeur a été ouvert : elle prend le type enregistré.
    ouverture: Option<String>,
    pub(super) cible_feuille: Option<String>,
    pub(super) cible_piece: Option<String>,
    /// Type édité ; None : nouveau type.
    type_id: Option<String>,
    nom: String,
    largeur: f64,
    hauteur: f64,
    epaisseur: f64,
    famille: FamilleModelee,
    vue: VueFamille,
    construction: Option<usize>,
    vue_3d: bool,
    /// Vues ouvertes dans ce document : 0 élévation, 1 plan, 2 coupe, 3 trois dimensions.
    vues_ouvertes: Vec<usize>,
    camera_3d: crate::rendu::camera::Camera,
    cadrage_3d_initial: bool,
    navigation: [NavigationFamille;3],
    rect_navigation: egui::Rect,
    outil: OutilFamille,
    selection: SelectionFamille,
    /// Plans choisis pour la cote ou l'égalité en cours.
    clics: Vec<String>,
    /// Début du rectangle d'un volume en cours (repère de la famille).
    debut: Option<(f64, f64)>,
    /// Plan déplacé à la souris.
    deplace: Option<String>,
    /// Sommets de l'esquisse en cours (verrouillés et résolus).
    esquisse: Vec<(PointEsquisse, (f64, f64))>,
    trace_polyligne: Option<crate::dessin::dao::Trace>,
    saisie_trace: super::saisie_trace::SaisieTrace,
    reperage: super::reperage::Reperage,
    axe_revolution: Option<[[f64; 2]; 2]>,
    /// Points déjà cliqués par l'outil de transformation actif (repère de la vue).
    points_transfo: Vec<(f64, f64)>,
    /// Paramètres partagés du projet (copie modifiable ; seules les valeurs changées
    /// sont reportées à l'enregistrement).
    partages: BTreeMap<String, f64>,
    /// Paramètres partagés tels que lus à l'ouverture.
    partages_origine: BTreeMap<String, f64>,
    /// Matériaux du projet (nom, couleur) : valeurs des paramètres matériau.
    materiaux_projet: Vec<(String, [u8; 3])>,
    poignee: Option<Poignee>,
    /// État chargé dans le projet ou enregistré (nom, cotes, famille) : modifiée s'il
    /// diffère.
    reference: (String, f64, f64, f64, FamilleModelee),
    message: Option<String>,
    erreur: Option<String>,
    /// Cote dont la ligne est glissée.
    cote_deplacee: Option<usize>,
    /// Annotations déplacées ensemble, avec le dernier point écran du geste.
    deplacement_2d: Option<DeplacementFamille>,
    /// Autres éléments sélectionnés (Ctrl+clic, fenêtre) ; `selection` reste
    /// l'élément principal (panneau, poignées).
    multiple: Vec<SelectionFamille>,
    /// Fenêtre de sélection en cours (même règle que le projet).
    fenetre: Option<FenetreSelection>,
}

#[derive(Clone,Copy)]
pub(super) struct NavigationFamille { zoom:f64, decalage:(f64,f64) }
impl Default for NavigationFamille {
    fn default()->Self {Self{zoom:1.0,decalage:(0.0,0.0)}}
}
impl NavigationFamille {
    pub(super) fn cadre(&self,base:&Cadre)->Cadre {
        Cadre{centre:base.centre,milieu:(base.milieu.0+self.decalage.0,base.milieu.1+self.decalage.1),echelle:base.echelle*self.zoom}
    }
    pub(super) fn naviguer(&mut self,base:&Cadre,curseur:egui::Pos2,molette:f32,pan:egui::Vec2) {
        let avant=self.cadre(base);
        if molette!=0.0 {
            let point=avant.modele(curseur);
            self.zoom=(self.zoom*super::accrochage::facteur_molette(molette)).clamp(0.05,100.0);
            let apres=self.cadre(base).modele(curseur);
            self.decalage.0+=point.0-apres.0;
            self.decalage.1+=point.1-apres.1;
        }
        let echelle=base.echelle*self.zoom;
        self.decalage.0-=pan.x as f64/echelle;
        self.decalage.1+=pan.y as f64/echelle;
    }

    pub(super) fn souris(&mut self, ui: &egui::Ui, vue: &egui::Response, base: &Cadre, vue_plane: bool, outil_actif: bool) {
        if vue.double_clicked_by(PointerButton::Middle) {
            *self = Self::default();
            return;
        }
        // Les réponses de glissement lisent elles-mêmes le contexte egui.
        let panoramique = vue.dragged_by(PointerButton::Middle)
            || (vue_plane && !outil_actif && vue.dragged_by(PointerButton::Secondary));
        let pan = if panoramique { vue.drag_delta() } else { egui::Vec2::ZERO };
        let survole = vue.hovered();
        let (molette, curseur) = ui.input_mut(|i| {
            let molette = if survole { std::mem::take(&mut i.smooth_scroll_delta.y) } else { 0.0 };
            let ancre = if vue_plane { base.centre } else { i.pointer.hover_pos().unwrap_or(base.centre) };
            (molette, ancre)
        });
        self.naviguer(base, curseur, molette, pan);
        if panoramique {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
    }
}

/// Transformation de la vue : repère de la famille (h, v) ↔ écran.
pub(super) struct Cadre {
    pub centre: egui::Pos2,
    pub milieu: (f64, f64),
    pub echelle: f64,
}

impl Cadre {
    pub(super) fn ecran(&self, h: f64, v: f64) -> egui::Pos2 {
        self.centre + egui::vec2(((h - self.milieu.0) * self.echelle) as f32, (-(v - self.milieu.1) * self.echelle) as f32)
    }
    pub(super) fn modele(&self, p: egui::Pos2) -> (f64, f64) {
        let d = p - self.centre;
        (self.milieu.0 + d.x as f64 / self.echelle, self.milieu.1 - d.y as f64 / self.echelle)
    }
}

fn couleur_materiau(f: &FamilleModelee, m: Materiau) -> Color32 {
    let [r, g, b] = f.couleur;
    match m {
        Materiau::Menuiserie => Color32::from_rgb(r, g, b),
        Materiau::Vitrage => Color32::from_rgba_unmultiplied(150, 200, 225, 170),
        Materiau::Panneau => Color32::from_rgb((r as f32 * 0.85) as u8, (g as f32 * 0.85) as u8, (b as f32 * 0.85) as u8),
        Materiau::Appui => Color32::from_rgb(200, 195, 185),
    }
}

/// Nom libre dérivé de `base` (« Plan 3 »).
fn nom_libre(existants: impl Fn(&str) -> bool, base: &str) -> String {
    (1..).map(|k| format!("{base} {k}")).find(|n| !existants(n)).unwrap()
}

impl EditeurFamille {

    /// Modifiée depuis le dernier chargement dans le projet ou enregistrement.
    /// Pas encore de type dans le projet.
    pub(super) fn nouvelle(&self) -> bool {
        self.type_id.is_none()
    }

    pub(super) fn modifiee(&self) -> bool {
        (self.type_id.is_none() && self.document_famille.chemin_fichier.is_none()) || self.contenu_modifie()
    }

    pub(super) fn contenu_modifie(&self) -> bool {
        self.reference != (self.nom.clone(), self.largeur, self.hauteur, self.epaisseur, self.famille.clone())
    }

    fn marquer_enregistree(&mut self) {
        self.reference = (self.nom.clone(), self.largeur, self.hauteur, self.epaisseur, self.famille.clone());
    }

    fn fichier(&self) -> FichierFamille {
        FichierFamille { nom: self.nom.trim().into(), ..self.etat() }
    }

    /// État édité, tel que l'historique le garde.
    fn etat(&self) -> FichierFamille {
        FichierFamille {
            format: crate::modele::famille::modelee::FORMAT_FAMILLE.into(),
            nom: self.nom.clone(),
            largeur: self.largeur,
            hauteur: self.hauteur,
            profondeur: self.epaisseur,
            famille: self.famille.clone(),
        }
    }

    /// Verse la modification de l'image dans l'historique ; un geste (souris enfoncée,
    /// saisie) reste une seule étape.
    pub(super) fn suivre_historique(&mut self, geste: bool) {
        let etat = self.etat();
        if let Err(e) = self.document_famille.suivre(&etat, geste) {
            self.message = Some(format!("Historique : {e}"));
        }
    }

    /// Repart d'un état neuf : rien à annuler.
    fn historique_neuf(&mut self) {
        self.document_famille.historique = Default::default();
        self.document_famille.instantane = self.etat();
    }

    pub(super) fn libelles_historique(&self, retablir: bool) -> Vec<String> {
        self.document_famille.historique.libelles(retablir)
    }

    /// Annule ou rétablit `nombre` étapes de la famille.
    pub(super) fn parcourir_historique(&mut self, retablir: bool, nombre: usize) -> Result<(), String> {
        self.abandonner_saisie();
        let actuel = self.etat();
        let cible = self.document_famille.cible(&actuel, retablir, nombre)?;
        (self.nom, self.largeur, self.hauteur, self.epaisseur, self.famille) = (cible.nom.clone(), cible.largeur, cible.hauteur, cible.profondeur, cible.famille.clone());
        let existe = match &self.selection {
            SelectionFamille::Aucune => true,
            SelectionFamille::Plan(n) => self.famille.plan(n).is_some(),
            SelectionFamille::Volume(k) => *k < self.famille.volumes.len(),
            SelectionFamille::Contrainte(k) => *k < self.famille.contraintes.len(),
            SelectionFamille::Extrusion(k) => *k < self.famille.extrusions.len(),
            SelectionFamille::Symbole(k) => *k < self.famille.symboles.len(),
            SelectionFamille::Element2D(k) => *k < self.famille.elements_2d.len(),
            SelectionFamille::CoteMesure(k) => *k < self.famille.cotes_mesure.len(),
        };
        if !existe {
            self.selection = SelectionFamille::Aucune;
        }
        self.multiple.clear();
        Ok(())
    }

    /// Supprime un élément de la famille (panneau ou touche Suppr) ; un plan hôte ou
    /// encore utilisé reste.
    fn supprimer(&mut self, element: &SelectionFamille) -> Result<(), String> {
        let f = &mut self.famille;
        match element {
            SelectionFamille::Aucune => return Ok(()),
            SelectionFamille::Plan(nom) => {
                let k = f.plans.iter().position(|p| &p.nom == nom).ok_or("Plan introuvable.")?;
                if f.plans[k].hote.is_some() || f.plans[k].fixe {
                    return Err(format!("« {nom} » est un plan fixe de la famille."));
                }
                let point = |p: &PointEsquisse| p.h.plan == *nom || p.v.plan == *nom;
                let utilise = f.plans_construction.iter().any(|p| p.reference.as_ref() == Some(nom))
                    || f.contraintes.iter().any(|c| c.plans().contains(&nom.as_str()))
                    || f.volumes.iter().any(|v| v.min.iter().chain(&v.max).any(|x| &x.plan == nom))
                    || f.extrusions.iter().any(|e| e.profil.iter().any(point) || e.profondeur.iter().any(|x| &x.plan == nom))
                    || f.symboles.iter().any(|s| s.points.iter().any(point))
                    || f.cotes_mesure.iter().any(|c| c.points.iter().any(point));
                if utilise {
                    return Err(format!("« {nom} » est utilisé par une contrainte, un volume, une extrusion ou un symbole."));
                }
                f.plans.remove(k);
            }
            SelectionFamille::Volume(k) if *k < f.volumes.len() => drop(f.volumes.remove(*k)),
            SelectionFamille::Contrainte(k) if *k < f.contraintes.len() => drop(f.contraintes.remove(*k)),
            SelectionFamille::Extrusion(k) if *k < f.extrusions.len() => drop(f.extrusions.remove(*k)),
            SelectionFamille::Symbole(k) if *k < f.symboles.len() => {
                use crate::modele::famille::modelee::SectionProfil;
                if let Some(SectionProfil::Symbole(i)) = f.section_profil {
                    f.section_profil = if i == *k { None } else { Some(SectionProfil::Symbole(i - usize::from(i > *k))) };
                }
                f.symboles.remove(*k);
            }
            SelectionFamille::Element2D(k) if *k < f.elements_2d.len() => {
                use crate::modele::famille::modelee::SectionProfil;
                if let Some(SectionProfil::Element2d(i)) = f.section_profil {
                    f.section_profil = if i == *k { None } else { Some(SectionProfil::Element2d(i - usize::from(i > *k))) };
                }
                f.elements_2d.remove(*k);
            }
            SelectionFamille::CoteMesure(k) if *k < f.cotes_mesure.len() => drop(f.cotes_mesure.remove(*k)),
            _ => return Err("Élément introuvable.".into()),
        }
        if self.selection == *element {
            self.selection = SelectionFamille::Aucune;
        }
        Ok(())
    }

    /// Supprime la sélection (touche Suppr dans l'onglet Famille) : objets par indice
    /// décroissant, puis les plans devenus libres.
    pub(super) fn supprimer_selection(&mut self) {
        let mut liste = self.selectionnes();
        if liste.is_empty() {
            self.message = Some("Supprimer : sélectionner d'abord un élément de la famille.".into());
            return;
        }
        let rang = |s: &SelectionFamille| match s {
            SelectionFamille::Contrainte(k) => (0, usize::MAX - k),
            SelectionFamille::Volume(k) => (1, usize::MAX - k),
            SelectionFamille::Extrusion(k) => (2, usize::MAX - k),
            SelectionFamille::Symbole(k) => (3, usize::MAX - k),
            SelectionFamille::CoteMesure(k) | SelectionFamille::Element2D(k) => (3, usize::MAX - k),
            _ => (4, 0),
        };
        liste.sort_by_key(rang);
        let (mut n, mut erreurs) = (0, Vec::new());
        for e in &liste {
            match self.supprimer(e) {
                Ok(()) => n += 1,
                Err(x) => erreurs.push(x),
            }
        }
        self.selectionner(Vec::new());
        self.message = Some(format!("{n} élément(s) supprimé(s) — Ctrl+Z pour annuler."));
        self.erreur = (!erreurs.is_empty()).then(|| erreurs.join(" "));
    }

    /// Abandonne la saisie en cours (esquisse, cote, rectangle, glissement).
    fn abandonner_saisie(&mut self) {
        self.axe_revolution = None;
        self.points_transfo.clear();
        self.esquisse.clear();
        self.trace_polyligne = None;
        self.saisie_trace.vider();
        self.reperage = Default::default();
        self.clics.clear();
        self.debut = None;
        self.deplace = None;
        self.poignee = None;
        self.cote_deplacee = None;
        if let Some(geste) = self.deplacement_2d.take() { self.famille = geste.avant; }
        self.fenetre = None;
    }

    /// Élément sélectionné (principal ou parmi les autres).
    fn choisi(&self, e: &SelectionFamille) -> bool {
        self.selection == *e || self.multiple.contains(e)
    }

    /// Garde la correspondance des sections des raccordements lors de l'édition du profil.
    fn inserer_sommet_forme(&mut self, k: usize, i: usize) {
        let Some(forme) = self.famille.extrusions.get_mut(k) else { return };
        if i == 0 || i > forme.profil.len() { return; }
        forme.profil.insert(i, forme.profil[i - 1].clone());
        if let OperationForme::Guidage { guidage: crate::modele::famille::modelee::FormeGuidee::Raccordement { profil_fin, .. } | crate::modele::famille::modelee::FormeGuidee::RaccordementChemin { profil_fin, .. } } = &mut forme.operation {
            if !profil_fin.is_empty() && i <= profil_fin.len() {
                let (a, b) = (profil_fin[i - 1], profil_fin[i % profil_fin.len()]);
                profil_fin.insert(i, [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]);
            }
        }
    }

    fn supprimer_sommet_forme(&mut self, k: usize, i: usize) -> bool {
        let Some(forme) = self.famille.extrusions.get_mut(k) else { return false };
        if forme.profil.len() <= 3 || i >= forme.profil.len() { return false; }
        forme.profil.remove(i);
        if let OperationForme::Guidage { guidage: crate::modele::famille::modelee::FormeGuidee::Raccordement { profil_fin, .. } | crate::modele::famille::modelee::FormeGuidee::RaccordementChemin { profil_fin, .. } } = &mut forme.operation {
            if i < profil_fin.len() { profil_fin.remove(i); }
        }
        true
    }

    /// Tous les éléments sélectionnés, le principal d'abord.
    fn selectionnes(&self) -> Vec<SelectionFamille> {
        std::iter::once(self.selection.clone()).filter(|s| *s != SelectionFamille::Aucune).chain(self.multiple.iter().cloned()).collect()
    }

    /// Remplace la sélection (le premier devient le principal).
    fn selectionner(&mut self, mut liste: Vec<SelectionFamille>) {
        liste.retain(|s| *s != SelectionFamille::Aucune);
        liste.dedup();
        self.selection = if liste.is_empty() { SelectionFamille::Aucune } else { liste.remove(0) };
        self.multiple = liste;
    }

    pub(super) fn trace_en_cours(&self) -> bool {
        !self.esquisse.is_empty() || !self.clics.is_empty() || self.debut.is_some() || !self.points_transfo.is_empty()
            || !self.saisie_trace.texte.is_empty()
    }

    /// Translate des éléments de la vue sans déplacer leurs plans de référence.
    /// La sélection est entièrement vérifiée avant toute modification.
    fn translater_selection(&mut self, selection: &[SelectionFamille], dh: f64, dv: f64) -> Result<(), String> {
        if !dh.is_finite() || !dv.is_finite() { return Err("Déplacement non fini.".into()); }
        let vue = self.vue.esquisse();
        let admissible = selection.iter().all(|s| match s {
            SelectionFamille::Element2D(k) => self.famille.elements_2d.get(*k).is_some_and(|e| e.vue == vue && !crate::ui::dessin_plan::poignees_forme(&e.forme).is_empty()),
            SelectionFamille::Symbole(k) => self.famille.symboles.get(*k).is_some_and(|s| s.vue == vue),
            SelectionFamille::Extrusion(k) => self.famille.extrusions.get(*k).is_some_and(|e| e.vue == vue && e.construction.is_none() && matches!(e.operation, OperationForme::Extrusion)),
            SelectionFamille::Volume(k) => *k < self.famille.volumes.len(),
            _ => false,
        });
        if !admissible { return Err("Déplacement groupé impossible : retirer les plans, contraintes, formes guidées ou éléments d’une autre vue de la sélection.".into()); }
        let (ah, av) = self.vue.axes();
        for s in selection {
            match *s {
                SelectionFamille::Element2D(k) => for p in crate::ui::dessin_plan::points_forme_mut(&mut self.famille.elements_2d[k].forme) { p[0] += dh; p[1] += dv; },
                SelectionFamille::Symbole(k) => for p in &mut self.famille.symboles[k].points { p.h.decalage += dh; p.v.decalage += dv; },
                SelectionFamille::Extrusion(k) => for p in &mut self.famille.extrusions[k].profil { p.h.decalage += dh; p.v.decalage += dv; },
                SelectionFamille::Volume(k) => {
                    let volume = &mut self.famille.volumes[k];
                    for faces in [&mut volume.min, &mut volume.max] { faces[ah as usize].decalage += dh; faces[av as usize].decalage += dv; }
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    /// Termine le tracé 2D avec le moteur commun, puis conserve les références de plans.
    fn terminer_polyligne(&mut self) {
        use crate::dessin::dao::{EvenementTrace, Forme2D, OutilDessin, Trace};
        let mut trace = self.trace_polyligne.take().unwrap_or_else(|| Trace::new(OutilDessin::Polyligne));
        if let Some(Forme2D::Polyligne { fermee, .. }) = trace.appliquer(EvenementTrace::Terminer).into_iter().next() {
            let points = std::mem::take(&mut self.esquisse).into_iter().map(|(p, _)| p).collect();
            self.famille.symboles.push(Symbole { vue: self.vue.esquisse(), points, arc: false, fermee, remplissage: None });
            self.selectionner(vec![SelectionFamille::Symbole(self.famille.symboles.len() - 1)]);
        }
    }

    /// Change d'outil ; la sélection reste l'outil par défaut.
    fn choisir_outil(&mut self, o: OutilFamille) {
        self.outil = if self.outil_disponible(o) { o } else { OutilFamille::Selection };
        self.abandonner_saisie();
        if let Some(outil) = self.outil.outil_dessin() { self.trace_polyligne = Some(crate::dessin::dao::Trace::new(outil)); }
        if self.outil.est_transformation() { self.message = Some(self.invite_transformation().into()); }
        else if self.outil.est_revolution() { self.message = Some("Révolution : cliquer les deux points de l'axe, puis dessiner le profil fermé d'un seul côté de l'axe.".into()); }
        else if self.outil.est_forme() { self.message = Some("Dessiner le profil fermé. Clic droit ou retour au premier sommet pour terminer ; régler ensuite la forme dans Propriétés.".into()); }
    }

    fn outil_disponible(&self, outil: OutilFamille) -> bool {
        self.indisponibilite_outil(outil).is_none()
    }

    fn indisponibilite_outil(&self, outil: OutilFamille) -> Option<&'static str> {
        if outil == OutilFamille::Selection { return None; }
        let Some(d) = catalogue::OUTILS.iter().find(|d| d.outil == outil) else {
            return Some("Outil de famille inconnu.");
        };
        if outil.est_transformation() && self.selection == SelectionFamille::Aucune && self.multiple.is_empty() {
            return Some("Sélectionner d'abord un ou plusieurs éléments de la famille.");
        }
        catalogue::indisponibilite(d, &catalogue::Contexte { famille_plane: false, vue_3d: self.vue_3d, plan_oblique: self.construction.is_some() })
    }

    /// Les deux premiers points de l'outil révolution définissent son axe.
    fn capter_axe_revolution(&mut self) -> bool {
        if !self.outil.est_revolution() || self.axe_revolution.is_some() { return false; }
        if self.esquisse.len() == 2 {
            let (a,b) = (self.esquisse[0].1,self.esquisse[1].1);
            if (b.0-a.0).hypot(b.1-a.1) < 0.1 {
                self.esquisse.pop();
                self.erreur = Some("Choisir deux points distincts pour l'axe.".into());
            } else {
                self.axe_revolution = Some([[a.0,a.1],[b.0,b.1]]);
                self.esquisse.clear();
                self.erreur = None;
                self.message = Some("Axe défini : tracer le profil fermé d'un seul côté, puis clic droit ou retour au premier sommet. L'angle se règle dans Propriétés.".into());
            }
        }
        true
    }

    /// Ouvre une vue de la famille.
    fn ouvrir_vue(&mut self, v: VueFamille) {
        self.construction = None;
        self.vue_3d = false;
        self.vue = v;
        let indice = v as usize;
        if !self.vues_ouvertes.contains(&indice) { self.vues_ouvertes.push(indice); }
        self.choisir_outil(self.outil);
    }

    fn ouvrir_vue_3d(&mut self) {
        self.vue_3d = true;
        self.construction = None;
        if !self.vues_ouvertes.contains(&3) { self.vues_ouvertes.push(3); }
        self.choisir_outil(OutilFamille::Selection);
    }

    fn resoudre(&self) -> Resolution {
        self.famille.resoudre_occurrence(self.largeur, self.hauteur, self.epaisseur, &self.partages, &BTreeMap::new())
    }

    fn position(&self, r: &Resolution, nom: &str) -> f64 {
        r.positions.get(nom).copied().unwrap_or(0.0)
    }

    /// Renomme un plan et toutes ses références (contraintes, faces).
    fn renommer_plan(&mut self, ancien: &str, nouveau: &str) -> Result<(), String> {
        let nouveau = nouveau.trim();
        if nouveau.is_empty() || (nouveau != ancien && self.famille.plan(nouveau).is_some()) {
            return Err(format!("Nom de plan vide ou déjà pris : « {nouveau} »."));
        }
        let f = &mut self.famille;
        for p in f.plans.iter_mut().filter(|p| p.nom == ancien) {
            p.nom = nouveau.into();
        }
        for plan in &mut f.plans_construction { if plan.reference.as_deref() == Some(ancien) { plan.reference = Some(nouveau.into()); } }
        for c in &mut f.contraintes {
            match c {
                Contrainte::Distance { a, b, .. } => [a, b].into_iter().filter(|x| *x == ancien).for_each(|x| *x = nouveau.into()),
                Contrainte::Egalite { a, m, b, .. } => [a, m, b].into_iter().filter(|x| *x == ancien).for_each(|x| *x = nouveau.into()),
            }
        }
        let faces = f.volumes.iter_mut().flat_map(|v| v.min.iter_mut().chain(v.max.iter_mut()))
            .chain(f.extrusions.iter_mut().flat_map(|e| e.profil.iter_mut().flat_map(|p| [&mut p.h, &mut p.v]).chain(e.profondeur.iter_mut())))
            .chain(f.symboles.iter_mut().flat_map(|s| s.points.iter_mut().flat_map(|p| [&mut p.h, &mut p.v])));
        for face in faces.filter(|x| x.plan == ancien) {
            face.plan = nouveau.into();
        }
        if self.selection == SelectionFamille::Plan(ancien.into()) {
            self.selection = SelectionFamille::Plan(nouveau.into());
        }
        Ok(())
    }

    /// Face accrochée pour une coordonnée : le plan le plus proche de l'axe, sans
    /// décalage à moins de `tol`, sinon avec le décalage arrondi au mm.
    fn accrocher(&self, r: &Resolution, axe: AxePlan, x: f64, tol: f64) -> Option<Face> {
        let (plan, d) = self.plan_proche(r, axe, x)?;
        Some(Face { plan, decalage: if d.abs() <= tol { 0.0 } else { d.round() } })
    }

    /// Couleur d'un solide : matériau du projet donné par un paramètre, sinon le rôle.
    fn couleur_solide(&self, r: &Resolution, materiau: Materiau, parametre: &Option<String>) -> Color32 {
        let projet = parametre.as_ref().and_then(|p| r.materiaux.get(p)).and_then(|m| self.materiaux_projet.iter().find(|(n, _)| n == m));
        match projet {
            Some((_, [cr, cg, cb])) if materiau != Materiau::Vitrage => Color32::from_rgb(*cr, *cg, *cb),
            _ => couleur_materiau(&self.famille, materiau),
        }
    }

    /// Plan de l'axe le plus proche d'une valeur (et son écart).
    fn plan_proche(&self, r: &Resolution, axe: AxePlan, x: f64) -> Option<(String, f64)> {
        self.famille
            .plans
            .iter()
            .filter(|p| p.axe == axe)
            .map(|p| (p.nom.clone(), x - self.position(r, &p.nom)))
            .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
    }

    /// Faces d'un volume sur l'axe que la vue ne montre pas : celles du dernier volume
    /// du même genre, sinon les faces du mur (± 5 mm pour un vide), sinon les plans
    /// extrêmes.
    fn faces_profondeur(&self, r: &Resolution, axe: AxePlan, vide: bool) -> [Face; 2] {
        let k = axe as usize;
        if let Some(e) = self.famille.extrusions.iter().rev().find(|e| e.vide == vide && e.vue.axes().2 == axe) {
            return e.profondeur.clone();
        }
        if let Some(v) = self.famille.volumes.iter().rev().find(|v| v.vide == vide) {
            return [v.min[k].clone(), v.max[k].clone()];
        }
        let marge = if vide { 5.0 } else { 0.0 };
        if axe == AxePlan::Y {
            let hote = |h| self.famille.plans.iter().find(|p| p.hote == Some(h)).map(|p| p.nom.clone());
            if let (Some(i), Some(e)) = (hote(Hote::Interieur), hote(Hote::Exterieur)) {
                return [Face { plan: i, decalage: -marge }, Face { plan: e, decalage: marge }];
            }
        }
        let mut plans: Vec<&PlanRef> = self.famille.plans.iter().filter(|p| p.axe == axe).collect();
        plans.sort_by(|a, b| self.position(r, &a.nom).total_cmp(&self.position(r, &b.nom)));
        let nom = |p: Option<&&PlanRef>| p.map_or_else(String::new, |p| p.nom.clone());
        [Face { plan: nom(plans.first()), decalage: 0.0 }, Face { plan: nom(plans.last()), decalage: 0.0 }]
    }
}

#[cfg(test)]
mod tests_navigation_famille {
    use super::*;
    #[test]
    fn zoom_conserve_le_point_sous_la_souris_et_pan_deplace_la_vue() {
        let base=Cadre{centre:egui::pos2(500.,400.),milieu:(0.,1000.),echelle:0.2};
        let mut n=NavigationFamille::default();
        let q=egui::pos2(650.,250.);
        let avant=n.cadre(&base).modele(q);
        n.naviguer(&base,q,200.,egui::Vec2::ZERO);
        let apres=n.cadre(&base).modele(q);
        assert!((avant.0-apres.0).abs()<1e-9 && (avant.1-apres.1).abs()<1e-9);
        let ecran=n.cadre(&base).ecran(avant.0,avant.1);
        n.naviguer(&base,q,0.,egui::vec2(30.,-20.));
        let suivant=n.cadre(&base).ecran(avant.0,avant.1);
        assert!((suivant-ecran-egui::vec2(30.,-20.)).length()<1e-4);
    }
}
