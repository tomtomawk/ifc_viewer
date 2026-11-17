//! Espaces IFC (`IfcSpace`) convertis en pièces natives.
//!
//! Pour chaque espace : emprise au sol lue dans sa géométrie (faces du dessous), point
//! intérieur de cette emprise, contour recalculé par la détection native
//! (`modele::elements::piece::detecter`) sur les murs convertis. La pièce garde le numéro
//! (`Name`), le nom (`LongName`, sinon `Name`), le niveau, l'origine et les jeux de
//! l'espace. Sans contour, ou si l'aire détectée s'écarte de plus de `ECART_MAX` de
//! l'emprise ou de `NetFloorArea`, l'espace reste un élément importé transparent.
use crate::donnees_ifc;
use glam::{DVec2, DVec3};
use ifc_base::semantique::{IfcModel, pretty_type};
use ifc_base::step::Id;
use modele::{Conversion, Origine, Piece, identifiant};
use std::collections::HashMap;

/// Écart d'aire admis entre le contour détecté et l'emprise de l'espace.
pub const ECART_MAX: f64 = 0.05;
/// Couleur d'un espace resté élément importé : bleu transparent.
const COULEUR_REPLI: [u8; 4] = [89, 153, 230, 56];

/// Emprise au sol d'un espace : triangles du dessous (normale vers le bas, à la cote la
/// plus basse), en plan (mm).
fn emprise(tris: &[[DVec3; 3]]) -> Vec<[DVec2; 3]> {
    let bas = tris.iter().flatten().map(|p| p.z).fold(f64::INFINITY, f64::min);
    tris.iter()
        .filter(|t| t.iter().all(|p| (p.z - bas).abs() < 1.0))
        .filter(|t| {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            n.length() > 1e-9 && n.normalize().z.abs() > 0.99
        })
        .map(|t| t.map(|p| p.truncate()))
        .collect()
}

fn aire_triangle(t: &[DVec2; 3]) -> f64 {
    (t[1] - t[0]).perp_dot(t[2] - t[0]).abs() / 2.0
}

fn distance_segment(q: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    q.distance(a + ab * t)
}

/// Point intérieur de l'emprise, le plus loin possible de son bord : parmi les centres
/// des triangles et les milieux de leurs arêtes intérieures (une emprise en L a son
/// centre de boîte hors d'elle).
pub(crate) fn point_interieur(faces: &[[DVec2; 3]]) -> Option<DVec2> {
    let cle = |p: DVec2| ((p.x * 10.0).round() as i64, (p.y * 10.0).round() as i64);
    let mut aretes: HashMap<((i64, i64), (i64, i64)), (usize, DVec2, DVec2)> = HashMap::new();
    for t in faces {
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            let k = if cle(a) < cle(b) { (cle(a), cle(b)) } else { (cle(b), cle(a)) };
            aretes.entry(k).or_insert((0, a, b)).0 += 1;
        }
    }
    let bord: Vec<(DVec2, DVec2)> = aretes.values().filter(|x| x.0 == 1).map(|x| (x.1, x.2)).collect();
    let candidats = faces
        .iter()
        .filter(|t| aire_triangle(t) > 1.0)
        .map(|t| (t[0] + t[1] + t[2]) / 3.0)
        .chain(aretes.values().filter(|x| x.0 > 1).map(|x| (x.1 + x.2) / 2.0));
    let profondeur = |q: DVec2| bord.iter().map(|(a, b)| distance_segment(q, *a, *b)).fold(f64::INFINITY, f64::min);
    candidats.map(|q| (profondeur(q), q)).filter(|(d, _)| d.is_finite()).max_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1)
}

/// Écart relatif de `aire` à `reference`.
fn ecart(aire: f64, reference: f64) -> f64 {
    (aire - reference) / reference
}

