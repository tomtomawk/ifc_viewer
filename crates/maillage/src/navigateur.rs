//! Arborescence du projet natif (vues, familles, matériaux) pour le navigateur.
use glam::DVec3;
use modele::identifiants::{id_natif, numero_vue};
use modele::Projet;
use noyau::Id;
use scene::*;

/// Arborescence du projet natif : vue 3D, vues enregistrées (plans d'étage, coupes),
/// types de murs et de dalles, matériaux et éléments qui les portent.
pub(crate) fn navigateur(projet: &Projet) -> Navigateur {
    use modele::GenreVue;
    let num = numero_vue;
    let vue = |revit_id: i64, genre: &str, nom: &str, echelle: f64| NavVue {
        revit_id,
        phase: 0,
        filtre_phase: -1,
        coupe: None,
        limite_basse: None,
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
    let mut vues = vec![vue(num("3d"), "Vue 3D", "Vue 3D", 100.0)];
    for v in &projet.vues {
        let mut n = vue(num(&v.id), "", &v.nom, v.echelle);
        n.type_vue = v.type_vue.clone();
        match &v.genre {
            GenreVue::Plan { niveau, coupe, bas_niveau, bas_decalage } => {
                n.limite_basse = projet.niveau(bas_niveau).map(|l| (l.altitude + bas_decalage) / 1000.0);
                n.genre = "Plan d'étage".into();
                n.etage = Some(id_natif("niveau", niveau));
                n.coupe = Some(coupe / 1000.0);
                n.direction = DVec3::Z;
                let (sin, cos) = v.orientation_plan.sin_cos();
                n.haut = DVec3::new(-sin, cos, 0.0);
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
            // vue caméra : vue 3D (point de vue appliqué à l'ouverture)
            GenreVue::Camera { oeil, cible, .. } => {
                n.genre = "Vue 3D".into();
                n.direction = -(glam::DVec3::from_array(*cible) - glam::DVec3::from_array(*oeil)).normalize_or(DVec3::Y);
            }
        }
        vues.push(n);
    }
    // jeux de feuilles : une vue par jeu (toutes ses feuilles à la suite) ; identifiant
    // tiré du rang du jeu, stable si on le renomme
    for (k, jeu) in modele::documentation::jeux(projet).iter().enumerate() {
        vues.push(vue(num(&format!("jeu:{k}")), "Jeu de feuilles", jeu, 1.0));
    }
    let types = |categorie: &str, nom: &str, ts: &[modele::TypeCompose], genre: &str| NavFamille {
        categorie: categorie.into(),
        nom: nom.into(),
        types: ts.iter().map(|t| (t.nom.clone(), Some(id_natif(genre, &t.id)))).collect(),
    };
    let mut familles = Vec::new();
    if !projet.types_murs.is_empty() {
        familles.push(types("Murs", "Mur de base", &projet.types_murs, "type-mur"));
    }
    // types d'éléments horizontaux rangés par catégorie de leurs éléments (sols par défaut)
    for (categorie, famille) in [("Sols", "Sol"), ("Plafonds", "Plafond composé"), ("Toits", "Toit de base"), ("Terre-pleins", "Terre-plein")] {
        let ts: Vec<modele::TypeCompose> = projet
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
            let porte = |t: Option<&modele::TypeCompose>| t.is_some_and(|t| t.couches.iter().any(|c| c.materiau.as_deref() == Some(m.nom.as_str())));
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
