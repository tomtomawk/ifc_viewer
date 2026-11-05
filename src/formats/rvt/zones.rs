//! Zones de surface (`RoomElem` avec `m_areaSchemeId`) : surface et périmètre
//! (LISEZMOI-RVT.md du dossier Decodage RVT, § 7 « Zones de surface »).
//!
//! Revit enregistre la topologie de plan de chaque niveau et schéma (`LevelRoomPlan`,
//! `Topologie`) : la surface d'une pièce ou d'une zone est celle de son circuit
//! (`RoomElem.m_cachedCircuitId`). À défaut, la surface d'une zone se déduit des lignes
//! de zone (`CurveElem` avec `m_isAreaMeasurementScheme` = 1) de même schéma
//! (`m_areaSchemeId`) et de même niveau (`m_assocLevelId`) : plus petite face de leur
//! arrangement qui contient le point de la zone (`m_point`), îlots retirés (`surface`).

use super::champs::{Obj, Record, Val};
use glam::DVec2;
use std::collections::HashMap;

/// Topologie de plan d'un niveau (`LevelRoomPlan.m_planTopology`), calculée par Revit
/// et enregistrée : segments des limites (`m_segments`, clé élément + `m_subIndex`,
/// points `m_info.m_points`), circuits (`m_circuits` : `m_id`, surface `m_area` en
/// pieds², côtés `m_sides`, `m_subIndex` négatif = ~indice, segment parcouru à
/// l'envers), composantes (`m_outerCircuit`, `m_enclosingCircuit` : îlot dans un
/// circuit), pièces ou zones placées (`m_rooms`).
pub struct Topologie {
    pub pieces: Vec<i64>,
    segments: HashMap<(i64, i64), f64>,
    circuits: HashMap<i64, (f64, Vec<(i64, i64)>)>,
    composantes: Vec<(i64, i64)>,
}

fn corps<'r, 's>(r: &'r Record<'s>, v: &'r Val<'s>) -> Option<&'r Obj<'s>> {
    r.deref(v).map(|b| &b.obj).or(v.obj())
}

/// Clé d'un segment ou d'un côté : (élément, `m_subIndex`).
fn cle(r: &Record, o: &Obj) -> Option<(i64, i64)> {
    let item = o.get("m_item").and_then(|v| corps(r, v))?;
    Some((item.get("m_linkInstOrHostId").and_then(Val::element_id)?, o.get("m_subIndex").and_then(Val::int).unwrap_or(0)))
}

fn id_circuit(r: &Record, v: Option<&Val>) -> Option<i64> {
    v.and_then(|v| corps(r, v))?.get("m_id").and_then(Val::int)
}

impl Topologie {
    /// Lit `LevelRoomPlan`.
    pub fn lire(r: &Record) -> Option<Topologie> {
        let t = r.root.get("m_planTopology").and_then(|v| corps(r, v))?;
        let liste = |k: &str| t.get(k).map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).collect::<Vec<_>>();
        let mut segments = HashMap::new();
        for s in liste("m_segments") {
            let (Some(k), Some(info)) = (s.get("m_key").and_then(|v| corps(r, v)).and_then(|k| cle(r, k)), s.get("m_info").and_then(|v| corps(r, v))) else { continue };
            let pts: Vec<DVec2> = info.get("m_points").map(Val::list).unwrap_or_default().iter().filter_map(|p| match p.reals().as_deref() {
                Some(&[x, y]) => Some(DVec2::new(x, y)),
                _ => None,
            }).collect();
            segments.insert(k, pts.windows(2).map(|w| w[0].distance(w[1])).sum());
        }
        let mut circuits = HashMap::new();
        for c in liste("m_circuits") {
            let (Some(id), Some(aire)) = (id_circuit(r, c.get("m_id")), c.get("m_area").and_then(Val::real)) else { continue };
            let cotes = c.get("m_sides").map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).filter_map(|o| cle(r, o)).collect();
            circuits.insert(id, (aire, cotes));
        }
        let composantes = liste("m_components").iter().filter_map(|c| Some((id_circuit(r, c.get("m_outerCircuit"))?, id_circuit(r, c.get("m_enclosingCircuit")).unwrap_or(-1)))).collect();
        let pieces = t.get("m_rooms").map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id).collect();
        Some(Topologie { pieces, segments, circuits, composantes })
    }

    /// Surface (pieds²) et longueur des côtés (pieds) d'un circuit, îlots compris (leur
    /// contour extérieur, d'aire négative, s'ajoute).
    pub fn mesure(&self, circuit: i64) -> Option<(f64, f64)> {
        let un = |c: i64| -> Option<(f64, f64)> {
            let (aire, cotes) = self.circuits.get(&c)?;
            let l = cotes.iter().map(|&(e, k)| self.segments.get(&(e, if k >= 0 { k } else { !k })).copied()).sum::<Option<f64>>()?;
            Some((*aire, l))
        };
        let (mut a, mut p) = un(circuit)?;
        for &(ext, dans) in &self.composantes {
            if dans == circuit {
                let (a2, p2) = un(ext)?;
                a += a2;
                p += p2;
            }
        }
        Some((a, p))
    }
}

