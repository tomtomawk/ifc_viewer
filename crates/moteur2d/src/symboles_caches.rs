//! Symboles des menuiseries en coupe et en élévation : ils sont dessinés à plat sur le
//! plan de la vue, par-dessus le modèle. Ceux d'une ouverture cachée (mur d'en face,
//! autre élément devant elle entre la coupe et l'ouverture) sont retirés.
use glam::DVec3;
use modele::documentation::repere;
use modele::identifiants::id_natif;
use modele::{GenreVue, Ouverture, Projet, Vue};
use noyau::trace2d::Primitive;
use scene::Scene;
use std::collections::HashSet;

/// Points de l'ouverture testés : (fraction de la largeur, fraction de la hauteur).
const ECHANTILLONS: [(f64, f64); 5] = [(0.0, 0.5), (-0.3, 0.2), (0.3, 0.2), (-0.3, 0.8), (0.3, 0.8)];

/// Vrai si l'ouverture se voit depuis la vue (`oeil` : vers l'observateur ; `o` : point
/// du plan de coupe, mm) : au moins deux points sur cinq ne sont masqués par aucun objet
/// visible autre que l'ouverture et son mur, entre l'ouverture et le plan de coupe.
pub fn ouverture_visible(p: &Projet, ouv: &Ouverture, s: &Scene, visible: &[bool], o: DVec3, oeil: DVec3) -> bool {
    let Ok((axe, _, niveau)) = modele::volumes::solides::hote(p, &ouv.mur) else { return true };
    let Some(z0) = p.niveau(&niveau).map(|n| n.altitude + ouv.allege) else { return true };
    let hote = p.murs.iter().find(|m| m.id == ouv.mur).and_then(|m| m.assemblage.clone());
    let murs: Vec<&String> = match hote.as_deref().and_then(|a| p.assemblage(a)) {
        Some(a) => a.couches.iter().collect(),
        None => vec![&ouv.mur],
    };
    let exclus: HashSet<u32> = std::iter::once(id_natif("ouverture", &ouv.id)).chain(murs.iter().map(|m| id_natif("mur", m)))
        .filter_map(|id| s.object_of.get(&id).copied()).collect();
    let mut vus = 0;
    for (a, b) in ECHANTILLONS {
        let (c, _) = modele::volumes::solides::repere(&axe, ouv.abscisse + a * ouv.largeur);
        let q = DVec3::new(c.x, c.y, z0 + b * ouv.hauteur);
        // distance au plan de coupe (m) ; un point devant la coupe ne se voit pas
        let t_max = (o - q).dot(oeil) * 0.001;
        if t_max <= 0.0 { continue; }
        let depart = (q * 0.001 - s.origin).as_vec3();
        let masque = s.pick_tous(depart, oeil.as_vec3(), visible, None, None).into_iter()
            .any(|(i, d)| !exclus.contains(&i) && d > 1e-4 && (d as f64) < t_max - 1e-4);
        if !masque { vus += 1; }
    }
    vus >= 2
}

/// Retire les symboles (primitives marquées de leur ouverture) des ouvertures cachées,
/// dans une coupe ou une élévation ; les autres vues sont rendues telles quelles.
pub fn retirer_symboles_caches(p: &Projet, v: &Vue, s: &Scene, visible: &[bool], primitives: Vec<Primitive>) -> Vec<Primitive> {
    if !matches!(v.genre, GenreVue::Coupe { .. }) { return primitives; }
    let marques: HashSet<u64> = primitives.iter().filter_map(|x| x.element).collect();
    let (o, u, h) = repere(p, v);
    let oeil = u.cross(h);
    let caches: HashSet<u64> = p.ouvertures.iter()
        .map(|x| (x, id_natif("ouverture", &x.id)))
        .filter(|(x, id)| marques.contains(id) && !ouverture_visible(p, x, s, visible, o, oeil))
        .map(|(_, id)| id).collect();
    if caches.is_empty() { return primitives; }
    primitives.into_iter().filter(|x| !x.element.is_some_and(|e| caches.contains(&e))).collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Façades de la maison d'exemple : les menuiseries du mur d'en face (cachées par le
    /// mur vu) perdent leurs symboles ; celles du mur vu les gardent.
    #[test]
    fn symboles_des_menuiseries_cachees_retires() {
        let p = Projet::lire_json(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/maison_exemple.bem")).unwrap()).unwrap();
        let s = maillage::depuis_natif(&p, std::path::Path::new("essai.bem")).unwrap();
        let visible = vec![true; s.objects.len()];
        let mut total = (0, 0);
        for v in p.vues.iter().filter(|v| matches!(v.genre, GenreVue::Coupe { elevation: true, .. })) {
            let tout = modele::documentation::annotations(&p, v);
            let marques: HashSet<u64> = tout.iter().filter_map(|x| x.element).filter(|e| p.ouvertures.iter().any(|o| id_natif("ouverture", &o.id) == *e)).collect();
            let gardes: HashSet<u64> = retirer_symboles_caches(&p, v, &s, &visible, tout).iter().filter_map(|x| x.element).filter(|e| marques.contains(e)).collect();
            // une ouverture gardée est sur la face la plus proche de l'observateur de son orientation
            let (o, u, h) = repere(&p, v);
            let oeil = u.cross(h);
            let profondeur = |e: &u64| { let x = p.ouvertures.iter().find(|x| id_natif("ouverture", &x.id) == *e).unwrap(); let (c, _) = modele::volumes::solides::repere(&modele::volumes::solides::hote(&p, &x.mur).unwrap().0, x.abscisse); (o - DVec3::new(c.x, c.y, o.z)).dot(oeil) };
            let plus_proche = marques.iter().map(profondeur).fold(f64::INFINITY, f64::min);
            println!("{} : {} marquées, {} gardées", v.nom, marques.len(), gardes.len());
            assert!(!gardes.is_empty(), "{}", v.nom);
            assert!(gardes.iter().any(|e| (profondeur(e) - plus_proche).abs() < 500.0), "{}", v.nom);
            total = (total.0 + marques.len(), total.1 + gardes.len());
        }
        assert!(total.1 < total.0, "{total:?}");
    }
}
