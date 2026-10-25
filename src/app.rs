//! Interface egui, disposée comme Revit : propriétés à gauche, vue au centre,
//! arborescence du projet à droite (vues, nomenclatures, feuilles, familles, groupes et
//! liens d'un fichier Revit ; arborescence spatiale et classes d'un IFC). Les deux
//! panneaux s'ancrent d'un côté ou flottent ; sous la vue, la boîte de visibilité par
//! catégorie de modèle et d'annotation.

use crate::camera::{Camera, MAX_PITCH};
use crate::dessin::{self, Poche};
use crate::model::pretty_type;
use crate::pilote::{self, Requete};
use crate::render::{FrameParams, ParamsPasse, Renderer, StyleVisuel};
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

/// Panneaux déplaçables, comme les palettes de Revit.
#[derive(PartialEq, Clone, Copy, Debug)]
enum Panneau {
    Proprietes,
    Arborescence,
}

impl Panneau {
    const TOUS: [Panneau; 2] = [Panneau::Proprietes, Panneau::Arborescence];

    /// Nom pour le pilote (commande `panneau`).
    fn nom(self) -> &'static str {
        match self {
            Panneau::Proprietes => "proprietes",
            Panneau::Arborescence => "arborescence",
        }
    }
}

/// Place d'un panneau : ancré d'un côté de la vue, ou flottant au-dessus.
#[derive(PartialEq, Clone, Copy, Debug)]
enum Ancrage {
    Gauche,
    Droite,
    Flottant,
}

impl Ancrage {
    const TOUS: [Ancrage; 3] = [Ancrage::Gauche, Ancrage::Droite, Ancrage::Flottant];

    fn libelle(self) -> &'static str {
        match self {
            Ancrage::Gauche => "Ancrer à gauche",
            Ancrage::Droite => "Ancrer à droite",
            Ancrage::Flottant => "Flottant",
        }
    }

    /// Nom pour le pilote (commande `panneau`).
    fn nom(self) -> &'static str {
        match self {
            Ancrage::Gauche => "gauche",
            Ancrage::Droite => "droite",
            Ancrage::Flottant => "flottant",
        }
    }
}

/// Catégorie de la boîte de visibilité, comme dans Revit : catégorie de modèle (éléments
/// à géométrie : murs, fenêtres…) ou d'annotation (éléments 2D : cotes, textes, traits…).
struct Categorie {
    annotation: bool,
    nom: String,
    /// Clés de masquage de ses objets (dans `hidden_classes`) : leur classe IFC, ou
    /// « classe:nom » quand la classe regroupe plusieurs catégories (genres des
    /// annotations, catégories Revit des proxys).
    cles: Vec<Arc<str>>,
}

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

/// Onglet du ruban en haut de la fenêtre.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OngletRuban {
    Fichier,
    Exporter,
}

/// Niveau de détail d'une vue, comme dans Revit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NiveauDetail {
    Faible,
    Moyen,
    Eleve,
}

impl NiveauDetail {
    const TOUS: [NiveauDetail; 3] = [NiveauDetail::Faible, NiveauDetail::Moyen, NiveauDetail::Eleve];

    fn libelle(self) -> &'static str {
        match self {
            NiveauDetail::Faible => "Faible",
            NiveauDetail::Moyen => "Moyen",
            NiveauDetail::Eleve => "Élevé",
        }
    }

    /// Valeur du paramètre VIEW_DETAIL_LEVEL (`ViewDetailLevel` de l'API : 1, 2, 3).
    fn depuis_revit(v: i64) -> Option<Self> {
        match v {
            1 => Some(NiveauDetail::Faible),
            2 => Some(NiveauDetail::Moyen),
            3 => Some(NiveauDetail::Eleve),
            _ => None,
        }
    }
}

/// Navigation en vol façon Unity : clic droit maintenu pour regarder autour de soi,
/// clavier pour se déplacer. Les touches sont lues par leur position physique (W A S D Q E
/// d'un clavier QWERTY, Z Q S D A E d'un AZERTY).
struct Vol {
    actif: bool,
    touches: HashSet<egui::Key>,
    /// Multiplicateur de vitesse, réglé à la molette pendant le vol.
    vitesse: f32,
}

impl Default for Vol {
    fn default() -> Self {
        Vol { actif: false, touches: HashSet::new(), vitesse: 1.0 }
    }
}

