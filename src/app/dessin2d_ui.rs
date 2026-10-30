//! Interface du dessin 2D : clics et saisie au clavier, accrochages, repérage polaire
//! (Maj : ortho), aperçu élastique, invite et ligne de commande en bas de la vue, à la
//! manière de CADCraft. Sert aux annotations (Documenter → Dessin 2D, dans le repère de
//! la vue native ouverte) et aux murs (Modéliser → Mur, sur le plan horizontal de leur
//! base : ligne, rectangle, arc ou cercle).
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

/// Ce que produit le tracé.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Usage {
    /// Annotations de la vue native ouverte.
    Annotation,
    /// Murs, suivant le type et les réglages de l'outil Mur.
    Mur,
    /// Élément horizontal (dalle, sol, plafond, toiture) : contour fermé.
    Surface,
    /// Escalier : ligne de foulée (palier à chaque sommet intermédiaire).
    Escalier,
    /// Garde-corps : tracé en plan.
    GardeCorps,
}

/// Contour d'élément horizontal d'une forme fermée (cercle : 48 côtés).
pub(super) fn contour_surface(f: &d2::Forme2D) -> Option<Vec<[f64; 2]>> {
    let pts: Vec<DVec2> = match f {
        d2::Forme2D::Polyligne { points, .. } | d2::Forme2D::Spline { points, .. } if points.len() >= 3 => points.clone(),
        d2::Forme2D::Cercle { centre, rayon } => {
            let mut p = d2::points_arc(*centre, *rayon, 0.0, std::f64::consts::TAU);
            p.pop();
            // 48 côtés au plus, quelle que soit la finesse de l'aperçu
            let pas = (p.len() / 48).max(1);
            p.into_iter().step_by(pas).collect()
        }
        _ => return None,
    };
    Some(pts.into_iter().map(|p| p.to_array()).collect())
}

/// Lignes tracées des murs d'une forme : un mur par segment, un mur courbe par arc, deux
/// demi-cercles pour un cercle.
pub(super) fn axes_murs(f: &d2::Forme2D) -> Vec<crate::modele::Axe> {
    use crate::modele::Axe;
    let droit = |a: DVec2, b: DVec2| Axe::Droit { debut: a.to_array(), fin: b.to_array() };
    match f {
        d2::Forme2D::Ligne([a, b]) => vec![droit(*a, *b)],
        d2::Forme2D::Polyligne { points, fermee } | d2::Forme2D::Spline { points, fermee } => {
            let mut v: Vec<Axe> = points.windows(2).map(|w| droit(w[0], w[1])).collect();
            if *fermee && points.len() >= 3 {
                v.push(droit(points[points.len() - 1], points[0]));
            }
            v
        }
        d2::Forme2D::Arc([a, b, c]) => d2::arc_3_points(*a, *b, *c)
            .map(|(o, _, _, balayage)| Axe::Arc { debut: a.to_array(), fin: c.to_array(), centre: o.to_array(), direct: balayage > 0.0 })
            .into_iter()
            .collect(),
        d2::Forme2D::Cercle { centre, rayon } => {
            let (a, b) = (*centre + DVec2::X * *rayon, *centre - DVec2::X * *rayon);
            let arc = |d: DVec2, f: DVec2| Axe::Arc { debut: d.to_array(), fin: f.to_array(), centre: centre.to_array(), direct: true };
            vec![arc(a, b), arc(b, a)]
        }
    }
}

fn nom_outil(o: OutilDessin) -> &'static str {
    match o {
        OutilDessin::Rectangle => "RECTANGLE",
        OutilDessin::Ligne => "LIGNE",
        OutilDessin::Polyligne => "POLYLIGNE",
        OutilDessin::Spline => "SPLINE",
        OutilDessin::Cercle => "CERCLE",
        OutilDessin::Arc => "ARC",
    }
}

impl ViewerApp {
    /// Outil de dessin actif : annotation ou mur.
    pub(super) fn outil_dessin(&self) -> Option<OutilDessin> {
        self.usage_dessin().map(|(_, o)| o)
    }

