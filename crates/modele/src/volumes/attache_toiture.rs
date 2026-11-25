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
    if let Some(cle) = p.cle_profil_mur(m) {
        return crate::murs::profil::volume(p, cle, &contour, booleen);
    }
    let volume = prisme(&contour, bas, haut);
    let attaches: std::collections::BTreeSet<_> = std::iter::once(&m.id)
        .chain(m.assemblage.iter())
        .filter_map(|id| p.attaches_toitures.get(id))
        .flatten()
        .collect();
    if attaches.is_empty() {
        return Ok(volume);
    }

    let plan_horizontal = Plan {
        gradient: DVec2::ZERO,
        origine: 0.0,
        secante: 1.0,
    };
    let appliquer = |a: &[Tri], b: &[Tri], op| {
        booleen(a, b, op, 1e-5).ok_or_else(|| "Attachement à la toiture trop complexe.".to_owned())
    };
    let couche = |toit: &crate::Dalle, plans: &[Plan], z, e0, e1| {
        pente::couche(&toit.contours, plans, z, e0, e1)
            .into_iter()
            .map(|t| t.map(|v| v * 1000.0))
            .collect::<Vec<_>>()
    };

    let mut toits = Vec::new();
    let mut sommet = haut;
    for id in attaches {
        let toit = p
            .dalles
            .iter()
            .find(|d| &d.id == id)
            .ok_or("Toiture d’attachement absente.")?;
        let z = p
            .niveau(&toit.niveau)
            .ok_or("Niveau de toiture absent.")?
            .altitude
            + toit.decalage;
        let epaisseur = p
            .type_dalle(&toit.type_dalle)
            .ok_or("Type de toiture absent.")?
            .epaisseur();
        let plans = pente::plans(toit);
        let sous_face = -epaisseur;
        let max = pente::morceaux(&toit.contours, &plans, sous_face)
            .iter()
            .flat_map(|(plan, poly)| poly.iter().map(move |v| z + plan.hauteur(*v)))
            .fold(f64::NEG_INFINITY, f64::max);
        sommet = sommet.max(max);
        toits.push((toit, plans, z, sous_face));
    }

    let prolongement = prisme(&contour, bas, sommet + 1.0);
    let mut exterieur = volume;
    let mut interieur = Vec::new();
    let mut au_dessus = Vec::new();
    for (toit, plans, z, sous_face) in toits {
        let emprise = couche(toit, &[plan_horizontal], bas, -1.0, sommet - bas + 2.0);
        let emprise = appliquer(&prolongement, &emprise, Op::Intersection)?;
        if signed_volume(&emprise).abs() <= 1e-5 {
            // Le lien reste enregistré. Déplacer le toit ou le mur peut le rendre actif.
            continue;
        }
        let couvert = if pente::surface_plus_bas_des_plans(&toit.contours, &plans) {
            // Surface = plus bas des versants : la partie couverte est l'emprise coupée
            // sous chaque versant par un bloc convexe fermé. Le volume par morceaux de la
            // couche a des jonctions en T qui rendent l'intersection instable (IFC-TOIT1).
            let mut c = emprise.clone();
            for q in &plans {
                c = appliquer(&c, &bloc_sous_plan(&emprise, &q.decale(sous_face), z), Op::Intersection)?;
                if c.is_empty() {
                    break;
                }
            }
            c
        } else {
            // Toutes les sécantes sont >= 1 : ce décalage place le bas du
            // gabarit de coupe sous le pied du mur pour chaque versant.
            let dessous = couche(toit, &plans, z, sous_face - (sommet - bas) - 1.0, sous_face);
            appliquer(&prolongement, &dessous, Op::Intersection)?
        };
        if signed_volume(&couvert).abs() <= 1e-5 {
            continue;
        }
        if !exterieur.is_empty() {
            exterieur = appliquer(&exterieur, &emprise, Op::Difference)?;
        }
        interieur = if interieur.is_empty() {
            couvert.clone()
        } else {
            appliquer(&interieur, &couvert, Op::Union)?
        };
        au_dessus.push((emprise, couvert));
    }

    // Là où deux toits se recouvrent, le mur s’arrête à la première sous-face
    // rencontrée en montant. Les trémies sont absentes des masques d’emprise.
    if au_dessus.len() > 1 {
        for (emprise, couvert) in au_dessus {
            let vide = appliquer(&emprise, &couvert, Op::Difference)?;
            if !vide.is_empty() && !interieur.is_empty() {
                interieur = appliquer(&interieur, &vide, Op::Difference)?;
            }
        }
    }
    match (exterieur.is_empty(), interieur.is_empty()) {
        (true, _) => Ok(interieur),
        (_, true) => Ok(exterieur),
        _ => appliquer(&interieur, &exterieur, Op::Union),
    }
}

