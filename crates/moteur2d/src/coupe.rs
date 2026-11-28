//! Coupe par un plan : poché, hachures, contours en trait fort.

use super::*;
use noyau::Id;
use std::sync::Arc;

/// Coupe des objets visibles par le plan `plan` (n, d ; repère de la scène, n·p + d ≥ 0
/// gardé) : poché, hachures et contours en trait fort, en primitives 2D dans le plan
/// (repère de la scène). `echelle` : dénominateur de l'échelle du dessin (pas des
/// hachures).
pub fn coupe(scene: &Scene, visible: &[bool], affichage: Affichage, plan: Vec4, echelle: f64, poche: Poche) -> Vec<Primitive> {
    coupe_avec_cache(scene, visible, affichage, plan, echelle, poche, &mut CacheCoupe::default())
}

/// Primitives de coupe de chaque objet, gardées d'une coupe à la suivante : un objet dont
/// l'empreinte (géométrie, couleurs, masques, bandes de matériau) n'a pas changé n'est
/// pas recoupé tant que le plan, l'échelle, le poché et l'affichage restent les mêmes.
#[derive(Default)]
pub struct CacheCoupe {
    cle: u64,
    objets: std::collections::HashMap<Id, (u64, Arc<Vec<Primitive>>)>,
    /// Objets recoupés lors du dernier appel (les autres sont repris).
    pub recoupes: usize,
}

/// Coupe verticale d'une couche de mur droit (axe et épaisseur en mètres, repère du
/// modèle) : abscisse dans le plan (repère `o`, `e1`) où l'axe le traverse, et largeur
/// apparente de la couche (épaisseur / cosinus de l'angle entre la normale de la coupe
/// et le mur). None si la coupe est presque parallèle au mur (à moins de 15°).
fn axe_isolation_verticale((axe, e): ([[f64; 2]; 2], f64), origine: DVec3, o: DVec3, n: DVec3, e1: DVec3) -> Option<(f64, f64)> {
    let a = DVec3::new(axe[0][0] - origine.x, axe[0][1] - origine.y, o.z);
    let d = DVec3::new(axe[1][0] - axe[0][0], axe[1][1] - axe[0][1], 0.0).try_normalize()?;
    // la ligne de coupe (horizontale, normale à n) traverse la bande de la couche sur
    // e / |n · d| ; coupe presque parallèle au mur (|n · d| petit) : pas de ce calcul
    let cos = n.dot(d).abs();
    if cos < 0.26 {
        return None;
    }
    // point de l'axe dans le plan : (a + t d - o) · n = 0
    let t = (o - a).dot(n) / n.dot(d);
    let pied = (a + d * t - o).dot(e1);
    Some((pied, e / cos))
}

/// `coupe`, en reprenant du cache les objets inchangés (après une édition, seuls les
/// objets modifiés sont recoupés).
pub fn coupe_avec_cache(scene: &Scene, visible: &[bool], affichage: Affichage, plan: Vec4, echelle: f64, poche: Poche, cache: &mut CacheCoupe) -> Vec<Primitive> {
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
    let cle = {
        let mut h = Empreinte::default();
        for x in [plan.x, plan.y, plan.z, plan.w] { h.mot(x.to_bits() as u64); }
        h.mot(echelle.to_bits());
        for x in scene.origin.to_array() { h.mot(x.to_bits()); }
        h.texte(&format!("{poche:?} {affichage:?}"));
        h.0
    };
    if cache.cle != cle {
        cache.objets.clear();
        cache.cle = cle;
    }
    let objets = &cache.objets;
    let recoupes = std::sync::atomic::AtomicUsize::new(0);
    let resultats: Vec<(Id, u64, Arc<Vec<Primitive>>)> = (0..scene.objects.len())
        .into_par_iter()
        .filter_map(|i| {
            let ob = &scene.objects[i];
            if !visible[i] || &*ob.class == "IFCANNOTATION" {
                return None;
            }
            // objet inchangé depuis la coupe précédente (même plan) : primitives reprises
            let e = empreinte_objet(scene, i);
            if let Some((h, p)) = objets.get(&ob.id) && *h == e {
                return Some((ob.id, e, p.clone()));
            }
            recoupes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Some((ob.id, e, Arc::new(coupe_objet(scene, i, affichage, n, d, o, e1, e2, poche, fond, hachure, &motif, echelle))))
        })
        .collect();
    let mut t = web_time::Instant::now();
    noyau::chrono::chrono_lent("coupe : empreintes et objets", &mut t);
    let prims: Vec<Primitive> = resultats.iter().flat_map(|r| r.2.iter().cloned()).collect();
    cache.objets = resultats.into_iter().map(|(id, e, p)| (id, (e, p))).collect();
    cache.recoupes = recoupes.into_inner();
    noyau::chrono::chrono_lent("coupe : copie des primitives", &mut t);
    let r = fusionner_materiaux(scene, prims);
    noyau::chrono::chrono_lent("coupe : fusion des matériaux", &mut t);
    r
}

