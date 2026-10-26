//! Adaptation du modèle natif vers la chaîne de rendu commune.
use super::*;
use crate::geom::{Part, ProductGeom};
use crate::modele::{Axe, Projet};
use crate::step::{Entity, Value};

pub fn id_natif(genre: &str, id: &str) -> u64 {
    genre.bytes().chain([0]).chain(id.bytes()).fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

pub fn depuis_natif(projet: &Projet, path: &Path) -> Result<Scene, String> {
    if !projet.ouvertures.is_empty() || !projet.importes.is_empty() {
        return Err("L’affichage natif prend actuellement en charge les niveaux, les murs droits et les dalles. Ce projet contient d’autres éléments ; son fichier est conservé.".into());
    }
    let mut file = StepFile { schema: "Modèle natif v0".into(), entities: Default::default() };
    let mut entite = |id: u64, classe: &str, guid: &str, nom: &str, n: usize| -> Result<(), String> {
        let mut args = vec![Value::Null; n.max(3)];
        args[0] = Value::Str(guid.into());
        args[2] = Value::Str(nom.into());
        if file.entities.insert(id, Entity { ty: classe.into(), args }).is_some() { return Err("Identifiants natifs dupliqués.".into()); }
        Ok(())
    };
    let racine = id_natif("projet", "racine");
    entite(racine, "IFCPROJECT", "projet-natif", &projet.nom, 9)?;
    for n in &projet.niveaux {
        if !n.altitude.is_finite() { return Err("Altitude de niveau non valide.".into()); }
        entite(id_natif("niveau", &n.id), "IFCBUILDINGSTOREY", &n.id, &n.nom, 10)?;
    }
    for t in &projet.types_murs { entite(id_natif("type-mur", &t.id), "IFCWALLTYPE", &t.id, &t.nom, 10)?; }
    for a in &projet.assemblages { entite(id_natif("assemblage", &a.id), "IFCELEMENTASSEMBLY", &a.id, &a.nom, 9)?; }
    for m in &projet.murs { entite(id_natif("mur", &m.id), "IFCWALL", &m.id, &m.nom, 9)?; }
    for t in &projet.types_dalles { entite(id_natif("type-dalle", &t.id), "IFCSLABTYPE", &t.id, &t.nom, 10)?; }
    for d in &projet.dalles { entite(id_natif("dalle", &d.id), "IFCSLAB", &d.id, &d.nom, 9)?; }
    for n in &projet.niveaux { file.entities.get_mut(&id_natif("niveau", &n.id)).unwrap().args[9] = Value::Real(n.altitude); }
    let mut model = IfcModel::new(file);
    model.length_scale = 0.001;
    for n in &projet.niveaux {
        let id = id_natif("niveau", &n.id);
        model.parent.insert(id, racine);
        model.children.entry(racine).or_default().push(id);
    }
    // assemblages sous le niveau de leur première couche ; couches sous l'assemblage
    for a in &projet.assemblages {
        let Some(premiere) = projet.couches_assemblage(&a.id).first().map(|m| m.niveau.clone()) else { continue };
        let (id, niveau) = (id_natif("assemblage", &a.id), id_natif("niveau", &premiere));
        model.parent.insert(id, niveau);
        model.children.entry(niveau).or_default().push(id);
        model.type_of.insert(id, id_natif("type-mur", &a.type_mur));
    }
    let mut couches_coupe = Vec::new();
    let mut geoms = Vec::new();
    let jonctions = crate::modele::jonctions::coupes(projet);
    // couches d'un même assemblage consécutives (sélection d'un seul tenant)
    let mut ordre: Vec<&crate::modele::Mur> = projet.murs.iter().filter(|m| m.assemblage.is_none()).collect();
    for a in &projet.assemblages {
        ordre.extend(projet.couches_assemblage(&a.id));
    }
    ordre.extend(projet.murs.iter().filter(|m| m.assemblage.as_ref().is_some_and(|a| projet.assemblage(a).is_none_or(|x| !x.couches.contains(&m.id)))));
    for m in ordre {
        let Axe::Droit { debut, fin } = m.axe else { return Err(format!("{} : le mur courbe natif n’est pas encore affichable.", m.nom)); };
        let type_mur = projet.type_mur(&m.type_mur).ok_or_else(|| format!("{} : type de mur introuvable.", m.nom))?;
        if projet.niveau(&m.niveau).is_none() { return Err(format!("{} : niveau introuvable.", m.nom)); }
        let (bas, haut) = projet.bas_haut(m).ok_or_else(|| format!("{} : contrainte de hauteur non valide.", m.nom))?;
        let epaisseur = type_mur.epaisseur();
        if !debut.iter().chain(&fin).chain([&bas, &haut, &epaisseur]).all(|x| x.is_finite()) || epaisseur <= 0.0 || haut <= bas {
            return Err(format!("{} : dimensions non valides.", m.nom));
        }
        let d = glam::DVec2::from_array(fin) - glam::DVec2::from_array(debut);
        if d.length() < 1.0 { return Err(format!("{} : longueur inférieure à 1 mm.", m.nom)); }
        // contour en plan, extrémités raccordées aux murs voisins (angles, T)
        let contour = crate::modele::jonctions::contour(projet, m, &jonctions)
            .ok_or_else(|| format!("{} : contour non valide.", m.nom))?
            .map(glam::DVec2::from_array);
        let points: Vec<DVec3> = [bas, haut].into_iter().flat_map(|z| contour.map(|p| DVec3::new(p.x, p.y, z) * 0.001)).collect();
        let faces = [[3,2,1,0],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]];
        let tris = faces.into_iter().flat_map(|[a,b,c,d]| [[points[a],points[b],points[c]], [points[a],points[c],points[d]]]).collect();
        let id = id_natif("mur", &m.id);
        let parent = match m.assemblage.as_ref().filter(|a| projet.assemblage(a).is_some()) {
            Some(a) => id_natif("assemblage", a),
            None => id_natif("niveau", &m.niveau),
        };
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        model.type_of.insert(id, id_natif("type-mur", &m.type_mur));
        // matériau d'un mur monocouche (couche d'assemblage) : couleur et motif de coupe
        let materiau = match type_mur.couches.as_slice() {
            [c] => c.materiau.as_ref().map(|n| projet.materiau(n).cloned().unwrap_or_else(|| crate::modele::assemblage::materiau_par_defaut(n))),
            _ => None,
        };
        let couleur = materiau.as_ref().map_or([0.72, 0.72, 0.69, 1.0], |x| {
            let [r, g, b] = x.couleur.map(|c| c as f32 / 255.0);
            [r, g, b, 1.0]
        });
        if let Some(x) = &materiau {
            let neutre = Vec4::new(0.0, 0.0, 0.0, 1.0);
            couches_coupe.push((id, coupe_materiau(x, [neutre, neutre])));
        }
        geoms.push(ProductGeom { id, parts: vec![Part { color: couleur, tris }], lignes: Vec::new(), rendu: vec![materiau.is_some().then_some(couleur)] });
    }
    // dalles : un prisme par couche, du dessus (niveau + décalage) vers le bas
    let mut bandes_dalles: Vec<(u64, Vec<CoucheCoupe>)> = Vec::new();
    for d in &projet.dalles {
        let t = projet.type_dalle(&d.type_dalle).ok_or_else(|| format!("{} : type de dalle introuvable.", d.nom))?;
        let alt = projet.niveau(&d.niveau).ok_or_else(|| format!("{} : niveau introuvable.", d.nom))?.altitude;
        let id = id_natif("dalle", &d.id);
        let niveau = id_natif("niveau", &d.niveau);
        model.parent.insert(id, niveau);
        model.children.entry(niveau).or_default().push(id);
        model.type_of.insert(id, id_natif("type-dalle", &d.type_dalle));
        let mut z = alt + d.decalage;
        let (mut parts, mut rendu, mut bandes) = (Vec::new(), Vec::new(), Vec::new());
        for c in &t.couches {
            let (haut, bas) = (z, z - c.epaisseur);
            z = bas;
            let tris = prisme(&d.contours, bas, haut);
            if tris.is_empty() { continue; }
            let m = c.materiau.as_ref().map(|n| projet.materiau(n).cloned().unwrap_or_else(|| crate::modele::assemblage::materiau_par_defaut(n)));
            let couleur = m.as_ref().map_or([0.70, 0.70, 0.68, 1.0], |x| {
                let [r, g, b] = x.couleur.map(|c| c as f32 / 255.0);
                [r, g, b, 1.0]
            });
            if let Some(m) = &m {
                // bande de coupe entre le dessous et le dessus de la couche (m, repère du fichier)
                let plans = [Vec4::new(0.0, 0.0, -1.0, (haut * 0.001) as f32), Vec4::new(0.0, 0.0, 1.0, -(bas * 0.001) as f32)];
                bandes.extend(coupe_materiau(m, plans));
            }
            rendu.push(m.is_some().then_some(couleur));
            parts.push(Part { color: couleur, tris });
        }
        if parts.is_empty() {
            return Err(format!("{} : contour non valide.", d.nom));
        }
        if !bandes.is_empty() { bandes_dalles.push((id, bandes)); }
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu });
    }
    let mut scene = assemble(model, geoms, path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| projet.nom.clone()));
    scene.couches_coupe.extend(couches_coupe);
    // plans des bandes ramenés au repère de la scène (origine retirée)
    let o = scene.origin;
    for (id, mut bandes) in bandes_dalles {
        for b in &mut bandes {
            for pl in &mut b.plans {
                pl.w += (pl.truncate().as_dvec3().dot(o)) as f32;
            }
        }
        scene.couches_coupe.insert(id, bandes);
    }
    if scene.objects.is_empty() { scene.min = Vec3::splat(-5.0); scene.max = Vec3::splat(5.0); }
    Ok(scene)
}

