//! Plan de dessin actif de l'éditeur de familles : un état de l'éditeur, pas du modèle,
//! choisi dans la barre sous la vue — « Sol » par défaut, un plan de référence (les
//! formes posées dessus le référencent et suivent ses contraintes et son angle) ou la
//! face d'un solide cliquée (repère figé au moment du choix). En vue 3D, le plan actif
//! s'affiche avec sa grille, les profils y sont saisis par projection du curseur et
//! l'accrochage commun (`saisie/moteur_accrochage.rs`) travaille dans son repère.
use super::*;
use crate::saisie::moteur_accrochage::{self as accrochage, Geometrie};
use glam::{DVec2, DVec3};
use moteur2d::dao::Accroche;
use noyau::forme2d::Geom;
use noyau::trace2d::{Forme, Primitive, Role, Style};

#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) enum PlanActif {
    #[default]
    Sol,
    Reference(String),
    Face(Repere),
}

impl PlanActif {
    pub(crate) fn nom(&self) -> String {
        match self {
            Self::Sol => "Sol".into(),
            Self::Reference(n) => n.clone(),
            Self::Face(_) => "Face d'un solide".into(),
        }
    }
}

/// Vue 3D de la famille à l'écran : caméra du rendu et rectangle.
pub(super) struct Ecran3D<'a> {
    pub camera: &'a Camera,
    pub rect: egui::Rect,
}

impl Ecran3D<'_> {
    fn aspect(&self) -> f32 {
        self.rect.width() / self.rect.height().max(1.0)
    }
    /// Rayon du curseur, repère de la famille (mm).
    pub fn rayon(&self, pos: egui::Pos2) -> (DVec3, DVec3) {
        let n = crate::socle::projection::ndc(self.rect, pos);
        let (o, d) = self.camera.ray(n.x, n.y, self.aspect());
        (o.as_dvec3() * 1000.0, d.as_dvec3())
    }
    /// Position écran d'un point de la famille (mm).
    pub fn ecran(&self, p: DVec3) -> Option<egui::Pos2> {
        crate::socle::projection::ecran(self.camera.view_proj(self.aspect()), self.rect, (p / 1000.0).as_vec3())
    }
}

/// Repère de la face d'un triangle touché : origine au point touché, h horizontal si
/// possible.
fn repere_face(tri: &[DVec3; 3], point: DVec3) -> Option<Repere> {
    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).try_normalize()?;
    let h = DVec3::Z.cross(n).try_normalize().unwrap_or(DVec3::X);
    Some(Repere { origine: point, h, v: n.cross(h) })
}

impl EditeurFamille {
    /// Repère du plan actif pour la résolution `r`.
    pub(super) fn repere_actif(&self, r: &Resolution) -> Option<Repere> {
        match &self.plan_actif {
            PlanActif::Sol => Some(Repere::axe(AxePlan::Z, 0.0)),
            PlanActif::Reference(n) => self.famille.repere_plan(n, r),
            PlanActif::Face(rep) => Some(*rep),
        }
    }

    /// Support des formes saisies en 3D : le plan de référence suivi, sinon un repère figé.
    pub(super) fn support_actif(&self) -> SupportForme {
        match &self.plan_actif {
            PlanActif::Reference(n) => SupportForme::Plan { plan: n.clone() },
            PlanActif::Sol => SupportForme::fige(&Repere::axe(AxePlan::Z, 0.0)),
            PlanActif::Face(rep) => SupportForme::fige(rep),
        }
    }

    /// Vue plane : plan d'axe actif (parallèle à la vue) d'où part la profondeur des formes.
    pub(super) fn plan_actif_parallele(&self) -> Option<String> {
        match &self.plan_actif {
            PlanActif::Reference(n) if !self.vue_3d && self.famille.plan(n).is_some() => Some(n.clone()),
            _ => None,
        }
    }

    /// Après un changement de vue : un plan devenu non parallèle revient au sol.
    pub(super) fn revoir_plan_actif(&mut self) {
        self.choix_face = false;
        if let Err(e) = self.choisir_plan_actif(self.plan_actif.clone()) {
            self.plan_actif = PlanActif::Sol;
            self.message = Some(format!("{e} Plan de dessin : Sol."));
        }
    }

