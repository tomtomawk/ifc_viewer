//! Dessin graphique des arbres (style Graphique, plans) : houppier en grappe de volumes
//! bosselés et tronc en 3D, symbole de plan par essence (contour festonné, étoilé ou
//! cercle, tronc et croix au pied). Repris de l'ancien élément Arbre ; porté par les
//! familles d'arbres sous forme de maillages dans la boîte du type
//! (`famille::modelee::MaillageFamille`).
use crate::elements::generateur_vegetal::{tronc, Alea, Essence};
use crate::famille::modelee::GeometrieMaillage;
use glam::DVec3;
use std::sync::{Arc, Mutex, OnceLock};

/// Préfixe des références de géométrie d'arbre : `arbre:<essence>:<partie>`.
pub const PREFIXE: &str = "arbre:";

/// Référence de la partie `partie` (`houppier`, `tronc`, `symbole`) d'une essence.
pub fn reference(essence: Essence, partie: &str) -> String {
    let e = match essence { Essence::Feuillu => "feuillu", Essence::Conifere => "conifere", Essence::Fastigie => "fastigie" };
    format!("{PREFIXE}{e}:{partie}")
}

/// Formes générées gardées en mémoire au plus (une par référence et par occurrence).
const MEMOIRE_MAX: usize = 4096;

/// Géométrie (boîte du type) d'une référence `arbre:<essence>:<partie>`, tirée de
/// `graine` (identifiant de l'occurrence : chaque arbre a sa forme ; None : forme type
/// de la famille), calculée une fois puis gardée en mémoire. None : référence inconnue.
pub fn generer(reference: &str, graine: Option<&str>) -> Option<Arc<GeometrieMaillage>> {
    static MEMOIRE: OnceLock<Mutex<std::collections::HashMap<String, Arc<GeometrieMaillage>>>> = OnceLock::new();
    let memoire = MEMOIRE.get_or_init(Default::default);
    let cle = format!("{reference}#{}", graine.unwrap_or(""));
    if let Some(g) = memoire.lock().unwrap_or_else(|e| e.into_inner()).get(&cle) {
        return Some(g.clone());
    }
    let (essence, partie) = reference.strip_prefix(PREFIXE)?.split_once(':')?;
    let essence = match essence { "feuillu" => Essence::Feuillu, "conifere" => Essence::Conifere, "fastigie" => Essence::Fastigie, _ => return None };
    let g = match partie {
        "houppier" | "tronc" => {
            let (t, h) = maillage_boite(essence, graine);
            indexer(if partie == "tronc" { &t } else { &h })
        }
        "symbole" => GeometrieMaillage { traits_plan: symbole_plan(essence), ..Default::default() },
        _ => return None,
    };
    let g = Arc::new(g);
    let mut m = memoire.lock().unwrap_or_else(|e| e.into_inner());
    if m.len() >= MEMOIRE_MAX { m.clear(); }
    m.insert(cle, g.clone());
    Some(g)
}

/// Triangles indexés (sommets confondus fusionnés).
fn indexer(tris: &[[DVec3; 3]]) -> GeometrieMaillage {
    let mut index = std::collections::HashMap::new();
    let mut sommets: Vec<[f32; 3]> = Vec::new();
    let triangles = tris.iter().map(|t| t.map(|p| {
        let s = [p.x as f32, p.y as f32, p.z as f32];
        *index.entry(s.map(f32::to_bits)).or_insert_with(|| { sommets.push(s); sommets.len() as u32 - 1 })
    })).collect();
    GeometrieMaillage { sommets, triangles, traits_plan: Vec::new() }
}

