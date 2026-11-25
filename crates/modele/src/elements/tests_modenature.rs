use super::*;
use crate::creation::{Implantation, ParametresMur};

fn projet(composite: bool, axe: Axe) -> (Projet, String) {
    let mut p = Projet::depuis_gabarit("Essai", &crate::gabarit::Gabarit::defaut());
    let rdc = p.niveaux[0].id.clone();
    let t = p.types_murs.iter().find(|t| (t.couches.len() > 1) == composite).unwrap().id.clone();
    let prm = ParametresMur { hauteur: 2700.0, implantation: Implantation::Axe, niveau: Some(rdc), type_mur: Some(t), ..Default::default() };
    let id = p.ajouter_mur_axe(axe, &prm).unwrap();
    p.types_modenatures.push(TypeModenature { id: "bandeau".into(), nom: "Bandeau 200 × 50".into(), creux: false, profil: None, largeur: 50.0, hauteur: 200.0, materiau: None, decalage: 0.0, retrait: 0.0 });
    p.types_modenatures.push(TypeModenature { id: "engravure".into(), nom: "Engravure 20 × 20".into(), creux: true, profil: None, largeur: 20.0, hauteur: 20.0, materiau: None, decalage: 0.0, retrait: 0.0 });
    (p, id)
}

fn droit() -> Axe { Axe::Droit { debut: [0.0, 0.0], fin: [4000.0, 0.0] } }

fn volume_murs(p: &Projet) -> f64 {
    let j = crate::murs::jonctions::coupes(p);
    p.murs.iter().map(|m| crate::volumes::geometrie::volume(&crate::volumes::solides::mur(p, m, &j).unwrap())).sum()
}

fn boite(t: &[Tri]) -> (DVec3, DVec3) {
    t.iter().flatten().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), v| (a.min(*v), b.max(*v)))
}

#[test]
fn relief_le_long_de_la_face_exterieure() {
    for composite in [false, true] {
        let (mut p, id) = projet(composite, droit());
        let m = Modenature { id: "m".into(), nom: "Bandeau".into(), mur: id.clone(), type_modenature: "bandeau".into(), interieur: false, hauteur: 1000.0, depuis_haut: false, debut: None, fin: None };
        p.modenatures.push(m.clone());
        p.verifier().unwrap();
        let s = solide(&p, &m).unwrap();
        assert!((crate::volumes::geometrie::volume(&s) - 4000.0 * 200.0 * 50.0).abs() < 1e-3);
        let e = if composite { p.type_mur(&p.assemblage(&id).unwrap().type_mur).unwrap().epaisseur() } else { p.type_mur(&p.murs[0].type_mur).unwrap().epaisseur() };
        // extérieur à gauche de l'axe (y > 0) : de la face à 50 mm au-delà, de 1000 à 1200
        let (a, b) = boite(&s);
        assert!((a.y - e / 2.0).abs() < 1e-6 && (b.y - e / 2.0 - 50.0).abs() < 1e-6, "{composite} {a} {b}");
        assert!((a.z - 1000.0).abs() < 1e-6 && (b.z - 1200.0).abs() < 1e-6);
        // face intérieure, depuis le haut, retrait et bornes
        let m2 = Modenature { interieur: true, depuis_haut: true, hauteur: 300.0, debut: Some(500.0), fin: Some(3000.0), ..m };
        let (a, b) = boite(&solide(&p, &m2).unwrap());
        assert!((b.y + e / 2.0).abs() < 1e-6 && (a.y + e / 2.0 + 50.0).abs() < 1e-6);
        assert!((a.z - 2400.0).abs() < 1e-6 && (a.x - 500.0).abs() < 1e-6 && (b.x - 3000.0).abs() < 1e-6);
    }
}

#[test]
fn creux_retire_du_mur() {
    let (mut p, id) = projet(true, droit());
    let v = volume_murs(&p);
    p.modenatures.push(Modenature { id: "c".into(), nom: "Engravure".into(), mur: id, type_modenature: "engravure".into(), interieur: false, hauteur: 1000.0, depuis_haut: false, debut: None, fin: None });
    p.verifier().unwrap();
    let retire = v - volume_murs(&p);
    // 20 × 20 sur 4 m, dans la première couche (les jonctions aux bouts sont libres)
    assert!((retire - 20.0 * 20.0 * 4000.0).abs() < 1.0, "{retire}");
}

#[test]
fn mur_courbe_et_refus() {
    let (mut p, id) = projet(false, Axe::Arc { debut: [0.0, 0.0], fin: [4000.0, 0.0], centre: [2000.0, -1000.0], direct: true });
    let m = Modenature { id: "m".into(), nom: "Bandeau".into(), mur: id.clone(), type_modenature: "bandeau".into(), interieur: false, hauteur: 1000.0, depuis_haut: false, debut: None, fin: None };
    p.modenatures.push(m.clone());
    p.verifier().unwrap();
    assert!(crate::volumes::geometrie::volume(&solide(&p, &m).unwrap()) > 0.0);
    // bornes inversées, hôte absent, type absent
    p.modenatures[0].debut = Some(3000.0);
    p.modenatures[0].fin = Some(1000.0);
    assert!(p.verifier().is_err());
    p.modenatures[0] = Modenature { mur: "absent".into(), ..m.clone() };
    assert!(p.verifier().is_err());
    p.modenatures[0] = Modenature { type_modenature: "absent".into(), ..m };
    assert!(p.verifier().is_err());
    let q = Projet::lire_json(&p.json().unwrap()).unwrap();
    assert_eq!(q.types_modenatures, p.types_modenatures);
}