    /// Choisit le plan de dessin. Dans une vue plane, un plan non parallèle à la vue est
    /// refusé (les plans tournés et les faces se choisissent en vue 3D).
    pub(crate) fn choisir_plan_actif(&mut self, choix: PlanActif) -> Result<(), String> {
        if let PlanActif::Reference(n) = &choix && !self.famille.existe_plan(n) {
            return Err(format!("Plan de référence « {n} » introuvable."));
        }
        if !self.vue_3d {
            let profondeur = self.vue.esquisse().axes().2;
            match &choix {
                PlanActif::Sol => {}
                PlanActif::Reference(n) if self.famille.plan(n).is_some_and(|p| p.axe == profondeur) => {}
                PlanActif::Reference(n) => return Err(format!("« {n} » n'est pas parallèle à la vue « {} » : le choisir en vue 3D.", self.vue.nom())),
                PlanActif::Face(_) => return Err("Face d'un solide : la choisir en vue 3D.".into()),
            }
        }
        if self.plan_actif != choix {
            self.abandonner_saisie();
        }
        self.plan_actif = choix;
        self.choix_face = false;
        Ok(())
    }

    /// Solide touché par un rayon (mm) : distance, indice d'objet, triangle.
    pub(super) fn solide_touche(&self, r: &Resolution, origine: DVec3, direction: DVec3) -> Option<(f64, usize, [DVec3; 3])> {
        let mut meilleur: Option<(f64, usize, [DVec3; 3])> = None;
        for (objet, solide) in self.famille.solides(r).iter().enumerate().filter(|(_, s)| self.montre_solide(s) && !s.vide) {
            for tri in &solide.tris {
                let [a, b, c] = *tri;
                let (e1, e2) = (b - a, c - a);
                let h = direction.cross(e2);
                let det = e1.dot(h);
                if det.abs() < 1e-12 { continue; }
                let f = 1.0 / det;
                let q = origine - a;
                let u = f * q.dot(h);
                if !(0.0..=1.0).contains(&u) { continue; }
                let v = f * direction.dot(q.cross(e1));
                if v < 0.0 || u + v > 1.0 { continue; }
                let t = f * e2.dot(q.cross(e1));
                if t > 0.0 && meilleur.is_none_or(|m| t < m.0) { meilleur = Some((t, objet, *tri)); }
            }
        }
        meilleur
    }

    /// Source d'accrochage dans le repère du plan actif : axes du plan, traces des autres
    /// plans de référence (droites), arêtes des solides contenues dans le plan, profils
    /// des formes posées sur le même support, esquisse en cours.
    fn source_plan(&self, r: &Resolution, rep: &Repere) -> Geometrie {
        let mut s = Geometrie { geoms: Vec::new(), droites: vec![(DVec2::ZERO, DVec2::X), (DVec2::ZERO, DVec2::Y)] };
        let local = |p: DVec3| { let q = rep.local(p); (DVec2::new(q.x, q.y), q.z) };
        for nom in self.famille.noms_plans() {
            if let Some(autre) = self.famille.repere_plan(nom, r) && let Some((p, d)) = modele::famille::modelee::intersection(rep, &autre) {
                let (o, _) = local(p);
                s.droites.push((o, DVec2::new(d.dot(rep.h), d.dot(rep.v)).normalize_or_zero()));
            }
        }
        for solide in self.famille.solides(r).iter().filter(|x| self.montre_solide(x)) {
            for [a, b] in noyau::aretes::aretes(&solide.tris).vives {
                let ((pa, za), (pb, zb)) = (local(a), local(b));
                if za.abs() < 0.5 && zb.abs() < 0.5 && pa.distance(pb) > 1e-6 { s.geoms.push(Geom::Segment([pa, pb])); }
            }
        }
        let support = Some(self.support_actif());
        for e in self.famille.extrusions.iter().filter(|e| e.support == support) {
            let pts: Vec<DVec2> = e.profil.iter().map(|p| { let (h, v) = self.famille.point_esquisse(r, p); DVec2::new(h, v) }).collect();
            s.geoms.extend((0..pts.len()).map(|i| Geom::Segment([pts[i], pts[(i + 1) % pts.len()]])));
        }
        let esquisse: Vec<DVec2> = self.esquisse.iter().map(|(_, p)| DVec2::new(p.0, p.1)).collect();
        s.geoms.extend(esquisse.windows(2).map(|w| Geom::Segment([w[0], w[1]])));
        if let [p] = esquisse.as_slice() { s.geoms.push(Geom::Segment([*p, *p])); }
        s
    }