/// Icosphère (une subdivision : 80 faces) déformée : rayons (rx, ry, rz), bosses de ±`relief`.
fn boule(c: DVec3, r: DVec3, relief: f64, alea: &mut Alea) -> Vec<[DVec3; 3]> {
    let t = (1.0 + 5f64.sqrt()) / 2.0;
    let mut v: Vec<DVec3> = [
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ]
    .iter()
    .map(|p| DVec3::from_array(*p).normalize())
    .collect();
    let mut f: Vec<[usize; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..1 {
        let mut milieux = std::collections::HashMap::new();
        let mut milieu = |a: usize, b: usize, v: &mut Vec<DVec3>| {
            *milieux.entry((a.min(b), a.max(b))).or_insert_with(|| {
                v.push(((v[a] + v[b]) * 0.5).normalize());
                v.len() - 1
            })
        };
        let mut g = Vec::with_capacity(f.len() * 4);
        for [a, b, c] in f {
            let (ab, bc, ca) = (milieu(a, b, &mut v), milieu(b, c, &mut v), milieu(c, a, &mut v));
            g.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        f = g;
    }
    let bosses: Vec<f64> = (0..v.len()).map(|_| 1.0 + alea.entre(-relief, relief)).collect();
    let p: Vec<DVec3> = v.iter().zip(&bosses).map(|(d, k)| c + *d * r * *k).collect();
    f.into_iter().map(|[a, b, c]| [p[a], p[b], p[c]]).collect()
}

/// Tronc et houppier (mm) d'un arbre de l'essence aux dimensions par défaut, pied à
/// l'origine ; forme tirée de `graine` (même forme pour la même graine).
fn solides(essence: Essence, graine: &str) -> (Vec<[DVec3; 3]>, Vec<[DVec3; 3]>) {
    let mut alea = Alea::de(graine);
    let (h, d) = essence.dimensions();
    let pied = DVec3::ZERO;
    let rt = (d * 0.035).clamp(60.0, 400.0);
    match essence {
        Essence::Feuillu => {
            let fut = h * 0.35;
            let corps = tronc(pied, fut + (h - fut) * 0.3, rt, rt * 0.6, 8);
            // grappe : un volume central et 6 à 8 autour
            let centre = pied + DVec3::Z * (fut + (h - fut) * 0.5);
            let (rh, rv) = (d * 0.5, (h - fut) * 0.5);
            let mut houppier = boule(centre, DVec3::new(rh * 0.7, rh * 0.7, rv * 0.75), 0.12, &mut alea);
            let n = 6 + (alea.f() * 3.0) as usize;
            for k in 0..n {
                let a = (k as f64 + alea.entre(-0.3, 0.3)) / n as f64 * std::f64::consts::TAU;
                let z = alea.entre(-0.35, 0.45) * rv;
                let ecart = rh * alea.entre(0.35, 0.55);
                let c = centre + DVec3::new(a.cos() * ecart, a.sin() * ecart, z);
                let r = rh * alea.entre(0.38, 0.5);
                houppier.extend(boule(c, DVec3::new(r, r, r * alea.entre(0.8, 1.0)), 0.15, &mut alea));
            }
            (corps, houppier)
        }
        Essence::Conifere => {
            // trois cônes lisses (16 pans), du plus large au plus étroit
            let fut = h * 0.12;
            let corps = tronc(pied, h * 0.6, rt, rt * 0.5, 8);
            let mut houppier = Vec::new();
            let n = 16;
            for (z, r, hc) in [(0.0, 0.5, 0.5), (0.3, 0.38, 0.42), (0.55, 0.26, 0.45)] {
                let base = pied + DVec3::Z * (fut + (h - fut) * z);
                let (rb, hh) = (d * r, (h - fut) * hc);
                let sommet = base + DVec3::Z * hh;
                for i in 0..n {
                    let (a, b) = (i as f64 / n as f64 * std::f64::consts::TAU, (i + 1) as f64 / n as f64 * std::f64::consts::TAU);
                    let p = |ang: f64| base + DVec3::new(ang.cos() * rb, ang.sin() * rb, 0.0);
                    houppier.push([p(a), p(b), sommet]);
                    houppier.push([p(b), p(a), base]);
                }
            }
            (corps, houppier)
        }
        Essence::Fastigie => {
            let fut = h * 0.1;
            let corps = tronc(pied, h * 0.5, rt, rt * 0.6, 8);
            let centre = pied + DVec3::Z * (fut + (h - fut) * 0.5);
            let (rh, rv) = (d * 0.5, (h - fut) * 0.5);
            let mut houppier = boule(centre, DVec3::new(rh, rh, rv), 0.1, &mut alea);
            for _ in 0..4 {
                let z = (alea.f() - 0.5) * rv * 1.2;
                let a = alea.entre(0.0, std::f64::consts::TAU);
                let c = centre + DVec3::new(a.cos(), a.sin(), 0.0) * rh * 0.35 + DVec3::Z * z;
                let r = rh * alea.entre(0.55, 0.7);
                houppier.extend(boule(c, DVec3::new(r, r, r * 1.6), 0.12, &mut alea));
            }
            (corps, houppier)
        }
    }
}

/// Tronc et houppier ramenés à la boîte du type : x, y de −0,5 à 0,5 (encombrement du
/// houppier), z de 0 à 1 (hauteur totale) ; forme tirée de `graine` (None : forme type).
pub fn maillage_boite(essence: Essence, graine: Option<&str>) -> (Vec<[DVec3; 3]>, Vec<[DVec3; 3]>) {
    let graine = graine.map_or_else(|| format!("famille:{essence:?}"), str::to_string);
    let (corps, houppier) = solides(essence, &graine);
    let (min, max) = corps.iter().chain(&houppier).flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
    // centre horizontal sur l'axe du tronc (origine), échelle par le plus grand côté
    let rayon = min.x.abs().max(max.x).max(min.y.abs()).max(max.y).max(1.0);
    let k = DVec3::new(0.5 / rayon, 0.5 / rayon, 1.0 / (max.z - min.z).max(1.0));
    let n = |t: [DVec3; 3]| t.map(|p| (p - DVec3::Z * min.z) * k);
    (corps.into_iter().map(n).collect(), houppier.into_iter().map(n).collect())
}

/// Symbole de plan dans la boîte du type (rayon 0,5) : contour du houppier (festonné à
/// 8 lobes pour un feuillu, 16 pointes pour un conifère, cercle pour un fastigié), tronc
/// (cercle de rayon 0,035) et croix au pied ; polylignes fermées répétant leur premier
/// point.
pub fn symbole_plan(essence: Essence) -> Vec<Vec<[f32; 2]>> {
    let r = 0.5;
    let n = 48;
    let rayon = |k: usize| {
        let t = k as f64 / n as f64;
        match essence {
            Essence::Feuillu => r * (0.9 + 0.1 * (t * 8.0 * std::f64::consts::TAU).cos().abs()),
            Essence::Conifere => r * if k.is_multiple_of(3) { 1.0 } else { 0.86 },
            Essence::Fastigie => r,
        }
    };
    let point = |k: usize, rr: f64| {
        let a = k as f64 / n as f64 * std::f64::consts::TAU;
        [(a.cos() * rr) as f32, (a.sin() * rr) as f32]
    };
    let contour = (0..=n).map(|k| point(k, rayon(k % n))).collect();
    let tronc = (0..=16).map(|k| point(k * 3, 0.035)).collect();
    let c = (r * 0.12) as f32;
    vec![contour, tronc, vec![[-c, 0.0], [c, 0.0]], vec![[0.0, -c], [0.0, c]]]
}