/// Prisme vertical (m) d'un contour pair-impair (mm) entre `bas` et `haut` (mm) :
/// dessus et dessous triangulés, une face par arête.
fn prisme(contours: &[Vec<[f64; 2]>], bas: f64, haut: f64) -> Vec<[DVec3; 3]> {
    let Some(&o) = contours.first().and_then(|c| c.first()) else { return Vec::new() };
    // triangulation en coordonnées locales (f32) : précision gardée loin de l'origine
    let locaux: Vec<Vec<[f32; 2]>> = contours.iter().map(|c| c.iter().map(|p| [(p[0] - o[0]) as f32, (p[1] - o[1]) as f32]).collect()).collect();
    let p3 = |p: [f32; 2], z: f64| DVec3::new(p[0] as f64 + o[0], p[1] as f64 + o[1], z) * 0.001;
    let mut tris = Vec::new();
    for t in crate::texte::trianguler(locaux.clone()) {
        // orientation : normales vers le haut au-dessus, vers le bas au-dessous
        let a = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0]);
        let t = if a < 0.0 { [t[0], t[2], t[1]] } else { t };
        tris.push([p3(t[0], haut), p3(t[1], haut), p3(t[2], haut)]);
        tris.push([p3(t[0], bas), p3(t[2], bas), p3(t[1], bas)]);
    }
    if tris.is_empty() {
        return tris;
    }
    for c in contours {
        let sens: f64 = (0..c.len()).map(|i| c[i][0] * c[(i + 1) % c.len()][1] - c[(i + 1) % c.len()][0] * c[i][1]).sum();
        for i in 0..c.len() {
            let (mut a, mut b) = (c[i], c[(i + 1) % c.len()]);
            if sens < 0.0 {
                std::mem::swap(&mut a, &mut b);
            }
            let q = |p: [f64; 2], z: f64| DVec3::new(p[0], p[1], z) * 0.001;
            tris.push([q(a, bas), q(b, bas), q(b, haut)]);
            tris.push([q(a, bas), q(b, haut), q(a, haut)]);
        }
    }
    tris
}

