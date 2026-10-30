//! Éditeur de familles d'ouverture, à la manière de Revit : un mur générique, des plans
//! de référence, des cotes et égalités (EQ) libellées par des paramètres et résolues par
//! priorité, des volumes pleins et des vides dont les faces sont verrouillées sur les
//! plans. Les vides percent le mur une fois la famille chargée dans la maquette.
//!
//! Vues : élévation intérieure (largeur × hauteur), plan (largeur × épaisseur, extérieur
//! en haut), coupe (épaisseur × hauteur, extérieur à droite). Outils : sélection (et
//! déplacement des plans libres), plan de référence, volume, vide, cote, égalité. Les
//! valeurs d'essai (largeur, hauteur, épaisseur du mur) font fléchir la famille avant
//! enregistrement ; rien n'est écrit dans le projet avant « Enregistrer le type » ou
//! « Nouveau type ».
use super::*;
use crate::modele::famille_modelee::{AxePlan, Contrainte, Extrusion, Face, FamilleModelee, GenreParametre, Hote, Materiau, Parametre, PlanRef, PointEsquisse, Portee, Resolution, Symbole, VueEsquisse, Volume, EPAISSEUR, HAUTEUR, LARGEUR};
use crate::modele::GenreOuverture;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VueFamille {
    Elevation,
    Plan,
    Coupe,
}

impl VueFamille {
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
    fn nom(self) -> &'static str {
        match self {
            Self::Elevation => "Élévation intérieure",
            Self::Plan => "Plan",
            Self::Coupe => "Coupe",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutilFamille {
    Selection,
    PlanVertical,
    PlanHorizontal,
    Volume,
    Vide,
    Cote,
    Egalite,
    /// Profil quelconque cliqué sommet par sommet, extrudé (plein ou vide).
    Extrusion,
    ExtrusionVide,
    /// Symbole 2D : ligne brisée, ou arc par trois points.
    Ligne,
    Arc,
}

/// Poignée déplacée dans la vue : sommet d'un profil ou d'un symbole, face d'un volume.
#[derive(Clone, Copy, PartialEq)]
enum Poignee {
    Sommet { extrusion: bool, k: usize, i: usize },
    Face { k: usize, axe: usize, max: bool },
}

#[derive(Clone, PartialEq)]
enum SelectionFamille {
    Aucune,
    Plan(String),
    Volume(usize),
    Contrainte(usize),
    Extrusion(usize),
    Symbole(usize),
}

/// Action demandée depuis les panneaux de l'éditeur, traitée après leur tracé.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionFamille {
    /// Charger dans le projet : le type édité (false) ou un nouveau type (true).
    Charger(bool),
    EnregistrerSous,
    Ouvrir,
}

pub(super) struct EditeurFamille {
    /// Fenêtre d'où l'éditeur a été ouvert : elle prend le type enregistré.
    ouverture: Option<String>,
    /// Type édité ; None : nouveau type.
    type_id: Option<String>,
    nom: String,
    largeur: f64,
    hauteur: f64,
    epaisseur: f64,
    famille: FamilleModelee,
    vue: VueFamille,
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
    /// Paramètres partagés du projet (copie modifiable, réécrite à l'enregistrement).
    partages: BTreeMap<String, f64>,
    /// Matériaux du projet (nom, couleur) : valeurs des paramètres matériau.
    materiaux_projet: Vec<(String, [u8; 3])>,
    poignee: Option<Poignee>,
    /// État chargé dans le projet ou enregistré (nom, cotes, famille) : modifiée s'il
    /// diffère.
    reference: (String, f64, f64, FamilleModelee),
    message: Option<String>,
    erreur: Option<String>,
}

/// Transformation de la vue : repère de la famille (h, v) ↔ écran.
struct Cadre {
    centre: egui::Pos2,
    milieu: (f64, f64),
    echelle: f64,
}

impl Cadre {
    fn ecran(&self, h: f64, v: f64) -> egui::Pos2 {
        self.centre + egui::vec2(((h - self.milieu.0) * self.echelle) as f32, (-(v - self.milieu.1) * self.echelle) as f32)
    }
    fn modele(&self, p: egui::Pos2) -> (f64, f64) {
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
        self.type_id.is_none() || self.reference != (self.nom.clone(), self.largeur, self.hauteur, self.famille.clone())
    }

    fn marquer_enregistree(&mut self) {
        self.reference = (self.nom.clone(), self.largeur, self.hauteur, self.famille.clone());
    }

    fn fichier(&self) -> crate::modele::famille_modelee::FichierFamille {
        crate::modele::famille_modelee::FichierFamille {
            format: crate::modele::famille_modelee::FORMAT_FAMILLE.into(),
            nom: self.nom.trim().into(),
            largeur: self.largeur,
            hauteur: self.hauteur,
            famille: self.famille.clone(),
        }
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
        for c in &mut f.contraintes {
            match c {
                Contrainte::Distance { a, b, .. } => [a, b].into_iter().filter(|x| *x == ancien).for_each(|x| *x = nouveau.into()),
                Contrainte::Egalite { a, m, b, .. } => [a, m, b].into_iter().filter(|x| *x == ancien).for_each(|x| *x = nouveau.into()),
            }
        }
        for v in &mut f.volumes {
            for face in v.min.iter_mut().chain(v.max.iter_mut()).filter(|x| x.plan == ancien) {
                face.plan = nouveau.into();
            }
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

impl ViewerApp {
    /// Éditeur sur la famille du type de la fenêtre `id` (gabarit si le type n'a pas de
    /// famille modélisée).
    pub(super) fn ouvrir_editeur_famille(&mut self, id: &str) {
        let Some((p, _)) = &self.natif else { return };
        let Some(o) = p.ouvertures.iter().find(|o| o.id == id) else { return };
        let e = crate::modele::solides::hote(p, &o.mur).map_or(200.0, |h| h.1);
        let t = o.type_ouverture.clone();
        let (l, h, id, g) = (o.largeur, o.hauteur, o.id.clone(), o.genre);
        self.editeur_famille(t, Some(id), l, h, e, g);
    }

    /// Éditeur sur le type `t` (None : nouvelle famille de la catégorie `genre`), hors
    /// de toute ouverture placée.
    pub(super) fn ouvrir_editeur_famille_type(&mut self, t: Option<String>, genre: GenreOuverture) {
        let (l, h) = if genre == GenreOuverture::Porte { (900.0, 2150.0) } else { (1200.0, 1350.0) };
        self.editeur_famille(t, None, l, h, 300.0, genre);
    }

    fn editeur_famille(&mut self, t: Option<String>, ouverture: Option<String>, largeur: f64, hauteur: f64, epaisseur: f64, genre: GenreOuverture) {
        let Some((p, _)) = &self.natif else { return };
        let t = t.and_then(|id| p.types_ouvertures.iter().find(|x| x.id == id));
        let u = crate::options::unite();
        let (largeur, hauteur) = t.map_or((largeur, hauteur), |t| (t.largeur, t.hauteur));
        let genre = t.map_or(genre, |t| t.genre);
        let famille = t.and_then(|t| t.modele.clone()).unwrap_or_else(|| FamilleModelee::gabarit(genre));
        let prefixe = if famille.genre == GenreOuverture::Porte { "Porte" } else { "Fenêtre" };
        let partages = p.parametres_partages.clone();
        let materiaux_projet = p.materiaux.iter().map(|m| (m.nom.clone(), m.couleur)).collect();
        self.famille_active = true;
        self.famille = Some(EditeurFamille {
            ouverture,
            type_id: t.map(|t| t.id.clone()),
            nom: t.map_or_else(|| format!("{prefixe} {} × {}", u.afficher(largeur), u.afficher(hauteur)), |t| t.nom.clone()),
            largeur,
            hauteur,
            epaisseur,
            famille,
            esquisse: Vec::new(),
            partages,
            materiaux_projet,
            poignee: None,
            reference: (String::new(), 0.0, 0.0, FamilleModelee::gabarit_fenetre()),
            vue: VueFamille::Elevation,
            outil: OutilFamille::Selection,
            selection: SelectionFamille::Aucune,
            clics: Vec::new(),
            debut: None,
            deplace: None,
            message: None,
            erreur: None,
        });
        if let Some(ed) = self.famille.as_mut() {
            ed.marquer_enregistree();
        }
    }

    /// Enregistre la famille dans son type (ou un nouveau type) ; les fenêtres du type
    /// prennent ses dimensions ; la fenêtre d'origine prend le type.
    fn enregistrer_famille(&mut self, nouveau: bool) -> Result<String, String> {
        let ed = self.famille.as_ref().ok_or("Éditeur fermé.")?;
        if ed.nom.trim().is_empty() {
            return Err("Nommer le type.".into());
        }
        ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur)?;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut q = p.clone();
        let existant = ed.type_id.clone().filter(|id| !nouveau && q.types_ouvertures.iter().any(|t| &t.id == id));
        let id = match existant {
            Some(id) => {
                let t = q.types_ouvertures.iter_mut().find(|t| t.id == id).unwrap();
                (t.nom, t.largeur, t.hauteur, t.modele, t.genre) = (ed.nom.trim().into(), ed.largeur, ed.hauteur, Some(ed.famille.clone()), ed.famille.genre);
                t.famille = None;
                id
            }
            None => {
                let mut k = q.types_ouvertures.len() + 1;
                let id = loop {
                    let id = crate::modele::identifiant("type-ouverture", &format!("famille-{k}"));
                    if q.types_ouvertures.iter().all(|t| t.id != id) {
                        break id;
                    }
                    k += 1;
                };
                // nom unique parmi les types du projet
                let base = ed.nom.trim().to_string();
                let (mut nom, mut j) = (base.clone(), 2);
                while q.types_ouvertures.iter().any(|t| t.nom == nom) {
                    nom = format!("{base} ({j})");
                    j += 1;
                }
                q.types_ouvertures.push(crate::modele::TypeOuverture {
                    id: id.clone(),
                    nom,
                    genre: ed.famille.genre,
                    largeur: ed.largeur,
                    hauteur: ed.hauteur,
                    famille: None,
                    modele: Some(ed.famille.clone()),
                    origine: None,
                });
                id
            }
        };
        for o in q.ouvertures.iter_mut() {
            if ed.ouverture.as_ref() == Some(&o.id) {
                o.type_ouverture = Some(id.clone());
                o.representation = None;
            }
            if o.type_ouverture.as_ref() == Some(&id) {
                (o.largeur, o.hauteur, o.genre) = (ed.largeur, ed.hauteur, ed.famille.genre);
                // valeurs d'occurrence : seulement celles que la famille déclare
                o.valeurs.retain(|k, _| ed.famille.parametres.iter().any(|p| &p.nom == k && p.portee == Portee::Occurrence));
                o.materiaux.retain(|k, _| ed.famille.parametres.iter().any(|p| &p.nom == k && p.portee == Portee::Occurrence && p.genre == GenreParametre::Materiau));
            }
        }
        q.parametres_partages = ed.partages.clone();
        self.appliquer_projet(q)?;
        if let Some(ed) = self.famille.as_mut() {
            ed.type_id = Some(id.clone());
            ed.marquer_enregistree();
        }
        self.status = "Famille chargée dans le projet — Ctrl+S (hors de la famille) pour enregistrer le projet.".into();
        Ok(id)
    }

    /// Fenêtre de l'éditeur de familles.
    /// Titre de l'onglet de l'éditeur : catégorie et type.
    pub(super) fn titre_famille(&self) -> Option<String> {
        let ed = self.famille.as_ref()?;
        let genre = if ed.famille.genre == GenreOuverture::Porte { "Porte" } else { "Fenêtre" };
        Some(format!("Famille {genre} : {}{}", ed.nom, if ed.modifiee() { " *" } else { "" }))
    }

    /// Ferme l'onglet de l'éditeur de familles (la famille non enregistrée est perdue).
    pub(super) fn fermer_famille(&mut self) {
        self.famille = None;
        self.famille_active = false;
    }

    /// Vue centrale de l'onglet « Famille » : barre d'outils et vue de travail.
    pub(super) fn vue_famille(&mut self, ui: &mut egui::Ui) {
        let Some(ed) = self.famille.as_mut() else { return };
        egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(6.0).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            barre_outils(ui, ed);
            ui.separator();
            canevas(ui, ed);
        });
    }

