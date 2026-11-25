//! Régions masquantes à l'écran : les traits du modèle (contours coupés, hachures,
//! arêtes vues, symboles des familles) sont dessinés après les surfaces et ne sont pas
//! cachés par elles ; leurs parties intérieures aux régions qui masquent le modèle sont
//! retirées ici. Les exports vectoriels n'en ont pas besoin (ordre de tracé).
use glam::{DVec2, DVec3};
use noyau::trace2d::{Forme, Primitive, Role};

fn dedans(c: &[DVec2], q: DVec2) -> bool {
    let mut d = false;
    for i in 0..c.len() {
        let (a, b) = (c[i], c[(i + 1) % c.len()]);
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) * (b.x - a.x) / (b.y - a.y) { d = !d; }
    }
    d
}

/// Paramètres (0..1) où le segment a–b coupe un côté des régions.
fn coupures(regions: &[Vec<DVec2>], a: DVec2, b: DVec2) -> Vec<f64> {
    let d = b - a;
    let mut t = vec![0.0, 1.0];
    for c in regions {
        for i in 0..c.len() {
            let (p, q) = (c[i], c[(i + 1) % c.len()]);
            let e = q - p;
            let den = d.perp_dot(e);
            if den.abs() < 1e-12 { continue; }
            let s = (p - a).perp_dot(e) / den;
            let u = (p - a).perp_dot(d) / den;
            if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&u) { t.push(s); }
        }
    }
    t.sort_by(f64::total_cmp);
    t.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    t
}

/// Retire des traits sous les régions masquantes (`regions` : contours dans le repère
/// 2D donné par `plan`) les parties intérieures ; les traits au-dessus des régions
/// (annotations) et les surfaces sont rendus tels quels.
pub fn masquer_traits(prims: Vec<Primitive>, regions: &[Vec<DVec2>], plan: &dyn Fn(DVec3) -> DVec2) -> Vec<Primitive> {
    if regions.is_empty() { return prims; }
    let (min, max) = regions.iter().flatten().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
    let masque = |a: DVec3, b: DVec3, out: &mut Vec<[DVec3; 2]>| {
        let (p, q) = (plan(a), plan(b));
        // hors de l'emprise des régions : gardé entier
        if p.max(q).cmplt(min).any() || p.min(q).cmpgt(max).any() { out.push([a, b]); return; }
        let t = coupures(regions, p, q);
        for w in t.windows(2) {
            let milieu = p.lerp(q, (w[0] + w[1]) / 2.0);
            if !regions.iter().any(|c| dedans(c, milieu)) && w[1] - w[0] > 1e-9 { out.push([a.lerp(b, w[0]), a.lerp(b, w[1])]); }
        }
    };
    prims.into_iter().filter_map(|mut x| {
        if x.role.rang() >= Role::Masque.rang() { return Some(x); }
        let segments: Vec<[DVec3; 2]> = match &x.forme {
            Forme::Polyligne { points, fermee } => {
                let n = if *fermee { points.len() } else { points.len().saturating_sub(1) };
                (0..n).map(|i| [points[i], points[(i + 1) % points.len()]]).collect()
            }
            Forme::Segments(s) => s.clone(),
            _ => return Some(x),
        };
        let mut out = Vec::new();
        for [a, b] in &segments { masque(*a, *b, &mut out); }
        if out.is_empty() { return None; }
        if out.len() == segments.len() && out.iter().zip(&segments).all(|(a, b)| a == b) { return Some(x); }
        x.forme = Forme::Segments(out);
        Some(x)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use noyau::trace2d::Style;

    #[test]
    fn traits_du_modele_coupes_sous_la_region() {
        let region = vec![vec![DVec2::new(1.0, -1.0), DVec2::new(2.0, -1.0), DVec2::new(2.0, 1.0), DVec2::new(1.0, 1.0)]];
        let trait_ = |role| Primitive { forme: Forme::Polyligne { points: vec![DVec3::ZERO, DVec3::X * 3.0], fermee: false }, style: Style { couleur: [0, 0, 0, 255], epaisseur: 0.0 }, role, element: None };
        let r = masquer_traits(vec![trait_(Role::Coupe), trait_(Role::Annotation)], &region, &|q| q.truncate());
        let Forme::Segments(s) = &r[0].forme else { panic!("{:?}", r[0].forme) };
        assert_eq!(s.len(), 2);
        assert!((s[0][1].x - 1.0).abs() < 1e-9 && (s[1][0].x - 2.0).abs() < 1e-9);
        // annotation au-dessus : intacte
        assert!(matches!(&r[1].forme, Forme::Polyligne { points, .. } if points.len() == 2));
    }
}
