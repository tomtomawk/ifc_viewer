//! Interface egui, disposée comme Revit : propriétés à gauche, vue au centre,
//! arborescence du projet à droite (vues, nomenclatures, feuilles, familles, groupes et
//! liens d'un fichier Revit ; arborescence spatiale et classes d'un IFC).

use crate::camera::{Camera, MAX_PITCH};
use crate::dessin::{self, Poche};
use crate::model::pretty_type;
use crate::pilote::{self, Requete};
use crate::render::{FrameParams, ParamsPasse, Renderer};
use crate::rvt::{self, RvtInfo};
use crate::scene::{self, Scene};
use crate::trace2d;
use crate::step::Id;
use eframe::egui::{self, Align, Color32, PointerButton, RichText, Sense};
use eframe::glow;
use glam::{DMat4, Mat4, Vec2, Vec3, Vec4};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

enum LoadMsg {
    Progress(String),
    Done(Result<Box<Scene>, String>),
    /// Métadonnées, puis scène des murs (ou raison de son absence).
    Revit(Result<(Box<RvtInfo>, Result<Box<Scene>, String>), String>),
}

/// Classes masquées à l'ouverture.
const HIDDEN_BY_DEFAULT: &[&str] = &["IFCSPACE", "IFCOPENINGELEMENT", "IFCOPENINGSTANDARDCASE", "IFCVIRTUALELEMENT", "IFCANNOTATION"];

const SPATIAL: &[&str] = &[
    "IFCPROJECT", "IFCSITE", "IFCBUILDING", "IFCBUILDINGSTOREY", "IFCSPACE", "IFCFACILITY", "IFCFACILITYPART",
    "IFCBRIDGE", "IFCBRIDGEPART", "IFCROAD", "IFCROADPART", "IFCRAILWAY", "IFCRAILWAYPART", "IFCMARINEFACILITY",
    "IFCSPATIALZONE", "IFCEXTERNALSPATIALELEMENT",
];

/// Onglets de l'arborescence, choisis par la barre d'icônes : sections du navigateur de
/// projet de Revit (toutes, ou une seule), matériaux, arborescence spatiale, classes.
#[derive(PartialEq, Clone, Copy, Debug)]
enum LeftTab {
    /// Navigateur de projet de Revit, toutes sections.
    Tout,
    Vues,
    Feuilles,
    Nomenclatures,
    Familles,
    Materiaux,
    Groupes,
    Liens,
    /// Arborescence spatiale.
    Tree,
    Classes,
}

impl LeftTab {
    /// Onglets d'un fichier Revit (navigateur, puis modèle et classes) et d'un IFC.
    const REVIT: [LeftTab; 10] = [LeftTab::Tout, LeftTab::Vues, LeftTab::Feuilles, LeftTab::Nomenclatures, LeftTab::Familles, LeftTab::Materiaux, LeftTab::Groupes, LeftTab::Liens, LeftTab::Tree, LeftTab::Classes];
    const IFC: [LeftTab; 3] = [LeftTab::Tree, LeftTab::Classes, LeftTab::Materiaux];

    fn libelle(self) -> &'static str {
        match self {
            LeftTab::Tout => "Tout",
            LeftTab::Vues => "Vues",
            LeftTab::Feuilles => "Feuilles",
            LeftTab::Nomenclatures => "Nomenclatures",
            LeftTab::Familles => "Familles",
            LeftTab::Materiaux => "Matériaux",
            LeftTab::Groupes => "Groupes",
            LeftTab::Liens => "Liens Revit",
            LeftTab::Tree => "Modèle (arborescence spatiale)",
            LeftTab::Classes => "Classes",
        }
    }

    /// Nom pour le pilote (commande `onglet`).
    fn nom(self) -> &'static str {
        match self {
            LeftTab::Tout => "tout",
            LeftTab::Vues => "vues",
            LeftTab::Feuilles => "feuilles",
            LeftTab::Nomenclatures => "nomenclatures",
            LeftTab::Familles => "familles",
            LeftTab::Materiaux => "materiaux",
            LeftTab::Groupes => "groupes",
            LeftTab::Liens => "liens",
            LeftTab::Tree => "modele",
            LeftTab::Classes => "classes",
        }
    }

    /// Section du navigateur de projet de Revit.
    fn du_navigateur(self) -> bool {
        !matches!(self, LeftTab::Tree | LeftTab::Classes | LeftTab::Materiaux)
    }
}

/// Icône d'un onglet, tracée en traits dans `r` (environ 16 px).
fn icone(p: &egui::Painter, r: egui::Rect, onglet: LeftTab, c: Color32) {
    use egui::{Stroke, pos2, vec2};
    let t = Stroke::new(1.4, c);
    let (x0, y0, w, h) = (r.left(), r.top(), r.width(), r.height());
    let pt = |x: f32, y: f32| pos2(x0 + x * w, y0 + y * h);
    let rect = |a: f32, b: f32, c2: f32, d: f32| egui::Rect::from_min_max(pt(a, b), pt(c2, d));
    let seg = |a: (f32, f32), b: (f32, f32)| p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], t);
    let cadre = |r: egui::Rect, arrondi: f32| p.rect_stroke(r, arrondi, t, egui::StrokeKind::Inside);
    match onglet {
        LeftTab::Tout => {
            for (a, b) in [(0.05, 0.05), (0.55, 0.05), (0.05, 0.55), (0.55, 0.55)] {
                cadre(rect(a, b, a + 0.4, b + 0.4), 1.0);
            }
        }
        LeftTab::Vues => {
            // œil : deux arcs et la pupille
            let arc = |signe: f32| -> Vec<egui::Pos2> {
                (0..=12).map(|k| {
                    let u = k as f32 / 12.0;
                    pt(0.02 + 0.96 * u, 0.5 + signe * 0.32 * (std::f32::consts::PI * u).sin())
                }).collect()
            };
            p.add(egui::Shape::line(arc(-1.0), t));
            p.add(egui::Shape::line(arc(1.0), t));
            p.circle_filled(pt(0.5, 0.5), w * 0.13, c);
        }
        LeftTab::Feuilles => {
            // feuille au coin plié et son cartouche
            p.add(egui::Shape::line(vec![pt(0.15, 0.02), pt(0.65, 0.02), pt(0.88, 0.25), pt(0.88, 0.98), pt(0.15, 0.98), pt(0.15, 0.02)], t));
            seg((0.65, 0.02), (0.65, 0.25));
            seg((0.65, 0.25), (0.88, 0.25));
            cadre(rect(0.45, 0.72, 0.88, 0.98), 0.0);
        }
        LeftTab::Nomenclatures => {
            cadre(rect(0.04, 0.08, 0.96, 0.92), 0.0);
            p.rect_filled(rect(0.04, 0.08, 0.96, 0.3), 0.0, c);
            for y in [0.52, 0.72] {
                seg((0.04, y), (0.96, y));
            }
            seg((0.38, 0.3), (0.38, 0.92));
        }
        LeftTab::Familles => {
            // cube en perspective
            let (a, b, cc, d) = (pt(0.5, 0.04), pt(0.94, 0.27), pt(0.5, 0.5), pt(0.06, 0.27));
            p.add(egui::Shape::closed_line(vec![a, b, cc, d], t));
            p.add(egui::Shape::line(vec![d, pt(0.06, 0.73), pt(0.5, 0.96), pt(0.94, 0.73), b], t));
            p.line_segment([cc, pt(0.5, 0.96)], t);
        }
        LeftTab::Materiaux => {
            // échantillon : moitié pleine, moitié hachurée
            cadre(rect(0.06, 0.06, 0.94, 0.94), 1.0);
            p.add(egui::Shape::convex_polygon(vec![pt(0.06, 0.06), pt(0.94, 0.06), pt(0.06, 0.94)], c, Stroke::NONE));
            for k in [0.55, 0.75] {
                seg((k, 0.94), (0.94, k));
            }
        }
        LeftTab::Groupes => {
            cadre(rect(0.04, 0.04, 0.62, 0.62), 1.0);
            cadre(rect(0.38, 0.38, 0.96, 0.96), 1.0);
        }
        LeftTab::Liens => {
            // deux maillons de chaîne
            cadre(egui::Rect::from_center_size(pt(0.32, 0.62), vec2(w * 0.56, h * 0.36)), h * 0.18);
            cadre(egui::Rect::from_center_size(pt(0.68, 0.38), vec2(w * 0.56, h * 0.36)), h * 0.18);
        }
        LeftTab::Tree => {
            // arborescence : tronc et trois branches
            seg((0.2, 0.08), (0.2, 0.86));
            for y in [0.24, 0.52, 0.86] {
                seg((0.2, y), (0.5, y));
                p.rect_filled(rect(0.56, y - 0.09, 0.95, y + 0.09), 1.0, c);
            }
        }
        LeftTab::Classes => {
            // liste à puces
            for y in [0.2, 0.5, 0.8] {
                p.circle_filled(pt(0.12, y), w * 0.08, c);
                seg((0.3, y), (0.96, y));
            }
        }
    }
}

/// Action demandée depuis le navigateur de projet, appliquée après son tracé.
enum ActionNav {
    Selection(Id),
    Activer(usize),
    IsolerType(Id),
}

/// Rubrique du navigateur de projet d'un genre de vue (ordre de Revit) ; None pour les
/// nomenclatures et les feuilles, qui ont leur propre branche.
fn rubrique(genre: &str) -> Option<(u8, &'static str)> {
    Some(match genre {
        "Plan d'étage" => (0, "Plans d'étage"),
        "Plan de plafond" => (1, "Plans de plafond"),
        "Vue 3D" => (2, "Vues 3D"),
        "Élévation" => (3, "Élévations"),
        "Coupe" => (4, "Coupes"),
        "Vue de dessin" => (5, "Vues de dessin"),
        "Légende" => (6, "Légendes"),
        "Nomenclature" | "Feuille" => return None,
        _ => (7, "Autres vues"),
    })
}

/// Genres (`ObjectType`) des éléments 2D découpés au cadrage d'une vue, comme dans Revit.
const DETAIL: &[&str] = &["Trait", "Région remplie", "Composant de détail"];

/// Fenêtre de vue de la feuille ouverte, rendue dans sa propre passe (repère de la
/// scène) : passage du modèle à la feuille, rectangle de découpe sur la feuille (xmin,
/// ymin, xmax, ymax, à la cote z), plans de découpe, direction de la vue.
struct PasseFeuille {
    vers_feuille: Mat4,
    cadre: Option<[f32; 4]>,
    /// Étendue sur la feuille des objets de la passe (cadrage de la caméra d'une vue non
    /// cadrée).
    etendue: [f32; 4],
    z: f32,
    plans: Vec<Vec4>,
    direction: Vec3,
}

pub struct ViewerApp {
    gl: Option<Arc<glow::Context>>,
    renderer: Arc<Mutex<Option<Renderer>>>,
    scene: Option<Scene>,
    /// Métadonnées du fichier Revit ouvert.
    rvt: Option<RvtInfo>,
    /// Fiche Revit affichée à la place de la vue 3D.
    rvt_sheet: bool,
    /// Lecture des murs du fichier Revit : résumé si la scène en vient, sinon raison de l'échec.
    rvt_geom: Result<String, String>,
    rvt_preview: Option<egui::TextureHandle>,
    loading: Option<Receiver<LoadMsg>>,
    status: String,
    error: Option<String>,
    camera: Camera,
    selected: Option<Id>,
    reveal: Option<Id>,
    hidden_classes: HashSet<String>,
    hidden_objects: HashSet<u32>,
    hidden_storeys: HashSet<Id>,
    isolated: Option<HashSet<u32>>,
    visible: Vec<bool>,
    visibility_dirty: bool,
    left_tab: LeftTab,
    filter: String,
    show_unsupported: bool,
    /// Vue Revit ouverte (indice dans `Navigateur::vues`).
    vue_active: Option<usize>,
    /// Commandes du serveur MCP (voir `pilote.rs`) et celles en attente de traitement.
    pilote: Option<Receiver<Requete>>,
    requetes: Vec<Requete>,
    /// Réponses des ouvertures demandées par le pilote, envoyées en fin de chargement.
    attente_ouverture: Vec<Sender<String>>,
    /// Zone de la vue 3D dans la fenêtre (points), pour les captures.
    rect_vue: egui::Rect,
    /// Passes des fenêtres de vue de la feuille ouverte.
    passes_feuille: Vec<PasseFeuille>,
    /// Mode dessin : plans et élévations comme dans un logiciel de DAO.
    dessin: bool,
    /// Coupe horizontale du mode dessin : active, niveau de référence (sinon bas du
    /// modèle) et hauteur au-dessus de lui (m).
    coupe: bool,
    coupe_niveau: Option<Id>,
    coupe_hauteur: f32,
    poche: Poche,
    /// Dénominateur de l'échelle du dessin (épaisseur des contours, pas des hachures).
    echelle: f64,
    /// Tracé de la coupe à recalculer.
    coupe_dirty: bool,
    /// Niveaux de la scène (voir `niveaux`), relevés à son installation.
    liste_niveaux: Vec<(Id, String, f32)>,
    /// Matériaux de la scène et éléments qui les portent (onglet Matériaux).
    liste_materiaux: Vec<scene::NavMateriau>,
}