    /// Point visé sur le plan actif sous le curseur (repère local, mm), accroché.
    pub(super) fn viser_plan_actif(&self, r: &Resolution, rep: &Repere, ecran: &Ecran3D, pos: egui::Pos2, maj: bool) -> Option<(DVec2, Accroche)> {
        let (o, d) = ecran.rayon(pos);
        let n = rep.normale();
        if d.dot(n).abs() < 1e-9 { return None; }
        let t = n.dot(rep.origine - o) / d.dot(n);
        let q = rep.local(o + d * t);
        let c = DVec2::new(q.x, q.y);
        // tolérance : rayon écran ramené en mm au point visé
        let p0 = ecran.ecran(rep.point(c.x, c.y, 0.0))?;
        let p1 = ecran.ecran(rep.point(c.x + 100.0, c.y, 0.0))?;
        let tol = accrochage::TOLERANCE_PX * 100.0 / f64::from(p0.distance(p1)).max(1e-6);
        let depuis = self.esquisse.last().map(|(_, p)| DVec2::new(p.0, p.1));
        Some(accrochage::viser_simple(&self.source_plan(r, rep), c, tol, depuis, maj, 0.0, 1.0))
    }

    /// Point de l'espace pour « Plan par 3 points » : sommet d'un solide près du curseur,
    /// point touché sur un solide, sinon point accroché du plan actif.
    fn point_3d(&self, r: &Resolution, ecran: &Ecran3D, pos: egui::Pos2) -> Option<(DVec3, Accroche)> {
        let (o, d) = ecran.rayon(pos);
        if let Some((t, _, tri)) = self.solide_touche(r, o, d) {
            let proche = tri.iter().filter_map(|p| Some((*p, ecran.ecran(*p)?.distance(pos)))).min_by(|a, b| a.1.total_cmp(&b.1));
            return Some(match proche {
                Some((p, e)) if f64::from(e) <= accrochage::TOLERANCE_PX => (p, Accroche::Extremite),
                _ => (o + d * t, Accroche::Proche),
            });
        }
        let rep = self.repere_actif(r)?;
        let (q, a) = self.viser_plan_actif(r, &rep, ecran, pos, false)?;
        Some((rep.point(q.x, q.y, 0.0), a))
    }

    /// Saisie dans la vue 3D : choix d'une face, profils sur le plan actif, plan par trois
    /// points. Rend les primitives du plan actif (grille, esquisse) en repère de rendu (m).
    pub(super) fn saisie_3d(&mut self, ui: &egui::Ui, resp: &egui::Response, ecran: &Ecran3D, r: &Resolution) -> Vec<Primitive> {
        let painter = ui.painter_at(ecran.rect);
        let maj = ui.input(|i| i.modifiers.shift);
        let survol = resp.hover_pos();
        let clic = resp.clicked_by(PointerButton::Primary).then(|| resp.interact_pointer_pos()).flatten();
        if self.choix_face {
            if let Some(q) = clic {
                let (o, d) = ecran.rayon(q);
                match self.solide_touche(r, o, d).and_then(|(t, _, tri)| repere_face(&tri, o + d * t)) {
                    Some(rep) => { self.choisir_plan_actif(PlanActif::Face(rep)).ok(); self.message = Some("Plan de dessin : face du solide (repère figé).".into()); }
                    None => self.message = Some("Aucune face sous le curseur : cliquer un solide.".into()),
                }
            }
        } else if self.outil == OutilFamille::PlanTroisPoints {
            if let Some((p, a)) = survol.and_then(|q| self.point_3d(r, ecran, q)) && let Some(e) = ecran.ecran(p) {
                accrochage::peindre_marqueur(&painter, e, a);
            }
            if let Some((p, _)) = clic.and_then(|q| self.point_3d(r, ecran, q)) {
                self.points_3d.push(p);
                if let [a, b, c] = self.points_3d[..] {
                    self.points_3d.clear();
                    let nom = nom_libre(|n| self.famille.existe_plan(n), "Plan tourné");
                    match self.famille.plan_par_trois_points(r, &nom, [a, b, c]) {
                        Ok(p) => {
                            self.message = Some(format!("« {} » : tourné de {:.2}° depuis « {} » autour de la droite « {} » ; angle cotable dans Propriétés.", p.nom, p.angle, p.hote, p.appui));
                            self.famille.plans_tournes.push(p);
                            self.selection = SelectionFamille::Plan(nom);
                            self.erreur = None;
                        }
                        Err(e) => self.erreur = Some(e),
                    }
                } else {
                    self.message = Some(format!("Point {} sur 3.", self.points_3d.len()));
                }
            }
            for p in &self.points_3d {
                if let Some(e) = ecran.ecran(*p) { painter.circle_filled(e, 3.5, accrochage::COULEUR_MARQUEUR); }
            }
        }
        let Some(rep) = self.repere_actif(r) else { return Vec::new() };
        if self.outil.est_forme() {
            let vise = survol.and_then(|q| self.viser_plan_actif(r, &rep, ecran, q, maj));
            if let Some((p, a)) = vise && let Some(e) = ecran.ecran(rep.point(p.x, p.y, 0.0)) {
                accrochage::peindre_marqueur(&painter, e, a);
            }
            let premier = self.esquisse.first().and_then(|(_, p)| ecran.ecran(rep.point(p.0, p.1, 0.0)));
            let fermer = clic.is_some_and(|q| self.esquisse.len() >= 3 && premier.is_some_and(|e| e.distance(q) < 9.0));
            if fermer || resp.clicked_by(PointerButton::Secondary) {
                self.erreur = self.terminer_forme(Some(self.support_actif())).err();
            } else if let Some((p, _)) = clic.and_then(|q| self.viser_plan_actif(r, &rep, ecran, q, maj)) {
                let face = |x: f64| Face { plan: String::new(), decalage: x };
                self.esquisse.push((PointEsquisse { h: face(p.x), v: face(p.y) }, (p.x, p.y)));
                self.capter_axe_revolution();
            }
            return primitives_plan(self, &rep, vise.map(|v| v.0));
        }
        primitives_plan(self, &rep, None)
    }
}

