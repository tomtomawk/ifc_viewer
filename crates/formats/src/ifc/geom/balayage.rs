//! Balayage d'un profil le long d'une directrice : `IfcSurfaceCurveSweptAreaSolid`,
//! `IfcFixedReferenceSweptAreaSolid` et `IfcSectionedSolidHorizontal`.
//!
//! La directrice est échantillonnée en polyligne (`Geom::curve` : polyligne, arc, courbe
//! composite, courbe découpée) ; chaque segment porte une section rigide, raccordée à la
//! suivante par un onglet (plan bissecteur), ce qui donne un volume exact pour une
//! polyligne. Repère de la section :
//! - surface de référence (`IfcSurfaceCurveSweptAreaSolid`) : Y local du profil selon la
//!   normale de la surface (plan ; autre surface : Z du repère), Z selon la tangente ;
//! - référence fixe (`IfcFixedReferenceSweptAreaSolid`) : X local selon la direction de
//!   référence projetée perpendiculairement à la tangente.
use super::{clean_ring, signed_area, triangulate_2d, Geom, Profile};
use glam::{DMat4, DVec2, DVec3};
use ifc_base::step::{Entity, Value};
use noyau::csg::{self, Tri};

/// Nombre de points de directrice au-delà duquel le balayage est écarté.
const POINTS_MAX: usize = 100_000;
/// Nombre de sections d'un `IfcSectionedSolidHorizontal` au-delà duquel il est écarté.
const SECTIONS_MAX: usize = 10_000;

/// Orientation de la section le long de la directrice.
#[derive(Clone, Copy)]
pub(super) enum Repere {
    /// Y local selon cette normale.
    Normale(DVec3),
    /// X local selon cette direction.
    Reference(DVec3),
}

impl Repere {
    /// Axes X, Y de la section pour la tangente unitaire `t`.
    fn axes(self, t: DVec3) -> (DVec3, DVec3) {
        let proj = |v: DVec3| (v - t * v.dot(t)).try_normalize().unwrap_or_else(|| t.any_orthonormal_vector());
        match self {
            Repere::Normale(n) => {
                let y = proj(n);
                (y.cross(t), y)
            }
            Repere::Reference(r) => {
                let x = proj(r);
                (x, t.cross(x))
            }
        }
    }
}

