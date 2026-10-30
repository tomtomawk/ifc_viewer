//! Géométrie native en millimètres, commune au rendu, aux quantités et aux exports.
use super::*;
use glam::{DVec2, DVec3};
pub type Tri = [DVec3; 3];

/// Prisme rectangulaire vertical ou horizontal, dans le repère du projet.
pub fn barre(p: &Projet, b: &Barre) -> Result<Vec<Tri>, String> {
    let z = p
        .niveau(&b.niveau)
        .ok_or("Niveau de la structure absent.")?
        .altitude;
    if b.debut
        .iter()
        .chain(&b.fin)
        .chain([&b.largeur, &b.hauteur, &b.rotation])
        .any(|v| !v.is_finite())
        || b.largeur <= 0.0
        || b.hauteur <= 0.0
    {
        return Err("Dimensions de structure non valides.".into());
    }
    let a = DVec2::new(b.debut[0], b.debut[1]);
    let f = DVec2::new(b.fin[0], b.fin[1]);
    let (contour, bas, haut) = match b.genre {
        GenreBarre::Poteau => {
            if a.distance(f) > 1e-6 || b.fin[2] <= b.debut[2] {
                return Err("Le poteau doit être vertical et avoir une hauteur positive.".into());
            }
            let r = b.rotation.to_radians();
            let u = DVec2::new(r.cos(), r.sin()) * (b.largeur / 2.0);
            let v = DVec2::new(-r.sin(), r.cos()) * (b.hauteur / 2.0);
            (
                vec![
                    (a - u - v).to_array(),
                    (a + u - v).to_array(),
                    (a + u + v).to_array(),
                    (a - u + v).to_array(),
                ],
                z + b.debut[2],
                z + b.fin[2],
            )
        }
        GenreBarre::Poutre => {
            if (b.fin[2] - b.debut[2]).abs() > 1e-6 || a.distance(f) < 1e-6 {
                return Err(
                    "La poutre doit être horizontale et avoir une longueur positive.".into(),
                );
            }
            let v = (f - a).normalize().perp() * (b.largeur / 2.0);
            (
                vec![
                    (a - v).to_array(),
                    (f - v).to_array(),
                    (f + v).to_array(),
                    (a + v).to_array(),
                ],
                z + b.debut[2] - b.hauteur / 2.0,
                z + b.debut[2] + b.hauteur / 2.0,
            )
        }
    };
    Ok(prisme(&contour, bas, haut))
}

pub fn maillage(m: &Maillage) -> Result<Vec<Tri>, String> {
    if m.sommets.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Maillage : coordonnées non finies.".into());
    }
    m.triangles
        .iter()
        .map(|t| {
            let mut pts = [DVec3::ZERO; 3];
            for k in 0..3 {
                pts[k] = DVec3::from_array(
                    *m.sommets
                        .get(t[k] as usize)
                        .ok_or("Maillage : indice de sommet hors limites.")?,
                );
            }
            Ok(pts)
        })
        .collect()
}

