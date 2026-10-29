//! Rendu « dessin » des plans et élévations, à la manière d'un logiciel de DAO (CADCraft) :
//! fond blanc, faces blanches qui masquent ce qu'elles cachent (tampon de profondeur),
//! arêtes vives des maillages en trait fin, et coupe par un plan : poché (plein, hachuré ou
//! vide) et contour en trait fort. Épaisseurs et pas des hachures en mm du papier,
//! multipliés par l'échelle du dessin.

use crate::rvt::vues::{GrilleMotif, Motif, hachures_plan};
use crate::scene::{Scene, Vertex};
use crate::trace2d::{Forme, Primitive, Role, Style};
use glam::{DVec3, Vec3, Vec4};
use rayon::prelude::*;
use std::collections::HashMap;

/// Écart angulaire au-delà duquel deux faces voisines sont séparées par une arête vive
/// (cosinus de 30°) ; en deçà, facettes d'une même surface courbe.
const COS_VIF: f64 = 0.866;
/// Pas de quantification des positions (m).
const PAS: f64 = 1e-4;
/// Pas de quantification des directions.
const PAS_DIR: f64 = 1e-4;

/// Arêtes d'un maillage pour le dessin.
#[derive(Default)]
pub struct Aretes {
    /// Arêtes vives : toujours tracées (sauf parties cachées).
    pub vives: Vec<[DVec3; 2]>,
    /// Arêtes entre deux facettes d'une surface courbe, avec leurs normales : tracées
    /// seulement sur la silhouette (une facette vers l'observateur, l'autre non).
    pub douces: Vec<([DVec3; 2], DVec3, DVec3)>,
}

/// Arêtes d'un maillage (soupe de triangles, repère du fichier) : bords des régions planes
/// ou courbes douces, sans les diagonales de triangulation ni les découpes des faces
/// coplanaires (jonctions en T de la CSG comprises), et arêtes douces des surfaces
/// courbes facettisées (silhouettes).
///
/// Chaque arête de triangle est rangée sur sa droite porteuse, avec la normale du triangle
/// et son sens de parcours. Sur chaque intervalle élémentaire de la droite, les arêtes
/// sont groupées par orientation de face (normales à moins de 30°) : une face continue
/// parcourt l'intervalle une fois dans chaque sens (somme nulle) ; une somme non nulle
/// marque le bord d'une face, donc une arête vive ou un bord libre. Une face continue dont
/// les deux facettes ne sont pas coplanaires donne une arête douce.
pub fn aretes(tris: &[[DVec3; 3]]) -> Aretes {
    let (vives, douces) = aretes_proprietaires(tris.iter().map(|t| (*t, 0)), false);
    Aretes { vives: vives.into_iter().map(|(s, _)| s).collect(), douces: douces.into_iter().map(|(d, _)| d).collect() }
}

/// Arêtes d'éléments d'un même matériau pris ensemble, chacune rendue à l'élément
/// (indice de `tris`) qui la porte : deux faces collées l'une contre l'autre (normales
/// opposées, parcours opposés : contact entre deux murs) s'annulent, et les faces
/// coplanaires de part et d'autre d'une jonction se raccordent sans arête. Les angles
/// réels (faces non coplanaires) restent.
pub fn aretes_jointes(tris: &[([DVec3; 3], usize)]) -> (Vec<([DVec3; 2], usize)>, Vec<(([DVec3; 2], DVec3, DVec3), usize)>) {
    aretes_proprietaires(tris.iter().copied(), true)
}

#[allow(clippy::type_complexity)]
fn aretes_proprietaires(tris: impl Iterator<Item = ([DVec3; 3], usize)>, annuler_opposees: bool) -> (Vec<([DVec3; 2], usize)>, Vec<(([DVec3; 2], DVec3, DVec3), usize)>) {
    type Cle = ([i64; 3], [i64; 3]);
    /// Direction, point et arêtes (t0, t1, normale, sens, élément) d'une droite.
    type Droite = (DVec3, DVec3, Vec<(f64, f64, DVec3, i32, usize)>);
    let q = |x: f64, pas: f64| (x / pas).round() as i64;
    let mut droites: HashMap<Cle, Droite> = HashMap::new();
    for (t, proprietaire) in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        if n.length_squared() < 1e-20 {
            continue;
        }
        let n = n.normalize();
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let l = b - a;
            if l.length_squared() < PAS * PAS {
                continue;
            }
            let mut u = l.normalize();
            // direction canonique : composante de plus grande valeur absolue positive
            let m = if u.x.abs() >= u.y.abs() && u.x.abs() >= u.z.abs() { u.x } else if u.y.abs() >= u.z.abs() { u.y } else { u.z };
            let sens = if m < 0.0 {
                u = -u;
                -1
            } else {
                1
            };
            let p0 = a - u * a.dot(u);
            let cle = ([q(u.x, PAS_DIR), q(u.y, PAS_DIR), q(u.z, PAS_DIR)], [q(p0.x, PAS), q(p0.y, PAS), q(p0.z, PAS)]);
            let e = droites.entry(cle).or_insert((u, p0, Vec::new()));
            let (ta, tb) = (a.dot(e.0), b.dot(e.0));
            e.2.push((ta.min(tb), ta.max(tb), n, sens, proprietaire));
        }
    }
    let mut out = Vec::new();
    let mut douces = Vec::new();
    for (u, p0, segs) in droites.into_values() {
        // bornes des intervalles élémentaires, rapprochées au pas
        let mut bornes: Vec<f64> = segs.iter().flat_map(|s| [s.0, s.1]).collect();
        bornes.sort_by(f64::total_cmp);
        bornes.dedup_by(|a, b| (*a - *b).abs() < PAS);
        let mut courant: Option<(f64, f64, usize)> = None;
        for w in bornes.windows(2) {
            let (a, b) = (w[0], w[1]);
            let actifs = segs.iter().filter(|s| s.0 <= a + PAS && s.1 >= b - PAS);
            // groupes d'orientation : (normale de référence, somme des sens, normale la
            // plus écartée de la référence, élément)
            let mut groupes: Vec<(DVec3, i32, DVec3, usize)> = Vec::new();
            for s in actifs {
                match groupes.iter_mut().find(|g| g.0.dot(s.2) > COS_VIF) {
                    Some(g) => {
                        g.1 += s.3;
                        if s.2.dot(g.0) < g.2.dot(g.0) {
                            g.2 = s.2;
                        }
                    }
                    None => groupes.push((s.2, s.3, s.2, s.4)),
                }
            }
            if annuler_opposees {
                // faces collées : normales opposées, sommes opposées
                let mut k = 0;
                while k < groupes.len() {
                    match (k + 1..groupes.len()).find(|&l| groupes[k].0.dot(groupes[l].0) < -COS_VIF && groupes[k].1 + groupes[l].1 == 0 && groupes[k].1 != 0) {
                        Some(l) => {
                            groupes.remove(l);
                            groupes.remove(k);
                        }
                        None => k += 1,
                    }
                }
            }
            let porteur = groupes.iter().find(|g| g.1 != 0).map(|g| g.3);
            let vive = porteur.is_some();
            // une seule face, courbe à cet endroit (plus d'un quart de degré entre les
            // facettes ; en deçà, coplanaires aux arrondis près)
            if let [g] = groupes[..]
                && !vive
                && g.0.dot(g.2) < 0.99999
                && b - a > PAS
            {
                douces.push((([p0 + u * a, p0 + u * b], g.0, g.2), g.3));
            }
            match (&mut courant, porteur) {
                (Some(c), Some(e)) if (c.1 - a).abs() < PAS && c.2 == e => c.1 = b,
                (_, Some(e)) => {
                    if let Some(c) = courant.take() {
                        out.push(([p0 + u * c.0, p0 + u * c.1], c.2));
                    }
                    courant = Some((a, b, e));
                }
                (_, None) => {
                    if let Some(c) = courant.take() {
                        out.push(([p0 + u * c.0, p0 + u * c.1], c.2));
                    }
                }
            }
        }
        if let Some(c) = courant {
            out.push(([p0 + u * c.0, p0 + u * c.1], c.2));
        }
    }
    out.retain(|s| s.0[0].distance_squared(s.0[1]) > PAS * PAS);
    (out, douces)
}

