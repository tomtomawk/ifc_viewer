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
    for a in &projet.assemblages { entite(id_natif("assemblage", &a.id), "IFCELEMENTASSEMBLY", &a.id, &a.nom, 9)?; }
    for m in &projet.murs { entite(id_natif("mur", &m.id), "IFCWALL", &m.id, &m.nom, 9)?; }
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
            .map(|p| glam::DVec2::from_array(p));
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
            couches_coupe.push((id, coupe_materiau(x)));
        }
        geoms.push(ProductGeom { id, parts: vec![Part { color: couleur, tris }], lignes: Vec::new(), rendu: vec![materiau.is_some().then_some(couleur)] });
    }
    let mut scene = assemble(model, geoms, path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| projet.nom.clone()));
    scene.couches_coupe.extend(couches_coupe);
    if scene.objects.is_empty() { scene.min = Vec3::splat(-5.0); scene.max = Vec3::splat(5.0); }
    Ok(scene)
}

/// Poché de coupe d'un matériau : une bande couvrant tout l'élément (plans neutres),
/// motif selon le matériau (hachures en mm du papier).
fn coupe_materiau(m: &crate::modele::Materiau) -> Vec<CoucheCoupe> {
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
    let neutre = Vec4::new(0.0, 0.0, 0.0, 1.0);
    vec![CoucheCoupe {
        plans: [neutre, neutre],
        couleur: [r, g, b, 255],
        motif: motif.map(|grilles| std::sync::Arc::new(Motif { nom: m.nom.clone(), modele: false, grilles })),
        couleur_motif: encre,
        hachure_defaut: false,
    }]
}
