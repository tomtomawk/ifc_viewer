//! Dessin 2D à la manière des logiciels de DAO (CADCraft, AutoCAD) : outils ligne,
//! polyligne, spline, cercle et arc pilotés par clics et par saisie au clavier
//! (coordonnées absolues `x,y`, relatives `@dx,dy`, polaires `@d<a`, distance directe
//! dans la direction du curseur), accrochages aux objets (extrémité, milieu, centre,
//! quadrant, intersection, perpendiculaire, proche), repérage polaire et mode ortho.
//!
//! Indépendant de l'interface : coordonnées en mm dans le plan de la vue. Le même
//! moteur servira au tracé d'éléments 3D (contours d'éléments horizontaux, murs).

use glam::DVec2;

/// Géométrie à laquelle s'accrocher : segment, cercle, arc (centre, rayon, angle de
/// départ, balayage signé, radians), courbe échantillonnée (extrémités et proche
/// seulement).
#[derive(Clone, Debug)]
pub enum Geom {
    Segment([DVec2; 2]),
    Cercle { centre: DVec2, rayon: f64 },
    Arc { centre: DVec2, rayon: f64, depart: f64, balayage: f64 },
    Courbe(Vec<DVec2>),
}

/// Nature du point retenu sous le curseur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accroche {
    Extremite,
    Milieu,
    Centre,
    Quadrant,
    Intersection,
    Perpendiculaire,
    Proche,
    /// Repérage polaire : angle (degrés) depuis le point précédent.
    Polaire(f64),
    Ortho,
    Libre,
}

impl Accroche {
    pub fn libelle(self) -> String {
        match self {
            Self::Extremite => "Extrémité".into(),
            Self::Milieu => "Milieu".into(),
            Self::Centre => "Centre".into(),
            Self::Quadrant => "Quadrant".into(),
            Self::Intersection => "Intersection".into(),
            Self::Perpendiculaire => "Perpendiculaire".into(),
            Self::Proche => "Proche".into(),
            Self::Polaire(a) => format!("Polaire {a:.0}°"),
            Self::Ortho => "Ortho".into(),
            Self::Libre => String::new(),
        }
    }
}

/// Forme dessinée, prête à enregistrer.
#[derive(Clone, Debug, PartialEq)]
pub enum Forme2D {
    Ligne([DVec2; 2]),
    Polyligne { points: Vec<DVec2>, fermee: bool },
    Spline { points: Vec<DVec2>, fermee: bool },
    Cercle { centre: DVec2, rayon: f64 },
    /// Arc par trois points : départ, point de passage, fin.
    Arc([DVec2; 3]),
}

fn sur_segment(p: DVec2, a: DVec2, b: DVec2) -> DVec2 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    a + ab * t
}

/// Angle normalisé dans [0, 2π[.
fn angle_norm(a: f64) -> f64 {
    a.rem_euclid(std::f64::consts::TAU)
}

/// Vrai si l'angle `a` est dans le balayage d'un arc.
fn dans_arc(a: f64, depart: f64, balayage: f64) -> bool {
    let d = if balayage >= 0.0 { angle_norm(a - depart) } else { angle_norm(depart - a) };
    d <= balayage.abs() + 1e-9
}

