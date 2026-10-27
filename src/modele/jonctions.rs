//! Jonctions des murs droits en plan, comme Revit les nettoie : angle (L) en onglet,
//! arrivée sur un mur (T) arrêtée sur sa face, deux murs alignés bout à bout continus
//! (un troisième mur au même nœud s'arrête sur leur face). Un assemblage se raccorde d'un bloc :
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

/// Parties d'un mur retirées à l'affichage (mm le long de son axe depuis son début) : là
/// où le noyau d'un mur arrivant traverse ses couches de finition.
pub type Interruptions = HashMap<String, Vec<(f64, f64)>>;

/// Membre d'un mur logique : identifiant, sens par rapport à l'axe, couche porteuse,
/// écart de son milieu à l'axe (le long de la normale gauche).
struct Membre {
    id: String,
    sens: bool,
    porteur: bool,
    ecart: f64,
}

/// Mur « logique » : un mur seul ou un assemblage (axe au milieu de l'épaisseur
/// totale), avec ses membres et son noyau (couches porteuses : écarts de ses faces à
/// l'axe ; None : pas de couche porteuse).
struct Logique {
    a: DVec2,
    b: DVec2,
    epaisseur: f64,
    bas: f64,
    haut: f64,
    membres: Vec<Membre>,
    noyau: Option<(f64, f64)>,
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
            out.push(Logique { a, b, epaisseur: e, bas, haut, membres: vec![Membre { id: m.id.clone(), sens: true, porteur: true, ecart: 0.0 }], noyau: Some((-e / 2.0, e / 2.0)) });
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
        let mut couches_n: Vec<(f64, f64, bool)> = Vec::new();
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
            let porteur = p.type_mur(&c.type_mur).and_then(|t| t.couches.first()).is_some_and(|x| x.fonction == FonctionCouche::Structure);
            couches_n.push((o, h, porteur));
            membres.push(Membre { id: c.id.clone(), sens: sens > 0.0, porteur, ecart: o });
        }
        if !valide || max <= min {
            continue;
        }
        // axe au milieu de l'épaisseur totale, sur la longueur de la première couche
        let milieu = (min + max) / 2.0;
        let decale = n * (milieu - a0.dot(n));
        for m in &mut membres {
            m.ecart -= milieu;
        }
        let porteuses = couches_n.iter().filter(|c| c.2);
        let noyau = porteuses.fold(None, |acc: Option<(f64, f64)>, c| {
            let (lo, hi) = (c.0 - c.1 - milieu, c.0 + c.1 - milieu);
            Some(acc.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))))
        });
        out.push(Logique { a: a0 + decale, b: b0 + decale, epaisseur: max - min, bas, haut, membres, noyau });
    }
    out
}

/// Coupes de toutes les extrémités raccordées du projet.
pub fn coupes(p: &Projet) -> Coupes {
    jonctions(p).0
}

