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
use crate::modele::famille_modelee::{AxePlan, Contrainte, Face, FamilleModelee, Hote, Materiau, Parametre, PlanRef, Resolution, Volume, EPAISSEUR, HAUTEUR, LARGEUR};

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
}

#[derive(Clone, PartialEq)]
enum SelectionFamille {
    Aucune,
    Plan(String),
    Volume(usize),
    Contrainte(usize),
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
    fn resoudre(&self) -> Resolution {
        self.famille.resoudre(self.largeur, self.hauteur, self.epaisseur)
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
        let (l, h, id) = (o.largeur, o.hauteur, o.id.clone());
        self.editeur_famille(t, Some(id), l, h, e);
    }

    /// Éditeur sur le type `t` (None : nouveau type), hors de toute fenêtre placée.
    pub(super) fn ouvrir_editeur_famille_type(&mut self, t: Option<String>) {
        self.editeur_famille(t, None, 1200.0, 1350.0, 300.0);
    }

    fn editeur_famille(&mut self, t: Option<String>, ouverture: Option<String>, largeur: f64, hauteur: f64, epaisseur: f64) {
        let Some((p, _)) = &self.natif else { return };
        let t = t.and_then(|id| p.types_ouvertures.iter().find(|x| x.id == id));
        let u = crate::options::unite();
        let (largeur, hauteur) = t.map_or((largeur, hauteur), |t| (t.largeur, t.hauteur));
        self.famille = Some(EditeurFamille {
            ouverture,
            type_id: t.map(|t| t.id.clone()),
            nom: t.map_or_else(|| format!("Fenêtre {} × {}", u.afficher(largeur), u.afficher(hauteur)), |t| t.nom.clone()),
            largeur,
            hauteur,
            epaisseur,
            famille: t.and_then(|t| t.modele.clone()).unwrap_or_else(FamilleModelee::gabarit_fenetre),
            vue: VueFamille::Elevation,
            outil: OutilFamille::Selection,
            selection: SelectionFamille::Aucune,
            clics: Vec::new(),
            debut: None,
            deplace: None,
            message: None,
            erreur: None,
        });
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
                (t.nom, t.largeur, t.hauteur, t.modele) = (ed.nom.trim().into(), ed.largeur, ed.hauteur, Some(ed.famille.clone()));
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
                q.types_ouvertures.push(crate::modele::TypeOuverture {
                    id: id.clone(),
                    nom: ed.nom.trim().into(),
                    genre: crate::modele::GenreOuverture::Fenetre,
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
                (o.largeur, o.hauteur) = (ed.largeur, ed.hauteur);
            }
        }
        self.appliquer_projet(q)?;
        if let Some(ed) = self.famille.as_mut() {
            ed.type_id = Some(id.clone());
        }
        self.status = "Famille chargée dans le projet — Ctrl+S pour enregistrer.".into();
        Ok(id)
    }

    /// Fenêtre de l'éditeur de familles.
    pub(super) fn fenetre_famille(&mut self, ctx: &egui::Context) {
        if self.famille.is_none() {
            return;
        }
        let mut ouverte = true;
        let mut action: Option<bool> = None;
        let placement = self.famille.as_ref().is_some_and(|e| e.ouverture.is_none());
        egui::Window::new("Éditeur de familles — Fenêtre")
            .id(egui::Id::unique("editeur_famille"))
            .open(&mut ouverte)
            .collapsible(false)
            .default_size([1180.0, 720.0])
            .min_size([900.0, 560.0])
            .show(ctx, |ui| {
                let ed = self.famille.as_mut().unwrap();
                barre_outils(ui, ed);
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(360.0);
                        egui::ScrollArea::vertical().id_salt("famille_panneau").max_height((ui.available_height() - 70.0).max(200.0)).show(ui, |ui| panneau(ui, ed));
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
                        ui.horizontal(|ui| {
                            if ui.add_enabled(ed.type_id.is_some(), egui::Button::new("Enregistrer le type")).on_hover_text("Met à jour le type et toutes ses fenêtres").clicked() {
                                action = Some(false);
                            }
                            let aide = if placement { "Crée un type avec cette famille ; il sert au prochain placement" } else { "Crée un type avec cette famille et l'affecte à la fenêtre" };
                            if ui.button("Nouveau type").on_hover_text(aide).clicked() {
                                action = Some(true);
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| canevas(ui, ed));
                });
            });
        if !ouverte {
            self.famille = None;
            return;
        }
        if let Some(nouveau) = action {
            let r = self.enregistrer_famille(nouveau);
            if let Ok(id) = &r {
                if placement {
                    self.ruban_edition.ouverture_type_identique = Some(id.clone());
                }
            }
            if let Some(ed) = self.famille.as_mut() {
                ed.erreur = r.err();
            }
        }
    }
}