/// Centre, rayon, départ et balayage signé de l'arc passant par `a`, `b`, `c` (dans cet
/// ordre) ; None si les points sont alignés.
pub fn arc_3_points(a: DVec2, b: DVec2, c: DVec2) -> Option<(DVec2, f64, f64, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 * (a - c).length_squared().max(1.0) {
        return None;
    }
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    let centre = DVec2::new((a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d, (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d);
    let rayon = (a - centre).length();
    let ang = |p: DVec2| (p - centre).y.atan2((p - centre).x);
    let (aa, ab, ac) = (ang(a), ang(b), ang(c));
    // sens trigonométrique si b est entre a et c dans ce sens
    let direct = angle_norm(ab - aa) <= angle_norm(ac - aa);
    let balayage = if direct { angle_norm(ac - aa) } else { -angle_norm(aa - ac) };
    Some((centre, rayon, aa, balayage))
}

/// Points d'un arc (centre, rayon, départ, balayage), un tous les 5° au plus.
pub fn points_arc(centre: DVec2, rayon: f64, depart: f64, balayage: f64) -> Vec<DVec2> {
    let n = ((balayage.abs() / 5f64.to_radians()).ceil() as usize).max(2);
    (0..=n).map(|k| centre + DVec2::from_angle(depart + balayage * k as f64 / n as f64) * rayon).collect()
}

/// Courbe lisse passant par les points (Catmull-Rom centripète), 12 pas par travée.
pub fn points_spline(points: &[DVec2], fermee: bool) -> Vec<DVec2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let p = |i: isize| -> DVec2 {
        if fermee {
            points[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            points[0] * 2.0 - points[1]
        } else if i as usize >= n {
            points[n - 1] * 2.0 - points[n - 2]
        } else {
            points[i as usize]
        }
    };
    let travees = if fermee { n } else { n - 1 };
    let mut out = Vec::new();
    for i in 0..travees as isize {
        let (p0, p1, p2, p3) = (p(i - 1), p(i), p(i + 1), p(i + 2));
        let t = |a: DVec2, b: DVec2, t0: f64| t0 + (b - a).length().sqrt().max(1e-6);
        let t0 = 0.0;
        let t1 = t(p0, p1, t0);
        let t2 = t(p1, p2, t1);
        let t3 = t(p2, p3, t2);
        for k in 0..12 {
            let tt = t1 + (t2 - t1) * k as f64 / 12.0;
            let a1 = p0 * ((t1 - tt) / (t1 - t0)) + p1 * ((tt - t0) / (t1 - t0));
            let a2 = p1 * ((t2 - tt) / (t2 - t1)) + p2 * ((tt - t1) / (t2 - t1));
            let a3 = p2 * ((t3 - tt) / (t3 - t2)) + p3 * ((tt - t2) / (t3 - t2));
            let b1 = a1 * ((t2 - tt) / (t2 - t0)) + a2 * ((tt - t0) / (t2 - t0));
            let b2 = a2 * ((t3 - tt) / (t3 - t1)) + a3 * ((tt - t1) / (t3 - t1));
            out.push(b1 * ((t2 - tt) / (t2 - t1)) + b2 * ((tt - t1) / (t2 - t1)));
        }
    }
    out.push(if fermee { points[0] } else { points[n - 1] });
    out
}

impl Forme2D {
    /// Polylignes qui la représentent (pour le tracé et l'export).
    pub fn polylignes(&self) -> Vec<Vec<DVec2>> {
        match self {
            Self::Ligne([a, b]) => vec![vec![*a, *b]],
            Self::Polyligne { points, fermee } => {
                let mut v = points.clone();
                if *fermee && points.len() > 2 {
                    v.push(points[0]);
                }
                vec![v]
            }
            Self::Spline { points, fermee } => vec![points_spline(points, *fermee)],
            Self::Cercle { centre, rayon } => vec![points_arc(*centre, *rayon, 0.0, std::f64::consts::TAU)],
            Self::Arc([a, b, c]) => match arc_3_points(*a, *b, *c) {
                Some((o, r, d, s)) => vec![points_arc(o, r, d, s)],
                None => vec![vec![*a, *c]],
            },
        }
    }

    /// Géométries d'accrochage.
    pub fn geoms(&self) -> Vec<Geom> {
        match self {
            Self::Ligne(s) => vec![Geom::Segment(*s)],
            Self::Polyligne { .. } => self.polylignes()[0].windows(2).map(|w| Geom::Segment([w[0], w[1]])).collect(),
            Self::Spline { .. } => vec![Geom::Courbe(self.polylignes().remove(0))],
            Self::Cercle { centre, rayon } => vec![Geom::Cercle { centre: *centre, rayon: *rayon }],
            Self::Arc([a, b, c]) => match arc_3_points(*a, *b, *c) {
                Some((centre, rayon, depart, balayage)) => vec![Geom::Arc { centre, rayon, depart, balayage }],
                None => vec![Geom::Segment([*a, *c])],
            },
        }
    }
}

/// Intersections de deux géométries (segments, cercles, arcs ; courbes ignorées).
fn intersections(g: &Geom, h: &Geom) -> Vec<DVec2> {
    let cercle = |g: &Geom| match *g {
        Geom::Cercle { centre, rayon } => Some((centre, rayon, 0.0, std::f64::consts::TAU)),
        Geom::Arc { centre, rayon, depart, balayage } => Some((centre, rayon, depart, balayage)),
        _ => None,
    };
    match (g, h) {
        (Geom::Segment([a, b]), Geom::Segment([c, d])) => {
            let (r, s) = (*b - *a, *d - *c);
            let den = r.perp_dot(s);
            if den.abs() < 1e-12 {
                return Vec::new();
            }
            let t = (*c - *a).perp_dot(s) / den;
            let u = (*c - *a).perp_dot(r) / den;
            if (-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u) { vec![*a + r * t] } else { Vec::new() }
        }
        (Geom::Segment([a, b]), _) | (_, Geom::Segment([a, b])) => {
            let Some((o, rr, dep, bal)) = cercle(if matches!(g, Geom::Segment(_)) { h } else { g }) else { return Vec::new() };
            let d = *b - *a;
            let f = *a - o;
            let (qa, qb, qc) = (d.dot(d), 2.0 * f.dot(d), f.dot(f) - rr * rr);
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 || qa == 0.0 {
                return Vec::new();
            }
            [(-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa)]
                .into_iter()
                .filter(|t| (-1e-9..=1.0 + 1e-9).contains(t))
                .map(|t| *a + d * t)
                .filter(|p| dans_arc((*p - o).y.atan2((*p - o).x), dep, bal))
                .collect()
        }
        _ => {
            let (Some((o1, r1, d1, b1)), Some((o2, r2, d2, b2))) = (cercle(g), cercle(h)) else { return Vec::new() };
            let d = (o2 - o1).length();
            if d < 1e-9 || d > r1 + r2 || d < (r1 - r2).abs() {
                return Vec::new();
            }
            let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
            let hh = (r1 * r1 - a * a).max(0.0).sqrt();
            let m = o1 + (o2 - o1) * (a / d);
            let n = (o2 - o1).perp() / d;
            [m + n * hh, m - n * hh]
                .into_iter()
                .filter(|p| dans_arc((*p - o1).y.atan2((*p - o1).x), d1, b1) && dans_arc((*p - o2).y.atan2((*p - o2).x), d2, b2))
                .collect()
        }
    }
}

/// Point accroché près du curseur `c` (à `tol` mm près) : points remarquables d'abord
/// (extrémité, centre, quadrant, intersection, puis milieu), puis perpendiculaire depuis
/// `depuis`, puis le point le plus proche sur une géométrie.
pub fn accrocher(c: DVec2, tol: f64, geoms: &[Geom], depuis: Option<DVec2>) -> Option<(DVec2, Accroche)> {
    let mut candidats: Vec<(DVec2, Accroche)> = Vec::new();
    for g in geoms {
        match g {
            Geom::Segment([a, b]) => {
                candidats.push((*a, Accroche::Extremite));
                candidats.push((*b, Accroche::Extremite));
                candidats.push(((*a + *b) * 0.5, Accroche::Milieu));
            }
            Geom::Cercle { centre, rayon } => {
                candidats.push((*centre, Accroche::Centre));
                for k in 0..4 {
                    candidats.push((*centre + DVec2::from_angle(k as f64 * std::f64::consts::FRAC_PI_2) * *rayon, Accroche::Quadrant));
                }
            }
            Geom::Arc { centre, rayon, depart, balayage } => {
                candidats.push((*centre, Accroche::Centre));
                candidats.push((*centre + DVec2::from_angle(*depart) * *rayon, Accroche::Extremite));
                candidats.push((*centre + DVec2::from_angle(depart + balayage) * *rayon, Accroche::Extremite));
                candidats.push((*centre + DVec2::from_angle(depart + balayage * 0.5) * *rayon, Accroche::Milieu));
            }
            Geom::Courbe(p) => {
                if let (Some(a), Some(b)) = (p.first(), p.last()) {
                    candidats.push((*a, Accroche::Extremite));
                    candidats.push((*b, Accroche::Extremite));
                }
            }
        }
    }
    // intersections des géométries proches du curseur seulement
    let proches: Vec<&Geom> = geoms.iter().filter(|g| distance(c, g).is_some_and(|d| d <= tol * 2.0)).collect();
    for (i, g) in proches.iter().enumerate() {
        for h in &proches[i + 1..] {
            candidats.extend(intersections(g, h).into_iter().map(|p| (p, Accroche::Intersection)));
        }
    }
    let rang = |a: Accroche| match a {
        Accroche::Extremite | Accroche::Centre | Accroche::Intersection => 0,
        Accroche::Quadrant => 1,
        _ => 2,
    };
    let meilleur = candidats.into_iter().filter(|(p, _)| p.distance(c) <= tol).min_by(|x, y| (rang(x.1), x.0.distance(c)).partial_cmp(&(rang(y.1), y.0.distance(c))).unwrap());
    if meilleur.is_some() {
        return meilleur;
    }
    if let Some(d) = depuis {
        let perp = geoms.iter().filter_map(|g| match g {
            Geom::Segment([a, b]) => {
                let ab = *b - *a;
                let t = (d - *a).dot(ab) / ab.length_squared().max(1e-12);
                ((0.0..=1.0).contains(&t)).then(|| *a + ab * t)
            }
            Geom::Cercle { centre, rayon } | Geom::Arc { centre, rayon, .. } => (d.distance(*centre) > 1e-9).then(|| *centre + (d - *centre).normalize() * *rayon),
            Geom::Courbe(_) => None,
        });
        if let Some(p) = perp.filter(|p| p.distance(c) <= tol && p.distance(d) > 1e-6).min_by(|a, b| a.distance(c).total_cmp(&b.distance(c))) {
            return Some((p, Accroche::Perpendiculaire));
        }
    }
    geoms.iter().filter_map(|g| proche(c, g)).filter(|p| p.distance(c) <= tol * 0.6).min_by(|a, b| a.distance(c).total_cmp(&b.distance(c))).map(|p| (p, Accroche::Proche))
}

/// Point de la géométrie le plus proche de `c`.
fn proche(c: DVec2, g: &Geom) -> Option<DVec2> {
    match g {
        Geom::Segment([a, b]) => Some(sur_segment(c, *a, *b)),
        Geom::Cercle { centre, rayon } => (c.distance(*centre) > 1e-9).then(|| *centre + (c - *centre).normalize() * *rayon),
        Geom::Arc { centre, rayon, depart, balayage } => {
            let a = (c - *centre).y.atan2((c - *centre).x);
            if dans_arc(a, *depart, *balayage) {
                Some(*centre + DVec2::from_angle(a) * *rayon)
            } else {
                let (p, q) = (*centre + DVec2::from_angle(*depart) * *rayon, *centre + DVec2::from_angle(depart + balayage) * *rayon);
                Some(if p.distance(c) < q.distance(c) { p } else { q })
            }
        }
        Geom::Courbe(p) => p.windows(2).map(|w| sur_segment(c, w[0], w[1])).min_by(|a, b| a.distance(c).total_cmp(&b.distance(c))),
    }
}

fn distance(c: DVec2, g: &Geom) -> Option<f64> {
    proche(c, g).map(|p| p.distance(c))
}

/// Repérage polaire : si la direction depuis `depuis` est à `tol_deg` près d'un
/// multiple de `pas_deg`, point projeté sur cette direction et son angle (degrés).
pub fn polaire(depuis: DVec2, c: DVec2, pas_deg: f64, tol_deg: f64) -> Option<(DVec2, f64)> {
    let d = c - depuis;
    if d.length() < 1e-9 {
        return None;
    }
    let a = d.y.atan2(d.x).to_degrees();
    let rond = (a / pas_deg).round() * pas_deg;
    ((a - rond).abs() <= tol_deg).then(|| {
        let u = DVec2::from_angle(rond.to_radians());
        (depuis + u * d.dot(u), rond.rem_euclid(360.0))
    })
}

/// Mode ortho : horizontale ou verticale depuis `depuis`, la plus proche du curseur.
pub fn ortho(depuis: DVec2, c: DVec2) -> DVec2 {
    let d = c - depuis;
    if d.x.abs() >= d.y.abs() { DVec2::new(c.x, depuis.y) } else { DVec2::new(depuis.x, c.y) }
}

/// Point saisi au clavier (valeurs en unités de l'utilisateur, `facteur` mm par unité) :
/// `x,y` (absolu), `@dx,dy` (relatif au point précédent), `@d<a` (polaire, degrés), ou
/// une distance seule, portée dans la direction `direction` depuis le point précédent.
pub fn lire_saisie(texte: &str, precedent: Option<DVec2>, direction: Option<DVec2>, facteur: f64) -> Result<DVec2, String> {
    let t = texte.trim().replace(' ', "");
    let nombre = |s: &str| s.replace(',', ".").parse::<f64>().map_err(|_| format!("Nombre illisible : « {s} »"));
    // coordonnées séparées par « , » (décimales avec « . ») ou par « ; » (décimales avec
    // « , » ou « . »)
    let couple = |s: &str| -> Result<DVec2, String> {
        let (x, y) = s.split_once(';').or_else(|| s.split_once(',')).ok_or("Attendu : x,y")?;
        Ok(DVec2::new(nombre(x)?, nombre(y)?) * facteur)
    };
    if let Some(rel) = t.strip_prefix('@') {
        let p = precedent.ok_or("Pas de point précédent pour une saisie relative (@).")?;
        if let Some((d, a)) = rel.split_once('<') {
            return Ok(p + DVec2::from_angle(nombre(a)?.to_radians()) * nombre(d)? * facteur);
        }
        return Ok(p + couple(rel)?);
    }
    // distance<angle sans @ : depuis le point précédent aussi (saisie dynamique)
    if let (Some((d, a)), Some(p)) = (t.split_once('<'), precedent) {
        return Ok(p + DVec2::from_angle(nombre(a)?.to_radians()) * nombre(d)? * facteur);
    }
    if t.contains(';') || t.contains(',') {
        return couple(&t);
    }
    let d = nombre(&t)?;
    let p = precedent.ok_or("Distance seule : il faut un point précédent.")?;
    let u = direction.filter(|u| u.length() > 1e-9).ok_or("Distance seule : orienter d'abord le curseur.")?.normalize();
    Ok(p + u * d * facteur)
}

/// Outil de dessin 2D.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OutilDessin {
    #[default]
    Ligne,
    Polyligne,
    Spline,
    Cercle,
    Arc,
    /// Deux coins opposés, côtés parallèles au repère du tracé (`Trace::angle`).
    Rectangle,
}

/// Tracé en cours : outil et points déjà donnés.
#[derive(Clone, Debug)]
pub struct Trace {
    pub outil: OutilDessin,
    pub points: Vec<DVec2>,
    /// Orientation des côtés du rectangle (radians) : repère du plan tourné.
    pub angle: f64,
}

impl Trace {
    pub fn new(outil: OutilDessin) -> Trace {
        Trace { outil, points: Vec::new(), angle: 0.0 }
    }

    /// Invite de la ligne de commande, avec ses mots-clés.
    pub fn invite(&self) -> &'static str {
        use OutilDessin::*;
        match (self.outil, self.points.len()) {
            (Ligne | Polyligne | Spline, 0) => "Premier point",
            (Ligne, 1) | (Polyligne, 1) => "Point suivant (Entrée : terminer)",
            (Ligne, _) | (Polyligne, _) => "Point suivant · F : fermer · Entrée : terminer",
            (Spline, 1 | 2) => "Point de passage suivant",
            (Spline, _) => "Point suivant · F : fermer · Entrée : terminer",
            (Cercle, 0) => "Centre du cercle",
            (Cercle, _) => "Rayon (point ou distance)",
            (Arc, 0) => "Début de l'arc",
            (Arc, 1) => "Point de passage de l'arc",
            (Arc, _) => "Fin de l'arc",
            (Rectangle, 0) => "Premier coin",
            (Rectangle, _) => "Coin opposé (point ou @dx,dy)",
        }
    }

    /// Point précédent (départ du repérage polaire et des saisies relatives).
    pub fn precedent(&self) -> Option<DVec2> {
        self.points.last().copied()
    }

    /// Ajoute un point ; rend les formes achevées par ce point (la ligne est une suite
    /// de segments, comme LIGNE dans AutoCAD).
    pub fn ajouter(&mut self, p: DVec2) -> Vec<Forme2D> {
        if self.points.last().is_some_and(|q| q.distance(p) < 1e-6) {
            return Vec::new();
        }
        self.points.push(p);
        use OutilDessin::*;
        match (self.outil, self.points.len()) {
            // retour sur le premier point : la ligne se ferme et s'arrête
            (Ligne, n) if n >= 4 && self.points[0].distance(p) < 1e-6 => {
                let a = self.points[n - 2];
                self.points.clear();
                vec![Forme2D::Ligne([a, p])]
            }
            (Ligne, n) if n >= 2 => vec![Forme2D::Ligne([self.points[n - 2], self.points[n - 1]])],
            // polyligne : recliquer le premier sommet ferme le contour
            (Polyligne, n) if n >= 4 && self.points[0].distance(p) < 1e-6 => {
                self.points.pop();
                self.clore(true)
            }
            (Rectangle, 2) => {
                let coins = self.rectangle(self.points[0], self.points[1]);
                self.points.clear();
                coins.map(|points| Forme2D::Polyligne { points: points.to_vec(), fermee: true }).into_iter().collect()
            }
            (Cercle, 2) => {
                let (c, q) = (self.points[0], self.points[1]);
                self.points.clear();
                vec![Forme2D::Cercle { centre: c, rayon: c.distance(q) }]
            }
            (Arc, 3) => {
                let [a, b, c] = [self.points[0], self.points[1], self.points[2]];
                self.points.clear();
                if arc_3_points(a, b, c).is_some() { vec![Forme2D::Arc([a, b, c])] } else { Vec::new() }
            }
            _ => Vec::new(),
        }
    }

    /// Entrée sans saisie : termine la polyligne ou la spline (ou la ligne).
    pub fn terminer(&mut self) -> Vec<Forme2D> {
        self.clore(false)
    }

    /// Mot-clé F : ferme la ligne (segment de retour), la polyligne ou la spline.
    pub fn fermer(&mut self) -> Vec<Forme2D> {
        self.clore(true)
    }

    fn clore(&mut self, fermee: bool) -> Vec<Forme2D> {
        let points = std::mem::take(&mut self.points);
        use OutilDessin::*;
        match self.outil {
            Ligne if fermee && points.len() >= 3 => vec![Forme2D::Ligne([points[points.len() - 1], points[0]])],
            Polyligne if points.len() >= 2 => vec![Forme2D::Polyligne { fermee: fermee && points.len() >= 3, points }],
            Spline if points.len() >= 2 => vec![Forme2D::Spline { points, fermee }],
            _ => Vec::new(),
        }
    }

    /// Coins du rectangle de diagonale `a`–`b`, côtés suivant `angle` ; None s'il est
    /// plat.
    pub fn rectangle(&self, a: DVec2, b: DVec2) -> Option<[DVec2; 4]> {
        let u = DVec2::from_angle(self.angle);
        let v = u.perp();
        let d = b - a;
        let (du, dv) = (d.dot(u), d.dot(v));
        (du.abs() > 1e-6 && dv.abs() > 1e-6).then(|| [a, a + u * du, b, a + v * dv])
    }

    /// Aperçu sous le curseur (élastique) : polylignes à dessiner.
    pub fn apercu(&self, curseur: DVec2) -> Vec<Vec<DVec2>> {
        use OutilDessin::*;
        let mut pts = self.points.clone();
        pts.push(curseur);
        match self.outil {
            Ligne if pts.len() >= 2 => vec![vec![pts[pts.len() - 2], curseur]],
            Rectangle if pts.len() == 2 => match self.rectangle(pts[0], curseur) {
                Some(c) => vec![vec![c[0], c[1], c[2], c[3], c[0]]],
                None => vec![pts],
            },
            Polyligne => vec![pts],
            Spline => vec![points_spline(&pts, false)],
            Cercle if pts.len() == 2 => vec![points_arc(pts[0], pts[0].distance(curseur), 0.0, std::f64::consts::TAU), vec![pts[0], curseur]],
            Arc if pts.len() == 2 => vec![pts],
            Arc if pts.len() == 3 => match arc_3_points(pts[0], pts[1], pts[2]) {
                Some((o, r, d, s)) => vec![points_arc(o, r, d, s)],
                None => vec![pts],
            },
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proche_de(a: DVec2, b: DVec2) -> bool {
        a.distance(b) < 1e-6
    }

    #[test]
    fn accrochages() {
        let geoms = vec![Geom::Segment([DVec2::ZERO, DVec2::new(1000.0, 0.0)]), Geom::Segment([DVec2::new(500.0, -500.0), DVec2::new(500.0, 500.0)]), Geom::Cercle { centre: DVec2::new(2000.0, 0.0), rayon: 300.0 }];
        // extrémité, milieu (à l'écart de l'intersection), centre, quadrant
        assert_eq!(accrocher(DVec2::new(990.0, 8.0), 20.0, &geoms, None), Some((DVec2::new(1000.0, 0.0), Accroche::Extremite)));
        let (p, a) = accrocher(DVec2::new(505.0, 6.0), 20.0, &geoms, None).unwrap();
        assert!(proche_de(p, DVec2::new(500.0, 0.0)) && a == Accroche::Intersection, "{p} {a:?}");
        assert_eq!(accrocher(DVec2::new(2005.0, 5.0), 20.0, &geoms, None).unwrap().1, Accroche::Centre);
        assert_eq!(accrocher(DVec2::new(2300.0, 10.0), 20.0, &geoms, None).unwrap().1, Accroche::Quadrant);
        assert_eq!(accrocher(DVec2::new(250.0, 5.0), 20.0, &geoms, None).unwrap().1, Accroche::Proche);
        // perpendiculaire depuis un point
        let (p, a) = accrocher(DVec2::new(300.0, 10.0), 20.0, &geoms[..1], Some(DVec2::new(300.0, 800.0))).unwrap();
        assert!(proche_de(p, DVec2::new(300.0, 0.0)) && a == Accroche::Perpendiculaire);
        // rien à portée
        assert!(accrocher(DVec2::new(5000.0, 5000.0), 20.0, &geoms, None).is_none());
    }

    #[test]
    fn polaire_ortho_et_saisie() {
        let o = DVec2::ZERO;
        let (p, a) = polaire(o, DVec2::new(1000.0, 30.0), 15.0, 3.0).unwrap();
        assert!(proche_de(p, DVec2::new(1000.0, 0.0)) && a == 0.0);
        assert!(polaire(o, DVec2::new(1000.0, 140.0), 15.0, 3.0).is_none());
        assert_eq!(ortho(o, DVec2::new(100.0, 30.0)), DVec2::new(100.0, 0.0));
        let prec = Some(DVec2::new(100.0, 100.0));
        assert!(proche_de(lire_saisie("2000;3000", prec, None, 1.0).unwrap(), DVec2::new(2000.0, 3000.0)));
        assert!(proche_de(lire_saisie("2,5;3", prec, None, 1.0).unwrap(), DVec2::new(2.5, 3.0)));
        assert!(proche_de(lire_saisie("20.5,30", prec, None, 1.0).unwrap(), DVec2::new(20.5, 30.0)));
        assert!(proche_de(lire_saisie("@50,0", prec, None, 1.0).unwrap(), DVec2::new(150.0, 100.0)));
        assert!(proche_de(lire_saisie("@100<90", prec, None, 1.0).unwrap(), DVec2::new(100.0, 200.0)));
        assert!(proche_de(lire_saisie("100<90", prec, None, 1.0).unwrap(), DVec2::new(100.0, 200.0)));
        // distance directe dans la direction du curseur, en cm (facteur 10)
        assert!(proche_de(lire_saisie("25", prec, Some(DVec2::new(0.0, -3.0)), 10.0).unwrap(), DVec2::new(100.0, -150.0)));
        assert!(lire_saisie("abc", prec, None, 1.0).is_err());
        // rectangle tourné de 90° : côtés suivant le repère
        let mut t = Trace::new(OutilDessin::Rectangle);
        t.angle = std::f64::consts::FRAC_PI_2;
        t.ajouter(DVec2::ZERO);
        let Some(Forme2D::Polyligne { points, fermee: true }) = t.ajouter(DVec2::new(300.0, 200.0)).pop() else { panic!() };
        assert!(proche_de(points[1], DVec2::new(0.0, 200.0)) && proche_de(points[3], DVec2::new(300.0, 0.0)));
        // ligne refermée sur son premier point
        let mut t = Trace::new(OutilDessin::Ligne);
        for p in [DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::ZERO] {
            t.ajouter(p);
        }
        assert!(t.points.is_empty());
        // polyligne refermée sur son premier sommet
        let mut t = Trace::new(OutilDessin::Polyligne);
        let mut formes = Vec::new();
        for p in [DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::ZERO] {
            formes = t.ajouter(p);
        }
        assert!(matches!(formes.as_slice(), [Forme2D::Polyligne { fermee: true, points }] if points.len() == 3));
        assert!(lire_saisie("@1,2", None, None, 1.0).is_err());
    }

    #[test]
    fn outils() {
        // ligne : segments enchaînés, F ferme
        let mut t = Trace::new(OutilDessin::Ligne);
        assert!(t.ajouter(DVec2::ZERO).is_empty());
        assert_eq!(t.ajouter(DVec2::new(100.0, 0.0)), vec![Forme2D::Ligne([DVec2::ZERO, DVec2::new(100.0, 0.0)])]);
        t.ajouter(DVec2::new(100.0, 100.0));
        assert_eq!(t.fermer(), vec![Forme2D::Ligne([DVec2::new(100.0, 100.0), DVec2::ZERO])]);
        // polyligne terminée par Entrée
        let mut t = Trace::new(OutilDessin::Polyligne);
        for p in [DVec2::ZERO, DVec2::new(100.0, 0.0), DVec2::new(100.0, 50.0)] {
            assert!(t.ajouter(p).is_empty());
        }
        assert!(matches!(&t.terminer()[..], [Forme2D::Polyligne { points, fermee: false }] if points.len() == 3));
        // cercle : centre puis rayon
        let mut t = Trace::new(OutilDessin::Cercle);
        t.ajouter(DVec2::new(10.0, 10.0));
        assert_eq!(t.ajouter(DVec2::new(10.0, 60.0)), vec![Forme2D::Cercle { centre: DVec2::new(10.0, 10.0), rayon: 50.0 }]);
        // arc par trois points : demi-cercle passant par le haut
        let (c, r, _, s) = arc_3_points(DVec2::new(-1.0, 0.0), DVec2::new(0.0, 1.0), DVec2::new(1.0, 0.0)).unwrap();
        assert!(proche_de(c, DVec2::ZERO) && (r - 1.0).abs() < 1e-9 && (s.abs() - std::f64::consts::PI).abs() < 1e-9);
        let pts = points_arc(c, r, (DVec2::new(-1.0, 0.0)).y.atan2(-1.0), s);
        assert!(pts.iter().all(|p| p.y >= -1e-9), "l'arc passe par le haut");
        assert!(arc_3_points(DVec2::ZERO, DVec2::new(1.0, 1.0), DVec2::new(2.0, 2.0)).is_none());
        // spline : passe par ses points
        let pts = [DVec2::ZERO, DVec2::new(100.0, 50.0), DVec2::new(200.0, 0.0)];
        let s = points_spline(&pts, false);
        assert!(pts.iter().all(|p| s.iter().any(|q| proche_de(*p, *q))));
    }
}
