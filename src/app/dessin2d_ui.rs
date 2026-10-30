//! Interface du dessin 2D (Documenter → Dessin 2D) : clics et saisie au clavier dans la
//! vue native ouverte, accrochages, repérage polaire (Maj : ortho), aperçu élastique,
//! invite et ligne de commande en bas de la vue, à la manière de CADCraft.
use super::*;
use crate::dessin2d::{self as d2, Accroche, Geom, OutilDessin, Trace};
use crate::modele::commandes::Commande;
use crate::modele::documentation::{Annotation, AnnotationForme};
use glam::{DVec2, DVec3};

/// État du dessin 2D : tracé en cours, saisie au clavier, point sous le curseur (brut
/// et accroché), géométries d'accrochage de la vue (révision, vue).
#[derive(Default)]
pub(super) struct EtatDessin2D {
    pub trace: Option<Trace>,
    pub saisie: String,
    curseur: Option<DVec2>,
    point: Option<(DVec2, Accroche)>,
    geoms: Option<(u64, String, Vec<Geom>)>,
    pub message: Option<String>,
}

fn nom_outil(o: OutilDessin) -> &'static str {
    match o {
        OutilDessin::Ligne => "LIGNE",
        OutilDessin::Polyligne => "POLYLIGNE",
        OutilDessin::Spline => "SPLINE",
        OutilDessin::Cercle => "CERCLE",
        OutilDessin::Arc => "ARC",
    }
}

impl ViewerApp {
    /// Outil de dessin 2D actif (Documenter, vue native ouverte).
    pub(super) fn outil_dessin(&self) -> Option<OutilDessin> {
        use crate::ruban::Outil;
        if self.onglet_ruban != OngletRuban::Annoter || self.vue_native_id().is_none() {
            return None;
        }
        Some(match self.ruban_edition.outil()? {
            Outil::Ligne => OutilDessin::Ligne,
            Outil::Polyligne => OutilDessin::Polyligne,
            Outil::Spline => OutilDessin::Spline,
            Outil::Cercle => OutilDessin::Cercle,
            Outil::Arc => OutilDessin::Arc,
            _ => return None,
        })
    }

    /// Repère de la vue native ouverte : origine, droite, haut (mm).
    fn repere_dessin(&self) -> Option<(DVec3, DVec3, DVec3)> {
        let id = self.vue_native_id()?;
        let p = &self.natif.as_ref()?.0;
        let v = p.vues.iter().find(|v| v.id == id)?;
        Some(crate::modele::documentation::repere(p, v))
    }

