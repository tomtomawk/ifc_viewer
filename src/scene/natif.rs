//! Adaptation du modèle natif vers la chaîne de rendu commune.
use super::*;
use crate::geom::{Part, ProductGeom};
use crate::modele::{Axe, Projet};
use crate::step::{Entity, Value};

pub fn id_natif(genre: &str, id: &str) -> u64 {
    genre.bytes().chain([0]).chain(id.bytes()).fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

pub fn depuis_natif(projet: &Projet, path: &Path) -> Result<Scene, String> {
    if !projet.ouvertures.is_empty() || !projet.dalles.is_empty() || !projet.importes.is_empty() {
        return Err("L’affichage natif prend actuellement en charge les niveaux et les murs droits. Ce projet contient d’autres éléments ; son fichier est conservé.".into());
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
    for m in &projet.murs { entite(id_natif("mur", &m.id), "IFCWALL", &m.id, &m.nom, 9)?; }
    for n in &projet.niveaux { file.entities.get_mut(&id_natif("niveau", &n.id)).unwrap().args[9] = Value::Real(n.altitude); }
    let mut model = IfcModel::new(file);
    model.length_scale = 0.001;
    for n in &projet.niveaux {
        let id = id_natif("niveau", &n.id);
        model.parent.insert(id, racine);
        model.children.entry(racine).or_default().push(id);
    }
    let mut geoms = Vec::new();
    for m in &projet.murs {
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
        let n = glam::DVec2::new(-d.y, d.x).normalize() * epaisseur * 0.5;
        let a = glam::DVec2::from_array(debut);
        let b = glam::DVec2::from_array(fin);
        let contour = [a - n, b - n, b + n, a + n];
        let points: Vec<DVec3> = [bas, haut].into_iter().flat_map(|z| contour.map(|p| DVec3::new(p.x, p.y, z) * 0.001)).collect();
        let faces = [[3,2,1,0],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]];
        let tris = faces.into_iter().flat_map(|[a,b,c,d]| [[points[a],points[b],points[c]], [points[a],points[c],points[d]]]).collect();
        let id = id_natif("mur", &m.id);
        let niveau = id_natif("niveau", &m.niveau);
        model.parent.insert(id, niveau);
        model.children.entry(niveau).or_default().push(id);
        model.type_of.insert(id, id_natif("type-mur", &m.type_mur));
        geoms.push(ProductGeom { id, parts: vec![Part { color: [0.72,0.72,0.69,1.0], tris }], lignes: Vec::new(), rendu: Vec::new() });
    }
    let mut scene = assemble(model, geoms, path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| projet.nom.clone()));
    if scene.objects.is_empty() { scene.min = Vec3::splat(-5.0); scene.max = Vec3::splat(5.0); }
    Ok(scene)
}
