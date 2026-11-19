//! Moteur unique d'accrochage et de repérage des tracés 2D : vues du projet, éditeur de
//! familles et plan de dessin actif de la vue 3D des familles. Chaque contexte fournit seulement une
//! [`SourceAccrochage`] (sa géométrie, dans le repère 2D où l'on trace) ; types de
//! points, suivi polaire, guides, saisie de distance au clavier et marqueurs sont
//! communs. Le plan de dessin actif (plan tourné, face) trace dans son propre repère (h, v) : il
//! fournit sa géométrie dans ce repère et passe par ce même moteur.
use eframe::egui;
use glam::DVec2;
use moteur2d::dao::{self as d2, Accroche};
use noyau::forme2d::Geom;

use crate::saisie::reperage::Reperage;

/// Géométrie accrochable d'un contexte de tracé.
pub(crate) trait SourceAccrochage {
    /// Tracés accrochables (extrémité, milieu, centre, quadrant, intersection,
    /// perpendiculaire, proche).
    fn geoms(&self) -> &[Geom];
    /// Droites d'accrochage infinies (origine, direction unitaire) : plans de référence
    /// d'une famille. On s'y accroche à leurs intersections (entre elles et avec les
    /// tracés) et au point le plus proche, jamais à des extrémités ou milieux.
    fn droites(&self) -> &[(DVec2, DVec2)] {
        &[]
    }
}

/// Source faite de listes déjà calculées.
#[derive(Default, Clone)]
pub(crate) struct Geometrie {
    pub geoms: Vec<Geom>,
    pub droites: Vec<(DVec2, DVec2)>,
}

impl SourceAccrochage for Geometrie {
    fn geoms(&self) -> &[Geom] {
        &self.geoms
    }
    fn droites(&self) -> &[(DVec2, DVec2)] {
        &self.droites
    }
}

/// Rayon d'accrochage à l'écran (px), toutes vues.
pub(crate) const TOLERANCE_PX: f64 = 12.0;

/// Réglages d'une visée : tolérance (unités du tracé pour ~12 px), point précédent,
/// Maj, horloge et délai de repérage (options), repère des axes, saisie clavier.
pub(crate) struct Visee<'a> {
    pub tol: f64,
    pub precedent: Option<DVec2>,
    pub maj: bool,
    pub temps: f64,
    pub delai: f64,
    /// Angle des axes du repère (plan tourné), radians.
    pub angle_axes: f64,
    pub trace_actif: bool,
    /// Texte tapé (distance, `d<a`, `@x;y`, `#dx;dy`).
    pub saisie: &'a str,
    /// mm par unité de l'utilisateur.
    pub facteur: f64,
    /// Arrondi des longueurs libres (unités du tracé).
    pub pas: f64,
}

/// Accrochage aux objets d'une source, sans repérage : points remarquables des tracés,
/// intersections des droites entre elles et avec les tracés, perpendiculaire depuis
/// `depuis`, puis point le plus proche.
pub(crate) fn accrocher(c: DVec2, tol: f64, geoms: &[Geom], droites: &[(DVec2, DVec2)], depuis: Option<DVec2>) -> Option<(DVec2, Accroche)> {
    accrocher_points(c, tol, geoms, droites, depuis).or_else(|| proche(c, tol, geoms, droites))
}

/// Points remarquables seulement (sans « proche ») : ce que le repérage peut acquérir
/// comme référence.
pub(crate) fn accrocher_points(c: DVec2, tol: f64, geoms: &[Geom], droites: &[(DVec2, DVec2)], depuis: Option<DVec2>) -> Option<(DVec2, Accroche)> {
    let objets = d2::accrocher(c, tol, geoms, depuis);
    if let Some((_, a)) = objets && matches!(a, Accroche::Extremite | Accroche::Centre | Accroche::Intersection) {
        return objets;
    }
    // intersections des droites (entre elles, puis avec les segments)
    let mut points = Vec::new();
    for (i, &(a, d)) in droites.iter().enumerate() {
        for &(b, e) in &droites[i + 1..] {
            points.extend(croisement(a, d, b, e));
        }
        for g in geoms {
            if let Geom::Segment([p, q]) = g && p.distance(*q) > 1e-9 {
                if let Some(x) = croisement(a, d, *p, (*q - *p).normalize()) && (x - *p).dot(x - *q) <= 1e-9 {
                    points.push(x);
                }
            }
        }
    }
    if let Some(p) = points.into_iter().filter(|p| p.distance(c) <= tol).min_by(|x, y| x.distance(c).total_cmp(&y.distance(c))) {
        return Some((p, Accroche::Intersection));
    }
    if objets.is_some() {
        return objets;
    }
    // perpendiculaire à une droite depuis le point précédent
    if let Some(p) = depuis.and_then(|o| droites.iter().map(|&(a, d)| a + d * (o - a).dot(d)).filter(|p| p.distance(c) <= tol && p.distance(o) > 1e-6).min_by(|x, y| x.distance(c).total_cmp(&y.distance(c)))) {
        return Some((p, Accroche::Perpendiculaire));
    }
    None
}

