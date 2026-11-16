//! Tests déplacés de `modele/elements/site.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use commandes::Execution;
use modele::elements::site::*;

#[test]
fn site_en_phase_existant_cotes_ngf_et_epaisseur_du_type() {
    use {commandes::Commande, modele::{Projet, PHASE_EXISTANT}};
    let p = Projet::nouveau("Site");
    assert_eq!(p.phases, [PHASE_EXISTANT, "Projet"]);
    // cotes NGF autour de 45 m ; origine du projet à 45,00 m NGF
    let terrain = Terrain { id: String::new(), nom: "Terrain".into(), contour: vec![[0.,0.,45_000.], [4000.,0.,45_500.], [4000.,4000.,46_000.], [0.,4000.,45_000.]], points: vec![[2000.,2000.,46_200.]], type_terrain: None };
    let (p, t) = p.executer(Commande::Terrain { terrain }).unwrap();
    let limite = LimitePropriete { id: String::new(), nom: "Parcelle".into(), altitude: 0., contour: vec![[0.,0.], [4000.,0.], [4000.,4000.], [0.,4000.]] };
    let (p, l) = p.executer(Commande::LimitePropriete { limite }).unwrap();
    for id in [&t[0], &l[0]] {
        assert_eq!(p.contextes[id].phase.as_deref(), Some(PHASE_EXISTANT));
    }
    let (p, _) = p.executer(Commande::CoordonneesOrigine { coordonnees: [0., 0., 45_000.] }).unwrap();
    let hauteur = |p: &Projet| {
        let s = maillage::depuis_natif(p, std::path::Path::new("site.json")).unwrap();
        let o = &s.objects[s.object_of[&modele::identifiants::id_natif("terrain", &t[0])] as usize];
        ((o.min.z as f64 + s.origin.z) * 1000.0, (o.max.z as f64 + s.origin.z) * 1000.0)
    };
    // dessus à 1,2 m au plus au-dessus du niveau 0, épaisseur du type par défaut dessous
    let (bas, haut) = hauteur(&p);
    assert!((haut - 1200.0).abs() < 1.0 && (bas - (0.0 - TypeTerrain::defaut().epaisseur)).abs() < 1.0, "{bas} {haut}");
    // remonter le projet de 1 m : le terrain descend d'autant dans le repère du projet
    let (p, _) = p.executer(Commande::CoordonneesOrigine { coordonnees: [0., 0., 46_000.] }).unwrap();
    assert!((hauteur(&p).1 - 200.0).abs() < 1.0);
    // type plus épais : le dessous suit
    let mut q = p.clone();
    q.types_terrains.push(TypeTerrain { id: "remblai".into(), nom: "Remblai".into(), epaisseur: 3000.0, materiau: None, couches: Vec::new() });
    q.terrains[0].type_terrain = Some("remblai".into());
    q.verifier().unwrap();
    assert!((hauteur(&q).0 - (-1000.0 - 3000.0)).abs() < 1.0);
    q.terrains[0].type_terrain = Some("absent".into());
    assert!(q.verifier().is_err());
    // relu tel quel ; suppression : contextes retirés
    let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(relu.coordonnees_origine, [0., 0., 46_000.]);
    let (p, _) = p.executer(Commande::Supprimer { ids: vec![t[0].clone(), l[0].clone()] }).unwrap();
    assert!(p.contextes.is_empty());
}