    /// Panneau Propriétés dans l'onglet « Famille » : type, paramètres, contraintes,
    /// plans, volumes, extrusions, symboles ; état des contraintes ; chargement dans le
    /// projet et fichier.
    pub(super) fn proprietes_famille(&mut self, ui: &mut egui::Ui) {
        self.entete_panneau(ui, Panneau::Proprietes, "Propriétés de la famille");
        let placement = self.famille.as_ref().is_some_and(|e| e.ouverture.is_none());
        let mut action = None;
        let Some(ed) = self.famille.as_mut() else { return };
        let bas = 96.0;
        egui::ScrollArea::vertical().id_salt("famille_panneau").max_height((ui.available_height() - bas).max(160.0)).show(ui, |ui| panneau(ui, ed));
        ui.separator();
        let r = ed.resoudre();
        match (&ed.erreur, r.erreurs.first(), r.conflits.len()) {
            (Some(e), _, _) | (None, Some(e), _) => {
                ui.colored_label(Color32::from_rgb(200, 40, 40), e);
            }
            (None, None, n) if n > 0 => {
                ui.colored_label(Color32::from_rgb(200, 120, 20), format!("{n} contrainte(s) écartée(s) : priorité inférieure en conflit."));
            }
            _ => {
                ui.label(RichText::new("Contraintes satisfaites.").weak());
            }
        }
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(ed.type_id.is_some(), egui::Button::new("Charger dans le projet")).on_hover_text("Met à jour le type et toutes ses ouvertures (Ctrl+S dans l'onglet)").clicked() {
                action = Some(ActionFamille::Charger(false));
            }
            let aide = if placement { "Crée un type avec cette famille ; il sert au prochain placement" } else { "Crée un type avec cette famille et l'affecte à l'ouverture" };
            if ui.button("Nouveau type").on_hover_text(aide).clicked() {
                action = Some(ActionFamille::Charger(true));
            }
            if ui.button("Enregistrer sous…").on_hover_text("Fichier de famille .famille.json, à charger dans un autre projet").clicked() {
                action = Some(ActionFamille::EnregistrerSous);
            }
            if ui.button("Ouvrir…").on_hover_text("Ouvrir un fichier de famille dans l'éditeur").clicked() {
                action = Some(ActionFamille::Ouvrir);
            }
        });
        if action.is_some() {
            self.action_famille = action;
        }
    }

    /// Arborescence dans l'onglet « Famille » : vues de la famille (clic : ouvrir),
    /// paramètres, plans, volumes, extrusions et symboles (clic : sélectionner), puis
    /// les familles du projet.
    pub(super) fn arborescence_famille(&mut self, ui: &mut egui::Ui) {
        let titre = self.titre_famille().unwrap_or_default();
        self.entete_panneau(ui, Panneau::Arborescence, &format!("Arborescence - {titre}"));
        let mut ouvrir_type = None;
        let familles_projet = self.familles_projet();
        let Some(ed) = self.famille.as_mut() else { return };
        egui::ScrollArea::vertical().id_salt("famille_arborescence").auto_shrink([false, false]).show(ui, |ui| {
            egui::CollapsingHeader::new(RichText::new("Vues").strong()).default_open(true).show(ui, |ui| {
                for v in [VueFamille::Elevation, VueFamille::Plan, VueFamille::Coupe] {
                    if ui.selectable_label(ed.vue == v, v.nom()).clicked() {
                        ed.vue = v;
                        ed.esquisse.clear();
                        ed.debut = None;
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Paramètres ({})", ed.famille.parametres.len() + 3)).strong()).default_open(false).show(ui, |ui| {
                for n in [LARGEUR, HAUTEUR, EPAISSEUR] {
                    ui.label(RichText::new(format!("{n} (type / hôte)")).weak());
                }
                for p in &ed.famille.parametres {
                    ui.label(format!("{} · {} · {}", p.nom, p.genre.nom(), p.portee.nom()));
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Plans de référence ({})", ed.famille.plans.len())).strong()).default_open(false).show(ui, |ui| {
                for p in &ed.famille.plans {
                    if ui.selectable_label(ed.selection == SelectionFamille::Plan(p.nom.clone()), format!("{} · {}", p.nom, p.axe.nom())).clicked() {
                        ed.selection = SelectionFamille::Plan(p.nom.clone());
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Volumes ({})", ed.famille.volumes.len())).strong()).default_open(true).show(ui, |ui| {
                for (k, v) in ed.famille.volumes.iter().enumerate() {
                    let texte = if v.vide { format!("{} (vide)", v.nom) } else { v.nom.clone() };
                    if ui.selectable_label(ed.selection == SelectionFamille::Volume(k), texte).clicked() {
                        ed.selection = SelectionFamille::Volume(k);
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Extrusions ({})", ed.famille.extrusions.len())).strong()).default_open(true).show(ui, |ui| {
                for (k, e) in ed.famille.extrusions.iter().enumerate() {
                    let texte = if e.vide { format!("{} (vide)", e.nom) } else { e.nom.clone() };
                    if ui.selectable_label(ed.selection == SelectionFamille::Extrusion(k), texte).clicked() {
                        ed.selection = SelectionFamille::Extrusion(k);
                        // la vue de son esquisse, pour ses poignées
                        ed.vue = match e.vue {
                            VueEsquisse::Elevation => VueFamille::Elevation,
                            VueEsquisse::Plan => VueFamille::Plan,
                            VueEsquisse::Coupe => VueFamille::Coupe,
                        };
                    }
                }
            });
            egui::CollapsingHeader::new(RichText::new(format!("Symboles 2D ({})", ed.famille.symboles.len())).strong()).default_open(false).show(ui, |ui| {
                for (k, sy) in ed.famille.symboles.iter().enumerate() {
                    let texte = format!("{} {} · {}", if sy.arc { "Arc" } else { "Ligne" }, k + 1, match sy.vue { VueEsquisse::Plan => "plan", VueEsquisse::Elevation => "élévation", VueEsquisse::Coupe => "coupe" });
                    if ui.selectable_label(ed.selection == SelectionFamille::Symbole(k), texte).clicked() {
                        ed.selection = SelectionFamille::Symbole(k);
                    }
                }
            });
            ui.separator();
            egui::CollapsingHeader::new(RichText::new("Familles du projet").strong()).default_open(true).show(ui, |ui| {
                for (id, nom, genre) in &familles_projet {
                    let courant = ed.type_id.as_ref() == Some(id);
                    if ui.selectable_label(courant, format!("{nom} · {}", if *genre == GenreOuverture::Porte { "porte" } else { "fenêtre" })).on_hover_text("Ouvrir cette famille").clicked() && !courant {
                        ouvrir_type = Some((id.clone(), *genre));
                    }
                }
            });
        });
        if let Some((id, genre)) = ouvrir_type {
            if self.famille.as_ref().is_some_and(|e| e.modifiee()) {
                self.status = "Famille en cours modifiée : la charger dans le projet ou fermer son onglet avant d'en ouvrir une autre.".into();
            } else {
                self.ouvrir_editeur_famille_type(Some(id), genre);
            }
        }
    }

    /// Portes et fenêtres du projet : (type, nom, catégorie), par catégorie puis nom.
    pub(super) fn familles_projet(&self) -> Vec<(String, String, GenreOuverture)> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let mut v: Vec<(String, String, GenreOuverture)> = p.types_ouvertures.iter().filter(|t| t.genre != GenreOuverture::Libre).map(|t| (t.id.clone(), t.nom.clone(), t.genre)).collect();
        v.sort_by(|a, b| (a.2 != GenreOuverture::Porte, &a.1).cmp(&(b.2 != GenreOuverture::Porte, &b.1)));
        v
    }

    /// Enregistre la famille de l'éditeur dans un fichier.
    pub(super) fn enregistrer_famille_fichier(&mut self, chemin: &std::path::Path) -> Result<(), String> {
        let ed = self.famille.as_mut().ok_or("Éditeur fermé.")?;
        ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur)?;
        std::fs::write(chemin, ed.fichier().json()?).map_err(|e| format!("{} : {e}", chemin.display()))?;
        self.status = format!("Famille enregistrée : {}", chemin.display());
        Ok(())
    }

    /// Ouvre un fichier de famille dans l'éditeur (nouveau type, à charger ensuite).
    pub(super) fn ouvrir_famille_fichier(&mut self, chemin: &std::path::Path) -> Result<(), String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        let f = crate::modele::famille_modelee::FichierFamille::lire(&texte)?;
        self.editeur_famille(None, None, f.largeur, f.hauteur, 300.0, f.famille.genre);
        let ed = self.famille.as_mut().ok_or("Aucun projet natif.")?;
        (ed.nom, ed.famille) = (f.nom, f.famille);
        // ouverte depuis un fichier : pas encore dans le projet
        ed.reference.0.clear();
        Ok(())
    }

    /// Charge un fichier de famille dans le projet : un nouveau type (nom rendu unique).
    pub(super) fn charger_famille_fichier(&mut self, chemin: &std::path::Path) -> Result<String, String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        let f = crate::modele::famille_modelee::FichierFamille::lire(&texte)?;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let mut q = p.clone();
        let mut nom = f.nom.clone();
        let mut k = 2;
        while q.types_ouvertures.iter().any(|t| t.nom == nom) {
            nom = format!("{} ({k})", f.nom);
            k += 1;
        }
        let mut n = q.types_ouvertures.len() + 1;
        let id = loop {
            let id = crate::modele::identifiant("type-ouverture", &format!("famille-{n}"));
            if q.types_ouvertures.iter().all(|t| t.id != id) {
                break id;
            }
            n += 1;
        };
        q.types_ouvertures.push(crate::modele::TypeOuverture { id: id.clone(), nom: nom.clone(), genre: f.famille.genre, largeur: f.largeur, hauteur: f.hauteur, famille: None, modele: Some(f.famille), origine: None });
        self.appliquer_projet(q)?;
        self.status = format!("Famille « {nom} » chargée dans le projet.");
        Ok(id)
    }

    /// Traite l'action demandée dans les panneaux de l'éditeur (après leur tracé).
    pub(super) fn traiter_action_famille(&mut self) {
        let Some(action) = self.action_famille.take() else { return };
        let placement = self.famille.as_ref().is_some_and(|e| e.ouverture.is_none());
        let r = match action {
            ActionFamille::Charger(nouveau) => self.enregistrer_famille(nouveau).map(|id| {
                if placement {
                    self.ruban_edition.ouverture_type_identique = Some(id);
                }
            }),
            ActionFamille::EnregistrerSous => {
                let nom = self.famille.as_ref().map_or_else(String::new, |e| e.nom.trim().to_string());
                match rfd::FileDialog::new().add_filter("Famille", &["json"]).set_file_name(format!("{nom}.famille.json")).save_file() {
                    Some(c) => self.enregistrer_famille_fichier(&c),
                    None => Ok(()),
                }
            }
            ActionFamille::Ouvrir => match rfd::FileDialog::new().add_filter("Famille", &["json"]).pick_file() {
                Some(c) => self.ouvrir_famille_fichier(&c),
                None => Ok(()),
            },
        };
        if let Some(ed) = self.famille.as_mut() {
            ed.erreur = r.err();
        }
    }

    /// Fermeture de l'onglet : confirmation si la famille n'est pas chargée dans le
    /// projet.
    pub(super) fn demander_fermeture_famille(&mut self) {
        if self.famille.as_ref().is_some_and(|e| e.modifiee()) {
            self.confirmer_famille = true;
        } else {
            self.fermer_famille();
        }
    }

    pub(super) fn confirmation_famille(&mut self, ctx: &egui::Context) {
        if !self.confirmer_famille {
            return;
        }
        let nouveau = self.famille.as_ref().is_some_and(|e| e.type_id.is_none());
        let mut choix = None;
        egui::Modal::new(egui::Id::unique("fermer_famille")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Famille modifiée");
            ui.label("Les modifications ne sont pas chargées dans le projet.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(if nouveau { "Créer le type et fermer" } else { "Charger et fermer" }).clicked() {
                    choix = Some(0);
                }
                if ui.button("Fermer sans charger").clicked() {
                    choix = Some(1);
                }
                if ui.button("Annuler").clicked() {
                    choix = Some(2);
                }
            });
        });
        match choix {
            Some(0) => {
                self.confirmer_famille = false;
                match self.enregistrer_famille(nouveau) {
                    Ok(_) => self.fermer_famille(),
                    Err(e) => {
                        if let Some(ed) = self.famille.as_mut() {
                            ed.erreur = Some(e);
                        }
                    }
                }
            }
            Some(1) => {
                self.confirmer_famille = false;
                self.fermer_famille();
            }
            Some(_) => self.confirmer_famille = false,
            None => {}
        }
    }
}