impl Geom<'_> {
    /// `IfcSurfaceCurveSweptAreaSolid` et `IfcFixedReferenceSweptAreaSolid`.
    pub(super) fn balayage(&self, e: &Entity, m: DMat4) -> Option<Vec<Tri>> {
        let profils = self.profile(e.arg(0).as_ref()?, 0);
        let pos = m * self.axis2(e.arg(1).as_ref());
        let directrice = self.get(e.arg(2).as_ref()?)?;
        let mut chemin = self.curve(e.arg(2).as_ref()?, 0);
        if directrice.is("IFCPOLYLINE") {
            chemin = decouper_polyligne(chemin, e.arg(3).as_f64(), e.arg(4).as_f64());
        }
        let repere = if e.is("IFCFIXEDREFERENCESWEPTAREASOLID") {
            Repere::Reference(self.direction(e.arg(5).as_ref()).unwrap_or(DVec3::X))
        } else {
            Repere::Normale(self.normale_surface(e.arg(5).as_ref()))
        };
        let mut out = Vec::new();
        for p in &profils {
            balayer(p, &chemin, repere, &mut out);
        }
        for t in &mut out {
            *t = t.map(|q| pos.transform_point3(q));
        }
        csg::fix_orientation(&mut out);
        Some(out)
    }

    /// Normale d'une surface de référence : Z de la position d'un `IfcPlane` (ou de
    /// toute surface élémentaire), Z global sinon.
    fn normale_surface(&self, id: Option<ifc_base::step::Id>) -> DVec3 {
        let Some(s) = id.and_then(|i| self.get(i)) else { return DVec3::Z };
        if s.is("IFCPLANE") {
            return self.axis2(s.arg(0).as_ref()).transform_vector3(DVec3::Z);
        }
        DVec3::Z
    }

    /// `IfcSectionedSolidHorizontal` (Directrix, CrossSections, CrossSectionPositions) :
    /// sections placées par distance le long de la directrice (`IfcAxis2PlacementLinear`
    /// d'un `IfcPointByDistanceExpression`), Y de section vers le haut (Z), raccordées
    /// deux à deux par interpolation linéaire. Sections de nombres de sommets différents
    /// ou à trous : None.
    pub(super) fn solide_sectionne(&self, e: &Entity, m: DMat4) -> Option<Vec<Tri>> {
        let chemin = self.curve(e.arg(0).as_ref()?, 0);
        let sections: Vec<_> = e.arg(1).refs().collect();
        let positions: Vec<_> = e.arg(2).refs().collect();
        if sections.len() < 2 || sections.len() != positions.len() || sections.len() > SECTIONS_MAX {
            return None;
        }
        let mut anneaux: Vec<(f64, Vec<DVec2>)> = Vec::new();
        for (s, p) in sections.iter().zip(&positions) {
            let [prof] = self.profile(*s, 0).try_into().ok()?;
            if !prof.holes.is_empty() {
                return None;
            }
            let (d, decalage) = self.distance_lineaire(*p)?;
            let mut r = clean_ring(prof.outer);
            if signed_area(&r) < 0.0 {
                r.reverse();
            }
            anneaux.push((d, r.into_iter().map(|q| q + decalage).collect()));
        }
        let n = anneaux[0].1.len();
        if n < 3 || anneaux.iter().any(|(_, r)| r.len() != n) || anneaux.windows(2).any(|w| w[1].0 < w[0].0) {
            return None;
        }
        let abscisses = abscisses(&chemin);
        let mut out = Vec::new();
        let mut precedent: Option<Vec<DVec3>> = None;
        let mut premier: Option<Vec<DVec3>> = None;
        for w in anneaux.windows(2) {
            let (d0, d1) = (w[0].0, w[1].0);
            let mut ds: Vec<f64> = vec![d0];
            ds.extend(abscisses.iter().copied().filter(|&d| d > d0 + 1e-9 && d < d1 - 1e-9));
            ds.push(d1);
            for (k, &d) in ds.iter().enumerate() {
                if k == 0 && precedent.is_some() {
                    continue;
                }
                let a = if d1 > d0 { (d - d0) / (d1 - d0) } else { 0.0 };
                let (o, t) = point_a(&chemin, &abscisses, d)?;
                let (x, y) = Repere::Normale(DVec3::Z).axes(t);
                let anneau: Vec<DVec3> = (0..n)
                    .map(|i| {
                        let q = w[0].1[i].lerp(w[1].1[i], a);
                        m.transform_point3(o + x * q.x + y * q.y)
                    })
                    .collect();
                if let Some(prev) = &precedent {
                    cotes(prev, &anneau, &mut out);
                }
                premier.get_or_insert_with(|| anneau.clone());
                precedent = Some(anneau);
            }
        }
        for (anneau, r, inverse) in [(premier?, &anneaux[0].1, true), (precedent?, &anneaux[anneaux.len() - 1].1, false)] {
            for t in triangulate_2d(r, &[]) {
                let idx = t.map(|q| r.iter().position(|p| *p == q).unwrap_or(0));
                let tri = [anneau[idx[0]], anneau[idx[1]], anneau[idx[2]]];
                out.push(if inverse { [tri[0], tri[2], tri[1]] } else { tri });
            }
        }
        csg::fix_orientation(&mut out);
        Some(out)
    }

    /// Distance le long de la directrice et décalage (latéral, vertical) d'un
    /// `IfcAxis2PlacementLinear` dont la position est un `IfcPointByDistanceExpression`.
    fn distance_lineaire(&self, id: ifc_base::step::Id) -> Option<(f64, DVec2)> {
        let pl = self.get(id)?;
        let pt = self.get(pl.arg(0).as_ref()?)?;
        if !pt.is("IFCPOINTBYDISTANCEEXPRESSION") {
            return None;
        }
        let f = |v: &Value| v.as_f64().filter(|x| x.is_finite());
        let d = f(pt.arg(0))?;
        let lat = f(pt.arg(1)).unwrap_or(0.0);
        let vert = f(pt.arg(2)).unwrap_or(0.0);
        // OffsetLateral positif vers la gauche de la directrice (−X de la section).
        Some((d, DVec2::new(-lat, vert)))
    }
}

/// Polyligne restreinte aux paramètres [début, fin] (paramètre IFC d'une polyligne :
/// i + fraction sur le segment i). Paramètres absents ou incohérents : polyligne entière.
fn decouper_polyligne(p: Vec<DVec3>, debut: Option<f64>, fin: Option<f64>) -> Vec<DVec3> {
    let n = p.len();
    if n < 2 {
        return p;
    }
    let max = (n - 1) as f64;
    let debut = debut.filter(|x| x.is_finite()).unwrap_or(0.0).clamp(0.0, max);
    let fin = fin.filter(|x| x.is_finite()).unwrap_or(max).clamp(0.0, max);
    if fin <= debut + 1e-12 {
        return p;
    }
    let en = |t: f64| {
        let i = (t.floor() as usize).min(n - 2);
        p[i].lerp(p[i + 1], t - i as f64)
    };
    let mut out = vec![en(debut)];
    out.extend((debut.floor() as usize + 1..=fin.ceil() as usize).filter(|&i| (i as f64) > debut && (i as f64) < fin).map(|i| p[i]));
    out.push(en(fin));
    out
}

