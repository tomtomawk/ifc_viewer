//! Adaptation du modèle natif vers la chaîne de rendu commune.
use super::*;
use crate::geom::{Part, ProductGeom};
use crate::modele::{Axe, Projet, SorteDalle};
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
    for d in &projet.dalles {
        let classe = match d.sorte {
            SorteDalle::Dalle => "IFCSLAB",
            SorteDalle::Sol | SorteDalle::Plafond => "IFCCOVERING",
            SorteDalle::Toiture => "IFCROOF",
        };
        entite(id_natif("dalle", &d.id), classe, &d.id, &d.nom, 9)?;
    }
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
    // matériau de chaque partie des éléments (jonctions sans arête entre mêmes matériaux)
    let mut materiaux_parts: Vec<Vec<Option<String>>> = Vec::new();
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
            couches_coupe.push((id, coupe_materiau(x, vec![neutre, neutre])));
        }
        materiaux_parts.push(vec![materiau.as_ref().map(|m| m.nom.clone())]);
        geoms.push(ProductGeom { id, parts: vec![Part { color: couleur, tris }], lignes: Vec::new(), rendu: vec![materiau.is_some().then_some(couleur)], motifs: vec![materiau.as_ref().and_then(motif_surface)] });
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
        // couches du haut vers le bas, décalées de la surface de référence (sous-face ou
        // dessus, en pente si des arêtes en définissent une)
        let plans = crate::modele::pente::plans(d);
        let z0 = alt + d.decalage;
        let (_, mut e) = d.decalages_couches(t.epaisseur());
        let (mut parts, mut rendu, mut bandes, mut noms, mut motifs) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for c in &t.couches {
            let (haut, bas) = (e, e - c.epaisseur);
            e = bas;
            let tris = crate::modele::pente::couche(&d.contours, &plans, z0, bas, haut);
            if tris.is_empty() { continue; }
            let m = c.materiau.as_ref().map(|n| projet.materiau(n).cloned().unwrap_or_else(|| crate::modele::assemblage::materiau_par_defaut(n)));
            let couleur = m.as_ref().map_or([0.70, 0.70, 0.68, 1.0], |x| {
                let [r, g, b] = x.couleur.map(|c| c as f32 / 255.0);
                [r, g, b, 1.0]
            });
            if let Some(m) = &m {
                // bandes de coupe entre le dessous et le dessus de la couche (m, repère du fichier)
                for jeu in crate::modele::pente::bandes(&plans, z0, bas, haut) {
                    bandes.extend(coupe_materiau(m, jeu));
                }
            }
            rendu.push(m.is_some().then_some(couleur));
            noms.push(m.as_ref().map(|m| m.nom.clone()));
            motifs.push(m.as_ref().and_then(motif_surface));
            parts.push(Part { color: couleur, tris });
        }
        if parts.is_empty() {
            return Err(format!("{} : contour non valide.", d.nom));
        }
        if !bandes.is_empty() { bandes_dalles.push((id, bandes)); }
        materiaux_parts.push(noms);
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu, motifs });
    }
    let aretes = aretes_jointes(&geoms, &materiaux_parts);
    let mut scene = crate::scene::assemble_avec(model, geoms, path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| projet.nom.clone()), aretes);
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
    scene.navigateur = Some(navigateur(projet));
    Ok(scene)
}

/// Motifs de surface d'un matériau pour le rendu : [premier plan | arrière-plan << 4,
/// couleur du premier plan]. Premier plan : 0 aucun, 1 diagonales, 2 croisées, 3 uni ;
/// arrière-plan : 0 uni (couleur des faces), 1 diagonales, 2 croisées, 4 aucun.
fn motif_surface(m: &crate::modele::Materiau) -> Option<[u8; 4]> {
    use crate::modele::MotifCoupe;
    let premier = match m.surface_motif {
        Some(MotifCoupe::Diagonales) => 1,
        Some(MotifCoupe::Croisees) => 2,
        Some(MotifCoupe::Uni) => 3,
        _ => 0,
    };
    let arriere = match m.fond_surface() {
        MotifCoupe::Uni => 0,
        MotifCoupe::Diagonales => 1,
        MotifCoupe::Croisees => 2,
        MotifCoupe::Vide => 4,
    };
    if premier == 0 && arriere == 0 {
        return None;
    }
    let [r, g, b] = m.encre_surface();
    Some([premier | (arriere << 4), r, g, b])
}

