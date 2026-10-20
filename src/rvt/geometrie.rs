//! Géométrie calculée d'un élément : le `GElement` de la section 0x67
//! (docs/LISEZMOI-RVT.md § 7 « Géométrie calculée » ; portage de
//! `tools/rvt-research/geometrie.py`).
//!
//! Le `GElement` est un arbre de `GGroup` ; chaque `Geometry` (`GBRep`) est un B-rep.
//! Une face porte une surface (`Plane`, `CylSurf`) et une chaîne de boucles
//! (`EdgeLoop.m_nextLoop`). Une boucle et ses arêtes forment une liste circulaire :
//! `EdgeLoop.m_next` → arête, puis `Edge.m_next[s]` jusqu'au retour à la boucle, s étant
//! le côté de l'arête qui touche la face (`Edge.m_pFace[s]`). Une arête ne porte pas de
//! courbe 3D : ses points sont donnés en (u, v) sur chacune de ses deux faces. Le côté
//! s parcourt l'arête du premier au dernier point si s = `m_flags & 1`, en sens inverse
//! sinon ; ainsi parcourue, une boucle extérieure tourne dans le sens direct autour de
//! la normale sortante, un trou en sens inverse. Unités internes : pieds ; les sommets
//! rendus sont en mm.

use super::champs::{Obj, Record, Val};
use glam::{DVec2, DVec3};
use std::collections::{BTreeSet, HashMap};

/// mm par pied (unité interne de Revit).
pub const FT: f64 = 304.8;

pub struct Face {
    /// Matériau (`GFace.m_renderStyleId`).
    pub materiau: i64,
    /// Triangles en mm, sens direct autour de la normale sortante.
    pub triangles: Vec<[DVec3; 3]>,
}

pub struct FaceMaillee {
    pub materiau: i64,
    pub triangles: Vec<[usize; 3]>,
}

/// Maillage indexé, sommets fusionnés à 10⁻⁶ mm.
pub struct Maillage {
    pub sommets: Vec<DVec3>,
    pub faces: Vec<FaceMaillee>,
}

#[derive(Debug, Clone, Copy)]
pub struct Controle {
    pub min: DVec3,
    pub max: DVec3,
    pub volume_m3: f64,
    pub aire_m2: f64,
    /// Arêtes de triangles non appariées (0 : maillage fermé et orienté).
    pub non_appariees: usize,
    pub triangles: usize,
}

fn err<T>(m: impl Into<String>) -> Result<T, String> {
    Err(m.into())
}

fn vec3(o: &Obj, name: &str) -> Result<DVec3, String> {
    match o.get(name).and_then(Val::reals).as_deref() {
        Some(&[x, y, z]) => Ok(DVec3::new(x, y, z)),
        _ => err(format!("{}.{name} : 3 réels attendus", o.class_name())),
    }
}

fn real(o: &Obj, name: &str) -> Result<f64, String> {
    o.get(name).and_then(Val::real).ok_or_else(|| format!("{}.{name} : réel attendu", o.class_name()))
}

/// ElementId d'un champ (−1 s'il est absent).
fn eid(o: &Obj, name: &str) -> i64 {
    o.get(name).and_then(Val::element_id).unwrap_or(-1)
}

/// Point (pieds) de la surface s au paramètre (u, v).
pub fn point(s: &Obj, u: f64, v: f64) -> Result<DVec3, String> {
    match s.class_name() {
        "Plane" => Ok(vec3(s, "m_origin")? + u * vec3(s, "m_xVec")? + v * vec3(s, "m_yVec")?),
        "CylSurf" => {
            let r = real(s, "m_radius")?;
            let (su, cu) = u.sin_cos();
            Ok(vec3(s, "m_center")? + r * (cu * vec3(s, "m_xVec")? + su * vec3(s, "m_yVec")?) + v * vec3(s, "m_zVec")?)
        }
        k => err(format!("surface {k} non décodée")),
    }
}

/// Racine `GElement` d'un enregistrement 0x67.
pub fn gelement<'r, 's>(rec: &'r Record<'s>) -> Result<&'r Obj<'s>, String> {
    if rec.root.class_name() != "GElement" {
        return err(format!("enregistrement 0x67 de classe {}, pas de géométrie", rec.root.class_name()));
    }
    Ok(&rec.root)
}

/// Nœuds de l'arbre, en profondeur.
fn noeuds<'r, 's>(rec: &'r Record<'s>, n: &'r Obj<'s>, out: &mut Vec<&'r Obj<'s>>) {
    out.push(n);
    if out.len() > 100_000 {
        return;
    }
    for p in n.get("m_subNodes").map(Val::list).unwrap_or_default() {
        if let Some(b) = rec.deref(p) {
            noeuds(rec, &b.obj, out);
        }
    }
}