pub fn prisme(contour: &[[f64; 2]], bas: f64, haut: f64) -> Vec<Tri> {
    if haut <= bas || contour.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let coords: Vec<f64> = contour.iter().flatten().copied().collect();
    let Ok(indices) = earcutr::earcut(&coords, &[], 2) else {
        return out;
    };
    let p = |i: usize, z: f64| DVec3::new(contour[i][0], contour[i][1], z);
    let area: f64 = (0..contour.len())
        .map(|i| {
            let a = contour[i];
            let b = contour[(i + 1) % contour.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    for t in indices.chunks_exact(3) {
        let mut top = [p(t[0], haut), p(t[1], haut), p(t[2], haut)];
        if (top[1] - top[0]).cross(top[2] - top[0]).z < 0.0 {
            top.swap(1, 2);
        }
        out.push(top);
        out.push([top[2].with_z(bas), top[1].with_z(bas), top[0].with_z(bas)]);
    }
    for i in 0..contour.len() {
        let j = (i + 1) % contour.len();
        let mut ts = [
            [p(i, bas), p(j, bas), p(j, haut)],
            [p(i, bas), p(j, haut), p(i, haut)],
        ];
        if area < 0.0 {
            for t in &mut ts {
                t.swap(1, 2);
            }
        }
        out.extend(ts);
    }
    out
}

/// Position et tangente à une abscisse curviligne (mm).
pub fn repere(axe: &Axe, s: f64) -> (DVec2, DVec2) {
    match *axe {
        Axe::Droit { debut, fin } => {
            let a = DVec2::from_array(debut);
            let u = (DVec2::from_array(fin) - a).normalize_or_zero();
            (a + u * s, u)
        }
        Axe::Arc { centre, .. } => {
            let (r, a, b) = axe.arc().unwrap();
            let angle = a + b.signum() * s / r;
            let radial = DVec2::new(angle.cos(), angle.sin());
            (
                DVec2::from_array(centre) + radial * r,
                radial.perp() * b.signum(),
            )
        }
    }
}

/// Contour d'une portion de mur, avec une flèche maximale de 0,5 mm pour les arcs.
pub fn bande(axe: &Axe, debut: f64, fin: f64, epaisseur: f64) -> Result<Vec<[f64; 2]>, String> {
    if ![debut, fin, epaisseur, axe.longueur()]
        .iter()
        .all(|x| x.is_finite())
        || fin <= debut
        || epaisseur <= 0.0
    {
        return Err("Dimensions de mur non valides.".into());
    }
    let n = match axe {
        Axe::Droit { debut, fin } => {
            if debut.iter().chain(fin).any(|v| !v.is_finite()) {
                return Err("Axe non fini.".into());
            }
            1
        }
        Axe::Arc {
            debut: a,
            fin: b,
            centre,
            ..
        } => {
            if a.iter().chain(b).chain(centre).any(|v| !v.is_finite()) {
                return Err("Arc non fini.".into());
            }
            let (r, _, _) = axe.arc().unwrap();
            let rb = (DVec2::from_array(*b) - DVec2::from_array(*centre)).length();
            if r <= epaisseur / 2.0 || (r - rb).abs() > 1.0 {
                return Err("Rayon du mur courbe non valide.".into());
            }
            let angle = (2.0 * (1.0 - 0.5 / (r + epaisseur / 2.0)).clamp(-1.0, 1.0).acos())
                .min(5.0_f64.to_radians())
                .max(1e-5);
            (((fin - debut) / r / angle).ceil() as usize).clamp(1, 4096)
        }
    };
    let mut c = Vec::with_capacity((n + 1) * 2);
    for k in 0..=n {
        let (p, u) = repere(axe, debut + (fin - debut) * k as f64 / n as f64);
        c.push((p - u.perp() * epaisseur / 2.0).to_array());
    }
    for k in (0..=n).rev() {
        let (p, u) = repere(axe, debut + (fin - debut) * k as f64 / n as f64);
        c.push((p + u.perp() * epaisseur / 2.0).to_array());
    }
    Ok(c)
}

/// Axe et épaisseur totale d'un hôte, indépendamment des décalages de ses couches.
pub fn hote(p: &Projet, id: &str) -> Result<(Axe, f64, String), String> {
    if let Some(a) = p.assemblage(id) {
        let m = p
            .couches_assemblage(id)
            .first()
            .copied()
            .ok_or("Assemblage vide.")?;
        let t = p.type_mur(&a.type_mur).ok_or("Type d'assemblage absent.")?;
        let d = t.decalages_couches().first().copied().unwrap_or(0.0);
        Ok((
            assemblage::decaler_axe(&m.axe, -d),
            t.epaisseur(),
            m.niveau.clone(),
        ))
    } else {
        let m = p
            .murs
            .iter()
            .find(|m| m.id == id)
            .ok_or("Hôte de l'ouverture introuvable.")?;
        Ok((
            m.axe.clone(),
            p.type_mur(&m.type_mur)
                .ok_or("Type de mur absent.")?
                .epaisseur(),
            m.niveau.clone(),
        ))
    }
}

fn mur_brut(
    p: &Projet,
    m: &Mur,
    jonctions: &crate::modele::jonctions::Coupes,
) -> Result<Vec<Tri>, String> {
    let e = p
        .type_mur(&m.type_mur)
        .ok_or("Type de mur introuvable.")?
        .epaisseur();
    let (bas, mut haut) = p.bas_haut(m).ok_or("Contrainte de hauteur non valide.")?;
    if ![bas, haut, e].iter().all(|v| v.is_finite()) || haut <= bas || m.axe.longueur() < 1.0 {
        return Err("Dimensions du mur non valides.".into());
    }
    let toit = p
        .attaches_toitures
        .get(&m.id)
        .or_else(|| {
            m.assemblage
                .as_ref()
                .and_then(|a| p.attaches_toitures.get(a))
        })
        .and_then(|id| p.dalles.iter().find(|d| &d.id == id));
    if let Some(d) = toit {
        let z = p
            .niveau(&d.niveau)
            .ok_or("Niveau de toiture absent.")?
            .altitude
            + d.decalage;
        haut = pente::morceaux(&d.contours, &pente::plans(d), 0.0)
            .iter()
            .flat_map(|(q, poly)| poly.iter().map(move |v| z + q.hauteur(*v)))
            .fold(haut, f64::max)
            + 1.0;
    }
    let contour = match m.axe {
        Axe::Droit { .. } => crate::modele::jonctions::contour(p, m, jonctions)
            .ok_or("Contour du mur non valide.")?
            .to_vec(),
        Axe::Arc { .. } => bande(&m.axe, 0.0, m.axe.longueur(), e)?,
    };
    if contour.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Contour non fini.".into());
    }
    let mut tris = prisme(&contour, bas, haut);
    if let Some(d) = toit {
        if contour.iter().any(|&v| {
            d.contours
                .iter()
                .filter(|c| geometrie::dedans(v, c))
                .count()
                % 2
                == 0
        }) {
            return Err(format!(
                "{} : le mur attaché doit être entièrement sous la toiture (prévoir un débord).",
                m.nom
            ));
        }
        let z = p.niveau(&d.niveau).unwrap().altitude + d.decalage;
        let plans = pente::plans(d);
        let cap = pente::couche(
            &d.contours,
            &plans,
            z,
            -(haut - bas + z.abs() + bas.abs() + 1000.0),
            0.0,
        )
        .into_iter()
        .map(|t| t.map(|v| v * 1000.0))
        .collect::<Vec<_>>();
        tris = crate::csg::boolean(&tris, &cap, crate::csg::Op::Intersection, 1e-5)
            .ok_or("Attachement à la toiture trop complexe.")?;
        if tris.is_empty() {
            return Err("La toiture se trouve sous le pied du mur.".into());
        }
    }
    for o in p
        .ouvertures
        .iter()
        .filter(|o| o.mur == m.id || m.assemblage.as_ref() == Some(&o.mur))
    {
        let (axe, ep, niveau) = hote(p, &o.mur)?;
        let z = p
            .niveau(&niveau)
            .ok_or("Niveau de l'ouverture introuvable.")?
            .altitude
            + o.allege;
        if ![o.abscisse, o.largeur, o.hauteur, z]
            .iter()
            .all(|v| v.is_finite())
            || o.largeur <= 0.0
            || o.hauteur <= 0.0
        {
            return Err("Dimensions d'ouverture non valides.".into());
        }
        // famille modélisée : ses vides percent le mur
        if let Some(vides) = vides_famille(p, o)? {
            for vide in vides {
                if tris.is_empty() {
                    break;
                }
                tris = crate::csg::boolean(&tris, &vide, crate::csg::Op::Difference, 1e-5)
                    .ok_or("Calcul du percement trop complexe ; projet conservé.")?;
            }
            continue;
        }
        // Marge transversale pour couper même les faces coplanaires d'une couche.
        let vide = prisme(
            &bande(
                &axe,
                o.abscisse - o.largeur / 2.0,
                o.abscisse + o.largeur / 2.0,
                ep + 2.0,
            )?,
            z,
            z + o.hauteur,
        );
        if tris.is_empty() {
            break;
        }
        tris = crate::csg::boolean(&tris, &vide, crate::csg::Op::Difference, 1e-5)
            .ok_or("Calcul du percement trop complexe ; projet conservé.")?;
    }
    Ok(tris)
}

/// Jonctions volumétriques : aux croisements et aux raccords courbes, la couche
/// prioritaire conserve son volume ; l'autre s'arrête sur ses faces. L'ordre est stable.
pub fn mur(
    p: &Projet,
    m: &Mur,
    jonctions: &crate::modele::jonctions::Coupes,
) -> Result<Vec<Tri>, String> {
    let mut tris = mur_brut(p, m, jonctions)?;
    let priorite = |m: &Mur| {
        let defaut = p
            .type_mur(&m.type_mur)
            .and_then(|t| t.couches.first())
            .map_or(0, |c| match c.fonction {
                FonctionCouche::Structure => 30,
                FonctionCouche::Isolation => 20,
                FonctionCouche::Finition => 10,
                FonctionCouche::Autre => 0,
            });
        (
            p.priorites_jonction.get(&m.id).copied().unwrap_or(defaut),
            m.id.clone(),
        )
    };
    let bbox = |tris: &[Tri]| {
        tris.iter().flatten().fold(
            (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(a, b), v| (a.min(*v), b.max(*v)),
        )
    };
    let (mn, mx) = bbox(&tris);
    let cle = priorite(m);
    for autre in &p.murs {
        if autre.id == m.id
            || priorite(autre) <= cle
            || (m.assemblage.is_some() && m.assemblage == autre.assemblage)
        {
            continue;
        }
        let ep = p
            .type_mur(&autre.type_mur)
            .ok_or("Type absent")?
            .epaisseur();
        let (z0, z1) = p.bas_haut(autre).ok_or("Niveau absent")?;
        // Élargir le filtre vertical si le haut est piloté par une toiture.
        if p.attaches_toitures.is_empty() && (z0 >= mx.z - 1e-5 || z1 <= mn.z + 1e-5) {
            continue;
        }
        let points = bande(&autre.axe, 0.0, autre.axe.longueur(), ep + 2.0)?;
        let (lo, hi) = points.iter().map(|p| DVec2::from_array(*p)).fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(a, b), v| (a.min(v), b.max(v)),
        );
        if lo.x >= mx.x || hi.x <= mn.x || lo.y >= mx.y || hi.y <= mn.y {
            continue;
        }
        let coupe = mur_brut(p, autre, jonctions)?;
        let (a, b) = bbox(&coupe);
        if a.cmpge(mx - DVec3::splat(1e-5)).any() || b.cmple(mn + DVec3::splat(1e-5)).any() {
            continue;
        }
        tris = crate::csg::boolean(&tris, &coupe, crate::csg::Op::Difference, 1e-5)
            .ok_or("Jonction trop complexe ; projet conservé.")?;
        if tris.is_empty() {
            break;
        }
    }
    Ok(tris)
}

pub fn ouverture(p: &Projet, o: &Ouverture) -> Result<Vec<Tri>, String> {
    Ok(ouverture_parts(p, o)?.into_iter().flat_map(|(_, t)| t).collect())
}

/// Menuiserie d'une ouverture par rôle (dormant, ouvrant, vitrage…) quand son type
/// porte une famille paramétrique ; sinon une seule part (rôle dormant).
/// Famille modélisée du type de l'ouverture.
fn famille_modelee<'a>(p: &'a Projet, o: &Ouverture) -> Option<&'a super::famille_modelee::FamilleModelee> {
    let t = o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t))?;
    t.modele.as_ref().filter(|_| o.representation.is_none())
}