impl Vol {
    /// Direction demandée (avant, droite, haut) d'après les touches enfoncées.
    fn direction(&self) -> Vec3 {
        use egui::Key::*;
        let axe = |plus, moins| self.touches.contains(&plus) as i32 as f32 - self.touches.contains(&moins) as i32 as f32;
        Vec3::new(axe(W, S), axe(D, A), axe(E, Q))
    }
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
    /// Nomenclature affichée (indice dans `Navigateur::vues`).
    nomenclature_ouverte: Option<usize>,
    /// Dernier point cliqué sur un objet, repère du fichier (m).
    point_clique: Option<glam::DVec3>,
    /// Navigation en vol façon Unity (vue 3D, clic droit maintenu).
    vol: Vol,
    /// Commandes du serveur MCP (voir `pilote.rs`) et celles en attente de traitement.
    pilote: Option<Receiver<Requete>>,
    requetes: Vec<Requete>,
    /// Réponses des ouvertures demandées par le pilote, envoyées en fin de chargement.
    attente_ouverture: Vec<Sender<String>>,
    /// Zone de la vue 3D dans la fenêtre (points), pour les captures.
    rect_vue: egui::Rect,
    /// Passes des fenêtres de vue de la feuille ouverte, et leurs objets (catégories
    /// masquées comprises).
    passes_feuille: Vec<PasseFeuille>,
    objets_feuille: Vec<u32>,
    /// Mode dessin : plans et élévations comme dans un logiciel de DAO.
    /// Style visuel de la vue (Lignes cachées : dessin des plans et élévations).
    style: StyleVisuel,
    /// Coupe horizontale du mode dessin : active, niveau de référence (sinon bas du
    /// modèle) et hauteur au-dessus de lui (m).
    coupe: bool,
    coupe_niveau: Option<Id>,
    coupe_hauteur: f32,
    poche: Poche,
    /// Dénominateur de l'échelle du dessin (épaisseur des contours, pas des hachures).
    echelle: f64,
    /// Niveau de détail de la vue (barre de la vue), lu dans la vue Revit ouverte.
    detail: NiveauDetail,
    onglet_ruban: OngletRuban,
    /// Zone de coupe 3D (min, max), repère de la scène ; appliquée aux vues 3D quand
    /// elle est active, modifiable par ses poignées dans toutes les vues.
    boite: Option<(Vec3, Vec3)>,
    boite_active: bool,
    /// Tracé de la coupe à recalculer.
    coupe_dirty: bool,
    /// Niveaux de la scène (voir `niveaux`), relevés à son installation.
    liste_niveaux: Vec<(Id, String, f32)>,
    /// Matériaux de la scène et éléments qui les portent (onglet Matériaux).
    liste_materiaux: Vec<scene::NavMateriau>,
    /// Catégories de visibilité de la scène, et pour chaque objet sa catégorie et sa clé
    /// de masquage (voir [`Categorie`]).
    categories: Vec<Categorie>,
    categorie_objet: Vec<usize>,
    cle_objet: Vec<Arc<str>>,
    /// Boîte de visibilité ouverte.
    voir_visibilite: bool,
    /// Place des panneaux (indice : `Panneau`), coin haut gauche des panneaux flottants
    /// et position imposée à la prochaine image (panneau qu'on glisse par son titre).
    ancrages: [Ancrage; 2],
    pos_flottants: [egui::Pos2; 2],
    pos_imposee: [Option<egui::Pos2>; 2],
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
            point_clique: None,
            vol: Vol::default(),
            nomenclature_ouverte: None,
            pilote: pilote::demarrer(cc.egui_ctx.clone()),
            requetes: Vec::new(),
            attente_ouverture: Vec::new(),
            rect_vue: egui::Rect::NOTHING,
            passes_feuille: Vec::new(),
            objets_feuille: Vec::new(),
            style: StyleVisuel::Realiste,
            coupe: false,
            coupe_niveau: None,
            coupe_hauteur: 1.2,
            poche: Poche::Hachure,
            echelle: 100.0,
            detail: NiveauDetail::Moyen,
            onglet_ruban: OngletRuban::Fichier,
            boite: None,
            boite_active: false,
            coupe_dirty: true,
            liste_niveaux: Vec::new(),
            liste_materiaux: Vec::new(),
            categories: Vec::new(),
            categorie_objet: Vec::new(),
            cle_objet: Vec::new(),
            voir_visibilite: false,
            ancrages: [Ancrage::Gauche, Ancrage::Droite],
            pos_flottants: [egui::pos2(60.0, 80.0), egui::pos2(400.0, 80.0)],
            pos_imposee: [None, None],
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
            r.upload_vertices(gl, &scene.vertices, &scene.couleurs_rendu);
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
        self.style = StyleVisuel::Realiste;
        self.coupe_niveau = None;
        self.liste_niveaux = niveaux(&scene);
        self.left_tab = if scene.navigateur.is_some() { LeftTab::Tout } else { LeftTab::Tree };
        self.liste_materiaux = match &scene.navigateur {
            Some(n) => n.materiaux.clone(),
            None => materiaux_ifc(&scene),
        };
        (self.categories, self.categorie_objet, self.cle_objet) = categories(&scene);
        self.boite = None;
        self.boite_active = false;
        self.scene = Some(scene);
        let secondaires = self.masques_variantes();
        self.hidden_objects.extend(secondaires);
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
                && !self.hidden_classes.contains(&*self.cle_objet[i])
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
        if self.style != StyleVisuel::LignesCachees || !self.coupe {
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

    /// Zone de coupe appliquée au rendu et à la sélection : active, vue 3D.
    fn boite_appliquee(&self) -> Option<(Vec3, Vec3)> {
        if self.boite_active && self.vue_3d() { self.boite } else { None }
    }

    /// Active la zone de coupe ; la première fois, elle entoure tout le modèle.
    fn activer_boite(&mut self, active: bool) {
        self.boite_active = active;
        if active
            && self.boite.is_none()
            && let Some(s) = &self.scene
        {
            let marge = ((s.max - s.min).length() * 0.02).max(0.1);
            self.boite = Some((s.min - Vec3::splat(marge), s.max + Vec3::splat(marge)));
        }
    }

    /// Zone de coupe dans les propriétés : bornes en coordonnées du fichier (m).
    fn proprietes_boite(&mut self, ui: &mut egui::Ui) {
        let (Some((mut a, mut b)), Some(s)) = (self.boite, &self.scene) else { return };
        // coordonnées du fichier en f64 : origine éventuellement lointaine (géoréférencement)
        let o = s.origin;
        egui::CollapsingHeader::new(RichText::new("Zone de coupe").strong()).id_salt("zone_de_coupe").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_zone_de_coupe").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("");
                ui.label("Min");
                ui.label("Max");
                ui.end_row();
                for (k, nom) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*nom);
                    let (mut lo, mut hi) = (a[k] as f64 + o[k], b[k] as f64 + o[k]);
                    ui.add(egui::DragValue::new(&mut lo).speed(0.05).max_decimals(3).suffix(" m"));
                    ui.add(egui::DragValue::new(&mut hi).speed(0.05).max_decimals(3).suffix(" m"));
                    a[k] = (lo - o[k]) as f32;
                    b[k] = (hi - o[k]) as f32;
                    ui.end_row();
                }
            });
            if ui.button("Ajuster au modèle").clicked() {
                self.boite = None;
                self.activer_boite(true);
                (a, b) = self.boite.unwrap_or((a, b));
            }
        });
        self.regler_boite(a, b);
    }

    /// Nouvelles bornes de la zone de coupe, épaisseur minimale gardée.
    fn regler_boite(&mut self, a: Vec3, b: Vec3) {
        let ep = 0.05;
        let b = b.max(a + Vec3::splat(ep));
        self.boite = Some((a, b));
    }

    /// Contour de la zone de coupe sur la vue et ses poignées : une au centre de chaque
    /// face vue par la tranche ou de biais (en plan : les quatre côtés ; en élévation :
    /// dessus, dessous et côtés). Glisser une poignée déplace sa face le long de sa
    /// normale.
    fn poignees_boite(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let Some((a, b)) = self.boite else { return };
        if !self.boite_active || self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)).is_some_and(|v| v.genre == "Feuille") {
            return;
        }
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |p: Vec3| -> Option<egui::Pos2> {
            let c = vp * p.extend(1.0);
            (c.w > 1e-4).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let coin = |i: usize| Vec3::new(if i & 1 == 0 { a.x } else { b.x }, if i & 2 == 0 { a.y } else { b.y }, if i & 4 == 0 { a.z } else { b.z });
        let couleur = Color32::from_rgb(230, 120, 20);
        let painter = ui.painter_at(rect);
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0
                    && let (Some(p), Some(q)) = (ecran(coin(i)), ecran(coin(i | bit)))
                {
                    painter.line_segment([p, q], egui::Stroke::new(1.5, couleur));
                }
            }
        }
        let f = self.camera.forward();
        let centre = (a + b) * 0.5;
        let (mut na, mut nb) = (a, b);
        for k in 0..3 {
            for haut in [false, true] {
                let mut n = Vec3::ZERO;
                n[k] = if haut { 1.0 } else { -1.0 };
                if n.dot(f).abs() > 0.9 {
                    continue; // face vue de face : pas de sens à la glisser à l'écran
                }
                let mut c = centre;
                c[k] = if haut { b[k] } else { a[k] };
                let (Some(p), Some(q)) = (ecran(c), ecran(c + n)) else { continue };
                if !rect.contains(p) {
                    continue;
                }
                let r = egui::Rect::from_center_size(p, egui::vec2(14.0, 14.0));
                let resp = ui.interact(r, ui.make_persistent_id(("poignee_boite", k, haut)), Sense::drag());
                let actif = resp.hovered() || resp.dragged();
                painter.circle(p, if actif { 7.0 } else { 5.5 }, if actif { couleur } else { Color32::WHITE }, egui::Stroke::new(1.5, couleur));
                if resp.dragged() {
                    // pixels par mètre le long de la normale, à l'écran
                    let s = q - p;
                    let d = resp.drag_delta().dot(s) / s.length_sq().max(1e-6);
                    if haut {
                        nb[k] += d;
                    } else {
                        na[k] += d;
                    }
                }
                if actif {
                    resp.on_hover_cursor(egui::CursorIcon::Grab);
                }
            }
        }
        if (na, nb) != (a, b) {
            self.regler_boite(na.min(nb - Vec3::splat(0.05)), nb);
        }
    }

    /// Plage de vue du dessin (style Lignes cachées), en tête des propriétés comme dans
    /// Revit : coupe horizontale, niveau, hauteur du plan de coupe, poché.
    fn plage_de_vue(&mut self, ui: &mut egui::Ui) {
        let niveaux = self.niveaux();
        let nom = |n: Option<Id>| n.and_then(|n| niveaux.iter().find(|x| x.0 == n)).map_or("Bas du modèle".to_string(), |x| x.1.clone());
        let mut change = false;
        egui::CollapsingHeader::new(RichText::new("Plage de vue").strong()).id_salt("plage_de_vue").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_plage_de_vue").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Plan de coupe");
                change |= ui.checkbox(&mut self.coupe, "").on_hover_text("Parties coupées en poché et trait fort, parties au-dessus retirées").changed();
                ui.end_row();
                ui.add_enabled_ui(self.coupe, |ui| ui.label("Niveau"));
                ui.add_enabled_ui(self.coupe, |ui| {
                    egui::ComboBox::from_id_salt("coupe_niveau").selected_text(nom(self.coupe_niveau)).show_ui(ui, |ui| {
                        change |= ui.selectable_value(&mut self.coupe_niveau, None, "Bas du modèle").changed();
                        for (id, n, z) in &niveaux {
                            change |= ui.selectable_value(&mut self.coupe_niveau, Some(*id), format!("{n} ({z:+.2})")).changed();
                        }
                    });
                });
                ui.end_row();
                ui.add_enabled_ui(self.coupe, |ui| ui.label("Décalage du plan de coupe"));
                ui.add_enabled_ui(self.coupe, |ui| {
                    change |= ui.add(egui::DragValue::new(&mut self.coupe_hauteur).speed(0.01).range(-50.0..=200.0).suffix(" m")).on_hover_text("Hauteur du plan de coupe au-dessus du niveau").changed();
                });
                ui.end_row();
                ui.add_enabled_ui(self.coupe, |ui| ui.label("Poché"));
                ui.add_enabled_ui(self.coupe, |ui| {
                    egui::ComboBox::from_id_salt("poche").selected_text(self.poche.libelle()).show_ui(ui, |ui| {
                        for p in [Poche::Hachure, Poche::Plein, Poche::Vide] {
                            change |= ui.selectable_value(&mut self.poche, p, p.libelle()).changed();
                        }
                    });
                });
                ui.end_row();
            });
        });
        if change {
            self.visibility_dirty = true;
        }
    }

    /// Export SVG du dessin de la vue, après choix du fichier.
    fn bouton_svg(&mut self, ui: &mut egui::Ui) {
        if ui.button("Dessin SVG…").on_hover_text("Exporter le dessin de la vue en SVG (vue orthographique)").clicked() {
            let nom = self.scene.as_ref().map(|s| s.file_name.rsplit_once('.').map_or(s.file_name.clone(), |x| x.0.to_string())).unwrap_or_default();
            if let Some(chemin) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name(format!("{nom}.svg")).save_file() {
                match self.exporter_svg(&chemin) {
                    Ok(n) => self.status = format!("{} : {n} primitives exportées", chemin.display()),
                    Err(e) => self.error = Some(e),
                }
            }
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

    /// Ruban en haut : onglets (Fichier, Exporter), puis les commandes de l'onglet ouvert.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            for (o, nom) in [(OngletRuban::Fichier, "Fichier"), (OngletRuban::Exporter, "Exporter")] {
                ui.selectable_value(&mut self.onglet_ruban, o, RichText::new(nom).strong());
            }
            if self.loading.is_some() {
                ui.separator();
                ui.spinner();
            }
        });
        ui.separator();
        ui.horizontal(|ui| match self.onglet_ruban {
            OngletRuban::Fichier => {
                if ui.button("Ouvrir…").on_hover_text("Ctrl+O").clicked() {
                    self.open_dialog(&ctx);
                }
                if self.rvt.is_some() && self.rvt_geom.is_ok() {
                    ui.toggle_value(&mut self.rvt_sheet, "Fiche Revit").on_hover_text("Métadonnées du fichier Revit");
                }
            }
            OngletRuban::Exporter => {
                ui.add_enabled_ui(self.scene.is_some(), |ui| self.bouton_svg(ui));
            }
        });
    }

    /// Vue 3D : modèle libre (IFC, ou fichier Revit sans vue ouverte) ou vue 3D Revit ;
    /// pas un plan, une coupe, une élévation ni une feuille.
    fn vue_3d(&self) -> bool {
        match self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            Some(v) => v.genre == "Vue 3D",
            None => true,
        }
    }

    /// Glisser dans la vue : orbite au bouton gauche et regard sur place au bouton droit
    /// (vol) en vue 3D ; panoramique au bouton du milieu, à Maj + gauche, et aux boutons
    /// gauche et droit dans une vue 2D (plan, coupe, élévation, feuille), dont
    /// l'orientation est celle de la vue Revit.
    fn glisser(&mut self, bouton: PointerButton, d: egui::Vec2, shift: bool, hauteur: f32) {
        if bouton == PointerButton::Primary && !shift && self.vue_3d() {
            self.camera.orbit(d.x, d.y);
        } else if bouton == PointerButton::Secondary && self.vue_3d() {
            self.camera.ortho = false;
            self.camera.regarder(d.x, d.y);
        } else {
            self.camera.pan(d.x, d.y, hauteur);
        }
    }

    /// Navigation en vol (vue 3D) : tant que le bouton droit est enfoncé sur la vue, la
    /// souris oriente le regard, W/S (Z/S en AZERTY) avancent et reculent, A/D (Q/D) vont à
    /// gauche et à droite, Q/E (A/E) descendent et montent ; Maj triple la vitesse, la
    /// molette la règle. Comme dans la vue Scène d'Unity.
    fn voler(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        let enfonce = self.vue_3d() && resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_down(PointerButton::Secondary));
        if !enfonce {
            self.vol.actif = false;
            self.vol.touches.clear();
            return;
        }
        if !self.vol.actif {
            self.vol.actif = true;
            self.camera.ortho = false;
            self.camera.rapprocher_cible((self.camera.scene_radius * 0.25).max(1.0));
        }
        let (delta, molette, maj, dt) = ui.input(|i| {
            for e in &i.events {
                if let egui::Event::Key { key, physical_key, pressed, .. } = e {
                    let k = physical_key.unwrap_or(*key);
                    if *pressed {
                        self.vol.touches.insert(k);
                    } else {
                        self.vol.touches.remove(&k);
                    }
                }
            }
            (i.pointer.delta(), i.smooth_scroll_delta.y, i.modifiers.shift, i.stable_dt.min(0.1))
        });
        if delta != egui::Vec2::ZERO {
            self.camera.regarder(delta.x, delta.y);
        }
        if molette != 0.0 {
            self.vol.vitesse = (self.vol.vitesse * (molette * 0.004).exp()).clamp(0.02, 50.0);
        }
        let dir = self.vol.direction();
        if dir != Vec3::ZERO {
            let f = self.camera.forward();
            let droite = f.cross(Vec3::Z).normalize_or(Vec3::X);
            let v = (self.camera.scene_radius * 0.3).max(1.0) * self.vol.vitesse * if maj { 3.0 } else { 1.0 };
            self.camera.deplacer((f * dir.x + droite * dir.y + Vec3::Z * dir.z) * v * dt);
            ui.ctx().request_repaint();
        }
    }

    /// Barre de la vue, sous la vue comme celle de Revit : échelle (celle de la vue
    /// ouverte, qui règle aussi le dessin 2D), cadrage, projection en vue 3D.
    fn barre_vue(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.scene.is_some(), |ui| {
                let r = ui.add(egui::DragValue::new(&mut self.echelle).speed(1.0).range(1.0..=10000.0).prefix("1 : "));
                if r.on_hover_text("Échelle de la vue : épaisseur des traits coupés et pas des hachures du dessin 2D, export SVG").changed() {
                    self.visibility_dirty = true;
                }
                ui.separator();
                self.choix_detail(ui);
                ui.separator();
                self.choix_style(ui);
                ui.separator();
                if ui.button("Cadrer").on_hover_text("Tout le modèle visible dans la vue (F)").clicked() {
                    self.fit_all();
                }
                ui.separator();
                ui.toggle_value(&mut self.voir_visibilite, "Visibilité").on_hover_text("Catégories de modèle et d'annotation affichées dans la vue (V)");
                if ui.button("Tout afficher").on_hover_text("Montrer les éléments masqués et isolés").clicked() {
                    self.hidden_objects.clear();
                    self.hidden_storeys.clear();
                    self.isolated = None;
                    self.visibility_dirty = true;
                }
                ui.separator();
                let mut zone = self.boite_active;
                if ui.toggle_value(&mut zone, "Zone de coupe").on_hover_text("Boîte qui découpe le modèle dans les vues 3D ; ses faces se règlent aux poignées, en 3D, en plan ou en élévation, ou dans les propriétés").changed() {
                    self.activer_boite(zone);
                }
                if self.vue_3d() {
                    ui.toggle_value(&mut self.camera.ortho, "Ortho").on_hover_text("Projection orthographique (sinon perspective)");
                }
            });
        });
    }

    /// Niveau de détail de la vue (Faible, Moyen, Élevé), celui de la vue Revit ouverte.
    /// La visibilité des géométries des familles selon le niveau de détail n'est pas
    /// encore lue : le choix n'a pas d'effet sur l'affichage.
    fn choix_detail(&mut self, ui: &mut egui::Ui) {
        let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
        icone_detail(ui.painter(), r.shrink(3.0), self.detail);
        ui.menu_button(self.detail.libelle(), |ui| {
            for d in NiveauDetail::TOUS {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(120.0, 24.0), Sense::click());
                let v = ui.visuals();
                if d == self.detail {
                    ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
                }
                icone_detail(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(5.0, 4.0), egui::vec2(16.0, 16.0)), d);
                ui.painter().text(r.min + egui::vec2(30.0, 12.0), egui::Align2::LEFT_CENTER, d.libelle(), egui::TextStyle::Button.resolve(ui.style()), v.text_color());
                if resp.clicked() {
                    self.detail = d;
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Niveau de détail de la vue (lu dans la vue Revit). Sans effet sur l'affichage pour l'instant : la visibilité des géométries de familles par niveau de détail n'est pas encore lue");
    }

    /// Catégorie montrée : une de ses clés au moins n'est pas masquée, ni sa classe.
    fn categorie_visible(&self, c: &Categorie) -> bool {
        c.cles.iter().any(|cle| !self.hidden_classes.contains(&**cle) && !self.hidden_classes.contains(classe_cle(cle)))
    }

    /// Affiche ou masque la catégorie `k` de la boîte de visibilité.
    fn montrer_categorie(&mut self, k: usize, montrer: bool) {
        for cle in self.categories[k].cles.clone() {
            let classe = classe_cle(&cle);
            if !montrer {
                self.hidden_classes.insert(cle.to_string());
                continue;
            }
            // classe masquée en bloc (annotations à l'ouverture) : on la montre en
            // masquant ses autres catégories
            if &*cle != classe && self.hidden_classes.remove(classe) {
                for c in self.categories.iter().flat_map(|c| &c.cles).filter(|c| **c != cle && classe_cle(c) == classe) {
                    self.hidden_classes.insert(c.to_string());
                }
            }
            self.hidden_classes.remove(&*cle);
        }
        self.visibility_dirty = true;
        self.preparer_feuille(self.vue_active);
    }

    /// Boîte de visibilité, au bas de la vue : catégories de modèle et d'annotation des
    /// éléments de la vue ouverte (de toute la scène hors vue Revit), à cocher une à une
    /// ou par groupe.
    fn fenetre_visibilite(&mut self, ctx: &egui::Context) {
        if !self.voir_visibilite || self.scene.is_none() {
            return;
        }
        let mut nombres = vec![0usize; self.categories.len()];
        for (i, &k) in self.categorie_objet.iter().enumerate() {
            if self.isolated.as_ref().is_none_or(|iso| iso.contains(&(i as u32))) {
                nombres[k] += 1;
            }
        }
        // objets des vues placées d'une feuille
        for &i in &self.objets_feuille {
            nombres[self.categorie_objet[i as usize]] += 1;
        }
        let mut ouverte = true;
        let mut bascules = Vec::new();
        egui::Window::new("Visibilité")
            .open(&mut ouverte)
            .collapsible(false)
            .resizable(true)
            .default_size([480.0, 300.0])
            .pivot(egui::Align2::LEFT_BOTTOM)
            .default_pos(self.rect_vue.left_bottom() + egui::vec2(10.0, -10.0))
            .constrain_to(self.rect_vue)
            .show(ctx, |ui| {
                ui.columns(2, |colonnes| {
                    for (ui, annotation) in colonnes.iter_mut().zip([false, true]) {
                        let titre = if annotation { "Catégories d'annotation (2D)" } else { "Catégories de modèle" };
                        let ks: Vec<usize> = (0..self.categories.len()).filter(|&k| self.categories[k].annotation == annotation && nombres[k] > 0).collect();
                        let vues = ks.iter().filter(|&&k| self.categorie_visible(&self.categories[k])).count();
                        let mut toutes = !ks.is_empty() && vues == ks.len();
                        let groupe = egui::Checkbox::new(&mut toutes, RichText::new(titre).strong()).indeterminate(vues > 0 && vues < ks.len());
                        if ui.add_enabled(!ks.is_empty(), groupe).changed() {
                            bascules.extend(ks.iter().map(|&k| (k, toutes)));
                        }
                        ui.separator();
                        egui::ScrollArea::vertical().id_salt(("visibilite", annotation)).auto_shrink([false, false]).show(ui, |ui| {
                            if ks.is_empty() {
                                ui.label(RichText::new("Aucune dans la vue").weak());
                            }
                            for &k in &ks {
                                let mut vue = self.categorie_visible(&self.categories[k]);
                                if ui.checkbox(&mut vue, format!("{} ({})", self.categories[k].nom, nombres[k])).changed() {
                                    bascules.push((k, vue));
                                }
                            }
                        });
                    }
                });
            });
        self.voir_visibilite = ouverte;
        for (k, montrer) in bascules {
            self.montrer_categorie(k, montrer);
        }
    }

    /// Contenu d'un panneau déplaçable.
    fn panneau(&mut self, ui: &mut egui::Ui, p: Panneau) {
        match p {
            Panneau::Proprietes => self.right_panel(ui),
            Panneau::Arborescence => self.left_panel(ui),
        }
    }

    /// Titre d'un panneau, par lequel on le déplace : glissé hors de son ancrage il
    /// devient flottant ; relâché contre le bord gauche ou droit de la fenêtre, il s'y
    /// ancre. Menu d'ancrage à droite du titre.
    fn entete_panneau(&mut self, ui: &mut egui::Ui, p: Panneau, titre: &str) {
        let i = p as usize;
        let mut ancrage = None;
        // de droite à gauche : menu, puis le titre sur la largeur restante (sans élargir
        // le panneau)
        ui.horizontal(|ui| ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.menu_button("…", |ui| {
                for a in Ancrage::TOUS {
                    if ui.selectable_label(self.ancrages[i] == a, a.libelle()).clicked() {
                        ancrage = Some(a);
                        ui.close();
                    }
                }
            });
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::hover());
            let resp = ui.interact(rect, egui::Id::unique(("entete_panneau", p.nom())), Sense::click_and_drag());
            let encre = ui.visuals().strong_text_color();
            ui.painter_at(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, titre, egui::FontId::proportional(15.0), encre);
            if resp.drag_started() && self.ancrages[i] != Ancrage::Flottant {
                ancrage = Some(Ancrage::Flottant);
                self.pos_imposee[i] = resp.interact_pointer_pos().map(|q| q - egui::vec2(60.0, 10.0));
            } else if resp.dragged() && self.ancrages[i] == Ancrage::Flottant {
                self.pos_imposee[i] = Some(self.pos_flottants[i] + resp.drag_delta());
            }
            if resp.drag_stopped() && self.ancrages[i] == Ancrage::Flottant {
                let ecran = ui.ctx().content_rect();
                match ui.ctx().pointer_latest_pos() {
                    Some(q) if q.x < ecran.left() + 40.0 => ancrage = Some(Ancrage::Gauche),
                    Some(q) if q.x > ecran.right() - 40.0 => ancrage = Some(Ancrage::Droite),
                    _ => {}
                }
            }
            let survol = if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab };
            resp.on_hover_cursor(survol).on_hover_text("Glisser pour déplacer ; relâcher contre un bord de la fenêtre pour ancrer");
        }));
        if let Some(a) = ancrage {
            self.ancrer(p, a);
        }
    }

    /// Ancre un panneau, ou le rend flottant au-dessus de la vue (à sa dernière place
    /// flottante, ramenée dans la vue).
    fn ancrer(&mut self, p: Panneau, a: Ancrage) {
        let i = p as usize;
        if a == Ancrage::Flottant && self.ancrages[i] != a && self.pos_imposee[i].is_none() && self.rect_vue.is_positive() {
            let v = self.rect_vue;
            let q = self.pos_flottants[i];
            self.pos_imposee[i] = Some(egui::pos2(q.x.clamp(v.left(), (v.right() - 200.0).max(v.left())), q.y.clamp(v.top(), (v.bottom() - 200.0).max(v.top()))));
        }
        self.ancrages[i] = a;
    }

    /// Soleil du style Rendu : direction (depuis le sud-ouest, 45° de hauteur environ),
    /// vue orthographique du soleil englobant la scène, décalage contre l'acné d'ombre
    /// (un texel et demi de la carte d'ombres).
    fn soleil(&self) -> Option<(Vec3, Mat4, f32)> {
        let s = self.scene.as_ref()?;
        let dir = Vec3::new(-0.45, -0.6, 0.66).normalize();
        let (c, r) = ((s.min + s.max) * 0.5, ((s.max - s.min).length() * 0.5).max(0.1) * 1.02);
        let vue = glam::camera::rh::view::look_at_mat4(c + dir * r * 2.0, c, Vec3::Z);
        let proj = glam::camera::rh::proj::opengl::orthographic(-r, r, -r, r, 0.0, r * 4.0);
        Some((dir, proj * vue, 1.5 * 2.0 * r / 2048.0))
    }

    /// Style visuel : bouton à l'icône du style en cours, menu des styles.
    fn choix_style(&mut self, ui: &mut egui::Ui) {
        let (r, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::hover());
        icone_style(ui.painter(), r.shrink(2.0), self.style);
        let mut choisi = None;
        ui.menu_button(self.style.libelle(), |ui| {
            for st in StyleVisuel::TOUS {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(190.0, 24.0), Sense::click());
                let v = ui.visuals();
                if st == self.style {
                    ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
                }
                icone_style(ui.painter(), egui::Rect::from_min_size(r.min + egui::vec2(4.0, 3.0), egui::vec2(18.0, 18.0)), st);
                ui.painter().text(r.min + egui::vec2(30.0, 12.0), egui::Align2::LEFT_CENTER, st.libelle(), egui::TextStyle::Button.resolve(ui.style()), v.text_color());
                if resp.clicked() {
                    choisi = Some(st);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Style visuel");
        if let Some(st) = choisi {
            self.style = st;
            self.visibility_dirty = true;
        }
    }

    /// Cube d'orientation (vue 3D), dans le coin haut droit de la vue : clic sur une face,
    /// une arête ou un coin pour regarder le modèle depuis cette direction ; glisser pour
    /// tourner autour. Boussole en anneau à sa base : clic sur N, E, S ou O pour regarder
    /// depuis ce point cardinal (inclinaison gardée) ; glisser l'anneau pour tourner autour
    /// de la verticale.
    fn cube_orientation(&mut self, ui: &mut egui::Ui, vue: egui::Rect) {
        use egui::{Stroke, pos2};
        let cote = 132.0;
        let zone = egui::Rect::from_min_size(pos2(vue.right() - cote - 8.0, vue.top() + 8.0), egui::vec2(cote, cote));
        let resp = ui.interact(zone, ui.make_persistent_id("cube_orientation"), Sense::click_and_drag());
        let rot = self.camera.view();
        let centre = zone.center();
        let k = cote * 0.2;
        let ecran = |p: Vec3| {
            let q = rot.transform_vector3(p);
            pos2(centre.x + q.x * k, centre.y - q.y * k)
        };
        // faces : normale, axes (u, v) de leur grille 3 × 3, nom
        let faces = [
            (Vec3::Z, Vec3::X, Vec3::Y, "HAUT"),
            (Vec3::NEG_Z, Vec3::X, Vec3::NEG_Y, "BAS"),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z, "AVANT"),
            (Vec3::Y, Vec3::NEG_X, Vec3::Z, "ARRIÈRE"),
            (Vec3::X, Vec3::Y, Vec3::Z, "DROITE"),
            (Vec3::NEG_X, Vec3::NEG_Y, Vec3::Z, "GAUCHE"),
        ];
        let cellule = |n: Vec3, u: Vec3, v: Vec3, i: usize, j: usize| {
            let (a0, a1) = ([-1.0, -0.6, 0.6][i], [-0.6, 0.6, 1.0][i]);
            let (b0, b1) = ([-1.0, -0.6, 0.6][j], [-0.6, 0.6, 1.0][j]);
            [n + u * a0 + v * b0, n + u * a1 + v * b0, n + u * a1 + v * b1, n + u * a0 + v * b1].map(ecran)
        };
        let dedans = |p: egui::Pos2, q: &[egui::Pos2; 4]| {
            let signe = |a: egui::Pos2, b: egui::Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let s: Vec<f32> = (0..4).map(|m| signe(q[m], q[(m + 1) % 4])).collect();
            s.iter().all(|x| *x >= 0.0) || s.iter().all(|x| *x <= 0.0)
        };
        // boussole : anneau dans le plan de la base du cube, quadrilatères (repère du
        // modèle) ; points cardinaux (nom, direction depuis le centre, libellé)
        const R0: f32 = 1.55;
        const R1: f32 = 1.95;
        const Z: f32 = -1.0;
        let n_seg = 64;
        let au = |a: f32, r: f32| Vec3::new(a.cos() * r, a.sin() * r, Z);
        let segments: Vec<[Vec3; 4]> = (0..n_seg)
            .map(|i| {
                let (a, b) = (i as f32 / n_seg as f32 * std::f32::consts::TAU, (i + 1) as f32 / n_seg as f32 * std::f32::consts::TAU);
                [au(a, R0), au(a, R1), au(b, R1), au(b, R0)]
            })
            .collect();
        let cardinaux = [("N", Vec3::Y, "le nord"), ("E", Vec3::X, "l'est"), ("S", Vec3::NEG_Y, "le sud"), ("O", Vec3::NEG_X, "l'ouest")];
        // lettre : centrée sur l'anneau, le haut vers l'extérieur
        let lettre = |d: Vec3| {
            let c = d * ((R0 + R1) / 2.0) + Vec3::Z * Z;
            (c, d.cross(Vec3::Z), d)
        };
        let lettre_quad = |d: Vec3| {
            let (c, u, v) = lettre(d);
            let h = 0.22;
            [c - u * h - v * h, c + u * h - v * h, c + u * h + v * h, c - u * h + v * h].map(ecran)
        };

        // ce que désigne un point : cellule du cube, sinon lettre, sinon anneau
        let visibles: Vec<_> = faces.iter().filter(|f| rot.transform_vector3(f.0).z > 1e-3).collect();
        enum Cible {
            Cellule(Vec3, Vec3, usize, usize),
            Lettre(usize),
            Anneau,
        }
        let cible = |p: egui::Pos2| -> Option<Cible> {
            let mut c = None;
            for f in &visibles {
                for i in 0..3 {
                    for j in 0..3 {
                        if dedans(p, &cellule(f.0, f.1, f.2, i, j)) {
                            c = Some(Cible::Cellule(f.0 + f.1 * (i as f32 - 1.0) + f.2 * (j as f32 - 1.0), f.0, i, j));
                        }
                    }
                }
            }
            c.or_else(|| cardinaux.iter().position(|x| dedans(p, &lettre_quad(x.1))).map(Cible::Lettre))
                .or_else(|| segments.iter().any(|q| dedans(p, &q.map(ecran))).then_some(Cible::Anneau))
        };
        let survol = resp.hover_pos().and_then(cible);
        // glisser : depuis l'anneau, autour de la verticale seulement
        if resp.dragged_by(PointerButton::Primary) {
            let d = resp.drag_delta();
            let depuis_anneau = ui.input(|i| i.pointer.press_origin()).and_then(cible).is_some_and(|c| matches!(c, Cible::Anneau | Cible::Lettre(_)));
            self.camera.orbit(d.x, if depuis_anneau { 0.0 } else { d.y });
        }

        let painter = ui.painter_at(zone);
        let encre = Color32::from_gray(70);
        let bleu = Color32::from_rgb(40, 110, 210);
        let anneau_survole = matches!(survol, Some(Cible::Anneau));
        let profondeur = |q: &[Vec3; 4]| rot.transform_vector3((q[0] + q[2]) / 2.0).z;
        let tracer_anneau = |devant: bool| {
            let fond = if anneau_survole { Color32::from_rgba_unmultiplied(170, 200, 240, 220) } else { Color32::from_rgba_unmultiplied(225, 225, 225, 210) };
            for q in segments.iter().filter(|q| (profondeur(q) > 0.0) == devant) {
                painter.add(egui::Shape::convex_polygon(q.map(ecran).to_vec(), fond, Stroke::NONE));
                // bords intérieur et extérieur
                painter.line_segment([ecran(q[0]), ecran(q[3])], Stroke::new(1.0, encre));
                painter.line_segment([ecran(q[1]), ecran(q[2])], Stroke::new(1.0, encre));
            }
            // lettres imprimées sur le dessus de l'anneau : illisibles (en miroir) vues
            // d'en dessous, écrasées vues par la tranche
            let dessus_visible = rot.transform_vector3(Vec3::Z).z > 0.08;
            for (i, (nom, d, _)) in cardinaux.iter().enumerate() {
                let (c, u, v) = lettre(*d);
                if !dessus_visible || (rot.transform_vector3(c).z > 0.0) != devant {
                    continue;
                }
                let couleur = if matches!(survol, Some(Cible::Lettre(j)) if j == i) { bleu } else if *nom == "N" { Color32::from_rgb(200, 40, 40) } else { encre };
                painter.add(texte_sur_face(nom, c, u, v, &ecran, couleur, 0.32));
            }
        };
        // anneau derrière le cube, cube, anneau devant
        tracer_anneau(false);
        for f in &visibles {
            let coins = [f.0 - f.1 - f.2, f.0 + f.1 - f.2, f.0 + f.1 + f.2, f.0 - f.1 + f.2].map(ecran);
            let face = rot.transform_vector3(f.0).z;
            let gris = (215.0 + 30.0 * face) as u8;
            painter.add(egui::Shape::convex_polygon(coins.to_vec(), Color32::from_rgba_unmultiplied(gris, gris, gris, 235), Stroke::new(1.0, encre)));
            if let Some(Cible::Cellule(_, n, i, j)) = survol.as_ref().filter(|c| matches!(c, Cible::Cellule(_, n, _, _) if *n == f.0)) {
                painter.add(egui::Shape::convex_polygon(cellule(*n, f.1, f.2, *i, *j).to_vec(), Color32::from_rgba_unmultiplied(80, 150, 230, 170), Stroke::NONE));
            }
            // nom imprimé sur la face : glyphes de la police (texte::ligne) posés dans le
            // plan de la face (u vers la droite, v vers le haut vu de l'extérieur), puis
            // projetés avec le cube ; caché sur une face presque de profil
            if face > 0.15 {
                painter.add(texte_sur_face(f.3, f.0, f.1, f.2, &ecran, encre, 0.34));
            }
        }
        tracer_anneau(true);

        let info = match &survol {
            Some(Cible::Cellule(d, ..)) => {
                let n = |x: f32, pos: &str, neg: &str| if x > 0.5 { pos.to_string() } else if x < -0.5 { neg.to_string() } else { String::new() };
                let t = [n(d.z, "haut", "bas"), n(-d.y, "avant", "arrière"), n(d.x, "droite", "gauche")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
                Some(format!("Vue depuis : {t}"))
            }
            Some(Cible::Lettre(i)) => Some(format!("Vue depuis {}", cardinaux[*i].2)),
            Some(Cible::Anneau) => Some("Glisser pour tourner autour de la verticale".to_string()),
            None => None,
        };
        let clic = resp.clicked();
        if let Some(t) = info {
            resp.on_hover_text(t);
        }
        if !clic {
            return;
        }
        match survol {
            Some(Cible::Cellule(d, ..)) => {
                // regarder depuis d vers le centre ; dessus et dessous : nord en haut
                let d = d.normalize();
                self.camera.pitch = d.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
                self.camera.yaw = if d.truncate().length() < 1e-4 { -std::f32::consts::FRAC_PI_2 } else { d.y.atan2(d.x) };
            }
            // depuis un point cardinal, inclinaison gardée
            Some(Cible::Lettre(i)) => self.camera.yaw = cardinaux[i].1.y.atan2(cardinaux[i].1.x),
            _ => {}
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(e) = &self.error {
                ui.colored_label(Color32::from_rgb(200, 40, 40), format!("Erreur : {e}"));
                ui.separator();
            }
            ui.label(&self.status);
            if let (Some(p), Some(s)) = (self.point_clique, &self.scene) {
                // coordonnées partagées (Revit placé) ou du fichier
                ui.separator();
                let texte = match &s.partage {
                    Some(r) => {
                        let q = r.partage(p);
                        format!("E/O {:.3} · N/S {:.3} · Élév. {:.3} m ({})", q.x, q.y, q.z, r.nom)
                    }
                    None => format!("X {:.3} · Y {:.3} · Z {:.3} m", p.x, p.y, p.z),
                };
                ui.label(texte).on_hover_text("Point cliqué");
            }
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
                let aide = if self.vue_3d() { "Gauche : orbite · Droit : vol · Milieu : pan" } else { "Gauche/droit/milieu : pan" };
                ui.label(RichText::new(format!("{aide} · Molette : zoom · Double-clic : cadrer")).weak());
            });
        });
    }

    /// Arborescence du projet (panneau de droite, comme dans Revit).
    fn left_panel(&mut self, ui: &mut egui::Ui) {
        let nom = self.scene.as_ref().map(|s| s.file_name.clone()).unwrap_or_default();
        let titre = if nom.is_empty() { "Arborescence du projet".to_string() } else { format!("Arborescence du projet - {nom}") };
        self.entete_panneau(ui, Panneau::Arborescence, &titre);
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
                self.nomenclature_ouverte = Some(i);
                return;
            }
            _ => (annot.clone(), annot.clone()),
        };
        let (d, haut, entite) = (v.direction.as_vec3(), v.haut.as_vec3(), v.entite);
        let genre_feuille = v.genre == "Feuille";
        let coupe_vue = v.coupe;
        // mode dessin pour les plans, coupes et élévations ; coupe à hauteur sur le niveau
        // d'un plan d'étage
        let (genre, etage, echelle) = (v.genre.clone(), v.etage, v.echelle);
        if let Some(d) = NiveauDetail::depuis_revit(v.detail) {
            self.detail = d;
        }
        match genre.as_str() {
            "Plan d'étage" | "Plan de plafond" | "Coupe" | "Élévation" => {
                self.style = StyleVisuel::LignesCachees;
                self.coupe = genre == "Plan d'étage";
                if self.coupe {
                    self.coupe_niveau = etage;
                    // hauteur du plan de coupe de la plage de vue
                    if let Some(h) = coupe_vue {
                        self.coupe_hauteur = h as f32;
                    }
                }
            }
            // vue 3D : le style 3D en cours, réaliste en sortant d'un dessin
            "Vue 3D" | "Feuille" if self.style == StyleVisuel::LignesCachees => self.style = StyleVisuel::Realiste,
            _ => {}
        }
        // échelle de la barre de vue : celle de la vue ouverte
        if echelle > 0.0 {
            self.echelle = echelle;
        }
        self.vue_active = Some(i);
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
        // filtre de phase de la vue : objets dont l'état dans la phase de la vue est masqué
        let masques = self.masques_de_phase(i);
        self.hidden_objects.extend(masques);
        let secondaires = self.masques_variantes();
        self.hidden_objects.extend(secondaires);
        // vues placées d'une feuille, selon la visibilité de la vue (annotations montrées)
        if genre_feuille && self.isolated.is_some() {
            self.hidden_classes.remove("IFCANNOTATION");
        }
        self.preparer_feuille(Some(i));
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

    /// Objets des options de variante secondaires (masqués par défaut, comme dans Revit).
    fn masques_variantes(&self) -> Vec<u32> {
        let Some(s) = &self.scene else { return Vec::new() };
        let Some(nav) = &s.navigateur else { return Vec::new() };
        if nav.variantes_secondaires.is_empty() {
            return Vec::new();
        }
        (0..s.objects.len() as u32).filter(|&o| nav.variantes_secondaires.contains(&s.objects[o as usize].id)).collect()
    }

    /// Objets qu'une vue Revit ne montre pas selon sa phase et son filtre de phase (état
    /// de l'objet dans la phase, présentation « masqué » du filtre).
    fn masques_de_phase(&self, i: usize) -> Vec<u32> {
        let Some(s) = &self.scene else { return Vec::new() };
        let Some(nav) = &s.navigateur else { return Vec::new() };
        let (Some(v), Some(filtre)) = (nav.vues.get(i), nav.vues.get(i).and_then(|v| nav.filtres_phase.get(&v.filtre_phase))) else { return Vec::new() };
        (0..s.objects.len() as u32)
            .filter(|&o| {
                nav.phases_objets.get(&s.objects[o as usize].id).is_some_and(|&(c, d)| filtre[scene::etat_de_phase(&nav.phases, c, d, v.phase)] == 0)
            })
            .collect()
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
        let mut objets_feuille = Vec::new();
        if let Some(s) = &self.scene {
            let nav = s.navigateur.as_ref();
            let feuille = i.and_then(|i| nav?.vues.get(i)).filter(|v| v.genre == "Feuille");
            let o = s.origin;
            // catégorie montrée (boîte de visibilité)
            let montre = |k: &u32| !self.hidden_classes.contains(&*s.objects[*k as usize].class) && !self.hidden_classes.contains(&*self.cle_objet[*k as usize]);
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
                for (mut objs, cadre, plans) in passes_vue {
                    objets_feuille.extend_from_slice(&objs);
                    objs.retain(montre);
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
        // un objet de plusieurs vues placées compte une fois
        objets_feuille.sort_unstable();
        objets_feuille.dedup();
        self.objets_feuille = objets_feuille;
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
        self.entete_panneau(ui, Panneau::Proprietes, "Propriétés");
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
        if self.scene.is_some() && self.style == StyleVisuel::LignesCachees {
            self.plage_de_vue(ui);
            ui.separator();
        }
        if self.boite_active {
            self.proprietes_boite(ui);
            ui.separator();
        }
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
                self.hidden_classes.remove(&*self.cle_objet[o as usize]);
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

    /// Tableau d'une nomenclature Revit, recomposé depuis les éléments lus ; les colonnes
    /// que Revit calcule (formules, pièces d'une porte…) sont signalées.
    fn fenetre_nomenclature(&mut self, ctx: &egui::Context) {
        let Some(i) = self.nomenclature_ouverte else { return };
        let Some(v) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)) else { return };
        let Some((titres, lignes, nc, tris)) = &v.tableau else { return };
        let mut ouvert = true;
        egui::Window::new(format!("Nomenclature : {}", v.nom)).open(&mut ouvert).default_size([720.0, 420.0]).show(ctx, |ui| {
            if !nc.is_empty() {
                let noms: Vec<&str> = nc.iter().filter_map(|&k| titres.get(k).map(String::as_str)).collect();
                ui.label(RichText::new(format!("Non calculées (calcul de Revit) : {}", noms.join(", "))).weak());
            }
            ui.label(RichText::new(format!("{} lignes{}", lignes.len(), if tris.is_empty() { String::new() } else { format!(" ; tri : {tris}") })).weak());
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("table_nomenclature").striped(true).show(ui, |ui| {
                    for t in titres {
                        ui.label(RichText::new(t).strong());
                    }
                    ui.end_row();
                    for l in lignes {
                        for c in l {
                            ui.label(c);
                        }
                        ui.end_row();
                    }
                });
            });
        });
        if !ouvert {
            self.nomenclature_ouverte = None;
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
                s.pick(o, d, &app.visible, app.plan_coupe(), app.boite_appliquee()).map(|(obj, t)| (obj, o + d * t))
            };

            // Orbite autour du point visé (vue 3D seulement).
            if resp.drag_started_by(PointerButton::Primary) && !ui.input(|i| i.modifiers.shift) && self.vue_3d() {
                if let Some((_, p)) = resp.interact_pointer_pos().and_then(|pp| pick(self, pp)) {
                    self.camera.set_pivot(p);
                }
            }
            let shift = ui.input(|i| i.modifiers.shift);
            self.voler(ui, &resp);
            for b in [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle] {
                if resp.dragged_by(b) && !(self.vol.actif && b == PointerButton::Secondary) {
                    self.glisser(b, resp.drag_delta(), shift, rect.height());
                }
            }

            if self.vol.actif {
                // molette : vitesse du vol, pas de zoom
            } else if let Some(hp) = resp.hover_pos() {
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
                self.point_clique = hit.map(|(_, p)| p.as_dvec3() + self.scene.as_ref().unwrap().origin);
                self.select(id, true);
            }

            if (resp.hovered() || resp.has_focus()) && !self.vol.actif {
                let (f, esc, h, i, v) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::F),
                        i.key_pressed(egui::Key::Escape),
                        i.key_pressed(egui::Key::H),
                        i.key_pressed(egui::Key::I),
                        i.key_pressed(egui::Key::V),
                    )
                });
                if v {
                    self.voir_visibilite = !self.voir_visibilite;
                }
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
        if self.scene.is_some() {
            self.poignees_boite(ui, rect);
        }
        if self.vol.actif {
            ui.painter().text(
                rect.left_bottom() + egui::vec2(10.0, -10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Vol · vitesse ×{:.2} (molette) · ZQSD/WASD, A/E : descendre/monter, Maj ×3", self.vol.vitesse),
                egui::FontId::proportional(12.0),
                ui.visuals().strong_text_color(),
            );
        }
        if self.scene.is_some() && self.vue_3d() {
            self.cube_orientation(ui, rect);
        }

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
            background: if self.style.papier() { [1.0, 1.0, 1.0] } else { [0.925, 0.933, 0.945] },
            style: self.style,
            soleil: (self.style == StyleVisuel::Rendu).then(|| self.soleil()).flatten(),
            coupe: self.plan_coupe(),
            boite: self.boite_appliquee(),
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
            p.push(("vue", objet(&[("indice", self.vue_active.unwrap_or(0).to_string()), ("genre", texte(&v.genre)), ("nom", texte(&v.nom)), ("echelle", v.echelle.to_string()), ("niveau_detail", texte(self.detail.libelle()))])));
        }
        if let (Some((a, b)), Some(s)) = (self.boite, &self.scene) {
            let (a, b) = (a.as_dvec3() + s.origin, b.as_dvec3() + s.origin);
            p.push(("zone_de_coupe", objet(&[("active", self.boite_active.to_string()), ("min", format!("[{:.3},{:.3},{:.3}]", a.x, a.y, a.z)), ("max", format!("[{:.3},{:.3},{:.3}]", b.x, b.y, b.z))])));
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
                ("actif", (self.style == StyleVisuel::LignesCachees).to_string()),
                ("style", texte(self.style.nom())),
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
        p.push(("panneaux", objet(&Panneau::TOUS.map(|q| (q.nom(), texte(self.ancrages[q as usize].nom()))))));
        p.push(("visibilite_ouverte", self.voir_visibilite.to_string()));
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
            "zone" => {
                // zone on|off|ajuster, ou zone <xmin> <ymin> <zmin> <xmax> <ymax> <zmax> (m, repère du fichier)
                let v: Vec<f64> = argument.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                match (argument.trim(), &v[..]) {
                    ("on", _) => self.activer_boite(true),
                    ("off", _) => self.boite_active = false,
                    ("ajuster", _) => {
                        self.boite = None;
                        self.activer_boite(true);
                    }
                    (_, &[x0, y0, z0, x1, y1, z1]) => {
                        let o = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin);
                        self.boite_active = true;
                        self.regler_boite((glam::DVec3::new(x0, y0, z0) - o).as_vec3(), (glam::DVec3::new(x1, y1, z1) - o).as_vec3());
                    }
                    _ => return erreur("usage : zone on|off|ajuster, ou zone <xmin> <ymin> <zmin> <xmax> <ymax> <zmax>"),
                }
                self.etat()
            }
            "camera" => {
                // vue 2D : orientation et projection fixées par la vue Revit
                if !self.vue_3d() && !matches!(argument, "cadrer" | "ortho") {
                    return erreur("vue 2D : orientation fixe (cadrer seulement ; tout_afficher revient au modèle 3D)");
                }
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
                self.style = if argument == "non" { StyleVisuel::Realiste } else { StyleVisuel::LignesCachees };
                self.visibility_dirty = true;
                self.etat()
            }
            "style" => {
                let Some(&st) = StyleVisuel::TOUS.iter().find(|x| x.nom() == argument) else {
                    return erreur("usage : style filaire|lignes_cachees|ombre|couleurs|realiste|rendu");
                };
                self.style = st;
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
                    self.style = StyleVisuel::LignesCachees;
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
            "panneau" => {
                // panneau proprietes|arborescence gauche|droite|flottant
                let (p, a) = argument.split_once(' ').unwrap_or((argument, ""));
                let p = Panneau::TOUS.into_iter().find(|x| x.nom() == p);
                let a = Ancrage::TOUS.into_iter().find(|x| x.nom() == a.trim());
                let (Some(p), Some(a)) = (p, a) else { return erreur("usage : panneau proprietes|arborescence gauche|droite|flottant") };
                self.ancrer(p, a);
                self.etat()
            }
            "visibilite" => {
                // visibilite oui|non : boîte ; visibilite <catégorie> oui|non
                let (nom, oui) = match argument.rsplit_once(' ') {
                    Some((n, o)) if o == "oui" || o == "non" => (n.trim(), o == "oui"),
                    _ => ("", argument != "non"),
                };
                if nom.is_empty() {
                    self.voir_visibilite = oui;
                    return self.etat();
                }
                let n = nom.to_lowercase();
                let k = self.categories.iter().position(|c| c.nom.to_lowercase() == n).or_else(|| self.categories.iter().position(|c| c.nom.to_lowercase().contains(&n)));
                let Some(k) = k else { return erreur(&format!("catégorie introuvable : {nom}")) };
                self.montrer_categorie(k, oui);
                self.update_visibility();
                self.etat()
            }
            "categories" => {
                let mut nombres = vec![0usize; self.categories.len()];
                for &k in &self.categorie_objet {
                    nombres[k] += 1;
                }
                let c = self.categories.iter().zip(nombres).map(|(c, n)| {
                    objet(&[("nom", texte(&c.nom)), ("genre", texte(if c.annotation { "annotation" } else { "modele" })), ("objets", n.to_string()), ("visible", self.categorie_visible(c).to_string())])
                });
                succes(&[("categories", liste(c))])
            }
            "glisser" => {
                // glisser <dx> <dy> [gauche|droit|milieu] [maj] : comme à la souris (pixels)
                let mots: Vec<&str> = argument.split_whitespace().collect();
                let (Some(Ok(dx)), Some(Ok(dy))) = (mots.first().map(|x| x.parse::<f32>()), mots.get(1).map(|x| x.parse::<f32>())) else {
                    return erreur("usage : glisser <dx> <dy> [gauche|droit|milieu] [maj]");
                };
                let bouton = match mots.get(2).copied() {
                    Some("droit") => PointerButton::Secondary,
                    Some("milieu") => PointerButton::Middle,
                    _ => PointerButton::Primary,
                };
                self.glisser(bouton, egui::vec2(dx, dy), mots.contains(&"maj"), self.rect_vue.height());
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
                match s.pick(o, d, &self.visible, self.plan_coupe(), self.boite_appliquee()) {
                    Some((obj, t)) => {
                        let p = o + d * t;
                        let f = p.as_dvec3() + s.origin;
                        let mut r = vec![("entite", self.decrire(s.objects[obj as usize].id)), ("point", format!("[{:.3},{:.3},{:.3}]", f.x, f.y, f.z))];
                        if let Some(g) = &s.partage {
                            let q = g.partage(f);
                            r.push(("partage", format!("[{:.3},{:.3},{:.3}]", q.x, q.y, q.z)));
                        }
                        succes(&r)
                    }
                    None => succes(&[("entite", "null".into())]),
                }
            }
            _ => erreur(&format!("commande inconnue : {commande}")),
        }
    }
}