/// Grille et axes du plan actif, esquisse en cours et aperçu jusqu'au point visé, en
/// primitives du rendu commun (repère de rendu, m).
fn primitives_plan(ed: &EditeurFamille, rep: &Repere, vise: Option<DVec2>) -> Vec<Primitive> {
    let m = |h: f64, v: f64| rep.point(h, v, 0.0) / 1000.0;
    let etendue = ed.largeur.max(ed.hauteur).max(ed.epaisseur) * 0.75 + 500.0;
    let pas = if etendue > 3000.0 { 500.0 } else { 100.0 };
    let n = (etendue / pas).ceil() as i32;
    let l = f64::from(n) * pas;
    let mut grille = Vec::new();
    for k in (-n..=n).filter(|k| *k != 0) {
        let x = f64::from(k) * pas;
        grille.push([m(x, -l), m(x, l)]);
        grille.push([m(-l, x), m(l, x)]);
    }
    let style = |couleur| Style { couleur, epaisseur: 0.0 };
    let segments = |s: Vec<[DVec3; 2]>, couleur| Primitive { forme: Forme::Segments(s), style: style(couleur), role: Role::Annotation, element: None };
    let mut prims = vec![segments(grille, [150, 180, 210, 255]), segments(vec![[m(-l, 0.0), m(l, 0.0)], [m(0.0, -l), m(0.0, l)]], [40, 90, 160, 255])];
    let mut esquisse: Vec<DVec3> = ed.esquisse.iter().map(|(_, p)| m(p.0, p.1)).collect();
    if let Some(p) = vise.filter(|_| !esquisse.is_empty()) { esquisse.push(m(p.x, p.y)); }
    if esquisse.len() >= 2 {
        prims.push(Primitive { forme: Forme::Polyligne { points: esquisse, fermee: false }, style: style([20, 120, 60, 255]), role: Role::Annotation, element: None });
    }
    prims
}