    /// Tracé actif et son usage : dessin 2D (Documenter, vue native ouverte) ou mur
    /// (Modéliser, projet natif chargé).
    pub(super) fn usage_dessin(&self) -> Option<(Usage, OutilDessin)> {
        use crate::ruban::Outil;
        let modeliser = self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.scene.is_some() && self.loading.is_none();
        if modeliser && self.ruban_edition.mur_actif() {
            return Some((Usage::Mur, self.ruban_edition.forme_mur));
        }
        if modeliser && self.ruban_edition.dalle_active().is_some() {
            let forme = match self.ruban_edition.forme_dalle {
                f @ (OutilDessin::Rectangle | OutilDessin::Cercle) => f,
                _ => OutilDessin::Polyligne,
            };
            return Some((Usage::Surface, forme));
        }
        if modeliser && self.ruban_edition.outil() == Some(Outil::Escalier) {
            return Some((Usage::Escalier, OutilDessin::Polyligne));
        }
        if modeliser && self.ruban_edition.outil() == Some(Outil::GardeCorps) {
            return Some((Usage::GardeCorps, OutilDessin::Polyligne));
        }
        if self.onglet_ruban != OngletRuban::Annoter || self.vue_native_id().is_none() {
            return None;
        }
        Some((Usage::Annotation, match self.ruban_edition.outil()? {
            Outil::Ligne => OutilDessin::Ligne,
            Outil::Polyligne => OutilDessin::Polyligne,
            Outil::Spline => OutilDessin::Spline,
            Outil::Cercle => OutilDessin::Cercle,
            Outil::Arc => OutilDessin::Arc,
            _ => return None,
        }))
    }

    /// Un tracé a des points en cours (les raccourcis et Suppr attendent).
    pub(super) fn trace_en_cours(&self) -> bool {
        self.dessin2d.trace.as_ref().is_some_and(|t| !t.points.is_empty())
    }

    /// Altitude de la base des murs tracés (mm) : niveau de base et décalage.
    fn altitude_murs(&self) -> f64 {
        let Some((p, _)) = &self.natif else { return 0.0 };
        let o = self.ruban_edition.parametres_mur();
        o.niveau.as_ref().and_then(|id| p.niveau(id)).or_else(|| p.niveaux.first()).map_or(0.0, |n| n.altitude) + o.decalage
    }

    /// Altitude du plan de tracé horizontal (mm) : base du mur, ou niveau et décalage de
    /// l'élément horizontal (dessus d'une dalle ou d'un sol, sous-face d'un plafond).
    fn altitude_trace(&self) -> f64 {
        match self.usage_dessin().map(|(u, _)| u) {
            Some(Usage::Escalier | Usage::GardeCorps) => {
                let Some((p, _)) = &self.natif else { return 0.0 };
                let decalage = if self.usage_dessin().is_some_and(|(u, _)| u == Usage::GardeCorps) { self.ruban_edition.nombre_option(crate::ruban::Outil::GardeCorps, 1) } else { 0.0 };
                self.niveau_trace().and_then(|n| p.niveau(&n)).map_or(0.0, |n| n.altitude) + decalage
            }
            Some(Usage::Surface) => {
                let (Some((p, _)), Some(d)) = (&self.natif, self.ruban_edition.dalle_active()) else { return 0.0 };
                d.niveau.as_ref().and_then(|id| p.niveau(id)).or_else(|| p.niveaux.first()).map_or(0.0, |n| n.altitude) + d.decalage
            }
            _ => self.altitude_murs(),
        }
    }

    /// Repère du tracé : origine, droite, haut (mm). Annotation : celui de la vue native
    /// ouverte ; mur ou élément horizontal : plan horizontal de son altitude.
    fn repere_dessin(&self) -> Option<(DVec3, DVec3, DVec3)> {
        match self.usage_dessin()?.0 {
            Usage::Mur | Usage::Surface | Usage::Escalier | Usage::GardeCorps => Some((DVec3::new(0.0, 0.0, self.altitude_trace()), DVec3::X, DVec3::Y)),
            Usage::Annotation => {
                let id = self.vue_native_id()?;
                let p = &self.natif.as_ref()?.0;
                let v = p.vues.iter().find(|v| v.id == id)?;
                Some(crate::modele::documentation::repere(p, v))
            }
        }
    }