/// Icône d'un style visuel : petit cube en perspective, tracé dans `r`.
/// Icône du niveau de détail : un, deux ou trois traits, comme dans Revit.
fn icone_detail(p: &egui::Painter, r: egui::Rect, d: NiveauDetail) {
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

fn icone_style(p: &egui::Painter, r: egui::Rect, style: StyleVisuel) {
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
            StyleVisuel::Filaire => None,
            StyleVisuel::LignesCachees => Some(Color32::WHITE),
            StyleVisuel::Ombre => Some(Color32::from_gray([235, 185, 150][i])),
            StyleVisuel::CouleursHomogenes => Some(Color32::from_rgb(150, 180, 210)),
            StyleVisuel::Realiste | StyleVisuel::Rendu => Some([Color32::from_rgb(232, 196, 150), Color32::from_rgb(200, 150, 100), Color32::from_rgb(165, 118, 75)][i]),
        }
    };
    if style == StyleVisuel::Rendu {
        // ombre portée au sol
        p.add(egui::Shape::convex_polygon(vec![pt(1., -1., -1.), pt(2.4, -0.2, -1.), pt(2.4, 1.8, -1.), pt(1., 1., -1.)], Color32::from_gray(150), Stroke::NONE));
    }
    if style == StyleVisuel::Filaire {
        // arêtes cachées aussi
        let a = pt(1., 1., -1.);
        for b in [pt(1., -1., -1.), pt(-1., 1., -1.), pt(1., 1., 1.)] {
            p.line_segment([a, b], Stroke::new(1.0, Color32::from_gray(140)));
        }
    }
    for (coins, i) in faces {
        let trait_ = if matches!(style, StyleVisuel::Realiste | StyleVisuel::Rendu) { Stroke::NONE } else { t };
        match remplissage(i) {
            Some(f) => p.add(egui::Shape::convex_polygon(coins.to_vec(), f, trait_)),
            None => p.add(egui::Shape::closed_line(coins.to_vec(), t)),
        };
    }
}

