//! Formes guidées dans l'éditeur de familles : tracé graphique du chemin dans la vue
//! (outil « Tracer le chemin ») et propriétés communes (erreurs, profil d'arrivée
//! piloté par les plans).
//!
//! Le chemin est tracé dans le plan de la vue active : le premier clic est l'origine de
//! la section, l'horizontale de la vue donne H et sa verticale V (D = 0). Clics
//! accrochés comme les autres outils ; A : le prochain clic est le point de passage
//! d'un arc par trois points ; clic sur le premier point : chemin fermé (extrusion par
//! chemin) ; Entrée ou clic droit : terminer ; Retour arrière : retirer le dernier
//! point ; Échap : abandonner. Le chemin tracé remplace celui de la forme choisie.
use super::*;
use modele::famille::modelee::FormeGuidee;

/// Tracé du chemin en cours, en coordonnées de la vue.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TraceChemin {
    pub points: Vec<(f64, f64)>,
    /// Indices des points de passage d'arcs.
    pub passages: Vec<usize>,
    /// Le prochain point est un point de passage d'arc.
    pub arc: bool,
}

impl TraceChemin {
    /// Ajoute un point ; le point qui suit un passage d'arc termine l'arc.
    pub(crate) fn ajouter(&mut self, p: (f64, f64)) {
        if self.arc && !self.points.is_empty() && !self.passages.contains(&(self.points.len() - 1)) {
            self.passages.push(self.points.len());
        }
        self.arc = false;
        self.points.push(p);
    }

    pub(crate) fn retirer(&mut self) {
        self.points.pop();
        let n = self.points.len();
        self.passages.retain(|&i| i < n);
    }

    /// Le dernier point attend la fin de son arc.
    pub(crate) fn arc_ouvert(&self) -> bool {
        self.points.len().checked_sub(1).is_some_and(|i| self.passages.contains(&i))
    }

    /// Chemin local (origine au premier point), arcs et fermeture.
    pub(crate) fn chemin(&self) -> Vec<[f64; 3]> {
        let o = self.points.first().copied().unwrap_or_default();
        self.points.iter().map(|p| [p.0 - o.0, p.1 - o.1, 0.0]).collect()
    }

    /// Polyligne d'aperçu dans la vue (arcs discrétisés), jusqu'au point `curseur`.
    pub(crate) fn apercu(&self, curseur: Option<(f64, f64)>, ferme: bool) -> Vec<(f64, f64)> {
        let mut t = self.clone();
        if let Some(c) = curseur { t.ajouter(c); }
        let o = t.points.first().copied().unwrap_or_default();
        let passages: Vec<usize> = t.passages.iter().copied().filter(|&i| i + 1 < t.points.len() || ferme).collect();
        match FormeGuidee::points_chemin(&t.chemin(), &passages, ferme && t.points.len() >= 3) {
            Ok(mut pts) => {
                if ferme && let Some(p) = pts.first().copied() { pts.push(p); }
                pts.into_iter().map(|p| (p[0] + o.0, p[1] + o.1)).collect()
            }
            Err(_) => t.points,
        }
    }
}

impl EditeurFamille {
    /// Forme guidée par chemin actuellement choisie.
    fn forme_chemin(&self) -> Option<usize> {
        match self.selection {
            SelectionFamille::Extrusion(k) => self.famille.extrusions.get(k)
                .filter(|e| matches!(&e.operation, OperationForme::Guidage { guidage } if guidage.clone().trace_mut().is_some()))
                .map(|_| k),
            _ => None,
        }
    }

