//! Fonctions d'aide de l'interface : icônes vectorielles, libellés, catégories IFC.
use super::*;

/// Icône d'un style visuel : petit cube en perspective, tracé dans `r`.
/// Icône du niveau de détail : un, deux ou trois traits, comme dans Revit.
/// Icônes vectorielles : indépendantes des caractères disponibles dans la police.
pub(super) fn icone_theme(p:&egui::Painter,r:egui::Rect,c:Color32) {
    let centre=r.center();let rayon=r.width().min(r.height())*0.43;
    let mut demi=vec![centre];
    for k in 0..=24 {let a=-std::f32::consts::FRAC_PI_2 + std::f32::consts::PI*k as f32/24.;demi.push(centre+egui::vec2(a.cos(),a.sin())*rayon);}
    p.add(egui::Shape::convex_polygon(demi,c,egui::Stroke::NONE));
    p.circle_stroke(centre,rayon,egui::Stroke::new(1.4,c));
}

pub(super) fn icone_soleil(p:&egui::Painter,r:egui::Rect,c:Color32) {
    let centre=r.center();let rayon=r.width().min(r.height())*0.22;
    p.circle_stroke(centre,rayon,egui::Stroke::new(1.3,c));
    for k in 0..8 {let a=k as f32*std::f32::consts::TAU/8.;let d=egui::vec2(a.cos(),a.sin());p.line_segment([centre+d*(rayon+2.),centre+d*(rayon+4.)],egui::Stroke::new(1.3,c));}
}