    fn vers_ecran(&self, rect: egui::Rect, q: DVec2) -> Option<egui::Pos2> {
        let (o, u, h) = self.repere_dessin()?;
        let s = self.scene.as_ref()?;
        let w = ((o + u * q.x + h * q.y) * 0.001 - s.origin).as_vec3();
        let c = self.camera.view_proj(rect.width() / rect.height().max(1.0)) * w.extend(1.0);
        (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
    }

    fn depuis_ecran(&self, rect: egui::Rect, q: egui::Pos2) -> Option<DVec2> {
        let (o, u, h) = self.repere_dessin()?;
        let s = self.scene.as_ref()?;
        let x = (q.x - rect.left()) / rect.width() * 2.0 - 1.0;
        let y = 1.0 - (q.y - rect.top()) / rect.height() * 2.0;
        let (r0, dir) = self.camera.ray(x, y, rect.width() / rect.height().max(1.0));
        let (r0, dir) = ((r0.as_dvec3() + s.origin) * 1000.0, dir.as_dvec3());
        let n = u.cross(h);
        let den = n.dot(dir);
        if den.abs() < 1e-12 {
            return None;
        }
        let t = n.dot(o - r0) / den;
        let p = r0 + dir * t - o;
        Some(DVec2::new(p.dot(u), p.dot(h)))
    }

    /// Géométries d'accrochage de la vue : annotations du dessin 2D, et en plan les murs
    /// (contours et axes), éléments horizontaux et quadrillages.
    fn geoms_dessin(&mut self) -> Vec<Geom> {
        let Some(vue) = self.vue_native_id() else { return Vec::new() };
        if let Some((r, v, g)) = &self.dessin2d.geoms {
            if *r == self.bim.revision && *v == vue {
                return g.clone();
            }
        }
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let mut g = Vec::new();
        for a in p.annotations.iter().filter(|a| a.vue == vue) {
            if let Some(f) = a.forme.forme_2d() {
                g.extend(f.geoms());
            }
        }
        let niveau = p.vues.iter().find(|v| v.id == vue).and_then(|v| match &v.genre {
            crate::modele::GenreVue::Plan { niveau, .. } => Some(niveau.clone()),
            _ => None,
        });
        if let Some(niveau) = niveau {
            let seg = |a: [f64; 2], b: [f64; 2]| Geom::Segment([DVec2::from_array(a), DVec2::from_array(b)]);
            let coupes = crate::modele::jonctions::coupes(p);
            for m in p.murs.iter().filter(|m| m.niveau == niveau) {
                if let Some(c) = crate::modele::jonctions::contour(p, m, &coupes) {
                    for k in 0..4 {
                        g.push(seg(c[k], c[(k + 1) % 4]));
                    }
                }
                if let crate::modele::Axe::Droit { debut, fin } = m.axe {
                    g.push(seg(debut, fin));
                }
            }
            for d in p.dalles.iter().filter(|d| d.niveau == niveau) {
                for c in &d.contours {
                    for k in 0..c.len() {
                        g.push(seg(c[k], c[(k + 1) % c.len()]));
                    }
                }
            }
            for q in &p.quadrillages {
                g.push(seg(q.debut, q.fin));
            }
        }
        self.dessin2d.geoms = Some((self.bim.revision, vue, g.clone()));
        g
    }

    /// Point retenu pour le curseur `c` : accrochage, sinon ortho (Maj) ou repérage
    /// polaire (tous les 15°) depuis le point précédent, sinon le point libre.
    fn point_dessin(&mut self, rect: egui::Rect, c: DVec2, maj: bool) -> (DVec2, Accroche) {
        // tolérance : 12 pixels en mm du plan de la vue
        let tol = self.vers_ecran(rect, c).zip(self.vers_ecran(rect, c + DVec2::X * 1000.0)).map_or(200.0, |(a, b)| 12.0 * 1000.0 / (a.distance(b) as f64).max(1e-6));
        let precedent = self.dessin2d.trace.as_ref().and_then(Trace::precedent);
        let mut geoms = self.geoms_dessin();
        if let Some(t) = &self.dessin2d.trace {
            geoms.extend(t.points.windows(2).map(|w| Geom::Segment([w[0], w[1]])));
            if let [p] = t.points.as_slice() {
                geoms.push(Geom::Segment([*p, *p]));
            }
        }
        if !maj {
            if let Some(a) = d2::accrocher(c, tol, &geoms, precedent) {
                return a;
            }
        }
        match precedent {
            Some(p) if maj => (d2::ortho(p, c), Accroche::Ortho),
            Some(p) => d2::polaire(p, c, 15.0, 2.5).map_or((c, Accroche::Libre), |(q, a)| (q, Accroche::Polaire(a))),
            None => (c, Accroche::Libre),
        }
    }

    /// Enregistre les formes achevées comme annotations de la vue.
    fn enregistrer_formes(&mut self, formes: Vec<d2::Forme2D>) {
        let Some(vue) = self.vue_native_id() else { return };
        for f in formes {
            let annotation = Annotation { id: String::new(), vue: vue.clone(), hauteur: 2.5, forme: AnnotationForme::depuis_forme_2d(&f) };
            if let Err(e) = self.executer_bim(Commande::Annoter { annotation }) {
                self.error = Some(e);
            }
        }
        self.bim.selection.clear();
    }

    /// Clavier du dessin 2D, lu avant les raccourcis : chiffres et `@ < , ; . -` dans la
    /// ligne de commande, F fermer, Entrée ou Espace valider (vide : terminer),
    /// Retour arrière, Échap (saisie, puis tracé en cours, puis outil).
    pub(super) fn clavier_dessin(&mut self, ctx: &egui::Context) {
        let Some(outil) = self.outil_dessin() else {
            self.dessin2d.trace = None;
            self.dessin2d.saisie.clear();
            return;
        };
        if self.dessin2d.trace.as_ref().is_none_or(|t| t.outil != outil) {
            if self.dessin2d.trace.is_none() {
                self.select(None, false);
            }
            self.dessin2d.trace = Some(Trace::new(outil));
            self.dessin2d.saisie.clear();
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut valider = false;
        let mut echap = false;
        let mut effacer = false;
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        if ch.is_ascii_digit() || "@<,;.-".contains(ch) || ch.eq_ignore_ascii_case(&'f') {
                            self.dessin2d.saisie.push(ch);
                        } else if ch == ' ' {
                            valider = true;
                        }
                    }
                    false
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    valider = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => {
                    echap = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                    effacer = true;
                    false
                }
                egui::Event::Key { key: egui::Key::Space | egui::Key::F | egui::Key::Delete, .. } => false,
                _ => true,
            });
        });
        if effacer {
            self.dessin2d.saisie.pop();
        }
        if echap {
            self.dessin2d.message = None;
            if !self.dessin2d.saisie.is_empty() {
                self.dessin2d.saisie.clear();
            } else if let Some(t) = self.dessin2d.trace.as_mut().filter(|t| !t.points.is_empty()) {
                // Échap abandonne le tracé en cours ; la ligne garde ses segments déjà créés
                t.points.clear();
            } else {
                self.ruban_edition.annuler();
                self.dessin2d.trace = None;
            }
        }
        if valider {
            self.valider_saisie_dessin();
        }
    }

    /// Entrée : point saisi (coordonnées, distance), mot-clé F (fermer), ou fin du tracé.
    fn valider_saisie_dessin(&mut self) {
        let saisie = std::mem::take(&mut self.dessin2d.saisie);
        let Some(trace) = self.dessin2d.trace.as_mut() else { return };
        self.dessin2d.message = None;
        let formes = if saisie.is_empty() {
            trace.terminer()
        } else if saisie.eq_ignore_ascii_case("f") {
            trace.fermer()
        } else {
            let precedent = trace.precedent();
            let direction = self.dessin2d.point.zip(precedent).map(|((c, _), p)| c - p);
            match d2::lire_saisie(&saisie, precedent, direction, crate::options::unite().facteur()) {
                Ok(p) => trace.ajouter(p),
                Err(e) => {
                    self.dessin2d.message = Some(e);
                    Vec::new()
                }
            }
        };
        self.enregistrer_formes(formes);
    }

    /// Souris dans la vue avec un outil de dessin 2D : point accroché sous le curseur,
    /// clic gauche : point, clic droit : terminer (comme Entrée).
    pub(super) fn souris_dessin(&mut self, ui: &egui::Ui, rect: egui::Rect, resp: &egui::Response) {
        let maj = ui.input(|i| i.modifiers.shift);
        let survol = resp.hover_pos().or(resp.interact_pointer_pos());
        self.dessin2d.curseur = survol.and_then(|q| self.depuis_ecran(rect, q));
        self.dessin2d.point = self.dessin2d.curseur.map(|c| self.point_dessin(rect, c, maj));
        if resp.clicked() {
            if let (Some((p, _)), Some(t)) = (self.dessin2d.point, self.dessin2d.trace.as_mut()) {
                self.dessin2d.saisie.clear();
                self.dessin2d.message = None;
                let formes = t.ajouter(p);
                self.enregistrer_formes(formes);
            }
        }
        if resp.clicked_by(PointerButton::Secondary) {
            self.valider_saisie_dessin();
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    /// Tracé par-dessus la vue : réticule, aperçu élastique, repérage polaire, marque et
    /// nom de l'accrochage, cote dynamique, ligne de commande.
    pub(super) fn dessiner_dessin2d(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(trace) = &self.dessin2d.trace else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(0, 120, 215);
        let vert = Color32::from_rgb(20, 160, 60);
        if let Some((p, accroche)) = self.dessin2d.point {
            if let Some(e) = self.vers_ecran(rect, p) {
                // réticule
                let gris = Color32::from_rgba_unmultiplied(60, 60, 60, 160);
                painter.line_segment([e - egui::vec2(28.0, 0.0), e + egui::vec2(28.0, 0.0)], egui::Stroke::new(1.0, gris));
                painter.line_segment([e - egui::vec2(0.0, 28.0), e + egui::vec2(0.0, 28.0)], egui::Stroke::new(1.0, gris));
                // aperçu
                for l in trace.apercu(p) {
                    let pts: Vec<egui::Pos2> = l.iter().filter_map(|q| self.vers_ecran(rect, *q)).collect();
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, bleu)));
                }
                // repérage polaire / ortho : ligne de construction en pointillés
                if let (Some(prec), Accroche::Polaire(_) | Accroche::Ortho) = (trace.precedent(), accroche) {
                    if let Some(a) = self.vers_ecran(rect, prec) {
                        let dir = (e - a).normalized();
                        painter.add(egui::Shape::dashed_line(&[a - dir * 4000.0, a + dir * 4000.0], egui::Stroke::new(1.0, vert), 6.0, 4.0));
                    }
                }
                // marque d'accrochage
                let m = 6.0;
                let trait_ = egui::Stroke::new(2.0, vert);
                match accroche {
                    Accroche::Extremite => {
                        painter.rect_stroke(egui::Rect::from_center_size(e, egui::vec2(2.0 * m, 2.0 * m)), 0.0, trait_, egui::StrokeKind::Middle);
                    }
                    Accroche::Milieu => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(0.0, -m), e + egui::vec2(m, m), e + egui::vec2(-m, m)], trait_));
                    }
                    Accroche::Centre => {
                        painter.circle_stroke(e, m, trait_);
                    }
                    Accroche::Quadrant => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(0.0, -m), e + egui::vec2(m, 0.0), e + egui::vec2(0.0, m), e + egui::vec2(-m, 0.0)], trait_));
                    }
                    Accroche::Intersection => {
                        painter.line_segment([e + egui::vec2(-m, -m), e + egui::vec2(m, m)], trait_);
                        painter.line_segment([e + egui::vec2(-m, m), e + egui::vec2(m, -m)], trait_);
                    }
                    Accroche::Perpendiculaire => {
                        painter.add(egui::Shape::line(vec![e + egui::vec2(-m, -m), e + egui::vec2(-m, m), e + egui::vec2(m, m)], trait_));
                        painter.line_segment([e + egui::vec2(-m, 0.0), e + egui::vec2(0.0, 0.0)], trait_);
                    }
                    Accroche::Proche => {
                        painter.add(egui::Shape::closed_line(vec![e + egui::vec2(-m, -m), e + egui::vec2(m, -m), e + egui::vec2(-m, m), e + egui::vec2(m, m)], trait_));
                    }
                    _ => {}
                }
                // étiquettes : accrochage, longueur et angle depuis le point précédent
                let mut infos = Vec::new();
                if accroche != Accroche::Libre {
                    infos.push(accroche.libelle());
                }
                if let Some(prec) = trace.precedent() {
                    let v = p - prec;
                    let u = crate::options::unite();
                    let decimales = u.facteur().log10().round().max(0.0) as usize;
                    infos.push(format!("{:.*}{} · {:.1}°", decimales, v.length() / u.facteur(), u.suffixe(), v.y.atan2(v.x).to_degrees().rem_euclid(360.0)));
                }
                if !infos.is_empty() {
                    let texte = painter.layout_no_wrap(infos.join("   "), egui::FontId::proportional(12.0), Color32::WHITE);
                    let cadre = egui::Rect::from_min_size(e + egui::vec2(16.0, 14.0), texte.size() + egui::vec2(10.0, 4.0));
                    painter.rect_filled(cadre, 3.0, Color32::from_rgba_unmultiplied(40, 40, 40, 220));
                    painter.galley(cadre.min + egui::vec2(5.0, 2.0), texte, Color32::WHITE);
                }
            }
        }
        // points déjà donnés
        for q in &trace.points {
            if let Some(e) = self.vers_ecran(rect, *q) {
                painter.circle_filled(e, 3.0, bleu);
            }
        }
        // ligne de commande : outil, invite, saisie (ou erreur)
        let u = crate::options::unite();
        let ligne = match &self.dessin2d.message {
            Some(m) => format!("{} — {} : {}", nom_outil(trace.outil), trace.invite(), m),
            None => format!("{} — {} : {}▏", nom_outil(trace.outil), trace.invite(), self.dessin2d.saisie),
        };
        let aide = format!("Clavier : x,y · @dx,dy · d<angle · distance seule ({}) · Maj : ortho · clic droit ou Entrée : terminer · Échap : annuler", u.suffixe().trim());
        let police = egui::FontId::monospace(13.0);
        let g1 = painter.layout_no_wrap(ligne, police, if self.dessin2d.message.is_some() { Color32::from_rgb(255, 170, 150) } else { Color32::WHITE });
        let g2 = painter.layout_no_wrap(aide, egui::FontId::proportional(11.0), Color32::from_gray(200));
        let hauteur = g1.size().y + g2.size().y + 12.0;
        let bande = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - hauteur), rect.right_bottom());
        painter.rect_filled(bande, 0.0, Color32::from_rgba_unmultiplied(30, 30, 30, 225));
        let h1 = g1.size().y;
        painter.galley(bande.min + egui::vec2(10.0, 4.0), g1, Color32::WHITE);
        painter.galley(bande.min + egui::vec2(10.0, 8.0 + h1), g2, Color32::from_gray(200));
    }
}