    /// Remplace le chemin de la forme `k` par le tracé ; refusé (tracé conservé) si la
    /// forme obtenue est invalide.
    pub(crate) fn appliquer_trace_chemin(&mut self, k: usize, ferme: bool) -> Result<(), String> {
        let trace = self.modifier.chemin.clone();
        if trace.points.len() < 2 || trace.arc_ouvert() { return Err("Chemin : au moins deux points, et chaque arc terminé.".into()); }
        let r = self.resoudre();
        let e = self.famille.extrusions.get(k).ok_or("Forme introuvable.")?;
        let OperationForme::Guidage { guidage } = &e.operation else { return Err("Choisir une forme par chemin.".into()) };
        let mut guidage = guidage.clone();
        let (chemin, arcs, fermeture) = guidage.trace_mut().ok_or("Le raccordement droit n'a pas de chemin.")?;
        *chemin = trace.chemin();
        *arcs = trace.passages.clone();
        match fermeture {
            Some(f) => *f = ferme,
            None if ferme => return Err("Chemin fermé : réservé à l'extrusion par chemin (les deux profils d'un raccordement diffèrent).".into()),
            None => {}
        }
        let contour: Vec<[f64; 2]> = e.profil.iter().map(|p| { let (h, v) = self.famille.point_esquisse(&r, p); [h, v] }).collect();
        guidage.resolue(|p| self.famille.point_esquisse(&r, p)).valider(&contour)?;
        if let OperationForme::Guidage { guidage: g } = &mut self.famille.extrusions[k].operation { *g = guidage; }
        self.modifier.chemin = TraceChemin::default();
        self.erreur = None;
        self.message = Some("Chemin tracé appliqué — Ctrl+Z pour annuler.".into());
        Ok(())
    }
}

/// Outil « Tracer le chemin » dans le canevas ; `vise` : point accroché sous le curseur.
pub(super) fn canevas_chemin(ui: &mut egui::Ui, ed: &mut EditeurFamille, painter: &egui::Painter, cadre: &Cadre, resp: &egui::Response, vise: Option<(f64, f64)>) {
    let Some(k) = ed.forme_chemin() else {
        ed.erreur = Some("Tracer le chemin : choisir d'abord une extrusion ou un raccordement par chemin.".into());
        ed.choisir_outil(OutilFamille::Selection);
        return;
    };
    let orange = Color32::from_rgb(230, 120, 20);
    let clavier = !ui.ctx().egui_wants_keyboard_input();
    let trace = &mut ed.modifier.chemin;
    if clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::A)) { trace.arc = !trace.arc; }
    if clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)) { trace.retirer(); }
    let premier = trace.points.first().map(|p| cadre.ecran(p.0, p.1));
    let pres_premier = |q: egui::Pos2| trace.points.len() >= 3 && !trace.arc_ouvert() && premier.is_some_and(|p| p.distance(q) < 9.0);
    let survol = resp.hover_pos().filter(|_| resp.hovered());
    let fermeture = survol.is_some_and(pres_premier);
    let ligne: Vec<egui::Pos2> = trace.apercu(vise.filter(|_| !fermeture), fermeture).into_iter().map(|p| cadre.ecran(p.0, p.1)).collect();
    if ligne.len() >= 2 { painter.add(egui::Shape::line(ligne, egui::Stroke::new(1.5, orange))); }
    for (i, p) in trace.points.iter().enumerate() {
        let rayon = if trace.passages.contains(&i) { 2.5 } else { 4.0 };
        painter.circle_stroke(cadre.ecran(p.0, p.1), rayon, egui::Stroke::new(1.5, orange));
    }
    if fermeture && let Some(p) = premier { painter.circle_stroke(p, 7.0, egui::Stroke::new(2.0, Color32::from_rgb(30, 140, 60))); }
    let invite = if trace.arc { "Arc : cliquer le point de passage" } else if trace.arc_ouvert() { "Arc : cliquer le point d'arrivée" } else { "Chemin : cliquer les points · A : arc · premier point : fermer · Entrée ou Échap : terminer" };
    ed.message = Some(invite.into());
    let terminer = std::mem::take(&mut ed.echap_valide) || resp.clicked_by(egui::PointerButton::Secondary) || (clavier && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)));
    let fermer = resp.clicked() && resp.interact_pointer_pos().is_some_and(pres_premier);
    if fermer || terminer {
        if let Err(e) = ed.appliquer_trace_chemin(k, fermer) { ed.erreur = Some(e); return; }
        ed.choisir_outil(OutilFamille::Selection);
        ed.selectionner(vec![SelectionFamille::Extrusion(k)]);
    } else if resp.clicked() && let Some(p) = vise {
        ed.modifier.chemin.ajouter(p);
    }
}

