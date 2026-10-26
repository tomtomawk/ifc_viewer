//! Jonctions des murs droits en plan, comme Revit les nettoie : angle (L) en onglet,
//! arrivée sur un mur (T) arrêtée sur sa face. Un assemblage se raccorde d'un bloc :
//! la même coupe s'applique à toutes ses couches, si bien que deux assemblages de
//! même composition se rejoignent couche à couche. Calcul d'affichage seulement : les
//! axes enregistrés ne changent pas.
use super::*;
use glam::DVec2;
use std::collections::HashMap;

/// Coupe d'une extrémité de mur : droite passant par `point`, de direction `direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coupe {
    pub point: [f64; 2],
    pub direction: [f64; 2],
}

/// Coupes des deux extrémités (début, fin) de chaque mur raccordé.
pub type Coupes = HashMap<String, [Option<Coupe>; 2]>;

/// Mur « logique » : un mur seul ou un assemblage (axe au milieu de l'épaisseur
/// totale), avec ses membres et le sens de chacun par rapport à son axe.
struct Logique {
    a: DVec2,
    b: DVec2,
    epaisseur: f64,
    bas: f64,
    haut: f64,
    membres: Vec<(String, bool)>,
}

fn v(p: [f64; 2]) -> DVec2 {
    DVec2::from_array(p)
}

/// Intersection des droites p + t·u et q + s·w (None si presque parallèles).
fn intersection(p: DVec2, u: DVec2, q: DVec2, w: DVec2) -> Option<DVec2> {
    let d = u.perp_dot(w);
    if d.abs() < 1e-9 * u.length() * w.length() {
        return None;
    }
    Some(p + u * ((q - p).perp_dot(w) / d))
}

fn logiques(p: &Projet) -> Vec<Logique> {
    let mut out = Vec::new();
    let droit = |m: &Mur| match m.axe {
        Axe::Droit { debut, fin } => Some((v(debut), v(fin))),
        Axe::Arc { .. } => None,
    };
    let epaisseur = |m: &Mur| p.type_mur(&m.type_mur).map_or(0.0, |t| t.epaisseur());
    for m in p.murs.iter().filter(|m| m.assemblage.as_ref().is_none_or(|a| p.assemblage(a).is_none())) {
        let (Some((a, b)), Some((bas, haut))) = (droit(m), p.bas_haut(m)) else { continue };
        let e = epaisseur(m);
        if e > 0.0 && a.distance(b) > 1.0 {
            out.push(Logique { a, b, epaisseur: e, bas, haut, membres: vec![(m.id.clone(), true)] });
        }
    }
    for asm in &p.assemblages {
        let couches = p.couches_assemblage(&asm.id);
        let Some(premiere) = couches.first() else { continue };
        let Some((a0, b0)) = droit(premiere) else { continue };
        let d = (b0 - a0).normalize_or_zero();
        if d == DVec2::ZERO {
            continue;
        }
        let n = d.perp();
        // faces extrêmes le long de la normale gauche, sens de chaque couche
        let (mut min, mut max) = (f64::MAX, f64::MIN);
        let (mut bas, mut haut) = (f64::MAX, f64::MIN);
        let mut membres = Vec::new();
        let mut valide = true;
        for c in &couches {
            let (Some((a, b)), Some((zb, zh))) = (droit(c), p.bas_haut(c)) else { valide = false; break };
            let sens = (b - a).dot(d);
            if (b - a).perp_dot(d).abs() > 1e-6 * (b - a).length() || sens == 0.0 {
                valide = false;
                break;
            }
            let o = a.dot(n);
            let h = epaisseur(c) / 2.0;
            min = min.min(o - h);
            max = max.max(o + h);
            bas = bas.min(zb);
            haut = haut.max(zh);
            membres.push((c.id.clone(), sens > 0.0));
        }
        if !valide || max <= min {
            continue;
        }
        // axe au milieu de l'épaisseur totale, sur la longueur de la première couche
        let decale = n * ((min + max) / 2.0 - a0.dot(n));
        out.push(Logique { a: a0 + decale, b: b0 + decale, epaisseur: max - min, bas, haut, membres });
    }
    out
}