/// Poché de coupe d'un matériau entre deux plans (neutres : tout l'élément), motif
/// selon le matériau (hachures en mm du papier).
fn coupe_materiau(m: &crate::modele::Materiau, plans: [Vec4; 2]) -> Vec<CoucheCoupe> {
    use crate::modele::MotifCoupe;
    use crate::rvt::vues::{GrilleMotif, Motif};
    let grille = |angle: f64, pas: f64| GrilleMotif { angle, origine: [0.0, 0.0], glissement: 0.0, decalage: pas, tirets: Vec::new() };
    let [r, g, b] = m.couleur;
    let (motif, encre) = match m.motif {
        MotifCoupe::Diagonales => (Some(vec![grille(std::f64::consts::FRAC_PI_4, 2.0)]), [70, 70, 70, 255]),
        MotifCoupe::Croisees => (Some(vec![grille(std::f64::consts::FRAC_PI_4, 1.5), grille(-std::f64::consts::FRAC_PI_4, 1.5)]), [110, 95, 40, 255]),
        // sans grille : aplat de la couleur du motif
        MotifCoupe::Uni => (Some(Vec::new()), [r, g, b, 255]),
        MotifCoupe::Vide => (None, [70, 70, 70, 255]),
    };
    vec![CoucheCoupe {
        plans,
        couleur: [r, g, b, 255],
        motif: motif.map(|grilles| std::sync::Arc::new(Motif { nom: m.nom.clone(), modele: false, grilles })),
        couleur_motif: encre,
        hachure_defaut: false,
    }]
}