fn barre_outils(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    ui.horizontal_wrapped(|ui| {
        for v in [VueFamille::Elevation, VueFamille::Plan, VueFamille::Coupe] {
            if ui.selectable_label(ed.vue == v, v.nom()).clicked() {
                ed.vue = v;
                ed.debut = None;
                ed.esquisse.clear();
            }
        }
        ui.separator();
        let (h, v) = ed.vue.axes();
        for (o, nom, aide) in [
            (OutilFamille::Selection, "Sélection", "Choisir un plan, un volume ou une cote ; glisser un plan libre pour le déplacer".to_string()),
            (OutilFamille::PlanVertical, "Plan |", format!("Plan de référence vertical ({}) : cliquer pour le poser", h.nom())),
            (OutilFamille::PlanHorizontal, "Plan —", format!("Plan de référence horizontal ({}) : cliquer pour le poser", v.nom())),
            (OutilFamille::Volume, "Volume", "Extrusion pleine : glisser un rectangle ; ses faces se verrouillent sur les plans les plus proches".to_string()),
            (OutilFamille::Vide, "Vide", "Extrusion vide : perce le mur hôte une fois la famille chargée".to_string()),
            (OutilFamille::Cote, "Cote", "Cliquer deux plans parallèles ; libeller ensuite la cote par un paramètre".to_string()),
            (OutilFamille::Egalite, "EQ", "Cliquer trois plans parallèles : le second reste au milieu des deux autres".to_string()),
            (OutilFamille::Extrusion, "Extrusion", "Profil quelconque : cliquer les sommets (accrochés aux plans), recliquer le premier ou clic droit pour fermer".to_string()),
            (OutilFamille::ExtrusionVide, "Extr. vide", "Profil quelconque qui perce le mur hôte".to_string()),
            (OutilFamille::Ligne, "Ligne 2D", "Symbole 2D (plan ou élévation) affiché dans les vues du projet : clic droit pour finir".to_string()),
            (OutilFamille::Arc, "Arc 2D", "Symbole 2D en arc : départ, point de passage, fin".to_string()),
        ] {
            if ui.selectable_label(ed.outil == o, nom).on_hover_text(aide).clicked() {
                ed.outil = o;
                ed.clics.clear();
                ed.debut = None;
                ed.esquisse.clear();
            }
        }
        if let Some(m) = &ed.message {
            ui.separator();
            ui.label(RichText::new(m).weak());
        }
    });
}

