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

use super::champs::{self, Obj, Record, Val};
use super::schema::Schema;
use glam::{DMat3, DVec2, DVec3};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

/// mm par pied (unité interne de Revit).
pub const FT: f64 = 304.8;

pub struct Face {
    /// Matériau (`GFace.m_renderStyleId`).
    pub materiau: i64,
    /// Triangles en pieds, sens direct autour de la normale sortante.
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
    /// Solides non décodés (surface ou courbe inconnue, boucle discontinue…), écartés.
    pub echecs: Vec<String>,
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

/// Point (pieds) de la surface s au paramètre (u, v). Surface réglée (`RuledSurf`) :
/// u ∈ [0, 1] parcourt les deux courbes de profil entre leurs `m_endParams`, v ∈ [0, 1]
/// va de la première à la seconde (une courbe absente est remplacée par `m_Point1` ou
/// `m_Point2`) ; vérifié par l'écart nul des arêtes partagées avec les faces planes et
/// par le volume d'un sol de Kerreut 2025 (1,100868 m³ dans Revit).
pub fn point(rec: &Record, s: &Obj, u: f64, v: f64) -> Result<DVec3, String> {
    match s.class_name() {
        "Plane" => Ok(vec3(s, "m_origin")? + u * vec3(s, "m_xVec")? + v * vec3(s, "m_yVec")?),
        "CylSurf" => {
            let r = real(s, "m_radius")?;
            let (su, cu) = u.sin_cos();
            Ok(vec3(s, "m_center")? + r * (cu * vec3(s, "m_xVec")? + su * vec3(s, "m_yVec")?) + v * vec3(s, "m_zVec")?)
        }
        "RuledSurf" => {
            let profil = |c: &str, p: &str| match rec.field_body(s, c) {
                Some(c) => point_courbe(c, u),
                None => vec3(s, p),
            };
            let (a, b) = (profil("m_pProfileCurve1", "m_Point1")?, profil("m_pProfileCurve2", "m_Point2")?);
            Ok(a + v * (b - a))
        }
        "SurfRev" => {
            // profil donné dans le repère local (m_xVec, m_yVec, m_zVec), tourné de u autour de z
            let c = rec.field_body(s, "m_pProfileCurve").ok_or("SurfRev sans profil")?;
            let q = point_courbe_abs(c, v)?;
            let (su, cu) = u.sin_cos();
            Ok(vec3(s, "m_center")? + (q.x * cu - q.y * su) * vec3(s, "m_xVec")? + (q.x * su + q.y * cu) * vec3(s, "m_yVec")? + q.z * vec3(s, "m_zVec")?)
        }
        k => err(format!("surface {k} non décodée")),
    }
}

/// Point (pieds) d'une `GLine`, d'un `GArc` ou d'une `GHermiteSpline` au paramètre
/// normalisé s ∈ [0, 1] entre ses `m_endParams`.
///
/// `GHermiteSpline` : nœuds (`m_iParametr`, `m_iPoint`, `m_iTangent`), le paramètre
/// étant l'abscisse curviligne et la tangente unitaire ; entre deux nœuds, Hermite
/// cubique de pas h = écart des paramètres, tangentes × h. Sur une volée hélicoïdale de
/// Saint-Malo, les arêtes partagées avec les faces cylindriques s'écartent d'au plus
/// 0,06 mm (la spline approche l'hélice exacte), contre 21 mm sans le facteur h.
fn point_courbe(c: &Obj, s: f64) -> Result<DVec3, String> {
    match c.get("m_endParams").and_then(Val::reals).as_deref() {
        Some(&[t0, t1]) => point_courbe_abs(c, t0 + s * (t1 - t0)),
        _ => err(format!("{}.m_endParams : 2 réels attendus", c.class_name())),
    }
}

/// Point (pieds) d'une courbe à son paramètre propre t.
fn point_courbe_abs(c: &Obj, t: f64) -> Result<DVec3, String> {
    match c.class_name() {
        "GLine" => Ok(vec3(c, "m_origin")? + t * vec3(c, "m_dirVec")?),
        "GArc" => {
            let (st, ct) = t.sin_cos();
            Ok(vec3(c, "m_center")? + real(c, "m_radius")? * (ct * vec3(c, "m_xVec")? + st * vec3(c, "m_yVec")?))
        }
        "GHermiteSpline" => {
            let noeuds = c.get("m_NodeArray").map(Val::list).unwrap_or_default();
            let noeud = |i: usize| -> Result<(f64, DVec3, DVec3), String> {
                let n = noeuds.get(i).and_then(Val::obj).ok_or("GHermiteSpline : nœud illisible")?;
                Ok((real(n, "m_iParametr")?, vec3(n, "m_iPoint")?, vec3(n, "m_iTangent")?))
            };
            if noeuds.len() < 2 {
                return err("GHermiteSpline : moins de 2 nœuds");
            }
            let mut i = 0;
            while i + 2 < noeuds.len() && t > noeud(i + 1)?.0 {
                i += 1;
            }
            let ((t0, p0, m0), (t1, p1, m1)) = (noeud(i)?, noeud(i + 1)?);
            let h = t1 - t0;
            let x = (t - t0) / h;
            let (x2, x3) = (x * x, x * x * x);
            Ok((2.0 * x3 - 3.0 * x2 + 1.0) * p0 + (x3 - 2.0 * x2 + x) * h * m0 + (3.0 * x2 - 2.0 * x3) * p1 + (x3 - x2) * h * m1)
        }
        k => err(format!("courbe {k} non décodée")),
    }
}

/// Racine `GElement` d'un enregistrement 0x67.
pub fn gelement<'r, 's>(rec: &'r Record<'s>) -> Result<&'r Obj<'s>, String> {
    if rec.root.class_name() != "GElement" {
        return err(format!("enregistrement 0x67 de classe {}, pas de géométrie", rec.root.class_name()));
    }
    Ok(&rec.root)
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
    // un sommet de ab posé à l'intérieur de pq bloque aussi : le pont vers un trou
    // passait par le coin d'un autre trou aligné (fenêtre de BIBLIO.rvt)
    let l = q - p;
    let dedans = |x: DVec2| {
        let t = (x - p).dot(l) / l.length_squared();
        x != p && x != q && t > 0.0 && t < 1.0 && cross(p, q, x).abs() <= 1e-9 * l.length_squared()
    };
    if dedans(a) || dedans(b) {
        return true;
    }
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
pub fn trianguler(uv: &[DVec2], xyz: &[DVec3], polys: &[Vec<usize>], surface: Option<&dyn Fn(DVec2) -> Option<DVec3>>) -> Result<Vec<[usize; 3]>, String> {
    if polys.is_empty() {
        return Ok(vec![]);
    }
    // décisions sur des (u, v) arrondis à 10⁻⁹ pi : des sommets alignés à 10⁻¹⁴ près (coins de
    // trous rectangulaires d'une fenêtre de BIBLIO.rvt) faisaient partir un pont du mauvais
    // coin et laissaient des fentes nulles où le découpage échouait
    let uv: &[DVec2] = &uv.iter().map(|p| (*p * 1e9).round() / 1e9).collect::<Vec<_>>();
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
            let mut best: Option<((f64, f64), usize)> = None;
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
                // flèche de la diagonale (milieu (u, v) sur la surface contre milieu de la
                // corde), arrondie à 10⁻⁶ pi : nulle sur un plan, nulle le long des génératrices
                let fleche = surface.and_then(|p| p((uv[a] + uv[c]) / 2.0)).map_or(0.0, |m| (m.distance((xyz[a] + xyz[c]) / 2.0) * 1e6).round());
                let cle = (fleche, xyz[a].distance(xyz[c]));
                if best.is_none_or(|b| cle < b.0) {
                    best = Some((cle, i));
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

/// Bits de `GInfo.m_flags` d'un objet non visible (ouverture de fenêtre, plans de
/// référence…) : `get_Geometry` de l'API l'écarte (`Options.IncludeNonVisibleObjects`).
const INVISIBLE: i64 = 0xE0;

/// Niveau de détail retenu (`ViewDetailLevel.Fine` de l'API).
const DETAIL_FIN: i64 = 3;

/// Flèche maximale (pieds) des faces courbes densifiées : 2 mm.
const TOLERANCE: f64 = 2.0 / FT;

/// Points de grille au plus, par axe, dans une face courbe.
const GRILLE_MAX: usize = 64;

/// Écart toléré (pieds) entre la fin d'une arête et le début de la suivante : la
/// tolérance de sommet de Revit (0,0005 pi = 0,15 mm). Les familles laissent des écarts
/// de 0,007 à 0,017 mm (douche de Saint-Malo) ; les murs, aucun (≤ 3·10⁻¹¹ mm).
const TOLERANCE_SOMMET: f64 = 5e-4;

/// Condition d'un `GFilter`, évaluée pour une vue 3D en détail fin (règles vérifiées
/// contre l'API sur 448 instances, docs/LISEZMOI-RVT.md § 7) : `GConditionDir` et
/// `GConditionCut` sont vraies si niées (vue non parallèle à l'axe, élément non coupé),
/// `GConditionInt` de paramètre 3 compare le niveau de détail (`m_comp` 0 : différent,
/// 3 : égal) ; les autres sont tenues pour vraies.
fn condition(c: &Obj) -> bool {
    let int = |k| c.get(k).and_then(Val::int).unwrap_or(-1);
    match c.class_name() {
        "GConditionDir" | "GConditionCut" => int("m_negateDirCondition") == 1,
        "GConditionInt" if int("m_param") == 3 => match int("m_comp") {
            0 => int("m_value") != DETAIL_FIN,
            3 => int("m_value") == DETAIL_FIN,
            _ => true,
        },
        _ => true,
    }
}

/// Repère d'une instance (`InstInfoBase.m_Trf`) : p' = origine + R·p, en pieds ; la
/// colonne j de `m_3x3` (composantes `m_3x3[i][j]`) est l'axe j (214 instances sur 214
/// conformes à `GetTotalTransform` de l'API).
#[derive(Clone, Copy)]
struct Trf {
    r: DMat3,
    o: DVec3,
}

impl Trf {
    const ID: Trf = Trf { r: DMat3::IDENTITY, o: DVec3::ZERO };

    fn lire(t: &Obj) -> Option<Trf> {
        let l = t.get("m_3x3")?.list();
        let ligne = |i: usize| -> Option<DVec3> {
            match l.get(i)?.reals().as_deref() {
                Some(&[a, b, c]) => Some(DVec3::new(a, b, c)),
                _ => None,
            }
        };
        let r = DMat3::from_cols(ligne(0)?, ligne(1)?, ligne(2)?).transpose();
        Some(Trf { r, o: vec3(t, "m_or").ok()? })
    }

    /// Ce repère appliqué après `local`.
    fn apres(&self, local: &Trf) -> Trf {
        Trf { r: self.r * local.r, o: self.r * local.o + self.o }
    }
}

/// Faces et échecs de décodage d'un type de famille, dans son repère.
type FacesType = Arc<(Vec<Face>, Vec<String>)>;

/// Faces des types (`GElement` de la section 0x67 d'un `Symbol` : type de famille,
/// marches d'escalier…), calculées une fois dans le repère du type puis partagées par
/// leurs instances.
pub struct Bibliotheque<'a, 's> {
    sch: &'s Schema,
    octets: &'a HashMap<i64, Vec<u8>>,
    cache: Mutex<HashMap<i64, FacesType>>,
}

impl<'a, 's> Bibliotheque<'a, 's> {
    pub fn new(sch: &'s Schema, octets: &'a HashMap<i64, Vec<u8>>) -> Self {
        Bibliotheque { sch, octets, cache: Mutex::new(HashMap::new()) }
    }

    /// Faces (pieds, repère du type) et échecs de décodage du type `id`.
    fn type_famille(&self, id: i64, prof: u32) -> Option<FacesType> {
        if let Some(f) = self.cache.lock().unwrap().get(&id) {
            return Some(f.clone());
        }
        let rec = champs::read(self.sch, self.octets.get(&id)?).ok()?;
        let mut p = Parcours { bib: self, tout: false, faces: Vec::new(), echecs: Vec::new() };
        p.noeud(&rec, gelement(&rec).ok()?, &Trf::ID, prof);
        let f = Arc::new((p.faces, p.echecs));
        self.cache.lock().unwrap().insert(id, f.clone());
        Some(f)
    }
}

struct Parcours<'b, 'a, 's> {
    bib: &'b Bibliotheque<'a, 's>,
    tout: bool,
    faces: Vec<Face>,
    echecs: Vec<String>,
}

impl Parcours<'_, '_, '_> {
    /// Parcourt un nœud et ses sous-nœuds ; `prof` compte les imbrications de familles.
    fn noeud(&mut self, rec: &Record, n: &Obj, t: &Trf, prof: u32) {
        let flags = n.get("m_GInfo").and_then(Val::obj).and_then(|g| g.get("m_flags")).and_then(Val::int).unwrap_or(0);
        if flags & INVISIBLE != 0 && !self.tout {
            return;
        }
        match n.class_name() {
            "GFilter" => {
                let conds = n.get("m_oConditions").map(Val::list).unwrap_or_default();
                if !conds.iter().filter_map(|c| rec.deref(c)).all(|c| condition(&c.obj)) {
                    return;
                }
            }
            "GInstance" => return self.instance(rec, n, t, prof),
            _ => {}
        }
        if n.get("m_pFaces").is_some() {
            match solide(rec, n, self.tout) {
                Ok(Some(fs)) => self.ajouter(fs.iter(), t),
                Ok(None) => {}
                Err(e) => self.echecs.push(e),
            }
        }
        for p in n.get("m_subNodes").map(Val::list).unwrap_or_default() {
            if let Some(b) = rec.deref(p) {
                self.noeud(rec, &b.obj, t, prof);
            }
        }
    }

    /// Instance de famille : géométrie incorporée (`m_oEmbeddedSymbolGRep`) ou celle du
    /// type `m_instanceInfo.m_symbolId`, placée par `m_instanceInfo.m_Trf`.
    fn instance(&mut self, rec: &Record, n: &Obj, t: &Trf, prof: u32) {
        if prof > 16 {
            return self.echecs.push("familles imbriquées trop profondément".into());
        }
        let Some(ii) = rec.field_body(n, "m_instanceInfo") else { return };
        let Some(local) = ii.get("m_Trf").and_then(Val::obj).and_then(Trf::lire) else {
            return self.echecs.push("GInstance sans repère lisible".into());
        };
        let t2 = t.apres(&local);
        if let Some(emb) = rec.field_body(n, "m_oEmbeddedSymbolGRep") {
            return self.noeud(rec, emb, &t2, prof + 1);
        }
        let id = ii.get("m_symbolId").and_then(Val::element_id).unwrap_or(-1);
        match self.bib.type_famille(id, prof + 1) {
            Some(f) => {
                self.ajouter(f.0.iter(), &t2);
                self.echecs.extend(f.1.iter().cloned());
            }
            None => self.echecs.push(format!("géométrie du type {id} absente")),
        }
    }

    /// Ajoute des faces placées par t (sens des triangles inversé pour un repère miroir).
    fn ajouter<'f>(&mut self, faces: impl Iterator<Item = &'f Face>, t: &Trf) {
        let miroir = t.r.determinant() < 0.0;
        for f in faces {
            let triangles = f
                .triangles
                .iter()
                .map(|tr| {
                    let [a, b, c] = tr.map(|p| t.r * p + t.o);
                    if miroir { [a, c, b] } else { [a, b, c] }
                })
                .collect();
            self.faces.push(Face { materiau: f.materiau, triangles });
        }
    }
}

/// Faces (pieds) d'une `Geometry` si c'est un solide fermé de volume positif ; une
/// feuille (plans de référence d'un mur) n'est gardée qu'avec `tout`. Les arêtes hors
/// boucle de `m_pEdges` (murs joints) sont ignorées.
fn solide(rec: &Record, n: &Obj, tout: bool) -> Result<Option<Vec<Face>>, String> {
    let mut faces = Vec::new();
    let mut aretes = BTreeSet::new();
    for fp in n.get("m_pFaces").map(Val::list).unwrap_or_default() {
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
                if point(rec, s, fin.x, fin.y)?.distance(point(rec, s, debut.x, debut.y)?) > TOLERANCE_SOMMET {
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
        let mut xyz = uv.iter().map(|p| point(rec, s, p.x, p.y)).collect::<Result<Vec<_>, _>>()?;
        let surface = |q: DVec2| point(rec, s, q.x, q.y).ok();
        let mut tris = trianguler(&uv, &xyz, &polys, Some(&surface))?;
        if s.class_name() != "Plane" {
            densifier(&mut uv, &mut xyz, &polys, &mut tris, &surface);
        }
        faces.push(Face { materiau: eid(f, "m_renderStyleId"), triangles: tris.into_iter().map(|t| t.map(|i| xyz[i])).collect() });
    }
    let ferme = aretes.iter().all(|&k| rec.by_num(k).is_some_and(|e| refs(&e.obj, "m_pFace")[1].is_some_and(|f| f != 0)));
    // Un solide fermé de volume négatif est un vide (réservation d'une famille : l'API de
    // Revit lui donne le même volume négatif et le dit coupant) ; ignoré.
    let volume: f64 = faces.iter().flat_map(|f| &f.triangles).map(|[a, b, c]| a.dot(b.cross(*c))).sum();
    Ok((tout || (ferme && volume >= 0.0)).then_some(faces))
}

/// Points intérieurs d'une face courbe : sans eux, la triangulation du seul contour
/// aplatit les faces doublement courbes (couronne d'un arbre : −51 % de volume). Grille
/// (u, v) dont le pas garde la flèche sous `TOLERANCE` le long des isoparamétriques du
/// bord et du milieu ; chaque point strictement intérieur est inséré dans son triangle,
/// puis les arêtes voisines basculent selon le critère de Delaunay dans un plan (u, v)
/// mis à l'échelle 3D. Les arêtes du contour, partagées avec les faces voisines, ne
/// bougent pas : le maillage reste fermé.
fn densifier(uv: &mut Vec<DVec2>, xyz: &mut Vec<DVec3>, polys: &[Vec<usize>], tris: &mut Vec<[usize; 3]>, p: &dyn Fn(DVec2) -> Option<DVec3>) {
    if tris.is_empty() {
        return;
    }
    let (lo, hi) = uv.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
    let d = hi - lo;
    if d.x < 1e-12 || d.y < 1e-12 {
        return;
    }
    let mi = (lo + hi) / 2.0;
    let segments = |axe_u: bool| -> usize {
        let at = |t: f64, f: f64| if axe_u { DVec2::new(t, f) } else { DVec2::new(f, t) };
        let (a, b, fixes) = if axe_u { (lo.x, hi.x, [lo.y, mi.y, hi.y]) } else { (lo.y, hi.y, [lo.x, mi.x, hi.x]) };
        let mut n = 1;
        while n < GRILLE_MAX {
            let ok = fixes.iter().all(|&f| {
                (0..n).all(|k| {
                    let (t0, t1) = (a + (b - a) * k as f64 / n as f64, a + (b - a) * (k + 1) as f64 / n as f64);
                    match (p(at(t0, f)), p(at(t1, f)), p(at((t0 + t1) / 2.0, f))) {
                        (Some(x), Some(y), Some(m)) => m.distance((x + y) / 2.0) <= TOLERANCE,
                        _ => true,
                    }
                })
            });
            if ok {
                return n;
            }
            n *= 2;
        }
        GRILLE_MAX
    };
    let (nu, nv) = (segments(true), segments(false));
    if nu < 2 || nv < 2 {
        return; // génératrices droites (cylindre, surface réglée) : pas de point intérieur utile
    }
    // échelle 3D des axes au centre de la face
    let h = 1e-4;
    let echelle = |du: DVec2| match (p(mi + du), p(mi - du)) {
        (Some(a), Some(b)) => (a.distance(b) / (2.0 * du.length())).max(1e-9),
        _ => 1.0,
    };
    let k = DVec2::new(echelle(DVec2::new(h * d.x, 0.0)), echelle(DVec2::new(0.0, h * d.y)));
    let s = |uv: &[DVec2], i: usize| uv[i] * k;
    let sens = if tris.iter().map(|t| cross(s(uv, t[0]), s(uv, t[1]), s(uv, t[2]))).sum::<f64>() > 0.0 { 1.0 } else { -1.0 };
    let mut bord = std::collections::HashSet::new();
    for poly in polys {
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            bord.insert((a.min(b), a.max(b)));
        }
    }
    // arête orientée → triangle qui la porte
    let mut aretes: HashMap<(usize, usize), usize> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for j in 0..3 {
            aretes.insert((t[j], t[(j + 1) % 3]), ti);
        }
    }
    let poser = |aretes: &mut HashMap<(usize, usize), usize>, tris: &mut Vec<[usize; 3]>, ti: usize, t: [usize; 3]| {
        if ti == tris.len() {
            tris.push(t);
        } else {
            let ancien = tris[ti];
            for j in 0..3 {
                let e = (ancien[j], ancien[(j + 1) % 3]);
                if aretes.get(&e) == Some(&ti) {
                    aretes.remove(&e);
                }
            }
            tris[ti] = t;
        }
        for j in 0..3 {
            aretes.insert((t[j], t[(j + 1) % 3]), ti);
        }
    };
    let contour: Vec<Vec<DVec2>> = polys.iter().map(|poly| poly.iter().map(|&i| uv[i]).collect()).collect();
    let pas = DVec2::new(d.x / nu as f64, d.y / nv as f64);
    for i in 1..nu {
        for j in 1..nv {
            let q = lo + DVec2::new(pas.x * i as f64, pas.y * j as f64);
            if contour.iter().filter(|c| dans(q, c)).count() % 2 == 0 {
                continue;
            }
            if contour.iter().flatten().any(|c| (c.x - q.x).abs() < 0.3 * pas.x && (c.y - q.y).abs() < 0.3 * pas.y) {
                continue; // trop près du contour
            }
            let sq = q * k;
            let Some(ti) = tris.iter().position(|t| (0..3).all(|m| sens * cross(s(uv, t[m]), s(uv, t[(m + 1) % 3]), sq) > 0.0)) else { continue };
            let Some(pq) = p(q) else { continue };
            let pi = uv.len();
            uv.push(q);
            xyz.push(pq);
            let [a, b, c] = tris[ti];
            poser(&mut aretes, tris, ti, [a, b, pi]);
            let n = tris.len();
            poser(&mut aretes, tris, n, [b, c, pi]);
            poser(&mut aretes, tris, n + 1, [c, a, pi]);
            let mut pile = vec![(a, b), (b, c), (c, a)];
            while let Some((x, y)) = pile.pop() {
                if bord.contains(&(x.min(y), x.max(y))) {
                    continue;
                }
                let (Some(&t1), Some(&t2)) = (aretes.get(&(x, y)), aretes.get(&(y, x))) else { continue };
                let Some(&o) = tris[t2].iter().find(|&&z| z != x && z != y) else { continue };
                if sens * incercle(s(uv, x), s(uv, y), s(uv, pi), s(uv, o)) > 0.0 {
                    poser(&mut aretes, tris, t1, [x, o, pi]);
                    poser(&mut aretes, tris, t2, [o, y, pi]);
                    pile.push((x, o));
                    pile.push((o, y));
                }
            }
        }
    }
}

