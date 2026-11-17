use crate::elements::pente::{self, Plan};
use crate::murs::jonctions;
use crate::volumes::solides::{Tri, bande, prisme};
use crate::{Axe, Mur, Projet};
use glam::DVec2;
use noyau::csg::{Op, signed_volume};

type Booleen = fn(&[Tri], &[Tri], Op, f64) -> Option<Vec<Tri>>;

fn contour_mur(
    p: &Projet,
    m: &Mur,
    jonctions: &jonctions::Coupes,
    epaisseur: f64,
) -> Result<Vec<[f64; 2]>, String> {
    let contour = match m.axe {
        Axe::Droit { .. } => jonctions::contour(p, m, jonctions)
            .ok_or("Contour du mur non valide.")?
            .to_vec(),
        Axe::Arc { .. } => bande(&m.axe, 0.0, m.axe.longueur(), epaisseur)?,
    };
    if contour.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Contour non fini.".into());
    }
    Ok(contour)
}

pub(super) fn volume_mur(
    p: &Projet,
    m: &Mur,
    jonctions: &jonctions::Coupes,
    (bas, haut): (f64, f64),
    epaisseur: f64,
    booleen: Booleen,
) -> Result<Vec<Tri>, String> {
    let contour = contour_mur(p, m, jonctions, epaisseur)?;
    let volume = prisme(&contour, bas, haut);
    let attache_propre = p.attaches_toitures.get(&m.id);
    let attache = attache_propre.or_else(|| {
        m.assemblage
            .as_ref()
            .and_then(|id| p.attaches_toitures.get(id))
    });
    let Some(toit) = attache.and_then(|id| p.dalles.iter().find(|d| &d.id == id)) else {
        return Ok(volume);
    };

    let z = p
        .niveau(&toit.niveau)
        .ok_or("Niveau de toiture absent.")?
        .altitude
        + toit.decalage;
    let plans = pente::plans(toit);
    let sommet = pente::morceaux(&toit.contours, &plans, 0.0)
        .iter()
        .flat_map(|(plan, polygone)| polygone.iter().map(move |v| z + plan.hauteur(*v)))
        .fold(haut, f64::max);
    let couche = |plans: &[Plan], z, e0, e1| {
        pente::couche(&toit.contours, plans, z, e0, e1)
            .into_iter()
            .map(|t| t.map(|v| v * 1000.0))
            .collect::<Vec<_>>()
    };
    let appliquer = |a: &[Tri], b: &[Tri], op| {
        booleen(a, b, op, 1e-5).ok_or_else(|| "Attachement à la toiture trop complexe.".to_owned())
    };

    // La partie sous le toit peut dépasser la hauteur nominale du mur.
    let sous_face = couche(&plans, z, bas.min(z) - sommet.max(z) - 1.0, 0.0);
    let prolongement = prisme(&contour, bas, sommet + 1.0);
    let couvert = appliquer(&prolongement, &sous_face, Op::Intersection)?;

    // Le prisme de l'emprise prend aussi en compte les trémies du toit.
    let plan = Plan {
        gradient: DVec2::ZERO,
        origine: 0.0,
        secante: 1.0,
    };
    let emprise = couche(&[plan], bas, -1.0, haut - bas + 1.0);
    if signed_volume(&couvert).abs() <= 1e-5 {
        let recouvre = |volume: &[Tri]| -> Result<bool, String> {
            Ok(signed_volume(&appliquer(volume, &emprise, Op::Intersection)?).abs() > 1e-5)
        };
        if recouvre(&volume)? {
            return Err("La toiture se trouve sous le pied du mur.".into());
        }
        // Un bord de toiture peut recouvrir certaines couches seulement.
        if let Some(id) = m.assemblage.as_deref().filter(|_| attache_propre.is_none()) {
            for autre in p.couches_assemblage(id) {
                if autre.id == m.id || p.attaches_toitures.contains_key(&autre.id) {
                    continue;
                }
                let epaisseur = p
                    .type_mur(&autre.type_mur)
                    .ok_or("Type de mur introuvable.")?
                    .epaisseur();
                let contour = contour_mur(p, autre, jonctions, epaisseur)?;
                if recouvre(&prisme(&contour, bas, haut))? {
                    return Ok(volume);
                }
            }
        }
        return Err(format!(
            "{} : le mur ne recouvre pas l'emprise de la toiture.",
            m.nom
        ));
    }

    let exterieur = appliquer(&volume, &emprise, Op::Difference)?;
    if exterieur.is_empty() {
        return Ok(couvert);
    }
    appliquer(&couvert, &exterieur, Op::Union)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SorteDalle;
    use crate::creation::{ParametresDalle, ParametresMur};
    use crate::volumes::solides::{mur, murs, tests::exemple};

    fn rectangle(x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<[f64; 2]> {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    fn toiture(p: &mut Projet, contours: &[Vec<[f64; 2]>], decalage: f64) -> String {
        p.ajouter_dalle_contours(
            contours,
            &ParametresDalle {
                sorte: SorteDalle::Toiture,
                type_dalle: p.types_murs[0].clone(),
                pente: None,
                niveau: Some("n".into()),
                decalage,
                phase: None,
                sous_projet: None,
                variante: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn mur_partiellement_couvert_suit_le_toit_et_garde_sa_hauteur_a_lexterieur() {
        let mut p = exemple();
        let id = toiture(&mut p, &[rectangle(1000.0, 5000.0, -500.0, 500.0)], 4200.0);
        p.attaches_toitures.insert("m".into(), id);
        let coupes = jonctions::coupes(&p);
        for hauteur in [4200.0, 2100.0] {
            p.dalles[0].decalage = hauteur;
            let tris = mur(&p, &p.murs[0], &coupes).unwrap();
            let attendu = 200.0 * (1000.0 * 3000.0 + 3000.0 * hauteur);
            assert!((signed_volume(&tris) - attendu).abs() < 1.0);
            for (x, haut) in [(0.0, 3000.0), (4000.0, hauteur)] {
                let z = tris
                    .iter()
                    .flatten()
                    .filter(|v| (v.x - x).abs() < 1e-5)
                    .map(|v| v.z)
                    .fold(f64::NEG_INFINITY, f64::max);
                assert!((z - haut).abs() < 1e-5, "x = {x}, z = {z}");
            }
        }
        p.attaches_toitures.clear();
        assert!((signed_volume(&mur(&p, &p.murs[0], &coupes).unwrap()) - 2.4e9).abs() < 1.0);
    }

    #[test]
    fn mur_traversant_un_toit_en_pente_avec_tremie() {
        let mut p = exemple();
        let contours = [
            rectangle(1000.0, 3000.0, -500.0, 500.0),
            rectangle(1500.0, 2000.0, -300.0, 300.0),
        ];
        let id = toiture(&mut p, &contours, 3500.0);
        // Une pente de 45° à partir du bord x = 1000 mm.
        p.dalles[0].pentes = vec![vec![None, None, None, Some(45.0)], vec![None; 4]];
        p.attaches_toitures.insert("m".into(), id);
        let tris = mur(&p, &p.murs[0], &jonctions::coupes(&p)).unwrap();
        let attendu = 200.0 * (2500.0 * 3000.0 + 500.0 * 3750.0 + 1000.0 * 5000.0);
        assert!((signed_volume(&tris) - attendu).abs() < 1.0);
    }

    #[test]
    fn attache_un_assemblage_dont_une_couche_reste_hors_toiture() {
        let mut p = exemple();
        p.murs.clear();
        let mut autre_couche = p.types_murs[0].couches[0].clone();
        autre_couche.epaisseur = 100.0;
        p.types_murs[0].couches.push(autre_couche);
        let parametres = ParametresMur {
            type_mur: Some("t".into()),
            hauteur: 3000.0,
            ..Default::default()
        };
        let id = p
            .ajouter_mur([0.0, 0.0], [4000.0, 0.0], &parametres)
            .unwrap();
        let toit = toiture(&mut p, &[rectangle(-500.0, 4500.0, -50.0, 500.0)], 4000.0);
        p.attaches_toitures.insert(id, toit);
        let coupes = jonctions::coupes(&p);
        let mut hauteurs: Vec<_> = murs(&p, &coupes)
            .into_iter()
            .map(|tris| {
                tris.unwrap()
                    .iter()
                    .flatten()
                    .map(|v| v.z)
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .collect();
        hauteurs.sort_by(f64::total_cmp);
        assert_eq!(hauteurs.len(), 2);
        for (haut, attendu) in hauteurs.into_iter().zip([3000.0, 4000.0]) {
            assert!((haut - attendu).abs() < 1e-5, "{haut} au lieu de {attendu}");
        }
    }

    #[test]
    fn un_toit_sans_recouvrement_ou_sous_le_pied_du_mur_reste_refuse() {
        let mut p = exemple();
        let id = toiture(&mut p, &[rectangle(4000.0, 5000.0, -500.0, 500.0)], 3500.0);
        p.attaches_toitures.insert("m".into(), id);
        let coupes = jonctions::coupes(&p);
        assert!(
            mur(&p, &p.murs[0], &coupes)
                .unwrap_err()
                .contains("emprise")
        );
        p.dalles[0].contours = vec![rectangle(1000.0, 5000.0, -500.0, 500.0)];
        p.dalles[0].decalage = -100.0;
        assert!(
            mur(&p, &p.murs[0], &coupes)
                .unwrap_err()
                .contains("sous le pied")
        );
    }
}