/// Points (u, v) d'une arête sur son côté s, du premier au dernier point.
fn points_arete(e: &Obj, s: usize) -> Result<Vec<DVec2>, String> {
    let ends = e.get("m_firstAndLastEdgePnts").map(Val::list).unwrap_or_default();
    let inner = e.get("m_interiorEdgePnts").map(Val::list).unwrap_or_default();
    if ends.len() != 2 {
        return err("Edge.m_firstAndLastEdgePnts : 2 points attendus");
    }
    let seq = std::iter::once(&ends[0]).chain(inner).chain(std::iter::once(&ends[1]));
    seq.map(|p| {
        let uv = p.obj().and_then(|o| o.get("uv")).map(Val::list).unwrap_or_default();
        match uv.get(s).and_then(Val::reals).as_deref() {
            Some(&[u, v]) => Ok(DVec2::new(u, v)),
            _ => err("EdgePnt.uv illisible"),
        }
    })
    .collect()
}

fn refs(e: &Obj, name: &str) -> [Option<u32>; 2] {
    let l = e.get(name).map(Val::list).unwrap_or_default();
    [l.first().and_then(Val::reference), l.get(1).and_then(Val::reference)]
}

/// Boucles de la face : listes de (numéro d'arête, [(u, v)…]) dans l'ordre de parcours
/// de la face, chaque arête dans le sens de ce parcours.
fn boucles<'s>(rec: &Record<'s>, fnum: u32, face: &Obj<'s>) -> Result<Vec<Vec<(u32, Vec<DVec2>)>>, String> {
    let mut out = Vec::new();
    let mut lp = face.get("m_pFirstLoop").and_then(|v| rec.deref(v));
    while let Some(b) = lp {
        let (lnum, l) = (b.num, &b.obj);
        let mut pts = Vec::new();
        let mut prev = lnum;
        let mut cur = l.get("m_next").and_then(Val::reference);
        while cur != Some(lnum) {
            let Some(k) = cur else { return err(format!("boucle {lnum} interrompue")) };
            let e = &rec.by_num(k).ok_or_else(|| format!("arête {k} introuvable"))?.obj;
            let (faces, precs) = (refs(e, "m_pFace"), refs(e, "m_prev"));
            let cotes: Vec<usize> = (0..2).filter(|&s| faces[s] == Some(fnum) && precs[s] == Some(prev)).collect();
            let [s] = cotes[..] else { return err(format!("arête {k} : côté introuvable sur la face {fnum}")) };
            let mut uv = points_arete(e, s)?;
            let flags = e.get("m_flags").and_then(Val::int).unwrap_or(0);
            if s as i64 != flags & 1 {
                uv.reverse();
            }
            pts.push((k, uv));
            prev = k;
            cur = refs(e, "m_next")[s];
            if pts.len() > rec.bodies.len() {
                return err(format!("boucle {lnum} non refermée"));
            }
        }
        out.push(pts);
        lp = l.get("m_nextLoop").and_then(|v| rec.deref(v));
    }
    Ok(out)
}

fn cross(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn aire(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].x * poly[(i + 1) % n].y - poly[(i + 1) % n].x * poly[i].y).sum::<f64>() / 2.0
}

/// p strictement dans le polygone (règle pair-impair).
fn dans(p: DVec2, poly: &[DVec2]) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            c = !c;
        }
    }
    c
}