impl ViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut error = None;
        let renderer = match cc.gl.as_ref().map(|gl| Renderer::new(gl)) {
            Some(Ok(r)) => Some(r),
            Some(Err(e)) => {
                error = Some(e);
                None
            }
            None => {
                error = Some("Contexte OpenGL indisponible".into());
                None
            }
        };
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut app = ViewerApp {
            gl: cc.gl.clone(),
            renderer: Arc::new(Mutex::new(renderer)),
            scene: None,
            rvt: None,
            rvt_sheet: false,
            rvt_geom: Err(String::new()),
            rvt_preview: None,
            loading: None,
            status: "Ouvrir un fichier IFC ou Revit (Ctrl+O), ou le glisser dans la fenêtre.".into(),
            error,
            camera: Camera::default(),
            selected: None,
            reveal: None,
            hidden_classes: HashSet::new(),
            hidden_objects: HashSet::new(),
            hidden_storeys: HashSet::new(),
            isolated: None,
            visible: Vec::new(),
            visibility_dirty: false,
            left_tab: LeftTab::Tree,
            filter: String::new(),
            show_unsupported: false,
            vue_active: None,
            pilote: pilote::demarrer(cc.egui_ctx.clone()),
            requetes: Vec::new(),
            attente_ouverture: Vec::new(),
            rect_vue: egui::Rect::NOTHING,
            passes_feuille: Vec::new(),
            dessin: false,
            coupe: true,
            coupe_niveau: None,
            coupe_hauteur: 1.2,
            poche: Poche::Hachure,
            echelle: 100.0,
            coupe_dirty: true,
            liste_niveaux: Vec::new(),
            liste_materiaux: Vec::new(),
        };
        if let Some(p) = path {
            app.open(p, &cc.egui_ctx);
        }
        app
    }

    fn open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.loading = Some(rx);
        self.error = None;
        self.status = format!("Chargement de {}…", path.display());
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let progress = |s: String| {
                    let _ = tx.send(LoadMsg::Progress(s));
                    ctx.request_repaint();
                };
                if is_revit_file(&path) {
                    progress(format!("Lecture des métadonnées Revit de {}…", path.display()));
                    let res = rvt::load(&path).map(|info| {
                        let titre = info.title.clone().unwrap_or_else(|| info.file_name.clone());
                        let format = info.format().unwrap_or("?").to_string();
                        let scene = rvt::maquette::load_scene(&path, &titre, &format, &progress).map(Box::new);
                        (Box::new(info), scene)
                    });
                    let _ = tx.send(LoadMsg::Revit(res));
                } else {
                    let res = scene::load(&path, &progress).map(Box::new);
                    let _ = tx.send(LoadMsg::Done(res));
                }
                ctx.request_repaint();
            })
            .expect("thread de chargement");
    }

    fn poll_loading(&mut self) {
        let Some(rx) = &self.loading else { return };
        while let Ok(msg) = rx.try_recv() {
            match msg {
                LoadMsg::Progress(s) => self.status = s,
                LoadMsg::Done(Ok(scene)) => {
                    self.loading = None;
                    self.install_scene(*scene);
                    return;
                }
                LoadMsg::Revit(Ok((info, scene))) => {
                    self.loading = None;
                    let head = format!("{} — {} Revit {}", info.file_name, info.kind, info.format().unwrap_or("?"));
                    match scene {
                        Ok(scene) => {
                            let summary = format!(
                                "{} objets, {} triangles, lus dans le fichier en {:.2} s",
                                scene.objects.len(),
                                scene.triangles,
                                scene.load_ms as f64 / 1000.0
                            );
                            self.install_scene(*scene);
                            self.status = format!("{head} — {summary}");
                            self.rvt_geom = Ok(summary);
                            self.rvt_sheet = false;
                        }
                        Err(e) => {
                            self.status = format!("{head} — métadonnées seules ({e})");
                            self.rvt_geom = Err(e);
                            self.rvt_sheet = true;
                        }
                    }
                    self.rvt_preview = None;
                    self.rvt = Some(*info);
                    return;
                }
                LoadMsg::Revit(Err(e)) | LoadMsg::Done(Err(e)) => {
                    self.loading = None;
                    self.status = "Échec du chargement.".into();
                    self.error = Some(e);
                    return;
                }
            }
        }
    }

    fn install_scene(&mut self, scene: Scene) {
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_vertices(gl, &scene.vertices);
        }
        self.status = format!(
            "{} — {} — {} entités, {} objets 3D, {} triangles — {:.2} s",
            scene.file_name,
            if scene.model.file.schema.is_empty() { "schéma ?" } else { &scene.model.file.schema },
            scene.model.file.entities.len(),
            scene.objects.len(),
            scene.triangles,
            scene.load_ms as f64 / 1000.0,
        );
        self.rvt = None;
        self.rvt_sheet = false;
        self.rvt_preview = None;
        self.camera = Camera::default();
        self.camera.scene_radius = (scene.max - scene.min).length() * 0.5;
        self.camera.fit(scene.min, scene.max);
        self.selected = None;
        self.hidden_objects.clear();
        self.hidden_storeys.clear();
        self.isolated = None;
        self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
        // fichier sans objet visible par défaut (famille 2D de Revit : traits et textes
        // seulement) : on montre les annotations
        if scene.objects.iter().all(|o| self.hidden_classes.contains(&*o.class)) {
            self.hidden_classes.remove("IFCANNOTATION");
        }
        // cadrage sur les objets visibles par défaut (les vues de dessin et les feuilles
        // d'un projet Revit sont loin du modèle)
        let (mut mn, mut mx) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for o in scene.objects.iter().filter(|o| !self.hidden_classes.contains(&*o.class)) {
            mn = mn.min(o.min);
            mx = mx.max(o.max);
        }
        if mn.x <= mx.x {
            self.camera.scene_radius = (mx - mn).length() * 0.5;
            self.camera.fit(mn, mx);
        }
        self.visible = vec![true; scene.objects.len()];
        self.vue_active = None;
        self.dessin = false;
        self.coupe_niveau = None;
        self.liste_niveaux = niveaux(&scene);
        self.left_tab = if scene.navigateur.is_some() { LeftTab::Tout } else { LeftTab::Tree };
        self.liste_materiaux = match &scene.navigateur {
            Some(n) => n.materiaux.clone(),
            None => materiaux_ifc(&scene),
        };
        self.scene = Some(scene);
        self.visibility_dirty = true;
        self.preparer_feuille(None);
    }

    fn update_visibility(&mut self) {
        if !self.visibility_dirty {
            return;
        }
        self.visibility_dirty = false;
        let Some(scene) = &self.scene else { return };
        // coupe du mode dessin : objets du modèle entièrement au-dessus écartés
        let dessus = self.plan_coupe().map(|p| p.w);
        let mut opaque = Vec::new();
        let mut transp = Vec::new();
        let mut lines = Vec::new();
        let mut aretes = Vec::new();
        for (i, o) in scene.objects.iter().enumerate() {
            let vis = !self.hidden_classes.contains(&*o.class)
                && !self.hidden_objects.contains(&(i as u32))
                && o.storey.is_none_or(|s| !self.hidden_storeys.contains(&s))
                && self.isolated.as_ref().is_none_or(|iso| iso.contains(&(i as u32)))
                && dessus.is_none_or(|z| o.min.z <= z || &*o.class == "IFCANNOTATION");
            self.visible[i] = vis;
            if vis {
                opaque.extend(o.opaque.start..o.opaque.start + o.opaque.count);
                transp.extend(o.transparent.start..o.transparent.start + o.transparent.count);
                lines.extend(o.lines.start..o.lines.start + o.lines.count);
                aretes.extend(o.aretes.start..o.aretes.start + o.aretes.count);
            }
        }
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_indices(gl, &opaque, &transp, &lines, &aretes);
        }
        self.coupe_dirty = true;
    }

    fn niveaux(&self) -> Vec<(Id, String, f32)> {
        self.liste_niveaux.clone()
    }

    /// Plan de la coupe horizontale du mode dessin (n, d ; repère de la scène, partie
    /// gardée sous la cote de coupe), None hors mode dessin ou sans coupe.
    fn plan_coupe(&self) -> Option<Vec4> {
        if !self.dessin || !self.coupe {
            return None;
        }
        let s = self.scene.as_ref()?;
        let base = self.coupe_niveau.and_then(|n| self.liste_niveaux.iter().find(|x| x.0 == n)).map_or(s.min.z, |x| x.2);
        Some(Vec4::new(0.0, 0.0, -1.0, base + self.coupe_hauteur))
    }

    /// Recalcule le tracé de la coupe (poché, hachures, contours) s'il le faut.
    fn update_coupe(&mut self) {
        if !self.coupe_dirty {
            return;
        }
        self.coupe_dirty = false;
        let (tris, traits) = match (&self.scene, self.plan_coupe()) {
            (Some(s), Some(p)) => {
                let prims = dessin::coupe(s, &self.visible, p, self.echelle, self.poche);
                dessin::sommets(s, &prims, p.truncate().as_dvec3().normalize(), self.echelle)
            }
            _ => Default::default(),
        };
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_coupe(gl, &tris, &traits);
        }
    }

    /// Dessin vectoriel de la vue (orthographique) : coupe, arêtes vues sans les parties
    /// cachées, éléments 2D de la vue Revit ouverte ; repère de la scène, axes de la caméra.
    fn dessin_vue(&self) -> Result<trace2d::Dessin, String> {
        let s = self.scene.as_ref().ok_or("aucun fichier ouvert")?;
        if !self.camera.ortho {
            return Err("vue orthographique requise (bouton Ortho)".into());
        }
        let vue = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i));
        if vue.is_some_and(|v| v.genre == "Feuille") {
            return Err("export d'une feuille non pris en charge".into());
        }
        let f = self.camera.forward().as_dvec3().normalize();
        let droite = f.cross(self.camera.up().as_dvec3()).normalize();
        let haut = droite.cross(f);
        let mut primitives = Vec::new();
        let coupe = self.plan_coupe();
        if let Some(p) = coupe {
            primitives.extend(dessin::coupe(s, &self.visible, p, self.echelle, self.poche));
        }
        primitives.extend(dessin::vu(s, &self.visible, droite, haut, -f, coupe, 4096));
        // éléments 2D de la vue Revit ouverte, s'ils sont affichés (repère du fichier →
        // repère de la scène)
        if let Some(v) = vue {
            for p in v.primitives.iter().filter(|p| p.element.and_then(|e| s.object_of.get(&e)).is_some_and(|&o| self.visible[o as usize])) {
                primitives.push(p.translater(-s.origin));
            }
        }
        Ok(trace2d::Dessin { droite, haut, echelle: self.echelle, primitives })
    }

    /// Export SVG du dessin de la vue ; nombre de primitives écrites.
    fn exporter_svg(&self, chemin: &std::path::Path) -> Result<usize, String> {
        let d = self.dessin_vue()?;
        let s = self.scene.as_ref().ok_or("aucun fichier ouvert")?;
        let vue = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i)).map(|v| format!(" — {} : {}", v.genre, v.nom)).unwrap_or_default();
        let titre = format!("{}{vue} — 1:{}", s.file_name, d.echelle);
        std::fs::write(chemin, d.svg(&titre)).map_err(|e| format!("écriture impossible : {e}"))?;
        Ok(d.primitives.len())
    }

    /// Réglages du mode dessin : coupe, niveau, hauteur, poché, échelle.
    fn reglages_dessin(&mut self, ui: &mut egui::Ui) {
        let mut change = ui.checkbox(&mut self.coupe, "Coupe").on_hover_text("Coupe horizontale (plan) : parties coupées en poché et trait fort").changed();
        if self.coupe {
            let niveaux = self.niveaux();
            let nom = |n: Option<Id>| n.and_then(|n| niveaux.iter().find(|x| x.0 == n)).map_or("Bas du modèle".to_string(), |x| x.1.clone());
            egui::ComboBox::from_id_salt("coupe_niveau").selected_text(nom(self.coupe_niveau)).show_ui(ui, |ui| {
                change |= ui.selectable_value(&mut self.coupe_niveau, None, "Bas du modèle").changed();
                for (id, n, z) in &niveaux {
                    change |= ui.selectable_value(&mut self.coupe_niveau, Some(*id), format!("{n} ({z:+.2})")).changed();
                }
            });
            change |= ui.add(egui::DragValue::new(&mut self.coupe_hauteur).speed(0.01).range(-50.0..=200.0).suffix(" m")).on_hover_text("Hauteur de coupe au-dessus du niveau").changed();
            egui::ComboBox::from_id_salt("poche").selected_text(self.poche.libelle()).show_ui(ui, |ui| {
                for p in [Poche::Hachure, Poche::Plein, Poche::Vide] {
                    change |= ui.selectable_value(&mut self.poche, p, p.libelle()).changed();
                }
            });
        }
        change |= ui.add(egui::DragValue::new(&mut self.echelle).speed(1.0).range(1.0..=10000.0).prefix("1:")).on_hover_text("Échelle : épaisseur des contours et pas des hachures").changed();
        if ui.button("SVG…").on_hover_text("Exporter le dessin de la vue en SVG (vue orthographique)").clicked() {
            let nom = self.scene.as_ref().map(|s| s.file_name.rsplit_once('.').map_or(s.file_name.clone(), |x| x.0.to_string())).unwrap_or_default();
            if let Some(chemin) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name(format!("{nom}.svg")).save_file() {
                match self.exporter_svg(&chemin) {
                    Ok(n) => self.status = format!("{} : {n} primitives exportées", chemin.display()),
                    Err(e) => self.error = Some(e),
                }
            }
        }
        if change {
            self.visibility_dirty = true;
        }
    }

    fn select(&mut self, id: Option<Id>, reveal: bool) {
        self.selected = id;
        if reveal {
            self.reveal = id;
            if id.is_some() {
                self.left_tab = LeftTab::Tree;
                self.filter.clear();
            }
        }
    }

    fn selected_object(&self) -> Option<u32> {
        let s = self.scene.as_ref()?;
        s.object_of.get(&self.selected?).copied()
    }

    /// Objets 3D d'un nœud et de ses descendants.
    fn objects_under(&self, id: Id) -> Vec<u32> {
        let Some(s) = &self.scene else { return vec![] };
        let mut out = Vec::new();
        let mut stack = vec![id];
        let mut seen = HashSet::new();
        while let Some(n) = stack.pop() {
            if !seen.insert(n) {
                continue;
            }
            if let Some(&o) = s.object_of.get(&n) {
                out.push(o);
            }
            if let Some(ch) = s.model.children.get(&n) {
                stack.extend(ch.iter().copied());
            }
        }
        out
    }

    fn frame_objects(&mut self, objs: &[u32]) {
        let Some(s) = &self.scene else { return };
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for &o in objs {
            let o = &s.objects[o as usize];
            mn = mn.min(o.min);
            mx = mx.max(o.max);
        }
        if mn.x <= mx.x {
            self.camera.fit(mn, mx);
        }
    }

    fn fit_all(&mut self) {
        let Some(s) = &self.scene else { return };
        let objs: Vec<u32> = (0..s.objects.len() as u32).filter(|&i| self.visible[i as usize]).collect();
        if objs.is_empty() {
            let (a, b) = (s.min, s.max);
            self.camera.fit(a, b);
        } else {
            self.frame_objects(&objs);
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("IFC et Revit", &["ifc", "IFC", "rvt", "rfa", "rte", "rft"])
            .add_filter("IFC", &["ifc", "IFC"])
            .add_filter("Revit (murs et métadonnées)", &["rvt", "rfa", "rte", "rft"])
            .pick_file()
        {
            self.open(p, ctx);
        }
    }

    // ------------------------------------------------------------------ UI

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            if ui.button("Ouvrir…").on_hover_text("Ctrl+O").clicked() {
                self.open_dialog(&ctx);
            }
            if self.rvt.is_some() && self.rvt_geom.is_ok() {
                ui.toggle_value(&mut self.rvt_sheet, "Fiche Revit").on_hover_text("Métadonnées du fichier Revit");
            }
            ui.separator();
            let has = self.scene.is_some();
            ui.add_enabled_ui(has, |ui| {
                if ui.button("Cadrer").on_hover_text("F").clicked() {
                    self.fit_all();
                }
                if ui.button("Dessus").clicked() {
                    self.camera.pitch = MAX_PITCH;
                    self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                }
                if ui.button("Face").clicked() {
                    self.camera.pitch = 0.0;
                    self.camera.yaw = -std::f32::consts::FRAC_PI_2;
                }
                if ui.button("Iso").clicked() {
                    self.camera.pitch = 0.6;
                    self.camera.yaw = -2.3;
                }
                ui.toggle_value(&mut self.camera.ortho, "Ortho");
                if ui.toggle_value(&mut self.dessin, "Dessin 2D").on_hover_text("Plans et élévations : faces blanches, arêtes vives, parties cachées éliminées, coupe").changed() {
                    self.visibility_dirty = true;
                }
                if self.dessin {
                    self.reglages_dessin(ui);
                }
                ui.separator();
                for (label, classes) in [
                    ("Espaces", &["IFCSPACE"][..]),
                    ("Ouvertures", &["IFCOPENINGELEMENT", "IFCOPENINGSTANDARDCASE"][..]),
                    ("Annotations", &["IFCANNOTATION"][..]),
                    ("Motifs", &["IFCSURFACEFEATURE"][..]),
                ] {
                    let mut shown = !self.hidden_classes.contains(classes[0]);
                    if ui.checkbox(&mut shown, label).changed() {
                        for c in classes {
                            if shown {
                                self.hidden_classes.remove(*c);
                            } else {
                                self.hidden_classes.insert(c.to_string());
                            }
                        }
                        self.visibility_dirty = true;
                    }
                }
                ui.separator();
                if ui.button("Tout afficher").clicked() {
                    self.hidden_objects.clear();
                    self.hidden_storeys.clear();
                    self.isolated = None;
                    self.visibility_dirty = true;
                }
            });
            if self.loading.is_some() {
                ui.separator();
                ui.spinner();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(e) = &self.error {
                ui.colored_label(Color32::from_rgb(200, 40, 40), format!("Erreur : {e}"));
                ui.separator();
            }
            if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
                ui.label(RichText::new(format!("1 : {}", v.echelle)).strong());
                ui.separator();
            }
            ui.label(&self.status);
            if let Some(s) = &self.scene {
                if !s.unsupported.is_empty() {
                    let n: usize = s.unsupported.iter().map(|u| u.1).sum();
                    ui.separator();
                    if ui.link(format!("{n} entités géométriques ignorées")).clicked() {
                        self.show_unsupported = !self.show_unsupported;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("Gauche : orbite · Droit/milieu : pan · Molette : zoom · Double-clic : cadrer").weak());
            });
        });
    }

    /// Arborescence du projet (panneau de droite, comme dans Revit).
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        let nom = self.scene.as_ref().map(|s| s.file_name.clone()).unwrap_or_default();
        ui.label(RichText::new(if nom.is_empty() { "Arborescence du projet".to_string() } else { format!("Arborescence du projet - {nom}") }).strong());
        let a_nav = self.scene.as_ref().is_some_and(|s| s.navigateur.is_some());
        let onglets: &[LeftTab] = if a_nav { &LeftTab::REVIT } else { &LeftTab::IFC };
        if !onglets.contains(&self.left_tab) {
            self.left_tab = onglets[0];
        }
        // barre d'icônes : une section ou un mode de l'arborescence chacune
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &o in onglets {
                if o == LeftTab::Tree && a_nav {
                    ui.separator();
                }
                let (r, resp) = ui.allocate_exact_size(egui::vec2(25.0, 24.0), Sense::click());
                let actif = self.left_tab == o;
                let v = ui.visuals();
                let fond = if actif { v.selection.bg_fill } else if resp.hovered() { v.widgets.hovered.bg_fill } else { Color32::TRANSPARENT };
                let encre = if actif { v.selection.stroke.color } else { v.widgets.inactive.fg_stroke.color };
                ui.painter().rect_filled(r, 3.0, fond);
                icone(ui.painter(), r.shrink2(egui::vec2(4.5, 4.0)), o, encre);
                if resp.on_hover_text(o.libelle()).clicked() {
                    self.left_tab = o;
                }
            }
        });
        ui.label(RichText::new(self.left_tab.libelle()).small().weak());
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Rechercher"));
        ui.separator();
        if self.scene.is_none() {
            ui.label(RichText::new("Aucun modèle chargé").weak());
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.left_tab.du_navigateur() {
                self.navigateur_ui(ui, self.left_tab);
                return;
            }
            if self.left_tab == LeftTab::Materiaux {
                self.materiaux_ui(ui);
                return;
            }
            if !self.filter.trim().is_empty() {
                self.filtered_list(ui);
                return;
            }
            match self.left_tab {
                LeftTab::Classes => self.class_list(ui),
                _ => {
                    let roots = self.tree_roots();
                    for r in roots {
                        self.tree_node(ui, r, 0);
                    }
                }
            }
        });
        self.reveal = None;
    }

    /// Matériaux : pastille de couleur, nom, nombre d'éléments ; clic : isoler et cadrer
    /// les éléments qui portent le matériau.
    fn materiaux_ui(&mut self, ui: &mut egui::Ui) {
        let q = self.filter.trim().to_lowercase();
        let mut isoler: Option<Vec<Id>> = None;
        let mut n = 0;
        for m in self.liste_materiaux.iter().filter(|m| q.is_empty() || m.nom.to_lowercase().contains(&q)) {
            n += 1;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                let [cr, cg, cb] = m.couleur;
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(cr, cg, cb));
                ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
                let texte = format!("{} ({})", m.nom, m.elements.len());
                let resp = ui.selectable_label(false, if m.elements.is_empty() { RichText::new(texte).weak() } else { RichText::new(texte) });
                let resp = resp.on_hover_text(format!("Transparence {:.0} %\nClic : isoler les éléments qui le portent", m.transparence * 100.0));
                if resp.clicked() && !m.elements.is_empty() {
                    isoler = Some(m.elements.clone());
                }
            });
        }
        if n == 0 {
            ui.label(RichText::new("Aucun matériau").weak());
        }
        if let (Some(ids), Some(s)) = (isoler, &self.scene) {
            let objs: Vec<u32> = ids.iter().filter_map(|id| s.object_of.get(id).copied()).collect();
            self.isolate(objs.clone());
            self.frame_objects(&objs);
        }
    }

    /// Navigateur de projet de Revit : vues par genre et type, nomenclatures, feuilles et
    /// vues qu'elles portent, familles par catégorie, groupes, liens. Clic : propriétés ;
    /// double-clic : ouvrir la vue (ou isoler les instances d'un type).
    fn navigateur_ui(&mut self, ui: &mut egui::Ui, section: LeftTab) {
        let Some(s) = self.scene.as_ref() else { return };
        let Some(nav) = s.navigateur.as_ref() else { return };
        let q = self.filter.trim().to_lowercase();
        let garde = |t: &str| q.is_empty() || t.to_lowercase().contains(&q);
        let ouvert = !q.is_empty();
        let (selection, active) = (self.selected, self.vue_active);
        let mut action = None;
        let feuille = |ui: &mut egui::Ui, label: String, entite: Option<Id>, vue: Option<usize>, action: &mut Option<ActionNav>| {
            let actif = vue.is_some() && vue == active;
            let texte = if actif { RichText::new(label).strong() } else { RichText::new(label) };
            let r = ui.selectable_label(entite.is_some() && entite == selection, texte);
            if r.double_clicked() {
                *action = vue.map(ActionNav::Activer).or_else(|| entite.map(ActionNav::IsolerType));
            } else if r.clicked() {
                if let Some(e) = entite {
                    *action = Some(ActionNav::Selection(e));
                }
            }
        };
        let titre_vue = |v: &crate::scene::NavVue| if v.nom.is_empty() { format!("{} [{}]", v.genre, v.revit_id) } else { v.nom.clone() };
        // section : repliable sous « Tout », seule et dépliée sous son onglet
        let rubrique_nav = |ui: &mut egui::Ui, onglet: LeftTab, titre: String, ouverte: bool, contenu: &mut dyn FnMut(&mut egui::Ui)| {
            if section == LeftTab::Tout {
                egui::CollapsingHeader::new(RichText::new(titre).strong()).id_salt(("nav", onglet.nom())).default_open(ouverte).open(ouvert.then_some(true)).show(ui, |ui| contenu(ui));
            } else if section == onglet {
                contenu(ui);
            }
        };
        // Vues (tout) : rubriques dans l'ordre de Revit, puis type de vue
        let mut rubriques: BTreeMap<(u8, &str, String), Vec<usize>> = BTreeMap::new();
        for (i, v) in nav.vues.iter().enumerate() {
            if let Some((ordre, nom)) = rubrique(&v.genre) {
                if garde(&titre_vue(v)) {
                    rubriques.entry((ordre, nom, v.type_vue.clone())).or_default().push(i);
                }
            }
        }
        rubrique_nav(ui, LeftTab::Vues, "Vues (tout)".into(), true, &mut |ui| {
            for ((_, nom, type_vue), vues) in &rubriques {
                let mut vues = vues.clone();
                vues.sort_by_key(|&i| titre_vue(&nav.vues[i]).to_lowercase());
                // « Élévations (Élévation 1) » : type entre parenthèses s'il diffère du genre
                let genre = &nav.vues[vues[0]].genre;
                let titre = if type_vue.is_empty() || type_vue.eq_ignore_ascii_case(genre) { nom.to_string() } else { format!("{nom} ({type_vue})") };
                egui::CollapsingHeader::new(titre.clone()).id_salt(("nav-rub", titre)).default_open(true).open(ouvert.then_some(true)).show(ui, |ui| {
                    for i in vues {
                        feuille(ui, titre_vue(&nav.vues[i]), nav.vues[i].entite, Some(i), &mut action);
                    }
                });
            }
        });
        let mut nomenclatures: Vec<usize> = (0..nav.vues.len()).filter(|&i| nav.vues[i].genre == "Nomenclature" && garde(&nav.vues[i].nom)).collect();
        nomenclatures.sort_by_key(|&i| nav.vues[i].nom.to_lowercase());
        rubrique_nav(ui, LeftTab::Nomenclatures, format!("Nomenclatures/Quantités (tout) ({})", nomenclatures.len()), false, &mut |ui| {
            if nomenclatures.is_empty() {
                ui.label(RichText::new("Aucune nomenclature").weak());
            }
            for &i in &nomenclatures {
                feuille(ui, nav.vues[i].nom.clone(), nav.vues[i].entite, Some(i), &mut action);
            }
        });
        let mut feuilles: Vec<usize> = (0..nav.vues.len()).filter(|&i| nav.vues[i].genre == "Feuille").collect();
        feuilles.sort_by_key(|&i| (nav.vues[i].numero.clone(), nav.vues[i].nom.clone()));
        let indice: std::collections::HashMap<i64, usize> = nav.vues.iter().enumerate().map(|(i, v)| (v.revit_id, i)).collect();
        rubrique_nav(ui, LeftTab::Feuilles, format!("Feuilles (tout) ({})", feuilles.len()), false, &mut |ui| {
            if feuilles.is_empty() {
                ui.label(RichText::new("Aucune feuille").weak());
            }
            for &i in &feuilles {
                let f = &nav.vues[i];
                let titre = if f.numero.is_empty() { titre_vue(f) } else { format!("{} - {}", f.numero, f.nom) };
                let placees: Vec<usize> = f.placees.iter().filter_map(|v| indice.get(v).copied()).collect();
                if !garde(&titre) && !placees.iter().any(|&j| garde(&nav.vues[j].nom)) {
                    continue;
                }
                if placees.is_empty() {
                    feuille(ui, titre, f.entite, Some(i), &mut action);
                } else {
                    let id = ui.make_persistent_id(("nav-feuille", f.revit_id));
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, ouvert)
                        .show_header(ui, |ui| feuille(ui, titre, f.entite, Some(i), &mut action))
                        .body(|ui| {
                            for j in placees {
                                feuille(ui, titre_vue(&nav.vues[j]), nav.vues[j].entite, Some(j), &mut action);
                            }
                        });
                }
            }
        });
        // Familles : catégorie, famille, types
        let mut par_cat: BTreeMap<&str, Vec<&crate::scene::NavFamille>> = BTreeMap::new();
        for f in &nav.familles {
            if garde(&f.nom) || garde(&f.categorie) || f.types.iter().any(|t| garde(&t.0)) {
                par_cat.entry(f.categorie.as_str()).or_default().push(f);
            }
        }
        rubrique_nav(ui, LeftTab::Familles, "Familles".into(), false, &mut |ui| {
            for (&cat, fams) in &par_cat {
                egui::CollapsingHeader::new(cat).id_salt(("nav-cat", cat)).open(ouvert.then_some(true)).show(ui, |ui| {
                    for f in fams.iter() {
                        egui::CollapsingHeader::new(&f.nom).id_salt(("nav-fam", cat, &f.nom)).open(ouvert.then_some(true)).show(ui, |ui| {
                            for (t, e) in &f.types {
                                let label = if e.is_some() { t.clone() } else { format!("{t} (non utilisé)") };
                                feuille(ui, label, *e, None, &mut action);
                            }
                        });
                    }
                });
            }
        });
        let groupes: Vec<&String> = nav.groupes.iter().filter(|g| garde(g)).collect();
        rubrique_nav(ui, LeftTab::Groupes, format!("Groupes ({})", groupes.len()), false, &mut |ui| {
            if groupes.is_empty() {
                ui.label(RichText::new("Aucun groupe").weak());
            }
            for g in &groupes {
                ui.label(g.as_str());
            }
        });
        let liens: Vec<String> = self.rvt.iter().flat_map(|r| r.external_refs.iter()).filter(|e| e.kind.contains("Revit")).map(|e| e.path.clone()).collect();
        rubrique_nav(ui, LeftTab::Liens, format!("Liens Revit ({})", liens.len()), false, &mut |ui| {
            for l in &liens {
                ui.label(l.as_str());
            }
            if liens.is_empty() {
                ui.label(RichText::new("Aucun lien").weak());
            }
        });
        if section == LeftTab::Tout && !self.liste_materiaux.is_empty() {
            egui::CollapsingHeader::new(RichText::new(format!("Matériaux ({})", self.liste_materiaux.len())).strong()).id_salt(("nav", "materiaux")).open(ouvert.then_some(true)).show(ui, |ui| self.materiaux_ui(ui));
        }
        match action {
            Some(ActionNav::Selection(e)) => self.select(Some(e), false),
            Some(ActionNav::Activer(i)) => self.activer_vue(i),
            Some(ActionNav::IsolerType(t)) => {
                if let Some(s) = &self.scene {
                    let objs: Vec<u32> = s.objects.iter().enumerate().filter(|(_, o)| s.model.type_of.get(&o.id) == Some(&t)).map(|(i, _)| i as u32).collect();
                    self.select(Some(t), false);
                    self.isolate(objs.clone());
                    self.frame_objects(&objs);
                }
            }
            None => {}
        }
    }

    /// Ouvre une vue Revit comme dans Revit : caméra orthographique orientée selon la
    /// vue ; plan : son niveau seul ; coupe, élévation : tout le modèle ; vue de dessin,
    /// feuille, légende : ses éléments 2D ; vue 3D : modèle entier. Les éléments 2D de la
    /// vue sont affichés.
    fn activer_vue(&mut self, i: usize) {
        let Some(s) = &self.scene else { return };
        let Some(v) = s.navigateur.as_ref().and_then(|n| n.vues.get(i)) else { return };
        let annot = v.entite.map(|e| self.objects_under(e)).unwrap_or_default();
        let modele: Vec<u32> = (0..s.objects.len() as u32).filter(|&o| &*s.objects[o as usize].class != "IFCANNOTATION").collect();
        let (objs, cadre): (Vec<u32>, Vec<u32>) = match v.genre.as_str() {
            "Vue 3D" => (Vec::new(), modele.iter().copied().filter(|&o| !HIDDEN_BY_DEFAULT.contains(&&*s.objects[o as usize].class)).collect()),
            "Plan d'étage" | "Plan de plafond" => {
                let etage: Vec<u32> = modele.iter().copied().filter(|&o| v.etage.is_some() && s.objects[o as usize].storey == v.etage).collect();
                let tous = etage.iter().chain(&annot).copied().collect();
                (tous, if etage.is_empty() { modele.clone() } else { etage })
            }
            "Coupe" | "Élévation" => (modele.iter().chain(&annot).copied().collect(), modele.clone()),
            "Nomenclature" => {
                if let Some(e) = v.entite {
                    self.select(Some(e), false);
                }
                return;
            }
            _ => (annot.clone(), annot.clone()),
        };
        let (d, haut, entite) = (v.direction.as_vec3(), v.haut.as_vec3(), v.entite);
        let genre_feuille = v.genre == "Feuille";
        // mode dessin pour les plans, coupes et élévations ; coupe à hauteur sur le niveau
        // d'un plan d'étage
        let (genre, etage, echelle) = (v.genre.clone(), v.etage, v.echelle);
        match genre.as_str() {
            "Plan d'étage" | "Plan de plafond" | "Coupe" | "Élévation" => {
                self.dessin = true;
                self.coupe = genre == "Plan d'étage";
                if self.coupe {
                    self.coupe_niveau = etage;
                }
                if echelle > 0.0 {
                    self.echelle = echelle;
                }
            }
            "Vue 3D" | "Feuille" => self.dessin = false,
            _ => {}
        }
        self.vue_active = Some(i);
        self.preparer_feuille(Some(i));
        self.hidden_objects.clear();
        self.hidden_storeys.clear();
        // visibilité propre à chaque vue : celle par défaut, puis les objets isolés
        self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
        if objs.is_empty() {
            self.isolated = None;
            self.visibility_dirty = true;
        } else {
            self.isolate(objs);
        }
        // caméra : regarde vers −direction ; vue de dessus orientée par le haut de la vue
        self.camera.ortho = true;
        if d.length() > 0.5 {
            let d = d.normalize();
            self.camera.pitch = d.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
            self.camera.yaw = if d.z.abs() > 0.999 { (-haut.y).atan2(-haut.x) } else { d.y.atan2(d.x) };
        }
        if !cadre.is_empty() {
            self.frame_objects(&cadre);
        }
        if genre_feuille {
            self.cadrer_rectangle(&cadre);
        }
        if let Some(e) = entite {
            self.select(Some(e), false);
        }
    }

    /// Vue orthographique de dessus ajustée au rectangle des objets et des fenêtres de
    /// vue d'une feuille, selon les proportions de la vue.
    fn cadrer_rectangle(&mut self, objs: &[u32]) {
        let Some(s) = &self.scene else { return };
        let (mut mn, mut mx) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for &o in objs {
            mn = mn.min(s.objects[o as usize].min);
            mx = mx.max(s.objects[o as usize].max);
        }
        // cadrages des vues placées ; à défaut (vues non cadrées), étendue de leurs objets
        let cadres: Vec<([f32; 4], f32)> = self.passes_feuille.iter().filter_map(|p| Some((p.cadre?, p.z))).collect();
        let rects = if cadres.is_empty() { self.passes_feuille.iter().map(|p| (p.etendue, p.z)).collect() } else { cadres };
        for (c, z) in rects {
            mn = mn.min(Vec3::new(c[0], c[1], z));
            mx = mx.max(Vec3::new(c[2], c[3], z));
        }
        if mn.x > mx.x {
            return;
        }
        let aspect = if self.rect_vue.height() > 0.0 { self.rect_vue.width() / self.rect_vue.height() } else { 1.0 };
        let (l, h) = (mx.x - mn.x, mx.y - mn.y);
        let demi = (h / 2.0).max(l / 2.0 / aspect) * 1.04;
        self.camera.target = (mn + mx) / 2.0;
        self.camera.dist = demi / (self.camera.fov_y / 2.0).tan();
    }

    /// Passes des fenêtres de vue de la feuille `i` (aucune pour une autre vue) : pour
    /// chaque vue placée, ses objets du modèle (cadrés et découpés en profondeur) puis ses
    /// annotations, tracés à leur place sur la feuille.
    fn preparer_feuille(&mut self, i: Option<usize>) {
        let mut passes = Vec::new();
        let mut indices: Vec<[Vec<u32>; 3]> = Vec::new();
        if let Some(s) = &self.scene {
            let nav = s.navigateur.as_ref();
            let feuille = i.and_then(|i| nav?.vues.get(i)).filter(|v| v.genre == "Feuille");
            let o = s.origin;
            let modele: Vec<u32> = (0..s.objects.len() as u32).filter(|&k| !HIDDEN_BY_DEFAULT.contains(&&*s.objects[k as usize].class)).collect();
            for f in feuille.map(|v| v.fenetres.as_slice()).unwrap_or_default() {
                let Some(pv) = nav.and_then(|n| n.vues.iter().find(|x| x.revit_id == f.vue)) else { continue };
                // repère de la scène : q = p − origine, pour le modèle comme pour la feuille
                let m = DMat4::from_translation(-o) * f.vers_feuille * DMat4::from_translation(o);
                let rect = |c: [f64; 4]| [(c[0] - o.x) as f32, (c[1] - o.y) as f32, (c[2] - o.x) as f32, (c[3] - o.y) as f32];
                let plans = f.plans.iter().map(|p| Vec4::new(p.x as f32, p.y as f32, p.z as f32, (p.w + p.truncate().dot(o)) as f32)).collect();
                let dessin = matches!(pv.genre.as_str(), "Vue de dessin" | "Légende" | "Nomenclature");
                // objets du modèle, sauf ceux masqués dans la vue placée (id Revit : 8ᵉ
                // argument, l'étiquette, des produits de la scène)
                let masque = |k: &u32| match s.model.get(s.objects[*k as usize].id).and_then(|e| e.args.get(7)) {
                    Some(crate::step::Value::Str(t)) => t.parse::<i64>().is_ok_and(|id| f.masques.contains(&id)),
                    _ => false,
                };
                let objets: Vec<u32> = if dessin { Vec::new() } else { modele.iter().filter(|k| !masque(k)).copied().collect() };
                // éléments de détail (traits, régions, composants de détail) découpés au
                // cadrage comme le modèle ; annotations (textes, cotes, symboles) non
                // découpées, sauf cadrage d'annotation
                let genre = |k: u32| match s.model.get(s.objects[k as usize].id).and_then(|e| e.args.get(4)) {
                    Some(crate::step::Value::Str(g)) => g.clone(),
                    _ => String::new(),
                };
                let (detail, annot): (Vec<u32>, Vec<u32>) = pv.entite.map(|e| self.objects_under(e)).unwrap_or_default().into_iter().partition(|&k| DETAIL.contains(&genre(k).as_str()));
                let passes_vue = [(objets, f.cadre.map(rect), plans), (detail, f.cadre.map(rect), Vec::new()), (annot, f.cadre_annotations.map(rect), Vec::new())];
                for (objs, cadre, plans) in passes_vue {
                    if objs.is_empty() {
                        continue;
                    }
                    let mut ix: [Vec<u32>; 3] = Default::default();
                    for &k in &objs {
                        let ob = &s.objects[k as usize];
                        ix[0].extend(ob.opaque.start..ob.opaque.start + ob.opaque.count);
                        ix[1].extend(ob.transparent.start..ob.transparent.start + ob.transparent.count);
                        ix[2].extend(ob.lines.start..ob.lines.start + ob.lines.count);
                    }
                    indices.push(ix);
                    let mf = m.as_mat4();
                    let mut etendue = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                    for &k in &objs {
                        let ob = &s.objects[k as usize];
                        for c in 0..8 {
                            let q = Vec3::new([ob.min.x, ob.max.x][c & 1], [ob.min.y, ob.max.y][(c >> 1) & 1], [ob.min.z, ob.max.z][c >> 2]);
                            let q = mf.transform_point3(q);
                            etendue = [etendue[0].min(q.x), etendue[1].min(q.y), etendue[2].max(q.x), etendue[3].max(q.y)];
                        }
                    }
                    passes.push(PasseFeuille { vers_feuille: mf, cadre, etendue, z: -o.z as f32, plans, direction: f.direction.as_vec3() });
                }
            }
        }
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_passes(gl, &indices);
        }
        self.passes_feuille = passes;
    }

    fn tree_roots(&self) -> Vec<Id> {
        let s = self.scene.as_ref().unwrap();
        let mut roots: Vec<Id> = s.model.project.into_iter().collect();
        // Produits hors arborescence spatiale.
        let orphans = s.model.products.iter().any(|p| !s.model.parent.contains_key(p));
        if orphans {
            roots.push(0);
        }
        roots
    }

    fn tree_node(&mut self, ui: &mut egui::Ui, id: Id, depth: u32) {
        if depth > 32 {
            return;
        }
        let s = self.scene.as_ref().unwrap();
        let model = &s.model;
        let (label, children): (String, Vec<Id>) = if id == 0 {
            let orph: Vec<Id> = model.products.iter().copied().filter(|p| !model.parent.contains_key(p)).collect();
            ("Hors arborescence".into(), orph)
        } else {
            let ty = model.ty(id);
            let name = model.name(id);
            let label = if SPATIAL.contains(&ty) {
                format!("{} {}", pretty_type(ty).trim_start_matches("Ifc"), name)
            } else {
                model.label(id)
            };
            (label, model.children.get(&id).cloned().unwrap_or_default())
        };

        // Enfants spatiaux en nœuds, autres regroupés par classe.
        let mut spatial = Vec::new();
        let mut groups: BTreeMap<String, Vec<Id>> = BTreeMap::new();
        for c in children {
            let ty = model.ty(c);
            if SPATIAL.contains(&ty) {
                spatial.push(c);
            } else {
                groups.entry(ty.to_string()).or_default().push(c);
            }
        }
        let is_storey = model.ty(id) == "IFCBUILDINGSTOREY";
        let reveal_path = self.reveal.is_some_and(|r| self.is_ancestor(id, r));
        let egui_id = ui.make_persistent_id(("node", id));
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), egui_id, depth < 3);
        if reveal_path {
            state.set_open(true);
        }
        let selected = self.selected == Some(id);
        let leaf = spatial.is_empty() && groups.is_empty();
        let mut clicked = false;
        let mut storey_toggle = None;
        let mut header = |ui: &mut egui::Ui| {
            if is_storey {
                let mut shown = !self.hidden_storeys.contains(&id);
                if ui.checkbox(&mut shown, "").on_hover_text("Afficher le niveau").changed() {
                    storey_toggle = Some(shown);
                }
            }
            let r = ui.selectable_label(selected, label.clone());
            if self.reveal == Some(id) {
                r.scroll_to_me(Some(Align::Center));
            }
            clicked = r.clicked();
        };
        if leaf {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                header(ui);
            });
        } else {
            state
                .show_header(ui, |ui| header(ui))
                .body(|ui| {
                    for c in spatial {
                        self.tree_node(ui, c, depth + 1);
                    }
                    for (ty, items) in groups {
                        self.class_group(ui, id, &ty, items, depth + 1);
                    }
                });
        }
        if let Some(shown) = storey_toggle {
            if shown {
                self.hidden_storeys.remove(&id);
            } else {
                self.hidden_storeys.insert(id);
            }
            self.visibility_dirty = true;
        }
        if clicked && id != 0 {
            self.select(Some(id), false);
        }
    }

    fn class_group(&mut self, ui: &mut egui::Ui, parent: Id, ty: &str, items: Vec<Id>, depth: u32) {
        let egui_id = ui.make_persistent_id(("group", parent, ty));
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), egui_id, false);
        if self.reveal.is_some_and(|r| items.iter().any(|&i| i == r || self.is_ancestor(i, r))) {
            state.set_open(true);
        }
        state
            .show_header(ui, |ui| {
                ui.label(RichText::new(format!("{} ({})", pretty_type(ty), items.len())).color(Color32::from_gray(90)));
            })
            .body(|ui| {
                for i in items {
                    self.tree_node(ui, i, depth + 1);
                }
            });
    }

    fn is_ancestor(&self, anc: Id, mut id: Id) -> bool {
        let Some(s) = &self.scene else { return false };
        if anc == 0 {
            // Racine des orphelins
            while let Some(&p) = s.model.parent.get(&id) {
                id = p;
            }
            return s.model.project != Some(id);
        }
        for _ in 0..64 {
            if id == anc {
                return true;
            }
            match s.model.parent.get(&id) {
                Some(&p) => id = p,
                None => return false,
            }
        }
        false
    }

    fn class_list(&mut self, ui: &mut egui::Ui) {
        let s = self.scene.as_ref().unwrap();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for o in &s.objects {
            *counts.entry(o.class.to_string()).or_default() += 1;
        }
        ui.horizontal(|ui| {
            if ui.small_button("Tout").clicked() {
                self.hidden_classes.clear();
                self.visibility_dirty = true;
            }
            if ui.small_button("Rien").clicked() {
                self.hidden_classes = counts.keys().cloned().collect();
                self.visibility_dirty = true;
            }
        });
        for (cls, n) in counts {
            let mut shown = !self.hidden_classes.contains(&cls);
            if ui.checkbox(&mut shown, format!("{} ({n})", pretty_type(&cls))).changed() {
                if shown {
                    self.hidden_classes.remove(&cls);
                } else {
                    self.hidden_classes.insert(cls);
                }
                self.visibility_dirty = true;
            }
        }
    }

    fn filtered_list(&mut self, ui: &mut egui::Ui) {
        let s = self.scene.as_ref().unwrap();
        let q = self.filter.trim().to_lowercase();
        let mut hits = Vec::new();
        for &p in &s.model.products {
            let name = s.model.name(p);
            let ty = s.model.ty(p);
            let gid = s.model.global_id(p);
            if name.to_lowercase().contains(&q) || ty.to_lowercase().contains(&q) || gid.to_lowercase() == q {
                hits.push((p, format!("{}  ·  {}", s.model.label(p), pretty_type(ty))));
                if hits.len() >= 2000 {
                    break;
                }
            }
        }
        ui.label(RichText::new(format!("{} résultat(s){}", hits.len(), if hits.len() >= 2000 { " (limité)" } else { "" })).weak());
        let mut clicked = None;
        for (p, label) in hits {
            if ui.selectable_label(self.selected == Some(p), label).clicked() {
                clicked = Some(p);
            }
        }
        if let Some(p) = clicked {
            self.select(Some(p), false);
        }
    }

    /// Propriétés de l'élément sélectionné (panneau de gauche, comme dans Revit).
    fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Propriétés");
        if let (Some(s), Some(id)) = (&self.scene, self.selected) {
            // en-tête façon Revit : classe et type de l'élément
            let ty = s.model.type_of.get(&id).map(|t| s.model.name(*t)).filter(|t| !t.is_empty());
            let classe = pretty_type(s.model.ty(id)).trim_start_matches("Ifc").to_string();
            ui.label(RichText::new(match ty {
                Some(t) => format!("{classe} : {t}"),
                None => classe,
            }).color(Color32::from_gray(70)));
        }
        ui.separator();
        let (Some(s), Some(id)) = (&self.scene, self.selected) else {
            ui.label(RichText::new("Sélectionner un élément dans la vue ou l'arborescence.").weak());
            return;
        };
        let model = &s.model;
        let has_obj = s.object_of.contains_key(&id);
        let under = self.objects_under(id);
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!under.is_empty(), egui::Button::new("Cadrer")).clicked() {
                action = Some("frame");
            }
            if ui.add_enabled(!under.is_empty(), egui::Button::new("Isoler")).on_hover_text("I").clicked() {
                action = Some("isolate");
            }
            if ui.add_enabled(!under.is_empty(), egui::Button::new("Masquer")).on_hover_text("H").clicked() {
                action = Some("hide");
            }
            if ui.button("Copier GlobalId").clicked() {
                ui.ctx().copy_text(model.global_id(id));
            }
        });
        if !has_obj && under.is_empty() {
            ui.label(RichText::new("Pas de géométrie affichable.").weak());
        }
        ui.add_space(4.0);
        let attrs = model.attributes(id);
        let groups = model.property_groups(id);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("attrs").num_columns(2).striped(true).show(ui, |ui| {
                for (k, v) in attrs {
                    ui.label(RichText::new(k).strong());
                    ui.add(egui::Label::new(v).wrap());
                    ui.end_row();
                }
            });
            for (gi, g) in groups.into_iter().enumerate() {
                egui::CollapsingHeader::new(RichText::new(&g.name).strong())
                    .id_salt(("pset", id, gi))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new(("pgrid", id, gi)).num_columns(2).striped(true).show(ui, |ui| {
                            for (k, v) in g.props {
                                ui.label(k);
                                ui.add(egui::Label::new(v).wrap());
                                ui.end_row();
                            }
                        });
                    });
            }
        });
        match action {
            Some("frame") => self.frame_objects(&under),
            Some("isolate") => self.isolate(under),
            Some("hide") => self.hide(under),
            _ => {}
        }
    }

    fn isolate(&mut self, objs: Vec<u32>) {
        if objs.is_empty() {
            return;
        }
        // Les classes masquées par défaut doivent rester visibles si on les isole.
        if let Some(s) = &self.scene {
            for &o in &objs {
                self.hidden_classes.remove(&*s.objects[o as usize].class);
            }
        }
        self.isolated = Some(objs.into_iter().collect());
        self.visibility_dirty = true;
    }

    fn hide(&mut self, objs: Vec<u32>) {
        self.hidden_objects.extend(objs);
        self.visibility_dirty = true;
    }

    /// Fiche d'un fichier Revit : vignette, version, informations projet, références.
    fn revit_view(&mut self, ui: &mut egui::Ui) {
        if self.rvt_preview.is_none() {
            if let Some(img) = self.rvt.as_ref().and_then(|i| i.preview_png.as_deref()).and_then(decode_png) {
                self.rvt_preview = Some(ui.ctx().load_texture("rvt-preview", img, egui::TextureOptions::LINEAR));
            }
        }
        let Some(info) = &self.rvt else { return };
        let preview = self.rvt_preview.clone();
        let mut close = false;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(980.0);
            ui.horizontal(|ui| {
                ui.heading(info.title.as_deref().unwrap_or(&info.file_name));
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Fermer").clicked() {
                        close = true;
                    }
                });
            });
            let mut sub = format!("{} Revit {} · {} · {:.1} Mo", info.kind, info.format().unwrap_or("?"), info.file_name, info.file_size as f64 / 1e6);
            if let Some(c) = &info.category {
                sub += &format!(" · catégorie {c}");
            }
            ui.label(RichText::new(sub).color(Color32::from_gray(90)));
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(255, 246, 214))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(222, 196, 120)))
                .inner_margin(8.0)
                .corner_radius(4.0)
                .show(ui, |ui| match &self.rvt_geom {
                    Ok(summary) => {
                        ui.label(format!(
                            "Vue 3D : {summary}, avec la géométrie calculée par Revit (murs, sols, toits, familles, \
                             escaliers…), vues et éléments 2D, paramètres. Le contenu des nomenclatures et les valeurs \
                             calculées (surfaces, volumes) ne sont pas encore lus."
                        ));
                    }
                    Err(e) => {
                        ui.label(format!(
                            "Métadonnées seules : pas de murs affichables ({e}). Pour la 3D, exporter le modèle en IFC \
                             depuis Revit (Fichier › Exporter › IFC), puis ouvrir le fichier .ifc."
                        ));
                    }
                });
            ui.add_space(8.0);

            ui.horizontal_top(|ui| {
                if let Some(tex) = &preview {
                    let size = tex.size_vec2();
                    let scale = (256.0 / size.x.max(size.y)).clamp(1.0, 4.0);
                    egui::Frame::new().stroke(egui::Stroke::new(1.0, Color32::from_gray(200))).show(ui, |ui| {
                        ui.image((tex.id(), size * scale));
                    });
                    ui.add_space(12.0);
                }
                egui::Grid::new("rvt-main").num_columns(2).striped(true).show(ui, |ui| {
                    for key in ["Format", "Build", "Worksharing", "Central Model Path", "Last Save Path", "Locale when saved", "Unique Document GUID"] {
                        if let Some(v) = info.basic_value(key).filter(|v| !v.is_empty()) {
                            ui.label(RichText::new(rvt::basic_label(key)).strong());
                            ui.add(egui::Label::new(v).wrap());
                            ui.end_row();
                        }
                    }
                    if let Some(u) = &info.updated {
                        ui.label(RichText::new("Mise à jour").strong());
                        ui.label(u);
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);

            let param_grid = |ui: &mut egui::Ui, id: (&str, usize), params: &[rvt::Param]| {
                egui::Grid::new(id).num_columns(2).striped(true).min_col_width(220.0).show(ui, |ui| {
                    for p in params {
                        let hint = [p.kind.as_str(), p.origin.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                        let r = ui.label(&p.name);
                        if !hint.is_empty() {
                            r.on_hover_text(hint);
                        }
                        let v = match &p.units {
                            Some(u) => format!("{} {u}", p.value),
                            None => p.value.clone(),
                        };
                        ui.add(egui::Label::new(v).wrap());
                        ui.end_row();
                    }
                });
            };

            if !info.groups.is_empty() {
                let title = if info.kind == "Famille" { "Paramètres de famille" } else { "Informations sur le projet" };
                egui::CollapsingHeader::new(RichText::new(title).strong()).default_open(true).show(ui, |ui| {
                    for (i, g) in info.groups.iter().enumerate() {
                        ui.label(RichText::new(&g.title).color(Color32::from_gray(90)));
                        param_grid(ui, ("rvt-group", i), &g.params);
                        ui.add_space(4.0);
                    }
                });
            }
            if !info.family_types.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Types de famille ({})", info.family_types.len())).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for (i, t) in info.family_types.iter().enumerate() {
                            egui::CollapsingHeader::new(&t.title).id_salt(("rvt-type", i)).default_open(info.family_types.len() <= 3).show(ui, |ui| {
                                param_grid(ui, ("rvt-type-grid", i), &t.params);
                            });
                        }
                    });
            }
            if !info.external_refs.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Références externes ({})", info.external_refs.len())).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("rvt-refs").num_columns(3).striped(true).show(ui, |ui| {
                            for r in &info.external_refs {
                                ui.label(&r.kind);
                                let path = if r.path.is_empty() { "—" } else { r.path.as_str() };
                                let lbl = ui.add(egui::Label::new(path).wrap());
                                if !r.absolute_path.is_empty() {
                                    lbl.on_hover_text(&r.absolute_path);
                                }
                                let state = load_state_fr(&r.load_state);
                                let color = if r.load_state == "Loaded" { Color32::from_rgb(40, 130, 60) } else { Color32::from_rgb(180, 90, 30) };
                                ui.colored_label(color, state);
                                ui.end_row();
                            }
                        });
                    });
            }
            egui::CollapsingHeader::new(RichText::new("Informations de fichier (BasicFileInfo)").strong()).show(ui, |ui| {
                egui::Grid::new("rvt-basic").num_columns(2).striped(true).show(ui, |ui| {
                    for (k, v) in &info.basic {
                        ui.label(rvt::basic_label(k)).on_hover_text(k);
                        ui.add(egui::Label::new(v).wrap());
                        ui.end_row();
                    }
                });
            });
            egui::CollapsingHeader::new(RichText::new(format!("Structure du conteneur ({} flux)", info.streams.iter().filter(|s| !s.1).count())).strong())
                .show(ui, |ui| {
                    egui::Grid::new("rvt-streams").num_columns(2).striped(true).show(ui, |ui| {
                        for (path, is_dir, size) in &info.streams {
                            ui.label(RichText::new(path).monospace());
                            ui.label(if *is_dir { "dossier".to_string() } else { format!("{size} o") });
                            ui.end_row();
                        }
                    });
                });
            for w in &info.warnings {
                ui.colored_label(Color32::from_rgb(180, 90, 30), format!("Avertissement : {w}"));
            }
        });
        if close {
            if self.rvt_geom.is_ok() {
                // Retour à la vue 3D des murs.
                self.rvt_sheet = false;
            } else {
                self.rvt = None;
                self.rvt_sheet = false;
                self.rvt_preview = None;
                self.status = "Ouvrir un fichier IFC ou Revit (Ctrl+O), ou le glisser dans la fenêtre.".into();
            }
        }
    }

    fn unsupported_window(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.scene else { return };
        let mut open = self.show_unsupported;
        egui::Window::new("Entités non prises en charge").open(&mut open).show(ctx, |ui| {
            ui.label("Ces représentations ont été ignorées (aucune géométrie générée) :");
            egui::Grid::new("unsup").striped(true).show(ui, |ui| {
                for (t, n) in &s.unsupported {
                    ui.label(pretty_type(t));
                    ui.label(n.to_string());
                    ui.end_row();
                }
            });
        });
        self.show_unsupported = open;
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        // fenêtre réduite : on garde la dernière zone, taille des captures hors écran
        if rect.is_positive() {
            self.rect_vue = rect;
        }
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let aspect = rect.width() / rect.height().max(1.0);
        let ndc = |p: egui::Pos2| {
            Vec2::new((p.x - rect.left()) / rect.width() * 2.0 - 1.0, 1.0 - (p.y - rect.top()) / rect.height() * 2.0)
        };

        if self.scene.is_some() {
            let pick = |app: &Self, p: egui::Pos2| -> Option<(u32, Vec3)> {
                let s = app.scene.as_ref()?;
                let n = ndc(p);
                let (o, d) = app.camera.ray(n.x, n.y, aspect);
                s.pick(o, d, &app.visible, app.plan_coupe()).map(|(obj, t)| (obj, o + d * t))
            };

            // Orbite autour du point visé.
            if resp.drag_started_by(PointerButton::Primary) && !ui.input(|i| i.modifiers.shift) {
                if let Some((_, p)) = resp.interact_pointer_pos().and_then(|pp| pick(self, pp)) {
                    self.camera.set_pivot(p);
                }
            }
            let d = resp.drag_delta();
            let shift = ui.input(|i| i.modifiers.shift);
            if resp.dragged_by(PointerButton::Primary) && !shift {
                self.camera.orbit(d.x, d.y);
            } else if resp.dragged_by(PointerButton::Secondary)
                || resp.dragged_by(PointerButton::Middle)
                || (resp.dragged_by(PointerButton::Primary) && shift)
            {
                self.camera.pan(d.x, d.y, rect.height());
            }

            if let Some(hp) = resp.hover_pos() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let factor = (-scroll * 0.0018).exp();
                    let target = pick(self, hp).map(|(_, p)| p).or_else(|| {
                        // Point sur le plan de visée à la distance de la cible.
                        let n = ndc(hp);
                        let (o, dir) = self.camera.ray(n.x, n.y, aspect);
                        let f = self.camera.forward();
                        let t = (self.camera.target - o).dot(f) / dir.dot(f).max(1e-6);
                        Some(o + dir * t)
                    });
                    self.camera.zoom(factor, if self.camera.ortho { None } else { target });
                }
            }

            if resp.double_clicked() {
                match resp.interact_pointer_pos().and_then(|p| pick(self, p)) {
                    Some((obj, _)) => self.frame_objects(&[obj]),
                    None => self.fit_all(),
                }
            } else if resp.clicked() {
                let hit = resp.interact_pointer_pos().and_then(|p| pick(self, p));
                let id = hit.map(|(obj, _)| self.scene.as_ref().unwrap().objects[obj as usize].id);
                self.select(id, true);
            }

            if resp.hovered() || resp.has_focus() {
                let (f, esc, h, i) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::F),
                        i.key_pressed(egui::Key::Escape),
                        i.key_pressed(egui::Key::H),
                        i.key_pressed(egui::Key::I),
                    )
                });
                if f {
                    match self.selected.map(|s| self.objects_under(s)) {
                        Some(o) if !o.is_empty() => self.frame_objects(&o),
                        _ => self.fit_all(),
                    }
                }
                if esc {
                    if self.isolated.is_some() {
                        self.isolated = None;
                        self.visibility_dirty = true;
                    } else {
                        self.select(None, false);
                    }
                }
                if let Some(sel) = self.selected {
                    if h {
                        let o = self.objects_under(sel);
                        self.hide(o);
                    }
                    if i {
                        let o = self.objects_under(sel);
                        self.isolate(o);
                    }
                }
            }
        }

        self.update_visibility();
        self.update_coupe();

        let params = self.frame_params(aspect);
        let renderer = self.renderer.clone();
        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, painter| {
                let vp = info.viewport_in_pixels();
                if let Some(r) = renderer.lock().unwrap().as_ref() {
                    r.paint(painter.gl(), &params, [vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px]);
                }
            })),
        };
        ui.painter().add(callback);

        // onglet de la vue ouverte, comme dans Revit
        if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            let texte = if v.nom.is_empty() { v.genre.clone() } else { format!("{} : {}", v.genre, v.nom) };
            let galet = egui::Rect::from_min_size(rect.left_top() + egui::vec2(6.0, 6.0), egui::vec2(texte.len() as f32 * 7.2 + 20.0, 22.0));
            ui.painter().rect_filled(galet, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230));
            ui.painter().text(galet.left_center() + egui::vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, texte, egui::FontId::proportional(14.0), Color32::from_gray(40));
        }
        if self.scene.is_none() {
            let msg = if self.loading.is_some() { self.status.clone() } else { "Glisser un fichier .ifc ou .rvt ici\nou Ctrl+O".to_string() };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(20.0), Color32::from_gray(110));
        } else if self.loading.is_some() {
            ui.painter().text(
                rect.left_top() + egui::vec2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                &self.status,
                egui::FontId::proportional(15.0),
                Color32::from_gray(60),
            );
        }
    }
}