/// Remplissage des parties coupées.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Poche {
    /// Gris clair hachuré à 45°.
    Hachure,
    /// Gris foncé uni.
    Plein,
    /// Blanc : contour seul.
    Vide,
    /// Couleur du matériau (style Réaliste).
    Materiau,
}

impl Poche {
    pub fn nom(self) -> &'static str {
        match self {
            Poche::Hachure => "hachure",
            Poche::Plein => "plein",
            Poche::Vide => "vide",
            Poche::Materiau => "materiau",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Poche::Hachure => "Hachuré",
            Poche::Plein => "Plein",
            Poche::Vide => "Vide",
            Poche::Materiau => "Matériau",
        }
    }

    pub fn depuis(nom: &str) -> Option<Poche> {
        [Poche::Hachure, Poche::Plein, Poche::Vide].into_iter().find(|p| p.nom() == nom)
    }
}

/// Contour des parties coupées, mm du papier.
pub const TRAIT_FORT_MM: f64 = 0.35;
/// Pas des hachures du poché, mm du papier.
pub const PAS_HACHURE_MM: f64 = 1.5;

/// Couleurs des traits du dessin (arêtes vives et contours).
pub const NOIR: [u8; 4] = [20, 20, 20, 255];

/// Coupe des objets visibles par le plan `plan` (n, d ; repère de la scène, n·p + d ≥ 0
/// gardé) : poché, hachures et contours en trait fort, en primitives 2D dans le plan
/// (repère de la scène). `echelle` : dénominateur de l'échelle du dessin (pas des
/// hachures).
pub fn coupe(scene: &Scene, visible: &[bool], plan: Vec4, echelle: f64, poche: Poche) -> Vec<Primitive> {
    let n = plan.truncate().as_dvec3();
    let d = plan.w as f64 / n.length().max(1e-12);
    let n = n.normalize_or_zero();
    if n == DVec3::ZERO {
        return Vec::new();
    }
    // repère du plan : origine, deux axes ; x suit l'axe X du modèle quand c'est possible
    let o = -n * d;
    let ref_x = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let e1 = (ref_x - n * ref_x.dot(n)).normalize();
    let e2 = e1.cross(n);
    let motif = Motif {
        nom: "Poché".into(),
        modele: true,
        grilles: vec![GrilleMotif { angle: std::f64::consts::FRAC_PI_4, origine: [0.0, 0.0], glissement: 0.0, decalage: PAS_HACHURE_MM * echelle / 1000.0, tirets: Vec::new() }],
    };
    let (fond, hachure) = match poche {
        Poche::Hachure => ([226, 226, 226, 255], Some([70, 70, 70, 255])),
        Poche::Plein => ([75, 75, 75, 255], None),
        Poche::Vide | Poche::Materiau => ([255, 255, 255, 255], None),
    };
    let prims: Vec<Primitive> = (0..scene.objects.len())
        .into_par_iter()
        .flat_map_iter(|i| {
            let ob = &scene.objects[i];
            let mut prims = Vec::new();
            if !visible[i] || &*ob.class == "IFCANNOTATION" {
                return prims;
            }
            // boîte entièrement d'un côté du plan : pas de coupe
            let (mut smin, mut smax) = (f64::MAX, f64::MIN);
            for c in 0..8 {
                let p = DVec3::new([ob.min.x, ob.max.x][c & 1] as f64, [ob.min.y, ob.max.y][(c >> 1) & 1] as f64, [ob.min.z, ob.max.z][c >> 2] as f64);
                let s = n.dot(p) + d;
                smin = smin.min(s);
                smax = smax.max(s);
            }
            if smin > 0.0 || smax < 0.0 {
                return prims;
            }
            let mut segs = Vec::new();
            for r in [ob.opaque, ob.transparent] {
                for t in scene.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3) {
                    let p = [0, 1, 2].map(|k| Vec3::from(t[k].pos).as_dvec3());
                    if let Some(s) = section(p, n, d) {
                        segs.push(s.map(|q| [(q - o).dot(e1), (q - o).dot(e2)]));
                    }
                }
            }
            if segs.is_empty() {
                return prims;
            }
            let (boucles, ouvertes) = chainer(&segs);
            let en_3d = |p: [f64; 2]| o + e1 * p[0] + e2 * p[1];
            let primitive = |forme, couleur, epaisseur, role| Primitive { forme, style: Style { couleur, epaisseur }, role, element: Some(ob.id) };
            if !boucles.is_empty() {
                if let Some(couches) = scene.couches_coupe.get(&ob.id) {
                    let surface = primitive(Forme::Surface(boucles.iter().map(|b| b.iter().map(|p| en_3d(*p)).collect()).collect()), fond, 0.0, Role::Poche);
                    let triangles = surface.mailler(n, echelle).triangles;
                    let mut detail = Vec::new();
                    for couche in couches {
                        let mut tris = Vec::new();
                        for t in &triangles {
                            let mut polygone = t.to_vec();
                            for plan in &couche.plans { polygone = decouper(&polygone, plan.truncate().as_dvec3(), plan.w as f64); }
                            for k in 1..polygone.len().saturating_sub(1) {
                                let t = [polygone[0],polygone[k],polygone[k+1]];
                                if (t[1]-t[0]).cross(t[2]-t[0]).length_squared() > 1e-24 { tris.push(t); }
                            }
                        }
                        if tris.is_empty() { continue; }
                        let uni = poche == Poche::Hachure && couche.motif.as_ref().is_some_and(|m| m.grilles.is_empty());
                        // arrière-plan : uni (sa couleur), motif (sur blanc) ou aucun (blanc)
                        let fond_uni = couche.motif_fond.as_ref().is_some_and(|m| m.grilles.is_empty());
                        let couleur = match poche {
                            Poche::Materiau => couche.couleur,
                            _ if uni => couche.couleur_motif,
                            Poche::Hachure if !couche.hachure_defaut => if fond_uni { couche.fond } else { [255,255,255,255] },
                            _ => fond,
                        };
                        detail.push(primitive(Forme::Triangles(tris.clone()),couleur,0.0,Role::Poche));
                        if let (Poche::Hachure, Some(m)) = (poche, couche.motif_fond.as_ref().filter(|m| !m.grilles.is_empty())) {
                            let t2: Vec<_> = tris.iter().map(|t| t.map(|p| [(p-o).dot(e1),(p-o).dot(e2)])).collect();
                            let h = hachures_plan(&t2,m,echelle / 1000.0,20_000).into_iter().map(|s| s.map(en_3d)).collect();
                            detail.push(primitive(Forme::Segments(h),couche.fond,0.0,Role::Hachure));
                        }
                        if poche == Poche::Hachure && couche.isolation {
                            let t2: Vec<_> = tris.iter().map(|t| t.map(|p| [(p-o).dot(e1),(p-o).dot(e2)])).collect();
                            let h = ligne_isolation(&t2).into_iter().map(|s| s.map(en_3d)).collect();
                            detail.push(primitive(Forme::Segments(h),couche.couleur_motif,0.0,Role::Hachure));
                        }
                        if poche == Poche::Hachure && !uni && !couche.isolation {
                            let (motif, facteur) = match &couche.motif {
                                Some(m) => (m.as_ref(), if m.modele { 0.001 } else { echelle / 1000.0 }),
                                None if couche.hachure_defaut => (&motif, 1.0),
                                _ => { let contours = aretes(&tris).vives; detail.push(primitive(Forme::Segments(contours),NOIR,couche.trait_mm,Role::Coupe)); continue; }
                            };
                            let t2: Vec<_> = tris.iter().map(|t| t.map(|p| [(p-o).dot(e1),(p-o).dot(e2)])).collect();
                            let h = hachures_plan(&t2,motif,facteur,20_000).into_iter().map(|s| s.map(en_3d)).collect();
                            detail.push(primitive(Forme::Segments(h),couche.couleur_motif,0.0,Role::Hachure));
                        }
                        detail.push(primitive(Forme::Segments(aretes(&tris).vives),NOIR,couche.trait_mm,Role::Coupe));
                    }
                    if !detail.is_empty() { return detail; }
                }
            }
            if !boucles.is_empty() {
                // style Réaliste : couleur de l'élément (rendu, sinon affichage)
                let fond = match poche {
                    Poche::Materiau => {
                        let r = if ob.opaque.count > 0 { ob.opaque } else { ob.transparent };
                        let k = r.start as usize;
                        scene.couleurs_rendu.get(k).filter(|c| c[3] > 0).copied().or_else(|| scene.vertices.get(k).filter(|_| r.count > 0).map(|v| v.color)).map_or(fond, |c| [c[0], c[1], c[2], 255])
                    }
                    _ => fond,
                };
                prims.push(primitive(Forme::Surface(boucles.iter().map(|b| b.iter().map(|p| en_3d(*p)).collect()).collect()), fond, 0.0, Role::Poche));
            }
            if let Some(c) = hachure {
                let contours: Vec<Vec<[f32; 2]>> = boucles.iter().map(|b| b.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()).collect();
                let t2: Vec<[[f64; 2]; 3]> = crate::texte::trianguler(contours).iter().map(|t| t.map(|p| [p[0] as f64, p[1] as f64])).collect();
                let s: Vec<[DVec3; 2]> = hachures_plan(&t2, &motif, 1.0, 20_000).into_iter().map(|s| s.map(en_3d)).collect();
                if !s.is_empty() {
                    prims.push(primitive(Forme::Segments(s), c, 0.0, Role::Hachure));
                }
            }
            for (pts, fermee) in boucles.iter().map(|b| (b, true)).chain(ouvertes.iter().map(|b| (b, false))) {
                prims.push(primitive(Forme::Polyligne { points: pts.iter().map(|p| en_3d(*p)).collect(), fermee }, NOIR, TRAIT_FORT_MM, Role::Coupe));
            }
            prims
        })
        .collect();
    fusionner_materiaux(scene, prims)
}