/// Choix du plan de dessin dans la barre de la vue de famille.
pub(super) fn choix_plan_ui(ui: &mut egui::Ui, ed: &mut EditeurFamille) {
    let noms: Vec<String> = ed.famille.noms_plans().into_iter().map(String::from).collect();
    let mut choix = None;
    let mut face = false;
    egui::ComboBox::from_id_salt("plan_dessin_famille").selected_text(format!("Plan de dessin : {}", ed.plan_actif.nom())).show_ui(ui, |ui| {
        if ui.selectable_label(ed.plan_actif == PlanActif::Sol, "Sol").clicked() { choix = Some(PlanActif::Sol); }
        for n in noms {
            let actif = ed.plan_actif == PlanActif::Reference(n.clone());
            if ui.selectable_label(actif, &n).clicked() { choix = Some(PlanActif::Reference(n)); }
        }
        ui.separator();
        face = ui.add_enabled(ed.vue_3d, egui::Button::new("Face d'un solide…")).on_disabled_hover_text("Ouvrir la vue 3D, puis cliquer la face").clicked();
    }).response.on_hover_text("Plan où se dessinent les profils en vue 3D (grille) ; dans une vue plane, seuls les plans parallèles à la vue");
    if face {
        ed.choix_face = true;
        ed.message = Some("Cliquer la face d'un solide : elle devient le plan de dessin (repère figé).".into());
    }
    if let Some(c) = choix {
        ed.erreur = ed.choisir_plan_actif(c).err();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_actif_refuse_hors_parallele_et_profil_saisi_en_3d_suit_l_angle() {
        let ctx = egui::Context::default();
        let mut app = crate::Application::new(&eframe::CreationContext::_new_kittest(ctx), None);
        app.ouvrir_editeur_famille_type(None, modele::GenreOuverture::Fenetre);
        let ed = app.famille.as_mut().unwrap();
        ed.ouvrir_vue(VueFamille::Elevation);
        // vue plane : seuls Sol et les plans d'axe parallèles à la vue
        assert!(ed.choisir_plan_actif(PlanActif::Reference("Gauche".into())).unwrap_err().contains("parallèle"));
        assert!(ed.choisir_plan_actif(PlanActif::Face(Repere::axe(AxePlan::Y, 0.0))).is_err());
        ed.choisir_plan_actif(PlanActif::Reference("Cadre ext".into())).unwrap();
        assert_eq!(ed.plan_actif_parallele().as_deref(), Some("Cadre ext"));
        ed.ouvrir_vue(VueFamille::Plan);
        assert_eq!(ed.plan_actif, PlanActif::Sol, "plan non parallèle à la nouvelle vue : retour au sol");
        // vue 3D : plan tourné autour de la droite Bas ∩ Gauche, piloté par « Pente »
        ed.famille.parametres.push(Parametre::nouveau("Pente", "30"));
        ed.famille.plans_tournes.push(PlanTourne { nom: "Incliné".into(), hote: "Bas".into(), appui: "Gauche".into(), angle: 0.0 });
        ed.famille.contraintes.push(Contrainte::Angle { a: "Bas".into(), b: "Incliné".into(), valeur: "Pente".into(), priorite: 5, ligne: None });
        ed.ouvrir_vue_3d();
        ed.choisir_plan_actif(PlanActif::Reference("Incliné".into())).unwrap();
        ed.choisir_outil(OutilFamille::Extrusion);
        let r = ed.resoudre();
        let rep = ed.repere_actif(&r).unwrap();
        let camera = { let mut c = Camera::default(); c.fit(Vec3::new(-1.0, -0.5, -0.2), Vec3::new(1.0, 0.5, 1.5)); c };
        let ecran = Ecran3D { camera: &camera, rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)) };
        for (h, v) in [(0.0, 0.0), (300.0, 0.0), (300.0, 200.0), (0.0, 200.0)] {
            let pos = ecran.ecran(rep.point(h, v, 0.0)).unwrap();
            // Maj : contrainte orthogonale sans accrochage, pour lire la projection seule
            let (p, _) = ed.viser_plan_actif(&r, &rep, &ecran, pos, true).unwrap();
            assert!((p.x - h).abs() < 2.0 && (p.y - v).abs() < 2.0, "{p:?} ≠ ({h}, {v})");
            let face = |x: f64| Face { plan: String::new(), decalage: x.round() };
            ed.esquisse.push((PointEsquisse { h: face(p.x), v: face(p.y) }, (p.x.round(), p.y.round())));
        }
        ed.terminer_forme(Some(ed.support_actif())).unwrap();
        assert_eq!(ed.famille.extrusions.last().unwrap().support, Some(SupportForme::Plan { plan: "Incliné".into() }));
        ed.famille.valider(ed.largeur, ed.hauteur, ed.epaisseur).unwrap();
        // l'angle change : la forme suit le plan
        let dans_le_plan = |ed: &EditeurFamille| {
            let r = ed.resoudre();
            let rep = ed.famille.repere_plan("Incliné", &r).unwrap();
            ed.famille.solides(&r).last().unwrap().tris.iter().flatten().all(|p| (-1e-6..=100.0 + 1e-6).contains(&rep.local(*p).z))
        };
        assert!(dans_le_plan(ed));
        let avant = ed.famille.solides(&ed.resoudre()).last().unwrap().tris.clone();
        ed.famille.parametres.iter_mut().find(|p| p.nom == "Pente").unwrap().formule = "50".into();
        assert!(dans_le_plan(ed));
        assert_ne!(ed.famille.solides(&ed.resoudre()).last().unwrap().tris, avant);
    }
}