fn panneau(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    let r = ed.resoudre();
    let u = crate::options::unite();
    egui::Grid::new("famille_type").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label("Type");
        ui.text_edit_singleline(&mut ed.nom);
        ui.end_row();
        ui.label("Catégorie").on_hover_text("Range le type parmi les portes ou les fenêtres du sélecteur");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut ed.famille.genre, GenreOuverture::Porte, "Porte");
            ui.selectable_value(&mut ed.famille.genre, GenreOuverture::Fenetre, "Fenêtre");
        });
        ui.end_row();
        ui.label(LARGEUR).on_hover_text("Paramètre de type : largeur de la baie");
        ui.add(egui::DragValue::new(&mut ed.largeur).speed(10.0).range(100.0..=20_000.0).dimensions());
        ui.end_row();
        ui.label(HAUTEUR).on_hover_text("Paramètre de type : hauteur de la baie");
        ui.add(egui::DragValue::new(&mut ed.hauteur).speed(10.0).range(100.0..=20_000.0).dimensions());
        ui.end_row();
        ui.label(EPAISSEUR).on_hover_text("Épaisseur du mur hôte : essai seulement, la maquette donne celle du mur");
        ui.add(egui::DragValue::new(&mut ed.epaisseur).speed(10.0).range(50.0..=2000.0).dimensions());
        ui.end_row();
        ui.label("Couleur");
        ui.color_edit_button_srgb(&mut ed.famille.couleur);
        ui.end_row();
    });

    egui::CollapsingHeader::new(RichText::new("Paramètres").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        let mut partager = None;
        let materiaux_projet = &ed.materiaux_projet;
        egui::Grid::new("famille_parametres").num_columns(6).spacing([5.0, 4.0]).show(ui, |ui| {
            for (k, p) in ed.famille.parametres.iter_mut().enumerate() {
                ui.add_sized([88.0, 20.0], egui::TextEdit::singleline(&mut p.nom));
                // valeur selon la nature : formule, case oui/non, matériau du projet
                match p.genre {
                    _ if p.portee == Portee::Partage && ed.partages.contains_key(&p.nom) => {
                        ui.label(RichText::new("projet").weak()).on_hover_text("Valeur du paramètre partagé du projet (section ci-dessous) ; la formule sert à défaut");
                    }
                    GenreParametre::OuiNon => {
                        let mut oui = evaluer_oui(&p.formule);
                        if ui.checkbox(&mut oui, "").on_hover_text("Valeur par défaut (une formule reste possible : 0 = non)").changed() {
                            p.formule = if oui { "1".into() } else { "0".into() };
                        }
                    }
                    GenreParametre::Materiau => {
                        let texte = if p.formule.trim().is_empty() { "(rôle)".to_string() } else { p.formule.clone() };
                        egui::ComboBox::from_id_salt(("valeur_materiau", k)).width(80.0).selected_text(texte).show_ui(ui, |ui| {
                            ui.selectable_value(&mut p.formule, String::new(), "(couleur du rôle)");
                            for (n, _) in materiaux_projet {
                                ui.selectable_value(&mut p.formule, n.clone(), n);
                            }
                        });
                    }
                    GenreParametre::Longueur => {
                        let aide = if p.portee == Portee::Occurrence { "Valeur par défaut ; chaque ouverture placée peut la changer dans Propriétés" } else { "Formule : nombres (mm), paramètres, + - * / ( )" };
                        ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(&mut p.formule)).on_hover_text(aide);
                    }
                }
                egui::ComboBox::from_id_salt(("genre_parametre", k)).width(70.0).selected_text(p.genre.nom()).show_ui(ui, |ui| {
                    for g in GenreParametre::TOUS {
                        if ui.selectable_value(&mut p.genre, g, g.nom()).changed() {
                            p.formule = match g {
                                GenreParametre::Longueur => "100".into(),
                                GenreParametre::OuiNon => "1".into(),
                                GenreParametre::Materiau => String::new(),
                            };
                        }
                    }
                }).response.on_hover_text("Longueur (mm), oui/non (visibilité des volumes) ou matériau (couleur des volumes)");
                egui::ComboBox::from_id_salt(("portee", k)).width(74.0).selected_text(p.portee.nom()).show_ui(ui, |ui| {
                    for x in Portee::TOUTES.into_iter().filter(|x| *x != Portee::Partage || p.genre != GenreParametre::Materiau) {
                        ui.selectable_value(&mut p.portee, x, x.nom());
                    }
                }).response.on_hover_text("Type : valeur de la famille · Occurrence : propre à chaque ouverture · Partagé : valeur du projet (longueurs et oui/non)");
                match (p.genre, r.valeurs.get(&p.nom), r.materiaux.get(&p.nom)) {
                    (GenreParametre::Materiau, _, Some(m)) => ui.label(RichText::new(if m.is_empty() { "—" } else { m.as_str() }).weak()),
                    (GenreParametre::OuiNon, Some(v), _) => ui.label(RichText::new(if v.abs() > 1e-9 { "oui" } else { "non" }).weak()),
                    (_, Some(v), _) => ui.label(RichText::new(u.afficher(*v)).weak()),
                    _ => ui.colored_label(Color32::from_rgb(200, 40, 40), "?"),
                };
                if ui.small_button("−").on_hover_text("Supprimer le paramètre").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
                if p.portee == Portee::Partage && !ed.partages.contains_key(&p.nom) {
                    partager = Some(k);
                }
            }
        });
        // un paramètre déclaré partagé rejoint la table du projet avec sa valeur actuelle
        if let Some(k) = partager {
            let p = &ed.famille.parametres[k];
            if let Some(v) = r.valeurs.get(&p.nom) {
                ed.partages.insert(p.nom.clone(), *v);
            }
        }
        if let Some(k) = supprimer {
            ed.famille.parametres.remove(k);
        }
        ui.horizontal(|ui| {
            if ui.button("Ajouter un paramètre").clicked() {
                let nom = (1..).map(|k| format!("Parametre{k}")).find(|n| ed.famille.parametres.iter().all(|p| &p.nom != n)).unwrap();
                ed.famille.parametres.push(Parametre::nouveau(&nom, "100"));
            }
            let disponibles: Vec<(String, f64)> = ed.partages.iter().filter(|(n, _)| ed.famille.parametres.iter().all(|p| &p.nom != *n)).map(|(n, v)| (n.clone(), *v)).collect();
            ui.add_enabled_ui(!disponibles.is_empty(), |ui| {
                ui.menu_button("Depuis le projet…", |ui| {
                    for (n, v) in &disponibles {
                        if ui.button(format!("{n} ({})", u.afficher(*v))).clicked() {
                            ed.famille.parametres.push(Parametre { portee: Portee::Partage, ..Parametre::nouveau(n, &format!("{v}")) });
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Réutiliser un paramètre partagé du projet");
            });
        });
    });

    egui::CollapsingHeader::new(RichText::new("Paramètres partagés du projet").strong()).default_open(false).show(ui, |ui| {
        ui.label(RichText::new("Communs à toutes les familles qui les déclarent « Partagé » ; écrits dans le projet à l'enregistrement.").small().weak());
        let mut retirer = None;
        egui::Grid::new("famille_partages").num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
            for (n, v) in ed.partages.iter_mut() {
                ui.label(n);
                ui.add(egui::DragValue::new(v).speed(5.0).dimensions());
                if ui.small_button("−").on_hover_text("Retirer du projet (les familles reprennent leur formule)").clicked() {
                    retirer = Some(n.clone());
                }
                ui.end_row();
            }
        });
        if let Some(n) = retirer {
            ed.partages.remove(&n);
        }
    });

    egui::CollapsingHeader::new(RichText::new("Contraintes").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        let noms_parametres: Vec<String> = [LARGEUR, HAUTEUR, EPAISSEUR].into_iter().map(String::from).chain(ed.famille.parametres.iter().map(|p| p.nom.clone())).collect();
        egui::Grid::new("famille_contraintes").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
            for (k, c) in ed.famille.contraintes.iter_mut().enumerate() {
                let conflit = r.conflits.contains(&k);
                let choisie = ed.selection == SelectionFamille::Contrainte(k);
                let texte = match c {
                    Contrainte::Distance { a, b, .. } => format!("{a} – {b}"),
                    Contrainte::Egalite { a, m, b, .. } => format!("EQ {a} | {m} | {b}"),
                };
                let couleur = if conflit { Color32::from_rgb(200, 40, 40) } else { ui.visuals().text_color() };
                if ui.selectable_label(choisie, RichText::new(texte).color(couleur)).on_hover_text(if conflit { "Écartée : en conflit avec une contrainte de priorité supérieure" } else { "Satisfaite" }).clicked() {
                    ed.selection = SelectionFamille::Contrainte(k);
                }
                match c {
                    Contrainte::Distance { valeur, .. } => {
                        ui.horizontal(|ui| {
                            ui.add_sized([80.0, 20.0], egui::TextEdit::singleline(valeur)).on_hover_text("Valeur (mm) ou formule : libeller la cote par un paramètre pour la piloter");
                            egui::ComboBox::from_id_salt(("libelle", k)).width(18.0).selected_text("").show_ui(ui, |ui| {
                                for n in &noms_parametres {
                                    if ui.selectable_label(valeur == n, n).clicked() {
                                        *valeur = n.clone();
                                    }
                                }
                            });
                        });
                    }
                    Contrainte::Egalite { .. } => {
                        ui.label("EQ");
                    }
                }
                ui.add(egui::DragValue::new(c.priorite_mut()).range(0..=9).prefix("P ")).on_hover_text("Priorité : en cas de conflit, la plus haute l'emporte (à égalité, la plus ancienne)");
                ui.label(if conflit { RichText::new("conflit").color(Color32::from_rgb(200, 40, 40)) } else { RichText::new("ok").color(Color32::from_rgb(30, 140, 60)) });
                if ui.small_button("−").on_hover_text("Supprimer la contrainte").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some(k) = supprimer {
            ed.famille.contraintes.remove(k);
            ed.selection = SelectionFamille::Aucune;
        }
    });

    egui::CollapsingHeader::new(RichText::new("Plans de référence").strong()).default_open(false).show(ui, |ui| {
        let mut renommer = None;
        let mut supprimer = None;
        egui::Grid::new("famille_plans").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
            for (k, p) in ed.famille.plans.iter_mut().enumerate() {
                let id = egui::Id::unique(("nom_plan", k));
                let mut texte = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| p.nom.clone());
                let rep = ui.add_sized([110.0, 20.0], egui::TextEdit::singleline(&mut texte));
                if rep.changed() {
                    ui.data_mut(|d| d.insert_temp(id, texte.clone()));
                }
                if rep.lost_focus() {
                    ui.data_mut(|d| d.remove::<String>(id));
                    if texte != p.nom {
                        renommer = Some((p.nom.clone(), texte));
                    }
                }
                ui.label(RichText::new(p.axe.nom()).weak());
                ui.label(u.afficher(r.positions.get(&p.nom).copied().unwrap_or(p.position)));
                if p.hote.is_some() {
                    ui.label(RichText::new("hôte").weak());
                } else {
                    ui.checkbox(&mut p.fixe, "fixe").on_hover_text("Épinglé à sa position dessinée");
                }
                if p.hote.is_none() && ui.small_button("−").on_hover_text("Supprimer le plan (s'il n'est pas utilisé)").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some((a, n)) = renommer {
            ed.erreur = ed.renommer_plan(&a, &n).err();
        }
        if let Some(k) = supprimer {
            let nom = ed.famille.plans[k].nom.clone();
            let utilise = ed.famille.contraintes.iter().any(|c| c.plans().contains(&nom.as_str())) || ed.famille.volumes.iter().any(|v| v.min.iter().chain(&v.max).any(|f| f.plan == nom));
            if utilise {
                ed.erreur = Some(format!("« {nom} » est utilisé par une contrainte ou un volume."));
            } else {
                ed.famille.plans.remove(k);
                ed.selection = SelectionFamille::Aucune;
            }
        }
    });

    egui::CollapsingHeader::new(RichText::new("Volumes").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        for (k, v) in ed.famille.volumes.iter().enumerate() {
            ui.horizontal(|ui| {
                let texte = if v.vide { format!("{} (vide)", v.nom) } else { format!("{} · {}", v.nom, v.materiau.nom()) };
                if ui.selectable_label(ed.selection == SelectionFamille::Volume(k), texte).clicked() {
                    ed.selection = SelectionFamille::Volume(k);
                }
                if ui.small_button("−").on_hover_text("Supprimer le volume").clicked() {
                    supprimer = Some(k);
                }
            });
        }
        if let Some(k) = supprimer {
            ed.famille.volumes.remove(k);
            ed.selection = SelectionFamille::Aucune;
        }
        if let SelectionFamille::Volume(k) = ed.selection.clone() {
            if let Some(v) = ed.famille.volumes.get_mut(k) {
                ui.separator();
                volume_ui(ui, v, &ed.famille.plans, &ed.famille.parametres, k);
            }
        }
    });

    egui::CollapsingHeader::new(RichText::new("Extrusions").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        for (k, e) in ed.famille.extrusions.iter().enumerate() {
            ui.horizontal(|ui| {
                let texte = format!("{} · {} sommets{}", e.nom, e.profil.len(), if e.vide { " (vide)" } else { "" });
                if ui.selectable_label(ed.selection == SelectionFamille::Extrusion(k), texte).clicked() {
                    ed.selection = SelectionFamille::Extrusion(k);
                }
                if ui.small_button("−").on_hover_text("Supprimer l'extrusion").clicked() {
                    supprimer = Some(k);
                }
            });
        }
        if let Some(k) = supprimer {
            ed.famille.extrusions.remove(k);
            ed.selection = SelectionFamille::Aucune;
        }
        if let SelectionFamille::Extrusion(k) = ed.selection.clone() {
            if let Some(e) = ed.famille.extrusions.get_mut(k) {
                ui.separator();
                extrusion_ui(ui, e, &ed.famille.plans, &ed.famille.parametres, k);
            }
        }
    });

    egui::CollapsingHeader::new(RichText::new("Symboles 2D").strong()).default_open(true).show(ui, |ui| {
        let mut supprimer = None;
        for (k, s) in ed.famille.symboles.iter().enumerate() {
            ui.horizontal(|ui| {
                let vue = match s.vue {
                    VueEsquisse::Plan => "plan",
                    VueEsquisse::Elevation => "élévation",
                    VueEsquisse::Coupe => "coupe (éditeur seulement)",
                };
                let texte = format!("{} {} · {vue}", if s.arc { "Arc" } else { "Ligne" }, k + 1);
                if ui.selectable_label(ed.selection == SelectionFamille::Symbole(k), texte).clicked() {
                    ed.selection = SelectionFamille::Symbole(k);
                }
                if ui.small_button("−").on_hover_text("Supprimer le symbole").clicked() {
                    supprimer = Some(k);
                }
            });
        }
        if let Some(k) = supprimer {
            ed.famille.symboles.remove(k);
            ed.selection = SelectionFamille::Aucune;
        }
        ui.label(RichText::new("Les symboles de plan et d'élévation s'affichent dans les vues du projet et sur les feuilles.").small().weak());
    });
}