pub(super) fn icone_detail(p: &egui::Painter, r: egui::Rect, d: NiveauDetail) {
    let n = match d {
        NiveauDetail::Faible => 1,
        NiveauDetail::Moyen => 2,
        NiveauDetail::Eleve => 3,
    };
    let t = egui::Stroke::new(1.4, Color32::from_gray(50));
    p.rect_stroke(r, 1.0, egui::Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
    for k in 0..n {
        let y = r.top() + r.height() * (k as f32 + 1.0) / (n as f32 + 1.0);
        p.line_segment([egui::pos2(r.left() + 3.0, y), egui::pos2(r.right() - 3.0, y)], t);
    }
}

pub(super) fn icone_style(p: &egui::Painter, r: egui::Rect, style: StyleVisuel) {
    use egui::{Stroke, pos2};
    let c = r.center();
    let k = r.width() / 4.2;
    // perspective fixe : vu depuis l'avant gauche, en plongée
    let pt = |x: f32, y: f32, z: f32| pos2(c.x + (x - y) * 0.87 * k, c.y - (z * 1.0 + (x + y) * 0.45) * k);
    let encre = Color32::from_gray(50);
    let t = Stroke::new(1.1, encre);
    let faces = [
        ([pt(-1., -1., 1.), pt(1., -1., 1.), pt(1., 1., 1.), pt(-1., 1., 1.)], 0),
        ([pt(-1., -1., -1.), pt(1., -1., -1.), pt(1., -1., 1.), pt(-1., -1., 1.)], 1),
        ([pt(-1., -1., -1.), pt(-1., 1., -1.), pt(-1., 1., 1.), pt(-1., -1., 1.)], 2),
    ];
    let remplissage = |i: usize| -> Option<Color32> {
        match style {
            StyleVisuel::Graphique => Some(Color32::from_gray([235, 185, 150][i])),
            StyleVisuel::Realiste => Some([Color32::from_rgb(232, 196, 150), Color32::from_rgb(200, 150, 100), Color32::from_rgb(165, 118, 75)][i]),
        }
    };
    for (coins, i) in faces {
        let trait_ = if style == StyleVisuel::Realiste { Stroke::NONE } else { t };
        match remplissage(i) {
            Some(f) => p.add(egui::Shape::convex_polygon(coins.to_vec(), f, trait_)),
            None => p.add(egui::Shape::closed_line(coins.to_vec(), t)),
        };
    }
}

/// Texte gravé au milieu d'une face du cube d'orientation (normale `n`, axes `u`, `v`
/// dans le plan, côté du cube = 2) : capitales de `capitales`, largeur au plus 1,7, centré ;
/// maillage egui des glyphes projetés par `ecran`.
pub(super) fn texte_sur_face(texte: &str, n: Vec3, u: Vec3, v: Vec3, ecran: &dyn Fn(Vec3) -> egui::Pos2, couleur: Color32, capitales: f64) -> egui::Shape {
    const POLICE: &str = "Arial";
    let largeur = crate::noyau::texte::largeur(texte, POLICE, 1.0, true).max(1e-6);
    let taille = capitales.min(1.7 / largeur);
    let l = crate::noyau::texte::ligne(texte, POLICE, taille, true);
    let (dx, dy) = (-l.largeur / 2.0, -taille / 2.0);
    let mut m = egui::Mesh::default();
    for t in &l.triangles {
        let base = m.vertices.len() as u32;
        for p in t {
            let q = n + u * (p[0] + dx) as f32 + v * (p[1] + dy) as f32;
            m.colored_vertex(ecran(q), couleur);
        }
        m.add_triangle(base, base + 1, base + 2);
    }
    egui::Shape::mesh(m)
}

/// Matériaux d'un modèle IFC (`IfcMaterial`) et éléments qui les portent (association
/// directe ou par le type, couches, profils, constituants).
pub(super) fn materiaux_ifc(s: &Scene) -> Vec<scene::NavMateriau> {
    let m = &s.model;
    let mut usages: BTreeMap<Id, Vec<Id>> = BTreeMap::new();
    for o in &s.objects {
        let assoc = m.material_of.get(&o.id).or_else(|| m.material_of.get(m.type_of.get(&o.id)?));
        let mut ids = assoc.map(|a| m.material_ids(*a)).unwrap_or_default();
        ids.sort_unstable();
        ids.dedup();
        for k in ids {
            usages.entry(k).or_default().push(o.id);
        }
    }
    let mut out: Vec<scene::NavMateriau> = m
        .file
        .entities
        .iter()
        .filter(|(_, e)| e.is("IFCMATERIAL"))
        .map(|(&id, e)| {
            let c = m.material_rgba(id).unwrap_or([0.7, 0.7, 0.7, 1.0]);
            let octet = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
            scene::NavMateriau { nom: e.arg(0).display(), couleur: [octet(c[0]), octet(c[1]), octet(c[2])], transparence: 1.0 - c[3] as f64, elements: usages.remove(&id).unwrap_or_default() }
        })
        .collect();
    out.sort_by_key(|a| a.nom.to_lowercase());
    out
}

/// Niveaux du modèle : entité, nom, altitude dans le repère de la scène (élévation de
/// l'IfcBuildingStorey), du plus bas au plus haut.
pub(super) fn niveaux(s: &Scene) -> Vec<(Id, String, f32)> {
    let mut out: Vec<(Id, String, f32)> = s
        .model
        .file
        .entities
        .iter()
        .filter(|(_, e)| &*e.ty == "IFCBUILDINGSTOREY")
        .filter_map(|(&id, e)| Some((id, s.model.name(id), (e.arg(9).as_f64()? * s.model.length_scale - s.origin.z) as f32)))
        .collect();
    out.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
    out
}

/// Fichier Revit, reconnu à l'extension ou à la signature OLE.
/// Classe IFC d'une clé de masquage (« classe » ou « classe:nom »).
pub(super) fn classe_cle(cle: &str) -> &str {
    cle.split_once(':').map_or(cle, |c| c.0)
}

/// Catégories de visibilité de la scène, puis catégorie et clé de masquage de chaque
/// objet (voir [`Categorie`]) : genre (`ObjectType`) des annotations ; catégorie Revit
/// (propriété « Catégorie ») des proxys d'un fichier Revit ; nom de la classe IFC
/// sinon. Les quadrillages sont des annotations, comme dans Revit.
pub(super) fn categories(s: &Scene) -> (Vec<Categorie>, Vec<usize>, Vec<Arc<str>>) {
    let revit = s.navigateur.is_some();
    let noms: Vec<(bool, String)> = s
        .objects
        .iter()
        .map(|o| match &*o.class {
            "IFCANNOTATION" => (true, nom_genre(s.model.get(o.id).and_then(|e| e.arg(4).as_str()).unwrap_or(""))),
            "IFCBUILDINGELEMENTPROXY" if revit => {
                let cat = s.model.property_groups(o.id).into_iter().filter(|g| g.name == "Revit").flat_map(|g| g.props).find(|p| p.0 == "Catégorie");
                (false, cat.map_or_else(|| nom_classe(&o.class), |p| p.1))
            }
            c => (c == "IFCGRID", nom_classe(c)),
        })
        .collect();
    // classes à plusieurs catégories : une clé « classe:nom » par catégorie
    let mut par_classe: BTreeMap<&str, HashSet<&str>> = BTreeMap::new();
    for (o, (_, n)) in s.objects.iter().zip(&noms) {
        par_classe.entry(&o.class).or_default().insert(n);
    }
    let cles: Vec<Arc<str>> = s.objects.iter().zip(&noms).map(|(o, (_, n))| if par_classe[&*o.class].len() > 1 { format!("{}:{n}", o.class).into() } else { o.class.clone() }).collect();
    let mut groupes: BTreeMap<(bool, &str), Vec<Arc<str>>> = BTreeMap::new();
    for ((a, n), cle) in noms.iter().zip(&cles) {
        let g = groupes.entry((*a, n)).or_default();
        if !g.contains(cle) {
            g.push(cle.clone());
        }
    }
    let index: BTreeMap<(bool, &str), usize> = groupes.keys().enumerate().map(|(k, g)| (*g, k)).collect();
    let categorie_objet = noms.iter().map(|(a, n)| index[&(*a, n.as_str())]).collect();
    let categories = groupes.into_iter().map(|((annotation, nom), cles)| Categorie { annotation, nom: nom.to_string(), cles }).collect();
    (categories, categorie_objet, cles)
}

/// Nom de catégorie (au pluriel, comme dans Revit) d'une classe IFC d'élément de modèle.
pub(super) fn nom_classe(classe: &str) -> String {
    let nom = match classe {
        "IFCWALL" | "IFCWALLSTANDARDCASE" | "IFCWALLELEMENTEDCASE" => "Murs",
        "IFCWINDOW" | "IFCWINDOWSTANDARDCASE" => "Fenêtres",
        "IFCDOOR" | "IFCDOORSTANDARDCASE" => "Portes",
        "IFCSLAB" | "IFCSLABSTANDARDCASE" | "IFCSLABELEMENTEDCASE" => "Sols",
        "IFCCOVERING" => "Plafonds et revêtements",
        "IFCROOF" => "Toits",
        "IFCSTAIR" | "IFCSTAIRFLIGHT" => "Escaliers",
        "IFCRAMP" | "IFCRAMPFLIGHT" => "Rampes",
        "IFCRAILING" => "Garde-corps",
        "IFCCOLUMN" | "IFCCOLUMNSTANDARDCASE" => "Poteaux",
        "IFCBEAM" | "IFCBEAMSTANDARDCASE" => "Ossature",
        "IFCMEMBER" | "IFCMEMBERSTANDARDCASE" => "Meneaux et membrures",
        "IFCPLATE" | "IFCPLATESTANDARDCASE" => "Panneaux de murs-rideaux",
        "IFCCURTAINWALL" => "Murs-rideaux",
        "IFCFOOTING" => "Fondations",
        "IFCFURNITURE" | "IFCFURNISHINGELEMENT" => "Mobilier",
        "IFCSANITARYTERMINAL" => "Appareils sanitaires",
        "IFCBUILDINGELEMENTPROXY" => "Modèles génériques",
        "IFCSPACE" => "Pièces",
        "IFCGEOGRAPHICELEMENT" => "Topographie",
        "IFCSITE" => "Site",
        "IFCOPENINGELEMENT" | "IFCOPENINGSTANDARDCASE" => "Ouvertures",
        "IFCSURFACEFEATURE" => "Motifs de surface",
        "IFCVIRTUALELEMENT" => "Éléments virtuels",
        "IFCGRID" => "Quadrillages",
        _ => return pretty_type(classe).trim_start_matches("Ifc").to_string(),
    };
    nom.to_string()
}

/// Nom de catégorie d'un genre d'annotation (éléments 2D d'une vue Revit).
pub(super) fn nom_genre(genre: &str) -> String {
    match genre {
        "" => "Annotations",
        "Trait" => "Lignes",
        "Texte" => "Textes",
        "Cote" => "Cotes",
        "Région remplie" => "Régions remplies",
        "Étiquette" => "Étiquettes",
        "Fenêtre de vue" => "Fenêtres de vue",
        "Titre de vue" => "Titres de vue",
        "Symbole" => "Symboles",
        "Cartouche" => "Cartouches",
        "Composant de détail" => "Composants de détail",
        g => g,
    }
    .to_string()
}

pub(super) fn is_revit_file(path: &std::path::Path) -> bool {
    if rvt::is_revit_path(path) {
        return true;
    }
    let mut head = [0u8; 8];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && rvt::is_cfb(&head)
}

pub(super) fn decode_png(data: &[u8]) -> Option<egui::ColorImage> {
    let (w, h, rgba) = rvt::decode_png_rgba(data)?;
    Some(egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

pub(super) fn load_state_fr(s: &str) -> &str {
    match s {
        "Loaded" => "Chargé",
        "Not Found" => "Introuvable",
        "Unloaded" => "Déchargé",
        "NotLoaded" | "Not Loaded" => "Non chargé",
        _ => s,
    }
}

/// Aperçu d'un motif de hachures dans `r` : traits du motif, `px_mm` pixels par mm du papier.
pub(super) fn apercu_motif(p: &egui::Painter, r: egui::Rect, d: &crate::modele::DefinitionMotif, px_mm: f32) {
    use crate::formats::rvt::vues::{hachures_plan, GrilleMotif, Motif};
    let m = Motif {
        nom: d.nom.clone(),
        modele: false,
        grilles: d.lignes.iter().map(|l| GrilleMotif { angle: l.angle.to_radians(), origine: l.origine, glissement: l.glissement, decalage: l.pas, tirets: l.tirets.iter().map(|t| t.abs()).collect() }).collect(),
    };
    let (w, h) = ((r.width() / px_mm) as f64, (r.height() / px_mm) as f64);
    let tris = [[[0.0, 0.0], [w, 0.0], [w, h]], [[0.0, 0.0], [w, h], [0.0, h]]];
    let p = p.with_clip_rect(r);
    for [a, b] in hachures_plan(&tris, &m, 1.0, 4_000) {
        let q = |x: [f64; 2]| egui::pos2(r.left() + x[0] as f32 * px_mm, r.bottom() - x[1] as f32 * px_mm);
        p.line_segment([q(a), q(b)], egui::Stroke::new(1.0, Color32::from_gray(40)));
    }
}
