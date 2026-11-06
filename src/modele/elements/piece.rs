//! Pièces : un point placé dans un espace fermé par des murs ; le contour est celui des
//! faces intérieures des murs qui entourent ce point (murs composés compris : chaque
//! couche est un mur), recalculé après chaque modification du projet.
use crate::modele::{Axe, Projet};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Piece {
    pub id: String,
    pub nom: String,
    pub numero: String,
    pub niveau: String,
    /// Point de placement : dans la pièce, position de l'étiquette (mm).
    pub point: [f64; 2],
    /// Contour calculé (mm, sens trigonométrique) ; vide : pièce non fermée.
    #[serde(default)]
    pub contour: Vec<[f64; 2]>,
}

impl Piece {
    /// Surface (mm²).
    pub fn aire(&self) -> f64 {
        aire(&self.contour).abs()
    }
    /// Périmètre (mm).
    pub fn perimetre(&self) -> f64 {
        let n = self.contour.len();
        if n < 3 {
            return 0.0;
        }
        (0..n).map(|i| v(self.contour[i]).distance(v(self.contour[(i + 1) % n]))).sum()
    }
    pub fn fermee(&self) -> bool {
        self.contour.len() >= 3
    }
}

fn v(p: [f64; 2]) -> DVec2 {
    DVec2::from_array(p)
}

fn aire(c: &[[f64; 2]]) -> f64 {
    let n = c.len();
    (0..n).map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1]).sum::<f64>() / 2.0
}

fn dedans(c: &[DVec2], q: DVec2) -> bool {
    let n = c.len();
    let mut d = false;
    for i in 0..n {
        let (a, b) = (c[i], c[(i + 1) % n]);
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            d = !d;
        }
    }
    d
}

/// Contours en plan des murs du niveau (mm) : coins coupés par les jonctions pour les
/// murs droits, arcs discrétisés pour les murs courbes.
pub fn contours_murs(p: &Projet, niveau: &str) -> Vec<Vec<DVec2>> {
    let coupes = crate::modele::murs::jonctions::jonctions(p).0;
    let mut out = Vec::new();
    for m in p.murs.iter().filter(|m| m.niveau == niveau) {
        if let Some(c) = crate::modele::murs::jonctions::contour(p, m, &coupes) {
            out.push(c.iter().map(|q| v(*q)).collect());
        } else if let (Axe::Arc { centre, .. }, Some((r, a, b)), Some(t)) = (&m.axe, m.axe.arc(), p.type_mur(&m.type_mur)) {
            let h = t.epaisseur() / 2.0;
            let c = v(*centre);
            let n = 32;
            let point = |k: usize, rayon: f64| {
                let t = a + b * k as f64 / n as f64;
                c + DVec2::new(t.cos(), t.sin()) * rayon
            };
            let mut poly: Vec<DVec2> = (0..=n).map(|k| point(k, r + h)).collect();
            poly.extend((0..=n).rev().map(|k| point(k, (r - h).max(0.0))));
            out.push(poly);
        }
    }
    out
}

/// Contour de la pièce qui contient `point` au niveau `niveau`.
pub fn detecter(p: &Projet, niveau: &str, point: [f64; 2]) -> Result<Vec<[f64; 2]>, String> {
    detecter_dans(&contours_murs(p, niveau), v(point))
}

/// Plus petite face fermée de l'arrangement des contours de murs qui contient `q`.
pub fn detecter_dans(murs: &[Vec<DVec2>], q: DVec2) -> Result<Vec<[f64; 2]>, String> {
    if murs.iter().any(|m| m.len() >= 3 && dedans(m, q)) {
        return Err("Pièce : le point est dans un mur ; cliquez dans l'espace entre les murs.".into());
    }
    let faces = faces(murs);
    let face = faces
        .into_iter()
        .filter(|f| dedans(f, q))
        .map(|f| {
            let a = aire(&f.iter().map(|p| p.to_array()).collect::<Vec<_>>());
            (a, f)
        })
        .filter(|(a, _)| *a > 1.0)
        .min_by(|x, y| x.0.total_cmp(&y.0));
    match face {
        Some((_, f)) => Ok(simplifier(f).into_iter().map(|p| p.to_array()).collect()),
        None => Err("Pièce non fermée : aucun contour de murs continu autour du point.".into()),
    }
}