// ------------------------------------------------------------------ pilotage

impl ViewerApp {
    /// Traite les commandes du pilote ; répond aux ouvertures terminées.
    fn piloter(&mut self, ctx: &egui::Context) {
        if self.loading.is_none() && !self.attente_ouverture.is_empty() {
            let etat = match &self.error {
                Some(e) => pilote::erreur(e),
                None => self.etat(),
            };
            for tx in self.attente_ouverture.drain(..) {
                let _ = tx.send(etat.clone());
            }
        }
        for r in std::mem::take(&mut self.requetes) {
            let reponse = match r.commande.as_str() {
                "ouvrir" => {
                    let p = PathBuf::from(r.argument.trim_matches('"'));
                    if p.is_file() {
                        self.open(p, ctx);
                        self.attente_ouverture.push(r.reponse);
                        continue;
                    }
                    pilote::erreur(&format!("fichier introuvable : {}", r.argument))
                }
                "capture" => {
                    self.capture(ctx, &r.argument, r.reponse);
                    continue;
                }
                _ => self.commande(ctx, &r.commande, &r.argument),
            };
            let _ = r.reponse.send(reponse);
        }
    }

    fn frame_params(&self, aspect: f32) -> FrameParams {
        let vp = self.camera.view_proj(aspect);
        let passes = self
            .passes_feuille
            .iter()
            .map(|p| {
                // rectangle de découpe : coins projetés, en coordonnées normalisées
                let ciseaux = p.cadre.map(|c| {
                    let coins = [[c[0], c[1]], [c[2], c[1]], [c[2], c[3]], [c[0], c[3]]].map(|[x, y]| vp.project_point3(Vec3::new(x, y, p.z)));
                    let (mn, mx) = coins.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), q| (a.min(*q), b.max(*q)));
                    [mn.x, mn.y, mx.x, mx.y]
                });
                ParamsPasse { mvp: vp * p.vers_feuille, view_dir: -p.direction, ciseaux, plans: p.plans.clone() }
            })
            .collect();
        FrameParams {
            passes,
            mvp: vp,
            eye: self.camera.eye(),
            view_dir: self.camera.forward(),
            ortho: self.camera.ortho,
            selected: self.selected_object().unwrap_or(u32::MAX),
            background: if self.dessin { [1.0, 1.0, 1.0] } else { [0.925, 0.933, 0.945] },
            dessin: self.dessin,
            coupe: self.plan_coupe(),
        }
    }

    /// Capture en PNG, argument « vue|fenetre [LxH] chemin ». La vue 3D est rendue hors
    /// écran, à la taille de la vue ou à celle demandée, même fenêtre réduite ; la
    /// fenêtre entière (interface comprise) est lue à l'image suivante.
    fn capture(&mut self, ctx: &egui::Context, argument: &str, reponse: Sender<String>) {
        let (zone, reste) = argument.trim().split_once(' ').unwrap_or((argument, ""));
        let mut reste = reste.trim();
        let mut taille = None;
        if let Some((t, apres)) = reste.split_once(' ') {
            if let Some((Ok(l), Ok(h))) = t.split_once('x').map(|(l, h)| (l.parse::<u32>(), h.parse::<u32>())) {
                taille = Some((l, h));
                reste = apres.trim();
            }
        }
        let chemin = reste.trim_matches('"').to_string();
        if chemin.is_empty() {
            let _ = reponse.send(pilote::erreur("usage : capture vue|fenetre [LxH] <chemin.png>"));
            return;
        }
        let ecrire = move |l: usize, h: usize, rgba: &[u8]| {
            let res = pilote::png(l, h, rgba).and_then(|png| std::fs::write(&chemin, png).map_err(|e| e.to_string()));
            match res {
                Ok(()) => pilote::succes(&[("chemin", pilote::texte(&chemin)), ("largeur", l.to_string()), ("hauteur", h.to_string())]),
                Err(e) => pilote::erreur(&e),
            }
        };
        if zone == "fenetre" {
            // egui_glow ne lit pas une fenêtre de taille nulle (réduite)
            if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
                let _ = reponse.send(pilote::erreur("fenêtre réduite : seule la vue 3D se capture (zone « vue »)"));
                return;
            }
            ctx.request_screenshot(move |image| {
                let [w, h] = image.size;
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b(), 255]).collect();
                let _ = reponse.send(ecrire(w, h, &rgba));
            });
            ctx.request_repaint();
            return;
        }
        let ppp = ctx.pixels_per_point();
        let (l, h) = taille.unwrap_or(((self.rect_vue.width() * ppp).round() as u32, (self.rect_vue.height() * ppp).round() as u32));
        let (l, h) = (l.clamp(16, 4096), h.clamp(16, 4096));
        self.update_visibility();
        self.update_coupe();
        let params = self.frame_params(l as f32 / h as f32);
        let res = match (&self.gl, self.renderer.lock().unwrap().as_ref()) {
            (Some(gl), Some(r)) => r.capture(gl, &params, l as i32, h as i32),
            _ => Err("rendu OpenGL indisponible".into()),
        };
        let _ = reponse.send(match res {
            Ok(rgba) => ecrire(l as usize, h as usize, &rgba),
            Err(e) => pilote::erreur(&e),
        });
    }

    /// État du viewer : fichier, vue, caméra, visibilité, sélection.
    fn etat(&self) -> String {
        use pilote::{liste, objet, texte};
        let mut p = vec![
            ("fichier", self.scene.as_ref().map(|s| texte(&s.file_name)).unwrap_or_else(|| "null".into())),
            ("statut", texte(&self.status)),
            ("chargement", self.loading.is_some().to_string()),
            ("fiche_revit", (self.rvt.is_some() && self.rvt_sheet).to_string()),
        ];
        if let Some(e) = &self.error {
            p.push(("erreur_chargement", texte(e)));
        }
        if let Some(s) = &self.scene {
            p.push(("schema", texte(&s.model.file.schema)));
            p.push(("objets", s.objects.len().to_string()));
            p.push(("objets_visibles", self.visible.iter().filter(|v| **v).count().to_string()));
            p.push(("triangles", s.triangles.to_string()));
            p.push(("vues_revit", s.navigateur.as_ref().map_or(0, |n| n.vues.len()).to_string()));
        }
        if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            p.push(("vue", objet(&[("indice", self.vue_active.unwrap_or(0).to_string()), ("genre", texte(&v.genre)), ("nom", texte(&v.nom)), ("echelle", v.echelle.to_string())])));
        }
        let c = &self.camera;
        p.push((
            "camera",
            objet(&[
                ("lacet_deg", format!("{:.2}", c.yaw.to_degrees())),
                ("tangage_deg", format!("{:.2}", c.pitch.to_degrees())),
                ("distance", format!("{:.3}", c.dist)),
                ("cible", format!("[{:.3},{:.3},{:.3}]", c.target.x, c.target.y, c.target.z)),
                ("ortho", c.ortho.to_string()),
            ]),
        ));
        p.push((
            "dessin",
            objet(&[
                ("actif", self.dessin.to_string()),
                ("coupe_z", self.plan_coupe().map_or("null".into(), |c| format!("{:.3}", c.w))),
                ("niveau", self.coupe_niveau.map_or("null".into(), |n| self.decrire(n))),
                ("hauteur", format!("{:.3}", self.coupe_hauteur)),
                ("poche", texte(self.poche.nom())),
                ("echelle", self.echelle.to_string()),
            ]),
        ));
        let mut masquees: Vec<&String> = self.hidden_classes.iter().collect();
        masquees.sort();
        p.push(("classes_masquees", liste(masquees.into_iter().map(|c| texte(c)))));
        p.push(("isolement", self.isolated.as_ref().map_or("null".into(), |i| i.len().to_string())));
        if let Some(id) = self.selected {
            p.push(("selection", self.decrire(id)));
        }
        p.push(("vue_px", format!("[{:.0},{:.0}]", self.rect_vue.width(), self.rect_vue.height())));
        pilote::succes(&p)
    }

    fn decrire(&self, id: Id) -> String {
        let Some(s) = &self.scene else { return "null".into() };
        pilote::objet(&[("id", id.to_string()), ("classe", pilote::texte(s.model.ty(id))), ("nom", pilote::texte(&s.model.name(id)))])
    }

    /// Entité désignée par « #123 », « 123 » ou, à défaut, la sélection.
    fn entite(&self, argument: &str) -> Result<Id, String> {
        let s = self.scene.as_ref().ok_or("aucun fichier ouvert")?;
        let a = argument.trim().trim_start_matches('#');
        let id = if a.is_empty() { self.selected.ok_or("aucune sélection")? } else { a.parse::<Id>().map_err(|_| format!("id invalide : {argument}"))? };
        if s.model.get(id).is_none() {
            return Err(format!("entité #{id} absente"));
        }
        Ok(id)
    }

    fn commande(&mut self, ctx: &egui::Context, commande: &str, argument: &str) -> String {
        use pilote::{erreur, liste, objet, succes, texte};
        match commande {
            "etat" => self.etat(),
            "taille" => {
                let v: Vec<f32> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let [l, h] = v[..] else { return erreur("usage : taille <largeur> <hauteur>") };
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(l, h)));
                succes(&[])
            }
            "reduire" | "restaurer" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(commande == "reduire"));
                succes(&[])
            }
            "quitter" => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                succes(&[])
            }
            "fiche" => {
                self.rvt_sheet = self.rvt.is_some() && argument != "non";
                self.etat()
            }
            "onglet" => {
                let a = if argument == "projet" { "tout" } else { argument };
                let Some(&o) = LeftTab::REVIT.iter().find(|o| o.nom() == a) else {
                    return erreur("usage : onglet tout|vues|feuilles|nomenclatures|familles|materiaux|groupes|liens|modele|classes");
                };
                self.left_tab = o;
                self.etat()
            }
            "vues" => {
                let Some(nav) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()) else { return erreur("pas de vues Revit") };
                let vues = nav.vues.iter().enumerate().map(|(i, v)| {
                    objet(&[("indice", i.to_string()), ("genre", texte(&v.genre)), ("type", texte(&v.type_vue)), ("nom", texte(&v.nom)), ("echelle", v.echelle.to_string()), ("numero", texte(&v.numero))])
                });
                succes(&[("vues", liste(vues))])
            }
            "vue" => {
                let Some(nav) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()) else { return erreur("pas de vues Revit") };
                let a = argument.to_lowercase();
                let nom = |v: &scene::NavVue| format!("{} : {}", v.genre, v.nom).to_lowercase();
                let i = argument
                    .parse::<usize>()
                    .ok()
                    .filter(|&i| i < nav.vues.len())
                    .or_else(|| nav.vues.iter().position(|v| v.nom.to_lowercase() == a || nom(v) == a))
                    .or_else(|| nav.vues.iter().position(|v| nom(v).contains(&a)));
                let Some(i) = i else { return erreur(&format!("vue introuvable : {argument}")) };
                self.rvt_sheet = false;
                self.activer_vue(i);
                self.etat()
            }
            "camera" => {
                let c = &mut self.camera;
                match argument {
                    "dessus" => {
                        c.pitch = MAX_PITCH;
                        c.yaw = -std::f32::consts::FRAC_PI_2;
                    }
                    "face" => {
                        c.pitch = 0.0;
                        c.yaw = -std::f32::consts::FRAC_PI_2;
                    }
                    "iso" => {
                        c.pitch = 0.6;
                        c.yaw = -2.3;
                    }
                    "ortho" => c.ortho = true,
                    "perspective" => c.ortho = false,
                    "cadrer" => self.fit_all(),
                    _ => {
                        let v: Vec<f32> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                        if v.len() < 2 {
                            return erreur("usage : camera dessus|face|iso|cadrer|ortho|perspective, ou camera <lacet°> <tangage°> [distance]");
                        }
                        c.yaw = v[0].to_radians();
                        c.pitch = v[1].to_radians().clamp(-MAX_PITCH, MAX_PITCH);
                        if let Some(&d) = v.get(2) {
                            c.dist = d.max(1e-3);
                        }
                    }
                }
                self.etat()
            }
            "dessin" => {
                self.dessin = argument != "non";
                self.visibility_dirty = true;
                self.etat()
            }
            "coupe" => {
                // coupe non | coupe <hauteur m> [niveau : id ou nom]
                if argument == "non" {
                    self.coupe = false;
                } else {
                    let (h, niveau) = argument.split_once(' ').unwrap_or((argument, ""));
                    if !h.is_empty() {
                        let Ok(h) = h.replace(',', ".").parse::<f32>() else { return erreur("usage : coupe non | coupe <hauteur m> [niveau]") };
                        self.coupe_hauteur = h;
                    }
                    let niveau = niveau.trim().trim_start_matches('#').to_lowercase();
                    if !niveau.is_empty() {
                        let n = self.niveaux().into_iter().find(|x| x.0.to_string() == niveau || x.1.to_lowercase() == niveau || x.1.to_lowercase().contains(&niveau));
                        let Some(n) = n else { return erreur(&format!("niveau introuvable : {niveau}")) };
                        self.coupe_niveau = Some(n.0);
                    }
                    self.dessin = true;
                    self.coupe = true;
                }
                self.visibility_dirty = true;
                self.etat()
            }
            "exporter" => {
                // exporter <chemin.svg> : dessin vectoriel de la vue
                let chemin = argument.trim().trim_matches('"');
                if !chemin.to_lowercase().ends_with(".svg") {
                    return erreur("usage : exporter <chemin.svg>");
                }
                match self.exporter_svg(std::path::Path::new(chemin)) {
                    Ok(n) => succes(&[("chemin", texte(chemin)), ("primitives", n.to_string())]),
                    Err(e) => erreur(&e),
                }
            }
            "niveaux" => {
                let n = self.niveaux().into_iter().map(|(id, nom, z)| objet(&[("id", id.to_string()), ("nom", texte(&nom)), ("z", format!("{z:.3}"))]));
                succes(&[("niveaux", liste(n))])
            }
            "poche" => {
                let Some(p) = Poche::depuis(argument) else { return erreur("usage : poche hachure|plein|vide") };
                self.poche = p;
                self.visibility_dirty = true;
                self.etat()
            }
            "echelle" => {
                let Some(e) = argument.trim_start_matches("1:").parse::<f64>().ok().filter(|e| *e > 0.0) else { return erreur("usage : echelle <dénominateur> (100 pour 1:100)") };
                self.echelle = e;
                self.visibility_dirty = true;
                self.etat()
            }
            "zoom" => {
                let Ok(f) = argument.parse::<f32>() else { return erreur("usage : zoom <facteur> (2 = deux fois plus près)") };
                self.camera.zoom(1.0 / f.max(1e-3), None);
                self.etat()
            }
            "afficher" | "masquer" => {
                for c in argument.split([' ', ',']).filter(|c| !c.is_empty()) {
                    let c = c.to_uppercase();
                    if commande == "afficher" {
                        self.hidden_classes.remove(&c);
                    } else {
                        self.hidden_classes.insert(c);
                    }
                }
                self.visibility_dirty = true;
                self.etat()
            }
            "tout_afficher" => {
                self.hidden_objects.clear();
                self.hidden_storeys.clear();
                self.isolated = None;
                self.vue_active = None;
                self.visibility_dirty = true;
                self.etat()
            }
            "classes" => {
                let Some(s) = &self.scene else { return erreur("aucun fichier ouvert") };
                let mut n: BTreeMap<&str, usize> = BTreeMap::new();
                for o in &s.objects {
                    *n.entry(&*o.class).or_default() += 1;
                }
                let classes = n.into_iter().map(|(c, k)| objet(&[("classe", texte(c)), ("objets", k.to_string()), ("visible", (!self.hidden_classes.contains(c)).to_string())]));
                succes(&[("classes", liste(classes))])
            }
            "elements" => {
                // elements [CLASSE|*] [texte du nom]
                let Some(s) = &self.scene else { return erreur("aucun fichier ouvert") };
                let (classe, texte_nom) = argument.split_once(' ').unwrap_or((argument, ""));
                let (classe, texte_nom) = (classe.to_uppercase(), texte_nom.to_lowercase());
                let mut out = Vec::new();
                for o in &s.objects {
                    if (!classe.is_empty() && classe != "*" && *o.class != *classe) || (!texte_nom.is_empty() && !s.model.name(o.id).to_lowercase().contains(&texte_nom)) {
                        continue;
                    }
                    out.push(self.decrire(o.id));
                    if out.len() == 300 {
                        break;
                    }
                }
                succes(&[("elements", liste(out))])
            }
            "selectionner" => match self.entite(argument) {
                Ok(id) => {
                    self.select(Some(id), false);
                    self.etat()
                }
                Err(e) => erreur(&e),
            },
            "isoler" | "cadrer" => match self.entite(argument) {
                Ok(id) => {
                    let o = self.objects_under(id);
                    if o.is_empty() {
                        return erreur("pas de géométrie");
                    }
                    if commande == "isoler" {
                        self.isolate(o.clone());
                    }
                    self.frame_objects(&o);
                    self.etat()
                }
                Err(e) => erreur(&e),
            },
            "proprietes" => match self.entite(argument) {
                Ok(id) => {
                    let m = &self.scene.as_ref().unwrap().model;
                    let paires = |v: Vec<(String, String)>| objet(&v.iter().map(|(k, v)| (k.as_str(), texte(v))).collect::<Vec<_>>());
                    let groupes: Vec<String> = m.property_groups(id).into_iter().map(|g| objet(&[("nom", texte(&g.name)), ("proprietes", paires(g.props))])).collect();
                    succes(&[("entite", self.decrire(id)), ("attributs", paires(m.attributes(id))), ("groupes", liste(groupes))])
                }
                Err(e) => erreur(&e),
            },
            "objet" => {
                // objet x y : position dans la vue, de 0 à 1 depuis le coin haut gauche
                let v: Vec<f32> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let (&[x, y], Some(s)) = (&v[..], &self.scene) else { return erreur("usage : objet <x 0..1> <y 0..1>") };
                let aspect = self.rect_vue.width() / self.rect_vue.height().max(1.0);
                let (o, d) = self.camera.ray(x * 2.0 - 1.0, 1.0 - y * 2.0, aspect);
                match s.pick(o, d, &self.visible, self.plan_coupe()) {
                    Some((obj, t)) => {
                        let p = o + d * t;
                        succes(&[("entite", self.decrire(s.objects[obj as usize].id)), ("point", format!("[{:.3},{:.3},{:.3}]", p.x, p.y, p.z))])
                    }
                    None => succes(&[("entite", "null".into())]),
                }
            }
            _ => erreur(&format!("commande inconnue : {commande}")),
        }
    }
}