/// Arêtes des éléments calculées par matériau : là où deux éléments d'un même matériau se
/// joignent (murs raccordés, dalle sous un mur), pas d'arête sur les faces qui se
/// prolongent ; les parties sans matériau restent à part.
fn aretes_jointes(geoms: &[ProductGeom], materiaux: &[Vec<Option<String>>]) -> std::collections::HashMap<Id, crate::dessin::Aretes> {
    use std::collections::HashMap;
    let mut groupes: HashMap<String, Vec<([DVec3; 3], usize)>> = HashMap::new();
    for (k, (g, noms)) in geoms.iter().zip(materiaux).enumerate() {
        for (i, p) in g.parts.iter().enumerate() {
            let cle = noms.get(i).cloned().flatten().map_or_else(|| format!("\u{0}{k}:{i}"), |n| n);
            groupes.entry(cle).or_default().extend(p.tris.iter().map(|t| (*t, k)));
        }
    }
    let mut par_objet: HashMap<Id, crate::dessin::Aretes> = geoms.iter().map(|g| (g.id, crate::dessin::Aretes::default())).collect();
    let resultats: Vec<_> = groupes.into_par_iter().map(|(_, tris)| crate::dessin::aretes_jointes(&tris)).collect();
    for (vives, douces) in resultats {
        for (s, k) in vives {
            if let Some(a) = par_objet.get_mut(&geoms[k].id) { a.vives.push(s); }
        }
        for (d, k) in douces {
            if let Some(a) = par_objet.get_mut(&geoms[k].id) { a.douces.push(d); }
        }
    }
    par_objet
}

/// Identifiant numérique stable d'une vue native dans l'arborescence (« 3d » : la vue 3D).
pub fn numero_vue(id: &str) -> i64 {
    (id_natif("vue", id) >> 1) as i64
}