/// Convertit les `IfcSpace` du modèle (après la validation des murs) ; `couleurs` :
/// couleurs lues des produits. Les espaces non convertis deviennent des éléments
/// importés, avec leurs données et une couleur transparente.
pub(crate) fn convertir(m: &IfcModel, geometries: &HashMap<Id, Vec<[DVec3; 3]>>, couleurs: &HashMap<Id, [f32; 4]>, c: &mut Conversion) {
    let mut espaces: Vec<Id> = m.products.iter().copied().filter(|&id| m.ty(id) == "IFCSPACE").collect();
    espaces.sort_unstable();
    let fichier = c.projet.sources.first().map(|s| s.fichier.clone()).unwrap_or_default();
    let vide = Vec::new();
    for id in espaces {
        let e = m.get(id);
        let nom_ifc = e.and_then(|e| e.arg(2).as_str()).unwrap_or_default().trim().to_string();
        let nom_long = e.and_then(|e| e.arg(7).as_str()).unwrap_or_default().trim().to_string();
        let guid = Some(m.global_id(id)).filter(|g| !g.is_empty());
        let origine = Some(Origine { source: "s1".into(), id: format!("#{id}"), guid: guid.clone(), classe: m.ty(id).to_string() });
        let tris = geometries.get(&id).unwrap_or(&vide);
        let niveau = m.storey_of(id).and_then(|s| c.projet.niveaux.iter().find(|n| n.origine.as_ref().is_some_and(|o| o.id == format!("#{s}")))).map(|n| n.id.clone());
        let mut donnees = donnees_ifc::lire(m, id, couleurs.get(&id).copied());
        let net = donnees.proprietes.iter().flat_map(|j| &j.proprietes).find(|p| p.nom == "NetFloorArea").and_then(|p| p.nombre()).filter(|v| *v > 0.0).map(|v| v * 1e6);
        let faces = emprise(tris);
        let surface: f64 = faces.iter().map(aire_triangle).sum();
        let resultat = (|| -> Result<Piece, String> {
            let niveau = niveau.clone().ok_or("pas de niveau (IfcBuildingStorey)")?;
            let point = point_interieur(&faces).ok_or("emprise au sol non lue dans la géométrie")?;
            let contour = modele::elements::piece::detecter(&c.projet, &niveau, point.to_array()).map_err(|e| format!("aucun contour de murs : {e}"))?;
            let piece = Piece { id: String::new(), nom: String::new(), numero: String::new(), niveau, point: point.to_array(), contour, etiquette: String::new(), donnees: Default::default(), origine: None };
            let aire = piece.aire();
            for (reference, quoi) in [(Some(surface).filter(|s| *s > 0.0), "l'emprise de l'espace"), (net, "NetFloorArea")] {
                if let Some(r) = reference
                    && ecart(aire, r).abs() > ECART_MAX
                {
                    return Err(format!("aire du contour de murs {:.2} m² : écart de {:+.1} % avec {quoi} ({:.2} m²)", aire / 1e6, 100.0 * ecart(aire, r), r / 1e6));
                }
            }
            Ok(piece)
        })();
        let numero = if nom_ifc.is_empty() { format!("#{id}") } else { nom_ifc.clone() };
        let nom = if nom_long.is_empty() { numero.clone() } else { nom_long };
        match resultat {
            Ok(mut piece) => {
                piece.id = identifiant("piece", &guid.clone().unwrap_or_else(|| format!("{fichier}:#{id}")));
                piece.numero = numero;
                piece.nom = nom;
                piece.donnees = donnees;
                piece.origine = origine;
                c.projet.pieces.push(piece);
            }
            Err(raison) => {
                let avant = c.projet.importes.len();
                let titre = if nom == numero { numero } else { format!("{numero} {nom}") };
                c.importer(titre, pretty_type(m.ty(id)), niveau, tris, &format!("espace non converti en pièce : {raison}"), origine);
                if let Some(x) = c.projet.importes.get_mut(avant) {
                    if donnees.couleur.is_none() {
                        donnees.couleur = Some(COULEUR_REPLI);
                    }
                    x.donnees = donnees;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Emprise en L : le centre de la boîte est hors d'elle, le point retenu est dedans.
    #[test]
    fn point_dans_un_l() {
        let p = |x: f64, y: f64| DVec2::new(x, y);
        // L : carré 0..6 × 0..6 privé de 3..6 × 3..6, en 4 triangles
        let faces = vec![[p(0., 0.), p(6., 0.), p(6., 3.)], [p(0., 0.), p(6., 3.), p(0., 3.)], [p(0., 3.), p(3., 3.), p(3., 6.)], [p(0., 3.), p(3., 6.), p(0., 6.)]].into_iter().map(|t| t.map(|q| q * 1000.0)).collect::<Vec<_>>();
        let q = point_interieur(&faces).unwrap();
        let dedans = (q.x < 6000.0 && q.y < 3000.0) || (q.x < 3000.0 && q.y < 6000.0);
        assert!(dedans && q.x > 0.0 && q.y > 0.0, "{q:?}");
        let surface: f64 = faces.iter().map(aire_triangle).sum();
        assert!((surface - 27e6).abs() < 1.0);
    }
}