/// Tolérance de fusion des sommets, mm.
const TOL: f64 = 0.5;

/// Surface (mm²) et périmètre (mm) de la face contenant `p`, None si `p` n'est dans
/// aucune face fermée.
pub fn surface(segments: &[(DVec2, DVec2)], p: DVec2) -> Option<(f64, f64)> {
    // découpe des segments à leurs intersections et contacts
    let mut coupes: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; segments.len()];
    for i in 0..segments.len() {
        for j in i + 1..segments.len() {
            let ((a, b), (c, d)) = (segments[i], segments[j]);
            let (r, s) = (b - a, d - c);
            let den = r.perp_dot(s);
            if den.abs() > 1e-12 {
                let t = (c - a).perp_dot(s) / den;
                let u = (c - a).perp_dot(r) / den;
                let (et, eu) = (TOL / r.length().max(1e-9), TOL / s.length().max(1e-9));
                if t > -et && t < 1.0 + et && u > -eu && u < 1.0 + eu {
                    coupes[i].push(t.clamp(0.0, 1.0));
                    coupes[j].push(u.clamp(0.0, 1.0));
                }
            } else {
                // colinéaires : extrémités de l'un sur l'autre
                for (k, (x, y), q) in [(i, (a, b), c), (i, (a, b), d), (j, (c, d), a), (j, (c, d), b)] {
                    let v = y - x;
                    let t = (q - x).dot(v) / v.length_squared().max(1e-12);
                    if (x + v * t).distance(q) < TOL && t > 0.0 && t < 1.0 {
                        coupes[k].push(t);
                    }
                }
            }
        }
    }
    // sommets fusionnés et arêtes
    let mut sommets: Vec<DVec2> = Vec::new();
    let mut index: HashMap<(i64, i64), usize> = HashMap::new();
    let mut sommet = |q: DVec2| -> usize {
        let cle = ((q.x / TOL).round() as i64, (q.y / TOL).round() as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(&k) = index.get(&(cle.0 + dx, cle.1 + dy)) {
                    if sommets[k].distance(q) < TOL {
                        return k;
                    }
                }
            }
        }
        sommets.push(q);
        index.insert(cle, sommets.len() - 1);
        sommets.len() - 1
    };
    let mut aretes: Vec<(usize, usize)> = Vec::new();
    for (k, (a, b)) in segments.iter().enumerate() {
        let mut t = coupes[k].clone();
        t.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let pts: Vec<usize> = t.iter().map(|&t| sommet(*a + (*b - *a) * t)).collect();
        for w in pts.windows(2) {
            if w[0] != w[1] {
                aretes.push((w[0].min(w[1]), w[0].max(w[1])));
            }
        }
    }
    aretes.sort_unstable();
    aretes.dedup();
    // bouts pendants retirés
    loop {
        let mut degre = vec![0usize; sommets.len()];
        for &(u, v) in &aretes {
            degre[u] += 1;
            degre[v] += 1;
        }
        let n = aretes.len();
        aretes.retain(|&(u, v)| degre[u] > 1 && degre[v] > 1);
        if aretes.len() == n {
            break;
        }
    }
    // demi-arêtes : sortantes de chaque sommet rangées par angle
    let mut sortantes: HashMap<usize, Vec<usize>> = HashMap::new();
    for &(u, v) in &aretes {
        sortantes.entry(u).or_default().push(v);
        sortantes.entry(v).or_default().push(u);
    }
    for (u, l) in sortantes.iter_mut() {
        let o = sommets[*u];
        l.sort_by(|&a, &b| {
            let (x, y) = (sommets[a] - o, sommets[b] - o);
            x.y.atan2(x.x).partial_cmp(&y.y.atan2(y.x)).unwrap()
        });
    }
    // faces : après (u → v), l'arête qui précède (v → u) dans l'ordre trigonométrique
    // autour de v (face à gauche, aire positive pour une face bornée)
    let mut vues: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for &(a, b) in &aretes {
        for (u0, v0) in [(a, b), (b, a)] {
            if vues.contains(&(u0, v0)) {
                continue;
            }
            let mut face = Vec::new();
            let (mut u, mut v) = (u0, v0);
            while vues.insert((u, v)) {
                face.push(u);
                let l = &sortantes[&v];
                let k = l.iter().position(|&w| w == u).unwrap();
                let w = l[(k + l.len() - 1) % l.len()];
                (u, v) = (v, w);
            }
            faces.push(face);
        }
    }
    let aire = |f: &[usize]| (0..f.len()).map(|k| sommets[f[k]].perp_dot(sommets[f[(k + 1) % f.len()]])).sum::<f64>() / 2.0;
    let longueur = |f: &[usize]| (0..f.len()).map(|k| sommets[f[k]].distance(sommets[f[(k + 1) % f.len()]])).sum::<f64>();
    let dedans = |f: &[usize], q: DVec2| {
        let mut c = false;
        for k in 0..f.len() {
            let (a, b) = (sommets[f[k]], sommets[f[(k + 1) % f.len()]]);
            if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                c = !c;
            }
        }
        c
    };
    // face bornée la plus petite contenant p
    let zone = faces.iter().filter(|f| aire(f) > 0.0 && dedans(f, p)).min_by(|a, b| aire(a).partial_cmp(&aire(b)).unwrap())?;
    // îlots : contours extérieurs (aire négative) d'autres composantes, dans la zone,
    // ne contenant pas p
    let mut s = aire(zone);
    let mut perimetre = longueur(zone);
    for f in faces.iter().filter(|f| aire(f) < 0.0 && !f.iter().any(|v| zone.contains(v))) {
        if f.iter().all(|&v| dedans(zone, sommets[v])) && !dedans(f, p) {
            s += aire(f);
            perimetre += longueur(f);
        }
    }
    Some((s, perimetre))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carre_coupe_et_ilot() {
        let v = |x: f64, y: f64| DVec2::new(x, y);
        // carré 10×10 coupé en deux par une ligne qui dépasse, îlot 2×2 dans la moitié
        // gauche, bout pendant
        let s = [
            (v(0.0, 0.0), v(10.0, 0.0)),
            (v(10.0, 0.0), v(10.0, 10.0)),
            (v(10.0, 10.0), v(0.0, 10.0)),
            (v(0.0, 10.0), v(0.0, 0.0)),
            (v(5.0, -3.0), v(5.0, 12.0)),
            (v(1.0, 1.0), v(3.0, 1.0)),
            (v(3.0, 1.0), v(3.0, 3.0)),
            (v(3.0, 3.0), v(1.0, 3.0)),
            (v(1.0, 3.0), v(1.0, 1.0)),
            (v(8.0, 5.0), v(9.0, 6.0)),
        ];
        let (a, p) = surface(&s, v(4.0, 8.0)).unwrap();
        assert!((a - 46.0).abs() < 1e-9 && (p - 38.0).abs() < 1e-9, "{a} {p}");
        let (a, p) = surface(&s, v(7.0, 2.0)).unwrap();
        assert!((a - 50.0).abs() < 1e-9 && (p - 30.0).abs() < 1e-9, "{a} {p}");
        assert!(surface(&s, v(20.0, 2.0)).is_none());
    }
}