/// Arborescence du projet natif : vue 3D, vues enregistrées (plans d'étage, coupes),
/// types de murs et de dalles, matériaux et éléments qui les portent.
fn navigateur(projet: &Projet) -> Navigateur {
    use crate::modele::GenreVue;
    let num = numero_vue;
    let vue = |revit_id: i64, genre: &str, nom: &str, echelle: f64| NavVue {
        revit_id,
        phase: 0,
        filtre_phase: -1,
        coupe: None,
        plan_coupe: None,
        cadrage: Vec::new(),
        tableau: None,
        entite: None,
        genre: genre.into(),
        type_vue: String::new(),
        nom: nom.into(),
        echelle,
        detail: 2,
        numero: String::new(),
        haut: DVec3::Z,
        direction: DVec3::new(-1.0, -1.0, 1.0).normalize(),
        etage: None,
        placees: Vec::new(),
        fenetres: Vec::new(),
        primitives: Vec::new(),
    };
    let mut vues = vec![vue(num("3d"), "Vue 3D", "{3D}", 100.0)];
    for v in &projet.vues {
        let mut n = vue(num(&v.id), "", &v.nom, v.echelle);
        n.type_vue = v.type_vue.clone();
        match &v.genre {
            GenreVue::Plan { niveau, coupe } => {
                n.genre = "Plan d'étage".into();
                n.etage = Some(id_natif("niveau", niveau));
                n.coupe = Some(coupe / 1000.0);
                n.direction = DVec3::Z;
                n.haut = DVec3::Y;
            }
            GenreVue::Coupe { debut, fin, profondeur, elevation } => {
                n.genre = if *elevation { "Élévation" } else { "Coupe" }.into();
                let (a, b) = (glam::DVec2::from_array(*debut) / 1000.0, glam::DVec2::from_array(*fin) / 1000.0);
                let u = (b - a).normalize_or_zero();
                let gauche = u.perp();
                // l'observateur est à droite de la ligne et regarde vers sa gauche
                n.direction = -gauche.extend(0.0);
                n.haut = DVec3::Z;
                n.plan_coupe = Some(gauche.extend(0.0).extend(-gauche.dot(a)));
                // profondeur, puis les deux bouts de la ligne
                let plan = |n: glam::DVec2, p: glam::DVec2| n.extend(0.0).extend(-n.dot(p));
                n.cadrage = vec![plan(-gauche, a + gauche * (profondeur / 1000.0)), plan(u, a), plan(-u, b)];
            }
        }
        vues.push(n);
    }
    let types = |categorie: &str, nom: &str, ts: &[crate::modele::TypeCompose], genre: &str| NavFamille {
        categorie: categorie.into(),
        nom: nom.into(),
        types: ts.iter().map(|t| (t.nom.clone(), Some(id_natif(genre, &t.id)))).collect(),
    };
    let mut familles = Vec::new();
    if !projet.types_murs.is_empty() {
        familles.push(types("Murs", "Mur de base", &projet.types_murs, "type-mur"));
    }
    // types d'éléments horizontaux rangés par catégorie de leurs éléments (sols par défaut)
    for (categorie, famille) in [("Sols", "Sol"), ("Plafonds", "Plafond composé"), ("Toits", "Toit de base")] {
        let ts: Vec<crate::modele::TypeCompose> = projet
            .types_dalles
            .iter()
            .filter(|t| projet.dalles.iter().find(|d| d.type_dalle == t.id).map_or("Sols", |d| d.sorte.categorie()) == categorie)
            .cloned()
            .collect();
        if !ts.is_empty() {
            familles.push(types(categorie, famille, &ts, "type-dalle"));
        }
    }
    let materiaux = projet
        .materiaux
        .iter()
        .map(|m| {
            let porte = |t: Option<&crate::modele::TypeCompose>| t.is_some_and(|t| t.couches.iter().any(|c| c.materiau.as_deref() == Some(m.nom.as_str())));
            let mut elements: Vec<Id> = projet.murs.iter().filter(|x| porte(projet.type_mur(&x.type_mur))).map(|x| id_natif("mur", &x.id)).collect();
            elements.extend(projet.dalles.iter().filter(|x| porte(projet.type_dalle(&x.type_dalle))).map(|x| id_natif("dalle", &x.id)));
            NavMateriau { nom: m.nom.clone(), couleur: m.couleur, transparence: 0.0, elements }
        })
        .collect();
    Navigateur {
        phases: Vec::new(),
        filtres_phase: Default::default(),
        phases_objets: Default::default(),
        variantes_secondaires: Default::default(),
        vues,
        familles,
        groupes: Vec::new(),
        materiaux,
    }
}

/// Poché de coupe d'un matériau entre deux plans (neutres : tout l'élément), motif
/// selon le matériau (hachures en mm du papier).
fn coupe_materiau(m: &crate::modele::Materiau, plans: Vec<Vec4>) -> Vec<CoucheCoupe> {
    use crate::modele::MotifCoupe;
    use crate::rvt::vues::{GrilleMotif, Motif};
    let grille = |angle: f64, pas: f64| GrilleMotif { angle, origine: [0.0, 0.0], glissement: 0.0, decalage: pas, tirets: Vec::new() };
    let [r, g, b] = m.couleur;
    let grilles = |x: MotifCoupe| match x {
        MotifCoupe::Diagonales => Some(vec![grille(std::f64::consts::FRAC_PI_4, 2.0)]),
        MotifCoupe::Croisees => Some(vec![grille(std::f64::consts::FRAC_PI_4, 1.5), grille(-std::f64::consts::FRAC_PI_4, 1.5)]),
        // sans grille : aplat de la couleur du motif
        MotifCoupe::Uni => Some(Vec::new()),
        MotifCoupe::Vide => None,
    };
    let motif = grilles(m.motif);
    let [er, eg, eb] = m.encre_coupe();
    let (motif_fond, [fr, fg, fb]) = m.fond_coupe();
    let encre = [er, eg, eb, 255];
    vec![CoucheCoupe {
        plans,
        couleur: [r, g, b, 255],
        fond: [fr, fg, fb, 255],
        motif_fond: motif_fond.and_then(grilles).map(|grilles| std::sync::Arc::new(Motif { nom: format!("{} (arrière-plan)", m.nom), modele: false, grilles })),
        motif: motif.map(|grilles| std::sync::Arc::new(Motif { nom: m.nom.clone(), modele: false, grilles })),
        couleur_motif: encre,
        hachure_defaut: false,
    }]
}