/// Coupes des extrémités raccordées et interruptions des couches de finition traversées
/// par le noyau d'un mur arrivant (jonction des noyaux, comme dans Revit).
pub fn jonctions(p: &Projet) -> (Coupes, Interruptions) {
    let ls = logiques(p);
    // (mur logique, extrémité 0 début / 1 fin) → candidats (distance, coupe, est un L,
    // coupe des couches porteuses)
    let mut candidats: HashMap<(usize, usize), Vec<(f64, Coupe, bool, Option<Coupe>)>> = HashMap::new();
    let mut interruptions: Interruptions = HashMap::new();
    // arrivée en T de `arrivant` (extrémité e) sur `porteur` au point x : finitions
    // arrêtées sur la face, noyau prolongé jusqu'à la face du noyau du porteur, couches
    // de finition du porteur interrompues au passage
    let mut arrivee = |candidats: &mut HashMap<(usize, usize), Vec<(f64, Coupe, bool, Option<Coupe>)>>, k: usize, e: usize, dist: f64, porteur: &Logique, arrivant: &Logique, x: DVec2, loin: DVec2| {
        let dp = (porteur.b - porteur.a).normalize();
        let n = dp.perp();
        let cote = (loin - x).dot(n).signum();
        let face = cote * porteur.epaisseur / 2.0;
        let coupe = Coupe { point: (x + n * face).to_array(), direction: dp.to_array() };
        let mut noyau_coupe = None;
        if let (Some((lo, hi)), Some((alo, ahi))) = (porteur.noyau, arrivant.noyau) {
            let face_noyau = if cote > 0.0 { hi } else { lo };
            if (face - face_noyau).abs() > 0.5 {
                noyau_coupe = Some(Coupe { point: (x + n * face_noyau).to_array(), direction: dp.to_array() });
                // bords du noyau arrivant, coupés par la face et la face du noyau du porteur
                let da = (arrivant.b - arrivant.a).normalize();
                let mut s: Vec<f64> = Vec::new();
                for o in [alo, ahi] {
                    for f in [face, face_noyau] {
                        if let Some(q) = intersection(x + da.perp() * o, da, x + n * f, dp) {
                            s.push((q - porteur.a).dot(dp));
                        }
                    }
                }
                let (s0, s1) = s.iter().fold((f64::MAX, f64::MIN), |(a, b), &t| (a.min(t), b.max(t)));
                if s1 > s0 {
                    for m in porteur.membres.iter().filter(|m| !m.porteur && m.ecart * cote > 0.0 && (m.ecart - face_noyau) * cote > 0.0) {
                        let Some(mur) = p.murs.iter().find(|x| x.id == m.id) else { continue };
                        let Axe::Droit { debut, fin } = mur.axe else { continue };
                        let longueur = v(debut).distance(v(fin));
                        let t0 = (v(debut) - porteur.a).dot(dp);
                        let (a, b) = if m.sens { (s0 - t0, s1 - t0) } else { (t0 - s1, t0 - s0) };
                        if b > 0.0 && a < longueur {
                            interruptions.entry(m.id.clone()).or_default().push((a.max(0.0), b.min(longueur)));
                        }
                    }
                }
            }
        }
        candidats.entry((k, e)).or_default().push((dist, coupe, false, noyau_coupe));
    };
    // extrémités prolongées par un mur aligné bout à bout : jonction droite, sans onglet
    let mut continues: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for i in 0..ls.len() {
        for j in i + 1..ls.len() {
            let (a, b) = (&ls[i], &ls[j]);
            let (da, db) = ((a.b - a.a).normalize(), (b.b - b.a).normalize());
            if da.perp_dot(db).abs() >= 0.05 || (b.a - a.a).perp_dot(da).abs() > 5.0 {
                continue;
            }
            for (ea, pa) in [(0, a.a), (1, a.b)] {
                for (eb, pb) in [(0, b.a), (1, b.b)] {
                    if pa.distance(pb) <= 5.0 {
                        continues.insert((i, ea));
                        continues.insert((j, eb));
                    }
                }
            }
        }
    }
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
            let (ca, cb) = (continues.contains(&(i, ea)), continues.contains(&(j, eb)));
            if dist_a <= tol && dist_b <= tol && (ca || cb) {
                // nœud d'un mur continu (deux murs alignés) : le troisième s'arrête sur sa face
                if ca && cb {
                    continue;
                }
                let (k, e, dist, porteur, arrivant) = if ca { (j, eb, dist_b, a, b) } else { (i, ea, dist_a, b, a) };
                arrivee(&mut candidats, k, e, dist, porteur, arrivant, x, loin(arrivant, e));
            } else if dist_a <= tol && dist_b <= tol {
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
                candidats.entry((i, ea)).or_default().push((dist_a, c, true, None));
                candidats.entry((j, eb)).or_default().push((dist_b, c, true, None));
            } else {
                // arrivée en T : arrêt sur la face du mur traversé, côté de l'arrivant
                for (k, e, dist, porteur, arrivant) in [(i, ea, dist_a, b, a), (j, eb, dist_b, a, b)] {
                    if dist <= tol && interieur(porteur) {
                        arrivee(&mut candidats, k, e, dist, porteur, arrivant, x, loin(arrivant, e));
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
        let Some(&(_, coupe, _, noyau)) = cs.first() else { continue };
        for m in &ls[k].membres {
            // extrémité du membre qui correspond à celle du mur logique ; couches porteuses
            // prolongées jusqu'au noyau du mur rejoint
            let bout = if m.sens { e } else { 1 - e };
            let c = if m.porteur { noyau.unwrap_or(coupe) } else { coupe };
            out.entry(m.id.clone()).or_insert([None, None])[bout] = Some(c);
        }
    }
    (out, interruptions)
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
    fn murs_alignes_continus_et_t_au_noeud() {
        let mut p = Projet::nouveau("N");
        let o = ParametresMur { epaisseur: 200.0, ..Default::default() };
        p.ajouter_mur([0.0, 0.0], [3000.0, 0.0], &o).unwrap();
        p.ajouter_mur([3000.0, 0.0], [6000.0, 0.0], &o).unwrap();
        p.ajouter_mur([3000.0, 0.0], [3000.0, 3000.0], &o).unwrap();
        let cs = coupes(&p);
        let c: Vec<_> = p.murs.iter().map(|m| contour(&p, m, &cs).unwrap()).collect();
        let proche = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
        // les deux murs alignés se touchent bout à bout, extrémités droites en x = 3000
        assert!(proche(c[0][1], [3000.0, -100.0]) && proche(c[0][2], [3000.0, 100.0]), "{:?}", c[0]);
        assert!(proche(c[1][0], [3000.0, -100.0]) && proche(c[1][3], [3000.0, 100.0]), "{:?}", c[1]);
        // le troisième s'arrête sur leur face y = 100
        assert!((c[2][0][1] - 100.0).abs() < 1e-6 && (c[2][3][1] - 100.0).abs() < 1e-6, "{:?}", c[2]);
    }

    #[test]
    fn noyau_prolonge_jusqu_au_noyau_du_mur_rejoint() {
        let mut p = Projet::nouveau("N");
        let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f };
        p.types_murs.push(TypeCompose { id: "t".into(), nom: "Mur".into(), couches: vec![c("Enduit", 20.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Isolant", 100.0, FonctionCouche::Isolation)], couches_detaillees: true, origine: None });
        let o = ParametresMur { type_mur: Some("t".into()), ..Default::default() };
        // porteur de x = 0 à 6000 (extérieur, l'enduit, à gauche : y > 0), refend arrivant
        // par le haut (côté enduit)
        p.ajouter_mur([0.0, 0.0], [6000.0, 0.0], &o).unwrap();
        p.ajouter_mur([3000.0, 3000.0], [3000.0, 0.0], &o).unwrap();
        let (cs, trous) = jonctions(&p);
        let porteur = p.assemblages[0].couches.clone();
        let refend = p.assemblages[1].couches.clone();
        // enduit du porteur interrompu au passage du béton du refend (200 mm de large)
        let t = &trous[&porteur[0]];
        assert_eq!(t.len(), 1);
        assert!((t[0].1 - t[0].0 - 200.0).abs() < 1.0, "{t:?}");
        assert!(!trous.contains_key(&porteur[1]));
        // béton du refend arrêté sur la face du béton du porteur, enduit et isolant sur la face
        let y = |id: &String| { let m = p.murs.iter().find(|m| &m.id == id).unwrap(); contour(&p, m, &cs).unwrap()[1][1] };
        let (face_noyau, face) = (y(&refend[1]), y(&refend[0]));
        assert!(face - face_noyau > 15.0, "{face} {face_noyau}");
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