/// Matériaux d'un modèle IFC (`IfcMaterial`) et éléments qui les portent (association
/// directe ou par le type, couches, profils, constituants).
fn materiaux_ifc(s: &Scene) -> Vec<scene::NavMateriau> {
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
fn niveaux(s: &Scene) -> Vec<(Id, String, f32)> {
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
fn is_revit_file(path: &std::path::Path) -> bool {
    if rvt::is_revit_path(path) {
        return true;
    }
    let mut head = [0u8; 8];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && rvt::is_cfb(&head)
}

fn decode_png(data: &[u8]) -> Option<egui::ColorImage> {
    let (w, h, rgba) = rvt::decode_png_rgba(data)?;
    Some(egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

fn load_state_fr(s: &str) -> &str {
    match s {
        "Loaded" => "Chargé",
        "Not Found" => "Introuvable",
        "Unloaded" => "Déchargé",
        "NotLoaded" | "Not Loaded" => "Non chargé",
        _ => s,
    }
}

impl eframe::App for ViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading();
        self.piloter(&ctx);

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.open(p, &ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(&ctx);
        }

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.rvt.is_some() && self.rvt_sheet {
            egui::CentralPanel::default().show(ui, |ui| self.revit_view(ui));
        } else {
            // disposition de Revit : propriétés à gauche, arborescence du projet à droite
            egui::Panel::left("proprietes").resizable(true).default_size(300.0).show(ui, |ui| self.right_panel(ui));
            egui::Panel::right("arborescence").resizable(true).default_size(320.0).show(ui, |ui| self.left_panel(ui));
            egui::CentralPanel::no_frame().show(ui, |ui| self.viewport(ui));
        }
        self.unsupported_window(&ctx);
    }

    /// Appelé même fenêtre réduite : relève les commandes du pilote et demande alors le
    /// tracé complet, que `ui` les traite et qu'une capture ait des pixels.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.pilote {
            self.requetes.extend(rx.try_iter());
        }
        if !self.requetes.is_empty() || !self.attente_ouverture.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let (Some(gl), Some(r)) = (gl, self.renderer.lock().unwrap().as_ref()) {
            r.destroy(gl);
        }
    }
}
