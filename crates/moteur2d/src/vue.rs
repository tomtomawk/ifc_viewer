//! Sommets GPU des coupes, vue (arêtes vues et cachées), cadrage.

use super::*;

/// Sommets GPU des primitives d'une coupe de normale `normale` : triangles (poché,
/// contours) et traits (hachures), décalés vers l'observateur (côté retiré du plan) pour
/// passer devant les faces coupées ; marqués comme annotations, jamais découpés.
/// `fins` : affichage traits fins, tous les traits à un pixel ; `objet` : rang dans la
/// scène de l'élément d'une primitive (sélection).
pub fn sommets(objet: &dyn Fn(noyau::Id) -> Option<u32>, prims: &[Primitive], normale: DVec3, echelle: f64, fins: bool) -> (Vec<Vertex>, Vec<Vertex>) {
    let largeur = TRAIT_FORT_MM * echelle / 1000.0;
    let (mut tris, mut traits) = (Vec::new(), Vec::new());
    let raccords = (!fins).then(|| noyau::trace2d::Raccords::new(prims, echelle));
    for p in prims {
        let obj = p.element.and_then(objet).unwrap_or(u32::MAX);
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
    vu_limites(scene, visible, droite, haut, vers_oeil, coupe.as_slice(), resolution)
}

pub(super) fn vu_limites(scene: &Scene, visible: &[bool], droite: DVec3, haut: DVec3, vers_oeil: DVec3, coupes: &[Vec4], resolution: usize) -> Vec<Primitive> {
    let plans: Vec<_> = coupes.iter().map(|c| (c.truncate().as_dvec3(), c.w as f64)).collect();
    let proj = |p: DVec3| DVec3::new(p.dot(droite), p.dot(haut), p.dot(vers_oeil));
    let objets: Vec<usize> = (0..scene.objects.len()).filter(|&i| visible[i] && &*scene.objects[i].class != "IFCANNOTATION").collect();
    let pos = |v: &Vertex| Vec3::from(v.pos).as_dvec3();
    let (premier_plan, suivants) = plans.split_first().map_or((None, &[][..]), |(p, s)| (Some(*p), s));
    // triangles projetés, découpés par la coupe
    let mut tris: Vec<[DVec3; 3]> = Vec::new();
    for &i in &objets {
        let ob = &scene.objects[i];
        for r in [ob.opaque, ob.transparent] {
            for t in scene.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3) {
                let p = [pos(&t[0]), pos(&t[1]), pos(&t[2])];
                let mut poly = premier_plan.map_or_else(|| p.to_vec(), |(n, d)| decouper(&p, n, d));
                for &(n, d) in suivants {
                    if poly.len() < 3 { break; }
                    poly = decouper(&poly, n, d);
                }
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
        'arete: for v in scene.vertices[ob.aretes.start as usize..(ob.aretes.start + ob.aretes.count) as usize].chunks_exact(2) {
            if v[0].normal[3] < 0 {
                // arête douce : tracée sur la silhouette seulement (voir render.rs)
                let n1 = DVec3::new(v[0].normal[0] as f64, v[0].normal[1] as f64, v[0].normal[2] as f64) / 127.0;
                let n2 = DVec3::new(v[0].color[0] as f64, v[0].color[1] as f64, v[0].color[2] as f64) / 255.0 * 2.0 - DVec3::ONE;
                if n1.dot(vers_oeil) * n2.dot(vers_oeil) > 0.0 {
                    continue;
                }
            }
            let (mut a, mut b) = (pos(&v[0]), pos(&v[1]));
            for &(n, d) in &plans {
                let (sa, sb) = (n.dot(a) + d, n.dot(b) + d);
                if sa < 0.0 && sb < 0.0 {
                    continue 'arete;
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