/// Les segments pq et ab se coupent-ils en dehors de leurs extrémités communes ?
fn coupe(p: DVec2, q: DVec2, a: DVec2, b: DVec2) -> bool {
    if p == a || p == b || q == a || q == b {
        return false;
    }
    let (d1, d2, d3, d4) = (cross(p, q, a), cross(p, q, b), cross(a, b, p), cross(a, b, q));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Premier indice du maximum (comme `max` de Python).
fn argmax(n: usize, key: impl Fn(usize) -> f64) -> usize {
    (1..n).fold(0, |best, i| if key(i) > key(best) { i } else { best })
}

/// Premier indice du minimum (comme `min` de Python).
fn argmin(items: &[usize], key: impl Fn(usize) -> f64) -> Option<usize> {
    items.iter().copied().fold(None, |best, i| match best {
        Some(b) if key(i) >= key(b) => Some(b),
        _ => Some(i),
    })
}

/// Triangles (indices) d'une face : `polys` = boucles en indices de `uv` / `xyz`,
/// extérieures dans un sens, trous dans l'autre. Oreilles coupées dans le plan (u, v),
/// la plus courte diagonale 3D d'abord ; chaque triangle garde le sens de parcours des
/// boucles (donc de la normale sortante). Trous reliés au contour par un pont.
pub fn trianguler(uv: &[DVec2], xyz: &[DVec3], polys: &[Vec<usize>]) -> Result<Vec<[usize; 3]>, String> {
    if polys.is_empty() {
        return Ok(vec![]);
    }
    let pts = |p: &[usize]| p.iter().map(|&i| uv[i]).collect::<Vec<_>>();
    let aires: Vec<f64> = polys.iter().map(|p| aire(&pts(p))).collect();
    let sg = if aires[argmax(polys.len(), |k| aires[k].abs())] > 0.0 { 1.0 } else { -1.0 };
    let ext: Vec<&Vec<usize>> = polys.iter().zip(&aires).filter(|(_, a)| **a * sg > 0.0).map(|(p, _)| p).collect();
    let trous: Vec<&Vec<usize>> = polys.iter().zip(&aires).filter(|(_, a)| **a * sg < 0.0).map(|(p, _)| p).collect();
    let max_u = |t: &[usize]| t.iter().map(|&i| uv[i].x).fold(f64::NEG_INFINITY, f64::max);
    let mut tris = Vec::new();
    for o in ext {
        let mut poly = o.clone();
        let contour = pts(o);
        let mut mes: Vec<&Vec<usize>> = trous.iter().copied().filter(|t| dans(uv[t[0]], &contour)).collect();
        mes.sort_by(|a, b| (-max_u(a)).total_cmp(&-max_u(b)));
        for t in &mes {
            // pont entre le sommet le plus à droite du trou et le sommet visible le plus proche
            let k = argmax(t.len(), |j| uv[t[j]].x);
            let m = uv[t[k]];
            let mut bords = Vec::new();
            for c in std::iter::once(&poly).chain(mes.iter().copied()) {
                for i in 0..c.len() {
                    bords.push((uv[c[i]], uv[c[(i + 1) % c.len()]]));
                }
            }
            let vis: Vec<usize> = (0..poly.len()).filter(|&j| !bords.iter().any(|&(a, b)| coupe(m, uv[poly[j]], a, b))).collect();
            let j = argmin(&vis, |j| m.distance(uv[poly[j]])).ok_or("trou sans pont visible")?;
            let mut p = poly[..=j].to_vec();
            p.extend(&t[k..]);
            p.extend(&t[..=k]);
            p.extend(&poly[j..]);
            poly = p;
        }
        while poly.len() > 3 {
            let n = poly.len();
            let tri_at = |i: usize| [poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]];
            let mut best: Option<(f64, usize)> = None;
            for i in 0..n {
                let [a, b, c] = tri_at(i);
                if cross(uv[a], uv[b], uv[c]) * sg <= 1e-12 {
                    continue;
                }
                let tri = [uv[a], uv[b], uv[c]];
                let contient = poly.iter().any(|&p| !tri.contains(&uv[p]) && (0..3).all(|q| cross(tri[q], tri[(q + 1) % 3], uv[p]) * sg >= -1e-12));
                if contient {
                    continue;
                }
                let d = xyz[a].distance(xyz[c]);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, i));
                }
            }
            match best {
                Some((_, i)) => {
                    tris.push(tri_at(i));
                    poly.remove(i);
                }
                None => {
                    // sommets alignés restants : on retire le moins saillant sans triangle
                    let idx: Vec<usize> = (0..n).collect();
                    let i = argmin(&idx, |i| {
                        let [a, b, c] = tri_at(i);
                        cross(uv[a], uv[b], uv[c]).abs()
                    })
                    .unwrap();
                    poly.remove(i);
                }
            }
        }
        if poly.len() == 3 && cross(uv[poly[0]], uv[poly[1]], uv[poly[2]]).abs() > 1e-12 {
            tris.push([poly[0], poly[1], poly[2]]);
        }
    }
    Ok(tris)
}