/// Ligne d'isolant en boucles (laine, comme le composant « isolant » de Revit) au milieu
/// d'une région de coupe (triangles 2D) : axe le long de sa plus grande dimension,
/// boucles de la hauteur de son épaisseur, segments gardés dans la région.
pub fn ligne_isolation(tris: &[[[f64; 2]; 3]]) -> Vec<[[f64; 2]; 2]> {
    use glam::DVec2;
    let pts: Vec<DVec2> = tris.iter().flatten().map(|p| DVec2::from_array(*p)).collect();
    if pts.len() < 3 {
        return Vec::new();
    }
    let c = pts.iter().copied().sum::<DVec2>() / pts.len() as f64;
    // direction principale (covariance des sommets)
    let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
    for p in &pts {
        let d = *p - c;
        xx += d.x * d.x;
        xy += d.x * d.y;
        yy += d.y * d.y;
    }
    let a = 0.5 * (2.0 * xy).atan2(xx - yy);
    let u = DVec2::new(a.cos(), a.sin());
    let v = u.perp();
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in &pts {
        let d = *p - c;
        u0 = u0.min(d.dot(u));
        u1 = u1.max(d.dot(u));
        v0 = v0.min(d.dot(v));
        v1 = v1.max(d.dot(v));
    }
    let e = v1 - v0;
    if !(e > 1e-6) || u1 - u0 < e {
        return Vec::new();
    }
    let milieu = c + v * ((v0 + v1) / 2.0);
    // cycloïde allongée : boucles de hauteur 0,9 × épaisseur, une boucle par 0,6 × épaisseur
    let (d, r) = (0.45 * e, 0.6 * e / std::f64::consts::TAU);
    let dedans = |p: DVec2| tris.iter().any(|t| {
        let [a, b, c] = t.map(DVec2::from_array);
        let s = |x: DVec2, y: DVec2| (y - x).perp_dot(p - x);
        let (s1, s2, s3) = (s(a, b), s(b, c), s(c, a));
        (s1 >= -1e-12 && s2 >= -1e-12 && s3 >= -1e-12) || (s1 <= 1e-12 && s2 <= 1e-12 && s3 <= 1e-12)
    });
    let point = |th: f64| milieu + u * (u0 + d + r * th - d * th.sin()) + v * (-d * th.cos());
    let fin = ((u1 - u0 - 2.0 * d) / r).max(0.0);
    let pas = 0.2;
    let mut out = Vec::new();
    let mut th = 0.0;
    while th < fin {
        let (p, q) = (point(th), point((th + pas).min(fin)));
        if dedans((p + q) * 0.5) {
            out.push([p.to_array(), q.to_array()]);
        }
        th += pas;
    }
    out
}