/// Point le plus proche sur un tracé ou une droite, à mi-tolérance.
pub(crate) fn proche(c: DVec2, tol: f64, geoms: &[Geom], droites: &[(DVec2, DVec2)]) -> Option<(DVec2, Accroche)> {
    let sur_droite = droites.iter().map(|&(a, d)| a + d * (c - a).dot(d));
    geoms.iter().filter_map(|g| d2::proche(c, g)).chain(sur_droite).filter(|p| p.distance(c) <= tol * 0.5)
        .min_by(|x, y| x.distance(c).total_cmp(&y.distance(c))).map(|p| (p, Accroche::Proche))
}

fn croisement(a: DVec2, d: DVec2, b: DVec2, e: DVec2) -> Option<DVec2> {
    let det = d.perp_dot(e);
    (det.abs() > 1e-9).then(|| a + d * ((b - a).perp_dot(e) / det))
}

/// Contrainte depuis le point précédent quand rien n'est accroché : Maj, ortho dans le
/// repère ; sinon suivi polaire tous les 45° (à 2,5° près) ; sinon libre. Longueurs
/// arrondies à `pas` (premier point : coordonnées arrondies).
pub(crate) fn contraindre(c: DVec2, precedent: Option<DVec2>, maj: bool, angle_axes: f64, pas: f64) -> (DVec2, Accroche) {
    let tourner = |v: DVec2, a: f64| DVec2::from_angle(a).rotate(v);
    let arrondi = |v: DVec2| {
        let l = v.length();
        if l < 1e-9 { v } else { v * ((l / pas).round() * pas / l) }
    };
    match precedent {
        Some(p) if maj => (p + arrondi(tourner(d2::ortho(DVec2::ZERO, tourner(c - p, -angle_axes)), angle_axes)), Accroche::Ortho),
        Some(p) => match d2::polaire(DVec2::ZERO, tourner(c - p, -angle_axes), 45.0, 2.5) {
            Some((q, a)) => (p + arrondi(tourner(q, angle_axes)), Accroche::Polaire(a)),
            None => (p + arrondi(c - p), Accroche::Libre),
        },
        None => ((c / pas).round() * pas, Accroche::Libre),
    }
}

/// Visée sans repérage (points autres que celui sous le curseur : début d'un glisser,
/// poignée) : accrochage aux objets, sinon contrainte depuis `depuis`.
pub(crate) fn viser_simple(source: &dyn SourceAccrochage, c: DVec2, tol: f64, depuis: Option<DVec2>, maj: bool, angle_axes: f64, pas: f64) -> (DVec2, Accroche) {
    if !maj && let Some(r) = accrocher(c, tol, source.geoms(), source.droites(), depuis) {
        return r;
    }
    contraindre(c, depuis, maj, angle_axes, pas)
}

/// Visée du curseur : angle tapé, accrochage et repérage (guides, références acquises
/// après le délai des options), contrainte, puis distance tapée portée sur la direction
/// visée ou à l'intersection du cercle de cette distance avec un guide.
pub(crate) fn viser(reperage: &mut Reperage, source: &dyn SourceAccrochage, c: DVec2, v: &Visee) -> (DVec2, Accroche) {
    let r = viser_distance(reperage, source, c, v);
    reperage.vise = Some(r);
    r
}