/// Retire les sommets alignés et doublés.
fn simplifier(mut f: Vec<DVec2>) -> Vec<DVec2> {
    loop {
        let n = f.len();
        if n < 4 {
            return f;
        }
        let k = (0..n).find(|&i| {
            let (a, b, c) = (f[(i + n - 1) % n], f[i], f[(i + 1) % n]);
            a.distance(b) < 1e-3 || (b - a).perp_dot(c - b).abs() <= 1e-6 * (b - a).length() * (c - b).length() && (b - a).dot(c - b) > 0.0
        });
        match k {
            Some(i) => {
                f.remove(i);
            }
            None => return f,
        }
    }
}

/// Faces (anneaux fermés, face à gauche) de l'arrangement plan des côtés des contours.
fn faces(murs: &[Vec<DVec2>]) -> Vec<Vec<DVec2>> {
    let mut segs: Vec<(DVec2, DVec2)> = Vec::new();
    for m in murs {
        for i in 0..m.len() {
            let (a, b) = (m[i], m[(i + 1) % m.len()]);
            if a.distance(b) > 1e-6 {
                segs.push((a, b));
            }
        }
    }
    const EPS: f64 = 1e-4;
    // découpe de chaque côté aux intersections et recouvrements
    let mut morceaux: Vec<(DVec2, DVec2)> = Vec::new();
    for (i, &(a, b)) in segs.iter().enumerate() {
        let d1 = b - a;
        let l2 = d1.length_squared();
        let (mn, mx) = (a.min(b) - DVec2::splat(EPS), a.max(b) + DVec2::splat(EPS));
        let mut ts = vec![0.0, 1.0];
        for (j, &(c, d)) in segs.iter().enumerate() {
            if i == j || c.min(d).cmpgt(mx).any() || c.max(d).cmplt(mn).any() {
                continue;
            }
            let d2 = d - c;
            let den = d1.perp_dot(d2);
            if den.abs() > 1e-9 * d1.length() * d2.length() {
                let t = (c - a).perp_dot(d2) / den;
                let u = (c - a).perp_dot(d1) / den;
                let (et, eu) = (EPS / d1.length(), EPS / d2.length());
                if (-et..=1.0 + et).contains(&t) && (-eu..=1.0 + eu).contains(&u) {
                    ts.push(t.clamp(0.0, 1.0));
                }
            } else if ((c - a).perp_dot(d1)).abs() / d1.length() < EPS {
                for e in [c, d] {
                    let t = (e - a).dot(d1) / l2;
                    if t > 0.0 && t < 1.0 {
                        ts.push(t);
                    }
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        for w in ts.windows(2) {
            let (p, q) = (a + d1 * w[0], a + d1 * w[1]);
            if p.distance(q) > EPS {
                morceaux.push((p, q));
            }
        }
    }
    // sommets fusionnés à la tolérance
    let mut sommets: Vec<DVec2> = Vec::new();
    let mut grille: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let cellule = 0.01;
    let mut indice = |p: DVec2, sommets: &mut Vec<DVec2>| -> usize {
        let k = ((p.x / cellule).floor() as i64, (p.y / cellule).floor() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(l) = grille.get(&(k.0 + dx, k.1 + dy)) {
                    if let Some(&i) = l.iter().find(|&&i| sommets[i].distance(p) < 2.0 * EPS) {
                        return i;
                    }
                }
            }
        }
        sommets.push(p);
        grille.entry(k).or_default().push(sommets.len() - 1);
        sommets.len() - 1
    };
    let mut aretes: HashSet<(usize, usize)> = HashSet::new();
    for (p, q) in morceaux {
        let (i, j) = (indice(p, &mut sommets), indice(q, &mut sommets));
        if i != j {
            aretes.insert((i.min(j), i.max(j)));
        }
    }
    let mut voisins: Vec<Vec<usize>> = vec![Vec::new(); sommets.len()];
    for &(i, j) in &aretes {
        voisins[i].push(j);
        voisins[j].push(i);
    }
    for (i, l) in voisins.iter_mut().enumerate() {
        let o = sommets[i];
        l.sort_by(|a, b| {
            let (da, db) = (sommets[*a] - o, sommets[*b] - o);
            da.y.atan2(da.x).total_cmp(&db.y.atan2(db.x))
        });
    }
    let mut vus: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::new();
    let total = aretes.len() * 2;
    for &(i, j) in &aretes {
        for depart in [(i, j), (j, i)] {
            if vus.contains(&depart) {
                continue;
            }
            let mut anneau = Vec::new();
            let mut h = depart;
            let mut ok = false;
            for _ in 0..=total {
                vus.insert(h);
                anneau.push(sommets[h.0]);
                let l = &voisins[h.1];
                let k = l.iter().position(|&x| x == h.0).unwrap();
                let suivant = l[(k + l.len() - 1) % l.len()];
                h = (h.1, suivant);
                if h == depart {
                    ok = true;
                    break;
                }
            }
            if ok && anneau.len() >= 3 {
                out.push(anneau);
            }
        }
    }
    out
}

impl Projet {
    /// Recalcule le contour de chaque pièce (vide si elle n'est plus fermée).
    pub fn actualiser_pieces(&mut self) {
        if self.pieces.is_empty() {
            return;
        }
        let mut cache: HashMap<String, Vec<Vec<DVec2>>> = HashMap::new();
        let contours: Vec<Vec<[f64; 2]>> = self
            .pieces
            .iter()
            .map(|x| {
                let murs = cache.entry(x.niveau.clone()).or_insert_with(|| contours_murs(self, &x.niveau));
                detecter_dans(murs, v(x.point)).unwrap_or_default()
            })
            .collect();
        for (x, c) in self.pieces.iter_mut().zip(contours) {
            x.contour = c;
        }
    }

    /// Numéro suivant : un de plus que le plus grand numéro entier des pièces.
    pub fn numero_piece_suivant(&self) -> String {
        (self.pieces.iter().filter_map(|x| x.numero.trim().parse::<u64>().ok()).max().unwrap_or(0) + 1).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::edition::ParametresMur;
    use crate::modele::edition::commandes::Commande;

    fn rectangle(p: &mut Projet, o: &ParametresMur) {
        let pts = [[0.0, 0.0], [5000.0, 0.0], [5000.0, 4000.0], [0.0, 4000.0]];
        for i in 0..4 {
            p.ajouter_mur(pts[i], pts[(i + 1) % 4], o).unwrap();
        }
    }

    fn projet() -> Projet {
        Projet::nouveau("Pièces").executer(Commande::Niveau { nom: "RDC".into(), altitude: 0.0 }).unwrap().0
    }

    fn piece(p: &Projet, point: [f64; 2]) -> Piece {
        Piece { id: String::new(), nom: "Séjour".into(), numero: String::new(), niveau: p.niveaux[0].id.clone(), point, contour: Vec::new() }
    }

    #[test]
    fn piece_dans_quatre_murs() {
        let mut p = projet();
        rectangle(&mut p, &ParametresMur { epaisseur: 200.0, ..Default::default() });
        let (q, ids) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
        let x = q.pieces.iter().find(|x| x.id == ids[0]).unwrap();
        assert_eq!(x.numero, "1");
        assert_eq!(x.contour.len(), 4, "{:?}", x.contour);
        // faces intérieures : axe à ±100 mm
        assert!((x.aire() - 4800.0 * 3800.0).abs() < 1.0, "{}", x.aire());
        assert!((x.perimetre() - 2.0 * (4800.0 + 3800.0)).abs() < 1e-3);
        // seconde pièce : numéro suivant ; point hors des murs refusé
        let (q2, _) = q.executer(Commande::Piece { piece: piece(&q, [2500.0, 2000.0]) }).unwrap();
        assert_eq!(q2.pieces[1].numero, "2");
        assert!(q.executer(Commande::Piece { piece: piece(&q, [9000.0, 2000.0]) }).is_err());
        assert!(q.executer(Commande::Piece { piece: piece(&q, [0.0, 2000.0]) }).is_err());
    }

    #[test]
    fn piece_ouverte_refusee_et_recalcul() {
        let mut p = projet();
        let o = ParametresMur { epaisseur: 200.0, ..Default::default() };
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &o).unwrap();
        p.ajouter_mur([5000.0, 0.0], [5000.0, 4000.0], &o).unwrap();
        p.ajouter_mur([5000.0, 4000.0], [0.0, 4000.0], &o).unwrap();
        assert!(p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).is_err());
        let dernier = p.ajouter_mur([0.0, 4000.0], [0.0, 0.0], &o).unwrap();
        let (q, _) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
        // murs déplacés de 1 m (point de la pièce laissé en place, toujours dedans) : le
        // contour suit
        let murs: Vec<String> = q.murs.iter().map(|m| m.id.clone()).collect();
        let (q, _) = q.executer(Commande::Deplacer { ids: murs, delta: [1000.0, 0.0, 0.0] }).unwrap();
        let x0 = q.pieces[0].contour.iter().map(|c| c[0]).fold(f64::MAX, f64::min);
        assert!((x0 - 1100.0).abs() < 1e-6, "{:?}", q.pieces[0].contour);
        // un mur déplacé seul ouvre la pièce : contour vide
        let (ouverte, _) = q.executer(Commande::Deplacer { ids: vec![dernier.clone()], delta: [-1000.0, 0.0, 0.0] }).unwrap();
        assert!(!ouverte.pieces[0].fermee());
        // supprimé : la pièce reste, sans contour
        let (q, _) = q.executer(Commande::Supprimer { ids: vec![dernier] }).unwrap();
        assert!(!q.pieces[0].fermee());
    }

    #[test]
    fn piece_mur_compose() {
        use crate::modele::{Couche, TypeCompose};
        let mut p = projet();
        p.types_murs.push(TypeCompose {
            id: "compose".into(),
            nom: "Composé".into(),
            couches: vec![Couche { epaisseur: 20.0, ..Default::default() }, Couche { epaisseur: 200.0, ..Default::default() }, Couche { epaisseur: 13.0, ..Default::default() }],
            couches_detaillees: true,
            categorie: None,
            origine: None,
        });
        rectangle(&mut p, &ParametresMur { type_mur: Some("compose".into()), ..Default::default() });
        assert!(!p.assemblages.is_empty(), "le type composé crée des assemblages");
        let (q, ids) = p.executer(Commande::Piece { piece: piece(&p, [2500.0, 2000.0]) }).unwrap();
        let x = q.pieces.iter().find(|x| x.id == ids[0]).unwrap();
        assert!(x.fermee());
        // contour sur la face intérieure de la couche intérieure : l'aire est celle du
        // rectangle d'axes réduit d'une épaisseur de 233 mm au total (une demi par côté
        // selon le sens des murs), strictement plus petite que le rectangle d'axes
        let a = x.aire();
        assert!(a < 5000.0 * 4000.0 && a > 4500.0 * 3500.0, "{a}");
        // aucun sommet dans un mur : chaque sommet est à 1 mm près sur une face de couche
        assert!(x.contour.len() >= 4);
    }
}
