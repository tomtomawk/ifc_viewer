use super::*;
use crate::creation::{Implantation, ParametresMur};

/// Projet à un mur droit de 4 m × 2,7 m, type à une couche ; rend (projet, mur).
fn projet(composite: bool) -> (Projet, String) {
    let mut p = Projet::depuis_gabarit("Essai", &crate::gabarit::Gabarit::defaut());
    let rdc = p.niveaux[0].id.clone();
    let t = p.types_murs.iter().find(|t| (t.couches.len() > 1) == composite).unwrap().id.clone();
    let prm = ParametresMur { hauteur: 2700.0, implantation: Implantation::Axe, niveau: Some(rdc), type_mur: Some(t), ..Default::default() };
    let id = p.ajouter_mur_axe(Axe::Droit { debut: [0.0, 0.0], fin: [4000.0, 0.0] }, &prm).unwrap();
    (p, id)
}

fn volume_murs(p: &Projet) -> f64 {
    let j = crate::murs::jonctions::coupes(p);
    p.murs.iter().map(|m| crate::volumes::geometrie::volume(&crate::volumes::solides::mur(p, m, &j).unwrap())).sum()
}

#[test]
fn rectangle_de_depart_garde_le_volume() {
    for composite in [false, true] {
        let (mut p, id) = projet(composite);
        let v = volume_murs(&p);
        let r = p.profil_rectangle(&id).unwrap();
        p.profils_murs.insert(id, r);
        p.verifier().unwrap();
        assert!((volume_murs(&p) - v).abs() < 1e-6 * v, "{composite}");
    }
}

#[test]
fn pignon_et_vide() {
    let (mut p, id) = projet(false);
    let e = p.type_mur(&p.murs[0].type_mur).unwrap().epaisseur();
    // pignon : 4 m de large, 2,7 m aux rives, 4,2 m au faîtage ; vide de 1 × 1 m
    let pignon = vec![[0.0, 0.0], [4000.0, 0.0], [4000.0, 2700.0], [2000.0, 4200.0], [0.0, 2700.0]];
    p.profils_murs.insert(id.clone(), ProfilMur { contours: vec![pignon.clone()] });
    p.verifier().unwrap();
    let attendu = (4000.0 * 2700.0 + 4000.0 * 1500.0 / 2.0) * e;
    assert!((volume_murs(&p) - attendu).abs() < 1e-6 * attendu);
    let vide = vec![[1000.0, 500.0], [2000.0, 500.0], [2000.0, 1500.0], [1000.0, 1500.0]];
    p.profils_murs.insert(id.clone(), ProfilMur { contours: vec![pignon.clone(), vide] });
    p.verifier().unwrap();
    let attendu = attendu - 1000.0 * 1000.0 * e;
    assert!((volume_murs(&p) - attendu).abs() < 1e-6 * attendu);
    // un vide qui sort du contour est refusé ; la clé doit désigner un mur
    let dehors = vec![[3500.0, 500.0], [4500.0, 500.0], [4500.0, 1500.0], [3500.0, 1500.0]];
    p.profils_murs.insert(id.clone(), ProfilMur { contours: vec![pignon.clone(), dehors] });
    assert!(p.verifier().is_err());
    p.profils_murs.clear();
    p.profils_murs.insert("absent".into(), ProfilMur { contours: vec![pignon] });
    assert!(p.verifier().is_err());
}

#[test]
fn mur_courbe_refuse() {
    let (mut p, id) = projet(false);
    p.murs[0].axe = Axe::Arc { debut: [0.0, 0.0], fin: [4000.0, 0.0], centre: [2000.0, -1000.0], direct: true };
    assert!(p.profil_rectangle(&id).is_err());
    p.profils_murs.insert(id, ProfilMur { contours: vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]] });
    assert!(p.verifier().is_err());
}

#[test]
fn recalage_quand_la_ligne_change() {
    let (mut p, id) = projet(false);
    p.profils_murs.insert(id.clone(), p.profil_rectangle(&id).unwrap());
    let avant = p.clone();
    // allongé par le début : le profil reste en place dans le projet
    p.murs[0].axe = Axe::Droit { debut: [-1000.0, 0.0], fin: [4000.0, 0.0] };
    p.recaler_profils_murs(&avant);
    assert_eq!(p.profils_murs[&id].contours[0][0], [1000.0, 0.0]);
    // retourné : même place, abscisses lues depuis l'autre bout
    let avant = p.clone();
    p.murs[0].axe = Axe::Droit { debut: [4000.0, 0.0], fin: [-1000.0, 0.0] };
    p.recaler_profils_murs(&avant);
    assert_eq!(p.profils_murs[&id].contours[0][0], [4000.0, 0.0]);
    // déplacé en bloc : le profil suit le mur
    let avant = p.clone();
    p.murs[0].axe = Axe::Droit { debut: [4000.0, 3000.0], fin: [-1000.0, 3000.0] };
    p.recaler_profils_murs(&avant);
    assert_eq!(p.profils_murs[&id], avant.profils_murs[&id]);
    // lecture et écriture
    let q = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(q.profils_murs, p.profils_murs);
}