/// Coupes de toutes les extrémités raccordées du projet.
pub fn coupes(p: &Projet) -> Coupes {
    let ls = logiques(p);
    // (mur logique, extrémité 0 début / 1 fin) → candidats (distance, coupe, est un L)
    let mut candidats: HashMap<(usize, usize), Vec<(f64, Coupe, bool)>> = HashMap::new();
    for i in 0..ls.len() {
        for j in i + 1..ls.len() {
            let (a, b) = (&ls[i], &ls[j]);
            // murs superposés en hauteur seulement
            if a.haut.min(b.haut) - a.bas.max(b.bas) <= 1.0 {
                continue;
            }
            let (da, db) = ((a.b - a.a).normalize(), (b.b - b.a).normalize());
            if da.perp_dot(db).abs() < 0.05 {
                continue;
            }
            let Some(x) = intersection(a.a, da, b.a, db) else { continue };
            let tol = a.epaisseur + b.epaisseur + 5.0;
            let bout = |l: &Logique| -> (usize, f64) {
                let (d0, d1) = (l.a.distance(x), l.b.distance(x));
                if d0 <= d1 { (0, d0) } else { (1, d1) }
            };
            let interieur = |l: &Logique| {
                let d = (l.b - l.a).normalize();
                let t = (x - l.a).dot(d);
                t > tol && t < l.a.distance(l.b) - tol
            };
            let ((ea, dist_a), (eb, dist_b)) = (bout(a), bout(b));
            let loin = |l: &Logique, e: usize| if e == 0 { l.b } else { l.a };
            if dist_a <= tol && dist_b <= tol {
                // onglet : droite par les intersections des faces intérieures et extérieures
                let (ua, ub) = ((loin(a, ea) - x).normalize(), (loin(b, eb) - x).normalize());
                let s = ua.perp_dot(ub).signum();
                let (ha, hb) = (a.epaisseur / 2.0, b.epaisseur / 2.0);
                let face = |u: DVec2, h: f64, signe: f64| x + u.perp() * signe * h;
                let (Some(dedans), Some(dehors)) = (
                    intersection(face(ua, ha, s), ua, face(ub, hb, -s), ub),
                    intersection(face(ua, ha, -s), ua, face(ub, hb, s), ub),
                ) else {
                    continue;
                };
                let c = Coupe { point: dedans.to_array(), direction: (dehors - dedans).to_array() };
                candidats.entry((i, ea)).or_default().push((dist_a, c, true));
                candidats.entry((j, eb)).or_default().push((dist_b, c, true));
            } else {
                // arrivée en T : arrêt sur la face du mur traversé, côté de l'arrivant
                for (k, e, dist, porteur, arrivant) in [(i, ea, dist_a, b, a), (j, eb, dist_b, a, b)] {
                    if dist <= tol && interieur(porteur) {
                        let dp = (porteur.b - porteur.a).normalize();
                        let cote = (loin(arrivant, e) - x).dot(dp.perp()).signum();
                        let point = x + dp.perp() * cote * porteur.epaisseur / 2.0;
                        candidats.entry((k, e)).or_default().push((dist, Coupe { point: point.to_array(), direction: dp.to_array() }, false));
                    }
                }
            }
        }
    }
    let mut out: Coupes = HashMap::new();
    for ((k, e), mut cs) in candidats {
        // plus de deux murs au même angle : pas d'onglet, extrémité laissée droite
        let l_count = cs.iter().filter(|c| c.2).count();
        if l_count > 1 {
            cs.retain(|c| !c.2);
        }
        cs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let Some(&(_, coupe, _)) = cs.first() else { continue };
        for (id, sens) in &ls[k].membres {
            // extrémité du membre qui correspond à celle du mur logique
            let bout = if *sens { e } else { 1 - e };
            out.entry(id.clone()).or_insert([None, None])[bout] = Some(coupe);
        }
    }
    out
}