/// Valeur oui/non d'une formule simple (« 0 » : non ; illisible : oui).
fn evaluer_oui(formule: &str) -> bool {
    crate::modele::famille_modelee::evaluer(formule, &BTreeMap::new()).map_or(true, |v| v.abs() > 1e-9)
}

/// Remplit un polygone quelconque (triangulé).
fn remplir(painter: &egui::Painter, pts: &[egui::Pos2], couleur: Color32) {
    let mut m = egui::Mesh::default();
    for t in crate::texte::trianguler(vec![pts.iter().map(|p| [p.x, p.y]).collect()]) {
        for p in t {
            m.colored_vertex(egui::pos2(p[0], p[1]), couleur);
        }
    }
    m.indices = (0..m.vertices.len() as u32).collect();
    painter.add(egui::Shape::mesh(m));
}

fn distance_segment(q: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((q - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    q.distance(a + ab * t)
}

fn dans_polygone(q: egui::Pos2, p: &[egui::Pos2]) -> bool {
    let mut dedans = false;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            dedans = !dedans;
        }
    }
    dedans
}

/// Lignes de grille : visibilité commandée par un paramètre oui/non, couleur par un
/// paramètre matériau (pas pour un vide).
fn pilotage_ui(ui: &mut egui::Ui, visible: &mut Option<String>, materiau: &mut Option<String>, vide: bool, parametres: &[Parametre], id: (&str, usize)) {
    let liste = |ui: &mut egui::Ui, choix: &mut Option<String>, genre: GenreParametre, aucun: &str, sel: (&str, &str, usize)| {
        egui::ComboBox::from_id_salt(sel).width(130.0).selected_text(choix.clone().unwrap_or_else(|| aucun.into())).show_ui(ui, |ui| {
            ui.selectable_value(choix, None, aucun);
            for p in parametres.iter().filter(|p| p.genre == genre) {
                ui.selectable_value(choix, Some(p.nom.clone()), &p.nom);
            }
        });
    };
    ui.label("Visible si").on_hover_text("Paramètre oui/non : non masque le volume (un vide masqué ne perce plus)");
    liste(ui, visible, GenreParametre::OuiNon, "toujours", ("visible", id.0, id.1));
    ui.end_row();
    if !vide {
        ui.label("Matériau par").on_hover_text("Paramètre matériau : couleur du matériau du projet");
        liste(ui, materiau, GenreParametre::Materiau, "(rôle)", ("materiau_parametre", id.0, id.1));
        ui.end_row();
    }
}

/// Propriétés de l'extrusion choisie : nom, plein ou vide, matériau, profondeur.
fn extrusion_ui(ui: &mut egui::Ui, e: &mut Extrusion, plans: &[PlanRef], parametres: &[Parametre], k: usize) {
    egui::Grid::new(("extrusion", k)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut e.nom);
        ui.end_row();
        ui.label("Genre");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut e.vide, false, "Plein");
            ui.selectable_value(&mut e.vide, true, "Vide").on_hover_text("Perce le mur hôte");
        });
        ui.end_row();
        if !e.vide {
            ui.label("Matériau");
            egui::ComboBox::from_id_salt(("materiau_extrusion", k)).selected_text(e.materiau.nom()).show_ui(ui, |ui| {
                for m in Materiau::TOUS {
                    ui.selectable_value(&mut e.materiau, m, m.nom());
                }
            });
            ui.end_row();
        }
        pilotage_ui(ui, &mut e.visible, &mut e.materiau_parametre, e.vide, parametres, ("extrusion", k));
        let axe = e.vue.axes().2;
        let [d0, d1] = &mut e.profondeur;
        for (cote, face) in [("début", d0), ("fin", d1)] {
            ui.label(format!("{} {cote}", axe.nom()));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("profondeur", k, cote)).width(120.0).selected_text(face.plan.as_str()).show_ui(ui, |ui| {
                    for p in plans.iter().filter(|p| p.axe == axe) {
                        ui.selectable_value(&mut face.plan, p.nom.clone(), &p.nom);
                    }
                });
                ui.add(egui::DragValue::new(&mut face.decalage).speed(1.0).dimensions());
            });
            ui.end_row();
        }
    });
    ui.label(RichText::new(format!("Profil esquissé en {} : sommets verrouillés sur les plans.", match e.vue { VueEsquisse::Elevation => "élévation", VueEsquisse::Plan => "plan", VueEsquisse::Coupe => "coupe" })).small().weak());
}

/// Propriétés du volume choisi : nom, plein ou vide, matériau, faces verrouillées.
fn volume_ui(ui: &mut egui::Ui, v: &mut Volume, plans: &[PlanRef], parametres: &[Parametre], k: usize) {
    egui::Grid::new(("volume", k)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut v.nom);
        ui.end_row();
        ui.label("Genre");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut v.vide, false, "Plein");
            ui.selectable_value(&mut v.vide, true, "Vide").on_hover_text("Perce le mur hôte");
        });
        ui.end_row();
        if !v.vide {
            ui.label("Matériau");
            egui::ComboBox::from_id_salt(("materiau", k)).selected_text(v.materiau.nom()).show_ui(ui, |ui| {
                for m in Materiau::TOUS {
                    ui.selectable_value(&mut v.materiau, m, m.nom());
                }
            });
            ui.end_row();
        }
        pilotage_ui(ui, &mut v.visible, &mut v.materiau_parametre, v.vide, parametres, ("volume", k));
    });
    ui.label(RichText::new("Faces verrouillées (plan + décalage)").weak());
    egui::Grid::new(("faces", k)).num_columns(3).spacing([6.0, 4.0]).show(ui, |ui| {
        for axe in AxePlan::TOUS {
            let i = axe as usize;
            for (cote, face) in [("min", &mut v.min[i]), ("max", &mut v.max[i])] {
                ui.label(format!("{} {cote}", axe.nom()));
                egui::ComboBox::from_id_salt(("face", k, i, cote)).width(130.0).selected_text(face.plan.as_str()).show_ui(ui, |ui| {
                    for p in plans.iter().filter(|p| p.axe == axe) {
                        ui.selectable_value(&mut face.plan, p.nom.clone(), &p.nom);
                    }
                });
                ui.add(egui::DragValue::new(&mut face.decalage).speed(1.0).dimensions());
                ui.end_row();
            }
        }
    });
}