/// Boîtes résolues d'une famille modélisée placée : (prisme dans le projet, boîte).
fn boites_famille(p: &Projet, o: &Ouverture, f: &super::famille_modelee::FamilleModelee) -> Result<Vec<(Vec<Tri>, super::famille_modelee::BoiteVolume)>, String> {
    let (axe, e, niveau) = hote(p, &o.mur)?;
    let z = p.niveau(&niveau).ok_or("Niveau introuvable.")?.altitude + o.allege;
    let (c, u) = repere(&axe, o.abscisse);
    let n = u.perp();
    let r = f.resoudre(o.largeur, o.hauteur, e);
    Ok(f.boites(&r)
        .into_iter()
        .map(|b| {
            let contour = [(b.min.x, b.min.y), (b.max.x, b.min.y), (b.max.x, b.max.y), (b.min.x, b.max.y)].map(|(x, y)| (c + u * x + n * y).to_array());
            (prisme(&contour, z + b.min.z, z + b.max.z), b)
        })
        .collect())
}

/// Vides de la famille modélisée de l'ouverture (None : percement rectangulaire).
fn vides_famille(p: &Projet, o: &Ouverture) -> Result<Option<Vec<Vec<Tri>>>, String> {
    let Some(f) = famille_modelee(p, o) else { return Ok(None) };
    Ok(Some(boites_famille(p, o, f)?.into_iter().filter(|(_, b)| b.vide).map(|(t, _)| t).collect()))
}