fn barre_outils(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    ui.horizontal(|ui| {
        for v in [VueFamille::Elevation, VueFamille::Plan, VueFamille::Coupe] {
            if ui.selectable_label(ed.vue == v, v.nom()).clicked() {
                ed.vue = v;
                ed.debut = None;
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
        ] {
            if ui.selectable_label(ed.outil == o, nom).on_hover_text(aide).clicked() {
                ed.outil = o;
                ed.clics.clear();
                ed.debut = None;
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
        egui::Grid::new("famille_parametres").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            for (k, p) in ed.famille.parametres.iter_mut().enumerate() {
                ui.add_sized([96.0, 20.0], egui::TextEdit::singleline(&mut p.nom));
                ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut p.formule)).on_hover_text("Formule : nombres (mm), paramètres, + - * / ( )");
                match r.valeurs.get(&p.nom) {
                    Some(v) => ui.label(RichText::new(u.afficher(*v)).weak()),
                    None => ui.colored_label(Color32::from_rgb(200, 40, 40), "?"),
                };
                if ui.small_button("−").on_hover_text("Supprimer le paramètre").clicked() {
                    supprimer = Some(k);
                }
                ui.end_row();
            }
        });
        if let Some(k) = supprimer {
            ed.famille.parametres.remove(k);
        }
        if ui.button("Ajouter un paramètre").clicked() {
            let nom = (1..).map(|k| format!("Parametre{k}")).find(|n| ed.famille.parametres.iter().all(|p| &p.nom != n)).unwrap();
            ed.famille.parametres.push(Parametre { nom, formule: "100".into() });
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
                volume_ui(ui, v, &ed.famille.plans, k);
            }
        }
    });
}

/// Propriétés du volume choisi : nom, plein ou vide, matériau, faces verrouillées.
fn volume_ui(ui: &mut egui::Ui, v: &mut Volume, plans: &[PlanRef], k: usize) {
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
    for b in boites.iter().filter(|b| b.vide) {
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
        painter.rect(boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv])), 0.0, couleur_materiau(&ed.famille, b.materiau), if choisi { egui::Stroke::new(2.5, orange) } else { encre }, egui::StrokeKind::Inside);
    }
    for b in boites.iter().filter(|b| b.vide) {
        let r = boite((b.min[ih], b.min[iv]), (b.max[ih], b.max[iv]));
        let choisi = ed.selection == SelectionFamille::Volume(b.indice);
        let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        painter.add(egui::Shape::dashed_line(&pts, egui::Stroke::new(if choisi { 2.5 } else { 1.5 }, orange), 7.0, 4.0));
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
            if resp.drag_started() {
                ed.deplace = resp.interact_pointer_pos().and_then(plan_vise);
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
                    } else if let Some(n) = plan_vise(q) {
                        SelectionFamille::Plan(n)
                    } else if let Some(b) = pleins.iter().rev().copied().chain(boites.iter().filter(|b| b.vide)).find(|b| (b.min[ih]..=b.max[ih]).contains(&h) && (b.min[iv]..=b.max[iv]).contains(&v)) {
                        SelectionFamille::Volume(b.indice)
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
                        ed.famille.volumes.push(Volume { nom, vide, materiau: Materiau::Menuiserie, min, max });
                        ed.selection = SelectionFamille::Volume(ed.famille.volumes.len() - 1);
                        ed.message = Some("Faces verrouillées sur les plans les plus proches (avec décalage au-delà de 7 px).".into());
                    }
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