/// Propriétés propres aux formes guidées : erreur de construction, reprise du profil
/// initial, profil d'arrivée verrouillé sur les plans, accès au tracé du chemin.
pub(super) fn proprietes_guidage(ui: &mut egui::Ui, ed: &mut EditeurFamille, k: usize) {
    let r = ed.resoudre();
    let e = &ed.famille.extrusions[k];
    let OperationForme::Guidage { guidage } = &e.operation else { return };
    let contour: Vec<[f64; 2]> = e.profil.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(&r, p); [h, v] }).collect();
    let resolue = guidage.resolue(|p| ed.famille.point_esquisse(&r, p));
    let erreur = resolue.valider(&contour).err();
    if let Some(erreur) = erreur { ui.colored_label(ui.visuals().error_fg_color, erreur); }
    let par_chemin = guidage.clone().trace_mut().is_some();
    if par_chemin && ui.button("Tracer le chemin dans la vue").on_hover_text("Clics accrochés ; A : arc par trois points ; premier point : fermer ; Entrée : terminer").clicked() {
        ed.choisir_outil(OutilFamille::CheminGuidage);
        ed.selectionner(vec![SelectionFamille::Extrusion(k)]);
        return;
    }
    if guidage.profil_fin().is_none() { return; }
    let ancre = guidage.ancrage_fin().is_some();
    let (vue, sur_plan, profil) = (e.vue, e.support.is_some(), e.profil.clone());
    let plans: Vec<PlanRef> = ed.famille.plans.clone();
    let OperationForme::Guidage { guidage } = &mut ed.famille.extrusions[k].operation else { return };
    if ui.button("Reprendre le profil initial à l'arrivée").clicked() {
        match guidage {
            FormeGuidee::Raccordement { ancrage_fin: Some(a), .. } | FormeGuidee::RaccordementChemin { ancrage_fin: Some(a), .. } => *a = profil.clone(),
            FormeGuidee::Raccordement { profil_fin, .. } | FormeGuidee::RaccordementChemin { profil_fin, .. } => *profil_fin = contour.clone(),
            FormeGuidee::ExtrusionChemin { .. } => {}
        }
    }
    let Some(ancrage) = guidage.ancrage_fin_mut() else { return };
    let mut piloter = ancre;
    ui.checkbox(&mut piloter, "Profil d'arrivée verrouillé sur les plans").on_hover_text("Chaque sommet suit un plan de référence et un décalage : paramètres et formules s'appliquent comme au profil initial");
    if piloter != ancre { *ancrage = piloter.then(|| profil.clone()); }
    let Some(points) = ancrage.as_mut() else { return };
    let (ah, av, _) = vue.axes();
    let mut retirer = None;
    egui::CollapsingHeader::new("Profil d'arrivée sur les plans").default_open(true).show(ui, |ui| {
        ui.label("Nombre de sommets libre : la correspondance est automatique s'il diffère du profil initial.");
        let suppression = points.len() > 3;
        for (i, p) in points.iter_mut().enumerate() {
            ui.push_id(("ancrage_fin", k, i), |ui| ui.horizontal(|ui| {
                ui.label(format!("{}", i + 1));
                for (face, axe) in [(&mut p.h, ah), (&mut p.v, av)] {
                    egui::ComboBox::from_id_salt(axe.nom()).width(96.0).selected_text(if face.plan.is_empty() { "Origine" } else { face.plan.as_str() }).show_ui(ui, |ui| {
                        if sur_plan { ui.selectable_value(&mut face.plan, String::new(), "Origine du plan"); }
                        for plan in plans.iter().filter(|q| !sur_plan && q.axe == axe) { ui.selectable_value(&mut face.plan, plan.nom.clone(), &plan.nom); }
                    });
                    ui.add(egui::DragValue::new(&mut face.decalage).speed(1.0).dimensions());
                }
                if ui.add_enabled(suppression, egui::Button::new("−")).clicked() { retirer = Some(i); }
            }));
        }
        if ui.button("Ajouter un sommet").clicked() && let Some(dernier) = points.last().cloned() {
            let premier = points[0].clone();
            let milieu = |a: &Face, b: &Face| Face { plan: a.plan.clone(), decalage: a.decalage + (b.decalage - a.decalage) * 0.5 * (a.plan == b.plan) as i32 as f64 };
            points.push(PointEsquisse { h: milieu(&dernier.h, &premier.h), v: milieu(&dernier.v, &premier.v) });
        }
    });
    if let Some(i) = retirer { points.remove(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_avec_arc_et_chemin_local() {
        let mut t = TraceChemin::default();
        t.ajouter((100.0, 200.0));
        t.arc = true;
        t.ajouter((100.0 + 1000.0 - 1000.0 * std::f64::consts::FRAC_1_SQRT_2, 200.0 + 1000.0 * std::f64::consts::FRAC_1_SQRT_2));
        assert!(t.arc_ouvert());
        t.ajouter((1100.0, 1200.0));
        assert!(!t.arc_ouvert());
        assert_eq!(t.passages, vec![1]);
        assert_eq!(t.chemin()[0], [0.0; 3]);
        assert_eq!(t.chemin()[2], [1000.0, 1000.0, 0.0]);
        // aperçu : 18 cordes de 5° plus le départ
        assert_eq!(t.apercu(None, false).len(), 19);
        t.retirer();
        assert!(t.arc_ouvert());
        t.retirer();
        assert!(t.passages.is_empty());
    }

    #[test]
    fn trace_applique_a_la_forme_avec_fermeture() {
        let ctx = egui::Context::default();
        let mut app = crate::Application::new(&eframe::CreationContext::_new_kittest(ctx), None);
        app.ouvrir_editeur_famille_type(None, modele::GenreOuverture::Fenetre);
        let ed = app.famille.as_mut().unwrap();
        ed.famille.extrusions.clear();
        let face = |plan: &str, d| Face { plan: plan.into(), decalage: d };
        let (ah, av, _) = VueEsquisse::Elevation.axes();
        let ph = ed.famille.plans.iter().find(|p| p.axe == ah).unwrap().nom.clone();
        let pv = ed.famille.plans.iter().find(|p| p.axe == av).unwrap().nom.clone();
        let profil = [(-50.0, -50.0), (50.0, -50.0), (50.0, 50.0), (-50.0, 50.0)].map(|(h, v)| PointEsquisse { h: face(&ph, h), v: face(&pv, v) }).to_vec();
        ed.famille.extrusions.push(FormeProfil { visibilite: Default::default(), operation: OperationForme::Guidage { guidage: FormeGuidee::extrusion(vec![[0.0; 3], [0.0, 0.0, 1000.0]]) }, support: None, nom: "Chemin".into(), vide: false,
            materiau: Materiau::Menuiserie, vue: VueEsquisse::Elevation, profil, profondeur: [face("", 0.0), face("", 100.0)], visible: None, materiau_parametre: None });
        let k = ed.famille.extrusions.len() - 1;
        for p in [(0.0, 0.0), (2000.0, 0.0), (2000.0, 2000.0), (0.0, 2000.0)] { ed.modifier.chemin.ajouter(p); }
        ed.appliquer_trace_chemin(k, true).unwrap();
        let OperationForme::Guidage { guidage } = &ed.famille.extrusions[k].operation else { unreachable!() };
        assert!(guidage.ferme());
        assert_eq!(guidage.chemin().len(), 4);
        assert!(ed.modifier.chemin.points.is_empty());
        // tracé invalide (demi-tour) : refus, la forme et le tracé restent
        for p in [(0.0, 0.0), (1000.0, 0.0), (0.0, 0.0)] { ed.modifier.chemin.ajouter(p); }
        assert!(ed.appliquer_trace_chemin(k, false).is_err());
        assert_eq!(ed.modifier.chemin.points.len(), 3);
        let OperationForme::Guidage { guidage } = &ed.famille.extrusions[k].operation else { unreachable!() };
        assert!(guidage.ferme());
    }
}
