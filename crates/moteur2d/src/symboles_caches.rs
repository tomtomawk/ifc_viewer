//! Symboles des menuiseries en coupe et en élévation : ils sont dessinés à plat sur le
//! plan de la vue, par-dessus le modèle. Leurs traits sont découpés par la profondeur :
//! seules les parties qu'aucun objet visible ne masque, entre l'ouverture et le plan de
//! coupe, sont gardées (mur d'en face, arbre ou poteau devant une fenêtre…).
use glam::{DVec3, Vec3};
use modele::documentation::repere;
use modele::identifiants::id_natif;
use modele::{GenreVue, Ouverture, Projet, Vue};
use noyau::trace2d::{Forme, Primitive};
use scene::Scene;
use std::collections::HashSet;

/// Pas d'échantillonnage des traits (m).
const PAS: f64 = 0.02;
/// Marge autour de l'ouverture pour les objets candidats (m) : symboles qui débordent.
const MARGE: f64 = 0.3;

/// Ce qui peut masquer une ouverture : objets visibles entre elle et le plan de coupe.
struct Masque<'a> {
    s: &'a Scene,
    candidats: Vec<usize>,
    /// Vers l'observateur, unitaire.
    oeil: DVec3,
    /// Distance de l'ouverture au plan de coupe (m).
    profondeur: f64,
}

impl Masque<'_> {
    /// Ouverture d'une vue (coupe ou élévation) ; None si elle est devant la coupe.
    fn nouveau<'a>(p: &Projet, ouv: &Ouverture, s: &'a Scene, visible: &[bool], o: DVec3, oeil: DVec3) -> Option<Masque<'a>> {
        let (axe, _, niveau) = modele::volumes::solides::hote(p, &ouv.mur).ok()?;
        let z0 = p.niveau(&niveau)?.altitude + ouv.allege;
        let (c, _) = modele::volumes::solides::repere(&axe, ouv.abscisse);
        let profondeur = (o - DVec3::new(c.x, c.y, z0)).dot(oeil) * 0.001;
        if profondeur <= 0.0 { return None; }
        let hote = p.murs.iter().find(|m| m.id == ouv.mur).and_then(|m| m.assemblage.clone());
        let murs: Vec<&String> = match hote.as_deref().and_then(|a| p.assemblage(a)) {
            Some(a) => a.couches.iter().collect(),
            None => vec![&ouv.mur],
        };
        let exclus: HashSet<u32> = std::iter::once(id_natif("ouverture", &ouv.id)).chain(murs.iter().map(|m| id_natif("mur", m)))
            .filter_map(|id| s.object_of.get(&id).copied()).collect();
        // boîte balayée par l'ouverture jusqu'au plan de coupe (repère de la scène)
        let mut coins = Vec::new();
        for (a, b) in [(-0.5, 0.0), (0.5, 0.0), (-0.5, 1.0), (0.5, 1.0)] {
            let (q, _) = modele::volumes::solides::repere(&axe, ouv.abscisse + a * ouv.largeur);
            let q = DVec3::new(q.x, q.y, z0 + b * ouv.hauteur) * 0.001 - s.origin;
            coins.extend([q, q + oeil * profondeur]);
        }
        let (min, max) = coins.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
        let (min, max) = ((min - MARGE).as_vec3(), (max + MARGE).as_vec3());
        let candidats = s.objects.iter().enumerate()
            .filter(|(i, x)| visible.get(*i).copied().unwrap_or(false) && !exclus.contains(&(*i as u32)) && x.min.cmple(max).all() && x.max.cmpge(min).all())
            .map(|(i, _)| i).collect();
        Some(Masque { s, candidats, oeil, profondeur })
    }

    /// Vrai si le point du plan de vue (m, repère absolu) est masqué : le point de
    /// l'ouverture derrière lui est caché par un objet candidat.
    fn cache(&self, q: DVec3) -> bool {
        if self.candidats.is_empty() { return false; }
        let depart = (q - self.oeil * self.profondeur - self.s.origin).as_vec3();
        let dir = self.oeil.as_vec3();
        let t_max = self.profondeur as f32 - 1e-4;
        self.candidats.iter().any(|&i| {
            let o = &self.s.objects[i];
            if !rayon_boite(depart, dir, o.min, o.max, t_max) { return false; }
            [o.opaque, o.transparent].iter().any(|r| {
                self.s.vertices[r.start as usize..(r.start + r.count) as usize].as_chunks::<3>().0.iter()
                    .any(|t| rayon_triangle(depart, dir, Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos)).is_some_and(|d| d > 1e-4 && d < t_max))
            })
        })
    }

    /// Parties visibles du segment [a, b].
    fn decouper(&self, a: DVec3, b: DVec3, out: &mut Vec<[DVec3; 2]>) {
        let n = ((a.distance(b) / PAS).ceil() as usize).clamp(1, 400);
        let mut debut = None;
        for k in 0..n {
            let vu = !self.cache(a.lerp(b, (k as f64 + 0.5) / n as f64));
            match (vu, debut) {
                (true, None) => debut = Some(k as f64 / n as f64),
                (false, Some(t)) => { out.push([a.lerp(b, t), a.lerp(b, k as f64 / n as f64)]); debut = None; }
                _ => {}
            }
        }
        if let Some(t) = debut { out.push([a.lerp(b, t), b]); }
    }

    /// Primitive découpée ; None si rien ne se voit. Traits : parties visibles ; surfaces
    /// et textes : gardés entiers si le milieu de leur emprise se voit.
    fn primitive(&self, mut x: Primitive) -> Option<Primitive> {
        let segments: Vec<[DVec3; 2]> = match &x.forme {
            Forme::Polyligne { points, fermee } => {
                let n = if *fermee { points.len() } else { points.len().saturating_sub(1) };
                (0..n).map(|i| [points[i], points[(i + 1) % points.len()]]).collect()
            }
            Forme::Segments(s) => s.clone(),
            Forme::Surface(c) => return self.entier(x.clone(), c.iter().flatten()),
            Forme::Triangles(t) => return self.entier(x.clone(), t.iter().flatten()),
        };
        let mut out = Vec::new();
        for [a, b] in &segments { self.decouper(*a, *b, &mut out); }
        if out.is_empty() { return None; }
        // entièrement visible : forme d'origine (continuité des tirets)
        if out.len() == segments.len() && out.iter().zip(&segments).all(|(a, b)| a == b) { return Some(x); }
        x.forme = Forme::Segments(out);
        Some(x)
    }

    fn entier<'b>(&self, x: Primitive, points: impl Iterator<Item = &'b DVec3>) -> Option<Primitive> {
        let (min, max) = points.fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
        (min.x > max.x || !self.cache((min + max) / 2.0)).then_some(x)
    }
}