    /// Point du modèle (mm) à l'écran.
    fn ecran_3d(&self, rect: egui::Rect, p: DVec3) -> Option<egui::Pos2> {
        let s = self.scene.as_ref()?;
        let w = (p * 0.001 - s.origin).as_vec3();
        let c = self.camera.view_proj(rect.width() / rect.height().max(1.0)) * w.extend(1.0);
        (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
    }

    fn vers_ecran(&self, rect: egui::Rect, q: DVec2) -> Option<egui::Pos2> {
        let (o, u, h) = self.repere_dessin()?;
        self.ecran_3d(rect, o + u * q.x + h * q.y)
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

    /// Géométries d'accrochage. Annotation : dessins de la vue et, en plan, murs (contours
    /// et axes), éléments horizontaux et quadrillages de son niveau. Mur : murs qui
    /// traversent le plan de tracé, éléments horizontaux, quadrillages et dessins de la
    /// vue en plan ouverte (calque de tracé).
    fn geoms_dessin(&mut self) -> Vec<Geom> {
        let Some((usage, _)) = self.usage_dessin() else { return Vec::new() };
        let vue = self.vue_native_id();
        let altitude = self.altitude_trace();
        let cle = format!("{usage:?}|{}|{altitude}", vue.as_deref().unwrap_or(""));
        if let Some((r, k, g)) = &self.dessin2d.geoms {
            if *r == self.bim.revision && *k == cle {
                return g.clone();
            }
        }
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let plan = vue.as_ref().and_then(|id| p.vues.iter().find(|v| &v.id == id)).and_then(|v| match &v.genre {
            crate::modele::GenreVue::Plan { niveau, .. } => Some(niveau.clone()),
            _ => None,
        });
        let mut g = Vec::new();
        // dessins de la vue : toujours pour une annotation, en plan pour un mur
        if let Some(vue) = vue.as_ref().filter(|_| usage == Usage::Annotation || plan.is_some()) {
            for a in p.annotations.iter().filter(|a| &a.vue == vue) {
                if let Some(f) = a.forme.forme_2d() {
                    g.extend(f.geoms());
                }
            }
        }
        let modele = usage != Usage::Annotation || plan.is_some();
        if modele {
            let seg = |a: [f64; 2], b: [f64; 2]| Geom::Segment([DVec2::from_array(a), DVec2::from_array(b)]);
            let coupes = crate::modele::jonctions::coupes(p);
            let retenu = |m: &crate::modele::Mur| match usage {
                Usage::Annotation => plan.as_ref() == Some(&m.niveau),
                _ => p.bas_haut(m).is_some_and(|(b, h)| altitude >= b - 0.1 && altitude <= h + 0.1),
            };
            for m in p.murs.iter().filter(|m| retenu(m)) {
                if let Some(c) = crate::modele::jonctions::contour(p, m, &coupes) {
                    for k in 0..4 {
                        g.push(seg(c[k], c[(k + 1) % 4]));
                    }
                }
                match m.axe {
                    crate::modele::Axe::Droit { debut, fin } => g.push(seg(debut, fin)),
                    crate::modele::Axe::Arc { debut, fin, centre, .. } => {
                        let o = DVec2::from_array(centre);
                        let (a, b) = (DVec2::from_array(debut), DVec2::from_array(fin));
                        let r = a.distance(o);
                        let a0 = (a - o).to_angle();
                        let direct = matches!(m.axe, crate::modele::Axe::Arc { direct: true, .. });
                        let mut balayage = (b - o).to_angle() - a0;
                        balayage = if direct { balayage.rem_euclid(std::f64::consts::TAU) } else { -(-balayage).rem_euclid(std::f64::consts::TAU) };
                        g.push(Geom::Arc { centre: o, rayon: r, depart: a0, balayage });
                    }
                }
            }
            for d in p.dalles.iter().filter(|d| usage != Usage::Annotation || plan.as_ref() == Some(&d.niveau)) {
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
        self.dessin2d.geoms = Some((self.bim.revision, cle, g.clone()));
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
        // repère du plan tourné : ortho et angles ronds suivent l'écran ; longueurs et
        // coordonnées libres au pas de 10 mm pour un mur, 1 mm pour un dessin
        let th = self.angle_axes_plan();
        let pas = if self.usage_dessin().is_some_and(|(u, _)| u != Usage::Annotation) { 10.0 } else { 1.0 };
        let tourner = |v: DVec2, a: f64| DVec2::from_angle(a).rotate(v);
        let arrondi = |v: DVec2| {
            let l = v.length();
            if l < 1e-9 { v } else { v * ((l / pas).round() * pas / l) }
        };
        match precedent {
            Some(p) if maj => (p + arrondi(tourner(d2::ortho(DVec2::ZERO, tourner(c - p, -th)), th)), Accroche::Ortho),
            Some(p) => match d2::polaire(DVec2::ZERO, tourner(c - p, -th), 15.0, 2.5) {
                Some((q, a)) => (p + arrondi(tourner(q, th)), Accroche::Polaire(a)),
                None => ((c / pas).round() * pas, Accroche::Libre),
            },
            None => ((c / pas).round() * pas, Accroche::Libre),
        }
    }

    /// Enregistre les formes achevées : annotations de la vue, ou murs (une étape
    /// d'annulation par forme ; le dernier mur créé est sélectionné).
    fn enregistrer_formes(&mut self, formes: Vec<d2::Forme2D>) {
        if formes.is_empty() {
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier || u == Usage::GardeCorps) {
            let escalier = self.usage_dessin().is_some_and(|(u, _)| u == Usage::Escalier);
            for f in &formes {
                let d2::Forme2D::Polyligne { points, .. } = f else { continue };
                if points.len() < 2 {
                    continue;
                }
                let trace: Vec<[f64; 2]> = points.iter().map(|p| p.to_array()).collect();
                let r = if escalier { self.creer_escalier(trace) } else { self.creer_garde_corps(trace, None).map(|_| ()) };
                if let Err(e) = r {
                    self.dessin2d.message = Some(e);
                }
            }
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Surface) {
            let Some(parametres) = self.ruban_edition.dalle_active() else { return };
            for f in &formes {
                match contour_surface(f) {
                    Some(c) => self.creer_dalle(&c, &parametres),
                    None => self.dessin2d.message = Some("Contour fermé attendu : au moins trois sommets.".into()),
                }
            }
            return;
        }
        if self.usage_dessin().is_some_and(|(u, _)| u == Usage::Mur) {
            let options = self.ruban_edition.parametres_mur();
            for f in &formes {
                let Some((projet, _)) = &self.natif else { return };
                let mut projet = projet.clone();
                let ids: Result<Vec<String>, String> = axes_murs(f).into_iter().map(|axe| projet.ajouter_mur_axe(axe, &options)).collect();
                match ids.and_then(|ids| self.appliquer_projet(projet).map(|()| ids)) {
                    Ok(ids) => {
                        if let Some(id) = ids.last() {
                            let genre = if self.natif.as_ref().is_some_and(|(p, _)| p.assemblage(id).is_some()) { "assemblage" } else { "mur" };
                            self.selected = Some(scene::id_natif(genre, id));
                            self.reveal = self.selected;
                        }
                        self.status = format!("{} mur(s) créé(s) — Ctrl+S pour enregistrer.", ids.len());
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            // sans chaînage, chaque mur part d'un nouveau point
            if let Some(t) = self.dessin2d.trace.as_mut().filter(|t| t.outil == OutilDessin::Ligne && !self.ruban_edition.chainer_murs) {
                t.points.clear();
            }
            return;
        }
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
        let Some((usage, outil)) = self.usage_dessin() else {
            self.dessin2d.trace = None;
            self.dessin2d.saisie.clear();
            return;
        };
        if self.dessin2d.trace.as_ref().is_none_or(|t| t.outil != outil) {
            if self.dessin2d.trace.is_none() && usage == Usage::Annotation {
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
        // F (fermer) seulement pendant un tracé : sinon F cadre la vue
        let en_cours = self.trace_en_cours();
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t) => {
                    let mut lu = false;
                    for ch in t.chars() {
                        if ch.is_ascii_digit() || "@<,;.-".contains(ch) || (en_cours && ch.eq_ignore_ascii_case(&'f')) {
                            self.dessin2d.saisie.push(ch);
                            lu = true;
                        } else if ch == ' ' {
                            valider = true;
                            lu = true;
                        }
                    }
                    // les autres lettres restent aux raccourcis
                    !lu
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
                egui::Event::Key { key: egui::Key::Space, .. } => false,
                egui::Event::Key { key: egui::Key::Delete, .. } => !en_cours,
                egui::Event::Key { key: egui::Key::F, .. } => !en_cours,
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
        let angle = self.angle_axes_plan();
        if let Some(t) = self.dessin2d.trace.as_mut() {
            t.angle = angle;
        }
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
                // aperçu : ligne tracée, et volume pour un mur
                let apercu = trace.apercu(p);
                match self.usage_dessin().map(|(u, _)| u) {
                    Some(Usage::Mur) => self.apercu_volume_murs(&painter, rect, &apercu),
                    Some(Usage::Surface) => self.apercu_surface(&painter, rect, &apercu),
                    Some(Usage::Escalier) => self.apercu_escalier(&painter, rect, &apercu),
                    _ => {}
                }
                for l in apercu {
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
        let nom = match self.usage_dessin() {
            Some((Usage::Mur, o)) => format!("MUR · {}", nom_outil(o)),
            Some((Usage::Escalier, _)) => "ESCALIER · LIGNE DE FOULÉE".into(),
            Some((Usage::GardeCorps, _)) => "GARDE-CORPS".into(),
            Some((Usage::Surface, o)) => format!(
                "{} · {}",
                self.ruban_edition.dalle_active().map_or("SURFACE".into(), |d| d.sorte.nom().to_uppercase()),
                if o == OutilDessin::Polyligne { "POLYGONE" } else { nom_outil(o) }
            ),
            _ => nom_outil(trace.outil).into(),
        };
        let ligne = match &self.dessin2d.message {
            Some(m) => format!("{nom} — {} : {}", trace.invite(), m),
            None => format!("{nom} — {} : {}▏", trace.invite(), self.dessin2d.saisie),
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

    /// Volume des murs en cours : faces au sol et en tête, séparations des couches et
    /// arêtes verticales aux extrémités, le long de chaque polyligne de l'aperçu.
    fn apercu_volume_murs(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let Some((projet, _)) = &self.natif else { return };
        let options = self.ruban_edition.parametres_mur();
        let Ok((bas, haut)) = options.bornes(projet) else { return };
        let gabarit = options.gabarit(projet);
        let e = gabarit.epaisseur();
        if e <= 0.0 || haut <= bas {
            return;
        }
        // milieu de l'épaisseur : décalé de la ligne tracée selon l'implantation
        let centre = -gabarit.decalage_reference(options.implantation.reference());
        let mut separations = vec![-0.5, 0.5];
        let mut k = 0.5;
        for c in gabarit.couches.iter().take(gabarit.couches.len().saturating_sub(1)) {
            k -= c.epaisseur / e;
            separations.push(k);
        }
        let couleur = Color32::from_rgb(55, 130, 200);
        let ecran = |q: DVec2, z: f64| self.ecran_3d(rect, DVec3::new(q.x, q.y, z));
        for l in lignes {
            for w in l.windows(2) {
                let (a, b) = (w[0], w[1]);
                let Some(n) = (b - a).try_normalize().map(|d| d.perp()) else { continue };
                let c = |p: DVec2, k: f64| p + n * (centre + e * k);
                let sol: Option<Vec<_>> = [c(a, -0.5), c(b, -0.5), c(b, 0.5), c(a, 0.5)].iter().map(|&q| ecran(q, bas)).collect();
                if let Some(sol) = sol {
                    painter.add(egui::Shape::convex_polygon(sol, couleur.gamma_multiply(0.18), egui::Stroke::NONE));
                }
                for (i, &k) in separations.iter().enumerate() {
                    let trait_ = egui::Stroke::new(if i < 2 { 1.5 } else { 0.8 }, couleur.gamma_multiply(if i < 2 { 1.0 } else { 0.8 }));
                    for z in [bas, haut] {
                        if let (Some(p), Some(q)) = (ecran(c(a, k), z), ecran(c(b, k), z)) {
                            painter.line_segment([p, q], trait_);
                        }
                    }
                }
            }
            // arêtes verticales aux extrémités (pas pour une forme fermée)
            let ferme = l.len() > 2 && l.first().zip(l.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6);
            if ferme || l.len() < 2 {
                continue;
            }
            for (p, voisin) in [(l[0], l[1]), (l[l.len() - 1], l[l.len() - 2])] {
                let Some(n) = (p - voisin).try_normalize().map(|d| d.perp()) else { continue };
                for k in [-0.5, 0.5] {
                    let q = p + n * (centre + e * k);
                    if let (Some(x), Some(y)) = (ecran(q, bas), ecran(q, haut)) {
                        painter.line_segment([x, y], egui::Stroke::new(1.0, couleur));
                    }
                }
            }
        }
    }

    /// Surface en cours : remplissage et aire du contour (fermé sur le curseur).
    fn apercu_surface(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let couleur = Color32::from_rgb(55, 130, 200);
        for l in lignes {
            let mut pts = l.clone();
            if pts.len() > 2 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-6) {
                pts.pop();
            }
            if pts.len() < 3 {
                continue;
            }
            let Some(ecr) = pts.iter().map(|&q| self.vers_ecran(rect, q)).collect::<Option<Vec<_>>>() else { continue };
            let contour: Vec<[f64; 2]> = pts.iter().map(|p| p.to_array()).collect();
            let aire = crate::modele::geometrie::aire_pair_impair(std::slice::from_ref(&contour));
            let mut m = egui::Mesh::default();
            for t in crate::texte::trianguler(vec![ecr.iter().map(|p| [p.x, p.y]).collect()]) {
                for p in t {
                    m.colored_vertex(egui::pos2(p[0], p[1]), couleur.gamma_multiply(0.18));
                }
            }
            m.indices = (0..m.vertices.len() as u32).collect();
            painter.add(egui::Shape::mesh(m));
            let mut ferme = ecr.clone();
            ferme.push(ecr[0]);
            painter.add(egui::Shape::dashed_line(&ferme[ferme.len() - 2..], egui::Stroke::new(1.0, couleur), 5.0, 4.0));
            let c = ecr.iter().fold(egui::Vec2::ZERO, |s, p| s + p.to_vec2()) / ecr.len() as f32;
            let galee = painter.layout_no_wrap(format!("{:.2} m²", aire / 1e6), egui::FontId::proportional(13.0), Color32::from_gray(30));
            let cadre = egui::Rect::from_center_size(c.to_pos2(), galee.size() + egui::vec2(12.0, 6.0));
            painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 235), egui::Stroke::new(1.0, couleur), egui::StrokeKind::Inside);
            painter.galley(cadre.min + egui::vec2(6.0, 3.0), galee, Color32::from_gray(30));
        }
    }

    /// Barre d'options du tracé, en haut de la vue : forme du mur (et chaînage) ou de
    /// l'élément horizontal.
    pub(super) fn options_trace(&mut self, ui: &mut egui::Ui) {
        match self.usage_dessin().map(|(u, _)| u) {
            Some(Usage::Mur) => self.options_trace_mur(ui),
            Some(Usage::Escalier) => {
                ui.label("Ligne de foulée : départ, un sommet par palier, arrivée ; Entrée ou clic droit pour finir. Le nombre de marches suit la hauteur du niveau supérieur.");
            }
            Some(Usage::GardeCorps) => {
                ui.label("Tracé du garde-corps : cliquer les sommets ; Entrée ou clic droit pour finir. Pour un escalier : le sélectionner, puis « Garde-corps » dans la barre d'options.");
            }
            Some(Usage::Surface) => {
                ui.horizontal(|ui| {
                    ui.label("Forme");
                    let actuelle = self.usage_dessin().map(|(_, o)| o);
                    for (forme, nom, aide) in [
                        (OutilDessin::Polyligne, "Polygone", "Cliquer les sommets ; recliquer le premier, F, Entrée ou clic droit pour fermer"),
                        (OutilDessin::Rectangle, "Rectangle", "Deux coins opposés, dans les axes du plan"),
                        (OutilDessin::Cercle, "Cercle", "Centre puis rayon (contour à 48 côtés)"),
                    ] {
                        if ui.selectable_label(actuelle == Some(forme), nom).on_hover_text(aide).clicked() {
                            self.ruban_edition.forme_dalle = forme;
                        }
                    }
                });
            }
            _ => {}
        }
    }

    /// Barre d'options de l'outil Mur, en haut de la vue : forme tracée et chaînage.
    fn options_trace_mur(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Forme");
            for (forme, nom, aide) in [
                (OutilDessin::Ligne, "Ligne", "Murs droits, point par point"),
                (OutilDessin::Rectangle, "Rectangle", "Quatre murs par deux coins opposés"),
                (OutilDessin::Arc, "Arc", "Mur courbe : départ, point de passage, fin"),
                (OutilDessin::Cercle, "Cercle", "Deux murs en demi-cercle : centre puis rayon"),
            ] {
                if ui.selectable_label(self.ruban_edition.forme_mur == forme, nom).on_hover_text(aide).clicked() {
                    self.ruban_edition.forme_mur = forme;
                }
            }
            ui.separator();
            ui.add_enabled(self.ruban_edition.forme_mur == OutilDessin::Ligne, egui::Checkbox::new(&mut self.ruban_edition.chainer_murs, "Chaîner"))
                .on_hover_text("Le mur suivant part de la fin du précédent");
        });
    }

    /// Niveau du tracé d'un escalier ou d'un garde-corps : celui du plan ouvert, sinon
    /// le plus bas.
    pub(super) fn niveau_trace(&self) -> Option<String> {
        let (p, _) = self.natif.as_ref()?;
        let vue = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id));
        if let Some(crate::modele::GenreVue::Plan { niveau, .. }) = vue.map(|v| &v.genre) {
            return Some(niveau.clone());
        }
        p.niveaux.iter().min_by(|a, b| a.altitude.total_cmp(&b.altitude)).map(|n| n.id.clone())
    }

    /// Crée un escalier de ligne de foulée `trace` : du niveau de tracé au niveau
    /// immédiatement au-dessus (sinon 2,80 m), avec ses garde-corps selon le ruban.
    fn creer_escalier(&mut self, trace: Vec<[f64; 2]>) -> Result<(), String> {
        use crate::ruban::Outil;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let niveau = self.niveau_trace().ok_or("Créez d'abord un niveau.")?;
        let alt = p.niveau(&niveau).map_or(0.0, |n| n.altitude);
        let haut = p.niveaux.iter().filter(|n| n.altitude > alt + 1.0).min_by(|a, b| a.altitude.total_cmp(&b.altitude)).map(|n| n.id.clone());
        let escalier = crate::modele::Escalier {
            id: String::new(),
            nom: format!("Escalier {}", p.escaliers.len() + 1),
            niveau: niveau.clone(),
            niveau_haut: haut,
            decalage_bas: 0.0,
            hauteur: 2800.0,
            trace,
            largeur: self.ruban_edition.nombre_option(Outil::Escalier, 0),
            hauteur_marche: self.ruban_edition.nombre_option(Outil::Escalier, 1),
            paillasse: 150.0,
        };
        let id = self.executer_bim(Commande::Escalier { escalier })?.into_iter().next().ok_or("Escalier non créé.")?;
        let cotes = match self.ruban_edition.texte_option(Outil::Escalier, 2).as_str() {
            "Gauche" => vec![0],
            "Droite" => vec![1],
            "Aucun" => vec![],
            _ => vec![0, 1],
        };
        self.garde_corps_escalier(&id, &cotes)?;
        self.status = "Escalier créé — Ctrl+S pour enregistrer.".into();
        Ok(())
    }

    /// Garde-corps hébergés par l'escalier `id` : côtés 0 (gauche) et 1 (droite) du
    /// sens de montée, à 50 mm du bord.
    pub(super) fn garde_corps_escalier(&mut self, id: &str, cotes: &[usize]) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let e = p.escaliers.iter().find(|e| e.id == id).ok_or("Escalier absent.")?.clone();
        let traces = e.traces_garde_corps(50.0);
        for &k in cotes {
            self.creer_garde_corps(traces[k].clone(), Some(e.id.clone()))?;
        }
        Ok(())
    }

    /// Crée un garde-corps (réglages du ruban), hébergé ou non par un escalier.
    fn creer_garde_corps(&mut self, trace: Vec<[f64; 2]>, escalier: Option<String>) -> Result<String, String> {
        use crate::modele::Remplissage;
        use crate::ruban::Outil;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let niveau = match escalier.as_ref().and_then(|id| p.escaliers.iter().find(|e| &e.id == id)) {
            Some(e) => e.niveau.clone(),
            None => self.niveau_trace().ok_or("Créez d'abord un niveau.")?,
        };
        let remplissage = match self.ruban_edition.texte_option(Outil::GardeCorps, 2).as_str() {
            "Panneau plein" => Remplissage::Plein,
            "Vitrage" => Remplissage::Vitre,
            _ => Remplissage::Barreaudage,
        };
        let garde_corps = crate::modele::GardeCorps {
            id: String::new(),
            nom: format!("Garde-corps {}", p.garde_corps.len() + 1),
            niveau,
            decalage: if escalier.is_some() { 0.0 } else { self.ruban_edition.nombre_option(Outil::GardeCorps, 1) },
            trace,
            hauteur: self.ruban_edition.nombre_option(Outil::GardeCorps, 0),
            remplissage,
            escalier,
        };
        self.executer_bim(Commande::GardeCorps { garde_corps })?.into_iter().next().ok_or_else(|| "Garde-corps non créé.".to_string())
    }

    /// Escalier en cours : emmarchement (bords de la largeur) et flèche de montée.
    fn apercu_escalier(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let w = self.ruban_edition.nombre_option(crate::ruban::Outil::Escalier, 0) / 2.0;
        let couleur = Color32::from_rgb(55, 130, 200);
        for l in lignes.iter().filter(|l| l.len() >= 2) {
            let trace: Vec<[f64; 2]> = l.iter().map(|p| p.to_array()).collect();
            for d in [w, -w] {
                let bord: Vec<egui::Pos2> = crate::modele::escalier::decaler_polyligne(&trace, d).iter().filter_map(|q| self.vers_ecran(rect, DVec2::from_array(*q))).collect();
                painter.add(egui::Shape::line(bord, egui::Stroke::new(1.5, couleur)));
            }
            // flèche à l'arrivée
            let (a, b) = (l[l.len() - 2], l[l.len() - 1]);
            if let (Some(pa), Some(pb)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) {
                let d = (pb - pa).normalized() * 12.0;
                let n = egui::vec2(-d.y, d.x) * 0.5;
                painter.add(egui::Shape::convex_polygon(vec![pb, pb - d + n, pb - d - n], couleur, egui::Stroke::NONE));
            }
        }
    }
}