/// Coupe d'un objet par le plan (n, d) : poché, hachures, contours.
#[allow(clippy::too_many_arguments)]
fn coupe_objet(scene: &Scene, i: usize, affichage: Affichage, n: DVec3, d: f64, o: DVec3, e1: DVec3, e2: DVec3, poche: Poche, fond: [u8; 4], hachure: Option<[u8; 4]>, motif: &Motif, echelle: f64) -> Vec<Primitive> {
    let ob = &scene.objects[i];
    let mut prims = Vec::new();
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
        return coupe_par_solides(scene, ob, affichage, n, d, o, e1, e2, poche, fond, hachure, motif, echelle);
    }
    let mut segs = Vec::new();
    for r in [ob.opaque, ob.transparent] {
        for t in scene.primitives_montrees(r, 3, affichage) {
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
    if !boucles.is_empty()
        && let Some(couches) = scene.couches_coupe.get(&ob.id) {
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
                    // vue en plan d'un mur droit : le long de l'axe connu de la couche
                    let plan = |p: [f64; 2]| { let q = DVec3::new(p[0] - scene.origin.x, p[1] - scene.origin.y, o.z); [(q-o).dot(e1)*1000.0,(q-o).dot(e2)*1000.0] };
                    let segments = if let Some(([a, b], e)) = couche.axe_isolation.filter(|_| n.z.abs() > 0.999) {
                        motifs::ligne_isolation_axe(&t2, plan(a), plan(b), e * 1000.0)
                    } else if let Some((pied, e)) = couche.axe_isolation.filter(|_| n.z.abs() < 1e-3).and_then(|axe| axe_isolation_verticale(axe, scene.origin, o, n, e1)) {
                        // coupe verticale d'un mur droit : bande verticale centrée là où l'axe
                        // de la couche traverse le plan, largeur apparente dans la coupe
                        motifs::ligne_isolation_axe(&t2, [pied * 1000.0, 0.0], [pied * 1000.0, 1000.0], e * 1000.0)
                    } else if let Some([x, y]) = couche.centre_isolation.filter(|_| n.z.abs() > 0.999) {
                        let centre = DVec3::new(x - scene.origin.x, y - scene.origin.y, o.z);
                        motifs::ligne_isolation_courbe(&t2, [(centre-o).dot(e1)*1000.0,(centre-o).dot(e2)*1000.0])
                    } else { ligne_isolation(&t2) };
                    let h = segments.into_iter().map(|s| s.map(|p| en_3d([p[0] / 1000.0, p[1] / 1000.0]))).collect();
                    detail.push(primitive(Forme::Segments(h),couche.couleur_motif,0.0,Role::Hachure));
                }
                if poche == Poche::Hachure && !uni && !couche.isolation {
                    let (motif, facteur) = match &couche.motif {
                        Some(m) => (m.as_ref(), if m.modele { 0.001 } else { echelle / 1000.0 }),
                        None if couche.hachure_defaut => (motif, 1.0),
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
        let s: Vec<[DVec3; 2]> = hachures_plan(&t2, motif, 1.0, 20_000).into_iter().map(|s| s.map(en_3d)).collect();
        if !s.is_empty() {
            prims.push(primitive(Forme::Segments(s), c, 0.0, Role::Hachure));
        }
    }
    for (pts, fermee) in boucles.iter().map(|b| (b, true)).chain(ouvertes.iter().map(|b| (b, false))) {
        prims.push(primitive(Forme::Polyligne { points: pts.iter().map(|p| en_3d(*p)).collect(), fermee }, NOIR, TRAIT_FORT_MM, Role::Coupe));
    }
    prims
}

/// Coupe d'un élément sans bandes de matériau, solide par solide : les triangles sont
/// regroupés par arêtes communes (une arête partagée par deux triangles exactement),
/// chaque groupe donne ses propres contours, son poché et ses hachures. Un solide
/// translucide (vitrage) reste blanc, sans hachure, au trait fin.
#[allow(clippy::too_many_arguments)]
fn coupe_par_solides(scene: &Scene, ob: &scene::SceneObject, affichage: Affichage, n: DVec3, d: f64, o: DVec3, e1: DVec3, e2: DVec3, poche: Poche, fond: [u8; 4], hachure: Option<[u8; 4]>, motif: &Motif, echelle: f64) -> Vec<Primitive> {
    let cle = |p: [f32; 3]| p.map(|x| (x as f64 * 1e4).round() as i64);
    let mut tris: Vec<(usize, bool)> = Vec::new();
    for (r, translucide) in [(ob.opaque, false), (ob.transparent, true)] {
        tris.extend((0..r.count as usize / 3).map(|k| (r.start as usize + 3 * k, translucide)).filter(|(k, _)| scene.montre(*k, affichage)));
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
    /// Lot de section : segments 2D, premier sommet du solide (ordre des lots) et translucidité.
    type Lot = (Vec<[[f64; 2]; 2]>, usize, bool);
    let mut lots: HashMap<usize, Lot> = HashMap::new();
    for (i, &(k, translucide)) in tris.iter().enumerate() {
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
        // contour coupé : plume de la catégorie (styles d'objets), sinon trait fort
        let trait_mm = if translucide { TRAIT_FIN_MM } else { scene.traits_coupe.get(&ob.id).map(|t| t.epaisseur).filter(|e| *e > 0.0).unwrap_or(TRAIT_FORT_MM) };
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
    let mut t_fusion = web_time::Instant::now();
    // matériau d'un élément : sa bande de coupe unique (couches des murs natifs)
    let materiau = |id: Option<noyau::Id>| -> Option<([u8; 4], String)> {
        let c = scene.couches_coupe.get(&id?)?;
        let [b] = c.as_slice() else { return None };
        Some((b.couleur, b.motif.as_ref().map_or_else(String::new, |m| m.nom.clone())))
    };
    // traits de coupe rangés par droite porteuse : (primitive, rang, t0, t1, élément).
    // Une droite regroupe les traits dont les deux extrémités sont à moins de 0,1 mm d'elle,
    // quel que soit leur sens : une clé arrondie sépare à tort un trait court (direction
    // bruitée) ou un onglet à 45° (signe de la direction canonique instable) de sa droite.
    type Trait = (usize, usize, f64, f64, Option<noyau::Id>);
    const TOLERANCE: f64 = 1e-4;
    let case = |u: DVec3| [(u.x * 20.0).floor() as i64, (u.y * 20.0).floor() as i64, (u.z * 20.0).floor() as i64];
    let mut droites: Vec<(DVec3, DVec3, Vec<Trait>)> = Vec::new();
    // index : par case de direction, familles de directions identiques (à 1,4e-6 rad
    // près) ; dans une famille, droites rangées par décalage perpendiculaire (cases de
    // 1 mm). Sans lui, chaque trait parcourait toutes les droites de sa direction : coût
    // quadratique sur un bâtiment dont presque tous les murs sont en X ou en Y.
    struct Famille { v: DVec3, par_decalage: HashMap<[i64; 3], Vec<usize>> }
    let decalage = |q: DVec3, v: DVec3| { let r = (q - v * q.dot(v)) * 1000.0; [r.x.floor() as i64, r.y.floor() as i64, r.z.floor() as i64] };
    let mut index: HashMap<[i64; 3], Vec<Famille>> = HashMap::new();
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
                // droite existante : familles des cases voisines de u et de −u, droites
                // de décalage voisin ; même critère qu'avant (les deux extrémités à moins
                // de 0,1 mm de la droite)
                let mut trouvee = None;
                'cherche: for signe in [1.0, -1.0] {
                    let c = case(u * signe);
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                let Some(familles) = index.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) else { continue };
                                for f in familles {
                                    let k = decalage(s[0], f.v);
                                    for ex in -1..=1 { for ey in -1..=1 { for ez in -1..=1 {
                                        let Some(liste) = f.par_decalage.get(&[k[0] + ex, k[1] + ey, k[2] + ez]) else { continue };
                                        for &n in liste {
                                            let (v, o, _) = droites[n];
                                            if distance(s[0] - o, v) < TOLERANCE && distance(s[1] - o, v) < TOLERANCE {
                                                trouvee = Some(n);
                                                break 'cherche;
                                            }
                                        }
                                    } } }
                                }
                            }
                        }
                    }
                }
                let n = trouvee.unwrap_or_else(|| {
                    droites.push((u, s[0], Vec::new()));
                    let n = droites.len() - 1;
                    let familles = index.entry(case(u)).or_default();
                    let i = match familles.iter().position(|f| f.v.dot(u) > 1.0 - 1e-12) {
                        Some(i) => i,
                        None => { familles.push(Famille { v: u, par_decalage: HashMap::new() }); familles.len() - 1 }
                    };
                    let f = &mut familles[i];
                    f.par_decalage.entry(decalage(s[0], f.v)).or_default().push(n);
                    n
                });
                let v = droites[n].0;
                let (ta, tb) = (s[0].dot(v), s[1].dot(v));
                droites[n].2.push((k, i, ta.min(tb), ta.max(tb), p.element));
            }
        }
    }
    // partie de chaque trait recouverte par un trait d'un autre élément du même matériau :
    // retirée des deux (un trait court posé sur un trait long compris)
    noyau::chrono::chrono_lent(&format!("fusion : index ({} droites)", droites.len()), &mut t_fusion);
    let mut remplacer: HashMap<(usize, usize), Vec<[DVec3; 2]>> = HashMap::new();
    // matériau de chaque élément calculé une fois (rang dans la liste des matériaux vus)
    let mut rangs: HashMap<Option<noyau::Id>, Option<usize>> = HashMap::new();
    let mut vus: Vec<([u8; 4], String)> = Vec::new();
    let mut rang = |e: Option<noyau::Id>| -> Option<usize> {
        *rangs.entry(e).or_insert_with(|| {
            let m = materiau(e)?;
            Some(vus.iter().position(|x| *x == m).unwrap_or_else(|| { vus.push(m); vus.len() - 1 }))
        })
    };
    for (u, _, traits) in &mut droites {
        // rangés par début : les traits qui commencent après la fin d'un trait ne le
        // recouvrent pas (arrêt du parcours)
        traits.sort_by(|a, b| a.2.total_cmp(&b.2));
        let mats: Vec<Option<usize>> = traits.iter().map(|t| rang(t.4)).collect();
        for (j, &(k, i, t0, t1, e)) in traits.iter().enumerate() {
            let mut couverts: Vec<(f64, f64)> = traits
                .iter()
                .zip(&mats)
                .take_while(|(o, _)| o.2 < t1 - 1e-6)
                .filter(|(o, m)| o.4 != e && **m == mats[j] && o.3 > t0 + 1e-6)
                .map(|(o, _)| (o.2.max(t0), o.3.min(t1)))
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
    noyau::chrono::chrono_lent(&format!("fusion : recouvrements ({})", remplacer.len()), &mut t_fusion);
    if remplacer.is_empty() {
        return prims;
    }
    // seules les primitives dont un trait change sont parcourues (les hachures en ont
    // des centaines de milliers)
    let touchees: std::collections::BTreeSet<usize> = remplacer.keys().map(|c| c.0).collect();
    for k in touchees {
        if let Forme::Segments(segs) = &mut prims[k].forme {
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

/// Empreinte rapide (FNV sur des mots de 64 bits).
struct Empreinte(u64);
impl Default for Empreinte {
    fn default() -> Self { Empreinte(0xcbf2_9ce4_8422_2325) }
}
impl Empreinte {
    fn mot(&mut self, w: u64) { self.0 = (self.0 ^ w).wrapping_mul(0x0100_0000_01b3); }
    fn texte(&mut self, t: &str) { for b in t.bytes() { self.mot(b as u64); } }
    fn motif(&mut self, m: Option<&Motif>) {
        let Some(m) = m else { return self.mot(0) };
        self.texte(&m.nom);
        self.mot(m.modele as u64);
        for g in &m.grilles {
            for x in [g.angle, g.origine[0], g.origine[1], g.glissement, g.decalage] { self.mot(x.to_bits()); }
            for x in &g.tirets { self.mot(x.to_bits()); }
        }
    }
}

/// Empreinte de ce qui décide de la coupe de l'objet `i` : identifiant, classe, sommets
/// (positions, normales, couleurs, couleurs de rendu, masques ; pas l'indice d'objet, qui
/// change quand on en ajoute un), bandes de matériau.
fn empreinte_objet(scene: &Scene, i: usize) -> u64 {
    let ob = &scene.objects[i];
    let mut h = Empreinte::default();
    h.mot(ob.id);
    h.texte(&ob.class);
    for r in [ob.opaque, ob.transparent] {
        h.mot(r.count as u64);
        for k in r.start as usize..(r.start + r.count) as usize {
            let v = &scene.vertices[k];
            h.mot((v.pos[0].to_bits() as u64) << 32 | v.pos[1].to_bits() as u64);
            h.mot((v.pos[2].to_bits() as u64) << 32 | u32::from_le_bytes(v.normal.map(|x| x as u8)) as u64);
            h.mot((u32::from_le_bytes(v.color) as u64) << 32 | scene.couleurs_rendu.get(k).map_or(0, |c| u32::from_le_bytes(*c)) as u64);
            if let Some(m) = scene.visibilite.get(k) { h.mot(*m as u64); }
        }
    }
    if let Some(t) = scene.traits_coupe.get(&ob.id) { h.mot(t.epaisseur.to_bits()); }
    if let Some(couches) = scene.couches_coupe.get(&ob.id) {
        for c in couches {
            for p in &c.plans { for x in p.to_array() { h.mot(x.to_bits() as u64); } }
            h.mot(u32::from_le_bytes(c.couleur) as u64);
            h.mot(u32::from_le_bytes(c.fond) as u64);
            h.motif(c.motif_fond.as_deref());
            h.mot(c.isolation as u64);
            if let Some([x, y]) = c.centre_isolation { h.mot(x.to_bits()); h.mot(y.to_bits()); }
            if let Some((axe, e)) = c.axe_isolation { for x in axe.iter().flatten().chain([&e]) { h.mot(x.to_bits()); } }
            h.mot(c.trait_mm.to_bits());
            h.motif(c.motif.as_deref());
            h.mot(u32::from_le_bytes(c.couleur_motif) as u64);
            h.mot(c.hachure_defaut as u64);
        }
    }
    h.0
}
