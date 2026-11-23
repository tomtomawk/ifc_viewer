//! Paillasse lissée : dessous en pente continue par volée et horizontal aux paliers.
use super::Tri;
use glam::{DVec2, DVec3};

pub(super) struct Marche {
    pub contour: Vec<DVec2>,
    pub z: f64,
    /// Premier sommet du bord d'arrivée ; absent pour un palier.
    pub arrivee: Option<usize>,
}

struct Tranche<'a> {
    marche: &'a Marche,
    dessous: Vec<DVec3>,
}

fn tranche(marche: &Marche, bas: f64, haut: f64) -> Tranche<'_> {
    let q = &marche.contour;
    let mut altitudes = vec![bas; q.len()];
    if let Some(i) = marche.arrivee {
        // Chaque rive monte de bas à haut sur toute sa longueur, y compris aux
        // sommets des rives des marches balancées.
        let (gauche, droite) = altitudes.split_at_mut(i + 1);
        for (rive, niveaux, a, b) in [
            (&q[..=i], gauche, bas, haut),
            (&q[i + 1..], droite, haut, bas),
        ] {
            let longueur: f64 = rive.windows(2).map(|w| w[0].distance(w[1])).sum();
            let mut distance = 0.0;
            for (k, z) in niveaux.iter_mut().enumerate() {
                if k > 0 {
                    distance += rive[k - 1].distance(rive[k]);
                }
                *z = if k + 1 == rive.len() {
                    b
                } else {
                    a + (b - a) * distance / longueur
                };
            }
        }
    }
    let dessous = q.iter().zip(altitudes).map(|(p, z)| p.extend(z)).collect();
    Tranche { marche, dessous }
}

pub(super) fn lisser(marches: &[Marche], base: f64, epaisseur: f64) -> Result<Vec<Tri>, String> {
    let mut bas = base - epaisseur;
    let mut tranches = Vec::with_capacity(marches.len());
    for volee in marches.chunk_by(|a, b| a.arrivee.is_some() && b.arrivee.is_some()) {
        if volee[0].arrivee.is_none() {
            tranches.push(tranche(&volee[0], bas, bas));
            continue;
        }
        let fin = volee[volee.len() - 1].z - epaisseur;
        let pente = (fin - bas) / volee.len() as f64;
        for (i, marche) in volee.iter().enumerate() {
            let arrivee = if i + 1 == volee.len() {
                fin
            } else {
                bas + pente
            };
            tranches.push(tranche(marche, bas, arrivee));
            bas = arrivee;
        }
    }

    let mut triangles = Vec::new();
    for (i, t) in tranches.iter().enumerate() {
        let voisins = [
            i.checked_sub(1).and_then(|j| tranches.get(j)),
            tranches.get(i + 1),
        ];
        let autour = &tranches[i.saturating_sub(2)..(i + 3).min(tranches.len())];
        t.mailler(voisins, autour, &mut triangles)?;
    }
    Ok(triangles)
}

fn meme_point(a: DVec3, b: DVec3) -> bool {
    a.distance_squared(b) <= 1e-10
}

