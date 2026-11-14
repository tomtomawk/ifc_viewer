//! Tests déplacés de `modele/elements/composant.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::modele::*;
use crate::edition::commandes::Commande;
use crate::modele::elements::composant::*;
use glam::DVec3;

fn projet() -> (Projet, String) {
    let p = Projet::nouveau("Composants").executer(Commande::Niveau { nom: "RDC".into(), altitude: 0.0 }).unwrap().0;
    let n = p.niveaux[0].id.clone();
    (p, n)
}

fn boite(p: &Projet, id: &str) -> (DVec3, DVec3) {
    let c = p.composants.iter().find(|c| c.id == id).unwrap();
    c.solides(p).unwrap().into_iter().flat_map(|(_, t)| t).flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), q| (a.min(q), b.max(q)))
}

#[test]
fn composant_place_transforme_copie_supprime() {
    let (p, n) = projet();
    let c = Composant { id: String::new(), nom: String::new(), type_composant: String::new(), niveau: n, position: [1000.0, 2000.0], decalage: 0.0, rotation: 0.0 };
    // type vide : le type par défaut est chargé
    let (p, ids) = p.executer(Commande::Composant { composant: c }).unwrap();
    assert_eq!(p.types_composants.len(), 1);
    let id = ids[0].clone();
    let (a, b) = boite(&p, &id);
    assert!((a - DVec3::new(200.0, 1600.0, 0.0)).length() < 1e-6 && (b - DVec3::new(1800.0, 2400.0, 750.0)).length() < 1e-6, "{a} {b}");
    // déplacer
    let (p, _) = p.executer(Commande::Deplacer { ids: vec![id.clone()], delta: [500.0, 0.0, 100.0] }).unwrap();
    let c = &p.composants[0];
    assert_eq!(c.position, [1500.0, 2000.0]);
    assert!((c.decalage - 100.0).abs() < 1e-9);
    // tourner de 90° autour de l'origine : position et orientation suivent
    let (p, _) = p.executer(Commande::Tourner { ids: vec![id.clone()], centre: [0.0, 0.0], angle: 90.0 }).unwrap();
    let c = &p.composants[0];
    assert!((c.position[0] + 2000.0).abs() < 1e-6 && (c.position[1] - 1500.0).abs() < 1e-6, "{:?}", c.position);
    assert!((c.rotation - 90.0).abs() < 1e-6);
    let (a, b) = boite(&p, &id);
    assert!((b.x - a.x - 800.0).abs() < 1e-6 && (b.y - a.y - 1600.0).abs() < 1e-6);
    // symétrie par rapport à l'axe Y
    let (p, _) = p.executer(Commande::Symetrie { ids: vec![id.clone()], a: [0.0, 0.0], b: [0.0, 1.0] }).unwrap();
    assert!((p.composants[0].position[0] - 2000.0).abs() < 1e-6);
    // copier puis supprimer la copie
    let (p, copies) = p.executer(Commande::Copier { ids: vec![id.clone()], delta: [0.0, 3000.0, 0.0] }).unwrap();
    assert_eq!(p.composants.len(), 2);
    assert!((p.composants[1].position[1] - p.composants[0].position[1] - 3000.0).abs() < 1e-6);
    let (p, _) = p.executer(Commande::Supprimer { ids: copies }).unwrap();
    assert_eq!(p.composants.len(), 1);
    // l'échelle est refusée ; le niveau utilisé ne se supprime pas
    assert!(p.executer(Commande::Echelle { ids: vec![id.clone()], centre: [0.0, 0.0], facteur: 2.0 }).is_err());
    assert!(p.executer(Commande::Supprimer { ids: vec![p.niveaux[0].id.clone()] }).is_err());
    assert!(!p.composants[0].symboles_plan(&p).is_empty());
}