fn rayon_boite(o: Vec3, d: Vec3, min: Vec3, max: Vec3, t_max: f32) -> bool {
    let inv = d.recip();
    let (t1, t2) = ((min - o) * inv, (max - o) * inv);
    let (entree, sortie) = (t1.min(t2).max_element(), t1.max(t2).min_element());
    sortie >= entree.max(0.0) && entree <= t_max
}

/// Distance le long du rayon au triangle (Möller-Trumbore).
fn rayon_triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 { return None; }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) { return None; }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 { return None; }
    Some(e2.dot(q) * inv)
}

/// Découpe les symboles (primitives marquées de leur ouverture) par ce qui les masque,
/// dans une coupe ou une élévation ; les autres vues sont rendues telles quelles.
pub fn decouper_symboles_caches(p: &Projet, v: &Vue, s: &Scene, visible: &[bool], primitives: Vec<Primitive>) -> Vec<Primitive> {
    if !matches!(v.genre, GenreVue::Coupe { .. }) { return primitives; }
    let marques: HashSet<u64> = primitives.iter().filter_map(|x| x.element).collect();
    let (o, u, h) = repere(p, v);
    let oeil = u.cross(h);
    let masques: std::collections::HashMap<u64, Masque> = p.ouvertures.iter()
        .map(|x| (x, id_natif("ouverture", &x.id)))
        .filter(|(_, id)| marques.contains(id))
        .filter_map(|(x, id)| Some((id, Masque::nouveau(p, x, s, visible, o, oeil)?)))
        .collect();
    primitives.into_iter().filter_map(|x| match x.element.and_then(|e| masques.get(&e)) {
        Some(m) => m.primitive(x),
        None => Some(x),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn longueur(prims: &[Primitive], marques: &HashSet<u64>) -> f64 {
        prims.iter().filter(|x| x.element.is_some_and(|e| marques.contains(&e))).map(|x| match &x.forme {
            Forme::Segments(s) => s.iter().map(|[a, b]| a.distance(*b)).sum(),
            Forme::Polyligne { points, fermee } => points.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>() + if *fermee && points.len() > 1 { points[0].distance(points[points.len() - 1]) } else { 0.0 },
            _ => 0.0,
        }).sum()
    }

    /// Façades de la maison d'exemple : les symboles des menuiseries du mur d'en face
    /// (cachées par le mur vu) disparaissent ; ceux du mur vu restent.
    #[test]
    fn symboles_des_menuiseries_cachees_decoupes() {
        let p = Projet::lire_json(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_exemple.bem")).unwrap()).unwrap();
        let s = maillage::depuis_natif(&p, std::path::Path::new("essai.bem")).unwrap();
        let visible = vec![true; s.objects.len()];
        let mut total = (0.0, 0.0);
        for v in p.vues.iter().filter(|v| matches!(v.genre, GenreVue::Coupe { elevation: true, .. })) {
            let tout = modele::documentation::annotations(&p, v);
            let marques: HashSet<u64> = tout.iter().filter_map(|x| x.element).filter(|e| p.ouvertures.iter().any(|o| id_natif("ouverture", &o.id) == *e)).collect();
            let avant = longueur(&tout, &marques);
            let decoupes = decouper_symboles_caches(&p, v, &s, &visible, tout);
            let gardes: HashSet<u64> = decoupes.iter().filter_map(|x| x.element).filter(|e| marques.contains(e)).collect();
            let apres = longueur(&decoupes, &marques);
            // la menuiserie la plus proche de l'observateur garde son symbole
            let (o, u, h) = repere(&p, v);
            let oeil = u.cross(h);
            let profondeur = |e: &u64| { let x = p.ouvertures.iter().find(|x| id_natif("ouverture", &x.id) == *e).unwrap(); let (c, _) = modele::volumes::solides::repere(&modele::volumes::solides::hote(&p, &x.mur).unwrap().0, x.abscisse); (o - DVec3::new(c.x, c.y, o.z)).dot(oeil) };
            let plus_proche = marques.iter().map(profondeur).fold(f64::INFINITY, f64::min);
            println!("{} : {} marquées, {} gardées ; traits {avant:.1} → {apres:.1} m", v.nom, marques.len(), gardes.len());
            assert!(gardes.iter().any(|e| (profondeur(e) - plus_proche).abs() < 500.0), "{}", v.nom);
            assert!(apres > 0.0 && apres <= avant + 1e-9, "{}", v.nom);
            total = (total.0 + avant, total.1 + apres);
        }
        assert!(total.1 < 0.8 * total.0, "{total:?}");
    }

    /// Un poteau devant le milieu d'une fenêtre coupe ses traits en deux.
    #[test]
    fn poteau_devant_une_fenetre() {
        let mut p = Projet::lire_json(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_exemple.bem")).unwrap()).unwrap();
        let v = p.vues.iter().find(|v| v.nom == "Façade sud").unwrap().clone();
        let (o, u, h) = repere(&p, &v);
        let oeil = u.cross(h);
        let s = maillage::depuis_natif(&p, std::path::Path::new("essai.bem")).unwrap();
        let visible = vec![true; s.objects.len()];
        let tout = modele::documentation::annotations(&p, &v);
        let gardes: Vec<u64> = decouper_symboles_caches(&p, &v, &s, &visible, tout.clone()).iter().filter_map(|x| x.element).collect();
        // fenêtre vue la plus proche, poteau carré de 100 mm à 300 mm devant son milieu
        let ouv = p.ouvertures.iter().filter(|x| gardes.contains(&id_natif("ouverture", &x.id)))
            .min_by(|a, b| { let d = |x: &Ouverture| { let (c, _) = modele::volumes::solides::repere(&modele::volumes::solides::hote(&p, &x.mur).unwrap().0, x.abscisse); (o - DVec3::new(c.x, c.y, o.z)).dot(oeil) }; d(a).total_cmp(&d(b)) }).unwrap().clone();
        let (axe, e, niveau) = modele::volumes::solides::hote(&p, &ouv.mur).unwrap();
        let (c, _) = modele::volumes::solides::repere(&axe, ouv.abscisse);
        let pied = DVec3::new(c.x, c.y, 0.0) + oeil * (e / 2.0 + 300.0);
        p.barres.push(modele::Barre { id: "poteau-essai".into(), nom: "Poteau".into(), genre: modele::GenreBarre::Poteau, niveau, debut: pied.to_array(), fin: (pied + DVec3::Z * 3000.0).to_array(), largeur: 100.0, hauteur: 100.0, rotation: 0.0, pose: Default::default(), origine: None });
        let s2 = maillage::depuis_natif(&p, std::path::Path::new("essai.bem")).unwrap();
        let visible2 = vec![true; s2.objects.len()];
        let marque: HashSet<u64> = [id_natif("ouverture", &ouv.id)].into();
        let sans = longueur(&decouper_symboles_caches(&p, &v, &s, &visible, tout.clone()), &marque);
        let avec = longueur(&decouper_symboles_caches(&p, &v, &s2, &visible2, tout), &marque);
        println!("{} : {sans:.3} → {avec:.3} m", ouv.nom);
        assert!(avec > 0.5 * sans && avec < sans - 0.05, "{sans} {avec}");
    }
}