fn viser_distance(reperage: &mut Reperage, source: &dyn SourceAccrochage, c: DVec2, v: &Visee) -> (DVec2, Accroche) {
    let (point, accroche) = viser_sans_distance(reperage, source, c, v);
    let (Some(depart), Some(distance)) = (v.precedent, distance_saisie(v.saisie, v.facteur)) else { return (point, accroche) };
    if !matches!(accroche, Accroche::Polaire(_)) && let Some(cible) = reperage.guides.iter()
        .flat_map(|&(origine, direction)| intersections_cercle_guide(depart, distance, origine, direction))
        .filter(|p| p.distance(c) <= v.tol)
        .min_by(|a, b| a.distance(c).total_cmp(&b.distance(c)))
    { return (cible, Accroche::Intersection); }
    let direction = point - depart;
    if direction.length_squared() > 1e-12 {
        let cible = depart + direction.normalize() * distance;
        let deplace = cible.distance(point) > 1e-6;
        reperage.guides.retain(|(a, d)| (cible - *a).perp_dot(*d).abs() < 1e-5);
        return (cible, if deplace && !matches!(accroche, Accroche::Polaire(_) | Accroche::Ortho) { Accroche::Libre } else { accroche });
    }
    (point, accroche)
}

fn viser_sans_distance(reperage: &mut Reperage, source: &dyn SourceAccrochage, c: DVec2, v: &Visee) -> (DVec2, Accroche) {
    // une valeur d'angle en cours de saisie verrouille le guide dès qu'elle est lisible
    if let Some(p) = v.precedent
        && let Some((point, Some(angle))) = crate::saisie::saisie_trace::apercu_point(v.saisie, Some(p), c, v.facteur)
    {
        reperage.survol_accroche = None;
        reperage.survol_segment = None;
        reperage.guides = vec![(p, DVec2::from_angle(angle.to_radians()))];
        return (point, Accroche::Polaire(angle));
    }
    let (geoms, droites) = (source.geoms(), source.droites());
    if let Some(point) = reperage.calculer(c, v.tol, geoms, droites, v.precedent, v.maj, v.temps, v.delai, v.angle_axes, v.trace_actif) {
        return point;
    }
    if !v.maj && let Some(p) = proche(c, v.tol, geoms, droites) {
        return p;
    }
    contraindre(c, v.precedent, v.maj, v.angle_axes, v.pas)
}

/// Distance seule tapée (ni coordonnées ni angle), en mm.
pub(crate) fn distance_saisie(saisie: &str, facteur: f64) -> Option<f64> {
    if saisie.starts_with(['@', '#']) { return None; }
    let distance = saisie.split(['<', '>']).next()?.replace(',', ".").parse::<f64>().ok()? * facteur;
    (distance.is_finite() && distance > 0.0).then_some(distance)
}

pub(crate) fn intersections_cercle_guide(centre: DVec2, rayon: f64, origine: DVec2, direction: DVec2) -> Vec<DVec2> {
    let d = direction.normalize_or_zero();
    if d == DVec2::ZERO { return Vec::new(); }
    let pied = origine + d * (centre - origine).dot(d);
    let h2 = rayon * rayon - pied.distance_squared(centre);
    if h2 < 0.0 { return Vec::new(); }
    let h = h2.sqrt();
    vec![pied + d * h, pied - d * h]
}

/// Couleur des marqueurs d'accrochage (toutes les vues).
pub(crate) const COULEUR_MARQUEUR: egui::Color32 = egui::Color32::from_rgb(20, 150, 90);

