//! Formes 2D dessinées (mm, plan de la vue) : lignes, polylignes à segments courbes,
//! splines, cercles, arcs, et géométrie des arcs et courbes qui les décrit.
//! Les outils de tracé et d'accrochage restent dans `dessin::dao`.

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

/// Forme dessinée, prête à enregistrer.
#[derive(Clone, Debug, PartialEq)]
pub enum Forme2D {
    Ligne([DVec2; 2]),
    /// `courbures` : courbure de chaque segment (segment k : du sommet k au suivant ; le
    /// dernier d'une polyligne fermée revient au premier). Tangente du quart de l'angle
    /// au centre de l'arc, positive dans le sens trigonométrique ; 0 ou absent : droit.
    Polyligne { points: Vec<DVec2>, fermee: bool, courbures: Vec<f64> },
    Spline { points: Vec<DVec2>, fermee: bool },
    Cercle { centre: DVec2, rayon: f64 },
    /// Arc par trois points : départ, point de passage, fin.
    Arc([DVec2; 3]),
}

/// Angle normalisé dans [0, 2π[.
pub fn angle_norm(a: f64) -> f64 {
    a.rem_euclid(std::f64::consts::TAU)
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

/// Centre, rayon, départ et balayage signé du segment de courbure `k` de `a` à `b` ;
/// None s'il est droit.
pub fn arc_courbure(a: DVec2, b: DVec2, k: f64) -> Option<(DVec2, f64, f64, f64)> {
    let d = b - a;
    if !k.is_finite() || k.abs() < 1e-9 || d.length_squared() < 1e-18 {
        return None;
    }
    let centre = (a + b) * 0.5 + d.perp() * ((1.0 - k * k) / (4.0 * k));
    Some((centre, d.length() * (1.0 + k * k) / (4.0 * k.abs()), (a - centre).to_angle(), 4.0 * k.atan()))
}

/// Segments d'une polyligne : extrémités et courbure de chacun.
pub fn segments_polyligne(points: &[DVec2], fermee: bool, courbures: &[f64]) -> Vec<(DVec2, DVec2, f64)> {
    let n = points.len();
    let nombre = n.saturating_sub(1) + usize::from(fermee && n > 2);
    (0..nombre).map(|k| (points[k], points[(k + 1) % n], courbures.get(k).copied().unwrap_or(0.0))).collect()
}

/// Points d'un segment de polyligne, extrémités comprises : ses deux sommets, ou son arc
/// développé.
pub fn points_segment(a: DVec2, b: DVec2, k: f64) -> Vec<DVec2> {
    match arc_courbure(a, b, k) {
        Some((o, r, d, s)) => {
            let mut l = points_arc(o, r, d, s);
            // extrémités exactes : les sommets de la polyligne
            l[0] = a;
            *l.last_mut().unwrap() = b;
            l
        }
        None => vec![a, b],
    }
}

/// Polyligne développée, arcs compris ; fermée, elle revient à son premier point.
pub fn developper(points: &[DVec2], fermee: bool, courbures: &[f64]) -> Vec<DVec2> {
    let mut l: Vec<DVec2> = points.first().copied().into_iter().collect();
    for (a, b, k) in segments_polyligne(points, fermee, courbures) {
        l.extend(points_segment(a, b, k).into_iter().skip(1));
    }
    l
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
            Self::Polyligne { points, fermee, courbures } => vec![developper(points, *fermee, courbures)],
            Self::Spline { points, fermee } => vec![points_spline(points, *fermee)],
            Self::Cercle { centre, rayon } => vec![points_arc(*centre, *rayon, 0.0, std::f64::consts::TAU)],
            Self::Arc([a, b, c]) => match arc_3_points(*a, *b, *c) {
                Some((o, r, d, s)) => vec![points_arc(o, r, d, s)],
                None => vec![vec![*a, *c]],
            },
        }
    }

    /// Longueur du tracé (mm) ; un contour fermé compte son côté de fermeture.
    pub fn longueur(&self) -> f64 {
        match self {
            Self::Cercle { rayon, .. } => std::f64::consts::TAU * rayon,
            Self::Polyligne { points, fermee, courbures } => segments_polyligne(points, *fermee, courbures)
                .into_iter()
                .map(|(a, b, k)| arc_courbure(a, b, k).map_or(a.distance(b), |(_, r, _, s)| r * s.abs()))
                .sum(),
            _ => self.polylignes().iter().map(|l| l.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>()).sum(),
        }
    }

    /// Surface d'une forme fermée (mm²) : polyligne ou spline fermée, cercle ; None sinon.
    pub fn surface(&self) -> Option<f64> {
        match self {
            Self::Cercle { rayon, .. } => Some(std::f64::consts::PI * rayon * rayon),
            Self::Polyligne { points, fermee: true, courbures } => {
                // polygone des sommets, plus le segment circulaire (signé) de chaque arc
                let segments = segments_polyligne(points, true, courbures);
                let polygone: f64 = segments.iter().map(|(a, b, _)| a.perp_dot(*b)).sum::<f64>() * 0.5;
                let arcs: f64 = segments.iter().filter_map(|(a, b, k)| arc_courbure(*a, *b, *k)).map(|(_, r, _, s)| r * r * 0.5 * (s - s.sin())).sum();
                Some((polygone + arcs).abs())
            }
            Self::Spline { fermee: true, .. } => {
                let l = self.polylignes().remove(0);
                Some((l.windows(2).map(|w| w[0].perp_dot(w[1])).sum::<f64>() + l.last().zip(l.first()).map_or(0.0, |(a, b)| a.perp_dot(*b))).abs() * 0.5)
            }
            _ => None,
        }
    }

    /// Géométries d'accrochage.
    pub fn geoms(&self) -> Vec<Geom> {
        match self {
            Self::Ligne(s) => vec![Geom::Segment(*s)],
            Self::Polyligne { points, fermee, courbures } => segments_polyligne(points, *fermee, courbures).into_iter().map(|(a, b, k)| geom_segment(a, b, k)).collect(),
            Self::Spline { .. } => vec![Geom::Courbe(self.polylignes().remove(0))],
            Self::Cercle { centre, rayon } => vec![Geom::Cercle { centre: *centre, rayon: *rayon }],
            Self::Arc([a, b, c]) => match arc_3_points(*a, *b, *c) {
                Some((centre, rayon, depart, balayage)) => vec![Geom::Arc { centre, rayon, depart, balayage }],
                None => vec![Geom::Segment([*a, *c])],
            },
        }
    }
}

/// Géométrie d'accrochage d'un segment de polyligne : segment droit ou arc.
pub fn geom_segment(a: DVec2, b: DVec2, k: f64) -> Geom {
    match arc_courbure(a, b, k) {
        Some((centre, rayon, depart, balayage)) => Geom::Arc { centre, rayon, depart, balayage },
        None => Geom::Segment([a, b]),
    }
}
