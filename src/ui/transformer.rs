//! Socle graphique des commandes de transformation (voir docs/MODIFICATION.md) :
//! Déplacer, Copier, Tourner et Aligner se font entièrement à la souris dans la vue.
//! Chaque commande suit le même cycle : consigne au curseur, survol de la cible,
//! aperçu fantôme, cotes temporaires, valeur tapée facultative (Entrée), une seule
//! transaction à la validation, Échap pour revenir d'une étape.
use super::*;
use crate::modele::edition::commandes::Commande;
use crate::ui::ruban::Outil;

/// État de la commande de transformation en cours.
#[derive(Default)]
pub(super) struct Transformation {
    /// Points cliqués (mm, plan du projet) : base puis destination, ou direction de
    /// départ puis direction d'arrivée pour Tourner.
    pub points: Vec<[f64; 2]>,
    /// Tourner : centre de rotation ; absent, celui de la sélection.
    pub centre: Option<[f64; 2]>,
    /// Tourner : le prochain clic place le centre.
    pub placer_centre: bool,
    /// Aligner : élément de référence (mur ou mur composé).
    pub reference: Option<String>,
    /// Déplacer / Tourner : la transformation porte sur une copie.
    pub copie: bool,
    /// Copier : la commande reste sur le même point de base après chaque copie.
    pub multiple: bool,
    /// Valeur tapée au clavier (distance en unité d'affichage, ou angle en degrés).
    pub saisie: String,
    /// Dernière position du curseur dans le plan (mm), pour une valeur tapée.
    pub curseur: Option<[f64; 2]>,
    /// Outil auquel l'état se rapporte : un changement d'outil le remet à zéro.
    pub outil: Option<Outil>,
    /// Élément natif sous le curseur (Aligner : surligné avant le clic).
    pub cible: Option<String>,
}

impl Transformation {
    /// Échap : revient d'une étape ; faux s'il n'y a plus rien à défaire (quitter l'outil).
    pub fn reculer(&mut self) -> bool {
        if !self.saisie.is_empty() {
            self.saisie.clear();
        } else if self.placer_centre {
            self.placer_centre = false;
        } else if self.points.pop().is_some() {
        } else if self.reference.take().is_some() {
        } else {
            return false;
        }
        true
    }
    /// Remet l'état à zéro en gardant les options (copie, multiple).
    pub fn reinitialiser(&mut self) {
        let (copie, multiple, outil) = (self.copie, self.multiple, self.outil);
        *self = Self { copie, multiple, outil, ..Default::default() };
    }
    /// Valeur tapée attendue à l'étape courante.
    pub fn attend_valeur(&self, outil: Outil) -> bool {
        match outil {
            Outil::Deplacer | Outil::Copier => self.points.len() == 1,
            Outil::Tourner => !self.placer_centre,
            _ => false,
        }
    }
    /// Consigne de l'étape courante, affichée au curseur et sous la barre de commande.
    pub fn consigne(&self, outil: Outil) -> String {
        let valeur = if self.saisie.is_empty() { String::new() } else { format!(" · valeur : {} (Entrée)", self.saisie) };
        match outil {
            Outil::Deplacer | Outil::Copier if self.points.is_empty() => "Cliquer le point de base".into(),
            Outil::Deplacer | Outil::Copier => format!("Cliquer la destination, ou taper la distance{valeur} · Maj : orthogonal"),
            Outil::Tourner if self.placer_centre => "Cliquer le nouveau centre de rotation".into(),
            Outil::Tourner if self.points.is_empty() => format!("Cliquer la direction de départ, ou taper l'angle{valeur} · clic sur le centre : le déplacer"),
            Outil::Tourner => format!("Cliquer la direction d'arrivée, ou taper l'angle{valeur} · Maj : pas de 15°"),
            Outil::Aligner if self.reference.is_none() => "Cliquer la référence (axe d'un mur)".into(),
            Outil::Aligner => "Cliquer les éléments à aligner · Échap : changer de référence".into(),
            _ => String::new(),
        }
    }
}

/// Angle signé (degrés, sens trigonométrique) de la direction centre→a à centre→b.
pub(super) fn angle_deg(centre: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (u, v) = ([a[0] - centre[0], a[1] - centre[1]], [b[0] - centre[0], b[1] - centre[1]]);
    (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]).to_degrees()
}

/// Angle arrondi au multiple de `pas` (degrés) le plus proche.
pub(super) fn accrocher_angle(angle: f64, pas: f64) -> f64 {
    (angle / pas).round() * pas
}