/// Nettoyage des jonctions comme Revit : un trait de coupe commun à deux éléments du
/// même matériau (deux couches identiques raccordées en onglet) est retiré, les deux
/// régions se lisent d'un seul tenant.
fn fusionner_materiaux(scene: &Scene, mut prims: Vec<Primitive>) -> Vec<Primitive> {
    use std::collections::HashMap;
    // matériau d'un élément : sa bande de coupe unique (couches des murs natifs)
    let materiau = |id: Option<crate::step::Id>| -> Option<([u8; 4], String)> {
        let c = scene.couches_coupe.get(&id?)?;
        let [b] = c.as_slice() else { return None };
        Some((b.couleur, b.motif.as_ref().map_or_else(String::new, |m| m.nom.clone())))
    };
    // traits de coupe rangés par droite porteuse : (primitive, rang, t0, t1, élément)
    type Trait = (usize, usize, f64, f64, Option<crate::step::Id>);
    let q = |x: f64| (x * 1e4).round() as i64;
    let mut droites: HashMap<([i64; 3], [i64; 3]), (DVec3, Vec<Trait>)> = HashMap::new();
    for (k, p) in prims.iter().enumerate() {
        if let (Role::Coupe, Forme::Segments(segs)) = (p.role, &p.forme) {
            if materiau(p.element).is_none() {
                continue;
            }
            for (i, s) in segs.iter().enumerate() {
                let d = s[1] - s[0];
                if d.length() < 1e-9 {
                    continue;
                }
                let mut u = d.normalize();
                let m = if u.x.abs() >= u.y.abs() && u.x.abs() >= u.z.abs() { u.x } else if u.y.abs() >= u.z.abs() { u.y } else { u.z };
                if m < 0.0 {
                    u = -u;
                }
                let p0 = s[0] - u * s[0].dot(u);
                let cle = ([q(u.x), q(u.y), q(u.z)], [q(p0.x), q(p0.y), q(p0.z)]);
                let (ta, tb) = (s[0].dot(u), s[1].dot(u));
                droites.entry(cle).or_insert((u, Vec::new())).1.push((k, i, ta.min(tb), ta.max(tb), p.element));
            }
        }
    }
    // partie de chaque trait recouverte par un trait d'un autre élément du même matériau :
    // retirée des deux (un trait court posé sur un trait long compris)
    let mut remplacer: HashMap<(usize, usize), Vec<[DVec3; 2]>> = HashMap::new();
    for (u, traits) in droites.values() {
        for &(k, i, t0, t1, e) in traits {
            let mut couverts: Vec<(f64, f64)> = traits
                .iter()
                .filter(|o| o.4 != e && materiau(o.4) == materiau(e) && o.3 > t0 + 1e-6 && o.2 < t1 - 1e-6)
                .map(|o| (o.2.max(t0), o.3.min(t1)))
                .collect();
            if couverts.is_empty() {
                continue;
            }
            couverts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let Forme::Segments(segs) = &prims[k].forme else { continue };
            let s = segs[i];
            // point de la droite à l'abscisse t
            let base = s[0] - *u * s[0].dot(*u);
            let mut restes = Vec::new();
            let mut debut = t0;
            for (a, b) in couverts {
                if a > debut + 1e-6 {
                    restes.push([base + *u * debut, base + *u * a]);
                }
                debut = debut.max(b);
            }
            if t1 > debut + 1e-6 {
                restes.push([base + *u * debut, base + *u * t1]);
            }
            remplacer.insert((k, i), restes);
        }
    }
    if remplacer.is_empty() {
        return prims;
    }
    for (k, p) in prims.iter_mut().enumerate() {
        if let Forme::Segments(segs) = &mut p.forme {
            let anciens = std::mem::take(segs);
            for (i, s) in anciens.into_iter().enumerate() {
                match remplacer.remove(&(k, i)) {
                    Some(r) => segs.extend(r),
                    None => segs.push(s),
                }
            }
        }
    }
    prims
}

/// Sommets GPU des primitives d'une coupe de normale `normale` : triangles (poché,
/// contours) et traits (hachures), décalés vers l'observateur (côté retiré du plan) pour
/// passer devant les faces coupées ; marqués comme annotations, jamais découpés.
/// `fins` : affichage traits fins, tous les traits à un pixel.
pub fn sommets(scene: &Scene, prims: &[Primitive], normale: DVec3, echelle: f64, fins: bool) -> (Vec<Vertex>, Vec<Vertex>) {
    let largeur = TRAIT_FORT_MM * echelle / 1000.0;
    let (mut tris, mut traits) = (Vec::new(), Vec::new());
    for p in prims {
        let obj = p.element.and_then(|e| scene.object_of.get(&e).copied()).unwrap_or(u32::MAX);
        let decal = -normale * if p.role == Role::Coupe { (0.4 * largeur).min(0.004) } else { (0.2 * largeur).min(0.002) };
        let sommet = |q: DVec3| Vertex { pos: (q + decal).as_vec3().to_array(), normal: [0, 0, 0, 127], color: p.style.couleur, obj };
        let m = maillage(p, normale, echelle, fins);
        tris.extend(m.triangles.iter().flatten().map(|q| sommet(*q)));
        traits.extend(m.traits.iter().flatten().map(|q| sommet(*q)));
        // liseré d'un pixel des traits épais (contours, annotations) : jamais plus fins
        // qu'un pixel, donc stables au zoom
        if p.role != Role::Poche && !m.triangles.is_empty() {
            traits.extend(aretes(&m.triangles).vives.iter().flatten().map(|q| sommet(*q)));
        }
    }
    (tris, traits)
}

/// Maillage d'une primitive ; traits fins : ses traits épais ramenés à un pixel.
fn maillage(p: &Primitive, normale: DVec3, echelle: f64, fins: bool) -> crate::trace2d::Maillage {
    if fins && p.style.epaisseur > 0.0 {
        let mut fin = p.clone();
        fin.style.epaisseur = 0.0;
        return fin.mailler(normale, echelle);
    }
    p.mailler(normale, echelle)
}