/// Texte gravé au milieu d'une face du cube d'orientation (normale `n`, axes `u`, `v`
/// dans le plan, côté du cube = 2) : capitales de `capitales`, largeur au plus 1,7, centré ;
/// maillage egui des glyphes projetés par `ecran`.
fn texte_sur_face(texte: &str, n: Vec3, u: Vec3, v: Vec3, ecran: &dyn Fn(Vec3) -> egui::Pos2, couleur: Color32, capitales: f64) -> egui::Shape {
    const POLICE: &str = "Arial";
    let largeur = crate::texte::largeur(texte, POLICE, 1.0, true).max(1e-6);
    let taille = capitales.min(1.7 / largeur);
    let l = crate::texte::ligne(texte, POLICE, taille, true);
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
/// Classe IFC d'une clé de masquage (« classe » ou « classe:nom »).
fn classe_cle(cle: &str) -> &str {
    cle.split_once(':').map_or(cle, |c| c.0)
}

/// Catégories de visibilité de la scène, puis catégorie et clé de masquage de chaque
/// objet (voir [`Categorie`]) : genre (`ObjectType`) des annotations ; catégorie Revit
/// (propriété « Catégorie ») des proxys d'un fichier Revit ; nom de la classe IFC
/// sinon. Les quadrillages sont des annotations, comme dans Revit.
fn categories(s: &Scene) -> (Vec<Categorie>, Vec<usize>, Vec<Arc<str>>) {
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
fn nom_classe(classe: &str) -> String {
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
fn nom_genre(genre: &str) -> String {
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
            // disposition de Revit par défaut : propriétés à gauche, arborescence du projet
            // à droite ; chacun s'ancre d'un côté ou flotte (ancrages relevés en début
            // d'image, changés pour la suivante)
            let ancrages = self.ancrages;
            for p in Panneau::TOUS {
                let id = egui::IdSalt::new((p.nom(), ancrages[p as usize].nom()));
                let panneau = match ancrages[p as usize] {
                    Ancrage::Gauche => egui::Panel::left(id),
                    Ancrage::Droite => egui::Panel::right(id),
                    Ancrage::Flottant => continue,
                };
                panneau.resizable(true).default_size(if p == Panneau::Proprietes { 300.0 } else { 320.0 }).show(ui, |ui| self.panneau(ui, p));
            }
            // sous la vue seulement, entre les panneaux ancrés
            egui::Panel::bottom("barre_vue").show(ui, |ui| self.barre_vue(ui));
            egui::CentralPanel::no_frame().show(ui, |ui| self.viewport(ui));
            for p in Panneau::TOUS.into_iter().filter(|p| ancrages[*p as usize] == Ancrage::Flottant) {
                let i = p as usize;
                let mut fenetre = egui::Window::new(p.nom()).id(egui::Id::unique(("flottant", p.nom()))).title_bar(false).resizable(true).default_size([320.0, 520.0]).min_width(220.0);
                if let Some(q) = self.pos_imposee[i].take() {
                    fenetre = fenetre.current_pos(q);
                }
                let r = fenetre.show(&ctx, |ui| self.panneau(ui, p));
                // position imposée pendant ce tracé (titre glissé) : place de la suivante
                self.pos_flottants[i] = match (self.pos_imposee[i], r) {
                    (Some(q), _) => q,
                    (None, Some(r)) => r.response.rect.min,
                    (None, None) => self.pos_flottants[i],
                };
            }
            self.fenetre_visibilite(&ctx);
        }
        self.unsupported_window(&ctx);
        self.fenetre_nomenclature(&ctx);
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