impl Tranche<'_> {
    fn mailler(
        &self,
        voisins: [Option<&Self>; 2],
        autour: &[Self],
        out: &mut Vec<Tri>,
    ) -> Result<(), String> {
        let q = &self.marche.contour;
        let n = q.len();
        let z = self.marche.z;
        let coords: Vec<f64> = q.iter().flat_map(|p| [p.x, p.y]).collect();
        let indices = noyau::geometrie::earcut(&coords, &[], 2)
            .ok_or("Sous-face d'escalier : marche invalide.")?;
        for t in indices.as_chunks::<3>().0 {
            let mut ix = [t[0], t[1], t[2]];
            if (q[ix[1]] - q[ix[0]]).perp_dot(q[ix[2]] - q[ix[0]]) < 0.0 {
                ix.swap(1, 2);
            }
            out.push(ix.map(|k| q[k].extend(z)));
            out.push([
                self.dessous[ix[2]],
                self.dessous[ix[1]],
                self.dessous[ix[0]],
            ]);
        }
        let bord_commun = |k: usize, l: usize| {
            voisins
                .into_iter()
                .flatten()
                .find(|v| {
                    let b = &v.dessous;
                    (0..b.len()).any(|j| {
                        let (p, r) = (b[j], b[(j + 1) % b.len()]);
                        let (a, b) = (self.dessous[k], self.dessous[l]);
                        (meme_point(a, p) && meme_point(b, r))
                            || (meme_point(a, r) && meme_point(b, p))
                    })
                })
                .map(|v| v.marche.z)
        };
        // Au coin intérieur d'un palier, trois tranches peuvent partager un sommet.
        let cotes: Vec<Vec<f64>> = self
            .dessous
            .iter()
            .map(|&p| {
                let mut cotes: Vec<_> = autour
                    .iter()
                    .filter(|v| v.dessous.iter().any(|&q| meme_point(p, q)))
                    .map(|v| v.marche.z)
                    .collect();
                cotes.sort_by(f64::total_cmp);
                cotes.dedup();
                cotes
            })
            .collect();
        let inverse = (0..n).map(|k| q[k].perp_dot(q[(k + 1) % n])).sum::<f64>() < 0.0;
        let mut ajouter = |mut tri: Tri| {
            if (tri[1] - tri[0]).cross(tri[2] - tri[0]).length_squared() > 1e-18 {
                if inverse {
                    tri.swap(1, 2);
                }
                out.push(tri);
            }
        };
        for k in 0..n {
            let l = (k + 1) % n;
            let (mut a, mut b) = (self.dessous[k], self.dessous[l]);
            if let Some(autre) = bord_commun(k, l) {
                if autre >= z {
                    continue;
                }
                // Seule la partie au-dessus de la marche voisine est exposée.
                a.z = autre;
                b.z = autre;
            }
            let (ah, bh) = (a.with_z(z), b.with_z(z));
            let entre = |alt: f64, p: DVec3| alt > p.z && alt < z;
            let inter_a = cotes[k].iter().copied().filter(|&alt| entre(alt, a));
            let inter_b = cotes[l].iter().copied().filter(|&alt| entre(alt, b));
            if inter_a.clone().next().is_none() && inter_b.clone().next().is_none() {
                ajouter([a, b, bh]);
                ajouter([a, bh, ah]);
            } else {
                let mut contour = vec![a, b];
                contour.extend(inter_b.map(|alt| b.with_z(alt)));
                contour.extend([bh, ah]);
                contour.extend(inter_a.rev().map(|alt| a.with_z(alt)));
                let centre = (a + b + ah + bh) / 4.0;
                for j in 0..contour.len() {
                    ajouter([contour[j], contour[(j + 1) % contour.len()], centre]);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::elements::escalier::{Escalier, SousFace};
    use crate::{Niveau, Projet, volumes::geometrie::volume};
    use std::collections::HashMap;

    #[test]
    fn paillasse_lissee_fermee_en_droit_sur_palier_et_en_balance() {
        let mut projet = Projet::nouveau("Escalier");
        projet.niveaux = vec![Niveau {
            id: "rdc".into(),
            nom: "RDC".into(),
            altitude: 0.0,
            origine: None,
        }];
        for (trace, balance) in [
            (vec![[0.0, 0.0], [6000.0, 0.0]], None),
            (vec![[0.0, 0.0], [3000.0, 0.0], [3000.0, 3000.0]], None),
            (vec![[0.0, 0.0], [3000.0, 0.0], [3000.0, 3000.0]], Some(5)),
        ] {
            let e = Escalier {
                id: "e".into(),
                nom: "E".into(),
                niveau: "rdc".into(),
                niveau_haut: None,
                decalage_bas: 0.0,
                decalage_haut: 0.0,
                hauteur: 2800.0,
                trace,
                largeur: 1000.0,
                hauteur_marche: 175.0,
                paillasse: 150.0,
                sous_face: SousFace::Lissee,
                balance,
            };
            let t = e.solide(&projet).unwrap();
            assert!(volume(&t) > 0.0);
            let mut aretes = HashMap::new();
            let mut dessous_inclines = 0;
            for tri in t {
                let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
                dessous_inclines += usize::from(n.z < -1e-6 && n.truncate().length() > 1e-6);
                let v = tri.map(|p| p.to_array().map(|x| (x * 100_000.0).round() as i64));
                for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
                    let (cle, sens) = if a < b { ([a, b], 1) } else { ([b, a], -1) };
                    let (n, s) = aretes.entry(cle).or_insert((0, 0));
                    *n += 1;
                    *s += sens;
                }
            }
            assert!(dessous_inclines > 0);
            assert!(
                aretes.values().all(|&n| n == (2, 0)),
                "trace {:?}, balance {balance:?}, arêtes ouvertes {:?}",
                e.trace,
                aretes
                    .iter()
                    .filter(|(_, n)| **n != (2, 0))
                    .take(6)
                    .collect::<Vec<_>>()
            );

            let mut json = serde_json::to_value(&e).unwrap();
            assert_eq!(json["sous_face"], "lissee");
            json.as_object_mut().unwrap().remove("sous_face");
            assert_eq!(
                serde_json::from_value::<Escalier>(json).unwrap().sous_face,
                SousFace::Crantee
            );
        }
    }
}
