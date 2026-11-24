//! La famille rendue par le `Renderer` du projet, dans toutes ses vues (plan,
//! élévation, coupe, 3D) : solides en faces et arêtes comme les éléments du projet,
//! éléments 2D, cotes de mesure et mur générique en primitives 2D maillées par
//! `moteur2d::sommets`, comme les annotations des vues du projet. Fond, style visuel,
//! traits et textes sont donc ceux du projet ; l'éditeur n'ajoute que ses
//! superpositions (plans de référence, contraintes, poignées, aperçus d'outil).
use super::*;
use glam::DVec3;
use modele::famille::modelee::CategorieFamille as C;
use noyau::trace2d::{Forme, Primitive, Role, Style};

/// Solides de la famille en tampons du `Renderer` (repère de rendu en m).
#[derive(Default)]
pub(super) struct SceneFamille {
    pub vertices: Vec<scene::Vertex>,
    pub opaque: Vec<u32>,
    pub aretes: Vec<u32>,
    pub min: Option<(Vec3, Vec3)>,
    /// Nombre d'objets (volumes puis formes à profil), pour la sélection.
    pub objets: usize,
}

pub(super) fn scene_famille(ed: &EditeurFamille, r: &Resolution) -> SceneFamille {
    let solides = ed.famille.solides(r);
    let mut s = SceneFamille { objets: solides.len(), ..Default::default() };
    for (objet, solide) in solides.iter().enumerate().filter(|(_, s)| ed.montre_solide(s) && !s.vide) {
        let obj = objet as u32;
        let couleur = solide.materiau_nom.as_ref().and_then(|nom| ed.materiaux_projet.iter().find(|(n, _)| n == nom).map(|(_, c)| *c)).unwrap_or([190, 198, 207]);
        let tris: Vec<[DVec3; 3]> = solide.tris.iter().map(|t| t.map(|p| p / 1000.0)).collect();
        for t in &tris {
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            let normal = [(n.x * 127.0) as i8, (n.y * 127.0) as i8, (n.z * 127.0) as i8, 0];
            for p in t {
                let p = p.as_vec3();
                s.min = Some(s.min.map_or((p, p), |(a, b)| (a.min(p), b.max(p))));
                s.opaque.push(s.vertices.len() as u32);
                s.vertices.push(scene::Vertex { pos: p.to_array(), normal, color: [couleur[0], couleur[1], couleur[2], 255], obj });
            }
        }
        // arêtes vives et douces, codées comme celles des éléments du projet
        let a = noyau::aretes::aretes(&tris);
        for p in a.vives.iter().flatten() {
            s.aretes.push(s.vertices.len() as u32);
            s.vertices.push(scene::Vertex { pos: p.as_vec3().to_array(), normal: [0; 4], color: noyau::aretes::NOIR, obj });
        }
        for (seg, n1, n2) in &a.douces {
            let normal = [(n1.x * 127.0) as i8, (n1.y * 127.0) as i8, (n1.z * 127.0) as i8, -127];
            let c = [n2.x, n2.y, n2.z].map(|c| ((c * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8);
            for p in seg {
                s.aretes.push(s.vertices.len() as u32);
                s.vertices.push(scene::Vertex { pos: p.as_vec3().to_array(), normal, color: [c[0], c[1], c[2], 255], obj });
            }
        }
    }
    s
}

/// Arêtes vives des solides visibles projetées sur la vue de travail (mm, repère h, v
/// de la vue) : source d'accrochage des solides.
pub(super) fn aretes_vue(ed: &EditeurFamille, r: &Resolution) -> Vec<noyau::forme2d::Geom> {
    let (ah, av) = ed.vue.axes();
    let mut geoms: Vec<noyau::forme2d::Geom> = Vec::new();
    for solide in ed.famille.solides(r).iter().filter(|s| ed.montre_solide(s) && !s.vide) {
        for [a, b] in noyau::aretes::aretes(&solide.tris).vives {
            let (p, q) = (glam::DVec2::new(a[ah as usize], a[av as usize]), glam::DVec2::new(b[ah as usize], b[av as usize]));
            let doublon = |g: &noyau::forme2d::Geom| matches!(g, noyau::forme2d::Geom::Segment([x, y]) if (x.distance(p) < 1e-6 && y.distance(q) < 1e-6) || (x.distance(q) < 1e-6 && y.distance(p) < 1e-6));
            if p.distance(q) > 1e-6 && !geoms.iter().any(doublon) { geoms.push(noyau::forme2d::Geom::Segment([p, q])); }
        }
    }
    geoms
}

const SELECTION: [u8; 4] = [230, 120, 20, 255];

/// Primitives 2D de la vue plane ouverte (repère de la vue, m : x = h, y = v) et leur
/// profondeur : (devant les solides, derrière eux). `echelle` : pixels par mm.
fn primitives_vue(ed: &EditeurFamille, r: &Resolution, projet: &modele::Projet, echelle: f64) -> (Vec<Primitive>, Vec<Primitive>) {
    use modele::documentation::{self as doc, AnnotationForme};
    let pt = |h: f64, v: f64| DVec3::new(h / 1000.0, v / 1000.0, 0.0);
    let papier = ed.famille.categorie.documentaire();
    let echelle_vue = if papier { 1.0 } else { ed.navigation[ed.vue as usize].echelle_vue };
    let mut devant = Vec::new();
    for (indice, element) in ed.famille.elements_2d.iter().enumerate().filter(|(k, e)| e.vue == ed.vue.esquisse() && ed.montre_element_2d(r, *k)) {
        let element = ed.famille.element_resolu(r, element);
        let choisi = ed.choisi(&SelectionFamille::Element2D(indice));
        let mut prims = if let AnnotationForme::Texte { point, texte } = &element.forme {
            let apercu = crate::outils::annoter::outils_texte::apercu_famille(texte, projet, ed.vue.nom(), ed.famille.categorie, ed.cible_feuille.as_deref(), ed.cible_piece.as_deref());
            let hauteur = element.hauteur * echelle_vue / 1000.0;
            let largeur = if ed.famille.categorie == C::EtiquettePiece { noyau::texte::largeur(&apercu, "Arial", hauteur, false) } else { 0.0 };
            vec![doc::texte(&apercu, pt(point[0], point[1]) - DVec3::X * (largeur / 2.0), DVec3::X, DVec3::Y, hauteur, [30, 30, 30, 255])]
        } else if element.forme.forme_2d().is_some() {
            // types de trait : tirets à l'échelle de la vue
            doc::primitives_dessin(projet, &element.forme, echelle_vue, &|q| pt(q.x, q.y))
        } else {
            doc::forme_cartouche(projet, &element.forme)
        };
        if choisi { prims.iter_mut().for_each(|p| p.style.couleur = SELECTION); }
        devant.extend(prims);
    }
    // cotes de mesure : géométrie des cotes du projet, taille fixe à l'écran
    for (k, c) in ed.famille.cotes_mesure.iter().enumerate().filter(|(_, c)| c.vue == ed.vue.esquisse()) {
        let pts: Vec<glam::DVec2> = c.points.iter().map(|p| { let (h, v) = ed.famille.point_esquisse(r, p); glam::DVec2::new(h, v) }).collect();
        let Some(g) = geometrie_mesure(c.genre, c.vue, &pts, echelle) else { continue };
        let couleur = if ed.choisi(&SelectionFamille::CoteMesure(k)) { SELECTION } else { [20, 40, 120, 255] };
        let style = Style { couleur, epaisseur: 0.0 };
        devant.push(Primitive { forme: Forme::Segments(g.traits.iter().map(|[a, b]| [pt(a.x, a.y), pt(b.x, b.y)]).collect()), style, role: Role::Annotation, element: None });
        let hauteur = 11.0 / echelle / 1000.0;
        let largeur = noyau::texte::largeur(&g.texte.2, "Arial", hauteur, false);
        devant.push(doc::texte(&g.texte.2, pt(g.texte.0.x, g.texte.0.y) + DVec3::new(-largeur / 2.0, 2.0 / echelle / 1000.0, 0.0), DVec3::X, DVec3::Y, hauteur, couleur));
    }
    // mur générique des portes et fenêtres : élévation pleine, plan et coupe en bande ;
    // les vides le percent
    let mut derriere = Vec::new();
    if matches!(ed.famille.categorie, C::Porte | C::Fenetre) {
        let [h0, h1, v0, v1] = ed.navigation[ed.vue as usize].etendue;
        let e = ed.epaisseur / 2.0;
        let ([a, b], [c, d]) = match ed.vue {
            VueFamille::Elevation => ([h0, h1], [v0, v1]),
            VueFamille::Plan => ([h0, h1], [-e, e]),
            VueFamille::Coupe => ([-e, e], [v0, v1]),
        };
        let rectangle = |a: f64, b: f64, c: f64, d: f64| vec![pt(a, c), pt(b, c), pt(b, d), pt(a, d)];
        let (ih, iv) = ed.vue.axes();
        let (ih, iv) = (ih as usize, iv as usize);
        let mut contours = vec![rectangle(a, b, c, d)];
        for x in ed.famille.boites(r).iter().filter(|x| x.vide && ed.vide_actif(r, x.indice)) {
            let (xa, xb, xc, xd) = (x.min[ih].max(a), x.max[ih].min(b), x.min[iv].max(c), x.max[iv].min(d));
            if xa < xb && xc < xd { contours.push(rectangle(xa, xb, xc, xd)); }
        }
        derriere.push(Primitive { forme: Forme::Surface(contours), style: Style { couleur: [222, 222, 222, 255], epaisseur: 0.0 }, role: Role::Poche, element: None });
        if ed.vue != VueFamille::Elevation {
            derriere.push(Primitive { forme: Forme::Polyligne { points: rectangle(a, b, c, d), fermee: true }, style: Style { couleur: [90, 90, 90, 255], epaisseur: 0.25 }, role: Role::Coupe, element: None });
        }
    }
    (devant, derriere)
}

/// Plan de coupe qui garde tout : active le tracé des primitives 2D du `Renderer`.
fn plan_tout_garder(camera: &Camera) -> Vec4 {
    (-camera.forward()).extend(1.0e4)
}

impl Application {
    /// Rendu de la famille par le `Renderer` du projet avec `camera` (paramètres
    /// d'affichage du projet : fond, style, éclairage) ; image à placer sous les
    /// superpositions de l'éditeur.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn rendre_famille(&mut self, ui: &egui::Ui, rect: egui::Rect, camera: &mut Camera, scene: &SceneFamille, coupe: (&[scene::Vertex], &[scene::Vertex]), selection: Option<usize>, mm: f32) -> Option<egui::Shape> {
        std::mem::swap(&mut self.camera, camera);
        let mut params = self.frame_params(rect.width() / rect.height().max(1.0));
        std::mem::swap(&mut self.camera, camera);
        params.selected = (0..scene.objets).map(|k| u8::from(selection == Some(k))).collect();
        params.coupe = (!coupe.0.is_empty() || !coupe.1.is_empty()).then(|| plan_tout_garder(camera));
        params.boite = None;
        params.cadrage.clear();
        params.contour_vue.clear();
        params.passes.clear();
        params.soleil = None;
        params.table_motifs.clear();
        params.familles_motifs.clear();
        params.mm = mm;
        let renderer = self.renderer_famille.as_mut()?;
        let n = scene.vertices.len();
        renderer.upload_vertices(&scene.vertices, &vec![[0; 4]; n], &vec![[0; 4]; n], &[]);
        renderer.upload_indices(&scene.opaque, &[], &[], &scene.aretes);
        renderer.upload_coupe(coupe.0, coupe.1);
        renderer.upload_passes(&[]);
        let ppp = ui.ctx().pixels_per_point();
        let id = renderer.paint(&params, (rect.width() * ppp).max(1.0) as u32, (rect.height() * ppp).max(1.0) as u32)?;
        Some(egui::Shape::image(id, rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE))
    }

    /// Vue plane de famille : rendu placé à `fond` (forme réservée sous le canevas).
    pub(super) fn rendre_vue_plane_famille(&mut self, ui: &egui::Ui, fond: egui::layers::ShapeIdx) {
        let Some(ed) = self.famille.take() else { return };
        let r = ed.resoudre();
        let indice = ed.vue as usize;
        let mut camera = ed.navigation[indice].camera.clone();
        let rect = ed.rect_navigation;
        let (scene, devant, derriere) = if !rect.is_positive() {
            (SceneFamille::default(), Vec::new(), Vec::new())
        } else {
            let projet_vide = modele::Projet::default();
            let projet = self.natif.as_ref().map(|(p, _)| p).unwrap_or(&projet_vide);
            let echelle = Cadre::depuis(&camera, rect, (0, 1)).echelle;
            let (devant, derriere) = primitives_vue(&ed, &r, projet, echelle);
            (scene_famille(&ed, &r), devant, derriere)
        };
        // repère de la vue (h, v, profondeur vers l'observateur) → repère de rendu ;
        // primitives devant ou derrière les solides
        let (ah, av) = ed.vue.axes();
        let axes = (ah as usize, av as usize);
        let vers_observateur = -camera.forward();
        let k = 3 - axes.0 - axes.1;
        let signe = vers_observateur[k].signum();
        let (mn, mx) = scene.min.unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let (loin, pres) = if signe > 0.0 { (mn[k] - 0.01, mx[k] + 0.01) } else { (mx[k] + 0.01, mn[k] - 0.01) };
        let placer = |prims: Vec<Primitive>, p: f32| prims.iter().map(|x| x.transformer(|q: DVec3| point_vue(axes, q.x * 1000.0, q.y * 1000.0, p).as_dvec3())).collect::<Vec<_>>();
        let mut prims = placer(derriere, loin);
        prims.extend(placer(devant, pres));
        let papier = ed.famille.categorie.documentaire();
        let echelle_vue = if papier { 1.0 } else { ed.navigation[indice].echelle_vue };
        let (tris, traits) = moteur2d::sommets(&|_| None, &prims, vers_observateur.as_dvec3(), echelle_vue, self.traits_fins);
        let selection = match ed.selection { SelectionFamille::Volume(k) => Some(k), SelectionFamille::Extrusion(k) => Some(ed.famille.volumes.len() + k), _ => None };
        self.famille = Some(ed);
        if rect.is_positive() && let Some(image) = self.rendre_famille(ui, rect, &mut camera, &scene, (&tris, &traits), selection, (echelle_vue / 1000.0) as f32) {
            ui.painter().set(fond, image);
        }
    }
}
