//! Tests déplacés de `modele/elements/escalier.rs` (`mod tests`) : ils appellent du code situé au-dessus
//! de la crate `modele` (commandes, scène, formats, échanges, collaboration ou dessin).
use crate::edition::Execution;
use crate::modele::*;
use crate::modele::elements::escalier::*;

fn projet() -> Projet {
    let mut p = Projet::nouveau("Escalier");
    p.niveaux.push(Niveau { id: "rdc".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
    p.niveaux.push(Niveau { id: "r1".into(), nom: "R+1".into(), altitude: 2800.0, origine: None });
    p
}

fn escalier(trace: Vec<[f64; 2]>) -> Escalier {
    Escalier { id: "e".into(), nom: "Escalier".into(), niveau: "rdc".into(), niveau_haut: Some("r1".into()), decalage_bas: 0.0, decalage_haut: 0.0, hauteur: 2800.0, trace, largeur: 1000.0, hauteur_marche: 175.0, paillasse: 150.0 }
}

#[test]
fn commandes_escalier_et_garde_corps() {
    use crate::edition::commandes::Commande;
    let p = projet();
    let e = Escalier { id: String::new(), ..escalier(vec![[0.0, 0.0], [4050.0, 0.0]]) };
    let (p, ids) = p.executer(Commande::Escalier { escalier: e.clone() }).unwrap();
    let id = ids[0].clone();
    let [g, _] = p.escaliers[0].traces_garde_corps(50.0);
    let gc = GardeCorps { id: String::new(), nom: "GC".into(), niveau: "rdc".into(), decalage: 0.0, trace: g, hauteur: 1000.0, remplissage: Remplissage::Barreaudage, escalier: Some(id.clone()), type_garde_corps: None };
    let (p, _) = p.executer(Commande::GardeCorps { garde_corps: gc }).unwrap();
    // copie : le garde-corps suit l'escalier copié
    let (q, copies) = p.executer(Commande::Copier { ids: vec![id.clone()], delta: [0.0, 3000.0, 0.0] }).unwrap();
    assert_eq!((q.escaliers.len(), q.garde_corps.len()), (2, 2));
    assert!(q.garde_corps.iter().any(|g| g.escalier.as_ref() == Some(&copies[0])));
    // suppression : les garde-corps hébergés partent avec l'escalier
    let (q, _) = p.executer(Commande::Supprimer { ids: vec![id] }).unwrap();
    assert!(q.escaliers.is_empty() && q.garde_corps.is_empty());
    // escalier impossible refusé
    assert!(p.executer(Commande::Escalier { escalier: Escalier { trace: vec![[0.0, 0.0]], ..e } }).is_err());
    // export IFC : classes d'escalier et de garde-corps
    let ifc = crate::echanges::export_ifc::exporter(&p).unwrap();
    assert!(ifc.contains("IFCSTAIR(") && ifc.contains("IFCRAILING("));
}