/// Bloc convexe fermé (mm) sous le plan `q` (hauteur au-dessus de `z0`), débordant de
/// 1 m l'emprise en plan de `solide`, fond plat 1 m sous le plus bas de ses points et du plan.
fn bloc_sous_plan(solide: &[Tri], q: &Plan, z0: f64) -> Vec<Tri> {
    use glam::DVec3;
    let (mut min, mut max) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    let mut bas = f64::INFINITY;
    for v in solide.iter().flatten() {
        min = min.min(v.truncate());
        max = max.max(v.truncate());
        bas = bas.min(v.z);
    }
    let (min, max) = (min - DVec2::splat(1000.0), max + DVec2::splat(1000.0));
    let coins = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)];
    let haut = coins.map(|c| z0 + q.hauteur(c));
    let fond = haut.iter().copied().fold(bas, f64::min) - 1000.0;
    let h: [DVec3; 4] = std::array::from_fn(|i| coins[i].extend(haut[i]));
    let b: [DVec3; 4] = std::array::from_fn(|i| coins[i].extend(fond));
    let mut tris = vec![[h[0], h[1], h[2]], [h[0], h[2], h[3]], [b[0], b[2], b[1]], [b[0], b[3], b[2]]];
    for i in 0..4 {
        let j = (i + 1) % 4;
        tris.push([b[i], b[j], h[j]]);
        tris.push([b[i], h[j], h[i]]);
    }
    tris
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
                decalage: decalage + p.types_murs[0].epaisseur(),
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
        p.attaches_toitures.insert("m".into(), [id].into());
        let coupes = jonctions::coupes(&p);
        for hauteur in [4200.0, 2100.0] {
            p.dalles[0].decalage = hauteur + p.types_murs[0].epaisseur();
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
        p.dalles[0].decalage = 3500.0 + p.types_murs[0].epaisseur() * 2.0_f64.sqrt();
        p.dalles[0].pentes = vec![vec![None, None, None, Some(45.0)], vec![None; 4]];
        p.attaches_toitures.insert("m".into(), [id].into());
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
        p.attaches_toitures.insert(id, [toit].into());
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
    fn un_toit_hors_emprise_ou_sous_le_pied_ne_change_pas_le_mur() {
        let mut p = exemple();
        let id = toiture(&mut p, &[rectangle(4000.0, 5000.0, -500.0, 500.0)], 3500.0);
        p.attaches_toitures.insert("m".into(), [id].into());
        let coupes = jonctions::coupes(&p);
        let tris = mur(&p, &p.murs[0], &coupes).unwrap();
        assert!((signed_volume(&tris) - 2.4e9).abs() < 1.0);
        p.dalles[0].contours = vec![rectangle(1000.0, 5000.0, -500.0, 500.0)];
        p.dalles[0].decalage = -100.0;
        let tris = mur(&p, &p.murs[0], &coupes).unwrap();
        assert!((signed_volume(&tris) - 2.4e9).abs() < 1.0);
    }
    #[test]
    fn decalage_place_le_dessus_a_legout() {
        let mut p = exemple();
        toiture(&mut p, &[rectangle(1000.0, 5000.0, -500.0, 500.0)], 3500.0);
        let d = &mut p.dalles[0];
        d.decalage = 3500.0;
        d.pentes = vec![vec![None, None, None, Some(45.0)]];
        let (bas, haut) = d.decalages_couches(p.types_murs[0].epaisseur());
        let tris = pente::couche(&d.contours, &pente::plans(d), d.decalage, bas, haut);
        let cote = tris
            .iter()
            .flatten()
            .filter(|v| (v.x - 1.0).abs() < 1e-8)
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((cote - 3.5).abs() < 1e-8);
    }

    #[test]
    fn deux_toitures_decoupent_un_mur_et_la_plus_basse_prime_au_recouvrement() {
        let mut p = exemple();
        let a = toiture(&mut p, &[rectangle(-500.0, 2500.0, -500.0, 500.0)], 2500.0);
        let b = toiture(&mut p, &[rectangle(1500.0, 3500.0, -500.0, 500.0)], 4000.0);
        let c = toiture(&mut p, &[rectangle(5000.0, 6000.0, -500.0, 500.0)], 6000.0);
        p.attaches_toitures.insert("m".into(), [a, b, c].into());
        let tris = mur(&p, &p.murs[0], &jonctions::coupes(&p)).unwrap();
        let attendu = 200.0 * (2500.0 * 2500.0 + 1000.0 * 4000.0 + 500.0 * 3000.0);
        assert!((signed_volume(&tris) - attendu).abs() < 1.0);
    }

    /// IFC-TOIT1 : sous une toiture en croupe, les murs opposés de la maison d'exemple
    /// gardent le même volume quelle que soit la cote d'égout.
    #[test]
    fn murs_opposes_sous_croupe_symetriques() {
        let base = Projet::lire_json(include_str!("../../../../samples/maison_native.bem")).unwrap();
        for k in 0..40 {
            let mut p = base.clone();
            p.dalles.iter_mut().find(|d| d.id == "toiture").unwrap().decalage = 2600.155 + k as f64 * 13.7;
            let v = |id: &str| p.volume_mur(p.murs.iter().find(|m| m.id == id).unwrap()).unwrap();
            assert!((v("etage-m1") - v("etage-m3")).abs() < 1e3, "égout {k} : {} / {}", v("etage-m1"), v("etage-m3"));
            assert!((v("etage-m0") - v("etage-m2")).abs() < 1e3);
        }
    }
}