pub fn ouverture_parts(p: &Projet, o: &Ouverture) -> Result<Vec<(super::famille::Role, Vec<Tri>)>, String> {
    use super::famille::Role;
    if let Some(f) = famille_modelee(p, o) {
        let mut parts: Vec<(Role, Vec<Tri>)> = Vec::new();
        for (tris, b) in boites_famille(p, o, f)?.into_iter().filter(|(_, b)| !b.vide) {
            let role = b.materiau.role();
            match parts.iter_mut().find(|(r, _)| *r == role) {
                Some((_, t)) => t.extend(tris),
                None => parts.push((role, tris)),
            }
        }
        return Ok(parts);
    }
    let famille = o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t)).and_then(|t| t.famille.as_ref());
    match famille {
        Some(f) if o.representation.is_none() && o.genre == GenreOuverture::Fenetre => {
            let (axe, e, niveau) = hote(p, &o.mur)?;
            let z = p.niveau(&niveau).ok_or("Niveau introuvable.")?.altitude + o.allege;
            let (c, u) = repere(&axe, o.abscisse);
            let n = u.perp();
            let mut parts: Vec<(Role, Vec<Tri>)> = Vec::new();
            for b in f.pieces(o.largeur, o.hauteur, e) {
                let contour = [(b.min.x, b.min.y), (b.max.x, b.min.y), (b.max.x, b.max.y), (b.min.x, b.max.y)].map(|(x, y)| (c + u * x + n * y).to_array());
                let tris = prisme(&contour, z + b.min.z, z + b.max.z);
                match parts.iter_mut().find(|(r, _)| *r == b.role) {
                    Some((_, t)) => t.extend(tris),
                    None => parts.push((b.role, tris)),
                }
            }
            Ok(parts)
        }
        _ => Ok(vec![(Role::Dormant, ouverture_generique(p, o)?)]),
    }
}