/// Faces des B-rep de l'élément. Une `Geometry` est un solide si chaque arête de ses
/// boucles a deux faces ; les feuilles (plans de référence d'un mur) ne sont gardées
/// qu'avec `tout`. Les arêtes hors boucle de `m_pEdges` (murs joints) sont ignorées.
pub fn faces(rec: &Record, tout: bool) -> Result<Vec<Face>, String> {
    let root = gelement(rec)?;
    let mut ns = Vec::new();
    noeuds(rec, root, &mut ns);
    let mut out = Vec::new();
    for n in ns {
        let Some(pfaces) = n.get("m_pFaces") else { continue };
        let mut faces = Vec::new();
        let mut aretes = BTreeSet::new();
        for fp in pfaces.list() {
            let Some(fb) = rec.deref(fp) else { continue };
            let f = &fb.obj;
            let s = rec.field_body(f, "m_pSurf").ok_or("face sans surface")?;
            if rec.field_body(f, "m_pFirstLoop").is_none() {
                continue; // face sans boucle (E09 : faces de l'ouverture confondues avec celles du mur)
            }
            let (mut uv, mut polys) = (Vec::new(), Vec::new());
            for b in boucles(rec, fb.num, f)? {
                let mut poly = Vec::new();
                for (i, (k, ar)) in b.iter().enumerate() {
                    let suiv = &b[(i + 1) % b.len()].1;
                    // contrôle du sens de parcours : fin de chaque arête = début de la suivante
                    let (fin, debut) = (ar.last().unwrap(), suiv[0]);
                    if point(s, fin.x, fin.y)?.distance(point(s, debut.x, debut.y)?) > 1e-6 {
                        return err(format!("boucle de la face {} discontinue", fb.num));
                    }
                    aretes.insert(*k);
                    for p in &ar[..ar.len() - 1] {
                        poly.push(uv.len());
                        uv.push(*p);
                    }
                }
                polys.push(poly);
            }
            let xyz = uv.iter().map(|p| point(s, p.x, p.y)).collect::<Result<Vec<_>, _>>()?;
            let mm: Vec<DVec3> = xyz.iter().map(|q| *q * FT).collect();
            faces.push(Face {
                materiau: eid(f, "m_renderStyleId"),
                triangles: trianguler(&uv, &xyz, &polys)?.into_iter().map(|t| t.map(|i| mm[i])).collect(),
            });
        }
        let ferme = aretes.iter().all(|&k| rec.by_num(k).is_some_and(|e| refs(&e.obj, "m_pFace")[1].is_some_and(|f| f != 0)));
        if ferme || tout {
            out.extend(faces);
        }
    }
    Ok(out)
}

/// Maillage indexé des solides de l'élément.
pub fn maillage(rec: &Record, tout: bool) -> Result<Maillage, String> {
    let mut m = Maillage { sommets: Vec::new(), faces: Vec::new() };
    let mut index: HashMap<[i64; 3], usize> = HashMap::new();
    for f in faces(rec, tout)? {
        let triangles = f
            .triangles
            .iter()
            .map(|t| {
                t.map(|p| {
                    let k = p.to_array().map(|c| (c * 1e6).round() as i64);
                    *index.entry(k).or_insert_with(|| {
                        m.sommets.push(p);
                        m.sommets.len() - 1
                    })
                })
            })
            .collect();
        m.faces.push(FaceMaillee { materiau: f.materiau, triangles });
    }
    Ok(m)
}

impl Maillage {
    pub fn triangles(&self) -> impl Iterator<Item = [DVec3; 3]> + '_ {
        self.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| self.sommets[i])))
    }

    /// Boîte englobante (mm), volume (m³, théorème de la divergence), aire (m²) et
    /// nombre d'arêtes de triangles non appariées.
    pub fn controle(&self) -> Controle {
        let (mut vol, mut aire) = (0.0, 0.0);
        let mut dirigees: HashMap<(usize, usize), u32> = HashMap::new();
        let mut n = 0;
        for f in &self.faces {
            for &[i, j, k] in &f.triangles {
                let (a, b, c) = (self.sommets[i], self.sommets[j], self.sommets[k]);
                vol += a.dot(b.cross(c)) / 6.0;
                aire += (b - a).cross(c - a).length() / 2.0;
                for e in [(i, j), (j, k), (k, i)] {
                    *dirigees.entry(e).or_default() += 1;
                }
                n += 1;
            }
        }
        let non_appariees = dirigees.iter().filter(|&(&(a, b), &c)| c != 1 || dirigees.get(&(b, a)) != Some(&1)).count();
        let min = self.sommets.iter().copied().fold(DVec3::splat(f64::INFINITY), DVec3::min);
        let max = self.sommets.iter().copied().fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max);
        Controle { min, max, volume_m3: vol / 1e9, aire_m2: aire / 1e6, non_appariees, triangles: n }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carre_troue() {
        // carré 10 × 10 sens direct, trou 2 × 2 sens inverse
        let uv: Vec<DVec2> = [(0., 0.), (10., 0.), (10., 10.), (0., 10.), (4., 4.), (4., 6.), (6., 6.), (6., 4.)]
            .iter()
            .map(|&(u, v)| DVec2::new(u, v))
            .collect();
        let xyz: Vec<DVec3> = uv.iter().map(|p| p.extend(0.0)).collect();
        let tris = trianguler(&uv, &xyz, &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]]).unwrap();
        let a: f64 = tris.iter().map(|t| cross(uv[t[0]], uv[t[1]], uv[t[2]]) / 2.0).sum();
        assert!((a - 96.0).abs() < 1e-9, "aire {a}");
        assert!(tris.iter().all(|t| cross(uv[t[0]], uv[t[1]], uv[t[2]]) > 0.0));
    }
}
