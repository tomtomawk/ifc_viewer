//! Rendu « dessin » des plans et élévations, à la manière d'un logiciel de DAO (CADCraft) :
//! fond blanc, faces blanches qui masquent ce qu'elles cachent (tampon de profondeur),
//! arêtes vives des maillages en trait fin, et coupe par un plan : poché (plein, hachuré ou
//! vide) et contour en trait fort. Épaisseurs et pas des hachures en mm du papier,
//! multipliés par l'échelle du dessin.

mod motifs;
pub use motifs::*;

pub mod dao;
use crate::formats::rvt::vues::{GrilleMotif, Motif, hachures_plan};
use crate::rendu::scene::{Scene, Vertex};
use noyau::trace2d::{Forme, Primitive, Role, Style};
use glam::{DVec3, Vec3, Vec4};
use rayon::prelude::*;
use std::collections::HashMap;
use noyau::aretes::{NOIR, PAS, Polyligne, TRAIT_FORT_MM, aretes, chainer};

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
        [Poche::Hachure, Poche::Plein, Poche::Vide, Poche::Materiau].into_iter().find(|p| p.nom() == nom)
    }
}

/// Trait fin des pièces translucides coupées (vitrages), mm du papier.
pub const TRAIT_FIN_MM: f64 = 0.05;
/// Pas des hachures du poché, mm du papier.
pub const PAS_HACHURE_MM: f64 = 1.5;

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
            // sans bandes de matériau : chaque solide de l'élément (pièces d'une menuiserie)
            // est sectionné à part, sinon leurs contours jointifs se chaînent mal
            if !scene.couches_coupe.contains_key(&ob.id) {
                return coupe_par_solides(scene, ob, n, d, o, e1, e2, poche, fond, hachure, &motif, echelle);
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
                            // Le dessin de l'isolant calcule ses tolérances en millimètres ;
                            // les coordonnées de la scène sont en mètres.
                            let t2: Vec<_> = tris.iter().map(|t| t.map(|p| [(p-o).dot(e1) * 1000.0,(p-o).dot(e2) * 1000.0])).collect();
                            let segments = if let Some([x, y]) = couche.centre_isolation.filter(|_| n.z.abs() > 0.999) {
                                let centre = DVec3::new(x - scene.origin.x, y - scene.origin.y, o.z);
                                motifs::ligne_isolation_courbe(&t2, [(centre-o).dot(e1)*1000.0,(centre-o).dot(e2)*1000.0])
                            } else { ligne_isolation(&t2) };
                            let h = segments.into_iter().map(|s| s.map(|p| en_3d([p[0] / 1000.0, p[1] / 1000.0]))).collect();
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
                let t2: Vec<[[f64; 2]; 3]> = noyau::texte::trianguler(contours).iter().map(|t| t.map(|p| [p[0] as f64, p[1] as f64])).collect();
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

/// Coupe d'un élément sans bandes de matériau, solide par solide : les triangles sont
/// regroupés par arêtes communes (une arête partagée par deux triangles exactement),
/// chaque groupe donne ses propres contours, son poché et ses hachures. Un solide
/// translucide (vitrage) reste blanc, sans hachure, au trait fin.
#[allow(clippy::too_many_arguments)]
fn coupe_par_solides(scene: &Scene, ob: &crate::rendu::scene::SceneObject, n: DVec3, d: f64, o: DVec3, e1: DVec3, e2: DVec3, poche: Poche, fond: [u8; 4], hachure: Option<[u8; 4]>, motif: &Motif, echelle: f64) -> Vec<Primitive> {
    let cle = |p: [f32; 3]| p.map(|x| (x as f64 * 1e4).round() as i64);
    let mut tris: Vec<(usize, bool)> = Vec::new();
    for (r, translucide) in [(ob.opaque, false), (ob.transparent, true)] {
        tris.extend((0..r.count as usize / 3).map(|k| (r.start as usize + 3 * k, translucide)));
    }
    // union des triangles voisins par une arête
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn racine(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i { parent[i] = parent[parent[i]]; i = parent[i]; }
        i
    }
    let mut aretes: HashMap<([i64; 3], [i64; 3]), Vec<usize>> = HashMap::new();
    for (i, (k, _)) in tris.iter().enumerate() {
        let c = [0, 1, 2].map(|j| cle(scene.vertices[k + j].pos));
        for j in 0..3 {
            let (a, b) = (c[j], c[(j + 1) % 3]);
            if a != b { aretes.entry(if a < b { (a, b) } else { (b, a) }).or_default().push(i); }
        }
    }
    // pièces accolées par une face (arête commune à plus de deux triangles) : réunies
    // si elles ont la même couleur, donc le même matériau
    let couleur = |i: usize| scene.vertices[tris[i].0].color;
    for v in aretes.values() {
        for w in 1..v.len() {
            if let Some(&u) = v[..w].iter().find(|&&u| couleur(u) == couleur(v[w])) {
                let (a, b) = (racine(&mut parent, u), racine(&mut parent, v[w]));
                if a != b { parent[a] = b; }
            }
        }
    }
    let mut lots: HashMap<usize, (Vec<[[f64; 2]; 2]>, usize, bool)> = HashMap::new();
    for i in 0..tris.len() {
        let (k, translucide) = tris[i];
        let p = [0, 1, 2].map(|j| Vec3::from(scene.vertices[k + j].pos).as_dvec3());
        if let Some(s) = section(p, n, d) {
            let r = racine(&mut parent, i);
            lots.entry(r).or_insert_with(|| (Vec::new(), k, translucide)).0.push(s.map(|q| [(q - o).dot(e1), (q - o).dot(e2)]));
        }
    }
    let mut lots: Vec<_> = lots.into_values().collect();
    lots.sort_by_key(|l| l.1);
    // solide contenu dans un autre (paroi intérieure d'un élément creux) : sectionnés
    // ensemble, le contour intérieur fait un vide
    let dedans = |q: [f64; 2], boucles: &[Polyligne]| {
        boucles.iter().map(|b| (0..b.len()).filter(|&k| {
            let (a, c) = (b[k], b[(k + 1) % b.len()]);
            (a[1] > q[1]) != (c[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) / (c[1] - a[1]) * (c[0] - a[0])
        }).count()).sum::<usize>() % 2 == 1
    };
    let contours: Vec<Vec<Polyligne>> = lots.iter().map(|l| chainer(&l.0).0).collect();
    let mut cible: Vec<usize> = (0..lots.len()).collect();
    for i in 0..lots.len() {
        let Some(b) = contours[i].first() else { continue };
        let centre = [b.iter().map(|q| q[0]).sum::<f64>() / b.len() as f64, b.iter().map(|q| q[1]).sum::<f64>() / b.len() as f64];
        if let Some(j) = (0..lots.len()).find(|&j| j != i && cible[j] != i && dedans(centre, &contours[j])) { cible[i] = j; }
    }
    for i in 0..lots.len() {
        let mut j = cible[i];
        for _ in 0..lots.len() { if cible[j] == j { break; } j = cible[j]; }
        if j != i && cible[j] == j {
            let segs = std::mem::take(&mut lots[i].0);
            lots[j].0.extend(segs);
        }
    }
    lots.retain(|l| !l.0.is_empty());
    let en_3d = |p: [f64; 2]| o + e1 * p[0] + e2 * p[1];
    let primitive = |forme, couleur, epaisseur, role| Primitive { forme, style: Style { couleur, epaisseur }, role, element: Some(ob.id) };
    let mut prims = Vec::new();
    for (segs, k, translucide) in lots {
        // faces communes de deux pièces réunies : sections en double, retirées
        let cle2 = |q: [f64; 2]| ((q[0] / PAS).round() as i64, (q[1] / PAS).round() as i64);
        let cle_seg = |s: &[[f64; 2]; 2]| { let (a, b) = (cle2(s[0]), cle2(s[1])); if a < b { (a, b) } else { (b, a) } };
        let mut nombre: HashMap<_, usize> = HashMap::new();
        for s in &segs { *nombre.entry(cle_seg(s)).or_default() += 1; }
        let segs: Vec<_> = segs.into_iter().filter(|s| nombre[&cle_seg(s)] == 1).collect();
        let (boucles, ouvertes) = chainer(&segs);
        if !boucles.is_empty() {
            let fond = match poche {
                // style Réaliste : couleur du solide (rendu, sinon affichage)
                Poche::Materiau => scene.couleurs_rendu.get(k).filter(|c| c[3] > 0).copied().or_else(|| scene.vertices.get(k).map(|v| v.color)).map_or(fond, |c| [c[0], c[1], c[2], 255]),
                _ if translucide => [255, 255, 255, 255],
                _ => fond,
            };
            let surface = primitive(Forme::Surface(boucles.iter().map(|b| b.iter().map(|p| en_3d(*p)).collect()).collect()), fond, 0.0, Role::Poche);
            // hachures sur le maillage du poché (indépendant du sens des contours)
            if let Some(c) = hachure.filter(|_| !translucide) {
                let t2: Vec<[[f64; 2]; 3]> = surface.mailler(n, echelle).triangles.iter().map(|t| t.map(|p| [(p - o).dot(e1), (p - o).dot(e2)])).collect();
                let s: Vec<[DVec3; 2]> = hachures_plan(&t2, motif, 1.0, 20_000).into_iter().map(|s| s.map(en_3d)).collect();
                prims.push(surface);
                if !s.is_empty() { prims.push(primitive(Forme::Segments(s), c, 0.0, Role::Hachure)); }
            } else {
                prims.push(surface);
            }
        }
        let trait_mm = if translucide { TRAIT_FIN_MM } else { TRAIT_FORT_MM };
        for (pts, fermee) in boucles.iter().map(|b| (b, true)).chain(ouvertes.iter().map(|b| (b, false))) {
            prims.push(primitive(Forme::Polyligne { points: pts.iter().map(|p| en_3d(*p)).collect(), fermee }, NOIR, trait_mm, Role::Coupe));
        }
    }
    prims
}

/// Nettoyage des jonctions comme Revit : un trait de coupe commun à deux éléments du
/// même matériau (deux couches identiques raccordées en onglet) est retiré, les deux
/// régions se lisent d'un seul tenant.
fn fusionner_materiaux(scene: &Scene, mut prims: Vec<Primitive>) -> Vec<Primitive> {
    use std::collections::HashMap;
    // matériau d'un élément : sa bande de coupe unique (couches des murs natifs)
    let materiau = |id: Option<crate::formats::ifc::step::Id>| -> Option<([u8; 4], String)> {
        let c = scene.couches_coupe.get(&id?)?;
        let [b] = c.as_slice() else { return None };
        Some((b.couleur, b.motif.as_ref().map_or_else(String::new, |m| m.nom.clone())))
    };
    // traits de coupe rangés par droite porteuse : (primitive, rang, t0, t1, élément).
    // Une droite regroupe les traits dont les deux extrémités sont à moins de 0,1 mm d'elle,
    // quel que soit leur sens : une clé arrondie sépare à tort un trait court (direction
    // bruitée) ou un onglet à 45° (signe de la direction canonique instable) de sa droite.
    type Trait = (usize, usize, f64, f64, Option<crate::formats::ifc::step::Id>);
    const TOLERANCE: f64 = 1e-4;
    let case = |u: DVec3| [(u.x * 20.0).floor() as i64, (u.y * 20.0).floor() as i64, (u.z * 20.0).floor() as i64];
    let mut droites: Vec<(DVec3, DVec3, Vec<Trait>)> = Vec::new();
    let mut index: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
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
                let u = d.normalize();
                let distance = |q: DVec3, v: DVec3| (q - v * q.dot(v)).length();
                // droite existante : cases voisines de u et de −u
                let mut trouvee = None;
                'cherche: for signe in [1.0, -1.0] {
                    let c = case(u * signe);
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                let Some(liste) = index.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) else { continue };
                                for &n in liste {
                                    let (v, o, _) = droites[n];
                                    if distance(s[0] - o, v) < TOLERANCE && distance(s[1] - o, v) < TOLERANCE {
                                        trouvee = Some(n);
                                        break 'cherche;
                                    }
                                }
                            }
                        }
                    }
                }
                let n = trouvee.unwrap_or_else(|| {
                    droites.push((u, s[0], Vec::new()));
                    index.entry(case(u)).or_default().push(droites.len() - 1);
                    droites.len() - 1
                });
                let v = droites[n].0;
                let (ta, tb) = (s[0].dot(v), s[1].dot(v));
                droites[n].2.push((k, i, ta.min(tb), ta.max(tb), p.element));
            }
        }
    }
    // partie de chaque trait recouverte par un trait d'un autre élément du même matériau :
    // retirée des deux (un trait court posé sur un trait long compris)
    let mut remplacer: HashMap<(usize, usize), Vec<[DVec3; 2]>> = HashMap::new();
    for (u, _, traits) in &droites {
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
    let raccords = (!fins).then(|| noyau::trace2d::Raccords::new(prims, echelle));
    for p in prims {
        let obj = p.element.and_then(|e| scene.object_of.get(&e).copied()).unwrap_or(u32::MAX);
        let decal = -normale * if p.role == noyau::trace2d::Role::Coupe { (0.4 * largeur).min(0.004) } else { (0.2 * largeur).min(0.002) };
        let sommet = |q: DVec3| Vertex { pos: (q + decal).as_vec3().to_array(), normal: [0, 0, 0, 127], color: p.style.couleur, obj };
        let m = maillage(p, normale, echelle, fins, raccords.as_ref());
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

/// Maillage d'une primitive ; traits fins : ses traits épais ramenés à un pixel ; sinon
/// bouts raccordés aux traits rejoints.
fn maillage(p: &Primitive, normale: DVec3, echelle: f64, fins: bool, raccords: Option<&noyau::trace2d::Raccords>) -> noyau::trace2d::Maillage {
    if fins && p.style.epaisseur > 0.0 {
        let mut fin = p.clone();
        fin.style.epaisseur = 0.0;
        return fin.mailler(normale, echelle);
    }
    p.mailler_raccords(normale, echelle, raccords)
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
        let prims = coupe(scene, visible, plan, echelle, poche.unwrap_or(Poche::Vide));
        let raccords = (!fins).then(|| noyau::trace2d::Raccords::new(&prims, echelle));
        for mut p in prims {
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
            let maillage = maillage(&p, normale, echelle, fins, raccords.as_ref());
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

/// Cadrage vectoriel des arêtes et poches d'une coupe ou d'une élévation.
pub fn cadrer_primitives(primitives: Vec<Primitive>, plans: &[Vec4], normale: DVec3) -> Vec<Primitive> {
    if plans.is_empty() { return primitives; }
    let distance = |p: DVec3, plan: &Vec4| plan.truncate().as_dvec3().dot(p) + plan.w as f64;
    let segment = |mut s: [DVec3; 2]| {
        for plan in plans {
            let [a, b] = s.map(|p| distance(p, plan));
            if a < 0.0 && b < 0.0 { return None; }
            if (a < 0.0) != (b < 0.0) {
                let q = s[0].lerp(s[1], a / (a - b));
                s[usize::from(a >= 0.0)] = q;
            }
        }
        (s[0].distance_squared(s[1]) > 1e-20).then_some(s)
    };
    let faces = |tris: Vec<[DVec3; 3]>| {
        tris.into_iter().flat_map(|t| {
            let poly = plans.iter().fold(t.to_vec(), |poly, plan| decouper(&poly, plan.truncate().as_dvec3(), plan.w as f64));
            (1..poly.len().saturating_sub(1)).map(move |i| [poly[0], poly[i], poly[i + 1]])
        }).collect::<Vec<_>>()
    };
    primitives.into_iter().filter_map(|mut pr| {
        pr.forme = match pr.forme {
            Forme::Segments(s) => Forme::Segments(s.into_iter().filter_map(segment).collect()),
            Forme::Polyligne { points, fermee } => {
                let mut s: Vec<_> = points.windows(2).filter_map(|p| segment([p[0], p[1]])).collect();
                if fermee && points.len() > 2 && let Some(t) = segment([*points.last().unwrap(), points[0]]) { s.push(t); }
                Forme::Segments(s)
            }
            Forme::Triangles(t) => Forme::Triangles(faces(t)),
            Forme::Surface(c) => Forme::Triangles(faces(noyau::trace2d::trianguler(&c, normale))),
        };
        match &pr.forme {
            Forme::Segments(s) if s.is_empty() => None,
            Forme::Triangles(t) if t.is_empty() => None,
            _ => Some(pr),
        }
    }).collect()
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

#[cfg(test)]
mod tests {

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[[f64; 2]; 3]> {
        vec![[[x0, y0], [x1, y0], [x1, y1]], [[x0, y0], [x1, y1], [x0, y1]]]
    }

    #[test]
    fn isolant_rectangle_motif_dans_la_couche_et_le_long_de_l_axe() {
        let t = rect(0.0, 0.0, 6000.0, 100.0);
        let s = super::ligne_isolation(&t);
        assert!(!s.is_empty());
        for [a, b] in &s {
            for p in [a, b] {
                assert!(p[1] > -1e-6 && p[1] < 100.0 + 1e-6, "hors couche {p:?}");
            }
        }
        let (xmin, xmax) = s.iter().flatten().fold((f64::MAX, f64::MIN), |(m, n), p| (m.min(p[0]), n.max(p[0])));
        assert!(xmin < 100.0 && xmax > 5900.0, "motif incomplet {xmin} {xmax}");
    }

    #[test]
    fn isolant_mur_droit_avec_onglets_reste_dans_la_couche() {
        // Coupe d'une couche droite de 145 mm, avec deux onglets et des
        // sommets intermédiaires sur les faces issus du maillage.
        let a = [62.1, 8321.7];
        let b = [0.0, 8259.6];
        let c = [0.0, 3051.3];
        let d = [0.0, -855.0];
        let e = [82.8, -937.9];
        let f = [145.0, -1000.0];
        let g = [145.0, 4374.1];
        let h = [145.0, 8404.6];
        let t = [[a, b, c], [c, d, e], [e, f, g], [g, h, a], [a, c, e], [e, g, a]];
        let s = super::ligne_isolation(&t);
        assert!(!s.is_empty());
        assert!(s.iter().flatten().all(|p| (-1e-6..=145.0 + 1e-6).contains(&p[0])));
        assert!(s.iter().flatten().any(|p| p[1] < -850.0));
        assert!(s.iter().flatten().any(|p| p[1] > 8250.0));
    }

    #[test]
    fn isolant_en_l_garde_l_epaisseur_de_la_couche() {
        // couche de 100 retournée en tableau sur 400 : le motif reste à l'échelle de 100.
        let l: Vec<[f32; 2]> = vec![[0.0, 0.0], [3000.0, 0.0], [3000.0, 100.0], [100.0, 100.0], [100.0, 500.0], [0.0, 500.0]];
        let t: Vec<[[f64; 2]; 3]> = noyau::texte::trianguler(vec![l]).iter().map(|t| t.map(|p| [p[0] as f64, p[1] as f64])).collect();
        assert_eq!(super::composantes(&t).len(), 1);
        let s = super::ligne_isolation(&t);
        assert!(!s.is_empty());
        let ymax = s.iter().flatten().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert!(ymax <= 100.0 + 1e-6, "motif dans le retour : {ymax}");
    }

    #[test]
    fn isolant_morceaux_disjoints_traites_separement() {
        let mut t = rect(0.0, 0.0, 2000.0, 100.0);
        t.extend(rect(3000.0, 0.0, 5000.0, 100.0));
        let s = super::ligne_isolation(&t);
        assert!(s.iter().any(|[a, _]| a[0] < 2000.0) && s.iter().any(|[a, _]| a[0] > 3000.0));
        assert!(s.iter().all(|[a, b]| !(a[0] > 2000.0 + 1e-6 && a[0] < 3000.0 - 1e-6) || !(b[0] > 2000.0 && b[0] < 3000.0)));
    }

    #[test]
    fn isolant_suit_une_couche_courbe() {
        // couronne de rayons 5000 à 5100 sur 90°, en 48 facettes
        let mut t = Vec::new();
        let p = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
        for k in 0..48 {
            let (a0, a1) = (k as f64 / 48.0 * std::f64::consts::FRAC_PI_2, (k + 1) as f64 / 48.0 * std::f64::consts::FRAC_PI_2);
            t.push([p(5000.0, a0), p(5100.0, a0), p(5100.0, a1)]);
            t.push([p(5000.0, a0), p(5100.0, a1), p(5000.0, a1)]);
        }
        let s = super::ligne_isolation(&t);
        assert!(s.len() > 100);
        for p in s.iter().flatten() {
            let r = p[0].hypot(p[1]);
            assert!(r > 4999.0 && r < 5101.0, "hors couche {r}");
        }
        let angles: Vec<f64> = s.iter().flatten().map(|p| p[1].atan2(p[0])).collect();
        assert!(angles.iter().any(|a| *a < 0.1) && angles.iter().any(|a| *a > 1.47), "motif incomplet : {:?}", angles.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a,b), &x| (a.min(x), b.max(x))));
    }

    #[test]
    fn isolant_courbe_aux_unites_de_la_scene() {
        let p = |r: f64, a: f64| [r * a.cos(), r * a.sin()];
        let mut triangles = Vec::new();
        for k in 0..48 {
            let a = k as f64 / 48.0 * std::f64::consts::FRAC_PI_2;
            let b = (k + 1) as f64 / 48.0 * std::f64::consts::FRAC_PI_2;
            triangles.push([p(5.0, a), p(5.1, a), p(5.1, b)]);
            triangles.push([p(5.0, a), p(5.1, b), p(5.0, b)]);
        }
        let s = super::ligne_isolation(&triangles);
        assert!(s.len() > 100, "{} segments", s.len());
        assert!(s.iter().flatten().all(|p| (4.999..5.101).contains(&p[0].hypot(p[1]))));
        assert!(s.iter().flatten().any(|p| p[1].atan2(p[0]) > 1.47));
    }

    #[test]
    fn isolant_suit_le_mur_courbe_dans_la_coupe_native() {
        use crate::modele::{Axe, Couche, FonctionCouche, Projet, TypeCompose};
        use crate::modele::edition::ParametresMur;
        let mut projet = Projet::nouveau("Mur courbe");
        projet.types_murs.push(TypeCompose { id: "isolant".into(), nom: "Isolant".into(), couches: vec![Couche { materiau: Some("Laine minérale".into()), epaisseur: 100.0, fonction: FonctionCouche::Isolation, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None });
        let options = ParametresMur { type_mur: Some("isolant".into()), ..Default::default() };
        projet.ajouter_mur_axe(Axe::Arc { debut: [17000.0, -8000.0], fin: [12000.0, -3000.0], centre: [12000.0, -8000.0], direct: true }, &options).unwrap();
        projet.completer_materiaux();
        let scene = crate::rendu::scene::depuis_natif(&projet, std::path::Path::new("test.modele.json")).unwrap();
        let plan = Vec3::NEG_Z.extend((1.0 - scene.origin.z) as f32);
        let primitives = super::coupe(&scene, &vec![true; scene.objects.len()], plan, 100.0, Poche::Hachure);
        let segments: Vec<_> = primitives.iter().filter(|p| p.role == Role::Hachure).flat_map(|p| match &p.forme { Forme::Segments(s) => s.clone(), _ => Vec::new() }).collect();
        assert!(segments.len() > 100, "{} segments", segments.len());
        let rayon = |p: DVec3| (p.x + scene.origin.x - 12.0).hypot(p.y + scene.origin.y + 8.0);
        assert!(segments.iter().flatten().all(|&p| (4.94..5.06).contains(&rayon(p))), "le motif sort de la couche");
        let angles: Vec<_> = segments.iter().flatten().map(|p| (p.y + scene.origin.y + 8.0).atan2(p.x + scene.origin.x - 12.0)).collect();
        assert!(angles.iter().any(|a| *a < 0.1) && angles.iter().any(|a| *a > 1.47), "motif incomplet : {:?}", angles.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a,b), &x| (a.min(x), b.max(x))));
    }

    #[test]
    fn ossature_coupee_en_plan_montants_et_travees() {
        use crate::modele::{Axe, Couche, FonctionCouche, Materiau, Projet, TypeCompose};
        use crate::modele::edition::ParametresMur;
        use crate::modele::murs::ossature::Ossature;
        let mut projet = Projet::nouveau("Ossature");
        let couche = Couche { materiau: Some("Laine".into()), epaisseur: 145.0, fonction: FonctionCouche::Structure, ossature: Some(Ossature { materiau: Some("Sapin".into()), ..Default::default() }), ..Default::default() };
        projet.types_murs.push(TypeCompose { id: "mob".into(), nom: "MOB".into(), couches: vec![couche], categorie: None, couches_detaillees: true, origine: None });
        projet.materiaux.push(Materiau { nom: "Sapin".into(), couleur: [200, 150, 90], ..Default::default() });
        projet.materiaux.push(Materiau { nom: "Laine".into(), couleur: [250, 230, 120], motif: crate::modele::MotifCoupe::Isolation, ..Default::default() });
        let options = ParametresMur { type_mur: Some("mob".into()), ..Default::default() };
        projet.ajouter_mur([0.0, 0.0], [3000.0, 0.0], &options).unwrap();
        projet.ajouter_mur_axe(Axe::Arc { debut: [17000.0, -8000.0], fin: [12000.0, -3000.0], centre: [12000.0, -8000.0], direct: true }, &options).unwrap();
        let scene = crate::rendu::scene::depuis_natif(&projet, std::path::Path::new("test.modele.json")).unwrap();
        let plan = Vec3::NEG_Z.extend((1.0 - scene.origin.z) as f32);
        let primitives = super::coupe(&scene, &vec![true; scene.objects.len()], plan, 100.0, Poche::Materiau);
        let aire = |couleur: [u8; 4], droit: bool| -> f64 {
            primitives.iter().filter(|p| p.role == Role::Poche && p.style.couleur == couleur).flat_map(|p| match &p.forme { Forme::Triangles(t) => t.clone(), _ => Vec::new() })
                .filter(|t| ((t[0].y + scene.origin.y).abs() < 0.5) == droit)
                .map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0).sum()
        };
        // mur droit de 3 m : 6 montants de 45 × 145, le reste en remplissage
        let (bois, laine) = (aire([200, 150, 90, 255], true), aire([250, 230, 120, 255], true));
        assert!((bois - 6.0 * 0.045 * 0.145).abs() < 1e-6, "{bois}");
        assert!((bois + laine - 3.0 * 0.145).abs() < 1e-6, "{laine}");
        // la ligne d'isolant de chaque travée reste dans l'épaisseur du mur droit
        let hachures = super::coupe(&scene, &vec![true; scene.objects.len()], plan, 100.0, Poche::Hachure);
        let isolant: Vec<_> = hachures.iter().filter(|p| p.role == Role::Hachure).flat_map(|p| match &p.forme { Forme::Segments(s) => s.clone(), _ => Vec::new() })
            .filter(|s| (s[0].y + scene.origin.y).abs() < 0.5).collect();
        assert!(isolant.len() > 100);
        assert!(isolant.iter().flatten().all(|q| (q.y + scene.origin.y).abs() < 0.08 && (-0.001..3.001).contains(&(q.x + scene.origin.x))), "ligne d'isolant hors du mur : {:?}", isolant.iter().flatten().filter(|q| !((q.y + scene.origin.y).abs() < 0.08 && (-0.001..3.001).contains(&(q.x + scene.origin.x)))).map(|q| (((q.x + scene.origin.x) * 1e4).round() / 1e4, ((q.y + scene.origin.y) * 1e4).round() / 1e4)).take(12).collect::<Vec<_>>());
        // mur courbe (quart de cercle de 5 m de rayon) : 14 montants, sans doublon en face
        let (bois, laine) = (aire([200, 150, 90, 255], false), aire([250, 230, 120, 255], false));
        let total = std::f64::consts::FRAC_PI_2 * 5.0 * 0.145;
        assert!((bois + laine - total).abs() < 1e-3, "{bois} + {laine} / {total}");
        assert!((bois - 14.0 * 0.045 * 0.145).abs() < 2e-3, "{bois}");
    }

    #[test]
    fn fenetre_coupee_piece_par_piece() {
        use crate::modele::{Ouverture, Projet};
        let g = crate::modele::gabarit::Gabarit::defaut();
        let mut p = Projet::depuis_gabarit("OF", &g);
        let tm = p.types_murs.iter().find(|t| t.nom.starts_with("Mur BA 200 + doublage")).unwrap().id.clone();
        let mur = p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &crate::modele::edition::ParametresMur { type_mur: Some(tm), ..Default::default() }).unwrap();
        for (k, t) in p.types_ouvertures.clone().iter().filter(|t| t.genre == crate::modele::GenreOuverture::Fenetre).enumerate() {
            p.ouvertures = vec![Ouverture { id: format!("o{k}"), nom: "F".into(), genre: t.genre, mur: mur.clone(), type_ouverture: Some(t.id.clone()), abscisse: 2500.0, largeur: t.largeur, hauteur: t.hauteur, allege: 100.0, representation: None, valeurs: Default::default(), materiaux: Default::default(), retournee: false, inversee: false, origine: None }];
            p.verifier().unwrap();
            let scene = crate::rendu::scene::depuis_natif(&p, std::path::Path::new("test.modele.json")).unwrap();
            let id = crate::rendu::scene::id_natif("ouverture", &format!("o{k}"));
            // coupe verticale dans un vantail, coupe horizontale à mi-hauteur
            for plan in [Vec3::NEG_X.extend((2.3 - scene.origin.x) as f32), Vec3::NEG_Z.extend((0.8 - scene.origin.z) as f32)] {
                let prims: Vec<_> = super::coupe(&scene, &vec![true; scene.objects.len()], plan, 20.0, Poche::Hachure).into_iter().filter(|p| p.element == Some(id)).collect();
                // chaque pièce a son contour fermé
                assert!(prims.iter().all(|p| !matches!(p.forme, Forme::Polyligne { fermee: false, .. })), "{} : contour ouvert", t.nom);
                let pieces = prims.iter().filter(|p| p.role == Role::Poche).count();
                assert!(pieces >= 4, "{} : {pieces} pièces", t.nom);
                // vitrage : blanc, trait fin, sans hachure ; menuiserie hachurée
                let vitrages = prims.iter().filter(|p| p.role == Role::Poche && p.style.couleur == [255, 255, 255, 255]).count();
                assert!(vitrages >= 1 && vitrages < pieces, "{} : {vitrages} vitrages", t.nom);
                assert_eq!(prims.iter().filter(|p| p.role == Role::Coupe && p.style.epaisseur == TRAIT_FIN_MM).count(), vitrages);
                assert_eq!(prims.iter().filter(|p| p.role == Role::Hachure).count(), pieces - vitrages, "{}", t.nom);
            }
        }
    }

    #[test]
    fn isolant_tres_mince_et_long_n_explose_pas() {
        let s = super::ligne_isolation(&rect(0.0, 0.0, 30000.0, 1.0));
        assert!(s.is_empty());
    }
    use super::*;
    use noyau::aretes::aretes_jointes;

    /// Deux pavés du même matériau bout à bout : la jonction ne trace aucune arête ; en
    /// équerre, l'angle extérieur reste.
    #[test]
    fn jonction_sans_arete_meme_materiau() {
        let pave = |a: DVec3, b: DVec3| crate::modele::volumes::geometrie::tests::boite(a, b);
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
        use crate::rendu::scene::{Range, SceneObject};
        let mut s = crate::rendu::scene::projet_vide(&crate::modele::Projet::nouveau("Test"), std::path::Path::new("test.modele.json")).unwrap();
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
        let bande = |a: f32, b: f32, couleur| crate::rendu::scene::CoucheCoupe {
            plans: vec![Vec3::X.extend(-a),Vec3::NEG_X.extend(b)], couleur, fond: [255,255,255,255], motif_fond: None, motif: None, couleur_motif: [70,70,70,255], hachure_defaut: true, isolation: false, centre_isolation: None, trait_mm: TRAIT_FORT_MM,
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

    /// Deux murs de même type à un angle quelconque : la coupe en plan ne garde que le
    /// contour extérieur, aucun reste du trait d'onglet.
    #[test]
    fn angle_quelconque_sans_reste_d_onglet() {
        use crate::modele::{edition::ParametresMur, Projet};
        for deg in [90.0_f64, 70.0, 55.0, 35.0, 123.0] {
            let mut p = Projet::nouveau("A");
            let o = ParametresMur { epaisseur: 200.0, ..Default::default() };
            let r = deg.to_radians();
            let c = [5000.0 + 4000.0 * r.cos(), 4000.0 * r.sin()];
            p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &o).unwrap();
            p.ajouter_mur([5000.0, 0.0], c, &o).unwrap();
            let s = crate::rendu::scene::depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
            let visible = vec![true; s.objects.len()];
            let prims = crate::dessin::coupe(&s, &visible, glam::Vec4::new(0.0, 0.0, 1.0, -1.0), 100.0, crate::dessin::Poche::Hachure);
            let mut segs: Vec<[glam::DVec3; 2]> = Vec::new();
            for pr in prims.iter().filter(|p| p.role == noyau::trace2d::Role::Coupe) {
                if let noyau::trace2d::Forme::Segments(v) = &pr.forme { segs.extend(v.iter().copied()); }
            }
            // contour d'un angle plein : 6 côtés (18,4 m), sans reste du trait d'onglet
            let total: f64 = segs.iter().map(|s| s[0].distance(s[1])).sum();
            assert_eq!(segs.len(), 6, "{deg}° : {segs:?}");
            assert!((total - 18.4).abs() < 1e-3, "{deg}° : {total}");
        }
    }
}

#[cfg(test)]
mod test_isolant_arc {
    use crate::modele::*;
    use crate::modele::edition::ParametresMur;

    /// Mur courbe isolé passé par toute la chaîne (projet, scène, coupe) : extrémités non
    /// radiales comprises, le motif d'isolant couvre tout l'arc.
    #[test]
    fn isolant_d_un_mur_courbe_reel() {
        let mut p = Projet::nouveau("Arc");
        let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None, retour: false, ossature: None };
        p.types_murs.push(TypeCompose { id: "t".into(), nom: "Mur".into(), couches: vec![c("Enduit", 20.0, FonctionCouche::Finition), c("Isolant", 100.0, FonctionCouche::Isolation), c("Béton", 200.0, FonctionCouche::Structure)], categorie: None, couches_detaillees: true, origine: None });
        let o = ParametresMur { type_mur: Some("t".into()), ..Default::default() };
        p.ajouter_mur_axe(Axe::Arc { debut: [5000.0, 0.0], fin: [0.0, 5000.0], centre: [0.0, 0.0], direct: true }, &o).unwrap();
        let s = crate::rendu::scene::depuis_natif(&p, std::path::Path::new("t.json")).unwrap();
        let prims = super::coupe(&s, &vec![true; s.objects.len()], glam::Vec3::NEG_Z.extend(1.0), 100.0, super::Poche::Hachure);
        // la ligne d'isolant (série de boucles : la plus fournie des hachures) : elle doit couvrir l'arc
        // (corde de 5 m × √2 ≈ 6,9 m), pas un morceau droit
        let isolant = prims.iter().filter(|p| p.role == super::Role::Hachure).filter_map(|p| match &p.forme {
            super::Forme::Segments(v) => Some(v),
            _ => None,
        }).max_by_key(|v| v.len()).expect("ligne d'isolant");
        let pts: Vec<_> = isolant.iter().flatten().collect();
        let etendue = pts.iter().flat_map(|a| pts.iter().map(move |b| a.distance(**b))).fold(0.0, f64::max);
        assert!(etendue > 6.0, "motif d'isolant limité à {etendue:.2} m");
    }
}