/// Ferme les six faces d'une section box. La triangulation précède le découpage
/// par les autres faces : les trous des solides restent donc des trous.
/// Sans poché demandé, le fond reprend la couleur de l'élément coupé.
pub fn sommets_boite(scene: &Scene, visible: &[bool], a: Vec3, b: Vec3, echelle: f64, poche: Option<Poche>, rendu: bool, fins: bool) -> (Vec<Vertex>, Vec<Vertex>) {
    let (mut triangles, mut traits) = (Vec::new(), Vec::new());
    if !a.is_finite() || !b.is_finite() || a.cmpge(b).any() {
        return (triangles, traits);
    }
    let plans = [Vec3::X.extend(-a.x), Vec3::NEG_X.extend(b.x), Vec3::Y.extend(-a.y), Vec3::NEG_Y.extend(b.y), Vec3::Z.extend(-a.z), Vec3::NEG_Z.extend(b.z)];
    for plan in plans {
        let normale = plan.truncate().as_dvec3();
        for mut p in coupe(scene, visible, plan, echelle, poche.unwrap_or(Poche::Vide)) {
            let objet = p.element.and_then(|id| scene.object_of.get(&id).copied()).unwrap_or(u32::MAX);
            if poche.is_none() && p.role == Role::Poche {
                let couleur_couche = match &p.forme {
                    Forme::Triangles(t) => t.first().and_then(|t| {
                        let centre = (t[0]+t[1]+t[2])/3.0;
                        scene.couches_coupe.get(&p.element?).and_then(|couches| couches.iter().find(|c| c.plans.iter().all(|p| p.truncate().as_dvec3().dot(centre)+p.w as f64 >= -1e-7))).map(|c| c.couleur)
                    }),
                    _ => None,
                };
                if let Some(couleur) = couleur_couche { p.style.couleur = couleur; }
                else {
                if let Some(ob) = scene.objects.get(objet as usize) {
                    let r = if ob.opaque.count > 0 { ob.opaque } else { ob.transparent };
                    if let Some(v) = scene.vertices.get(r.start as usize).filter(|_| r.count > 0) {
                        let couleur = scene.couleurs_rendu.get(r.start as usize).filter(|c| rendu && c[3] > 0).copied().unwrap_or(v.color);
                        p.style.couleur = [couleur[0], couleur[1], couleur[2], 255];
                    }
                }
                }
            }
            let sommet = |q: DVec3| Vertex { pos: q.as_vec3().to_array(), normal: [0, 0, 0, 127], color: p.style.couleur, obj: objet };
            let maillage = maillage(&p, normale, echelle, fins);
            for t in maillage.triangles {
                let mut polygone = t.to_vec();
                for limite in plans {
                    polygone = decouper(&polygone, limite.truncate().as_dvec3(), limite.w as f64);
                    if polygone.len() < 3 { break; }
                }
                for k in 1..polygone.len().saturating_sub(1) {
                    triangles.extend([polygone[0], polygone[k], polygone[k + 1]].map(sommet));
                }
            }
            for [mut debut, mut fin] in maillage.traits {
                let mut garde = true;
                for limite in plans {
                    let n = limite.truncate().as_dvec3();
                    let (s, t) = (n.dot(debut) + limite.w as f64, n.dot(fin) + limite.w as f64);
                    if s < 0.0 && t < 0.0 { garde = false; break; }
                    if (s < 0.0) != (t < 0.0) {
                        let q = debut + (fin - debut) * (s / (s - t));
                        if s < 0.0 { debut = q; } else { fin = q; }
                    }
                }
                if garde { traits.extend([sommet(debut), sommet(fin)]); }
            }
        }
    }
    (triangles, traits)
}

/// Arêtes vues sans leurs parties cachées, pour un export vectoriel : vue orthographique
/// d'axes `droite` et `haut`, regardant vers −`vers_oeil` (vecteurs unitaires), parties
/// retirées par la coupe (n·p + d < 0) exclues. Les parties cachées sont éliminées par un
/// tampon de profondeur calculé ici (`resolution` pixels sur le grand côté) : chaque arête
/// est échantillonnée tous les demi-pixels et gardée là où aucune face n'est devant elle.
/// Arêtes vives et silhouettes des surfaces courbes, une primitive par objet.
pub fn vu(scene: &Scene, visible: &[bool], droite: DVec3, haut: DVec3, vers_oeil: DVec3, coupe: Option<Vec4>, resolution: usize) -> Vec<Primitive> {
    let plan = coupe.map(|c| (c.truncate().as_dvec3(), c.w as f64));
    let proj = |p: DVec3| DVec3::new(p.dot(droite), p.dot(haut), p.dot(vers_oeil));
    let objets: Vec<usize> = (0..scene.objects.len()).filter(|&i| visible[i] && &*scene.objects[i].class != "IFCANNOTATION").collect();
    let pos = |v: &Vertex| Vec3::from(v.pos).as_dvec3();
    // triangles projetés, découpés par la coupe
    let mut tris: Vec<[DVec3; 3]> = Vec::new();
    for &i in &objets {
        let ob = &scene.objects[i];
        for r in [ob.opaque, ob.transparent] {
            for t in scene.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3) {
                let p = [pos(&t[0]), pos(&t[1]), pos(&t[2])];
                let poly = match plan {
                    Some((n, d)) => decouper(&p, n, d),
                    None => p.to_vec(),
                };
                for k in 1..poly.len().saturating_sub(1) {
                    tris.push([proj(poly[0]), proj(poly[k]), proj(poly[k + 1])]);
                }
            }
        }
    }
    // arêtes (vives, silhouettes) projetées, découpées par la coupe, par objet
    let mut aretes: Vec<(usize, Vec<[DVec3; 2]>)> = Vec::new();
    for &i in &objets {
        let ob = &scene.objects[i];
        let mut segs = Vec::new();
        for v in scene.vertices[ob.aretes.start as usize..(ob.aretes.start + ob.aretes.count) as usize].chunks_exact(2) {
            if v[0].normal[3] < 0 {
                // arête douce : tracée sur la silhouette seulement (voir render.rs)
                let n1 = DVec3::new(v[0].normal[0] as f64, v[0].normal[1] as f64, v[0].normal[2] as f64) / 127.0;
                let n2 = DVec3::new(v[0].color[0] as f64, v[0].color[1] as f64, v[0].color[2] as f64) / 255.0 * 2.0 - DVec3::ONE;
                if n1.dot(vers_oeil) * n2.dot(vers_oeil) > 0.0 {
                    continue;
                }
            }
            let (mut a, mut b) = (pos(&v[0]), pos(&v[1]));
            if let Some((n, d)) = plan {
                let (sa, sb) = (n.dot(a) + d, n.dot(b) + d);
                if sa < 0.0 && sb < 0.0 {
                    continue;
                }
                if sa < 0.0 {
                    a = a + (b - a) * (sa / (sa - sb));
                } else if sb < 0.0 {
                    b = a + (b - a) * (sa / (sa - sb));
                }
            }
            segs.push([a, b]);
        }
        if !segs.is_empty() {
            aretes.push((i, segs));
        }
    }
    if aretes.is_empty() {
        return Vec::new();
    }
    // tampon de profondeur (plus grande profondeur vers l'œil = plus proche)
    let (mut mn, mut mx) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for p in tris.iter().flatten().copied().chain(aretes.iter().flat_map(|a| a.1.iter().flatten().map(|p| proj(*p)))) {
        mn = mn.min(p);
        mx = mx.max(p);
    }
    let taille = (mx.x - mn.x).max(mx.y - mn.y).max(1e-6);
    let px = taille / resolution.max(16) as f64;
    let (nx, ny) = (((mx.x - mn.x) / px).ceil() as usize + 2, ((mx.y - mn.y) / px).ceil() as usize + 2);
    let mut z = vec![f64::MIN; nx * ny];
    // coordonnées continues : le pixel (i, j) couvre [i, i+1[ × [j, j+1[, centre en +0,5
    let grille = |p: DVec3| (((p.x - mn.x) / px) + 0.5, ((p.y - mn.y) / px) + 0.5);
    for t in &tris {
        let [a, b, c] = t.map(|p| {
            let (x, y) = grille(p);
            DVec3::new(x, y, p.z)
        });
        let aire = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        if aire.abs() < 1e-12 {
            continue;
        }
        let (i0, i1) = (a.x.min(b.x).min(c.x).floor().max(0.0) as usize, (a.x.max(b.x).max(c.x).ceil() as usize).min(nx - 1));
        let (j0, j1) = (a.y.min(b.y).min(c.y).floor().max(0.0) as usize, (a.y.max(b.y).max(c.y).ceil() as usize).min(ny - 1));
        for j in j0..=j1 {
            let y = j as f64 + 0.5;
            for i in i0..=i1 {
                let x = i as f64 + 0.5;
                let w0 = ((b.x - x) * (c.y - y) - (c.x - x) * (b.y - y)) / aire;
                let w1 = ((c.x - x) * (a.y - y) - (a.x - x) * (c.y - y)) / aire;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                    continue;
                }
                let zz = w0 * a.z + w1 * b.z + w2 * c.z;
                let k = j * nx + i;
                if zz > z[k] {
                    z[k] = zz;
                }
            }
        }
    }
    // une arête est cachée là où les quatre pixels qui entourent l'échantillon ont une
    // face nettement devant elle (tolérance : deux pixels et demi de profondeur)
    let tolerance = 2.5 * px;
    let visible_en = |p: DVec3| {
        let (x, y) = grille(p);
        let (i, j) = ((x - 0.5).floor().max(0.0) as usize, (y - 0.5).floor().max(0.0) as usize);
        let mut devant = f64::MAX;
        for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (ii, jj) = ((i + di).min(nx - 1), (j + dj).min(ny - 1));
            devant = devant.min(z[jj * nx + ii]);
        }
        p.z >= devant - tolerance
    };
    aretes
        .into_iter()
        .filter_map(|(i, segs)| {
            let mut out = Vec::new();
            for [a, b] in segs {
                let (pa, pb) = (proj(a), proj(b));
                let n = (((pb.x - pa.x).hypot(pb.y - pa.y) / px) * 2.0).ceil().max(1.0) as usize;
                let mut debut: Option<usize> = None;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let vu = visible_en(pa + (pb - pa) * t);
                    match (vu, debut) {
                        (true, None) => debut = Some(k),
                        (false, Some(k0)) => {
                            if k - 1 > k0 {
                                out.push([a + (b - a) * (k0 as f64 / n as f64), a + (b - a) * ((k - 1) as f64 / n as f64)]);
                            }
                            debut = None;
                        }
                        _ => {}
                    }
                }
                if let Some(k0) = debut
                    && k0 < n
                {
                    out.push([a + (b - a) * (k0 as f64 / n as f64), b]);
                }
            }
            (!out.is_empty()).then(|| Primitive { forme: Forme::Segments(out), style: Style { couleur: NOIR, epaisseur: 0.0 }, role: Role::Vu, element: Some(scene.objects[i].id) })
        })
        .collect()
}