/// Marqueur d'accrochage, le même partout : carré (extrémité), triangle (milieu), cercle
/// (centre), losange (quadrant, angle d'un mur), croix (intersection), équerre
/// (perpendiculaire), sablier (proche). Rien pour un point libre, polaire ou ortho.
pub(crate) fn peindre_marqueur(painter: &egui::Painter, e: egui::Pos2, accroche: Accroche) {
    let m = 6.0;
    let t = egui::Stroke::new(2.0, COULEUR_MARQUEUR);
    let v = egui::vec2;
    match accroche {
        Accroche::Extremite => { painter.rect_stroke(egui::Rect::from_center_size(e, v(2.0 * m, 2.0 * m)), 0.0, t, egui::StrokeKind::Middle); }
        Accroche::Milieu => { painter.add(egui::Shape::closed_line(vec![e + v(0.0, -m), e + v(m, m), e + v(-m, m)], t)); }
        Accroche::Centre => { painter.circle_stroke(e, m, t); }
        Accroche::Quadrant => { painter.add(egui::Shape::closed_line(vec![e + v(0.0, -m), e + v(m, 0.0), e + v(0.0, m), e + v(-m, 0.0)], t)); }
        Accroche::Intersection => {
            painter.line_segment([e + v(-m, -m), e + v(m, m)], t);
            painter.line_segment([e + v(-m, m), e + v(m, -m)], t);
        }
        Accroche::Perpendiculaire => {
            painter.add(egui::Shape::line(vec![e + v(-m, -m), e + v(-m, m), e + v(m, m)], t));
            painter.line_segment([e + v(-m, 0.0), e + v(0.0, 0.0)], t);
        }
        Accroche::Proche => { painter.add(egui::Shape::closed_line(vec![e + v(-m, -m), e + v(m, -m), e + v(-m, m), e + v(m, m)], t)); }
        Accroche::Polaire(_) | Accroche::Ortho | Accroche::Libre => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plans de référence : intersection de deux droites, perpendiculaire, proche ; les
    /// points remarquables d'un tracé restent prioritaires.
    #[test]
    fn droites_d_accrochage() {
        let droites = [(DVec2::new(0.0, -5.0), DVec2::Y), (DVec2::new(-3.0, 100.0), DVec2::X)];
        let geoms = [Geom::Segment([DVec2::new(50.0, 0.0), DVec2::new(50.0, 300.0)])];
        assert_eq!(accrocher(DVec2::new(3.0, 98.0), 5.0, &geoms, &droites, None), Some((DVec2::new(0.0, 100.0), Accroche::Intersection)));
        assert_eq!(accrocher(DVec2::new(48.0, 101.0), 5.0, &geoms, &droites, None), Some((DVec2::new(50.0, 100.0), Accroche::Intersection)));
        assert_eq!(accrocher(DVec2::new(51.0, 152.0), 5.0, &geoms, &droites, None), Some((DVec2::new(50.0, 150.0), Accroche::Milieu)));
        assert_eq!(accrocher(DVec2::new(1.0, 30.0), 5.0, &geoms, &droites, None), Some((DVec2::new(0.0, 30.0), Accroche::Proche)));
        assert_eq!(accrocher(DVec2::new(1.0, 30.0), 5.0, &geoms, &droites, Some(DVec2::new(-40.0, 31.0))), Some((DVec2::new(0.0, 31.0), Accroche::Perpendiculaire)));
        assert!(accrocher(DVec2::new(20.0, 30.0), 5.0, &geoms, &droites, None).is_none());
    }

    #[test]
    fn contrainte_polaire_ortho_et_distance() {
        assert_eq!(contraindre(DVec2::new(1004.0, 230.0), Some(DVec2::ZERO), true, 0.0, 10.0), (DVec2::new(1000.0, 0.0), Accroche::Ortho));
        let (p, a) = contraindre(DVec2::new(1000.0, 1010.0), Some(DVec2::ZERO), false, 0.0, 10.0);
        assert_eq!(a, Accroche::Polaire(45.0));
        assert!((p.x - p.y).abs() < 1e-9);
        let mut r = Reperage::default();
        let source = Geometrie::default();
        let v = Visee { tol: 5.0, precedent: Some(DVec2::ZERO), maj: false, temps: 0.0, delai: 0.8, angle_axes: 0.0, trace_actif: true, saisie: "250", facteur: 1.0, pas: 10.0 };
        let (p, _) = viser(&mut r, &source, DVec2::new(400.0, 3.0), &v);
        assert!((p - DVec2::new(250.0, 0.0)).length() < 1e-9, "{p}");
        assert_eq!(distance_saisie("2,5>45", 1000.0), Some(2500.0));
        assert_eq!(intersections_cercle_guide(DVec2::ZERO, 5.0, DVec2::new(3.0, 0.0), DVec2::Y), vec![DVec2::new(3.0, 4.0), DVec2::new(3.0, -4.0)]);
    }
}
