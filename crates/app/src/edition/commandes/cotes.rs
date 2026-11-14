//! Cotes pilotantes : commande qui donne à une cote la valeur saisie.
use super::Commande;
use crate::modele::documentation::{angle_resolu, cotes_de, extremites_cote, AnnotationForme, Vue};
use crate::modele::Projet;
use glam::DVec2;

/// Commande qui donne à la cote la valeur saisie en déplaçant (longueur) ou en tournant
/// (angle, autour du sommet) l'élément sur lequel elle s'appuie ; l'autre référence ne
/// bouge pas.
pub fn piloter_cote(p: &Projet, v: &Vue, forme: &AnnotationForme, element: &str, valeur: f64) -> Result<Commande, String> {
    let (au_debut, a_la_fin) = cotes_de(forme, element);
    if au_debut == a_la_fin {
        return Err("Cette cote ne s'appuie pas sur l'élément d'un seul côté.".into());
    }
    if !valeur.is_finite() || valeur < 0.0 {
        return Err("Valeur de cote non valide.".into());
    }
    let ids = vec![element.to_string()];
    match forme {
        AnnotationForme::Cote { debut, fin, .. } => {
            let (a, b) = extremites_cote(p, v, debut, fin).ok_or("Cote : référence perdue.")?;
            let dir = (b - a).try_normalize().ok_or("Cote nulle : déplacez d'abord l'élément à la souris.")?;
            let delta = dir * (valeur - a.distance(b)) * if a_la_fin { 1.0 } else { -1.0 };
            Ok(Commande::Deplacer { ids, delta: [delta.x, delta.y, 0.0] })
        }
        _ => {
            let Some(AnnotationForme::CoteAngulaire { sommet, debut, fin, .. }) = angle_resolu(p, v, forme) else { return Err("Cote : référence perdue.".into()) };
            if valeur <= 0.0 || valeur >= 180.0 {
                return Err("L'angle doit être compris entre 0° et 180°.".into());
            }
            let s = DVec2::from_array(sommet);
            let (ta, tb) = ((DVec2::from_array(debut) - s).to_angle(), (DVec2::from_array(fin) - s).to_angle());
            // angle signé de la première branche vers la seconde, dans ]−180°, 180°]
            let signe = ((tb - ta + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI).to_degrees();
            let rotation = (valeur.copysign(signe) - signe) * if a_la_fin { 1.0 } else { -1.0 };
            Ok(Commande::Tourner { ids, centre: sommet, angle: rotation })
        }
    }
}

#[cfg(test)]
mod tests_aretes {
    use super::*;
    use crate::edition::Execution;
    use crate::modele::creation::ParametresMur;
    use crate::modele::documentation::{aretes_reference, valeur_pilotable, PointCote, RefArete};

    fn projet() -> (Projet, Vue, String, String) {
        let mut p = Projet::nouveau("Cotes");
        let a = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &ParametresMur::default()).unwrap();
        let b = p.ajouter_mur([0.0, 3000.0], [4000.0, 3000.0], &ParametresMur::default()).unwrap();
        let niveau = p.niveaux[0].id.clone();
        let vue = p.ajouter_vue_plan(&niveau, 1200.0).unwrap();
        let v = p.vues.iter().find(|v| v.id == vue).unwrap().clone();
        (p, v, a, b)
    }

    #[test]
    fn cote_entre_axes_suit_et_pilote_le_mur() {
        let (p, v, a, b) = projet();
        // axe du premier mur à l'abscisse 1000, axe du second : écart perpendiculaire
        let forme = AnnotationForme::Cote { debut: PointCote::Arete { id: a.clone(), arete: 0, t: 0.25 }, fin: PointCote::Arete { id: b.clone(), arete: 0, t: 0.9 }, decalage: 500.0 };
        let (x, y) = extremites_cote(&p, &v, match &forme { AnnotationForme::Cote { debut, .. } => debut, _ => unreachable!() }, match &forme { AnnotationForme::Cote { fin, .. } => fin, _ => unreachable!() }).unwrap();
        assert_eq!((x, y), (DVec2::new(1000.0, 0.0), DVec2::new(1000.0, 3000.0)));
        assert_eq!(valeur_pilotable(&p, &v, &forme, &b), Some(3000.0));
        assert_eq!(valeur_pilotable(&p, &v, &forme, "autre"), None);
        // 2500 saisi, second mur sélectionné : il se rapproche de 500 ; le premier ne bouge pas
        let (q, _) = p.executer(piloter_cote(&p, &v, &forme, &b, 2500.0).unwrap()).unwrap();
        assert_eq!(valeur_pilotable(&q, &v, &forme, &b), Some(2500.0));
        assert_eq!(aretes_reference(&q, &a)[0], aretes_reference(&p, &a)[0]);
        // premier mur sélectionné : il s'éloigne dans l'autre sens
        let (q, _) = p.executer(piloter_cote(&p, &v, &forme, &a, 3500.0).unwrap()).unwrap();
        assert_eq!(aretes_reference(&q, &a)[0][0], DVec2::new(0.0, -500.0));
    }

    #[test]
    fn angle_entre_aretes_pilote_la_rotation() {
        let (mut p, v, a, _) = projet();
        let c = p.ajouter_mur([6000.0, 0.0], [8000.0, 2000.0], &ParametresMur::default()).unwrap();
        let aretes = Some([RefArete { id: a.clone(), arete: 0 }, RefArete { id: c.clone(), arete: 0 }]);
        // branches posées vers +X et vers le haut à droite : 45° au croisement (6000, 0)
        let forme = AnnotationForme::CoteAngulaire { sommet: [6000.0, 0.0], debut: [7000.0, 0.0], fin: [7000.0, 1000.0], rayon: 800.0, aretes };
        assert!((valeur_pilotable(&p, &v, &forme, &c).unwrap() - 45.0).abs() < 1e-9);
        let (q, _) = p.executer(piloter_cote(&p, &v, &forme, &c, 60.0).unwrap()).unwrap();
        assert!((valeur_pilotable(&q, &v, &forme, &c).unwrap() - 60.0).abs() < 1e-6);
        // le mur de référence n'a pas tourné
        assert_eq!(aretes_reference(&q, &a)[0], aretes_reference(&p, &a)[0]);
        assert!(piloter_cote(&p, &v, &forme, &c, 180.0).is_err());
    }
}