/// Vue de travail : mur générique, volumes, vides, plans, cotes ; interactions selon
/// l'outil.
fn canevas(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    let taille = ui.available_size().max(egui::vec2(400.0, 400.0));
    let (rect, resp) = ui.allocate_exact_size(taille, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    let r = ed.resoudre();
    let (ah, av) = ed.vue.axes();
    let pos = |nom: &str| r.positions.get(nom).copied().unwrap_or(0.0);
    // cadrage : plans visibles, baie et mur
    let etendue = |axe: AxePlan, defaut: [f64; 2]| {
        ed.famille.plans.iter().filter(|p| p.axe == axe).map(|p| pos(&p.nom)).fold(defaut, |[a, b], x| [a.min(x), b.max(x)])
    };
    let base = |axe: AxePlan| match axe {
        AxePlan::X => [-ed.largeur / 2.0, ed.largeur / 2.0],
        AxePlan::Y => [-ed.epaisseur / 2.0, ed.epaisseur / 2.0],
        AxePlan::Z => [0.0, ed.hauteur],
    };
    let [h0, h1] = etendue(ah, base(ah));
    let [v0, v1] = etendue(av, base(av));
    let marge = (h1 - h0).max(v1 - v0) * 0.25 + 150.0;
    let (h0, h1, v0, v1) = (h0 - marge, h1 + marge, v0 - marge, v1 + marge);
    let echelle = (rect.width() as f64 / (h1 - h0)).min(rect.height() as f64 / (v1 - v0));
    let cadre = Cadre { centre: rect.center(), milieu: ((h0 + h1) / 2.0, (v0 + v1) / 2.0), echelle };
    let boite = |a: (f64, f64), b: (f64, f64)| egui::Rect::from_two_pos(cadre.ecran(a.0, a.1), cadre.ecran(b.0, b.1));
    let (ih, iv) = (ah as usize, av as usize);
    let boites = ed.famille.boites(&r);

    // mur générique : élévation pleine, plan et coupe en bande ; les vides le percent
    let gris = Color32::from_gray(222);
    let (mh, mv) = match ed.vue {
        VueFamille::Elevation => ([h0, h1], [v0, v1]),
        VueFamille::Plan => ([h0, h1], [-ed.epaisseur / 2.0, ed.epaisseur / 2.0]),
        VueFamille::Coupe => ([-ed.epaisseur / 2.0, ed.epaisseur / 2.0], [v0, v1]),
    };
    let mur = boite((mh[0], mv[0]), (mh[1], mv[1]));
    painter.rect_filled(mur, 0.0, gris);
    for b in boites.iter().filter(|b| b.vide && ed.famille.visible(&r, &ed.famille.volumes[b.indice].visible)) {
        let x = boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv])).intersect(mur);
        if x.is_positive() {
            painter.rect_filled(x, 0.0, Color32::WHITE);
        }
    }
    if ed.vue != VueFamille::Elevation {
        painter.rect_stroke(mur, 0.0, egui::Stroke::new(1.5, Color32::from_gray(90)), egui::StrokeKind::Middle);
    }
    let ext = match ed.vue {
        VueFamille::Elevation => None,
        VueFamille::Plan => Some((egui::Align2::CENTER_BOTTOM, cadre.ecran(0.0, ed.epaisseur / 2.0 + marge * 0.6))),
        VueFamille::Coupe => Some((egui::Align2::LEFT_CENTER, cadre.ecran(ed.epaisseur / 2.0 + marge * 0.3, (v0 + v1) / 2.0))),
    };
    if let Some((a, p)) = ext {
        painter.text(p, a, "Extérieur", egui::FontId::proportional(12.0), Color32::from_gray(110));
    }

    // volumes pleins (du plus loin au plus proche), puis contours des vides
    let profondeur = 3 - ih - iv;
    let mut pleins: Vec<_> = boites.iter().filter(|b| !b.vide).collect();
    pleins.sort_by(|a, b| a.min[profondeur].total_cmp(&b.min[profondeur]));
    if ed.vue == VueFamille::Elevation {
        // vu de l'intérieur (y négatif) : le plus proche a le y le plus petit
        pleins.reverse();
    }
    let encre = egui::Stroke::new(1.0, Color32::from_gray(50));
    let orange = Color32::from_rgb(230, 120, 20);
    for b in &pleins {
        let choisi = ed.selection == SelectionFamille::Volume(b.indice);
        // masqué par son paramètre oui/non : estompé
        let vol = &ed.famille.volumes[b.indice];
        let mut couleur = ed.couleur_solide(&r, b.materiau, &vol.materiau_parametre);
        if !ed.famille.visible(&r, &vol.visible) {
            couleur = couleur.gamma_multiply(0.25);
        }
        painter.rect(boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv])), 0.0, couleur, if choisi { egui::Stroke::new(2.5, orange) } else { encre }, egui::StrokeKind::Inside);
    }
    for b in boites.iter().filter(|b| b.vide) {
        let r = boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv]));
        let choisi = ed.selection == SelectionFamille::Volume(b.indice);
        let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        painter.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(if choisi { 2.5 } else { 1.5 }, orange), 7.0, 4.0));
    }

    // extrusions : profil dans leur vue d'esquisse, emprise ailleurs
    let solides = ed.famille.solides(&r);
    let decalage_ext = ed.famille.volumes.len();
    let mut contours_ext: Vec<(usize, Vec<egui::Pos2>)> = Vec::new();
    for (k, e) in ed.famille.extrusions.iter().enumerate() {
        let pts: Vec<egui::Pos2> = if e.vue == ed.vue.esquisse() {
            e.profil.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(&r, p); cadre.ecran(h, v) }).collect()
        } else {
            let Some(s) = solides.get(decalage_ext + k) else { continue };
            let (mut a, mut b) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
            for q in s.tris.iter().flatten() {
                let q = q.to_array();
                a = (a.0.min(q[ih]), a.1.min(q[iv]));
                b = (b.0.max(q[ih]), b.1.max(q[iv]));
            }
            if !a.0.is_finite() {
                continue;
            }
            let rr = boite(a, b);
            vec![rr.left_top(), rr.right_top(), rr.right_bottom(), rr.left_bottom()]
        };
        if pts.len() < 3 {
            continue;
        }
        let choisi = ed.selection == SelectionFamille::Extrusion(k);
        let mut ferme = pts.clone();
        ferme.push(pts[0]);
        if e.vide {
            if ed.famille.visible(&r, &e.visible) {
                remplir(&painter, &pts, Color32::WHITE);
            }
            painter.add(egui::Shape::dashed_line(&ferme, egui::Stroke::new(if choisi { 2.5 } else { 1.5 }, orange), 7.0, 4.0));
        } else {
            let mut couleur = ed.couleur_solide(&r, e.materiau, &e.materiau_parametre);
            if !ed.famille.visible(&r, &e.visible) {
                couleur = couleur.gamma_multiply(0.25);
            }
            remplir(&painter, &pts, couleur);
            painter.add(egui::Shape::line(ferme, if choisi { egui::Stroke::new(2.5, orange) } else { encre }));
        }
        contours_ext.push((k, pts));
    }

    // symboles 2D de la vue
    let marine = Color32::from_rgb(20, 40, 120);
    let mut lignes_symboles: Vec<(usize, Vec<egui::Pos2>)> = Vec::new();
    for (k, sy) in ed.famille.symboles.iter().enumerate() {
        if sy.vue != ed.vue.esquisse() {
            continue;
        }
        let mut seul = ed.famille.clone();
        seul.symboles = vec![sy.clone()];
        for l in seul.symboles_resolus(&r, sy.vue) {
            let pts: Vec<egui::Pos2> = l.into_iter().map(|(h, v)| cadre.ecran(h, v)).collect();
            let choisi = ed.selection == SelectionFamille::Symbole(k);
            painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(if choisi { 2.5 } else { 1.4 }, if choisi { orange } else { marine })));
            lignes_symboles.push((k, pts));
        }
    }

    // plans de référence
    let vert = Color32::from_rgb(40, 150, 90);
    let bleu = Color32::from_rgb(30, 90, 200);
    let police = egui::FontId::proportional(11.0);
    let mut lignes: Vec<(String, bool, f32)> = Vec::new(); // (nom, vertical, coordonnée écran)
    let (mut rang_pv, mut rang_ph) = (0usize, 0usize);
    for p in ed.famille.plans.iter().filter(|p| p.axe == ah || p.axe == av) {
        let vertical = p.axe == ah;
        let quinconce = if vertical { rang_pv += 1; (rang_pv % 3) as f32 * 13.0 } else { rang_ph += 1; (rang_ph % 2) as f32 * 90.0 };
        let x = pos(&p.nom);
        let choisi = ed.selection == SelectionFamille::Plan(p.nom.clone()) || ed.clics.contains(&p.nom);
        let couleur = if choisi { bleu } else if p.hote.is_some() { Color32::from_rgb(140, 90, 40) } else { vert };
        let trait_ = egui::Stroke::new(if choisi { 1.8 } else { 1.0 }, couleur);
        let (a, b, etiquette) = if vertical {
            let e = cadre.ecran(x, 0.0).x;
            (egui::pos2(e, rect.top()), egui::pos2(e, rect.bottom()), egui::pos2(e + 3.0, rect.bottom() - 4.0 - quinconce))
        } else {
            let e = cadre.ecran(0.0, x).y;
            (egui::pos2(rect.left(), e), egui::pos2(rect.right(), e), egui::pos2(rect.right() - 4.0 - quinconce, e - 2.0))
        };
        painter.add(egui::Shape::dashed_line(&[a, b], trait_, 8.0, 3.0));
        let ancrage = if vertical { egui::Align2::LEFT_BOTTOM } else { egui::Align2::RIGHT_BOTTOM };
        let nom = if p.fixe { format!("{} (fixe)", p.nom) } else { p.nom.clone() };
        painter.text(etiquette, ancrage, nom, police.clone(), couleur);
        lignes.push((p.nom.clone(), vertical, if vertical { a.x } else { a.y }));
    }

    // cotes et égalités dont les plans sont visibles ; étiquettes cliquables
    let mut etiquettes: Vec<(usize, egui::Rect)> = Vec::new();
    let (mut rang_h, mut rang_v) = (0, 0);
    for (k, c) in ed.famille.contraintes.iter().enumerate() {
        let noms = c.plans();
        let Some(axe) = ed.famille.plan(noms[0]).map(|p| p.axe) else { continue };
        if axe != ah && axe != av {
            continue;
        }
        let horizontale = axe == ah;
        let conflit = r.conflits.contains(&k);
        let choisie = ed.selection == SelectionFamille::Contrainte(k);
        let couleur = if conflit { Color32::from_rgb(210, 40, 40) } else if choisie { bleu } else { Color32::from_gray(40) };
        let (rang, base) = if horizontale {
            rang_h += 1;
            (rang_h, rect.top() + 14.0)
        } else {
            rang_v += 1;
            (rang_v, rect.left() + 14.0)
        };
        let decal = base + 18.0 * (rang - 1) as f32;
        let coord = |nom: &str| if horizontale { cadre.ecran(pos(nom), 0.0).x } else { cadre.ecran(0.0, pos(nom)).y };
        let segment = |a: f32, b: f32| if horizontale { [egui::pos2(a, decal), egui::pos2(b, decal)] } else { [egui::pos2(decal, a), egui::pos2(decal, b)] };
        let xs: Vec<f32> = noms.iter().map(|n| coord(n)).collect();
        let trait_ = egui::Stroke::new(1.0, couleur);
        for w in xs.windows(2) {
            painter.line_segment(segment(w[0], w[1]), trait_);
        }
        for &x in &xs {
            let [p, _] = segment(x, x);
            let t = if horizontale { egui::vec2(0.0, 5.0) } else { egui::vec2(5.0, 0.0) };
            painter.line_segment([p - t, p + t], trait_);
        }
        let texte = match c {
            Contrainte::Distance { valeur, .. } => {
                let v = pos(noms[1]) - pos(noms[0]);
                if valeur.trim().parse::<f64>().is_ok() { crate::options::unite().afficher(v) } else { format!("{valeur} = {}", crate::options::unite().afficher(v)) }
            }
            Contrainte::Egalite { .. } => "EQ".into(),
        };
        let milieu = (xs[0] + xs[xs.len() - 1]) / 2.0;
        let [p, _] = segment(milieu, milieu);
        let galee = painter.layout_no_wrap(texte, police.clone(), couleur);
        let decalage_texte = if horizontale { egui::vec2(0.0, -8.0) } else { egui::vec2(10.0 + galee.size().x / 2.0, 0.0) };
        let cadre_txt = egui::Rect::from_center_size(p + decalage_texte, galee.size() + egui::vec2(6.0, 2.0));
        painter.rect_filled(cadre_txt, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
        painter.galley(cadre_txt.min + egui::vec2(3.0, 1.0), galee, couleur);
        etiquettes.push((k, cadre_txt));
    }

    // interactions
    let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
    // Échap : abandonne l'esquisse, la cote ou le rectangle en cours
    if (resp.hovered() || !ed.esquisse.is_empty() || !ed.clics.is_empty()) && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        ed.esquisse.clear();
        ed.clics.clear();
        ed.debut = None;
    }
    let plan_vise = |q: egui::Pos2| {
        lignes
            .iter()
            .map(|(n, vertical, c)| (n.clone(), if *vertical { (q.x - c).abs() } else { (q.y - c).abs() }))
            .filter(|x| x.1 < 7.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    };
    let tol = 7.0 / echelle;
    match ed.outil {
        OutilFamille::Selection => {
            // poignées de l'élément choisi : sommets et milieux d'arêtes (insertion) d'un
            // profil ou d'un symbole de la vue, milieux des faces d'un volume
            let vue = ed.vue.esquisse();
            let mut poignees: Vec<(Poignee, egui::Pos2, bool)> = Vec::new(); // (poignée, position, milieu à insérer)
            let ecran_pt = |p: &PointEsquisse| { let (h, v) = ed.famille.point_esquisse(&r, p); cadre.ecran(h, v) };
            match ed.selection.clone() {
                SelectionFamille::Extrusion(k) if ed.famille.extrusions.get(k).is_some_and(|e| e.vue == vue) => {
                    let pts: Vec<egui::Pos2> = ed.famille.extrusions[k].profil.iter().map(ecran_pt).collect();
                    for (i, &q) in pts.iter().enumerate() {
                        poignees.push((Poignee::Sommet { extrusion: true, k, i }, q, false));
                        poignees.push((Poignee::Sommet { extrusion: true, k, i: i + 1 }, q.lerp(pts[(i + 1) % pts.len()], 0.5), true));
                    }
                }
                SelectionFamille::Symbole(k) if ed.famille.symboles.get(k).is_some_and(|s| s.vue == vue) => {
                    let sy = &ed.famille.symboles[k];
                    let pts: Vec<egui::Pos2> = sy.points.iter().map(ecran_pt).collect();
                    for (i, &q) in pts.iter().enumerate() {
                        poignees.push((Poignee::Sommet { extrusion: false, k, i }, q, false));
                        if !sy.arc && i + 1 < pts.len() {
                            poignees.push((Poignee::Sommet { extrusion: false, k, i: i + 1 }, q.lerp(pts[i + 1], 0.5), true));
                        }
                    }
                }
                SelectionFamille::Volume(k) => {
                    if let Some(b) = boites.iter().find(|b| b.indice == k) {
                        let (mh, mv) = ((b.min[ih] + b.max[ih]) / 2.0, (b.min[iv] + b.max[iv]) / 2.0);
                        poignees.push((Poignee::Face { k, axe: ih, max: false }, cadre.ecran(b.min[ih], mv), false));
                        poignees.push((Poignee::Face { k, axe: ih, max: true }, cadre.ecran(b.max[ih], mv), false));
                        poignees.push((Poignee::Face { k, axe: iv, max: false }, cadre.ecran(mh, b.min[iv]), false));
                        poignees.push((Poignee::Face { k, axe: iv, max: true }, cadre.ecran(mh, b.max[iv]), false));
                    }
                }
                _ => {}
            }
            for (_, q, milieu) in &poignees {
                if *milieu {
                    painter.circle(*q, 3.5, Color32::WHITE, egui::Stroke::new(1.2, bleu));
                } else {
                    painter.rect(egui::Rect::from_center_size(*q, egui::vec2(8.0, 8.0)), 0.0, Color32::WHITE, egui::Stroke::new(1.5, bleu), egui::StrokeKind::Middle);
                }
            }
            let visee = |q: egui::Pos2| poignees.iter().filter(|p| p.1.distance(q) < 7.0).min_by(|a, b| a.1.distance(q).total_cmp(&b.1.distance(q))).copied();
            // clic droit sur un sommet : le retirer (au moins 3 sommets pour un profil, 2
            // pour une ligne, l'arc garde ses 3 points)
            if resp.clicked_by(PointerButton::Secondary) {
                if let Some((Poignee::Sommet { extrusion, k, i }, _, false)) = resp.interact_pointer_pos().and_then(visee) {
                    if extrusion && ed.famille.extrusions[k].profil.len() > 3 {
                        ed.famille.extrusions[k].profil.remove(i);
                    } else if !extrusion && !ed.famille.symboles[k].arc && ed.famille.symboles[k].points.len() > 2 {
                        ed.famille.symboles[k].points.remove(i);
                    } else {
                        ed.message = Some("Sommet conservé : nombre minimal atteint.".into());
                    }
                }
            }
            // point d'appui du glissement (egui signale le glissement après quelques pixels)
            let appui = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            if resp.drag_started() {
                ed.poignee = None;
                if let Some((p, _, milieu)) = appui.and_then(visee) {
                    // milieu d'arête : un nouveau sommet est inséré puis déplacé
                    if let (true, Poignee::Sommet { extrusion, k, i }) = (milieu, p) {
                        let points = if extrusion { &mut ed.famille.extrusions[k].profil } else { &mut ed.famille.symboles[k].points };
                        let copie = points[i - 1].clone();
                        points.insert(i, copie);
                    }
                    ed.poignee = Some(p);
                }
            }
            if let (Some(p), Some(q)) = (ed.poignee, resp.interact_pointer_pos().filter(|_| resp.dragged())) {
                let (h, v) = cadre.modele(q);
                match p {
                    Poignee::Sommet { extrusion, k, i } => {
                        if let (Some(fh), Some(fv)) = (ed.accrocher(&r, ah, h, tol), ed.accrocher(&r, av, v, tol)) {
                            let points = if extrusion { &mut ed.famille.extrusions[k].profil } else { &mut ed.famille.symboles[k].points };
                            if let Some(pt) = points.get_mut(i) {
                                *pt = PointEsquisse { h: fh, v: fv };
                            }
                        }
                    }
                    Poignee::Face { k, axe, max } => {
                        let x = if axe == ih { h } else { v };
                        if let Some(f) = ed.accrocher(&r, AxePlan::TOUS[axe], x, tol) {
                            let vol = &mut ed.famille.volumes[k];
                            if max { vol.max[axe] = f } else { vol.min[axe] = f }
                        }
                    }
                }
                ed.message = Some("Poignée : accrochée au plan le plus proche (décalage au-delà de 7 px) ; clic droit sur un sommet pour le retirer.".into());
            }
            if resp.drag_stopped() {
                ed.poignee = None;
            }
            if resp.drag_started() && ed.poignee.is_none() {
                ed.deplace = appui.and_then(plan_vise);
            }
            if let (Some(nom), Some(q)) = (ed.deplace.clone(), resp.interact_pointer_pos().filter(|_| resp.dragged())) {
                let (h, v) = cadre.modele(q);
                let contraint = ed.famille.contraintes.iter().any(|c| c.plans().contains(&nom.as_str()));
                if let Some(p) = ed.famille.plans.iter_mut().find(|p| p.nom == nom) {
                    if p.fixe || p.hote.is_some() {
                        ed.message = Some(format!("« {nom} » est fixe."));
                    } else {
                        p.position = (if p.axe == ah { h } else { v }).round();
                        ed.message = contraint.then(|| format!("« {nom} » suit ses contraintes : modifier la cote, ou la supprimer pour le déplacer librement."));
                    }
                }
                ed.selection = SelectionFamille::Plan(nom);
            }
            if resp.drag_stopped() {
                ed.deplace = None;
            }
            if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let (h, v) = cadre.modele(q);
                    ed.selection = if let Some((k, _)) = etiquettes.iter().find(|(_, r)| r.contains(q)) {
                        SelectionFamille::Contrainte(*k)
                    } else if let Some((k, _)) = lignes_symboles.iter().find(|(_, l)| l.windows(2).any(|w| distance_segment(q, w[0], w[1]) < 6.0)) {
                        SelectionFamille::Symbole(*k)
                    } else if let Some((k, _)) = contours_ext.iter().rev().find(|(_, p)| dans_polygone(q, p)) {
                        // un objet l'emporte sur les plans qui le traversent (les plans
                        // courent sur toute la vue : on les choisit hors des objets)
                        SelectionFamille::Extrusion(*k)
                    } else if let Some(b) = pleins.iter().rev().copied().chain(boites.iter().filter(|b| b.vide)).find(|b| (b.min[ih]..=b.max[ih]).contains(&h) && (b.min[iv]..=b.max[iv]).contains(&v)) {
                        SelectionFamille::Volume(b.indice)
                    } else if let Some(n) = plan_vise(q) {
                        SelectionFamille::Plan(n)
                    } else {
                        SelectionFamille::Aucune
                    };
                }
            }
        }
        OutilFamille::PlanVertical | OutilFamille::PlanHorizontal => {
            let vertical = ed.outil == OutilFamille::PlanVertical;
            if let Some(q) = curseur.filter(|_| resp.hovered()) {
                let trait_ = egui::Stroke::new(1.0, vert.gamma_multiply(0.6));
                if vertical {
                    painter.line_segment([egui::pos2(q.x, rect.top()), egui::pos2(q.x, rect.bottom())], trait_);
                } else {
                    painter.line_segment([egui::pos2(rect.left(), q.y), egui::pos2(rect.right(), q.y)], trait_);
                }
            }
            if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let (h, v) = cadre.modele(q);
                    let axe = if vertical { ah } else { av };
                    let nom = nom_libre(|n| ed.famille.plan(n).is_some(), "Plan");
                    ed.famille.plans.push(PlanRef { nom: nom.clone(), axe, position: (if vertical { h } else { v }).round(), fixe: false, hote: None });
                    ed.selection = SelectionFamille::Plan(nom);
                }
            }
        }
        OutilFamille::Volume | OutilFamille::Vide => {
            let vide = ed.outil == OutilFamille::Vide;
            if resp.drag_started() {
                ed.debut = resp.interact_pointer_pos().map(|q| cadre.modele(q));
            }
            if let (Some(a), Some(q)) = (ed.debut, curseur) {
                let r = boite(a, cadre.modele(q));
                if vide {
                    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                    painter.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(1.5, orange), 7.0, 4.0));
                } else {
                    painter.rect(r, 0.0, couleur_materiau(&ed.famille, Materiau::Menuiserie).gamma_multiply(0.6), egui::Stroke::new(1.5, orange), egui::StrokeKind::Inside);
                }
            }
            if resp.drag_stopped() {
                if let (Some(a), Some(q)) = (ed.debut.take(), resp.interact_pointer_pos()) {
                    let b = cadre.modele(q);
                    let accroche = |axe: AxePlan, x: f64| match ed.plan_proche(&r, axe, x) {
                        Some((n, d)) if d.abs() <= tol => Face { plan: n, decalage: 0.0 },
                        Some((n, d)) => Face { plan: n, decalage: d.round() },
                        None => Face { plan: String::new(), decalage: 0.0 },
                    };
                    let axe_p = AxePlan::TOUS[profondeur];
                    let [p0, p1] = ed.faces_profondeur(&r, axe_p, vide);
                    let mut min = [p0.clone(), p0.clone(), p0];
                    let mut max = [p1.clone(), p1.clone(), p1];
                    min[ih] = accroche(ah, a.0.min(b.0));
                    max[ih] = accroche(ah, a.0.max(b.0));
                    min[iv] = accroche(av, a.1.min(b.1));
                    max[iv] = accroche(av, a.1.max(b.1));
                    if (a.0 - b.0).abs() * echelle > 4.0 && (a.1 - b.1).abs() * echelle > 4.0 && min.iter().chain(&max).all(|f| !f.plan.is_empty()) {
                        let nom = nom_libre(|n| ed.famille.volumes.iter().any(|v| v.nom == n), if vide { "Vide" } else { "Volume" });
                        ed.famille.volumes.push(Volume { nom, vide, materiau: Materiau::Menuiserie, min, max, visible: None, materiau_parametre: None });
                        ed.selection = SelectionFamille::Volume(ed.famille.volumes.len() - 1);
                        ed.message = Some("Faces verrouillées sur les plans les plus proches (avec décalage au-delà de 7 px).".into());
                    }
                }
            }
        }
        OutilFamille::Extrusion | OutilFamille::ExtrusionVide | OutilFamille::Ligne | OutilFamille::Arc => {
            // sommet accroché : sur chaque axe, le plan le plus proche (décalage au-delà
            // de 7 px)
            let accroche = |axe: AxePlan, x: f64| match ed.plan_proche(&r, axe, x) {
                Some((n, d)) if d.abs() <= tol => (Face { plan: n, decalage: 0.0 }, x - d),
                Some((n, d)) => (Face { plan: n, decalage: d.round() }, x - d + d.round()),
                None => (Face { plan: String::new(), decalage: 0.0 }, x),
            };
            let point = |q: egui::Pos2| {
                let (h, v) = cadre.modele(q);
                let ((fh, h), (fv, v)) = (accroche(ah, h), accroche(av, v));
                (PointEsquisse { h: fh, v: fv }, (h, v))
            };
            let profil = matches!(ed.outil, OutilFamille::Extrusion | OutilFamille::ExtrusionVide);
            let premier = ed.esquisse.first().map(|(_, p)| cadre.ecran(p.0, p.1));
            // élastique et repère d'accrochage
            if let Some(q) = curseur.filter(|_| resp.hovered()) {
                let (_, c) = point(q);
                let c = cadre.ecran(c.0, c.1);
                let mut pts: Vec<egui::Pos2> = ed.esquisse.iter().map(|(_, p)| cadre.ecran(p.0, p.1)).collect();
                pts.push(c);
                painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, orange)));
                painter.circle_stroke(c, 4.0, egui::Stroke::new(1.5, orange));
                if let Some(p0) = premier.filter(|p| profil && ed.esquisse.len() >= 3 && p.distance(q) < 9.0) {
                    painter.circle_stroke(p0, 7.0, egui::Stroke::new(2.0, Color32::from_rgb(30, 140, 60)));
                }
            }
            let mut finir = resp.clicked_by(PointerButton::Secondary);
            if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    if profil && ed.esquisse.len() >= 3 && premier.is_some_and(|p| p.distance(q) < 9.0) {
                        finir = true;
                    } else {
                        let p = point(q);
                        if p.0.h.plan.is_empty() || p.0.v.plan.is_empty() {
                            ed.message = Some("Ajouter d'abord des plans de référence dans cette vue.".into());
                        } else {
                            ed.esquisse.push(p);
                        }
                    }
                }
                if ed.outil == OutilFamille::Arc && ed.esquisse.len() == 3 {
                    finir = true;
                }
            }
            if finir {
                let points: Vec<PointEsquisse> = std::mem::take(&mut ed.esquisse).into_iter().map(|(p, _)| p).collect();
                let vue = ed.vue.esquisse();
                match ed.outil {
                    OutilFamille::Extrusion | OutilFamille::ExtrusionVide if points.len() >= 3 => {
                        let vide = ed.outil == OutilFamille::ExtrusionVide;
                        let profondeur = ed.faces_profondeur(&r, vue.axes().2, vide);
                        let nom = nom_libre(|n| ed.famille.extrusions.iter().any(|e| e.nom == n), if vide { "Extrusion vide" } else { "Extrusion" });
                        ed.famille.extrusions.push(Extrusion { nom, vide, materiau: Materiau::Menuiserie, vue, profil: points, profondeur, visible: None, materiau_parametre: None });
                        ed.selection = SelectionFamille::Extrusion(ed.famille.extrusions.len() - 1);
                        ed.message = Some("Extrusion créée : régler sa profondeur dans Extrusions.".into());
                    }
                    OutilFamille::Ligne if points.len() >= 2 => {
                        ed.famille.symboles.push(Symbole { vue, points, arc: false });
                        ed.selection = SelectionFamille::Symbole(ed.famille.symboles.len() - 1);
                    }
                    OutilFamille::Arc if points.len() == 3 => {
                        ed.famille.symboles.push(Symbole { vue, points, arc: true });
                        ed.selection = SelectionFamille::Symbole(ed.famille.symboles.len() - 1);
                    }
                    _ => ed.message = Some("Esquisse abandonnée : pas assez de points.".into()),
                }
            }
        }
        OutilFamille::Cote | OutilFamille::Egalite => {
            let n = if ed.outil == OutilFamille::Cote { 2 } else { 3 };
            if resp.clicked() {
                if let Some(nom) = resp.interact_pointer_pos().and_then(plan_vise) {
                    let axe = ed.famille.plan(&nom).map(|p| p.axe);
                    if ed.clics.first().and_then(|p| ed.famille.plan(p)).is_some_and(|p| Some(p.axe) != axe) {
                        ed.message = Some("Choisir des plans parallèles.".into());
                    } else if !ed.clics.contains(&nom) {
                        ed.clics.push(nom);
                    }
                }
                if ed.clics.len() == n {
                    let p = std::mem::take(&mut ed.clics);
                    let c = if n == 2 {
                        // sens croissant : la valeur est positive
                        let (a, b) = if pos(&p[0]) <= pos(&p[1]) { (p[0].clone(), p[1].clone()) } else { (p[1].clone(), p[0].clone()) };
                        let v = (pos(&b) - pos(&a)).round();
                        Contrainte::Distance { a, b, valeur: format!("{v}"), priorite: 5 }
                    } else {
                        Contrainte::Egalite { a: p[0].clone(), m: p[1].clone(), b: p[2].clone(), priorite: 5 }
                    };
                    ed.famille.contraintes.push(c);
                    ed.selection = SelectionFamille::Contrainte(ed.famille.contraintes.len() - 1);
                    ed.message = Some(if n == 2 { "Cote créée : la libeller par un paramètre dans Contraintes." } else { "Égalité créée." }.into());
                } else {
                    ed.message = Some(format!("{} plan(s) sur {n}.", ed.clics.len()));
                }
            }
        }
    }
    if resp.hovered() && ed.outil != OutilFamille::Selection {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}