/// Point à `distance` de `base` dans la direction du curseur.
pub(super) fn point_a_distance(base: [f64; 2], curseur: [f64; 2], distance: f64) -> Option<[f64; 2]> {
    let (dx, dy) = (curseur[0] - base[0], curseur[1] - base[1]);
    let l = dx.hypot(dy);
    (l > 1e-9).then(|| [base[0] + dx / l * distance, base[1] + dy / l * distance])
}

/// Rotation d'un point autour de `centre` (degrés).
pub(super) fn tourner_point(p: [f64; 2], centre: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.to_radians().sin_cos();
    let (x, y) = (p[0] - centre[0], p[1] - centre[1]);
    [centre[0] + x * c - y * s, centre[1] + x * s + y * c]
}

/// Valeur tapée : nombre décimal (virgule ou point).
pub(super) fn lire_valeur(saisie: &str) -> Option<f64> {
    saisie.replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

impl ViewerApp {
    /// Outil de transformation actif dans l'onglet Modifier, s'il peut s'exécuter.
    pub(super) fn outil_transformation(&self) -> Option<Outil> {
        if self.onglet_ruban != OngletRuban::Modifier || self.natif.is_none() || self.loading.is_some() {
            return None;
        }
        match self.ruban_edition.outil()? {
            o @ (Outil::Deplacer | Outil::Copier | Outil::Tourner) if !self.selection_bim().is_empty() => Some(o),
            Outil::Aligner => Some(Outil::Aligner),
            _ => None,
        }
    }

    /// Synchronise l'état avec l'outil actif : un nouvel outil repart de zéro.
    pub(super) fn suivre_transformation(&mut self) {
        let outil = self.outil_transformation();
        if outil != self.transfo.outil {
            self.transfo.outil = outil;
            self.transfo.reinitialiser();
        }
    }

    /// Centre de la sélection (milieu de l'emprise des axes et contours), en mm.
    pub(super) fn centre_selection(&self) -> Option<[f64; 2]> {
        let (projet, _) = self.natif.as_ref()?;
        let ids = self.selection_bim();
        let mut pts: Vec<[f64; 2]> = Vec::new();
        for id in &ids {
            if let Some((a, b, _)) = projet.axe_element(id) {
                pts.extend([a, b]);
            }
        }
        for d in projet.dalles.iter().filter(|d| ids.contains(&d.id)) {
            pts.extend(d.contours.iter().flatten().copied());
        }
        for b in projet.barres.iter().filter(|b| ids.contains(&b.id)) {
            pts.extend([[b.debut[0], b.debut[1]], [b.fin[0], b.fin[1]]]);
        }
        if pts.is_empty() {
            return None;
        }
        let (mut mn, mut mx) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in pts {
            for k in 0..2 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        Some([(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0])
    }

    fn centre_rotation(&self) -> Option<[f64; 2]> {
        self.transfo.centre.or_else(|| self.centre_selection())
    }

    /// Angle courant de Tourner pour une direction d'arrivée `p` (Maj : pas de 15°).
    fn angle_tourner(&self, p: [f64; 2], maj: bool) -> Option<f64> {
        let c = self.centre_rotation()?;
        let depart = *self.transfo.points.first()?;
        let a = angle_deg(c, depart, p);
        Some(if maj { accrocher_angle(a, 15.0) } else { a })
    }

    /// Exécute la transformation finale en une seule transaction. `copie` : sur une copie.
    fn appliquer_transformation(&mut self, commande: impl FnOnce(Vec<String>) -> Commande, copie: bool) {
        let ids = self.selection_bim();
        let resultat = if copie {
            // copie puis transformation des copies : une seule étape d'annulation
            self.executer_bim_suite(Commande::Copier { ids, delta: [0.0; 3] }, commande)
        } else {
            self.executer_bim(commande(ids)).map(|_| ())
        };
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }

    /// Clic dans la vue pendant une transformation. `cible` : élément natif sous le clic.
    pub(super) fn clic_transformation(&mut self, outil: Outil, p: [f64; 2], ecran: egui::Pos2, rect: egui::Rect, altitude: f64, maj: bool, cible: Option<String>) {
        match outil {
            Outil::Deplacer | Outil::Copier => match self.transfo.points.first().copied() {
                None => self.transfo.points.push(p),
                Some(a) => {
                    let delta = [p[0] - a[0], p[1] - a[1], 0.0];
                    self.valider_deplacement(outil, delta);
                }
            },
            Outil::Tourner => {
                let pres_du_centre = self.centre_rotation().and_then(|c| self.ecran_plan(rect, c, altitude)).is_some_and(|c| c.distance(ecran) <= 10.0);
                if self.transfo.placer_centre {
                    self.transfo.centre = Some(p);
                    self.transfo.placer_centre = false;
                } else if self.transfo.points.is_empty() && pres_du_centre {
                    self.transfo.placer_centre = true;
                } else if self.transfo.points.is_empty() {
                    self.transfo.points.push(p);
                } else if let Some(angle) = self.angle_tourner(p, maj) {
                    self.valider_rotation(angle);
                }
            }
            Outil::Aligner => {
                let Some(id) = cible else {
                    self.status = "Aligner : cliquer sur un élément.".into();
                    return;
                };
                match self.transfo.reference.clone() {
                    None => {
                        if self.natif.as_ref().is_some_and(|(p, _)| p.axe_element(&id).is_some()) {
                            self.transfo.reference = Some(id);
                        } else {
                            self.status = "Aligner : la référence doit être un mur droit.".into();
                        }
                    }
                    Some(r) if r == id => self.status = "Aligner : choisir un autre élément que la référence.".into(),
                    Some(r) => {
                        if let Err(e) = self.executer_bim(Commande::Aligner { ids: vec![id], reference: r }) {
                            self.error = Some(e);
                        } else {
                            self.status = "Élément aligné — cliquer le suivant, Ctrl+Z pour annuler.".into();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn valider_deplacement(&mut self, outil: Outil, delta: [f64; 3]) {
        let copier = outil == Outil::Copier || self.transfo.copie;
        let ids = self.selection_bim();
        let originaux = ids.clone();
        let commande = if copier { Commande::Copier { ids, delta } } else { Commande::Deplacer { ids, delta } };
        match self.executer_bim(commande) {
            Ok(nouveaux) => {
                if outil == Outil::Copier && self.transfo.multiple {
                    // la sélection reste l'original : copies successives depuis la même base
                    let base = self.transfo.points[0];
                    self.bim.selection = originaux;
                    self.transfo.reinitialiser();
                    self.transfo.points.push(base);
                    self.status = format!("{} copie(s) créée(s) — cliquer la destination suivante, Échap pour terminer.", nouveaux.len());
                } else {
                    self.transfo.reinitialiser();
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn valider_rotation(&mut self, angle: f64) {
        let Some(centre) = self.centre_rotation() else { return };
        let copie = self.transfo.copie;
        self.appliquer_transformation(|ids| Commande::Tourner { ids, centre, angle }, copie);
        self.transfo.reinitialiser();
        if self.error.is_none() {
            self.status = format!("Rotation de {angle:.1}° appliquée — Ctrl+Z pour annuler.");
        }
    }

    /// Clavier : chiffres, virgule, signe moins, Retour arrière et Entrée pour une valeur
    /// tapée. Lu avant les raccourcis, uniquement quand l'étape attend une valeur.
    pub(super) fn clavier_transformation(&mut self, ctx: &egui::Context) {
        self.suivre_transformation();
        let Some(outil) = self.transfo.outil else { return };
        if !self.transfo.attend_valeur(outil) || self.saisie_active {
            return;
        }
        let mut entree = false;
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t) if t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')) => {
                    self.transfo.saisie.push_str(t);
                    false
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } if !self.transfo.saisie.is_empty() => {
                    self.transfo.saisie.pop();
                    false
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } if !self.transfo.saisie.is_empty() => {
                    entree = true;
                    false
                }
                _ => true,
            })
        });
        if !entree {
            return;
        }
        let Some(v) = lire_valeur(&self.transfo.saisie) else {
            self.status = format!("Valeur illisible : {}", self.transfo.saisie);
            self.transfo.saisie.clear();
            return;
        };
        self.transfo.saisie.clear();
        match outil {
            Outil::Deplacer | Outil::Copier => {
                let (Some(base), Some(curseur)) = (self.transfo.points.first().copied(), self.transfo.curseur) else { return };
                let d = crate::config::options::unite().vers_mm(v);
                if let Some(p) = point_a_distance(base, curseur, d) {
                    self.valider_deplacement(outil, [p[0] - base[0], p[1] - base[1], 0.0]);
                }
            }
            Outil::Tourner => self.valider_rotation(v),
            _ => {}
        }
    }

    /// Aperçu de la transformation : consigne au curseur, accrochage, cotes temporaires,
    /// fantôme de la sélection transformée, centre de rotation, référence d'alignement.
    pub(super) fn apercu_transformation(&mut self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>, altitude: f64) {
        let cible = self.transfo.cible.clone();
        let Some(outil) = self.transfo.outil else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(55, 130, 200);
        let orange = Color32::from_rgb(230, 120, 20);
        let maj = ui.input(|i| i.modifiers.shift);
        let unite = crate::config::options::unite();
        let point = survol.and_then(|q| {
            let depart = match outil { Outil::Deplacer | Outil::Copier => self.transfo.points.first().copied(), _ => None };
            self.point_tracage(q, rect, maj && matches!(outil, Outil::Deplacer | Outil::Copier), depart, altitude)
        });
        self.transfo.curseur = point.map(|(p, _)| p);
        if let (Some(q), Some((p, acc))) = (survol, point) {
            if let Some(e) = self.ecran_plan(rect, p, altitude) {
                marqueur(&painter, e, acc, bleu, orange);
            }
            // consigne au curseur
            painter.text(q + egui::vec2(16.0, 14.0), egui::Align2::LEFT_TOP, self.transfo.consigne(outil), egui::FontId::proportional(12.0), bleu);
        }
        let fantome = |app: &Self, f: &dyn Fn([f64; 2]) -> [f64; 2]| {
            let Some((projet, _)) = &app.natif else { return };
            let ids = app.selection_bim();
            for id in &ids {
                let (Some((_, _, z)), Some((s, e, _))) = (projet.axe_element(id), projet.ligne_justification(id)) else { continue };
                if let (Some(x), Some(y)) = (app.ecran_plan(rect, f(s), z), app.ecran_plan(rect, f(e), z)) {
                    painter.line_segment([x, y], egui::Stroke::new(2.5, bleu.gamma_multiply(0.7)));
                }
            }
            for dalle in projet.dalles.iter().filter(|x| ids.contains(&x.id)) {
                let z = projet.niveau(&dalle.niveau).map_or(0.0, |n| n.altitude) + dalle.decalage;
                for c in &dalle.contours {
                    let pts: Vec<egui::Pos2> = c.iter().chain(c.first()).filter_map(|v| app.ecran_plan(rect, f(*v), z)).collect();
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, bleu.gamma_multiply(0.7))));
                }
            }
        };
        match outil {
            Outil::Deplacer | Outil::Copier => {
                let (Some(a), Some((p, _))) = (self.transfo.points.first().copied(), point) else { return };
                let p = lire_valeur(&self.transfo.saisie).and_then(|v| point_a_distance(a, p, unite.vers_mm(v))).unwrap_or(p);
                let d = [p[0] - a[0], p[1] - a[1]];
                if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, altitude), self.ecran_plan(rect, p, altitude)) {
                    painter.add(egui::Shape::dashed_line(&[x, y], egui::Stroke::new(1.5, bleu), 6.0, 4.0));
                    painter.circle_filled(x, 3.5, bleu);
                    painter.text(x.lerp(y, 0.5) + egui::vec2(0.0, -16.0), egui::Align2::CENTER_CENTER, unite.afficher(d[0].hypot(d[1])), egui::FontId::proportional(13.0), bleu);
                }
                fantome(self, &|v| [v[0] + d[0], v[1] + d[1]]);
            }
            Outil::Tourner => {
                let Some(c) = self.centre_rotation() else { return };
                let Some(ce) = self.ecran_plan(rect, c, altitude) else { return };
                // centre : croix cerclée, surlignée au survol (clic : le déplacer)
                let proche = survol.is_some_and(|q| q.distance(ce) <= 10.0) || self.transfo.placer_centre;
                let couleur = if proche { orange } else { bleu };
                painter.circle_stroke(ce, 7.0, egui::Stroke::new(1.8, couleur));
                painter.line_segment([ce - egui::vec2(10.0, 0.0), ce + egui::vec2(10.0, 0.0)], egui::Stroke::new(1.2, couleur));
                painter.line_segment([ce - egui::vec2(0.0, 10.0), ce + egui::vec2(0.0, 10.0)], egui::Stroke::new(1.2, couleur));
                if self.transfo.placer_centre {
                    return;
                }
                let Some(depart) = self.transfo.points.first().copied() else {
                    if let Some((p, _)) = point {
                        if let Some(e) = self.ecran_plan(rect, p, altitude) {
                            painter.add(egui::Shape::dashed_line(&[ce, e], egui::Stroke::new(1.2, bleu), 5.0, 4.0));
                        }
                    }
                    return;
                };
                let angle = lire_valeur(&self.transfo.saisie).or_else(|| point.and_then(|(p, _)| self.angle_tourner(p, maj)));
                let Some(angle) = angle else { return };
                let Some(de) = self.ecran_plan(rect, depart, altitude) else { return };
                let arrivee = tourner_point(depart, c, angle);
                let Some(ae) = self.ecran_plan(rect, arrivee, altitude) else { return };
                painter.line_segment([ce, de], egui::Stroke::new(1.2, bleu));
                painter.line_segment([ce, ae], egui::Stroke::new(1.8, orange));
                // rapporteur : arc de l'angle à l'écran
                let r = ce.distance(de).clamp(30.0, 90.0);
                let a0 = (de - ce).angle();
                let a1 = (ae - ce).angle();
                let mut balayage = a1 - a0;
                while balayage > std::f32::consts::PI { balayage -= std::f32::consts::TAU; }
                while balayage < -std::f32::consts::PI { balayage += std::f32::consts::TAU; }
                let pts: Vec<egui::Pos2> = (0..=24).map(|k| ce + egui::Vec2::angled(a0 + balayage * k as f32 / 24.0) * r).collect();
                painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, orange)));
                let milieu = ce + egui::Vec2::angled(a0 + balayage / 2.0) * (r + 16.0);
                painter.text(milieu, egui::Align2::CENTER_CENTER, format!("{angle:.1}°"), egui::FontId::proportional(13.0), orange);
                fantome(self, &|v| tourner_point(v, c, angle));
            }
            Outil::Aligner => {
                let Some((projet, _)) = &self.natif else { return };
                let trait_axe = |id: &str, couleur: Color32, epaisseur: f32| {
                    if let Some((a, b, z)) = projet.axe_element(id) {
                        if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) {
                            painter.add(egui::Shape::dashed_line(&[x, y], egui::Stroke::new(epaisseur, couleur), 8.0, 4.0));
                        }
                    }
                };
                if let Some(r) = &self.transfo.reference {
                    trait_axe(r, bleu, 2.0);
                }
                // survol : la cible du prochain clic est surlignée avant le clic
                if let Some(id) = cible.filter(|id| Some(id) != self.transfo.reference.as_ref()) {
                    trait_axe(&id, if self.transfo.reference.is_some() { orange } else { bleu }, 3.0);
                }
            }
            _ => {}
        }
    }

    /// État de la transformation pour le pilote : étape, consigne, centre à l'écran.
    pub(super) fn etat_transformation(&self) -> serde_json::Value {
        let Some(outil) = self.transfo.outil else { return serde_json::Value::Null };
        let rect = self.rect_vue;
        let altitude = self.altitude_vue_native();
        let centre = (outil == Outil::Tourner).then(|| self.centre_rotation()).flatten();
        serde_json::json!({
            "outil": outil.nom(),
            "consigne": self.transfo.consigne(outil),
            "points": self.transfo.points,
            "centre": centre,
            "centre_ecran": centre.and_then(|c| self.ecran_plan(rect, c, altitude)).map(|e| [e.x, e.y]),
            "placer_centre": self.transfo.placer_centre,
            "reference": self.transfo.reference,
            "copie": self.transfo.copie,
            "multiple": self.transfo.multiple,
            "saisie": self.transfo.saisie,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_signes_et_accrochage() {
        assert!((angle_deg([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]) - 90.0).abs() < 1e-9);
        assert!((angle_deg([0.0, 0.0], [1.0, 0.0], [0.0, -1.0]) + 90.0).abs() < 1e-9);
        assert!((angle_deg([5.0, 5.0], [6.0, 5.0], [4.0, 5.0]).abs() - 180.0).abs() < 1e-9);
        assert_eq!(accrocher_angle(37.0, 15.0), 30.0);
        assert_eq!(accrocher_angle(-52.0, 15.0), -45.0);
    }

    #[test]
    fn distance_tapee_et_rotation() {
        let p = point_a_distance([0.0, 0.0], [3.0, 4.0], 10.0).unwrap();
        assert!((p[0] - 6.0).abs() < 1e-9 && (p[1] - 8.0).abs() < 1e-9);
        assert!(point_a_distance([1.0, 1.0], [1.0, 1.0], 5.0).is_none());
        let q = tourner_point([2.0, 0.0], [1.0, 0.0], 90.0);
        assert!((q[0] - 1.0).abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9);
        assert_eq!(lire_valeur("12,5"), Some(12.5));
        assert_eq!(lire_valeur("-"), None);
    }

    #[test]
    fn echap_recule_d_une_etape_puis_quitte() {
        let mut t = Transformation { points: vec![[0.0, 0.0]], saisie: "12".into(), reference: Some("r".into()), copie: true, ..Default::default() };
        assert!(t.reculer());
        assert!(t.saisie.is_empty() && t.points.len() == 1);
        assert!(t.reculer());
        assert!(t.points.is_empty());
        assert!(t.reculer());
        assert!(t.reference.is_none());
        assert!(!t.reculer());
        assert!(t.copie, "les options survivent");
    }
}