/// Partie d'un triangle du côté gardé du plan (n·p + d ≥ 0) : polygone convexe.
fn decouper(t: &[DVec3], n: DVec3, d: f64) -> Vec<DVec3> {
    let mut out = Vec::with_capacity(4);
    for k in 0..t.len() {
        let (a, b) = (t[k], t[(k + 1) % t.len()]);
        let (sa, sb) = (n.dot(a) + d, n.dot(b) + d);
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out
}

/// Segment d'intersection d'un triangle et du plan (n, d), s'il le traverse.
fn section(p: [DVec3; 3], n: DVec3, d: f64) -> Option<[DVec3; 2]> {
    // sommet exactement sur le plan : compté du côté gardé
    let s = p.map(|q| {
        let s = n.dot(q) + d;
        if s == 0.0 { 1e-12 } else { s }
    });
    if s.iter().all(|&x| x > 0.0) || s.iter().all(|&x| x < 0.0) {
        return None;
    }
    let mut pts = Vec::with_capacity(2);
    for k in 0..3 {
        let (a, b) = (k, (k + 1) % 3);
        if (s[a] > 0.0) != (s[b] > 0.0) {
            let t = s[a] / (s[a] - s[b]);
            pts.push(p[a] + (p[b] - p[a]) * t);
        }
    }
    (pts.len() == 2 && pts[0].distance_squared(pts[1]) > 1e-14).then(|| [pts[0], pts[1]])
}

/// Suite de points dans le plan de coupe.
type Polyligne = Vec<[f64; 2]>;

/// Segments (coordonnées du plan) chaînés en boucles fermées et en lignes ouvertes.
pub(crate) fn chainer(segs: &[[[f64; 2]; 2]]) -> (Vec<Polyligne>, Vec<Polyligne>) {
    let cle = |p: [f64; 2]| ((p[0] / PAS).round() as i64, (p[1] / PAS).round() as i64);
    let mut voisins: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        if cle(s[0]) == cle(s[1]) {
            continue;
        }
        voisins.entry(cle(s[0])).or_default().push(i);
        voisins.entry(cle(s[1])).or_default().push(i);
    }
    let mut pris = vec![false; segs.len()];
    let (mut boucles, mut ouvertes) = (Vec::new(), Vec::new());
    // segment libre suivant au point p
    let suivant = |p: [f64; 2], pris: &[bool]| -> Option<(usize, [f64; 2])> {
        let i = *voisins.get(&cle(p))?.iter().find(|&&i| !pris[i])?;
        let s = segs[i];
        Some((i, if cle(s[0]) == cle(p) { s[1] } else { s[0] }))
    };
    for i in 0..segs.len() {
        if pris[i] || cle(segs[i][0]) == cle(segs[i][1]) {
            continue;
        }
        pris[i] = true;
        let depart = segs[i][0];
        let mut pts = vec![segs[i][0], segs[i][1]];
        while let Some((j, q)) = suivant(*pts.last().unwrap(), &pris) {
            pris[j] = true;
            pts.push(q);
            if cle(q) == cle(depart) {
                break;
            }
        }
        if pts.len() > 3 && cle(*pts.last().unwrap()) == cle(depart) {
            pts.pop();
            boucles.push(pts);
            continue;
        }
        // ligne ouverte : prolongée aussi vers l'arrière
        let mut avant = Vec::new();
        while let Some((j, q)) = suivant(*avant.last().unwrap_or(&depart), &pris) {
            pris[j] = true;
            avant.push(q);
        }
        avant.reverse();
        avant.extend(pts);
        ouvertes.push(avant);
    }
    (boucles, ouvertes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deux pavés du même matériau bout à bout : la jonction ne trace aucune arête ; en
    /// équerre, l'angle extérieur reste.
    #[test]
    fn jonction_sans_arete_meme_materiau() {
        let pave = |a: DVec3, b: DVec3| crate::modele::geometrie::tests::boite(a, b);
        let mut tris: Vec<([DVec3; 3], usize)> = pave(DVec3::ZERO, DVec3::new(1.0, 0.2, 1.0)).into_iter().map(|t| (t, 0)).collect();
        tris.extend(pave(DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 0.2, 1.0)).into_iter().map(|t| (t, 1)));
        let (vives, _) = aretes_jointes(&tris);
        // aucune arête dans le plan x = 1
        assert!(vives.iter().all(|(s, _)| !(s[0].x - 1.0).abs().lt(&1e-9) || !(s[1].x - 1.0).abs().lt(&1e-9)), "{vives:?}");
        // 12 arêtes d'un pavé 2 × 0,2 × 1 (les 4 longues coupées en deux, une par pavé)
        assert_eq!(vives.len(), 16);
        // sans jonction (calcul ordinaire) : le contour du plan x = 1 reste
        let seuls = aretes(&tris.iter().map(|t| t.0).collect::<Vec<_>>());
        assert!(seuls.vives.iter().any(|s| (s[0].x - 1.0).abs() < 1e-9 && (s[1].x - 1.0).abs() < 1e-9));
    }

    /// Cube unité en 12 triangles : 12 arêtes, aucune diagonale.
    fn cube() -> Vec<[DVec3; 3]> {
        let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z);
        let faces = [
            [v(0., 0., 0.), v(0., 1., 0.), v(1., 1., 0.), v(1., 0., 0.)],
            [v(0., 0., 1.), v(1., 0., 1.), v(1., 1., 1.), v(0., 1., 1.)],
            [v(0., 0., 0.), v(1., 0., 0.), v(1., 0., 1.), v(0., 0., 1.)],
            [v(1., 1., 0.), v(0., 1., 0.), v(0., 1., 1.), v(1., 1., 1.)],
            [v(0., 1., 0.), v(0., 0., 0.), v(0., 0., 1.), v(0., 1., 1.)],
            [v(1., 0., 0.), v(1., 1., 0.), v(1., 1., 1.), v(1., 0., 1.)],
        ];
        faces.iter().flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]]).collect()
    }

    fn longueur(a: &[[DVec3; 2]]) -> f64 {
        a.iter().map(|s| s[0].distance(s[1])).sum()
    }

    fn scene_section(tris: Vec<[DVec3; 3]>) -> Scene {
        use crate::scene::{Range, SceneObject};
        let mut s = crate::scene::projet_vide(&crate::modele::Projet::nouveau("Test"), std::path::Path::new("test.modele.json")).unwrap();
        s.vertices = tris.into_iter().flatten().map(|p| Vertex { pos: p.as_vec3().to_array(), normal: [0, 0, 127, 0], color: [140, 160, 180, 255], obj: 0 }).collect();
        s.couleurs_rendu = vec![[110, 130, 150, 255]; s.vertices.len()];
        s.objects.push(SceneObject { id: 1, class: "IFCWALL".into(), storey: None, opaque: Range { start: 0, count: s.vertices.len() as u32 }, transparent: Range { start: 0, count: 0 }, lines: Range { start: 0, count: 0 }, aretes: Range { start: 0, count: 0 }, min: Vec3::ZERO, max: Vec3::ONE });
        s.object_of.insert(1, 0);
        s
    }

    fn aire_poche(v: &[Vertex]) -> f64 {
        v.chunks_exact(3).filter(|t| t[0].color == [75, 75, 75, 255]).map(|t| {
            let p = [0, 1, 2].map(|k| Vec3::from(t[k].pos).as_dvec3());
            (p[1] - p[0]).cross(p[2] - p[0]).length() * 0.5
        }).sum()
    }

    #[test]
    fn section_box_six_faces_pleines() {
        let s = scene_section(cube());
        let a = Vec3::splat(0.2);
        let b = Vec3::splat(0.8);
        let (v, _) = sommets_boite(&s, &[true], a, b, 100.0, Some(Poche::Plein), false, false);
        assert!((aire_poche(&v) - 6.0 * 0.6 * 0.6).abs() < 1e-5);
        assert!(v.iter().all(|v| { let p = Vec3::from(v.pos); p.cmpge(a - Vec3::splat(1e-6)).all() && p.cmple(b + Vec3::splat(1e-6)).all() }));
    }

    #[test]
    fn section_box_conserve_le_vide_interieur() {
        let mut tris = cube();
        tris.extend(cube().into_iter().map(|t| t.map(|p| p * 0.5 + DVec3::splat(0.25))));
        let s = scene_section(tris);
        let (v, _) = sommets_boite(&s, &[true], Vec3::splat(-1.0), Vec3::new(2.0, 2.0, 0.5), 100.0, Some(Poche::Plein), false, false);
        assert!((aire_poche(&v) - 0.75).abs() < 1e-6, "aire {}", aire_poche(&v));
    }

    #[test]
    fn section_box_couches_materiaux_et_cavite() {
        let mut tris = cube();
        tris.extend(cube().into_iter().map(|t| t.map(|p| p * 0.5 + DVec3::splat(0.25))));
        let mut s = scene_section(tris);
        let bande = |a: f32, b: f32, couleur| crate::scene::CoucheCoupe {
            plans: vec![Vec3::X.extend(-a),Vec3::NEG_X.extend(b)], couleur, fond: [255,255,255,255], motif_fond: None, motif: None, couleur_motif: [70,70,70,255], hachure_defaut: true, isolation: false, trait_mm: TRAIT_FORT_MM,
        };
        s.couches_coupe.insert(1,vec![bande(0.0,0.2,[190,120,120,255]),bande(0.2,1.0,[120,190,120,255])]);
        let p = coupe(&s,&[true],Vec3::NEG_Z.extend(0.5),100.0,Poche::Hachure);
        let surfaces: Vec<_> = p.iter().filter(|p| p.role == Role::Poche).collect();
        assert_eq!(surfaces.len(),2);
        let aire = |p: &Primitive| p.mailler(DVec3::NEG_Z,100.0).triangles.iter().map(|t| (t[1]-t[0]).cross(t[2]-t[0]).length()*0.5).sum::<f64>();
        assert!((aire(surfaces[0])-0.2).abs()<1e-6);
        assert!((aire(surfaces[1])-0.55).abs()<1e-6);
        assert!(p.iter().any(|p| p.role == Role::Hachure));
        let (v,_) = sommets_boite(&s,&[true],Vec3::splat(-1.0),Vec3::new(2.,2.,0.5),100.0,None,false, false);
        assert!(v.iter().any(|v| v.color == [190,120,120,255]));
        assert!(v.iter().any(|v| v.color == [120,190,120,255]));
    }

    #[test]
    fn section_box_hachures_bornees() {
        let s = scene_section(cube());
        let (a, b) = (Vec3::splat(0.2), Vec3::splat(0.8));
        let (_, traits) = sommets_boite(&s, &[true], a, b, 100.0, Some(Poche::Hachure), false, false);
        assert!(!traits.is_empty());
        assert!(traits.iter().all(|v| { let p = Vec3::from(v.pos); p.cmpge(a - Vec3::splat(1e-6)).all() && p.cmple(b + Vec3::splat(1e-6)).all() }));
    }

    #[test]
    fn section_box_sans_coupe_ou_objet_masque() {
        let s = scene_section(cube());
        for (visible, a, b) in [(true, Vec3::splat(-1.0), Vec3::splat(2.0)), (false, Vec3::splat(0.2), Vec3::splat(0.8))] {
            let (v, t) = sommets_boite(&s, &[visible], a, b, 100.0, Some(Poche::Plein), false, false);
            assert!(v.is_empty() && t.is_empty());
        }
    }

    #[test]
    fn section_box_couleur_matiere_et_modification() {
        let s = scene_section(cube());
        for rendu in [false, true] {
            let (v, _) = sommets_boite(&s, &[true], Vec3::splat(0.2), Vec3::splat(0.8), 100.0, None, rendu, false);
            let couleur = if rendu { [110, 130, 150, 255] } else { [140, 160, 180, 255] };
            assert!(v.iter().any(|v| v.color == couleur));
            assert!(v.iter().all(|v| v.obj == 0));
        }
        let (v, _) = sommets_boite(&s, &[true], Vec3::splat(0.3), Vec3::splat(0.7), 100.0, Some(Poche::Plein), false, false);
        assert!((aire_poche(&v) - 6.0 * 0.4 * 0.4).abs() < 1e-5);
    }

    #[test]
    fn aretes_cube() {
        let a = aretes(&cube());
        assert!((longueur(&a.vives) - 12.0).abs() < 1e-6, "{}", longueur(&a.vives));
        assert!(a.douces.is_empty());
    }

    /// Face du dessus découpée avec une jonction en T (comme après une CSG) : la découpe
    /// n'est pas une arête.
    #[test]
    fn jonction_en_t() {
        let mut t = cube();
        t.retain(|t| !t.iter().all(|p| p.z == 1.0));
        let v = |x: f64, y: f64| DVec3::new(x, y, 1.0);
        // moitié gauche en deux triangles, moitié droite en trois (sommet en T à (0.5, 0.5))
        t.extend([[v(0., 0.), v(0.5, 0.), v(0.5, 1.)], [v(0., 0.), v(0.5, 1.), v(0., 1.)]]);
        t.extend([[v(0.5, 0.), v(1., 0.), v(0.5, 0.5)], [v(1., 0.), v(1., 1.), v(0.5, 0.5)], [v(0.5, 0.5), v(1., 1.), v(0.5, 1.)]]);
        let a = aretes(&t).vives;
        assert!((longueur(&a) - 12.0).abs() < 1e-6, "{}", longueur(&a));
    }

    /// Cylindre à 32 facettes : seuls les deux cercles sont des arêtes vives ; les 32
    /// génératrices sont des arêtes douces (silhouettes).
    #[test]
    fn cylindre_sans_generatrices() {
        let k = 32;
        let p = |i: usize, z: f64| {
            let a = i as f64 / k as f64 * std::f64::consts::TAU;
            DVec3::new(a.cos(), a.sin(), z)
        };
        let mut t = Vec::new();
        for i in 0..k {
            let j = (i + 1) % k;
            t.push([p(i, 0.), p(j, 0.), p(j, 1.)]);
            t.push([p(i, 0.), p(j, 1.), p(i, 1.)]);
            t.push([DVec3::ZERO, p(j, 0.), p(i, 0.)]);
            t.push([DVec3::Z, p(i, 1.), p(j, 1.)]);
        }
        let a = aretes(&t);
        let perimetre = 2.0 * k as f64 * 2.0 * (std::f64::consts::PI / k as f64).sin();
        assert!((longueur(&a.vives) - perimetre).abs() < 1e-6, "{} {}", longueur(&a.vives), perimetre);
        assert_eq!(a.douces.len(), k);
        assert!(a.douces.iter().all(|(s, n1, n2)| (s[0].distance(s[1]) - 1.0).abs() < 1e-9 && n1.dot(*n2) < 0.99));
    }

    /// Polygone découpé : un triangle à cheval garde un quadrilatère.
    #[test]
    fn decoupe_triangle() {
        let t = [DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 2.0)];
        let q = decouper(&t, DVec3::NEG_Z, 1.0);
        assert_eq!(q.len(), 4);
        assert!(q.iter().all(|p| p.z <= 1.0 + 1e-12));
    }

    #[test]
    fn coupe_carree() {
        let segs: Vec<[[f64; 2]; 2]> = vec![[[0., 0.], [1., 0.]], [[1., 1.], [1., 0.]], [[1., 1.], [0., 1.]], [[0., 1.], [0., 0.]], [[3., 0.], [4., 0.]]];
        let (b, o) = chainer(&segs);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].len(), 4);
        assert_eq!(o.len(), 1);
        let s = section([DVec3::ZERO, DVec3::new(1., 0., 2.), DVec3::new(0., 1., 2.)], DVec3::NEG_Z, 1.0).unwrap();
        assert!((s[0].z - 1.0).abs() < 1e-12 && (s[1].z - 1.0).abs() < 1e-12);
    }
}

#[cfg(test)]
mod mesure {
    /// Sphère de 500 000 triangles : durée du calcul des arêtes.
    #[test]
    #[ignore]
    fn sphere_500k() {
        use glam::DVec3;
        let (nu, nv) = (500, 500);
        let p = |i: usize, j: usize| {
            let (u, v) = (i as f64 / nu as f64 * std::f64::consts::TAU, j as f64 / nv as f64 * std::f64::consts::PI);
            DVec3::new(v.sin() * u.cos(), v.sin() * u.sin(), v.cos()) * 10.0
        };
        let mut t = Vec::new();
        for i in 0..nu {
            for j in 0..nv {
                let (a, b, c, d) = (p(i, j), p((i + 1) % nu, j), p((i + 1) % nu, j + 1), p(i, j + 1));
                t.push([a, b, c]);
                t.push([a, c, d]);
            }
        }
        let t0 = std::time::Instant::now();
        let a = super::aretes(&t);
        println!("{} triangles : {} vives, {} douces en {:?}", t.len(), a.vives.len(), a.douces.len(), t0.elapsed());
    }
}