fn ouverture_generique(p: &Projet, o: &Ouverture) -> Result<Vec<Tri>, String> {
    if let Some(m) = &o.representation {
        return maillage(m);
    }
    let (axe, e, niveau) = hote(p, &o.mur)?;
    let z = p.niveau(&niveau).ok_or("Niveau introuvable.")?.altitude + o.allege;
    if o.genre == GenreOuverture::Libre {
        return Ok(Vec::new());
    }
    let (c, u) = repere(&axe, o.abscisse);
    let n = u.perp();
    let b = 40.0_f64.min(o.largeur / 4.0).min(o.hauteur / 4.0);
    let mut tris = Vec::new();
    for (x0, x1, z0, z1) in [
        (-o.largeur / 2.0, -o.largeur / 2.0 + b, z, z + o.hauteur),
        (o.largeur / 2.0 - b, o.largeur / 2.0, z, z + o.hauteur),
        (
            -o.largeur / 2.0,
            o.largeur / 2.0,
            z + o.hauteur - b,
            z + o.hauteur,
        ),
    ] {
        let d = e.min(80.0) / 2.0;
        let contour =
            [(x0, -d), (x1, -d), (x1, d), (x0, d)].map(|(x, y)| (c + u * x + n * y).to_array());
        tris.extend(prisme(&contour, z0, z1));
    }
    if o.genre == GenreOuverture::Fenetre {
        let d = e.min(80.0) / 2.0;
        let contour = [
            (-o.largeur / 2.0, -d),
            (o.largeur / 2.0, -d),
            (o.largeur / 2.0, d),
            (-o.largeur / 2.0, d),
        ]
        .map(|(x, y)| (c + u * x + n * y).to_array());
        tris.extend(prisme(&contour, z, z + b));
    }
    Ok(tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn exemple() -> Projet {
        let mut p = Projet::nouveau("Essai");
        p.niveaux.push(Niveau {
            id: "n".into(),
            nom: "RDC".into(),
            altitude: 0.0,
            origine: None,
        });
        p.types_murs.push(TypeCompose {
            id: "t".into(),
            nom: "Mur".into(),
            couches: vec![Couche {
                epaisseur: 200.0,
                ..Default::default()
            }],
            couches_detaillees: true,
            origine: None,
        });
        p.murs.push(Mur {
            id: "m".into(),
            nom: "Mur".into(),
            type_mur: "t".into(),
            niveau: "n".into(),
            decalage_bas: 0.0,
            haut: Haut::Libre { hauteur: 3000.0 },
            axe: Axe::Droit {
                debut: [0.0, 0.0],
                fin: [4000.0, 0.0],
            },
            assemblage: None,
            phase: None,
            sous_projet: None,
            variante: None,
            origine: None,
        });
        p
    }
    fn baie(p: &mut Projet) {
        p.ouvertures.push(Ouverture {
            id: "o".into(),
            nom: "Porte".into(),
            genre: GenreOuverture::Porte,
            mur: "m".into(),
            type_ouverture: None,
            abscisse: 2000.0,
            largeur: 900.0,
            hauteur: 2100.0,
            allege: 0.0,
            representation: None,
            origine: None,
        });
    }
    #[test]
    fn mur_perce_volume_et_traversee_assemblage() {
        let mut p = exemple();
        baie(&mut p);
        let v = geometrie::volume(&mur(&p, &p.murs[0], &jonctions::coupes(&p)).unwrap());
        assert!((v - 2_022_000_000.0).abs() < 1.0, "{v}");
        p.types_murs[0].couches = vec![
            Couche {
                epaisseur: 100.0,
                ..Default::default()
            },
            Couche {
                epaisseur: 100.0,
                ..Default::default()
            },
        ];
        p.eclater_mur("m").unwrap();
        p.verifier().unwrap();
        let v: f64 = p
            .murs
            .iter()
            .map(|m| geometrie::volume(&mur(&p, m, &jonctions::coupes(&p)).unwrap()))
            .sum();
        assert!((v - 2_022_000_000.0).abs() < 1.0, "{v}");
    }
    #[test]
    fn mur_courbe_deux_sens_et_grandes_coordonnees() {
        for direct in [true, false] {
            let mut p = exemple();
            p.murs[0].axe = Axe::Arc {
                debut: [1e8 + 4000.0, 2e8],
                fin: [1e8, 2e8 + if direct { 4000.0 } else { -4000.0 }],
                centre: [1e8, 2e8],
                direct,
            };
            let tris = mur(&p, &p.murs[0], &jonctions::coupes(&p)).unwrap();
            let v = geometrie::volume(&tris);
            let attendu = std::f64::consts::FRAC_PI_2 * 4000.0 * 200.0 * 3000.0;
            assert!((v / attendu - 1.0).abs() < 0.001, "{v} {attendu}");
        }
    }
    #[test]
    fn conversion_ifc_rouvre_et_indices_invalides_refuses() {
        for fichier in ["samples/test_ifc4.ifc", "samples/test_ifc4_murs.ifc"] {
            let (p, _) = depuis_ifc::convertir(std::path::Path::new(fichier), &|_| {}).unwrap();
            let p = Projet::lire_json(&p.json().unwrap()).unwrap();
            let scene = crate::scene::depuis_natif(&p, std::path::Path::new("test.json")).unwrap();
            assert!(!scene.objects.is_empty());
        }
        let m = Maillage {
            sommets: vec![[0.0; 3]],
            triangles: vec![[0, 1, 2]],
        };
        assert!(maillage(&m).is_err());
    }
}

impl Projet {
    /// Recale une menuiserie figée quand son hôte, son niveau ou son abscisse change.
    /// Une géométrie explicitement remplacée/transformée par la commande reste intacte.
    pub fn recaler_menuiseries(&mut self, avant: &Projet) -> Result<(), String> {
        for k in 0..self.ouvertures.len() {
            let o = &self.ouvertures[k];
            let Some(old) = avant.ouvertures.iter().find(|a| a.id == o.id) else {
                continue;
            };
            let (Some(m), Some(ancien)) = (&o.representation, &old.representation) else {
                continue;
            };
            if m.sommets != ancien.sommets || m.triangles != ancien.triangles {
                continue;
            }
            let (a, _, n) = hote(avant, &old.mur)?;
            let (b, _, nn) = hote(self, &o.mur)?;
            let (pa, ua) = repere(&a, old.abscisse);
            let (pb, ub) = repere(&b, o.abscisse);
            let za = avant.niveau(&n).ok_or("Niveau absent")?.altitude + old.allege;
            let zb = self.niveau(&nn).ok_or("Niveau absent")?.altitude + o.allege;
            if pa == pb && ua == ub && za == zb {
                continue;
            }
            let mut mesh = ancien.clone();
            for p in &mut mesh.sommets {
                let d = DVec2::new(p[0], p[1]) - pa;
                let q = pb + ub * d.dot(ua) + ub.perp() * d.dot(ua.perp());
                *p = [q.x, q.y, p[2] + zb - za];
            }
            self.ouvertures[k].representation = Some(mesh);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_jonctions {
    use super::*;
    #[test]
    fn croisement_x_sans_double_volume_et_priorite() {
        let mut p = super::tests::exemple();
        let mut m = p.murs[0].clone();
        m.id = "traverse".into();
        m.axe = Axe::Droit {
            debut: [2000.0, -2000.0],
            fin: [2000.0, 2000.0],
        };
        p.murs.push(m);
        p.priorites_jonction.insert("m".into(), 100);
        let coupes = jonctions::coupes(&p);
        let v1 = geometrie::volume(&mur(&p, &p.murs[0], &coupes).unwrap());
        let v2 = geometrie::volume(&mur(&p, &p.murs[1], &coupes).unwrap());
        assert!((v1 - 2.4e9).abs() < 1.0);
        assert!((v1 + v2 - (4.8e9 - 0.12e9)).abs() < 1.0);
    }
    #[test]
    fn mur_attache_suit_la_toiture_et_se_detache() {
        let mut p = super::tests::exemple();
        let t = TypeCompose {
            id: "toit".into(),
            nom: "Toiture".into(),
            couches: vec![Couche {
                epaisseur: 200.0,
                ..Default::default()
            }],
            couches_detaillees: true,
            origine: None,
        };
        let prm = edition::ParametresDalle {
            sorte: SorteDalle::Toiture,
            type_dalle: t,
            pente: Some(35.0),
            niveau: Some("n".into()),
            decalage: 3000.0,
            phase: None,
            sous_projet: None,
            variante: None,
        };
        let id = p
            .ajouter_dalle(
                &[
                    [-500., -1500.],
                    [4500., -1500.],
                    [4500., 1500.],
                    [-500., 1500.],
                ],
                &prm,
            )
            .unwrap();
        p.attaches_toitures.insert("m".into(), id);
        let coupes = jonctions::coupes(&p);
        let v = geometrie::volume(&mur(&p, &p.murs[0], &coupes).unwrap());
        assert!(v > 2.4e9);
        p.dalles[0].decalage += 1000.0;
        let nv = geometrie::volume(&mur(&p, &p.murs[0], &coupes).unwrap());
        assert!((nv - v - 800e6).abs() < 10.0, "{v} {nv}");
        p.attaches_toitures.clear();
        assert!((geometrie::volume(&mur(&p, &p.murs[0], &coupes).unwrap()) - 2.4e9).abs() < 1.0);
    }
}