/// Abscisse curviligne de chaque point.
fn abscisses(p: &[DVec3]) -> Vec<f64> {
    let mut s = 0.0;
    let mut out = Vec::with_capacity(p.len());
    for (i, q) in p.iter().enumerate() {
        if i > 0 {
            s += q.distance(p[i - 1]);
        }
        out.push(s);
    }
    out
}

/// Point et tangente unitaire à l'abscisse `d` (prolongée aux extrémités).
fn point_a(p: &[DVec3], s: &[f64], d: f64) -> Option<(DVec3, DVec3)> {
    if p.len() < 2 {
        return None;
    }
    let i = s.partition_point(|&x| x <= d).clamp(1, p.len() - 1) - 1;
    let t = (p[i + 1] - p[i]).try_normalize()?;
    Some((p[i] + t * (d - s[i]), t))
}

/// Quadrilatères entre deux anneaux de même nombre de sommets.
fn cotes(a: &[DVec3], b: &[DVec3], out: &mut Vec<Tri>) {
    let n = a.len();
    for i in 0..n {
        let j = (i + 1) % n;
        out.push([a[i], a[j], b[j]]);
        out.push([a[i], b[j], b[i]]);
    }
}

/// Balayage du profil `p` le long de `chemin` (repère local du solide), avec onglets aux
/// sommets intérieurs et faces d'extrémité.
pub(super) fn balayer(p: &Profile, chemin: &[DVec3], repere: Repere, out: &mut Vec<Tri>) {
    let mut chemin = chemin.to_vec();
    chemin.dedup_by(|a, b| a.distance_squared(*b) < 1e-18);
    if chemin.len() < 2 || chemin.len() > POINTS_MAX {
        return;
    }
    let mut outer = clean_ring(p.outer.clone());
    if outer.len() < 3 {
        return;
    }
    if signed_area(&outer) < 0.0 {
        outer.reverse();
    }
    let holes: Vec<Vec<DVec2>> = p
        .holes
        .iter()
        .map(|h| {
            let mut h = clean_ring(h.clone());
            if signed_area(&h) > 0.0 {
                h.reverse();
            }
            h
        })
        .filter(|h| h.len() >= 3)
        .collect();
    let n = chemin.len();
    let tangentes: Vec<DVec3> = chemin.windows(2).map(|w| (w[1] - w[0]).normalize()).collect();
    // Point d'un sommet de profil au sommet k de la directrice, porté par le segment s
    // (repère de s) et projeté le long de sa tangente sur le plan d'onglet.
    let place = |q: DVec2, k: usize| -> DVec3 {
        let s = k.min(n - 2);
        let t = tangentes[s];
        let (x, y) = repere.axes(t);
        let v = chemin[k] + x * q.x + y * q.y;
        if k == 0 || k == n - 1 {
            return v;
        }
        let onglet = (tangentes[k - 1] + tangentes[k]).normalize_or(t);
        let den = t.dot(onglet);
        if den.abs() < 1e-6 {
            return v;
        }
        v - t * ((v - chemin[k]).dot(onglet) / den)
    };
    // Repères continus : chaque segment s utilise le repère de s aux deux bouts, la
    // jonction se fait au plan d'onglet (identique des deux côtés pour un repère plan).
    let au = |q: DVec2, k: usize, s: usize| -> DVec3 {
        if s + 1 == k && k < n - 1 {
            // bout aval du segment s : même plan d'onglet, repère de s
            let t = tangentes[s];
            let (x, y) = repere.axes(t);
            let v = chemin[k] + x * q.x + y * q.y;
            let onglet = (tangentes[s] + tangentes[k]).normalize_or(t);
            let den = t.dot(onglet);
            if den.abs() < 1e-6 {
                return v;
            }
            return v - t * ((v - chemin[k]).dot(onglet) / den);
        }
        place(q, k)
    };
    for ring in std::iter::once(&outer).chain(holes.iter()) {
        let m = ring.len();
        for s in 0..n - 1 {
            for i in 0..m {
                let (p0, p1) = (ring[i], ring[(i + 1) % m]);
                let (a0, b0, a1, b1) = (au(p0, s, s), au(p1, s, s), au(p0, s + 1, s), au(p1, s + 1, s));
                out.push([a0, b0, b1]);
                out.push([a0, b1, a1]);
            }
        }
    }
    for tri in triangulate_2d(&outer, &holes) {
        let d = tri.map(|q| place(q, 0));
        let f = tri.map(|q| place(q, n - 1));
        out.push([d[0], d[2], d[1]]);
        out.push(f);
    }
}

#[cfg(test)]
mod tests;