/// d dans le cercle circonscrit à (a, b, c) : signe selon le sens de (a, b, c).
fn incercle(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> f64 {
    let (a, b, c) = (a - d, b - d, c - d);
    let (la, lb, lc) = (a.length_squared(), b.length_squared(), c.length_squared());
    a.x * (b.y * lc - lb * c.y) - a.y * (b.x * lc - lb * c.x) + la * (b.x * c.y - b.y * c.x)
}

/// Maillage indexé (mm) des solides de l'élément, familles comprises ; les solides non
/// décodés sont comptés dans `Maillage::echecs`. Erreur si aucun ne l'est.
pub fn maillage(rec: &Record, tout: bool, bib: &Bibliotheque) -> Result<Maillage, String> {
    let mut p = Parcours { bib, tout, faces: Vec::new(), echecs: Vec::new() };
    p.noeud(rec, gelement(rec)?, &Trf::ID, 0);
    if let (true, Some(e)) = (p.faces.is_empty(), p.echecs.first()) {
        return Err(e.clone());
    }
    let mut m = Maillage { sommets: Vec::new(), faces: Vec::new(), echecs: p.echecs };
    let mut index: HashMap<[i64; 3], usize> = HashMap::new();
    for f in p.faces {
        let triangles = f
            .triangles
            .iter()
            .map(|t| {
                t.map(|p| {
                    let p = p * FT;
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
    /// Ajoute les faces d'un autre maillage (volées et paliers d'un escalier) ; les
    /// sommets ne sont pas fusionnés.
    pub fn ajouter(&mut self, autre: Maillage) {
        let n = self.sommets.len();
        self.sommets.extend(autre.sommets);
        self.echecs.extend(autre.echecs);
        self.faces.extend(autre.faces.into_iter().map(|f| FaceMaillee { materiau: f.materiau, triangles: f.triangles.into_iter().map(|t| t.map(|i| i + n)).collect() }));
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
        let tris = trianguler(&uv, &xyz, &[vec![0, 1, 2, 3], vec![4, 5, 6, 7]], None).unwrap();
        let a: f64 = tris.iter().map(|t| cross(uv[t[0]], uv[t[1]], uv[t[2]]) / 2.0).sum();
        assert!((a - 96.0).abs() < 1e-9, "aire {a}");
        assert!(tris.iter().all(|t| cross(uv[t[0]], uv[t[1]], uv[t[2]]) > 0.0));
    }
}