/// Contour en plan d'un mur droit (mm) : [début droite, fin droite, fin gauche, début
/// gauche], extrémités coupées par ses jonctions. None pour un arc.
pub fn contour(p: &Projet, m: &Mur, coupes: &Coupes) -> Option<[[f64; 2]; 4]> {
    let Axe::Droit { debut, fin } = m.axe else { return None };
    let (a, b) = (v(debut), v(fin));
    let d = (b - a).normalize_or_zero();
    let h = p.type_mur(&m.type_mur)?.epaisseur() / 2.0;
    let n = d.perp() * h;
    let mut c = [a - n, b - n, b + n, a + n];
    if let Some(cs) = coupes.get(&m.id) {
        for (bout, coupe) in cs.iter().enumerate() {
            let Some(coupe) = coupe else { continue };
            let (q, w) = (v(coupe.point), v(coupe.direction));
            let (droite, gauche) = if bout == 0 { (0, 3) } else { (1, 2) };
            let base = if bout == 0 { a } else { b };
            // garde-fou : une coupe presque parallèle au mur n'étire pas l'extrémité à l'infini
            if d.perp_dot(w.normalize_or_zero()).abs() < 0.05 {
                continue;
            }
            if let (Some(x), Some(y)) = (intersection(base - n, d, q, w), intersection(base + n, d, q, w)) {
                c[droite] = x;
                c[gauche] = y;
            }
        }
    }
    Some(c.map(|p| p.to_array()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::edition::ParametresMur;

    fn aire(c: &[[f64; 2]; 4]) -> f64 {
        (0..4).map(|k| c[k][0] * c[(k + 1) % 4][1] - c[(k + 1) % 4][0] * c[k][1]).sum::<f64>().abs() / 2.0
    }

    #[test]
    fn angle_en_onglet_et_arrivee_en_t() {
        let mut p = Projet::nouveau("J");
        let o = ParametresMur { epaisseur: 200.0, ..Default::default() };
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &o).unwrap();
        p.ajouter_mur([5000.0, 0.0], [5000.0, 4000.0], &o).unwrap();
        // arrivée en T au milieu du premier mur
        p.ajouter_mur([2500.0, 2000.0], [2500.0, 0.0], &o).unwrap();
        let cs = coupes(&p);
        let c: Vec<_> = p.murs.iter().map(|m| contour(&p, m, &cs).unwrap()).collect();
        // L : coins extérieur (5100, −100) et intérieur (4900, 100) communs aux deux murs
        let proche = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
        assert!(proche(c[0][1], [5100.0, -100.0]) && proche(c[0][2], [4900.0, 100.0]), "{:?}", c[0]);
        assert!([c[1][0], c[1][3]].iter().any(|x| proche(*x, [5100.0, -100.0])) && [c[1][0], c[1][3]].iter().any(|x| proche(*x, [4900.0, 100.0])), "{:?}", c[1]);
        // aire totale de l'angle : sans recouvrement ni manque
        assert!((aire(&c[0]) + aire(&c[1]) - (5100.0 * 200.0 + 200.0 * 3900.0)).abs() < 1e-3);
        // T : le mur arrivant s'arrête sur la face y = 100
        assert!((c[2][1][1] - 100.0).abs() < 1e-6 && (c[2][2][1] - 100.0).abs() < 1e-6, "{:?}", c[2]);
    }

    #[test]
    fn assemblages_raccordes_couche_a_couche() {
        let mut p = Projet::nouveau("A");
        let c = |m: &str, e: f64| Couche { materiau: Some(m.into()), epaisseur: e, ..Default::default() };
        p.types_murs.push(TypeCompose { id: "t".into(), nom: "Mur".into(), couches: vec![c("Enduit", 20.0), c("Isolant", 100.0), c("Béton", 200.0)], couches_detaillees: true, origine: None });
        let o = ParametresMur { type_mur: Some("t".into()), ..Default::default() };
        // tracé dans le sens horaire : extérieur à gauche, vers le dehors de l'angle
        p.ajouter_mur([0.0, 4000.0], [5000.0, 4000.0], &o).unwrap();
        p.ajouter_mur([5000.0, 4000.0], [5000.0, 0.0], &o).unwrap();
        let cs = coupes(&p);
        let polys: Vec<_> = p.murs.iter().map(|m| contour(&p, m, &cs).unwrap()).collect();
        assert_eq!(polys.len(), 6);
        // L de 320 mm d'épaisseur, coin extérieur (5160, 4160) : aire sans recouvrement
        let attendu = 5160.0 * 320.0 + 320.0 * (4000.0 - 160.0);
        let total: f64 = polys.iter().map(aire).sum();
        assert!((total - attendu).abs() < 1e-3, "{total} ≠ {attendu}");
        // chaque couche du premier mur rejoint la même couche du second (sommets communs)
        for k in 0..3 {
            let (a, b) = (polys[k], polys[k + 3]);
            let communs = [a[1], a[2]].iter().filter(|x| b.iter().any(|y| (x[0] - y[0]).hypot(x[1] - y[1]) < 1e-6)).count();
            assert_eq!(communs, 2, "couche {k} : {a:?} / {b:?}");
        }
    }
}
