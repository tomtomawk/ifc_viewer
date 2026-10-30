//! Interface egui, disposée comme Revit : propriétés à gauche, vue au centre,
//! arborescence du projet à droite (vues, nomenclatures, feuilles, familles, groupes et
//! liens d'un fichier Revit ; arborescence spatiale et classes d'un IFC). Les deux
//! panneaux s'ancrent d'un côté ou flottent ; sous la vue, la boîte de visibilité par
//! catégorie de modèle et d'annotation.

mod bim;
mod feuilles;
mod dessin2d_ui;
mod famille_ui;
use crate::options::Dimensions;
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
    Natif(Result<(Box<Scene>, crate::modele::Projet, PathBuf), String>),
    Progress(String),
    Done(Result<Box<Scene>, String>),
    /// Métadonnées, puis scène des murs (ou raison de son absence).
    Revit(Result<(Box<RvtInfo>, Result<Box<Scene>, String>, crate::modele::enregistrement::SourceRvt), String>),
}

enum SaveMsg {
    Progress(String),
    Done(Result<crate::modele::enregistrement::Bilan, String>),
}

struct Sauvegarde {
    document: usize,
    progression: String,
    messages: Receiver<SaveMsg>,
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
        "Nomenclature" | "Feuille" | "Jeu de feuilles" => return None,
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

/// Accrochage du point de tracé d'un mur.
#[derive(Clone, Copy, PartialEq, Debug)]
enum AccrocheMur {
    Grille,
    Extremite,
    Milieu,
    Orthogonal,
    /// Angle rond depuis le départ (degrés).
    Angle(f64),
    /// Angle d'un mur (contour raccordé).
    Coin,
}

/// Onglet du ruban en haut de la fenêtre.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OngletRuban {
    Debut,
    Modeliser,
    Modifier,
    Annoter,
    Sortie,
}

/// Une croix tracée au centre de son bouton, sans dépendre des métriques de police.
fn dessiner_croix_onglet(painter: &egui::Painter, centre: egui::Pos2, couleur: Color32) {
    let trace = egui::Stroke::new(1.2, couleur);
    painter.line_segment([centre + egui::vec2(-3.0, -3.0), centre + egui::vec2(3.0, 3.0)], trace);
    painter.line_segment([centre + egui::vec2(-3.0, 3.0), centre + egui::vec2(3.0, -3.0)], trace);
}

/// Teinte stable pendant toute la durée d'ouverture du fichier.
fn couleur_document(id: usize) -> Color32 {
    const TEINTES: [[u8; 3]; 8] = [
        [83, 122, 158], [93, 137, 112], [146, 111, 151], [165, 124, 78],
        [77, 139, 145], [162, 105, 111], [119, 124, 168], [132, 137, 83],
    ];
    let [r, g, b] = TEINTES[id.wrapping_sub(1) % TEINTES.len()];
    Color32::from_rgb(r, g, b)
}

/// Réglages conservés quand on quitte un onglet de vue.
#[derive(Clone)]
struct EtatVue {
    camera: Camera,
    hidden_classes: HashSet<String>,
    hidden_objects: HashSet<u32>,
    hidden_storeys: HashSet<Id>,
    isolated: Option<HashSet<u32>>,
    style: StyleVisuel,
    faces_blanches: bool,
    eclairage: bool,
    traits_fins: bool,
    ombres_portees: bool,
    ombre_ambiante: bool,
    coupe: bool,
    coupe_niveau: Option<Id>,
    coupe_hauteur: f32,
    poche: Poche,
    echelle: f64,
    detail: NiveauDetail,
    boite: Option<(Vec3, Vec3)>,
    boite_active: bool,
}

#[derive(Clone, Copy)]
struct FenetreSelection { debut: egui::Pos2, fin: egui::Pos2, glissement: bool, ajouter: bool }

/// Fichier conservé en mémoire quand un autre document devient actif.
struct DocumentOuvert {
    natif: Option<(crate::modele::Projet, PathBuf)>,
    historique: crate::modele::historique::Historique,
    modifie: bool,
    scene: Option<Scene>,
    rvt: Option<RvtInfo>,
    rvt_source: Option<crate::modele::enregistrement::SourceRvt>,
    rvt_sortie: Option<PathBuf>,
    rvt_geom: Result<String, String>,
    rvt_sheet: bool,
    vues: Vec<Option<usize>>,
    etats: BTreeMap<Option<usize>, EtatVue>,
    active: Option<usize>,
    selected: Option<Id>,
    filtre: String,
    onglet: LeftTab,
    status: String,
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
    bim: bim::EtatBim,
    /// Jeu de feuilles ouvert : cadrage, sélection, dessins préparés.
    feuilles: feuilles::EtatFeuilles,
    /// Dessin 2D (Documenter) : tracé, saisie, accrochage.
    dessin2d: dessin2d_ui::EtatDessin2D,
    /// Éditeur de famille de fenêtre ouvert.
    famille: Option<famille_ui::EditeurFamille>,
    /// Onglet de l'éditeur de familles au premier plan.
    famille_active: bool,
    /// Autres familles ouvertes (onglets à l'arrière-plan).
    familles_inactives: Vec<famille_ui::EditeurFamille>,
    prochain_ordre_famille: usize,
    /// Action des panneaux de l'éditeur de familles, traitée après leur tracé.
    action_famille: Option<famille_ui::ActionFamille>,
    /// Fermeture de l'onglet Famille à confirmer (modifications non chargées).
    confirmer_famille: bool,
    natif: Option<(crate::modele::Projet, PathBuf)>,
    historique: crate::modele::historique::Historique,
    natif_modifie: bool,
    /// Repère du plan : le prochain clic choisit l'arête à mettre à l'horizontale.
    alignement_plan: bool,
    /// Arête visée en mode Aligner (repère de la scène), surlignée.
    arete_alignement: Option<[Vec3; 2]>,
    /// Arêtes en plan de l'objet survolé en mode Aligner (cache).
    aretes_objet: Option<(u32, Vec<[Vec3; 2]>)>,
    /// Orientations de plan enregistrées hors projet natif (session).
    positions_plan: Vec<(String, f32)>,
    /// Boîte « Nouvelle position » ouverte : nom saisi.
    nom_position_plan: Option<String>,
    /// Nom de la vue native ouverte en cours de saisie (identifiant, texte).
    nom_vue: Option<(String, String)>,
    /// Début de la ligne de coupe en cours de tracé (mm).
    coupe_depart: Option<[f64; 2]>,
    raccourci_en_attente: Option<(char, f64)>,
    confirmer_fermeture: Option<(bool, usize)>, // application entière, ou identifiant du document
    documents: BTreeMap<usize, DocumentOuvert>,
    document_actif: usize,
    prochain_document: usize,
    gl: Option<Arc<glow::Context>>,
    renderer: Arc<Mutex<Option<Renderer>>>,
    scene: Option<Scene>,
    /// Métadonnées du fichier Revit ouvert.
    rvt: Option<RvtInfo>,
    rvt_source: Option<crate::modele::enregistrement::SourceRvt>,
    rvt_sortie: Option<PathBuf>,
    sauvegarde: Option<Sauvegarde>,
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
    selection_multiple: Vec<Id>,
    fenetre_selection: Option<FenetreSelection>,
    reveal: Option<Id>,
    hidden_classes: HashSet<String>,
    hidden_objects: HashSet<u32>,
    hidden_storeys: HashSet<Id>,
    isolated: Option<HashSet<u32>>,
    visible: Vec<bool>,
    visibility_dirty: bool,
    left_tab: LeftTab,
    /// Graphique : faces blanches (dessin au trait), éclairage des faces.
    faces_blanches: bool,
    eclairage: bool,
    /// Traits fins : tous les traits à un pixel, plumes ignorées (comme Revit).
    traits_fins: bool,
    /// Ombres portées et ombre ambiante : réglages à part du style.
    ombres_portees: bool,
    ombre_ambiante: bool,
    /// Pente donnée à une arête cochée (degrés), la dernière saisie.
    pente_defaut: f64,
    /// Options de l'application (menu Début > Options).
    options: crate::options::Options,
    filter: String,
    show_unsupported: bool,
    /// Vue Revit ouverte (indice dans `Navigateur::vues`).
    vue_active: Option<usize>,
    /// None désigne la vue générale du modèle ; Some, une vue du navigateur.
    vues_ouvertes: Vec<Option<usize>>,
    etats_vues: BTreeMap<Option<usize>, EtatVue>,
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
    /// Onglet actif avant qu'une sélection n'ouvre Modifier (rétabli à la désélection),
    /// et sélection native à l'image précédente.
    onglet_avant_selection: Option<OngletRuban>,
    selection_precedente: Vec<String>,
    ruban_edition: crate::ruban::RubanEdition,
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
    /// Poignée d'extrémité de l'axe du mur sélectionné en cours de déplacement : élément,
    /// extrémité (true : fin) et position courante (mm).
    poignee_axe: Option<(String, bool, [f64; 2])>,
    /// Matériau du projet natif dont les graphismes sont ouverts.
    materiau_edite: Option<String>,
    /// Éditeur de hachures ouvert, motif choisi, nom et tirets en cours de saisie (motif,
    /// texte des tirets de chaque famille).
    editeur_motifs: bool,
    /// Fenêtre « Épaisseurs de traits » ouverte : table des 12 plumes en cours d'édition.
    editeur_plumes: Option<Vec<f64>>,
    /// Saisie en cours dans un champ (texte, nombre tapé ou glissé hors de la vue) :
    /// les modifications du projet attendent sa validation (Entrée, autre clic, fin du
    /// glisser) pour être appliquées, en une seule étape d'historique.
    saisie_active: bool,
    /// Projet tel qu'avant la saisie en cours, si elle a modifié le projet.
    avant_saisie: Option<crate::modele::Projet>,
    /// Échap pendant la saisie : elle est abandonnée, l'ancienne valeur rétablie.
    saisie_annulee: bool,
    /// Échap a abandonné une saisie à cette image (champs à tampon propre).
    pub(crate) echap_saisie: bool,
    motif_choisi: Option<String>,
    nom_motif_saisi: String,
    tirets_saisis: (String, Vec<String>),
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
    deplacement_panneau: Option<(Panneau, egui::Vec2)>,
    zone_panneaux: egui::Rect,
    largeurs_panneaux: [f32; 2],
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
            bim: Default::default(),
            feuilles: Default::default(),
            natif: None,
            historique: Default::default(),
            natif_modifie: false,
            raccourci_en_attente: None,
            coupe_depart: None,
            nom_vue: None,
            alignement_plan: false,
            dessin2d: Default::default(),
            famille: None,
            famille_active: false,
            familles_inactives: Vec::new(),
            prochain_ordre_famille: 0,
            action_famille: None,
            confirmer_famille: false,
            arete_alignement: None,
            aretes_objet: None,
            positions_plan: Vec::new(),
            nom_position_plan: None,
            confirmer_fermeture: None,
            documents: BTreeMap::new(),
            document_actif: 0,
            prochain_document: 1,
            gl: cc.gl.clone(),
            renderer: Arc::new(Mutex::new(renderer)),
            scene: None,
            rvt: None,
            rvt_source: None,
            rvt_sortie: None,
            sauvegarde: None,
            rvt_sheet: false,
            rvt_geom: Err(String::new()),
            rvt_preview: None,
            loading: None,
            status: "Ouvrir un fichier IFC ou Revit (Ctrl+O), ou le glisser dans la fenêtre.".into(),
            error,
            camera: Camera::default(),
            selected: None,
            selection_multiple: Vec::new(),
            fenetre_selection: None,
            reveal: None,
            hidden_classes: HashSet::new(),
            hidden_objects: HashSet::new(),
            hidden_storeys: HashSet::new(),
            isolated: None,
            visible: Vec::new(),
            visibility_dirty: false,
            left_tab: LeftTab::Tree,
            pente_defaut: 35.0,
            ombres_portees: false,
            ombre_ambiante: false,
            faces_blanches: false,
            traits_fins: false,
            eclairage: true,
            options: {
                let o = crate::options::Options::charger();
                crate::options::definir_unite(o.unite);
                // gabarit choisi : lu au démarrage, défaut s'il est illisible
                if let Some(c) = &o.gabarit {
                    match crate::gabarit::Gabarit::charger(c) {
                        Ok(g) => crate::gabarit::definir(g),
                        Err(e) => eprintln!("{e} : gabarit par défaut utilisé"),
                    }
                }
                o
            },
            filter: String::new(),
            show_unsupported: false,
            vue_active: None,
            vues_ouvertes: Vec::new(),
            etats_vues: BTreeMap::new(),
            point_clique: None,
            vol: Vol::default(),
            nomenclature_ouverte: None,
            pilote: pilote::demarrer(cc.egui_ctx.clone()),
            requetes: Vec::new(),
            attente_ouverture: Vec::new(),
            rect_vue: egui::Rect::NOTHING,
            passes_feuille: Vec::new(),
            objets_feuille: Vec::new(),
            style: StyleVisuel::Graphique,
            coupe: false,
            coupe_niveau: None,
            coupe_hauteur: 1.2,
            poche: Poche::Hachure,
            echelle: 100.0,
            detail: NiveauDetail::Moyen,
            onglet_ruban: OngletRuban::Debut,
            onglet_avant_selection: None,
            selection_precedente: Vec::new(),
            ruban_edition: Default::default(),
            boite: None,
            boite_active: false,
            coupe_dirty: true,
            liste_niveaux: Vec::new(),
            liste_materiaux: Vec::new(),
            materiau_edite: None,
            editeur_motifs: false,
            editeur_plumes: None,
            saisie_active: false,
            avant_saisie: None,
            saisie_annulee: false,
            echap_saisie: false,
            motif_choisi: None,
            nom_motif_saisi: String::new(),
            tirets_saisis: (String::new(), Vec::new()),
            poignee_axe: None,
            categories: Vec::new(),
            categorie_objet: Vec::new(),
            cle_objet: Vec::new(),
            voir_visibilite: false,
            ancrages: [Ancrage::Gauche, Ancrage::Droite],
            pos_flottants: [egui::pos2(60.0, 80.0), egui::pos2(400.0, 80.0)],
            pos_imposee: [None, None],
            deplacement_panneau: None,
            zone_panneaux: egui::Rect::NOTHING,
            largeurs_panneaux: [300.0, 320.0],
        };
        if let Some(p) = path {
            app.open(p, &cc.egui_ctx);
        }
        app
    }

    fn open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let recuperation=crate::modele::enregistrement::chemin_recuperation(&path);
        let restaurer=if path.extension().is_some_and(|e|e.eq_ignore_ascii_case("json")) && recuperation.exists() {
            match rfd::MessageDialog::new().set_title("Récupération du projet").set_description("Une copie de récupération existe. Restaurer les modifications non enregistrées ?").set_buttons(rfd::MessageButtons::YesNoCancel).show() {
                rfd::MessageDialogResult::Yes=>true,
                rfd::MessageDialogResult::No=>false,
                _=>return,
            }
        } else {false};
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
                if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
                    let res = std::fs::read_to_string(if restaurer { &recuperation } else { &path }).map_err(|e| format!("Lecture impossible : {e}"))
                        .and_then(|json| crate::modele::Projet::lire_json(&json))
                        .and_then(|projet| scene::depuis_natif(&projet, &path).map(|scene| (Box::new(scene), projet, path)));
                    let _ = tx.send(LoadMsg::Natif(res));
                } else if is_revit_file(&path) {
                    progress(format!("Lecture des métadonnées Revit de {}…", path.display()));
                    let res = crate::modele::enregistrement::SourceRvt::ouvrir(&path).and_then(|source| rvt::load(&path).map(|info| {
                        let titre = info.title.clone().unwrap_or_else(|| info.file_name.clone());
                        let format = info.format().unwrap_or("?").to_string();
                        let scene = rvt::maquette::load_scene(&path, &titre, &format, &progress).map(Box::new);
                        (Box::new(info), scene, source)
                    }));
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
        loop {
            let msg=match rx.try_recv() {
                Ok(msg)=>msg,
                Err(std::sync::mpsc::TryRecvError::Empty)=>return,
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                    self.loading=None;self.error=Some("Le chargement a été interrompu ; le document précédent est conservé.".into());return;
                }
            };
            match msg {
                LoadMsg::Natif(Ok((scene, projet, chemin))) => {
                    self.loading = None;
                    self.install_scene(*scene);
                    self.status = format!("{} — projet natif v0", projet.nom);
                    let _ = self.historique.enregistrer(&projet);
                    if let Ok(ancien)=std::fs::read_to_string(&chemin).map_err(|e|e.to_string()).and_then(|s|crate::modele::Projet::lire_json(&s)) {
                        let _=self.historique.enregistrer(&ancien);
                        self.natif_modifie=self.historique.modifie(&projet);
                    }
                    self.natif = Some((projet, chemin));
                    self.avant_saisie = None;
                    return;
                }
                LoadMsg::Progress(s) => self.status = s,
                LoadMsg::Done(Ok(scene)) => {
                    self.loading = None;
                    self.install_scene(*scene);
                    return;
                }
                LoadMsg::Revit(Ok((info, scene, source))) => {
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
                            self.nouveau_document();
                            self.vues_ouvertes.clear();
                            self.etats_vues.clear();
                            self.vue_active = None;
                            self.nomenclature_ouverte = None;
                            self.status = format!("{head} — métadonnées seules ({e})");
                            self.rvt_geom = Err(e);
                            self.rvt_sheet = true;
                        }
                    }
                    self.rvt_preview = None;
                    self.rvt = Some(*info);
                    self.rvt_source = Some(source);
                    self.rvt_sortie = None;
                    return;
                }
                LoadMsg::Natif(Err(e)) | LoadMsg::Revit(Err(e)) | LoadMsg::Done(Err(e)) => {
                    self.loading = None;
                    self.status = "Échec du chargement.".into();
                    self.error = Some(e);
                    return;
                }
            }
        }
    }

    fn install_scene(&mut self, scene: Scene) {
        self.nouveau_document();
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_vertices(gl, &scene.vertices, &scene.couleurs_rendu, &scene.motifs_surface);
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
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.reveal = None;
        self.nomenclature_ouverte = None;
        self.point_clique = None;
        self.filter.clear();
        self.coupe = false;
        self.coupe_dirty = true;
        self.vol = Vol::default();
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
        self.style = StyleVisuel::Graphique;
        self.vues_ouvertes = vec![None];
        self.etats_vues.clear();
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
        // coupe du mode dessin : objets du modèle entièrement du côté retiré écartés (au-dessus
        // d'un plan, devant la ligne d'une coupe)
        let plan = self.plan_coupe();
        let garde = |o: &scene::SceneObject| plan.is_none_or(|p| {
            let n = p.truncate();
            n.x * if n.x > 0.0 { o.max.x } else { o.min.x } + n.y * if n.y > 0.0 { o.max.y } else { o.min.y } + n.z * if n.z > 0.0 { o.max.z } else { o.min.z } + p.w >= 0.0
        });
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
                && (garde(o) || &*o.class == "IFCANNOTATION");
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
        if !self.coupe {
            return None;
        }
        let s = self.scene.as_ref()?;
        // coupe native : plan vertical de la vue ouverte, ramené au repère de la scène
        if let Some(p) = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i)?.plan_coupe) {
            let n = p.truncate();
            return Some(Vec4::new(n.x as f32, n.y as f32, n.z as f32, (p.w + n.dot(s.origin)) as f32));
        }
        let base = self.coupe_niveau.and_then(|n| self.liste_niveaux.iter().find(|x| x.0 == n)).map_or(s.min.z, |x| x.2);
        Some(Vec4::new(0.0, 0.0, -1.0, base + self.coupe_hauteur))
    }

    /// Poché des parties coupées : celui de la plage de vue en Graphique, la couleur des
    /// matériaux en Réaliste.
    fn poche_style(&self) -> Poche {
        if self.style == StyleVisuel::Realiste { Poche::Materiau } else { self.poche }
    }

    /// Recalcule le tracé de la coupe (poché, hachures, contours) s'il le faut.
    fn update_coupe(&mut self) {
        if !self.coupe_dirty {
            return;
        }
        self.coupe_dirty = false;
        // élévation : vue depuis sa ligne, rien n'est coupé (pas de poché)
        let elevation = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)).is_some_and(|v| v.genre == "Élévation" && v.plan_coupe.is_some());
        let (mut tris, mut traits) = match (&self.scene, self.plan_coupe().filter(|_| !elevation)) {
            (Some(s), Some(p)) => {
                let prims = dessin::coupe(s, &self.visible, p, self.echelle, self.poche_style());
                dessin::sommets(s, &prims, p.truncate().as_dvec3().normalize(), self.echelle, self.traits_fins)
            }
            _ => Default::default(),
        };
        if let (Some(s), Some((a, b))) = (&self.scene, self.boite_appliquee()) {
            let poche = (self.style == StyleVisuel::Graphique).then_some(self.poche);
            let (t, l) = dessin::sommets_boite(s, &self.visible, a, b, self.echelle, poche, self.style == StyleVisuel::Realiste, self.traits_fins);
            tris.extend(t);
            traits.extend(l);
        }
        if let Some(s) = &self.scene {
            let prims=self.annotations_natives();
            let (t,l)=dessin::sommets(s,&prims,-self.camera.forward().as_dvec3(),self.echelle,self.traits_fins);
            tris.extend(t); traits.extend(l);
        }
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
            primitives.extend(dessin::coupe(s, &self.visible, p, self.echelle, self.poche_style()));
        }
        primitives.extend(dessin::vu(s, &self.visible, droite, haut, -f, coupe, 4096));
        // éléments 2D de la vue Revit ouverte, s'ils sont affichés (repère du fichier →
        // repère de la scène)
        if let Some(v) = vue {
            for p in v.primitives.iter().filter(|p| p.element.and_then(|e| s.object_of.get(&e)).is_some_and(|&o| self.visible[o as usize])) {
                primitives.push(p.translater(-s.origin));
            }
        }
        primitives.extend(self.annotations_natives());
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

    /// Propriétés de la vue native ouverte : nom, type, échelle, hauteur du plan de
    /// coupe (plan) ou profondeur (coupe, élévation), Retourner, Supprimer.
    fn proprietes_vue_native(&mut self, ui: &mut egui::Ui) {
        use crate::modele::GenreVue;
        let Some((projet, _)) = &self.natif else { return };
        let Some(i) = self.vue_active else { return };
        let Some(numero) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id) else { return };
        let Some(vue) = projet.vues.iter().find(|v| scene::numero_vue(&v.id) == numero).cloned() else { return };
        // nom en cours de saisie, propre à la vue
        if self.nom_vue.as_ref().is_none_or(|(id, _)| id != &vue.id) {
            self.nom_vue = Some((vue.id.clone(), vue.nom.clone()));
        }
        let mut nouvelle = vue.clone();
        let (mut renommer, mut retourner, mut supprimer) = (false, false, false);
        let sorte = match &vue.genre {
            GenreVue::Plan { .. } => "Plan d'étage",
            GenreVue::Coupe { elevation: true, .. } => "Élévation",
            GenreVue::Coupe { .. } => "Coupe",
        };
        egui::CollapsingHeader::new(RichText::new(format!("Vue · {sorte}")).strong()).id_salt("vue_native").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_vue_native").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Nom");
                if let Some((_, nom)) = &mut self.nom_vue {
                    let r = ui.add(egui::TextEdit::singleline(nom).desired_width(150.0));
                    renommer = r.lost_focus() && nom.trim() != vue.nom;
                }
                ui.end_row();
                if !vue.type_vue.is_empty() {
                    ui.label("Type");
                    ui.label(&vue.type_vue);
                    ui.end_row();
                }
                ui.label("Échelle");
                ui.add(egui::DragValue::new(&mut nouvelle.echelle).speed(1.0).range(1.0..=10_000.0).prefix("1 : "));
                ui.end_row();
                match &mut nouvelle.genre {
                    GenreVue::Plan { coupe, .. } => {
                        ui.label("Hauteur de coupe");
                        ui.add(egui::DragValue::new(coupe).speed(10.0).dimensions()).on_hover_text("Plan de coupe au-dessus du niveau");
                        ui.end_row();
                    }
                    GenreVue::Coupe { profondeur, .. } => {
                        ui.label("Profondeur");
                        ui.add(egui::DragValue::new(profondeur).speed(50.0).range(100.0..=200_000.0).dimensions()).on_hover_text("Au-delà, le modèle n'est pas montré");
                        ui.end_row();
                    }
                }
            });
            ui.horizontal(|ui| {
                if matches!(vue.genre, GenreVue::Coupe { .. }) {
                    retourner = ui.button("Retourner").on_hover_text("Regarder de l'autre côté de la ligne").clicked();
                }
                supprimer = ui.button(RichText::new("Supprimer la vue").color(Color32::from_rgb(180, 45, 40))).clicked();
            });
        });
        ui.separator();
        if !(renommer || retourner || supprimer || nouvelle != vue) {
            return;
        }
        let mut p = projet.clone();
        let resultat = if supprimer {
            p.supprimer_vue(&vue.id)
        } else if retourner {
            p.retourner_vue(&vue.id)
        } else if renommer {
            let nom = self.nom_vue.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
            p.renommer_vue(&vue.id, &nom)
        } else {
            *p.vues.iter_mut().find(|v| v.id == vue.id).unwrap() = nouvelle.clone();
            Ok(())
        };
        if let Err(e) = resultat.and_then(|_| self.appliquer_projet(p)) {
            self.error = Some(e);
            self.nom_vue = None;
            return;
        }
        if supprimer {
            self.nom_vue = None;
            self.vue_active = None;
            self.activer_vue(0);
        } else if retourner {
            // la caméra prend la nouvelle direction de la vue (déjà ouverte), puis cadre
            if let Some(d) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.get(i)).map(|v| v.direction.as_vec3().normalize()) {
                self.camera.pitch = d.z.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
                self.camera.yaw = d.y.atan2(d.x);
                self.fit_all();
                self.memoriser_vue();
            }
        } else if let GenreVue::Plan { coupe, .. } = nouvelle.genre {
            self.echelle = nouvelle.echelle;
            self.coupe_hauteur = (coupe / 1000.0) as f32;
            self.coupe_dirty = true;
        } else {
            self.echelle = nouvelle.echelle;
        }
    }

    /// Cadrage de la coupe ou de l'élévation native ouverte (profondeur, côtés), repère
    /// de la scène.
    fn cadrage_vue(&self) -> Vec<Vec4> {
        let Some(s) = &self.scene else { return Vec::new() };
        let Some(v) = self.vue_active.and_then(|i| s.navigateur.as_ref()?.vues.get(i)) else { return Vec::new() };
        v.cadrage.iter().map(|p| {
            let n = p.truncate();
            Vec4::new(n.x as f32, n.y as f32, n.z as f32, (p.w + n.dot(s.origin)) as f32)
        }).collect()
    }

    /// Zone de coupe appliquée au rendu et à la sélection : active, vue 3D.
    fn boite_appliquee(&self) -> Option<(Vec3, Vec3)> {
        if self.boite_active && self.vue_3d() { self.boite } else { None }
    }

    /// Active la zone de coupe ; la première fois, elle entoure tout le modèle.
    fn activer_boite(&mut self, active: bool) {
        self.boite_active = active;
        self.coupe_dirty = true;
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
                    ui.add(egui::DragValue::new(&mut lo).speed(0.05).max_decimals(3).dimensions_metres());
                    ui.add(egui::DragValue::new(&mut hi).speed(0.05).max_decimals(3).dimensions_metres());
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
        self.coupe_dirty = true;
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
        // coupe native : plan vertical de la vue, pas de niveau ni de hauteur
        let verticale = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?.plan_coupe).is_some();
        egui::CollapsingHeader::new(RichText::new("Plage de vue").strong()).id_salt("plage_de_vue").default_open(true).show(ui, |ui| {
            egui::Grid::new("grille_plage_de_vue").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Plan de coupe");
                change |= ui.checkbox(&mut self.coupe, if verticale { "ligne de coupe" } else { "" }).on_hover_text("Parties coupées en poché et trait fort, parties au-dessus retirées").changed();
                ui.end_row();
                if !verticale {
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
                    change |= ui.add(egui::DragValue::new(&mut self.coupe_hauteur).speed(0.01).range(-50.0..=200.0).dimensions_metres()).on_hover_text("Hauteur du plan de coupe au-dessus du niveau").changed();
                });
                ui.end_row();
                }
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
    fn exporter_svg_dialogue(&mut self) {
        {
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
        self.bim.selection.clear();
        self.selection_multiple.clear();
        self.coupe_dirty = true;
        self.selected = id;
        if reveal {
            self.reveal = id;
            if id.is_some() {
                self.left_tab = LeftTab::Tree;
                self.filter.clear();
            }
        }
    }

    /// Propriétés éditables d'un mur natif (mur seul ou couche d'assemblage) ou d'un
    /// assemblage. Retourne le projet modifié s'il a changé.
    /// Sélecteur de type de l'élément natif sélectionné (seul) : mur composé (couches
    /// reconstruites d'après le type), mur seul (types à une couche), couche d'un mur
    /// composé (type du mur composé), élément horizontal, porte ou fenêtre. Projet
    /// modifié si un autre type est choisi.
    fn selecteur_type_selection(&mut self, ui: &mut egui::Ui) -> Option<crate::modele::Projet> {
        use crate::modele::commandes::Commande;
        let ids = self.selection_bim();
        let [id] = ids.as_slice() else { return None };
        let (p, _) = self.natif.as_ref()?;
        // (élément, type actuel, types proposés (id, nom, détail), genre)
        enum Cible { Assemblage(String), Mur(String), Dalle(String), Ouverture(String) }
        let mur = p.murs.iter().find(|m| &m.id == id);
        let (cible, actuel, types): (Cible, String, Vec<(String, String, String)>) = if let Some(a) = p.assemblage(id).or_else(|| mur.and_then(|m| m.assemblage.as_deref()).and_then(|a| p.assemblage(a))) {
            let types = p.types_murs.iter().filter(|t| t.couches.len() > 1).map(|t| (t.id.clone(), t.nom.clone(), format!("{} couches · {}", t.couches.len(), crate::options::unite().afficher(t.epaisseur())))).collect();
            (Cible::Assemblage(a.id.clone()), a.type_mur.clone(), types)
        } else if let Some(m) = mur {
            let types = p.types_murs.iter().filter(|t| t.couches.len() == 1).map(|t| (t.id.clone(), t.nom.clone(), crate::options::unite().afficher(t.epaisseur()))).collect();
            (Cible::Mur(m.id.clone()), m.type_mur.clone(), types)
        } else if let Some(d) = p.dalles.iter().find(|d| &d.id == id) {
            let types = p.types_dalles.iter().map(|t| (t.id.clone(), t.nom.clone(), crate::options::unite().afficher(t.epaisseur()))).collect();
            (Cible::Dalle(d.id.clone()), d.type_dalle.clone(), types)
        } else if let Some(o) = p.ouvertures.iter().find(|o| &o.id == id) {
            // portes et fenêtres : même objet, rangé par catégorie (libre à part)
            use crate::modele::GenreOuverture as G;
            let libre = o.genre == G::Libre;
            let mut types: Vec<_> = p.types_ouvertures.iter().filter(|t| (t.genre == G::Libre) == libre).collect();
            types.sort_by_key(|t| t.genre != G::Porte);
            let types = types.into_iter().map(|t| {
                let categorie = match t.genre { G::Porte => "Porte", G::Fenetre => "Fenêtre", G::Libre => "Libre" };
                (t.id.clone(), t.nom.clone(), format!("{categorie} · {} × {}", crate::options::unite().afficher(t.largeur), crate::options::unite().afficher(t.hauteur)))
            }).collect();
            (Cible::Ouverture(o.id.clone()), o.type_ouverture.clone().unwrap_or_default(), types)
        } else {
            return None;
        };
        let nom_actuel = types.iter().find(|t| t.0 == actuel).map_or_else(|| p.type_mur(&actuel).or_else(|| p.type_dalle(&actuel)).map_or("Type".to_string(), |t| t.nom.clone()), |t| t.1.clone());
        let mut choisi = actuel.clone();
        egui::ComboBox::from_id_salt("selecteur_type_selection")
            .width(ui.available_width() - 8.0)
            .truncate()
            .selected_text(RichText::new(&nom_actuel).strong())
            .show_ui(ui, |ui| {
                for (tid, nom, detail) in &types {
                    ui.selectable_value(&mut choisi, tid.clone(), format!("{nom} — {detail}"));
                }
            })
            .response
            .on_hover_text(match cible {
                Cible::Assemblage(_) => "Type du mur composé : ses couches sont reconstruites d'après le type choisi",
                _ => "Type de l'élément sélectionné",
            });
        ui.separator();
        if choisi == actuel {
            return None;
        }
        let mut q = p.clone();
        match cible {
            Cible::Assemblage(a) => {
                if let Some(x) = q.assemblages.iter_mut().find(|x| x.id == a) {
                    x.type_mur = choisi;
                }
                match q.executer(Commande::ActualiserType { assemblage: a }) {
                    Ok((r, _)) => q = r,
                    Err(e) => {
                        self.error = Some(e);
                        return None;
                    }
                }
            }
            Cible::Mur(m) => q.murs.iter_mut().filter(|x| x.id == m).for_each(|x| x.type_mur = choisi.clone()),
            Cible::Dalle(d) => q.dalles.iter_mut().filter(|x| x.id == d).for_each(|x| x.type_dalle = choisi.clone()),
            Cible::Ouverture(o) => {
                let t = q.types_ouvertures.iter().find(|t| t.id == choisi).cloned();
                if let (Some(x), Some(t)) = (q.ouvertures.iter_mut().find(|x| x.id == o), t) {
                    x.type_ouverture = Some(t.id);
                    (x.largeur, x.hauteur, x.genre) = (t.largeur, t.hauteur, t.genre);
                }
            }
        }
        Some(q)
    }

    fn proprietes_natif(&mut self, ui: &mut egui::Ui, id: Id) -> Option<crate::modele::Projet> {
        use crate::modele::Haut;
        let mut pente_defaut = self.pente_defaut;
        let (projet, _) = self.natif.as_ref()?;
        let mut nouveau: Option<crate::modele::Projet> = None;
        let mut selection: Option<Id> = None;
        if let Some(a) = projet.assemblages.iter().find(|a| scene::id_natif("assemblage", &a.id) == id) {
            ui.label(RichText::new(format!("Mur composé · {}", a.nom)).strong());
            if let Some(t) = projet.type_mur(&a.type_mur) {
                ui.label(RichText::new(&t.nom).weak());
            }
            let couches = projet.couches_assemblage(&a.id);
            let compo: Vec<crate::modele::Couche> = couches.iter().filter_map(|m| projet.type_mur(&m.type_mur).and_then(|t| t.couches.first().cloned())).collect();
            crate::ruban::bande_couches(ui, projet, &compo, 14.0);
            ui.label(RichText::new("Couches, de l'extérieur vers l'intérieur (clic : sélectionner ; Tab parcourt)").small().weak());
            egui::Grid::new("couches_assemblage").num_columns(4).striped(true).spacing([8.0, 3.0]).show(ui, |ui| {
                for m in &couches {
                    let c = projet.type_mur(&m.type_mur).and_then(|t| t.couches.first());
                    let nom = c.and_then(|c| c.materiau.clone()).unwrap_or_else(|| m.nom.clone());
                    if ui.link(nom).clicked() {
                        selection = Some(scene::id_natif("mur", &m.id));
                    }
                    ui.label(crate::options::unite().afficher(c.map_or(0.0, |c| c.epaisseur)));
                    ui.label(projet.bas_haut(m).map_or(String::new(), |(b, h)| format!("h {:.0}", h - b)));
                    ui.label(RichText::new(m.phase.as_deref().unwrap_or("")).weak());
                    ui.end_row();
                }
            });
            ui.separator();
        } else if let Some(mur) = projet.murs.iter().find(|m| scene::id_natif("mur", &m.id) == id) {
            let mut m = mur.clone();
            let epaisseur = projet.type_mur(&m.type_mur).map_or(0.0, |t| t.epaisseur());
            let materiau = projet.type_mur(&m.type_mur).and_then(|t| (t.couches.len() == 1).then(|| t.couches[0].materiau.clone()).flatten());
            let assemblage = m.assemblage.as_ref().and_then(|a| projet.assemblage(a));
            ui.label(RichText::new(match (assemblage, &materiau) {
                (Some(a), Some(x)) => format!("Couche · {x} · de {}", a.nom),
                (Some(a), None) => format!("Couche de {}", a.nom),
                (None, _) => m.nom.clone(),
            }).strong());
            let liste = |ui: &mut egui::Ui, sel: &mut Option<String>, choix: &[String], sale: &str, vide: &str| {
                let mut change = false;
                egui::ComboBox::from_id_salt(sale).width(150.0).selected_text(sel.as_deref().unwrap_or(vide)).show_ui(ui, |ui| {
                    for c in choix {
                        change |= ui.selectable_value(sel, Some(c.clone()), c).changed();
                    }
                });
                change
            };
            let mut change = false;
            egui::Grid::new("mur_natif_proprietes").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let niveaux: Vec<(String, String)> = projet.niveaux.iter().map(|n| (n.id.clone(), n.nom.clone())).collect();
                let nom_niveau = |id: &str| niveaux.iter().find(|n| n.0 == id).map_or(id.to_string(), |n| n.1.clone());
                ui.label("Niveau de base");
                egui::ComboBox::from_id_salt("mur_niveau").width(150.0).selected_text(nom_niveau(&m.niveau)).show_ui(ui, |ui| {
                    for (nid, nom) in &niveaux {
                        change |= ui.selectable_value(&mut m.niveau, nid.clone(), nom).changed();
                    }
                });
                ui.end_row();
                ui.label("Décalage bas");
                change |= ui.add(egui::DragValue::new(&mut m.decalage_bas).speed(10.0).dimensions()).changed();
                ui.end_row();
                ui.label("Contrainte haute");
                let libre = matches!(m.haut, Haut::Libre { .. });
                let texte = match &m.haut { Haut::Libre { .. } => "Libre (hauteur)".to_string(), Haut::Niveau { niveau, .. } => nom_niveau(niveau) };
                let (bas, haut) = projet.bas_haut(&m).unwrap_or((0.0, 2800.0));
                egui::ComboBox::from_id_salt("mur_haut").width(150.0).selected_text(texte).show_ui(ui, |ui| {
                    if ui.selectable_label(libre, "Libre (hauteur)").clicked() && !libre {
                        m.haut = Haut::Libre { hauteur: haut - bas };
                        change = true;
                    }
                    for (nid, nom) in &niveaux {
                        let actif = matches!(&m.haut, Haut::Niveau { niveau, .. } if niveau == nid);
                        if ui.selectable_label(actif, nom).clicked() && !actif {
                            m.haut = Haut::Niveau { niveau: nid.clone(), decalage: 0.0 };
                            change = true;
                        }
                    }
                });
                ui.end_row();
                match &mut m.haut {
                    Haut::Libre { hauteur } => {
                        ui.label("Hauteur");
                        change |= ui.add(egui::DragValue::new(hauteur).speed(10.0).range(1.0..=f64::MAX).dimensions()).changed();
                    }
                    Haut::Niveau { decalage, .. } => {
                        ui.label("Décalage haut");
                        change |= ui.add(egui::DragValue::new(decalage).speed(10.0).dimensions()).changed();
                    }
                }
                ui.end_row();
                ui.label("Phase");
                change |= liste(ui, &mut m.phase, &projet.phases, "mur_phase", "Non renseignée");
                ui.end_row();
                ui.label("Sous-projet");
                change |= liste(ui, &mut m.sous_projet, &projet.sous_projets, "mur_sous_projet", "Non renseigné");
                ui.end_row();
                ui.label("Variante");
                ui.label(m.variante.as_ref().and_then(|id| projet.variantes.iter().find(|v| &v.id == id)).map_or_else(|| "Principale".into(), |v| v.libelle()));
                ui.end_row();
                for (nom, valeur) in [
                    ("Longueur", crate::options::unite().afficher(m.axe.longueur())),
                    ("Hauteur résultante", crate::options::unite().afficher(haut - bas)),
                    ("Épaisseur", crate::options::unite().afficher(epaisseur)),
                    ("Volume", format!("{:.3} m³", projet.volume_mur(&m).unwrap_or(0.0) / 1e9)),
                ] {
                    ui.label(nom);
                    ui.label(valeur);
                    ui.end_row();
                }
            });
            if change {
                let mut p = projet.clone();
                if let Some(x) = p.murs.iter_mut().find(|x| x.id == m.id) {
                    *x = m.clone();
                }
                match p.bas_haut(&m) {
                    Some((b, h)) if h > b => nouveau = Some(p),
                    _ => self.error = Some("La limite haute du mur doit être au-dessus de sa limite basse.".into()),
                }
            }
            ui.separator();
        } else if let Some(e) = projet.importes.iter().find(|e| scene::id_natif("importe", &e.id)==id) {
            ui.heading(&e.nom); ui.label(&e.categorie); ui.label("Géométrie importée"); ui.label(&e.raison);
            if let Some(n)=e.niveau.as_ref().and_then(|n|projet.niveau(n)) { ui.label(format!("Niveau : {}",n.nom)); }
            if let Some(o)=&e.origine { ui.label(format!("Source : {} · {}",o.source,o.id)); if let Some(g)=&o.guid {ui.label(g);} }
        } else if let Some(o) = projet.ouvertures.iter().find(|o| scene::id_natif("ouverture", &o.id)==id) {
            let mut o=o.clone(); let mut change=false;
            ui.heading(&o.nom);
            for (nom,v) in [("Abscisse",&mut o.abscisse),("Largeur",&mut o.largeur),("Hauteur",&mut o.hauteur),("Allège",&mut o.allege)] {
                ui.horizontal(|ui| { ui.label(nom); change|=ui.add_enabled(o.representation.is_none() || matches!(nom,"Abscisse"|"Allège"),egui::DragValue::new(v).dimensions().speed(10.0)).changed(); });
            }
            // paramètres d'occurrence de la famille : propres à cette ouverture
            if let Some(f) = o.type_ouverture.as_ref().and_then(|t| projet.types_ouvertures.iter().find(|x| &x.id == t)).and_then(|t| t.modele.as_ref()) {
                use crate::modele::famille_modelee::{GenreParametre, Portee};
                let occurrences: Vec<&crate::modele::famille_modelee::Parametre> = f.parametres.iter().filter(|p| p.portee == Portee::Occurrence).collect();
                if !occurrences.is_empty() {
                    let e = crate::modele::solides::hote(projet, &o.mur).map_or(200.0, |h| h.1);
                    let mut surcharges = projet.parametres_partages.clone();
                    surcharges.extend(o.valeurs.iter().map(|(k, v)| (k.clone(), *v)));
                    let r = f.resoudre_occurrence(o.largeur, o.hauteur, e, &surcharges, &o.materiaux);
                    ui.label(RichText::new("Paramètres d'occurrence").strong());
                    for p in occurrences {
                        let nom = p.nom.as_str();
                        ui.horizontal(|ui| {
                            ui.label(nom);
                            match p.genre {
                                GenreParametre::OuiNon => {
                                    let mut oui = r.valeurs.get(nom).is_none_or(|v| v.abs() > 1e-9);
                                    if ui.checkbox(&mut oui, "").changed() {
                                        o.valeurs.insert(nom.to_string(), if oui { 1.0 } else { 0.0 });
                                        change = true;
                                    }
                                }
                                GenreParametre::Materiau => {
                                    let mut m = r.materiaux.get(nom).cloned().unwrap_or_default();
                                    let avant = m.clone();
                                    egui::ComboBox::from_id_salt(("materiau_occurrence", nom)).width(140.0).selected_text(if m.is_empty() { "(rôle)" } else { m.as_str() }).show_ui(ui, |ui| {
                                        ui.selectable_value(&mut m, String::new(), "(couleur du rôle)");
                                        for x in &projet.materiaux {
                                            ui.selectable_value(&mut m, x.nom.clone(), &x.nom);
                                        }
                                    });
                                    if m != avant {
                                        o.materiaux.insert(nom.to_string(), m);
                                        change = true;
                                    }
                                }
                                GenreParametre::Longueur => {
                                    let mut v = r.valeurs.get(nom).copied().unwrap_or(0.0);
                                    if ui.add(egui::DragValue::new(&mut v).dimensions().speed(5.0)).changed() {
                                        o.valeurs.insert(nom.to_string(), v);
                                        change = true;
                                    }
                                }
                            }
                            if (o.valeurs.contains_key(nom) || o.materiaux.contains_key(nom)) && ui.small_button("Défaut").on_hover_text("Reprendre la valeur de la famille").clicked() {
                                o.valeurs.remove(nom);
                                o.materiaux.remove(nom);
                                change = true;
                            }
                        });
                    }
                }
            }
            if o.type_ouverture.as_ref().and_then(|t| projet.types_ouvertures.iter().find(|x| &x.id == t)).is_some_and(|t| t.modele.is_some()) {
                ui.horizontal(|ui| {
                    change |= ui.checkbox(&mut o.retournee, "Retourner").on_hover_text("Échange intérieur et extérieur : sens d'ouverture").changed();
                    change |= ui.checkbox(&mut o.inversee, "Inverser").on_hover_text("Échange gauche et droite : main de la porte").changed();
                });
            }
            if o.representation.is_some() && ui.button("Remplacer la menuiserie importée par un cadre paramétrique").clicked() {o.representation=None;change=true;}
            if change {
                if o.representation.is_some() { ui.label("La menuiserie importée est conservée ; sa forme n'est pas paramétrique."); }
                let mut p=projet.clone(); *p.ouvertures.iter_mut().find(|x|x.id==o.id).unwrap()=o.clone(); nouveau=Some(p);
            }
        } else {
            let dalle = projet.dalles.iter().find(|d| scene::id_natif("dalle", &d.id) == id)?;
            let mut d = dalle.clone();
            let t = projet.type_dalle(&d.type_dalle);
            ui.label(RichText::new(format!("{} · {}", d.sorte.nom(), t.map_or("", |t| t.nom.as_str()))).strong());
            if let Some(t) = t {
                for c in &t.couches {
                    ui.horizontal(|ui| {
                        let [r, g, b] = c.materiau.as_ref().and_then(|n| projet.materiau(n)).map_or([200, 200, 196], |m| m.couleur);
                        let (q, _) = ui.allocate_exact_size(egui::vec2(16.0, 12.0), Sense::hover());
                        ui.painter().rect_filled(q, 0.0, Color32::from_rgb(r, g, b));
                        ui.label(RichText::new(format!("{} · {}", c.materiau.as_deref().unwrap_or("Couche"), crate::options::unite().afficher(c.epaisseur))).small());
                    });
                }
            }
            let mut change = false;
            egui::Grid::new("dalle_natif_proprietes").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Niveau");
                let nom = projet.niveau(&d.niveau).map_or(d.niveau.clone(), |n| n.nom.clone());
                egui::ComboBox::from_id_salt("dalle_niveau").width(150.0).selected_text(nom).show_ui(ui, |ui| {
                    for n in &projet.niveaux {
                        change |= ui.selectable_value(&mut d.niveau, n.id.clone(), &n.nom).changed();
                    }
                });
                ui.end_row();
                ui.label("Sorte");
                egui::ComboBox::from_id_salt("dalle_sorte").width(150.0).selected_text(d.sorte.nom()).show_ui(ui, |ui| {
                    for s in crate::modele::SorteDalle::TOUTES {
                        change |= ui.selectable_value(&mut d.sorte, s, s.nom()).changed();
                    }
                })
                .response
                .on_hover_text("Même élément, nom et référence d'altitude différents (dessus ou sous-face)");
                ui.end_row();
                ui.label(match d.sorte {
                    crate::modele::SorteDalle::Plafond => "Hauteur sous plafond",
                    crate::modele::SorteDalle::Toiture => "Décalage (sous-face)",
                    _ => "Décalage (dessus)",
                });
                change |= ui.add(egui::DragValue::new(&mut d.decalage).speed(10.0).dimensions()).changed();
                ui.end_row();
                for (nom, valeur, choix, sale) in [("Phase", &mut d.phase, &projet.phases, "dalle_phase"), ("Sous-projet", &mut d.sous_projet, &projet.sous_projets, "dalle_sous_projet")] {
                    ui.label(nom);
                    egui::ComboBox::from_id_salt(sale).width(150.0).selected_text(valeur.as_deref().unwrap_or("Non renseigné")).show_ui(ui, |ui| {
                        for c in choix {
                            change |= ui.selectable_value(valeur, Some(c.clone()), c).changed();
                        }
                    });
                    ui.end_row();
                }
                let aire = crate::modele::geometrie::aire_pair_impair(&d.contours);
                for (nom, valeur) in [
                    ("Aire", format!("{:.2} m²", aire / 1e6)),
                    ("Épaisseur", crate::options::unite().afficher(t.map_or(0.0, |t| t.epaisseur()))),
                    ("Volume", format!("{:.3} m³", projet.volume_dalle(&d).unwrap_or(0.0) / 1e9)),
                ] {
                    ui.label(nom);
                    ui.label(valeur);
                    ui.end_row();
                }
            });
            if d.sorte.pente_possible() {
                ui.add_space(4.0);
                ui.label(RichText::new("Arêtes").strong())
                    .on_hover_text("Une arête cochée définit une pente : le versant monte depuis elle vers l'intérieur. Aussi en cliquant ses repères dans la vue.");
                egui::Grid::new("dalle_aretes").num_columns(3).striped(true).spacing([8.0, 3.0]).show(ui, |ui| {
                    for c in 0..d.contours.len() {
                        let n = d.contours[c].len();
                        for k in 0..n {
                            let (a, b) = (d.contours[c][k], d.contours[c][(k + 1) % n]);
                            let nom = if d.contours.len() > 1 { format!("{}.{}", c + 1, k + 1) } else { format!("{}", k + 1) };
                            ui.label(format!("Arête {nom} · {}", crate::options::unite().afficher((b[0] - a[0]).hypot(b[1] - a[1]))));
                            let mut pente = d.pente(c, k);
                            let mut oui = pente.is_some();
                            if ui.checkbox(&mut oui, "Pente").changed() {
                                pente = oui.then_some(pente_defaut);
                            }
                            match &mut pente {
                                Some(a) => {
                                    ui.add(egui::DragValue::new(a).speed(0.5).range(0.5..=85.0).max_decimals(1).suffix(" °"));
                                }
                                None => {
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                            if pente != d.pente(c, k) {
                                if let Some(a) = pente { pente_defaut = a; }
                                d.definir_pente(c, k, pente);
                                change = true;
                            }
                        }
                    }
                });
            }
            if change {
                let mut p = projet.clone();
                if let Some(x) = p.dalles.iter_mut().find(|x| x.id == d.id) {
                    *x = d;
                }
                nouveau = Some(p);
            }
            ui.separator();
        }
        self.pente_defaut = pente_defaut;
        if let Some(sel) = selection {
            self.select(Some(sel), true);
        }
        nouveau
    }

    /// Élément sélectionné par un clic sur l'objet `id` : une couche d'assemblage
    /// sélectionne son assemblage, sauf si l'assemblage (ou l'une de ses couches) l'est
    /// déjà : le clic suivant entre dans l'assemblage et prend la couche.
    fn cible_clic(&self, id: Id) -> Id {
        let Some(s) = &self.scene else { return id };
        let Some(&parent) = s.model.parent.get(&id) else { return id };
        if s.model.ty(parent) != "IFCELEMENTASSEMBLY" {
            return id;
        }
        let dedans = self.selected.is_some_and(|sel| sel == parent || s.model.parent.get(&sel) == Some(&parent));
        if dedans { id } else { parent }
    }

    /// Tab : de l'assemblage sélectionné à sa première couche, puis couche suivante,
    /// puis retour à l'assemblage.
    fn tab_selection(&mut self) {
        let Some(s) = &self.scene else { return };
        let Some(sel) = self.selected else { return };
        let (assemblage, courant) = if s.model.ty(sel) == "IFCELEMENTASSEMBLY" {
            (sel, None)
        } else {
            match s.model.parent.get(&sel) {
                Some(&p) if s.model.ty(p) == "IFCELEMENTASSEMBLY" => (p, Some(sel)),
                _ => return,
            }
        };
        let couches = s.model.children.get(&assemblage).cloned().unwrap_or_default();
        let suivant = match courant {
            None => couches.first().copied(),
            Some(c) => couches.iter().position(|&x| x == c).and_then(|k| couches.get(k + 1).copied()),
        };
        self.select(Some(suivant.unwrap_or(assemblage)), true);
    }

    /// Sélectionne les objets de la scène, y compris les couches des assemblages.
    fn objets_selectionnes(&self) -> Vec<u8> {
        let Some(scene) = &self.scene else { return Vec::new() };
        let mut marques = vec![0; scene.objects.len()];
        let mut entites = Vec::new();
        if !self.bim.selection.is_empty() {
            let selection: HashSet<&str> = self.bim.selection.iter().map(String::as_str).collect();
            if let Some((p, _)) = &self.natif {
                for (genre, cle) in p.murs.iter().map(|x| ("mur", &x.id))
                    .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
                    .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
                    .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                    .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
                    .chain(p.importes.iter().map(|x| ("importe", &x.id))) {
                    if selection.contains(cle.as_str()) {
                        entites.push(scene::id_natif(genre, cle));
                    }
                }
            }
        } else if !self.selection_multiple.is_empty() {
            entites.extend(self.selection_multiple.iter().copied());
        } else if let Some(id) = self.selected {
            entites.push(id);
        }
        for id in entites {
            let objets = if scene.model.ty(id) == "IFCELEMENTASSEMBLY" {
                self.objects_under(id)
            } else {
                scene.object_of.get(&id).copied().into_iter().collect()
            };
            for objet in objets {
                if let Some(marque) = marques.get_mut(objet as usize) { *marque = 255; }
            }
        }
        marques
    }

    fn selectionner_rectangle(&mut self, vue: egui::Rect, fenetre: FenetreSelection) {
        let Some(scene) = &self.scene else { return };
        let rectangle = egui::Rect::from_two_pos(fenetre.debut, fenetre.fin);
        let croisement = fenetre.fin.x < fenetre.debut.x;
        let resultats = crate::selection_rectangle::objets(scene, &self.visible, self.camera.view_proj(vue.width() / vue.height().max(1.0)), vue, rectangle, self.plan_coupe(), self.boite_appliquee(), &self.cadrage_vue());
        let mut ids = crate::selection_rectangle::groupes(scene.objects.iter().zip(resultats).map(|(o, r)| {
            let id = scene.model.parent.get(&o.id).filter(|id| scene.model.ty(**id) == "IFCELEMENTASSEMBLY").copied().unwrap_or(o.id);
            (id, r)
        }), croisement);
        ids.sort_unstable();
        if fenetre.ajouter {
            let mut ancienne = self.selection_multiple.clone();
            if ancienne.is_empty() {
                if !self.bim.selection.is_empty() {
                    if let Some((p, _)) = &self.natif {
                        let cles: HashSet<&str> = self.bim.selection.iter().map(String::as_str).collect();
                        ancienne.extend(p.murs.iter().map(|x| ("mur", &x.id))
                            .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
                            .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
                            .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                            .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
                            .chain(p.importes.iter().map(|x| ("importe", &x.id)))
                            .filter(|(_, id)| cles.contains(id.as_str())).map(|(genre, id)| scene::id_natif(genre, id)));
                    }
                } else if let Some(id) = self.selected { ancienne.push(id); }
            }
            ancienne.extend(ids); ancienne.sort_unstable(); ancienne.dedup(); ids = ancienne;
        }
        let mut vues = if fenetre.ajouter { self.bim.selection.clone() } else { Vec::new() };
        if self.vue_en_plan() {
            if let Some((p, _)) = &self.natif {
                let z = self.altitude_vue_native();
                for v in &p.vues {
                    if let crate::modele::GenreVue::Coupe { debut, fin, .. } = &v.genre {
                        if let (Some(a), Some(b)) = (self.ecran_plan(vue, *debut, z), self.ecran_plan(vue, *fin, z)) {
                            if crate::selection_rectangle::segment(rectangle, a, b, croisement) { vues.push(v.id.clone()); }
                        }
                    }
                }
                for g in &p.quadrillages {
                    if let (Some(a), Some(b)) = (self.ecran_plan(vue, g.debut, z), self.ecran_plan(vue, g.fin, z)) {
                        if crate::selection_rectangle::segment(rectangle, a, b, croisement) { vues.push(g.id.clone()); }
                    }
                }
            }
        }
        if let Some((p, _)) = &self.natif {
            vues.retain(|id| p.vues.iter().any(|v| v.id == *id) || p.quadrillages.iter().any(|g| g.id == *id) || p.annotations.iter().any(|a| a.id == *id));
        }
        for (id, bounds) in self.rectangles_annotations(vue) {
            if if croisement { rectangle.intersects(bounds) } else { rectangle.contains_rect(bounds) } { vues.push(id); }
        }
        vues.sort(); vues.dedup();
        self.selected = ids.first().copied();
        let recherches: HashSet<Id> = ids.iter().copied().collect();
        self.bim.selection = self.natif.as_ref().map(|(p, _)| {
            p.murs.iter().map(|x| ("mur", &x.id))
                .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
                .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
                .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
                .chain(p.importes.iter().map(|x| ("importe", &x.id)))
                .filter(|(genre, id)| recherches.contains(&scene::id_natif(genre, id)))
                .map(|(_, id)| id.clone()).collect()
        }).unwrap_or_default();
        self.bim.selection.extend(vues);
        self.coupe_dirty = true;
        self.selection_multiple = ids;
        self.status = format!("{} élément(s) sélectionné(s).", if self.natif.is_some() { self.bim.selection.len() } else { self.selection_multiple.len() });
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

    fn creer_projet(&mut self) {
        let Some(mut chemin) = rfd::FileDialog::new().set_title("Nouveau projet natif v0")
            .add_filter("Modèle natif v0", &["json"]).set_file_name("Nouveau projet.modele.json").save_file() else { return };
        if chemin.extension().is_none() {
            chemin.set_extension("modele.json");
        }
        let nom = chemin.file_stem().map(|n| n.to_string_lossy().trim_end_matches(".modele").to_string()).unwrap_or_else(|| "Nouveau projet".into());
        let projet = crate::modele::Projet::depuis_gabarit(&nom, &crate::gabarit::actif());
        let resultat = scene::depuis_natif(&projet, &chemin).and_then(|scene| {
            crate::modele::enregistrement::enregistrer_natif(&projet, &chemin)?;
            Ok(scene)
        });
        match resultat {
            Ok(scene) => {
                self.install_scene(scene);
                self.status = format!("{} — nouveau projet natif v0", projet.nom);
                let _ = self.historique.enregistrer(&projet);
                self.natif = Some((projet, chemin));
                self.avant_saisie = None;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn enregistrer_projet(&mut self, ctx: &egui::Context) {
        if self.loading.is_some() || self.sauvegarde.is_some() { return; }
        if self.natif.is_none() {
            self.enregistrer_rvt_dialogue(ctx);
            return;
        }
        let Some((projet, chemin)) = &self.natif else { return };
        match crate::modele::enregistrement::enregistrer_natif(projet, chemin) {
            Ok(()) => {
                self.natif_modifie = false;
                let _ = self.historique.enregistrer(projet);
                crate::modele::enregistrement::effacer_recuperation(chemin);
                self.status = format!("{} — enregistré", chemin.display());
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn confirmation_modifications(&mut self, ctx: &egui::Context) {
        let Some((application, document)) = self.confirmer_fermeture else { return };
        let mut action = None;
        egui::Modal::new(egui::Id::unique("modifications_non_enregistrees")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading("Enregistrer les modifications ?");
            ui.label(if application { "Des projets contiennent des modifications non enregistrées." } else { "Ce projet contient des modifications non enregistrées." });
            if let Some(e) = &self.error { ui.colored_label(Color32::from_rgb(180,45,40), e); }
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() { action = Some(0); }
                if ui.button("Fermer sans enregistrer").clicked() { action = Some(1); }
                if ui.button("Annuler").clicked() { action = Some(2); }
            });
        });
        let Some(action) = action else { return };
        if action == 2 { self.confirmer_fermeture = None; return; }
        if !application { self.changer_document(document); }
        if action == 0 {
            if self.natif_modifie {
                self.enregistrer_projet(&ctx);
                if self.natif_modifie { return; }
            }
            if application {
                for d in self.documents.values_mut().filter(|d| d.modifie) {
                    if let Some((p, chemin)) = &d.natif {
                        if let Err(e) = crate::modele::enregistrement::enregistrer_natif(p, chemin) {
                            self.error = Some(format!("Enregistrement impossible de {} : {e}", chemin.display()));
                            return;
                        }
                    }
                    if let Some((p, chemin)) = &d.natif { let _ = d.historique.enregistrer(p); crate::modele::enregistrement::effacer_recuperation(chemin); }
                    d.modifie = false;
                }
            }
        }
        self.confirmer_fermeture = None;
        if action==1 {
            if let Some((_,p))=&self.natif {crate::modele::enregistrement::effacer_recuperation(p);}
            if application {for d in self.documents.values() {if let Some((_,p))=&d.natif {crate::modele::enregistrement::effacer_recuperation(p);}}}
        }
        if application {
            self.natif_modifie = false;
            for d in self.documents.values_mut() { d.modifie = false; }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else { self.fermer_document(); }
    }

    fn enregistrer_rvt_dialogue(&mut self, ctx: &egui::Context) {
        let Some(source) = &self.rvt_source else { return };
        let sortie = match &self.rvt_sortie {
            Some(p) => p.clone(),
            None => {
                let nom = source.chemin.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Projet".into());
                let dialogue = rfd::FileDialog::new().set_title("Enregistrer le projet Revit en modèle natif v0")
                    .add_filter("Modèle natif v0", &["json"]).set_file_name(format!("{nom}.modele.json"));
                let Some(mut p) = dialogue.save_file() else { return };
                if p.extension().is_none() { p.set_extension("modele.json"); }
                p
            }
        };
        if let Err(e) = self.demarrer_enregistrement_rvt(ctx,sortie) { self.error = Some(e); }
    }

    fn demarrer_enregistrement_rvt(&mut self, ctx: &egui::Context, sortie: PathBuf) -> Result<(), String> {
        if self.sauvegarde.is_some() { return Err("Un enregistrement est déjà en cours.".into()); }
        if self.loading.is_some() { return Err("Attendre la fin du chargement avant d’enregistrer.".into()); }
        let source = self.rvt_source.clone().ok_or("Aucun fichier Revit ouvert.")?;
        let (tx, rx) = channel();
        let contexte = ctx.clone();
        std::thread::Builder::new().name("enregistrer-v0".into()).stack_size(64 << 20).spawn(move || {
            let progress = |p| { let _ = tx.send(SaveMsg::Progress(p)); contexte.request_repaint(); };
            let resultat = crate::modele::enregistrement::enregistrer_rvt(&source,&sortie,&progress);
            let _ = tx.send(SaveMsg::Done(resultat));
            contexte.request_repaint();
        }).map_err(|e| format!("Démarrage de l’enregistrement impossible : {e}"))?;
        self.sauvegarde = Some(Sauvegarde { document: self.document_actif, progression: "Préparation de l’enregistrement v0…".into(), messages: rx });
        self.error = None;
        Ok(())
    }

    fn poll_sauvegarde(&mut self) {
        loop {
            let Some(tache) = &mut self.sauvegarde else { return };
            let message = match tache.messages.try_recv() {
                Ok(m) => m,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => SaveMsg::Done(Err("L’enregistrement a été interrompu.".into())),
            };
            match message {
                SaveMsg::Progress(p) => tache.progression = p,
                SaveMsg::Done(resultat) => {
                    let document = self.sauvegarde.take().unwrap().document;
                    match resultat {
                        Ok(bilan) => {
                            let statut = format!("{} — enregistré en v0 : {} murs paramétriques, {} éléments importés",bilan.chemin.display(),bilan.murs,bilan.importes);
                            if document == self.document_actif {
                                self.rvt_sortie = Some(bilan.chemin.clone());
                                self.status = statut;
                            } else if let Some(d) = self.documents.get_mut(&document) {
                                d.rvt_sortie = Some(bilan.chemin.clone()); d.status = statut;
                            } else { self.status = statut; }
                            if let Err(e) = bilan.rapport { self.error = Some(format!("Le projet v0 est enregistré, mais son rapport n’a pas pu être écrit : {e}")); }
                        }
                        Err(e) => self.error = Some(e),
                    }
                    return;
                }
            }
        }
    }

    fn ajouter_mur_natif(&mut self, debut: [f64; 2], fin: [f64; 2], options: &crate::modele::edition::ParametresMur) -> Result<String, String> {
        let (projet, _) = self.natif.as_ref().ok_or("Créez ou ouvrez un projet natif pour dessiner un mur.")?;
        let mut projet = projet.clone();
        let id = projet.ajouter_mur(debut, fin, options)?;
        self.appliquer_projet(projet)?;
        // assemblage (type composé) ou mur seul
        let genre = if self.natif.as_ref().is_some_and(|(p, _)| p.assemblage(&id).is_some()) { "assemblage" } else { "mur" };
        self.selected = Some(scene::id_natif(genre, &id));
        self.reveal = self.selected;
        self.status = "Mur créé — Ctrl+S pour enregistrer.".into();
        Ok(id)
    }

    /// Ajoute une vue au projet natif (`ajout` rend son identifiant) et l'ouvre si
    /// `ouvrir` ; elle apparaît dans l'arborescence.
    fn creer_vue(&mut self, ajout: impl FnOnce(&mut crate::modele::Projet) -> Result<String, String>, ouvrir: bool) {
        let Some((projet, _)) = &self.natif else { return };
        let mut projet = projet.clone();
        let id = match ajout(&mut projet) {
            Ok(id) => id,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let nom = projet.vues.iter().find(|v| v.id == id).map(|v| v.nom.clone()).unwrap_or_default();
        if let Err(e) = self.appliquer_projet(projet) {
            self.error = Some(e);
            return;
        }
        self.status = format!("Vue « {nom} » créée — Ctrl+S pour enregistrer.");
        let numero = scene::numero_vue(&id);
        let indice = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()?.vues.iter().position(|v| v.revit_id == numero));
        if ouvrir && let Some(i) = indice {
            self.activer_vue(i);
        }
    }

    /// Crée une dalle de contour `pts` (mm) dans le projet natif et la sélectionne.
    fn creer_dalle(&mut self, pts: &[[f64; 2]], parametres: &crate::modele::edition::ParametresDalle) {
        let Some((projet, _)) = &self.natif else { return };
        let mut projet = projet.clone();
        match projet.ajouter_dalle(pts, parametres).and_then(|id| self.appliquer_projet(projet).map(|_| id)) {
            Ok(id) => {
                self.selected = Some(scene::id_natif("dalle", &id));
                self.reveal = self.selected;
                self.status = "Dalle créée — Ctrl+S pour enregistrer.".into();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Altitude (mm) du plan de tracé d'une vue native : niveau du plan ouvert, sinon le
    /// premier niveau.
    fn altitude_vue_native(&self) -> f64 {
        let Some((projet, _)) = &self.natif else { return 0.0 };
        let etage = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)?.etage);
        etage
            .and_then(|e| projet.niveaux.iter().find(|n| scene::id_natif("niveau", &n.id) == e))
            .or(projet.niveaux.first())
            .map_or(0.0, |n| n.altitude)
    }

    /// Projection écran d'un point du plan (mm) à l'altitude `z` (mm).
    fn ecran_plan(&self, rect: egui::Rect, p: [f64; 2], z: f64) -> Option<egui::Pos2> {
        let s = self.scene.as_ref()?;
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let q = (glam::DVec3::new(p[0], p[1], z) * 0.001 - s.origin).as_vec3();
        let c = vp * q.extend(1.0);
        (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
    }

    /// Arêtes de l'élément horizontal sélectionné qui peut porter une pente (plafond,
    /// toiture) : contour, rang, milieu et direction vers l'intérieur à l'écran, pente.
    fn aretes_pente(&self, rect: egui::Rect) -> Vec<(usize, usize, egui::Pos2, egui::Vec2, Option<f64>)> {
        use crate::modele::pente;
        let (Some((projet, _)), Some(sel)) = (&self.natif, self.selected) else { return Vec::new() };
        let Some(d) = projet.dalles.iter().find(|d| scene::id_natif("dalle", &d.id) == sel && d.sorte.pente_possible()) else { return Vec::new() };
        let z0 = projet.niveau(&d.niveau).map_or(0.0, |n| n.altitude) + d.decalage;
        let plans = pente::plans(d);
        let mut v = Vec::new();
        for (c, contour) in d.contours.iter().enumerate() {
            let n = contour.len();
            for k in 0..n {
                let (a, b) = (glam::DVec2::from_array(contour[k]), glam::DVec2::from_array(contour[(k + 1) % n]));
                let m = (a + b) * 0.5;
                let z = z0 + pente::hauteur(&plans, 0.0, m);
                let (Some(i), Some(q)) = (pente::vers_interieur(&d.contours, c, k), self.ecran_plan(rect, m.to_array(), z)) else { continue };
                let Some(qi) = self.ecran_plan(rect, (m + i * 300.0).to_array(), z) else { continue };
                let dir = (qi - q).normalized();
                v.push((c, k, q, if dir.is_finite() { dir } else { egui::Vec2::ZERO }, d.pente(c, k)));
            }
        }
        v
    }

    /// Repères des arêtes : triangle plein et pente pour une arête qui la définit, rond
    /// creux sinon ; un clic bascule.
    fn dessiner_aretes_pente(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        let aretes = self.aretes_pente(rect);
        if aretes.is_empty() { return; }
        let painter = ui.painter().with_clip_rect(rect);
        let accent = Color32::from_rgb(230, 120, 20);
        let gris = Color32::from_gray(90);
        for (_, _, q, dir, pente) in aretes {
            let survole = survol.is_some_and(|s| s.distance(q) <= 9.0);
            match pente {
                Some(a) => {
                    // triangle pointé vers le haut du versant
                    let (n, t) = (dir, egui::vec2(-dir.y, dir.x));
                    let pts = vec![q + n * 9.0, q - n * 4.0 + t * 6.0, q - n * 4.0 - t * 6.0];
                    painter.add(egui::Shape::convex_polygon(pts, accent, egui::Stroke::new(if survole { 2.0 } else { 1.0 }, Color32::WHITE)));
                    let texte = format!("{}°", crate::step::format_real((a * 10.0).round() / 10.0));
                    let g = painter.layout_no_wrap(texte, egui::FontId::proportional(12.0), Color32::from_gray(30));
                    let cadre = egui::Rect::from_center_size(q - n * 18.0, g.size() + egui::vec2(8.0, 4.0));
                    painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230), egui::Stroke::new(1.0, accent), egui::StrokeKind::Inside);
                    painter.galley(cadre.min + egui::vec2(4.0, 2.0), g, Color32::from_gray(30));
                }
                None => {
                    painter.circle(q, 5.0, Color32::from_rgba_unmultiplied(255, 255, 255, 220), egui::Stroke::new(if survole { 2.0 } else { 1.2 }, gris));
                }
            }
            if survole {
                let aide = if pente.is_some() { "Clic : arête sans pente" } else { "Clic : l'arête définit la pente" };
                painter.text(q + egui::vec2(12.0, -12.0), egui::Align2::LEFT_BOTTOM, aide, egui::FontId::proportional(12.0), Color32::from_gray(30));
            }
        }
    }

    /// Mur ou assemblage natif sélectionné : identifiant, axe (mm) et altitude de la base.
    fn axe_selection(&self) -> Option<(String, [f64; 2], [f64; 2], f64)> {
        let (projet, _) = self.natif.as_ref()?;
        let sel = self.selected?;
        let id = projet.assemblages.iter().find(|a| scene::id_natif("assemblage", &a.id) == sel).map(|a| a.id.clone())
            .or_else(|| projet.murs.iter().find(|m| scene::id_natif("mur", &m.id) == sel).map(|m| m.id.clone()))?;
        let (a, b, z) = projet.axe_element(&id)?;
        Some((id, a, b, z))
    }

    /// Axe du mur sélectionné en tireté et ses extrémités ; pendant un déplacement, le
    /// nouvel axe et sa longueur.
    fn dessiner_axe(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        let Some((_, mut a, mut b, z)) = self.axe_selection() else { return };
        if let Some((_, fin, p)) = &self.poignee_axe {
            if *fin { b = *p } else { a = *p }
        }
        let (Some(ea), Some(eb)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(40, 120, 210);
        painter.add(egui::Shape::dashed_line(&[ea, eb], egui::Stroke::new(1.5, bleu), 8.0, 4.0));
        for e in [ea, eb] {
            let survole = survol.is_some_and(|s| s.distance(e) <= 9.0);
            painter.circle(e, if survole { 6.5 } else { 5.0 }, Color32::WHITE, egui::Stroke::new(2.0, bleu));
            if survole {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
            }
        }
        if self.poignee_axe.is_some() {
            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
            let g = painter.layout_no_wrap(crate::options::unite().afficher(l), egui::FontId::proportional(13.0), Color32::from_gray(30));
            let cadre = egui::Rect::from_center_size(ea + (eb - ea) * 0.5 - egui::vec2(0.0, 16.0), g.size() + egui::vec2(10.0, 4.0));
            painter.rect(cadre, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 235), egui::Stroke::new(1.0, bleu), egui::StrokeKind::Inside);
            painter.galley(cadre.min + egui::vec2(5.0, 2.0), g, Color32::from_gray(30));
        }
    }

    /// Ajoute (pente par défaut) ou retire la pente de l'arête de l'élément sélectionné.
    fn basculer_pente(&mut self, c: usize, k: usize) {
        let (Some((projet, _)), Some(sel)) = (&self.natif, self.selected) else { return };
        let mut p = projet.clone();
        let Some(d) = p.dalles.iter_mut().find(|d| scene::id_natif("dalle", &d.id) == sel) else { return };
        let pente = if d.pente(c, k).is_some() { None } else { Some(self.pente_defaut) };
        d.definir_pente(c, k, pente);
        if let Err(e) = self.appliquer_projet(p) { self.error = Some(e); }
    }

    /// Repère en plan, comme dans Revit. Coupe : trait mixte, têtes rondes, flèches de
    /// visée vers la gauche de la ligne. Élévation : trait fin et, au milieu, cercle et
    /// flèche pleine dans la direction de visée. Nom, profondeur en tireté léger.
    #[allow(clippy::too_many_arguments)]
    fn dessiner_repere_coupe(&self, painter: &egui::Painter, rect: egui::Rect, a: [f64; 2], b: [f64; 2], z: f64, profondeur: f64, elevation: bool, nom: &str, couleur: Color32) {
        let (Some(pa), Some(pb)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) else { return };
        let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
        let g = [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
        let loin = |p: [f64; 2]| [p[0] + g[0] * profondeur, p[1] + g[1] * profondeur];
        if let (Some(pc), Some(pd)) = (self.ecran_plan(rect, loin(b), z), self.ecran_plan(rect, loin(a), z)) {
            painter.add(egui::Shape::dashed_line(&[pb, pc, pd, pa], egui::Stroke::new(0.8, couleur.gamma_multiply(0.5)), 5.0, 4.0));
        }
        // direction de visée à l'écran
        let visee = self.ecran_plan(rect, [a[0] + g[0] * 600.0, a[1] + g[1] * 600.0], z).map(|q| (q - pa).normalized());
        if elevation {
            painter.line_segment([pa, pb], egui::Stroke::new(1.0, couleur));
            let m = pa.lerp(pb, 0.5);
            if let Some(v) = visee {
                // cercle derrière la ligne, flèche vers la façade regardée
                let centre = m - v * 14.0;
                let n = egui::vec2(-v.y, v.x);
                painter.circle(centre, 9.0, Color32::WHITE, egui::Stroke::new(1.5, couleur));
                painter.add(egui::Shape::convex_polygon(vec![centre + v * 18.0, centre + n * 9.0, centre - n * 9.0], couleur, egui::Stroke::NONE));
                painter.text(centre - v * 14.0, egui::Align2::CENTER_CENTER, nom, egui::FontId::proportional(12.0), couleur);
            }
            return;
        }
        painter.add(egui::Shape::dashed_line(&[pa, pb], egui::Stroke::new(1.6, couleur), 14.0, 4.0));
        for p in [pa, pb] {
            painter.circle(p, 6.0, Color32::WHITE, egui::Stroke::new(1.5, couleur));
            if let Some(v) = visee {
                let n = egui::vec2(-v.y, v.x);
                painter.add(egui::Shape::convex_polygon(vec![p + v * 16.0, p + v * 6.0 + n * 6.0, p + v * 6.0 - n * 6.0], couleur, egui::Stroke::NONE));
            }
        }
        painter.text(pa + egui::vec2(-10.0, -10.0), egui::Align2::RIGHT_BOTTOM, nom, egui::FontId::proportional(12.0), couleur);
    }

    /// Repères des coupes natives dans une vue en plan.
    fn reperes_coupes(&self, ui: &egui::Ui, rect: egui::Rect, z: f64) {
        let Some((projet, _)) = &self.natif else { return };
        let painter = ui.painter().with_clip_rect(rect);
        for g in &projet.quadrillages {
            if self.bim.selection.contains(&g.id) {
                if let (Some(a), Some(b)) = (self.ecran_plan(rect, g.debut, z), self.ecran_plan(rect, g.fin, z)) {
                    painter.line_segment([a, b], egui::Stroke::new(2.0, Color32::from_rgb(230, 120, 20)));
                }
            }
        }
        for v in &projet.vues {
            if let crate::modele::GenreVue::Coupe { debut, fin, profondeur, elevation } = &v.genre {
                let couleur = if self.bim.selection.contains(&v.id) { Color32::from_rgb(230, 120, 20) } else { Color32::from_rgb(40, 90, 170) };
                self.dessiner_repere_coupe(&painter, rect, *debut, *fin, z, *profondeur, *elevation, &v.nom, couleur);
            }
        }
    }

    fn rectangles_annotations(&self, rect: egui::Rect) -> Vec<(String, egui::Rect)> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        let Some(vue) = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id)) else { return Vec::new() };
        let Some(scene) = &self.scene else { return Vec::new() };
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let mut rectangles: std::collections::HashMap<u64, egui::Rect> = std::collections::HashMap::new();
        for prim in crate::modele::documentation::annotations(p, vue) {
            let Some(id) = prim.element else { continue };
            let points: Vec<glam::DVec3> = match prim.forme {
                crate::trace2d::Forme::Polyligne { points, .. } => points,
                crate::trace2d::Forme::Segments(s) => s.into_iter().flatten().collect(),
                crate::trace2d::Forme::Triangles(t) => t.into_iter().flatten().collect(),
                crate::trace2d::Forme::Surface(c) => c.into_iter().flatten().collect(),
            };
            for p in points {
                let c = vp * (p - scene.origin).as_vec3().extend(1.0);
                if c.w <= 1e-6 || c.z < -c.w || c.z > c.w { continue; }
                let q = egui::pos2(rect.left()+(c.x/c.w+1.0)*0.5*rect.width(), rect.top()+(1.0-c.y/c.w)*0.5*rect.height());
                rectangles.entry(id).and_modify(|r| r.extend_with(q)).or_insert(egui::Rect::from_min_max(q,q));
            }
        }
        p.annotations.iter().filter_map(|a| rectangles.remove(&scene::id_natif("annotation", &a.id)).map(|r| (a.id.clone(),r.expand(2.0)))).collect()
    }

    fn repere_coupe_proche(&self, rect: egui::Rect, q: egui::Pos2, z: f64) -> Option<String> {
        let (p, _) = self.natif.as_ref()?;
        p.vues.iter().filter_map(|v| if let crate::modele::GenreVue::Coupe { debut, fin, .. } = &v.genre { Some((&v.id, debut, fin)) } else { None })
        .chain(p.quadrillages.iter().map(|g| (&g.id, &g.debut, &g.fin))).filter_map(|(id, debut, fin)| {
            let (a, b) = (self.ecran_plan(rect, *debut, z)?, self.ecran_plan(rect, *fin, z)?);
            let d = b-a; let t = if d.length_sq() > 0.0 { ((q-a).dot(d)/d.length_sq()).clamp(0.0,1.0) } else { 0.0 };
            let distance = q.distance(a+d*t);
            (distance <= 9.0).then_some((distance, id.clone()))
        }).min_by(|a,b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
    }

    /// Aperçu de la coupe en cours de tracé.
    fn apercu_coupe(&self, ui: &egui::Ui, rect: egui::Rect, survol: Option<([f64; 2], AccrocheMur)>, z: f64, prm: &crate::ruban::ParametresCoupe) {
        let painter = ui.painter().with_clip_rect(rect);
        let couleur = Color32::from_rgb(55, 130, 200);
        let accent = Color32::from_rgb(230, 120, 20);
        if let (Some(a), Some((b, _))) = (self.coupe_depart, survol)
            && (b[0] - a[0]).hypot(b[1] - a[1]) > 1.0
        {
            self.dessiner_repere_coupe(&painter, rect, a, b, z, prm.profondeur, prm.elevation, "", couleur);
        }
        if let Some((p, acc)) = survol
            && let Some(q) = self.ecran_plan(rect, p, z)
        {
            marqueur(&painter, q, acc, couleur, accent);
        }
        let sorte = if prm.elevation { "Élévation" } else { "Coupe" };
        let texte = if self.coupe_depart.is_some() { format!("{sorte} : cliquer la fin de la ligne · la vue regarde vers sa gauche · Échap : annuler") } else { format!("{sorte} ({}) : cliquer le début de la ligne", prm.type_vue) };
        painter.text(rect.left_top() + egui::vec2(12.0, 34.0), egui::Align2::LEFT_TOP, texte, egui::FontId::proportional(13.0), couleur);
    }

    /// Comme dans Revit : sélectionner un élément du projet natif (sans outil en cours)
    /// ouvre l'onglet Modifier ; tout désélectionner rend l'onglet d'avant.
    fn onglet_selon_selection(&mut self) {
        let ids = if self.natif.is_some() && self.jeu_actif().is_none() { self.selection_bim() } else { Vec::new() };
        let outil = self.ruban_edition.a_un_outil();
        let change = ids != self.selection_precedente;
        if change && !ids.is_empty() && !outil && self.onglet_ruban != OngletRuban::Modifier {
            self.onglet_avant_selection = Some(self.onglet_ruban);
            self.onglet_ruban = OngletRuban::Modifier;
        } else if change && ids.is_empty() && !outil && self.onglet_ruban == OngletRuban::Modifier {
            if let Some(o) = self.onglet_avant_selection.take() {
                self.onglet_ruban = o;
            }
        }
        if self.onglet_ruban != OngletRuban::Modifier {
            self.onglet_avant_selection = None;
        }
        self.selection_precedente = ids;
    }

    /// Saisie en cours : un champ a le clavier, ou un nombre est glissé (glisser commencé
    /// hors de la vue). À la fin de la saisie, applique en une fois les modifications
    /// faites pendant celle-ci (scène reconstruite, une étape d'historique).
    fn valider_saisie(&mut self, ctx: &egui::Context) {
        let glisser_champ = ctx.dragged_id().is_some() && ctx.input(|i| i.pointer.press_origin()).is_some_and(|p| !self.rect_vue.contains(p));
        let etait_active = self.saisie_active;
        self.saisie_active = ctx.egui_wants_keyboard_input() || glisser_champ;
        // Échap : egui retire le clavier au champ avant cette image ; la saisie est
        // abandonnée et la touche consommée (elle ne quitte pas l'outil en cours)
        self.echap_saisie = (etait_active || self.saisie_active) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if self.echap_saisie {
            self.saisie_annulee = true;
        }
        if self.saisie_active {
            return;
        }
        let annulee = std::mem::take(&mut self.saisie_annulee);
        let Some(avant) = self.avant_saisie.take() else { return };
        let Some((actuel, _)) = self.natif.as_mut() else { return };
        let projet = std::mem::replace(actuel, avant);
        if annulee {
            self.status = "Saisie annulée : ancienne valeur rétablie.".into();
            self.bim.revision = self.bim.revision.wrapping_add(1);
            return;
        }
        if let Err(e) = self.appliquer_projet(projet) {
            self.error = Some(e);
        }
    }

    /// Remplace le projet natif ouvert par `projet` (modifié) et reconstruit la scène,
    /// cadrage, niveaux, catégories et visibilité compris. Rien ne change en cas d'erreur.
    fn appliquer_projet(&mut self, mut projet: crate::modele::Projet) -> Result<(), String> {
        // saisie en cours : le projet suit la frappe, la scène et l'historique attendent
        // la validation (voir valider_saisie)
        if self.saisie_active {
            let (actuel, _) = self.natif.as_mut().ok_or("Aucun projet natif ouvert.")?;
            if self.avant_saisie.is_none() {
                self.avant_saisie = Some(actuel.clone());
            }
            *actuel = projet;
            return Ok(());
        }
        let chemin = self.natif.as_ref().ok_or("Aucun projet natif ouvert.")?.1.clone();
        projet.recaler_menuiseries(&self.natif.as_ref().unwrap().0)?;
        let scene = scene::depuis_natif(&projet, &chemin)?;
        self.historique.pousser(&self.natif.as_ref().unwrap().0, &projet)?;
        // Les indices de vues et d'objets peuvent changer après suppression ou annulation.
        // Retrouver leurs identifiants stables avant de remplacer la scène.
        let anciens=self.scene.as_ref();
        let ancienne_vue=|i:Option<usize>|->Option<Option<usize>> {
            match i {
                None=>Some(None),
                Some(i)=>{let id=anciens?.navigateur.as_ref()?.vues.get(i)?.revit_id;
                    scene.navigateur.as_ref()?.vues.iter().position(|v|v.revit_id==id).map(Some)}
            }
        };
        self.vue_active=ancienne_vue(self.vue_active).flatten();
        self.vues_ouvertes=self.vues_ouvertes.iter().filter_map(|i|ancienne_vue(*i)).collect();
        self.etats_vues=std::mem::take(&mut self.etats_vues).into_iter().filter_map(|(i,e)|ancienne_vue(i).map(|i|(i,e))).collect();
        if self.vues_ouvertes.is_empty(){self.vues_ouvertes.push(None);}
        let nouvel_objet=|i:u32|anciens.and_then(|s|s.objects.get(i as usize)).and_then(|o|scene.object_of.get(&o.id)).copied();
        self.hidden_objects=self.hidden_objects.iter().filter_map(|i|nouvel_objet(*i)).collect();
        self.isolated=self.isolated.as_ref().map(|ids|ids.iter().filter_map(|i|nouvel_objet(*i)).collect());

        let delta = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin) - scene.origin;
        self.camera.target += delta.as_vec3();
        self.camera.scene_radius = ((scene.max - scene.min).length() * 0.5).max(1.0);
        if let Some((a, b)) = &mut self.boite { *a += delta.as_vec3(); *b += delta.as_vec3(); }
        for etat in self.etats_vues.values_mut() {
            etat.camera.target += delta.as_vec3();
            etat.camera.scene_radius = self.camera.scene_radius;
            if let Some((a, b)) = &mut etat.boite { *a += delta.as_vec3(); *b += delta.as_vec3(); }
        }
        if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
            r.upload_vertices(gl, &scene.vertices, &scene.couleurs_rendu, &scene.motifs_surface);
        }
        self.liste_niveaux = niveaux(&scene);
        // matériaux de l'arborescence native (ceux du projet), à défaut ceux de la scène
        self.liste_materiaux = match &scene.navigateur {
            Some(n) => n.materiaux.clone(),
            None => materiaux_ifc(&scene),
        };
        (self.categories, self.categorie_objet, self.cle_objet) = categories(&scene);
        self.visible = vec![true; scene.objects.len()];
        if self.selected.is_some_and(|id| !scene.model.file.entities.contains_key(&id)) {
            self.selected = None;
        }
        self.scene = Some(scene);
        self.bim.revision=self.bim.revision.wrapping_add(1);
        self.natif_modifie = self.historique.modifie(&projet);
        self.natif = Some((projet, chemin));
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.error = None;
        if let Some((p, chemin)) = &self.natif {
            if let Err(e) = crate::modele::enregistrement::recuperer_plus_tard(p, chemin) { self.error = Some(e); }
        }
        Ok(())
    }


    /// Angle (rad) de l'horizontale de l'écran dans le repère du modèle en vue en plan
    /// (repère du plan tourné) ; 0 ailleurs (axes du modèle).
    fn angle_axes_plan(&self) -> f64 {
        if self.vue_en_plan() { crate::orientation_plan::angle(&self.camera) as f64 } else { 0.0 }
    }

    /// Direction principale d'un objet en plan (axe d'inertie de ses sommets en x, y) :
    /// la longueur d'un mur, le grand côté d'une dalle.
    fn direction_principale(&self, obj: u32) -> Option<glam::Vec2> {
        let s = self.scene.as_ref()?;
        let o = s.objects.get(obj as usize)?;
        let pts: Vec<glam::Vec2> = [o.opaque, o.transparent]
            .iter()
            .flat_map(|r| &s.vertices[r.start as usize..(r.start + r.count) as usize])
            .map(|v| glam::Vec2::new(v.pos[0], v.pos[1]))
            .collect();
        if pts.len() < 3 {
            return None;
        }
        let m = pts.iter().copied().sum::<glam::Vec2>() / pts.len() as f32;
        let (mut xx, mut xy, mut yy) = (0.0f32, 0.0f32, 0.0f32);
        for p in &pts {
            let d = *p - m;
            xx += d.x * d.x;
            xy += d.x * d.y;
            yy += d.y * d.y;
        }
        // objet sans direction dominante (carré, cercle) : pas d'alignement
        let ecart = ((xx - yy).powi(2) + 4.0 * xy * xy).sqrt();
        if ecart < 0.05 * (xx + yy) {
            return None;
        }
        let a = 0.5 * (2.0 * xy).atan2(xx - yy);
        Some(glam::Vec2::new(a.cos(), a.sin()))
    }

    /// Arête en plan de l'objet `obj` la plus proche du curseur `p` (écran), pour Aligner.
    /// Arêtes vives de l'objet, verticales exclues.
    fn arete_visee(&mut self, p: egui::Pos2, rect: egui::Rect, obj: u32) -> Option<[Vec3; 2]> {
        if self.aretes_objet.as_ref().is_none_or(|(o, _)| *o != obj) {
            let s = self.scene.as_ref()?;
            let o = s.objects.get(obj as usize)?;
            let tris: Vec<[glam::DVec3; 3]> = [o.opaque, o.transparent]
                .iter()
                .flat_map(|r| s.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3))
                .map(|t| [0, 1, 2].map(|k| Vec3::from(t[k].pos).as_dvec3()))
                .collect();
            let aretes = dessin::aretes(&tris).vives.into_iter().filter(|[a, b]| (b.truncate() - a.truncate()).length() > 1e-4).map(|e| e.map(|q| q.as_vec3())).collect();
            self.aretes_objet = Some((obj, aretes));
        }
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |q: Vec3| {
            let c = vp * q.extend(1.0);
            (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let (_, aretes) = self.aretes_objet.as_ref()?;
        aretes
            .iter()
            .filter_map(|e| {
                let (a, b) = (ecran(e[0])?, ecran(e[1])?);
                let ab = b - a;
                if ab.length_sq() < 4.0 {
                    return None;
                }
                let t = ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
                Some((p.distance(a + ab * t), *e))
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, e)| e)
    }

    /// Arête visée en mode Aligner : surlignée, prolongée en pointillés (future horizontale).
    fn dessiner_arete_alignement(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some([a, b]) = self.arete_alignement else { return };
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let ecran = |q: Vec3| {
            let c = vp * q.extend(1.0);
            (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        };
        let (Some(ea), Some(eb)) = (ecran(a), ecran(b)) else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let couleur = Color32::from_rgb(0, 150, 255);
        let d = (eb - ea).normalized();
        let loin = rect.size().length();
        painter.add(egui::Shape::dashed_line(&[ea - d * loin, eb + d * loin], egui::Stroke::new(1.0, couleur.gamma_multiply(0.7)), 6.0, 5.0));
        painter.line_segment([ea, eb], egui::Stroke::new(4.0, couleur));
        for e in [ea, eb] {
            painter.circle_filled(e, 3.5, couleur);
        }
    }

    /// Orientations de plan enregistrées : celles du projet natif, sinon de la session.
    fn positions_plan_liste(&self) -> Vec<(String, f32)> {
        match &self.natif {
            Some((p, _)) => p.positions_plan.iter().map(|x| (x.nom.clone(), (x.angle as f32).to_radians())).collect(),
            None => self.positions_plan.clone(),
        }
    }

    /// Boîte « Nouvelle position » : nom de l'orientation en cours, puis enregistrement.
    fn fenetre_position_plan(&mut self, ctx: &egui::Context) {
        let Some(mut nom) = self.nom_position_plan.take() else { return };
        let (mut ouverte, mut valider, mut annuler) = (true, false, false);
        egui::Window::new("Nouvelle position")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut ouverte)
            .show(ctx, |ui| {
                ui.label(format!("Orientation en cours : {:.1}°", crate::orientation_plan::angle(&self.camera).to_degrees()));
                let r = ui.add(egui::TextEdit::singleline(&mut nom).hint_text("Nom de la position").desired_width(220.0));
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
                let entree = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let existe = self.positions_plan_liste().iter().any(|(n, _)| n == nom.trim());
                if existe {
                    ui.colored_label(ui.visuals().warn_fg_color, "Une position porte déjà ce nom.");
                }
                ui.horizontal(|ui| {
                    let ok = !nom.trim().is_empty() && !existe;
                    if ui.add_enabled(ok, egui::Button::new("Enregistrer")).clicked() || (ok && entree) {
                        valider = true;
                    }
                    if ui.button("Annuler").clicked() {
                        annuler = true;
                    }
                });
            });
        if annuler || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ouverte = false;
        }
        if valider {
            self.enregistrer_position_plan(nom.trim().to_string());
        } else if ouverte {
            self.nom_position_plan = Some(nom);
        }
    }

    /// Enregistre l'orientation en cours du plan sous `nom` (dans le projet natif s'il y
    /// en a un).
    fn enregistrer_position_plan(&mut self, nom: String) {
        let angle = crate::orientation_plan::angle(&self.camera);
        match &self.natif {
            Some((p, _)) => {
                let mut p = p.clone();
                p.positions_plan.push(crate::modele::PositionPlan { nom: nom.clone(), angle: (angle.to_degrees() as f64 * 10.0).round() / 10.0 });
                let camera = self.camera.clone();
                if let Err(e) = self.appliquer_projet(p) {
                    self.error = Some(e);
                    return;
                }
                self.camera = camera;
            }
            None => self.positions_plan.push((nom.clone(), angle)),
        }
        self.status = format!("{nom} enregistrée ({:.1}°)", angle.to_degrees());
    }

    fn supprimer_position_plan(&mut self, i: usize) {
        match &self.natif {
            Some((p, _)) if i < p.positions_plan.len() => {
                let mut p = p.clone();
                p.positions_plan.remove(i);
                let camera = self.camera.clone();
                if let Err(e) = self.appliquer_projet(p) {
                    self.error = Some(e);
                    return;
                }
                self.camera = camera;
            }
            None if i < self.positions_plan.len() => {
                self.positions_plan.remove(i);
            }
            _ => {}
        }
    }

    /// Point de tracé sous le curseur sur le plan horizontal `altitude` (mm), et son
    /// accrochage : extrémité ou milieu d'un mur de ce niveau, angle d'un mur (contour
    /// raccordé), sommet d'une dalle ou du contour en cours, à dix pixels près ; puis
    /// depuis `depart` : Maj (axes du plan) ou angle rond ; sinon grille de 10 mm.
    fn point_tracage(&self, q: egui::Pos2, rect: egui::Rect, orthogonal: bool, depart: Option<[f64; 2]>, altitude: f64) -> Option<([f64; 2], AccrocheMur)> {
        let (projet, _) = self.natif.as_ref()?;
        let scene = self.scene.as_ref()?;
        let x = (q.x - rect.left()) / rect.width() * 2.0 - 1.0;
        let y = 1.0 - (q.y - rect.top()) / rect.height() * 2.0;
        let (o, d) = self.camera.ray(x, y, rect.width() / rect.height().max(1.0));
        if d.z.abs() < 1e-6 { return None; }
        let t = (altitude * 0.001 - scene.origin.z - o.z as f64) / d.z as f64;
        if !t.is_finite() || t <= 0.0 { return None; }
        let p = (o.as_dvec3() + d.as_dvec3() * t + scene.origin) * 1000.0;
        let mut point = [(p.x / 10.0).round() * 10.0, (p.y / 10.0).round() * 10.0];
        let mut accroche = AccrocheMur::Grille;
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let mut distance = 10.0;
        let mut candidats: Vec<([f64; 2], AccrocheMur)> = Vec::new();
        let jonctions = crate::modele::jonctions::coupes(projet);
        for m in &projet.murs {
            // murs qui traversent ce plan (base au niveau, ou plus bas et plus haut)
            if projet.bas_haut(m).is_none_or(|(b, h)| altitude < b - 0.1 || altitude > h + 0.1) { continue; }
            let (crate::modele::Axe::Droit { debut, fin } | crate::modele::Axe::Arc { debut, fin, .. }) = m.axe;
            if projet.bas_haut(m).is_some_and(|(b, _)| (b - altitude).abs() <= 0.1) {
                candidats.push((debut, AccrocheMur::Extremite));
                candidats.push((fin, AccrocheMur::Extremite));
                candidats.push(([(debut[0] + fin[0]) * 0.5, (debut[1] + fin[1]) * 0.5], AccrocheMur::Milieu));
            }
            if let Some(c) = crate::modele::jonctions::contour(projet, m, &jonctions) {
                candidats.extend(c.into_iter().map(|p| (p, AccrocheMur::Angle(f64::NAN))));
            }
        }
        for d in &projet.dalles {
            candidats.extend(d.contours.iter().flatten().map(|&p| (p, AccrocheMur::Extremite)));
        }
        for (v, genre) in candidats {
            let p = (glam::DVec3::new(v[0], v[1], altitude) * 0.001 - scene.origin).as_vec3();
            let clip = vp * p.extend(1.0);
            if clip.w <= 0.0 { continue; }
            let n = clip.truncate() / clip.w;
            let ecran = egui::pos2(rect.left() + (n.x + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - n.y) * 0.5 * rect.height());
            // à distance égale, l'extrémité l'emporte sur le milieu et l'angle
            let d = q.distance(ecran) + if genre == AccrocheMur::Extremite { 0.0 } else { 0.5 };
            if d < distance {
                point = v;
                distance = d;
                accroche = if matches!(genre, AccrocheMur::Angle(_)) { AccrocheMur::Coin } else { genre };
            }
        }
        if accroche != AccrocheMur::Grille { return Some((point, accroche)); }
        let Some(a) = depart else { return Some((point, accroche)) };
        let (dx, dy) = (point[0] - a[0], point[1] - a[1]);
        let longueur = dx.hypot(dy);
        // axes du plan tourné (repère du plan) : Maj et angles ronds suivent l'écran
        let theta = self.angle_axes_plan();
        let (u, v) = ([theta.cos(), theta.sin()], [-theta.sin(), theta.cos()]);
        if orthogonal {
            let (du, dv) = (dx * u[0] + dy * u[1], dx * v[0] + dy * v[1]);
            let (axe, l) = if du.abs() >= dv.abs() { (u, du) } else { (v, dv) };
            let l = (l / 10.0).round() * 10.0;
            return Some(([a[0] + axe[0] * l, a[1] + axe[1] * l], AccrocheMur::Orthogonal));
        }
        if longueur > 1.0 {
            let angle = (dy.atan2(dx) - theta).to_degrees();
            let rond = (angle / 15.0).round() * 15.0;
            if (angle - rond).abs() < 3.0 {
                let l = (longueur / 10.0).round() * 10.0;
                let r = rond.to_radians() + theta;
                return Some(([a[0] + l * r.cos(), a[1] + l * r.sin()], AccrocheMur::Angle(rond)));
            }
        }
        Some((point, accroche))
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Modèle natif, IFC et Revit", &["json", "ifc", "IFC", "rvt", "rfa", "rte", "rft"])
            .add_filter("Modèle natif v0", &["json"])
            .add_filter("IFC", &["ifc", "IFC"])
            .add_filter("Revit (murs et métadonnées)", &["rvt", "rfa", "rte", "rft"])
            .pick_file()
        {
            self.open(p, ctx);
        }
    }

    // ------------------------------------------------------------------ UI

    fn stocker_document(&mut self) {
        self.historique.geste(false);
        if self.scene.is_none() && self.rvt.is_none() {
            return;
        }
        self.memoriser_vue();
        let status = self.scene.as_ref().map(|s| format!("{} — {} objets 3D, {} triangles", s.file_name, s.objects.len(), s.triangles)).unwrap_or_else(|| self.status.clone());
        self.documents.insert(self.document_actif, DocumentOuvert {
            natif: self.natif.take(),
            historique: std::mem::take(&mut self.historique),
            modifie: self.natif_modifie,
            scene: self.scene.take(),
            rvt: self.rvt.take(),
            rvt_source: self.rvt_source.take(),
            rvt_sortie: self.rvt_sortie.take(),
            rvt_geom: std::mem::replace(&mut self.rvt_geom, Err(String::new())),
            rvt_sheet: self.rvt_sheet,
            vues: std::mem::take(&mut self.vues_ouvertes),
            etats: std::mem::take(&mut self.etats_vues),
            active: self.vue_active,
            selected: self.selected,
            filtre: std::mem::take(&mut self.filter),
            onglet: self.left_tab,
            status,
        });
    }

    fn nouveau_document(&mut self) {
        self.stocker_document();
        self.bim = Default::default();
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.natif_modifie = false;
        self.ruban_edition.niveau_mur = None;
        self.ruban_edition.niveau_haut_mur = None;
        self.ruban_edition.reinitialiser_contexte_creation();
        self.document_actif = self.prochain_document;
        self.prochain_document += 1;
    }

    fn changer_document(&mut self, id: usize) {
        if id == self.document_actif {
            return;
        }
        let Some(d) = self.documents.remove(&id) else { return };
        self.stocker_document();
        self.document_actif = id;
        self.bim = Default::default();
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.natif = d.natif;
        self.avant_saisie = None;
        self.historique = d.historique;
        self.natif_modifie = d.modifie;
        self.ruban_edition.niveau_mur = None;
        self.ruban_edition.niveau_haut_mur = None;
        self.ruban_edition.reinitialiser_contexte_creation();
        self.scene = d.scene;
        self.rvt = d.rvt;
        self.rvt_source = d.rvt_source;
        self.rvt_sortie = d.rvt_sortie;
        self.rvt_geom = d.rvt_geom;
        self.rvt_sheet = d.rvt_sheet;
        self.rvt_preview = None;
        self.vues_ouvertes = d.vues;
        self.etats_vues = d.etats;
        self.vue_active = d.active;
        self.selected = d.selected;
        self.reveal = None;
        self.filter = d.filtre;
        self.left_tab = d.onglet;
        self.status = d.status;
        self.nomenclature_ouverte = None;
        self.point_clique = None;
        self.passes_feuille.clear();
        self.objets_feuille.clear();
        self.voir_visibilite = false;
        self.show_unsupported = false;
        self.error = None;
        self.liste_niveaux.clear();
        self.liste_materiaux.clear();
        self.categories.clear();
        self.categorie_objet.clear();
        self.cle_objet.clear();
        self.visible.clear();
        if let Some(s) = &self.scene {
            self.liste_niveaux = niveaux(s);
            self.liste_materiaux = s.navigateur.as_ref().map_or_else(|| materiaux_ifc(s), |n| n.materiaux.clone());
            (self.categories, self.categorie_objet, self.cle_objet) = categories(s);
            self.visible = vec![true; s.objects.len()];
            if let (Some(gl), Some(r)) = (&self.gl, self.renderer.lock().unwrap().as_mut()) {
                r.upload_vertices(gl, &s.vertices, &s.couleurs_rendu, &s.motifs_surface);
            }
        }
        self.restaurer_vue(self.vue_active);
        self.visibility_dirty = self.scene.is_some();
        self.coupe_dirty = true;
    }

    fn fermer(&mut self) {
        if self.natif_modifie {
            self.confirmer_fermeture = Some((false, self.document_actif));
            return;
        }
        self.fermer_document();
    }

    fn fermer_document(&mut self) {
        self.natif_modifie = false;
        self.natif = None;
        self.avant_saisie = None;
        self.vues_ouvertes.clear();
        self.etats_vues.clear();
        self.loading = None;
        self.scene = None;
        self.rvt = None;
        self.rvt_source = None;
        self.rvt_sortie = None;
        self.rvt_sheet = false;
        self.rvt_geom = Err(String::new());
        self.rvt_preview = None;
        self.selected = None;
        self.selection_multiple.clear();
        self.fenetre_selection = None;
        self.reveal = None;
        self.vue_active = None;
        self.point_clique = None;
        self.nomenclature_ouverte = None;
        self.passes_feuille.clear();
        self.objets_feuille.clear();
        self.hidden_classes.clear();
        self.hidden_objects.clear();
        self.hidden_storeys.clear();
        self.isolated = None;
        self.visible.clear();
        self.liste_niveaux.clear();
        self.liste_materiaux.clear();
        self.categories.clear();
        self.categorie_objet.clear();
        self.cle_objet.clear();
        self.filter.clear();
        self.voir_visibilite = false;
        self.show_unsupported = false;
        self.boite = None;
        self.boite_active = false;
        self.coupe = false;
        self.coupe_niveau = None;
        self.coupe_dirty = true;
        self.visibility_dirty = false;
        self.camera = Camera::default();
        self.vol = Vol::default();
        self.error = None;
        self.status = "Ouvrir un fichier IFC ou Revit (Ctrl+O), ou le glisser dans la fenêtre.".into();
        for tx in self.attente_ouverture.drain(..) {
            let _ = tx.send("Erreur : ouverture annulée par la fermeture du fichier.".into());
        }
        if let Some(&id) = self.documents.keys().next_back() {
            self.changer_document(id);
        }
    }

    /// Ruban en haut : onglets, puis les commandes de l'onglet ouvert.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let maximise = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        // cadre du système : ni boutons de fenêtre ni zones de déplacement dessinés ici
        let natif = crate::cadre_natif();
        let (barre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::hover());
        let titre = egui::Rect::from_min_size(barre.min, egui::vec2(88.0, barre.height()));
        ui.put(titre, egui::Label::new(RichText::new("IFC Viewer").strong()));
        let mut zones_deplacement = vec![titre];
        let menus = egui::Rect::from_min_max(egui::pos2(titre.right() + 8.0, barre.top()), egui::pos2(barre.right() - if natif { 4.0 } else { 144.0 }, barre.bottom()));
        ui.scope_builder(egui::UiBuilder::new().id_salt("menu_fenetre").max_rect(menus).layout(egui::Layout::left_to_right(Align::Center)), |ui| {
            for (o, nom) in [
                (OngletRuban::Debut, "Début"),
                (OngletRuban::Modeliser, "Modéliser"),
                (OngletRuban::Modifier, "Modifier"),
                (OngletRuban::Annoter, "Documenter"),
                (OngletRuban::Sortie, "Sortie"),
            ] {
                ui.selectable_value(&mut self.onglet_ruban, o, RichText::new(nom).strong());
            }
            let disponible = self.natif.is_some() && self.loading.is_none() && self.sauvegarde.is_none();
            let mut historique_choisi = None;
            for (retablir, titre) in [(false, "Annuler"), (true, "Rétablir")] {
                let libelles = self.historique.libelles(retablir);
                let actif = disponible && !libelles.is_empty();
                let reponse = ui.add_enabled(actif, egui::Button::new("").min_size(egui::vec2(22.0, 20.0)));
                let centre = reponse.rect.center();
                let sens = if retablir { -1.0 } else { 1.0 };
                let couleur = if actif { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                let points: Vec<_> = [(-5.0, -2.0), (1.0, -2.0), (4.0, -1.0), (5.0, 2.0), (4.0, 5.0), (1.0, 6.0)].into_iter().map(|(x,y)| centre + egui::vec2(x * sens, y)).collect();
                ui.painter().add(egui::Shape::line(points, egui::Stroke::new(1.5, couleur)));
                let pointe = centre + egui::vec2(-5.0 * sens, -2.0);
                ui.painter().add(egui::Shape::convex_polygon(vec![pointe + egui::vec2(-2.0*sens,0.0), pointe + egui::vec2(2.0*sens,-3.5), pointe + egui::vec2(2.0*sens,3.5)], couleur, egui::Stroke::NONE));
                reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, actif, titre));
                if reponse.on_hover_text(if retablir { "Rétablir (Ctrl+Maj+Z)" } else { "Annuler (Ctrl+Z)" }).clicked() { historique_choisi = Some((retablir, 1)); }
                ui.add_enabled_ui(actif, |ui| {
                    ui.scope(|ui| {
                        ui.spacing_mut().button_padding = egui::Vec2::ZERO;
                        let bouton = egui::Button::new("").min_size(egui::vec2(16.0, 20.0));
                        let (reponse, _) = egui::containers::menu::MenuButton::from_button(bouton).ui(ui, |ui| {
                            ui.label(if retablir { "Rétablir jusqu'à :" } else { "Annuler jusqu'à :" });
                            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                for (i, nom) in libelles.iter().enumerate() {
                                    if ui.button(format!("{}  {}", i + 1, nom)).clicked() {
                                        historique_choisi = Some((retablir, i + 1));
                                        ui.close();
                                    }
                                }
                            });
                        });
                        let centre = reponse.rect.center();
                        let c = if actif { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                        ui.painter().add(egui::Shape::convex_polygon(vec![centre + egui::vec2(-3.5,-2.0), centre + egui::vec2(3.5,-2.0), centre + egui::vec2(0.0,2.0)], c, egui::Stroke::NONE));
                        reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, actif, if retablir { "Historique des rétablissements" } else { "Historique des annulations" }));
                        reponse.on_hover_text(if retablir { "Rétablir plusieurs modifications" } else { "Annuler plusieurs modifications" });
                    });
                });
            }
            if let Some((retablir, nombre)) = historique_choisi { self.parcourir_historique_n(retablir, nombre); }
            if self.loading.is_some() || self.sauvegarde.is_some() {
                ui.spinner();
                let texte = self.sauvegarde.as_ref().map_or(&self.status, |s| &s.progression);
                let reponse = ui.add_sized([ui.available_width().max(0.0), 24.0], egui::Label::new(texte).truncate()).on_hover_text(texte);
                zones_deplacement.push(reponse.rect);
            }
            let (libre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(0.0), 32.0), Sense::hover());
            zones_deplacement.push(libre);
        });
        if natif {
            zones_deplacement.clear();
        }
        for (k, rect) in zones_deplacement.into_iter().enumerate() {
            let reponse = ui.interact(rect, ui.scope_id().with(("deplacer_fenetre", k)), Sense::click_and_drag());
            if reponse.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximise));
            } else if reponse.drag_started_by(PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
        for (k, aide) in ["Réduire", if maximise { "Restaurer" } else { "Agrandir" }, "Fermer l’application"].into_iter().enumerate().filter(|_| !natif) {
            let rect = egui::Rect::from_min_size(egui::pos2(barre.right() - 138.0 + k as f32 * 46.0, barre.top()), egui::vec2(46.0, 30.0));
            let reponse = ui.interact(rect, ui.scope_id().with(("bouton_fenetre", k)), Sense::click()).on_hover_text(aide);
            let survole = reponse.hovered() || reponse.is_pointer_button_down_on();
            let couleur = if survole && k == 2 { Color32::WHITE } else { ui.visuals().text_color() };
            if survole {
                ui.painter().rect_filled(rect, 0.0, if k == 2 { Color32::from_rgb(196, 43, 28) } else { Color32::from_gray(225) });
            }
            let centre = rect.center();
            let icone = egui::Rect::from_center_size(centre, egui::vec2(10.0, 10.0));
            let trait_fin = egui::Stroke::new(1.0, couleur);
            match k {
                0 => { ui.painter().line_segment([icone.left_center(), icone.right_center()], trait_fin); }
                1 if maximise => {
                    let arriere = egui::Rect::from_min_size(icone.min + egui::vec2(3.0, 0.0), egui::vec2(7.0, 7.0));
                    ui.painter().line_segment([arriere.left_top(), arriere.right_top()], trait_fin);
                    ui.painter().line_segment([arriere.right_top(), arriere.right_bottom()], trait_fin);
                    let avant = egui::Rect::from_min_size(icone.min + egui::vec2(0.0, 3.0), egui::vec2(7.0, 7.0));
                    ui.painter().rect_stroke(avant, 0.0, trait_fin, egui::StrokeKind::Inside);
                }
                1 => { ui.painter().rect_stroke(icone, 0.0, trait_fin, egui::StrokeKind::Inside); }
                _ => {
                    ui.painter().line_segment([icone.left_top(), icone.right_bottom()], trait_fin);
                    ui.painter().line_segment([icone.right_top(), icone.left_bottom()], trait_fin);
                }
            }
            if reponse.clicked() {
                if k == 2 && self.sauvegarde.is_some() {
                    self.error = Some("Un enregistrement est en cours ; attendez sa fin avant de quitter.".into());
                    continue;
                }
                if k == 2 && (self.natif_modifie || self.documents.values().any(|d| d.modifie)) {
                    self.confirmer_fermeture = Some((true, self.document_actif));
                    continue;
                }
                ctx.send_viewport_cmd(match k {
                    0 => egui::ViewportCommand::Minimized(true),
                    1 => egui::ViewportCommand::Maximized(!maximise),
                    _ => egui::ViewportCommand::Close,
                });
            }
        }
        ui.separator();
        ui.set_min_height(28.0);
        let edition = match self.onglet_ruban {
            OngletRuban::Modeliser => Some(crate::ruban::Menu::Modeliser),
            OngletRuban::Modifier => Some(crate::ruban::Menu::Modifier),
            OngletRuban::Annoter => Some(crate::ruban::Menu::Annoter),
            _ => None,
        };
        if let Some(menu) = edition {
            match self.ruban_edition.afficher(ui, menu, self.natif.as_ref().map(|n| &n.0)) {
                Some(crate::ruban::Outil::CreerIdentique) => {
                    if let Err(e) = self.creer_identique() { self.error = Some(e); }
                }
                Some(crate::ruban::Outil::Supprimer) => self.demander_suppression(),
                Some(o @ (crate::ruban::Outil::Cadrer | crate::ruban::Outil::Isoler | crate::ruban::Outil::Masquer)) => self.visibilite_selection(o),
                Some(crate::ruban::Outil::Feuilles) => self.commande_feuilles(),
                Some(crate::ruban::Outil::Nomenclatures) => {
                    if self.natif.is_some() {
                        self.bim.documentation = true;
                    } else {
                        self.status = "Nomenclatures : ouvrir un projet natif.".into();
                    }
                }
                _ => {}
            }
            return;
        }
        match self.onglet_ruban {
            OngletRuban::Debut => {
                use crate::ruban::{Commande as C, Icone as I};
                let fichier = vec![
                    C::new("nouveau", "Nouveau", I::Nouveau, "Créer un projet natif v0 (Ctrl+N)", self.loading.is_none()),
                    C::new("ouvrir", "Ouvrir", I::Ouvrir, "Ouvrir un projet natif, un IFC, un RVT ou un RFA (Ctrl+O)", true),
                    C::new("fermer", "Fermer", I::Fermer, "Fermer le document actif", self.scene.is_some() || self.rvt.is_some() || self.loading.is_some()),
                    C::new("importer", "Importer", I::Importer, "Convertir un IFC ou RVT en nouveau document natif", self.loading.is_none()),
                ];
                let mut reglages = Vec::new();
                if self.rvt.is_some() && self.rvt_geom.is_ok() {
                    reglages.push(C { enfonce: self.rvt_sheet, ..C::new("fiche", "Fiche Revit", I::Fiche, "Métadonnées du fichier Revit", true) });
                }
                reglages.push(C::new("options", "Options", I::Options, "Unités, navigation IFC, épaisseurs de traits, gabarit", true));
                for (cle, r) in crate::ruban::bandeau(ui, &[("Fichier", fichier), ("Réglages", reglages)]) {
                    match cle {
                        "options" => {
                            egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| self.menu_options(ui));
                        }
                        _ if !r.clicked() => {}
                        "nouveau" => self.creer_projet(),
                        "ouvrir" => self.open_dialog(&ctx),
                        "fermer" => self.fermer(),
                        "importer" => self.importer_dialogue(&ctx),
                        "fiche" => self.rvt_sheet = !self.rvt_sheet,
                        _ => {}
                    }
                }
            }
            OngletRuban::Modeliser | OngletRuban::Modifier | OngletRuban::Annoter => {}
            OngletRuban::Sortie => {
                use crate::ruban::{Commande as C, Icone as I};
                let disponible = (self.natif.is_some() || self.rvt_source.is_some()) && self.loading.is_none() && self.sauvegarde.is_none();
                let enregistrer = vec![
                    C::new("enregistrer", "Enregistrer", I::Enregistrer, "Enregistrer en modèle natif v0 (Ctrl+S)", disponible),
                    C::new("enregistrer_sous", "Enregistrer sous", I::EnregistrerSous, "Enregistrer le projet natif dans un autre fichier", self.natif.is_some()),
                ];
                let exporter = vec![
                    C::new("svg", "Dessin SVG", I::Exporter("SVG"), "Exporter le dessin de la vue en SVG (vue orthographique)", self.scene.is_some()),
                    C::new("pdf", "Dessin PDF", I::Exporter("PDF"), "Exporter le dessin de la vue en PDF (vue orthographique)", self.scene.is_some()),
                    C::new("ifc", "IFC4", I::Exporter("IFC"), "Exporter le projet natif en IFC4", self.natif.is_some()),
                    C::new("jeu_pdf", "Jeu PDF", I::Exporter("PDF"), "Exporter le jeu de feuilles ouvert (sinon le premier) en un PDF, une page par feuille", self.natif.as_ref().is_some_and(|(p, _)| !p.feuilles.is_empty())),
                ];
                for (cle, r) in crate::ruban::bandeau(ui, &[("Enregistrer", enregistrer), ("Exporter", exporter)]) {
                    if !r.clicked() {
                        continue;
                    }
                    match cle {
                        "enregistrer" => self.enregistrer_projet(&ctx),
                        "enregistrer_sous" => self.enregistrer_sous(),
                        "svg" => self.exporter_svg_dialogue(),
                        "pdf" => self.export_pdf_dialogue(),
                        "ifc" => self.export_ifc_dialogue(),
                        "jeu_pdf" => {
                            let jeu = self.jeu_actif().or_else(|| self.natif.as_ref().and_then(|(p, _)| crate::modele::documentation::jeux(p).into_iter().next()));
                            if let Some(jeu) = jeu {
                                self.exporter_jeu_dialogue(&jeu);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Menu Options (Début) : unités, navigation IFC, épaisseurs de traits, gabarit.
    fn menu_options(&mut self, ui: &mut egui::Ui) {
            ui.label("Unités de longueur du modèle");
            for (unite, nom) in [(crate::options::Unite::Millimetre, "Millimètres"), (crate::options::Unite::Centimetre, "Centimètres"), (crate::options::Unite::Metre, "Mètres")] {
                if ui.selectable_value(&mut self.options.unite, unite, nom).changed() { crate::options::definir_unite(unite); self.options.enregistrer(); self.coupe_dirty = true; self.bim.revision = self.bim.revision.wrapping_add(1); }
            }
            ui.separator();
            if ui.checkbox(&mut self.options.navigation_ifc, "Navigation IFC")
                .on_hover_text("Arborescence spatiale, classes et attributs IFC (GlobalId, Id STEP) pour les modèles Revit et natifs")
                .changed()
            {
                self.options.enregistrer();
            }
            ui.separator();
            if ui.add_enabled(self.natif.is_some(), egui::Button::new("Épaisseurs de traits…"))
                .on_hover_text("Épaisseurs des 12 plumes du projet (mm du papier)")
                .on_disabled_hover_text("Ouvrir un projet natif")
                .clicked()
            {
                ui.close();
                self.editeur_plumes = self.natif.as_ref().map(|(p, _)| p.epaisseurs_plumes());
            }
            ui.separator();
            let g = crate::gabarit::actif();
            ui.label(RichText::new(format!("Gabarit : {}", g.nom)).strong())
                .on_hover_text(self.options.gabarit.as_ref().map_or("Gabarit embarqué".into(), |c| c.display().to_string()));
            ui.label(RichText::new(format!("{} types de murs · {} éléments horizontaux · {} vues", g.types_murs.len(), g.types_surfaces.len(), g.types_vues.len())).small().weak());
            if ui.button("Choisir un gabarit…").on_hover_text("Fichier .gabarit.json : matériaux, types de murs, d'éléments horizontaux et de vues").clicked() {
                ui.close();
                if let Some(c) = rfd::FileDialog::new().set_title("Gabarit").add_filter("Gabarit", &["json"]).pick_file() {
                    match crate::gabarit::Gabarit::charger(&c) {
                        Ok(g) => {
                            self.status = format!("Gabarit « {} » chargé", g.nom);
                            crate::gabarit::definir(g);
                            self.options.gabarit = Some(c);
                            self.options.enregistrer();
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
            if ui.add_enabled(self.options.gabarit.is_some(), egui::Button::new("Gabarit par défaut")).clicked() {
                ui.close();
                crate::gabarit::definir(crate::gabarit::Gabarit::defaut());
                self.options.gabarit = None;
                self.options.enregistrer();
            }
            if ui.button("Enregistrer une copie du gabarit…").on_hover_text("Point de départ d'un gabarit d'agence, à modifier dans un éditeur de texte").clicked() {
                ui.close();
                if let Some(c) = rfd::FileDialog::new().set_title("Copie du gabarit").add_filter("Gabarit", &["json"]).set_file_name("agence.gabarit.json").save_file() {
                    let texte = match &self.options.gabarit {
                        Some(src) => std::fs::read_to_string(src).map_err(|e| e.to_string()),
                        None => Ok(crate::gabarit::texte_defaut().to_string()),
                    };
                    if let Err(e) = texte.and_then(|t| std::fs::write(&c, t).map_err(|e| e.to_string())) {
                        self.error = Some(format!("Copie du gabarit impossible : {e}"));
                    }
                }
            }
    }

    /// Redimensionnement natif sur les bords de la fenêtre sans décoration.
    fn bords_fenetre(&self, ctx: &egui::Context) {
        use egui::ResizeDirection::*;
        if crate::cadre_natif() {
            return;
        }
        let (rect, position, maximise, presse) = ctx.input(|i| (i.viewport_rect(), i.pointer.hover_pos(), i.viewport().maximized.unwrap_or(false), i.pointer.primary_pressed()));
        let Some(p) = position.filter(|p| rect.contains(*p) && !maximise) else { return };
        let gauche = p.x < rect.left() + 5.0;
        let droite = p.x > rect.right() - 5.0;
        let haut = p.y < rect.top() + 5.0;
        let bas = p.y > rect.bottom() - 5.0;
        let direction = match (gauche, droite, haut, bas) {
            (true, _, true, _) => NorthWest,
            (_, true, true, _) => NorthEast,
            (true, _, _, true) => SouthWest,
            (_, true, _, true) => SouthEast,
            (true, _, _, _) => West,
            (_, true, _, _) => East,
            (_, _, true, _) => North,
            (_, _, _, true) => South,
            _ => return,
        };
        ctx.set_cursor_icon(match direction {
            NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
            NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
            North | South => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::ResizeHorizontal,
        });
        if presse {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }

    /// Vue 3D : modèle libre (IFC, ou fichier Revit sans vue ouverte) ou vue 3D Revit ;
    /// pas un plan, une coupe, une élévation ni une feuille.
    fn vue_3d(&self) -> bool {
        match self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            Some(v) => v.genre == "Vue 3D",
            None => true,
        }
    }

    /// Plans Revit et vues orthographiques du modèle regardées verticalement.
    fn vue_en_plan(&self) -> bool {
        if let Some(v) = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)) {
            return matches!(v.genre.as_str(), "Plan d'étage" | "Plan de plafond" | "Plan de structure" | "Plan de site");
        }
        self.camera.ortho && self.camera.pitch.abs() > 1.56
    }

    fn orientation_initiale_plan(&self) -> f32 {
        self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i))
            .map_or(-std::f32::consts::FRAC_PI_2, |v| (-v.haut.y as f32).atan2(-v.haut.x as f32))
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
                if ui.selectable_label(self.traits_fins, "Traits fins").on_hover_text("Tous les traits à un pixel, sans les épaisseurs des plumes").clicked() {
                    self.traits_fins = !self.traits_fins;
                    self.coupe_dirty = true;
                }
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
        // onglet Famille : les panneaux montrent la famille
        if self.famille_active && self.famille.is_some() {
            match p {
                Panneau::Proprietes => self.proprietes_famille(ui),
                Panneau::Arborescence => self.arborescence_famille(ui),
            }
            return;
        }
        match p {
            Panneau::Proprietes => self.right_panel(ui),
            Panneau::Arborescence => self.left_panel(ui),
        }
    }

    /// Le titre entier sert de poignée, sans menu d'ancrage.
    fn entete_panneau(&mut self, ui: &mut egui::Ui, p: Panneau, titre: &str) {
        let i = p as usize;
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::hover());
            let resp = ui.interact(rect, egui::Id::unique(("entete_panneau", p.nom())), Sense::click_and_drag());
            let encre = ui.visuals().strong_text_color();
            ui.painter_at(rect).text(rect.left_center(), egui::Align2::LEFT_CENTER, titre, egui::FontId::proportional(15.0), encre);
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some(q) = resp.interact_pointer_pos() {
                    let decalage = if self.ancrages[i] == Ancrage::Flottant {
                        q - self.pos_flottants[i]
                    } else {
                        egui::vec2((q.x - rect.left()).clamp(20.0, 140.0), 16.0)
                    };
                    self.pos_imposee[i] = Some(q - decalage);
                    self.deplacement_panneau = Some((p, decalage));
                    self.ancrer(p, Ancrage::Flottant);
                }
            }
            let survol = if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab };
            resp.on_hover_cursor(survol).on_hover_text("Glisser pour déplacer ; relâcher contre un bord de la fenêtre pour ancrer");
        });
    }

    fn cible_ancrage(&self, q: egui::Pos2) -> Option<Ancrage> {
        let r = self.zone_panneaux;
        if !r.is_positive() || q.y < r.top() || q.y > r.bottom() {
            return None;
        }
        if q.x <= r.left() + 64.0 { Some(Ancrage::Gauche) }
        else if q.x >= r.right() - 64.0 { Some(Ancrage::Droite) }
        else { None }
    }

    /// Le suivi reste actif pendant le passage d'un panneau ancré à une fenêtre flottante.
    fn deplacer_panneau(&mut self, ctx: &egui::Context) {
        let Some((p, decalage)) = self.deplacement_panneau else { return };
        let (position, enfonce) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down()));
        if let Some(q) = position {
            let r = self.zone_panneaux;
            let v = q - decalage;
            self.pos_imposee[p as usize] = Some(egui::pos2(v.x.clamp(r.left(), (r.right() - 80.0).max(r.left())), v.y.clamp(r.top(), (r.bottom() - 30.0).max(r.top()))));
            if !enfonce {
                if let Some(a) = self.cible_ancrage(q) {
                    self.ancrer(p, a);
                }
            }
        }
        if !enfonce {
            self.deplacement_panneau = None;
        }
        ctx.request_repaint();
    }

    fn apercu_ancrage(&self, ctx: &egui::Context) {
        let Some((p, _)) = self.deplacement_panneau else { return };
        let Some(cible) = ctx.pointer_latest_pos().and_then(|q| self.cible_ancrage(q)) else { return };
        let zone = self.zone_panneaux;
        let largeur = self.largeurs_panneaux[p as usize].min(zone.width() * 0.45);
        let x = if cible == Ancrage::Gauche { zone.left() } else { zone.right() - largeur };
        let rect = egui::Rect::from_min_size(egui::pos2(x, zone.top()), egui::vec2(largeur, zone.height()));
        let peintre = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("apercu_ancrage")));
        peintre.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(90, 140, 195, 45));
        peintre.rect_stroke(rect, 4.0, egui::Stroke::new(1.5, Color32::from_rgb(90, 140, 195)), egui::StrokeKind::Inside);
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
        if a != Ancrage::Flottant {
            self.pos_imposee[i] = None;
        }
    }

    /// Soleil du style Rendu : direction (depuis le sud-ouest, 45° de hauteur environ),
    /// vue orthographique du soleil englobant la scène, décalage contre l'acné d'ombre
    /// (un texel et demi de la carte d'ombres).
    /// Ombres portées : direction du soleil, matrice de la carte d'ombres et décalage contre
    /// l'acné. La carte couvre la partie de la scène vue par la caméra (et non la scène
    /// entière) : plus on zoome, plus elle est fine ; ses bornes suivent la grille de ses
    /// pixels pour que les ombres ne frémissent pas quand la caméra bouge.
    fn soleil(&self, aspect: f32) -> Option<(Vec3, Mat4, f32)> {
        let s = self.scene.as_ref()?;
        let dir = crate::render::SOLEIL;
        let (c, r) = ((s.min + s.max) * 0.5, ((s.max - s.min).length() * 0.5).max(0.1) * 1.02);
        let vue = glam::camera::rh::view::look_at_mat4(c + dir * r * 2.0, c, Vec3::Z);
        let coins_scene: Vec<Vec3> = (0..8).map(|k| Vec3::new(if k & 1 == 0 { s.min.x } else { s.max.x }, if k & 2 == 0 { s.min.y } else { s.max.y }, if k & 4 == 0 { s.min.z } else { s.max.z })).collect();
        let bornes = |pts: &mut dyn Iterator<Item = Vec3>| pts.map(|p| vue.transform_point3(p)).fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), q| (a.min(q), b.max(q)));
        let (smin, smax) = bornes(&mut coins_scene.iter().copied());
        // volume de vue limité à la sphère de la scène : rayons des coins de l'écran
        let inv = self.camera.view_proj(aspect).inverse();
        let mut vus = Vec::new();
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (0.0, 0.0)] {
            let (a, b) = (inv.project_point3(Vec3::new(x, y, -1.0)), inv.project_point3(Vec3::new(x, y, 1.0)));
            let d = b - a;
            let l = d.length();
            if !(l > 1e-6) || !l.is_finite() { continue; }
            let u = d / l;
            // entrée et sortie de la sphère (c, r) le long de [a, b]
            let m = (a - c).dot(u);
            let disc = m * m - ((a - c).length_squared() - r * r);
            if disc < 0.0 { continue; }
            // en perspective, ombres jusqu'à quatre fois la distance de la cible : au-delà
            // elles sont trop petites pour compter et la carte s'étalerait jusqu'à l'horizon
            let portee = if self.camera.ortho { l } else { (self.camera.dist * 4.0).min(l) };
            let (t0, t1) = ((-m - disc.sqrt()).max(0.0), (-m + disc.sqrt()).min(portee));
            if t1 < t0 { continue; }
            vus.extend([a + u * t0, a + u * t1]);
        }
        let (mut lo, mut hi) = (smin.truncate(), smax.truncate());
        if !vus.is_empty() {
            let (vmin, vmax) = bornes(&mut vus.iter().copied());
            // le centre de l'écran peut ne rien viser : on garde au moins sa projection
            lo = lo.max(vmin.truncate());
            hi = hi.min(vmax.truncate());
            if lo.cmpge(hi).any() { (lo, hi) = (smin.truncate(), smax.truncate()); }
        }
        // carré calé sur la grille des pixels de la carte
        let cote = (hi - lo).max_element().max(0.01) * 1.05;
        let texel = cote / crate::render::TAILLE_OMBRES as f32;
        let centre = (((lo + hi) * 0.5) / texel).floor() * texel;
        let (x0, y0) = (centre - glam::Vec2::splat(cote * 0.5)).into();
        let proj = glam::camera::rh::proj::opengl::orthographic(x0, x0 + cote, y0, y0 + cote, -smax.z - 0.01, -smin.z + 0.01);
        Some((dir, proj * vue, 1.5 * texel))
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
            ui.separator();
            let graphique = self.style == StyleVisuel::Graphique;
            let mut couleurs = !self.faces_blanches;
            let mut change = ui.add_enabled(graphique, egui::Checkbox::new(&mut couleurs, "Couleurs des faces"))
                .on_hover_text("Décoché : faces blanches, dessin au trait")
                .changed();
            self.faces_blanches = !couleurs;
            change |= ui.add_enabled(graphique && couleurs, egui::Checkbox::new(&mut self.eclairage, "Éclairage"))
                .on_hover_text("Décoché : couleurs en aplat")
                .changed();
            ui.separator();
            change |= ui.checkbox(&mut self.ombres_portees, "Ombres portées").on_hover_text("Ombres du soleil").changed();
            change |= ui.checkbox(&mut self.ombre_ambiante, "Ombre ambiante").on_hover_text("Assombrit les angles rentrants, pieds de murs et dessous des débords").changed();
            if change {
                self.visibility_dirty = true;
            }
        })
        .response
        .on_hover_text("Style visuel et ombres");
        if let Some(st) = choisi {
            self.style = st;
            self.visibility_dirty = true;
            self.coupe_dirty = true;
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
            if self.loading.is_none() {
                ui.label(&self.status);
            }
            if let (Some(p), Some(s)) = (self.point_clique, &self.scene) {
                // coordonnées partagées (Revit placé) ou du fichier
                ui.separator();
                let texte = match &s.partage {
                    Some(r) => {
                        let q = r.partage(p);
                        format!("E/O {} · N/S {} · Élév. {} ({})", self.options.unite.afficher(q.x*1000.0), self.options.unite.afficher(q.y*1000.0), self.options.unite.afficher(q.z*1000.0), r.nom)
                    }
                    None => format!("X {} · Y {} · Z {}", self.options.unite.afficher(p.x*1000.0), self.options.unite.afficher(p.y*1000.0), self.options.unite.afficher(p.z*1000.0)),
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
        let onglets: &[LeftTab] = match (a_nav, self.options.navigation_ifc) {
            (false, _) => &LeftTab::IFC,
            (true, true) => &LeftTab::REVIT,
            (true, false) => &LeftTab::REVIT[..LeftTab::REVIT.len() - 2],
        };
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
        let natif = self.natif.is_some();
        let mut editer: Option<String> = None;
        let mut n = 0;
        for m in self.liste_materiaux.iter().filter(|m| q.is_empty() || m.nom.to_lowercase().contains(&q)) {
            n += 1;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                let [cr, cg, cb] = m.couleur;
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(cr, cg, cb));
                ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
                let texte = format!("{} ({})", m.nom, m.elements.len());
                if natif && ui.small_button("…").on_hover_text("Graphismes du matériau : surface et coupe").clicked() {
                    editer = Some(m.nom.clone());
                }
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
        if editer.is_some() {
            self.materiau_edite = editer;
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
        // projet natif : jeux de feuilles et leurs feuilles, nomenclatures ; vues
        // glissables vers une feuille
        // jeux : (nom, feuilles : (id, titre, vues placées : (numéro de vue, nom, échelle)))
        type FeuilleNav = (String, String, Vec<(i64, String, f64)>, Vec<(String, String)>);
        let (jeux_natifs, nomenclatures_natives, glissables): (Vec<(String, Vec<FeuilleNav>)>, Vec<(String, String)>, std::collections::HashMap<i64, String>) = match &self.natif {
            Some((p, _)) => (
                crate::modele::documentation::jeux(p).into_iter().map(|j| {
                    let fs = p.feuilles.iter().filter(|f| f.jeu == j).map(|f| {
                        let vues = f.vues.iter().map(|v| (crate::scene::numero_vue(&v.vue), p.vues.iter().find(|x| x.id == v.vue).map_or("Vue absente".to_string(), |x| x.nom.clone()), v.echelle)).collect();
                        let tables = f.nomenclatures.iter().map(|n| (n.nomenclature.clone(), p.nomenclatures.iter().find(|x| x.id == n.nomenclature).map_or("Nomenclature absente".to_string(), |x| x.nom.clone()))).collect();
                        (f.id.clone(), format!("{} - {}", f.numero, f.nom), vues, tables)
                    }).collect();
                    (j, fs)
                }).collect(),
                p.nomenclatures.iter().map(|n| (n.id.clone(), n.nom.clone())).collect(),
                p.vues.iter().map(|v| (crate::scene::numero_vue(&v.id), v.id.clone())).collect(),
            ),
            None => (Vec::new(), Vec::new(), std::collections::HashMap::new()),
        };
        let mut ouvrir_jeu: Option<(String, Option<String>)> = None;
        // feuille glissée dans l'arborescence : (feuille, jeu, feuille devant laquelle)
        let mut deplacer: Option<(String, String, Option<String>)> = None;
        let mut ouvrir_nomenclature: Option<String> = None;
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
            // vue du projet natif : glissable sur une feuille d'un jeu
            let glissable = vue.and_then(|i| nav.vues.get(i)).filter(|v| matches!(v.genre.as_str(), "Plan d'étage" | "Coupe" | "Élévation")).and_then(|v| glissables.get(&v.revit_id));
            let mut bouton = egui::Button::selectable(entite.is_some() && entite == selection, texte);
            if glissable.is_some() {
                bouton = bouton.sense(Sense::click_and_drag());
            }
            let r = ui.add(bouton);
            if let Some(id) = glissable {
                r.dnd_set_drag_payload(feuilles::VueGlissee(id.clone()));
            }
            let r = if glissable.is_some() { r.on_hover_text("Double-clic : ouvrir · glisser sur une feuille d'un jeu pour la placer") } else { r };
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
        rubrique_nav(ui, LeftTab::Nomenclatures, format!("Nomenclatures/Quantités (tout) ({})", nomenclatures.len() + nomenclatures_natives.len()), false, &mut |ui| {
            if nomenclatures.is_empty() && nomenclatures_natives.is_empty() {
                ui.label(RichText::new("Aucune nomenclature").weak());
            }
            for (id, nom) in &nomenclatures_natives {
                if !garde(nom) {
                    continue;
                }
                let r = ui.add(egui::Button::selectable(false, nom.as_str()).sense(Sense::click_and_drag())).on_hover_text("Clic : ouvrir la nomenclature · glisser sur une feuille d'un jeu pour y poser le tableau");
                r.dnd_set_drag_payload(feuilles::NomenclatureGlissee(id.clone()));
                if r.clicked() {
                    ouvrir_nomenclature = Some(id.clone());
                }
            }
            for &i in &nomenclatures {
                feuille(ui, nav.vues[i].nom.clone(), nav.vues[i].entite, Some(i), &mut action);
            }
        });
        let mut feuilles: Vec<usize> = (0..nav.vues.len()).filter(|&i| nav.vues[i].genre == "Feuille").collect();
        feuilles.sort_by_key(|&i| (nav.vues[i].numero.clone(), nav.vues[i].nom.clone()));
        let indice: std::collections::HashMap<i64, usize> = nav.vues.iter().enumerate().map(|(i, v)| (v.revit_id, i)).collect();
        let jeu_ouvert = active.and_then(|i| nav.vues.get(i)).filter(|v| v.genre == "Jeu de feuilles").map(|v| v.nom.clone());
        let n_natives: usize = jeux_natifs.iter().map(|(_, fs)| fs.len()).sum();
        rubrique_nav(ui, LeftTab::Feuilles, format!("Feuilles (tout) ({})", feuilles.len() + n_natives), false, &mut |ui| {
            if feuilles.is_empty() && jeux_natifs.is_empty() {
                ui.label(RichText::new("Aucune feuille").weak());
            }
            // jeux du projet natif : clic sur le jeu : l'ouvrir ; sur une feuille : l'ouvrir
            // à cette feuille
            for (jeu, fs) in &jeux_natifs {
                let id = ui.make_persistent_id(("nav-jeu", jeu));
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                    .show_header(ui, |ui| {
                        let texte = RichText::new(format!("{jeu} ({})", fs.len()));
                        let texte = if jeu_ouvert.as_deref() == Some(jeu.as_str()) { texte.strong() } else { texte };
                        let r = ui.selectable_label(false, texte).on_hover_text("Ouvrir le jeu : toutes ses feuilles à la suite ; y déposer une feuille la met à la fin du jeu");
                        if r.clicked() {
                            ouvrir_jeu = Some((jeu.clone(), None));
                        }
                        // feuille déposée sur le jeu : à la fin du jeu
                        if r.dnd_hover_payload::<feuilles::FeuilleGlissee>().is_some() {
                            ui.painter().rect_stroke(r.rect, 3.0, egui::Stroke::new(1.5, ui.visuals().selection.stroke.color), egui::StrokeKind::Inside);
                        }
                        if let Some(g) = r.dnd_hover_payload::<feuilles::FeuilleGlissee>().and_then(|_| r.dnd_release_payload::<feuilles::FeuilleGlissee>()) {
                            deplacer = Some((g.0.clone(), jeu.clone(), None));
                        }
                    })
                    .body(|ui| {
                        // recherche : un jeu trouvé montre toutes ses feuilles ; une feuille,
                        // si son titre ou une de ses vues placées correspond
                        let jeu_trouve = garde(jeu);
                        for (k, (fid, titre, vues, tables)) in fs.iter().enumerate() {
                            if !jeu_trouve && !garde(titre) && !vues.iter().any(|v| garde(&v.1)) {
                                continue;
                            }
                            // feuille : dépliable sur ses vues placées (comme dans Revit)
                            let id_feuille = ui.make_persistent_id(("nav-feuille-native", fid));
                            let etat = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id_feuille, true);
                            let (_, entete, _) = etat.show_header(ui, |ui| ui.add(egui::Button::selectable(false, titre.as_str()).sense(Sense::click_and_drag())).on_hover_text("Clic : afficher la feuille · glisser : la déplacer dans le jeu ou vers un autre jeu"))
                                .body(|ui| {
                                    if vues.is_empty() && tables.is_empty() {
                                        ui.label(RichText::new("Aucune vue placée").weak().small());
                                    }
                                    for (id, nom) in tables {
                                        if ui.selectable_label(false, format!("{nom} (nomenclature)")).on_hover_text("Clic : ouvrir la nomenclature").clicked() {
                                            ouvrir_nomenclature = Some(id.clone());
                                        }
                                    }
                                    for (n, nom, echelle) in vues {
                                        let i = indice.get(n).copied();
                                        let r = ui.selectable_label(i.is_some() && i == active, format!("{nom} · 1:{echelle}")).on_hover_text("Clic : ouvrir la vue");
                                        if r.clicked() {
                                            action = i.map(ActionNav::Activer);
                                        }
                                    }
                                });
                            let r = entete.inner;
                            r.dnd_set_drag_payload(feuilles::FeuilleGlissee(fid.clone()));
                            if r.clicked() {
                                ouvrir_jeu = Some((jeu.clone(), Some(fid.clone())));
                            }
                            // dépôt : au-dessus ou au-dessous de cette feuille selon le curseur
                            let dessous = |q: Option<egui::Pos2>| q.is_some_and(|q| q.y > r.rect.center().y);
                            if r.dnd_hover_payload::<feuilles::FeuilleGlissee>().is_some() {
                                let y = if dessous(ui.input(|i| i.pointer.hover_pos())) { r.rect.bottom() } else { r.rect.top() };
                                ui.painter().line_segment([egui::pos2(r.rect.left(), y), egui::pos2(r.rect.right(), y)], egui::Stroke::new(2.5, ui.visuals().selection.stroke.color));
                            }
                            if let Some(g) = r.dnd_hover_payload::<feuilles::FeuilleGlissee>().and_then(|_| r.dnd_release_payload::<feuilles::FeuilleGlissee>()) {
                                let avant = if dessous(ui.input(|i| i.pointer.hover_pos())) { fs.get(k + 1).map(|x| x.0.clone()) } else { Some(fid.clone()) };
                                deplacer = Some((g.0.clone(), jeu.clone(), avant));
                            }
                        }
                    });
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
        // projet natif : portes et fenêtres (familles modélisées), à ouvrir dans l'éditeur
        let familles_natives = if self.natif.is_some() { Some(self.familles_projet()) } else { None };
        let mut ouvrir_famille: Option<(Option<String>, crate::modele::GenreOuverture)> = None;
        let mut charger_famille = false;
        rubrique_nav(ui, LeftTab::Familles, "Familles".into(), false, &mut |ui| {
            if let Some(familles) = &familles_natives {
                use crate::modele::GenreOuverture as G;
                for (genre, titre) in [(G::Porte, "Portes"), (G::Fenetre, "Fenêtres")] {
                    let liste: Vec<_> = familles.iter().filter(|f| f.2 == genre && garde(&f.1)).collect();
                    egui::CollapsingHeader::new(format!("{titre} ({})", liste.len())).id_salt(("nav-ouvertures", titre)).open(ouvert.then_some(true)).show(ui, |ui| {
                        for (id, nom, g) in liste {
                            if ui.selectable_label(false, nom).on_hover_text("Clic : ouvrir la famille dans l'éditeur").clicked() {
                                ouvrir_famille = Some((Some(id.clone()), *g));
                            }
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Nouvelle porte…").clicked() {
                        ouvrir_famille = Some((None, G::Porte));
                    }
                    if ui.small_button("Nouvelle fenêtre…").clicked() {
                        ouvrir_famille = Some((None, G::Fenetre));
                    }
                    if ui.small_button("Charger une famille…").on_hover_text("Fichier .famille.json : un nouveau type dans le projet").clicked() {
                        charger_famille = true;
                    }
                });
            }
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
        if let Some((jeu, f)) = ouvrir_jeu {
            self.ouvrir_jeu(&jeu, f);
        }
        if let Some((t, genre)) = ouvrir_famille {
            self.ouvrir_editeur_famille_type(t, genre);
        }
        if charger_famille {
            if let Some(c) = rfd::FileDialog::new().add_filter("Famille", &["json"]).pick_file() {
                if let Err(e) = self.charger_famille_fichier(&c) {
                    self.error = Some(e);
                }
            }
        }
        if let Some((id, jeu, avant)) = deplacer {
            self.deplacer_feuille(&id, &jeu, avant.as_deref());
        }
        if let Some(id) = ouvrir_nomenclature {
            self.ouvrir_nomenclature(&id);
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

    fn memoriser_vue(&mut self) {
        if !self.vues_ouvertes.contains(&self.vue_active) {
            return;
        }
        self.etats_vues.insert(self.vue_active, EtatVue {
            camera: self.camera.clone(),
            hidden_classes: self.hidden_classes.clone(),
            hidden_objects: self.hidden_objects.clone(),
            hidden_storeys: self.hidden_storeys.clone(),
            isolated: self.isolated.clone(),
            style: self.style,
            faces_blanches: self.faces_blanches,
            eclairage: self.eclairage,
            traits_fins: self.traits_fins,
            ombres_portees: self.ombres_portees,
            ombre_ambiante: self.ombre_ambiante,
            coupe: self.coupe,
            coupe_niveau: self.coupe_niveau,
            coupe_hauteur: self.coupe_hauteur,
            poche: self.poche,
            echelle: self.echelle,
            detail: self.detail,
            boite: self.boite,
            boite_active: self.boite_active,
        });
    }

    fn restaurer_vue(&mut self, vue: Option<usize>) {
        let Some(e) = self.etats_vues.get(&vue).cloned() else { return };
        self.vue_active = vue;
        self.nomenclature_ouverte = vue.filter(|&i| self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)).is_some_and(|v| v.genre == "Nomenclature"));
        self.camera = e.camera;
        self.hidden_classes = e.hidden_classes;
        self.hidden_objects = e.hidden_objects;
        self.hidden_storeys = e.hidden_storeys;
        self.isolated = e.isolated;
        self.style = e.style;
        self.faces_blanches = e.faces_blanches;
        self.eclairage = e.eclairage;
        self.traits_fins = e.traits_fins;
        self.ombres_portees = e.ombres_portees;
        self.ombre_ambiante = e.ombre_ambiante;
        self.coupe = e.coupe;
        self.coupe_niveau = e.coupe_niveau;
        self.coupe_hauteur = e.coupe_hauteur;
        self.poche = e.poche;
        self.echelle = e.echelle;
        self.detail = e.detail;
        self.boite = e.boite;
        self.boite_active = e.boite_active;
        self.vol = Vol::default();
        self.point_clique = None;
        self.visibility_dirty = true;
        self.coupe_dirty = true;
        self.preparer_feuille(vue);
    }

    /// Ferme les autres vues sans fermer les documents ni perdre leurs modifications.
    fn garder_derniere_vue_par_document(&mut self) {
        for d in self.documents.values_mut() {
            if let Some(&derniere) = d.vues.last() {
                d.vues.retain(|&vue| vue == derniere);
                d.etats.retain(|&vue, _| vue == derniere);
                d.active = derniere;
            }
        }
        if self.vues_ouvertes.len() > 1 {
            if let Some(&derniere) = self.vues_ouvertes.last() {
                self.memoriser_vue();
                self.restaurer_vue(derniere);
                self.vues_ouvertes.retain(|&vue| vue == derniere);
                self.etats_vues.retain(|&vue, _| vue == derniere);
            }
        }
    }

    fn onglets_vues(&mut self, ui: &mut egui::Ui) {
        /// Identifiant de l'onglet de l'éditeur de familles (hors des documents).
        const ONGLET_FAMILLE: usize = usize::MAX;
        let mut onglets = Vec::new();
        let mut titres = BTreeMap::new();
        let mut ajouter = |id: usize, scene: Option<&Scene>, rvt: Option<&RvtInfo>, vues: &[Option<usize>], modifie: bool| {
            let fichier = scene.map(|s| s.file_name.as_str()).or_else(|| rvt.map(|r| r.file_name.as_str())).unwrap_or("Modèle");
            titres.insert(id, fichier.to_string());
            for &vue in vues {
                let nom = vue.and_then(|i| scene?.navigateur.as_ref()?.vues.get(i)).map_or("Vue 3D", |v| v.nom.as_str());
                onglets.push((id, vue, format!("{nom}{}", if modifie { " *" } else { "" }), format!("{fichier} — {nom}")));
            }
            if vues.is_empty() {
                let nom = if scene.is_some() { "Aucune vue" } else { "Fiche Revit" };
                onglets.push((id, None, nom.to_string(), fichier.to_string()));
            }
        };
        for (&id, d) in &self.documents {
            ajouter(id, d.scene.as_ref(), d.rvt.as_ref(), &d.vues, d.modifie);
        }
        ajouter(self.document_actif, self.scene.as_ref(), self.rvt.as_ref(), &self.vues_ouvertes, self.natif_modifie);
        onglets.sort_by_key(|o| o.0);
        // éditeur de familles : un onglet à part, après les vues
        // éditeur de familles : un onglet par famille ouverte (rang dans `vue`)
        for (ordre, t, _) in self.onglets_familles() {
            titres.insert(ONGLET_FAMILLE, "Éditeur de familles".into());
            onglets.push((ONGLET_FAMILLE, Some(ordre), format!("Famille {t}"), format!("Éditeur de familles — {t}")));
        }
        let famille_active = self.famille_active && self.famille.is_some();
        let ordre_actif = self.ordre_famille();
        let est_actif = |id: usize, vue: Option<usize>, doc: usize, active: Option<usize>| if id == ONGLET_FAMILLE { famille_active && vue == ordre_actif } else { !famille_active && id == doc && vue == active };
        let mut activer = None;
        let mut fermer = None;
        // Les sélecteurs disposent de leurs propres emplacements, indépendants des onglets.
        let (barre, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), Sense::hover());
        let marge = 4.0;
        let fleche = 24.0;
        let bouton_fermeture = 28.0;
        let disponible = (barre.width() - 2.0 * (marge + fleche + 6.0) - bouton_fermeture).max(1.0);
        let rect_fermeture = egui::Rect::from_min_size(
            egui::pos2(barre.right() - marge - fleche - bouton_fermeture, barre.top() + 2.0),
            egui::vec2(24.0, 22.0),
        );
        let plusieurs_vues = self.vues_ouvertes.len() > 1 || self.documents.values().any(|d| d.vues.len() > 1);
        let fermeture = ui.put(rect_fermeture, egui::Button::new("").min_size(rect_fermeture.size()).sense(
            if plusieurs_vues { Sense::click() } else { Sense::hover() },
        )).on_hover_text("Fermer les autres onglets : garder la dernière vue ouverte de chaque document");
        fermeture.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, plusieurs_vues, "Fermer les autres onglets"));
        dessiner_croix_onglet(ui.painter(), rect_fermeture.center(), if plusieurs_vues { ui.visuals().text_color() } else { ui.visuals().weak_text_color() });
        let reduire = fermeture.clicked();
        let nombre = ((disponible / 180.0).floor() as usize).max(1).min(onglets.len().max(1));
        let actif = onglets.iter().position(|o| est_actif(o.0, o.1, self.document_actif, self.vue_active)).unwrap_or(0);
        let debut = actif.saturating_sub(nombre / 2).min(onglets.len().saturating_sub(nombre));
        let largeur = (disponible / nombre as f32).min(220.0);
        for droite in [false, true] {
            let x = if droite { barre.right() - marge - fleche } else { barre.left() + marge };
            let rect = egui::Rect::from_min_size(egui::pos2(x, barre.top() + 2.0), egui::vec2(fleche, 22.0));
            ui.scope_builder(egui::UiBuilder::new().id_salt(("selecteur_vues", droite)).max_rect(rect).layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)), |ui| {
                ui.spacing_mut().button_padding = egui::Vec2::ZERO;
                let bouton = egui::Button::new("").min_size(rect.size());
                let (reponse, _) = egui::containers::menu::MenuButton::from_button(bouton).ui(ui, |ui| {
                let mut precedent = None;
                for (id, vue, nom, infobulle) in &onglets {
                    if precedent != Some(*id) {
                        if precedent.is_some() {
                            ui.separator();
                        }
                        ui.label(RichText::new(&titres[id]).strong().color(couleur_document(*id)));
                        precedent = Some(*id);
                    }
                    if ui.selectable_label(est_actif(*id, *vue, self.document_actif, self.vue_active), RichText::new(nom).color(couleur_document(*id))).on_hover_text(infobulle).clicked() {
                        activer = Some((*id, *vue));
                        ui.close();
                    }
                }
                });
                let centre = rect.center();
                let sens = if droite { 1.0 } else { -1.0 };
                let points = vec![centre + egui::vec2(-sens * 2.0, -4.0), centre + egui::vec2(sens * 2.0, 0.0), centre + egui::vec2(-sens * 2.0, 4.0)];
                ui.painter().add(egui::Shape::convex_polygon(points, ui.visuals().text_color(), egui::Stroke::NONE));
                reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Liste des onglets ouverts"));
                reponse.on_hover_text("Liste des onglets ouverts");
            });
        }
        for (k, (id, vue, nom, infobulle)) in onglets.iter().skip(debut).take(nombre).enumerate() {
                        let x = barre.left() + marge + fleche + 6.0 + k as f32 * largeur;
                        let rect = egui::Rect::from_min_size(egui::pos2(x, barre.top() + 2.0), egui::vec2((largeur - 6.0).max(1.0), 22.0));
                        let couleur = couleur_document(*id);
                        let actif = est_actif(*id, *vue, self.document_actif, self.vue_active);
                        let diluer = |c: u8| ((c as u16 + 255 * if actif { 3 } else { 5 }) / if actif { 4 } else { 6 }) as u8;
                        let fond = Color32::from_rgb(diluer(couleur.r()), diluer(couleur.g()), diluer(couleur.b()));
                        let survole = ui.rect_contains_pointer(rect);
                        ui.painter().rect_filled(rect, 4.0, fond);
                        ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(if actif { 1.5 } else { 0.5 }, couleur), egui::StrokeKind::Inside);
                        let rect_croix = egui::Rect::from_min_max(egui::pos2((rect.right() - 21.0).max(rect.left()), rect.top() + 2.0), egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0));
                        let rect_corps = egui::Rect::from_min_max(rect.min, egui::pos2(rect_croix.left(), rect.bottom()));
                        let reponse = ui.interact(rect_corps, ui.scope_id().with(("onglet_vue", id, vue)), Sense::click()).on_hover_text(infobulle);
                        let texte_rect = rect_corps.shrink2(egui::vec2(7.0, 2.0));
                        // texte non sélectionnable : le clic revient à l'onglet
                        ui.put(texte_rect, egui::Label::new(RichText::new(nom).color(ui.visuals().text_color())).truncate().selectable(false).sense(Sense::hover()));
                        if reponse.clicked() {
                            activer = Some((*id, *vue));
                        }
                        let fermer_clic = if survole {
                            let reponse = ui.put(rect_croix, egui::Button::new("").min_size(rect_croix.size()).frame(false)).on_hover_text("Fermer la vue");
                            reponse.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Fermer la vue"));
                            dessiner_croix_onglet(ui.painter(), rect_croix.center(), ui.visuals().text_color());
                            reponse.clicked()
                        } else { false };
                        if reponse.clicked_by(PointerButton::Middle) || fermer_clic {
                            fermer = Some((*id, *vue));
                        }
                        if k + 1 < nombre && onglets.get(debut + k + 1).is_some_and(|suivant| suivant.0 != *id) {
                            let x = rect.right() + 3.0;
                            ui.painter().line_segment([egui::pos2(x, rect.top() + 4.0), egui::pos2(x, rect.bottom() - 4.0)], egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                        }
        }
        if reduire {
            self.garder_derniere_vue_par_document();
        }
        if let Some((id, vue)) = activer {
            self.famille_active = id == ONGLET_FAMILLE;
            if let (ONGLET_FAMILLE, Some(ordre)) = (id, vue) {
                self.activer_famille(ordre);
            }
        }
        if let Some((id, vue)) = activer.filter(|o| o.0 != ONGLET_FAMILLE) {
            self.changer_document(id);
            if let Some(i) = vue {
                self.activer_vue(i);
            } else {
                self.memoriser_vue();
                self.restaurer_vue(None);
            }
        }
        if let Some((ONGLET_FAMILLE, Some(ordre))) = fermer {
            self.fermer_onglet_famille(ordre);
        }
        if let Some((id, cible)) = fermer.filter(|o| o.0 != ONGLET_FAMILLE) {
            let precedent = self.document_actif;
            self.changer_document(id);
            if let Some(k) = self.vues_ouvertes.iter().position(|&v| v == cible) {
                let vue = self.vues_ouvertes.remove(k);
                self.etats_vues.remove(&vue);
                if self.vue_active == vue {
                    self.nomenclature_ouverte = None;
                    if let Some(&suivante) = self.vues_ouvertes.get(k.min(self.vues_ouvertes.len().saturating_sub(1))) {
                        self.restaurer_vue(suivante);
                    }
                }
            } else {
                self.fermer();
            }
            if precedent != id {
                self.changer_document(precedent);
            }
        }
        if self.scene.is_some() && self.vues_ouvertes.is_empty() && ui.button("Ouvrir la vue 3D du modèle").clicked() {
            self.vues_ouvertes.push(None);
            self.vue_active = None;
            self.nomenclature_ouverte = None;
            self.hidden_objects.clear();
            self.hidden_storeys.clear();
            self.hidden_classes = HIDDEN_BY_DEFAULT.iter().map(|s| s.to_string()).collect();
            self.isolated = None;
            self.style = StyleVisuel::Graphique;
            self.coupe = false;
            self.boite = None;
            self.boite_active = false;
            self.camera = Camera::default();
            if let Some(s) = &self.scene {
                self.camera.scene_radius = (s.max - s.min).length() * 0.5;
            }
            self.visibility_dirty = true;
            self.coupe_dirty = true;
            self.preparer_feuille(None);
            self.update_visibility();
            self.fit_all();
        }
    }

    /// Ouvre une vue Revit comme dans Revit : caméra orthographique orientée selon la
    /// vue ; plan : son niveau seul ; coupe, élévation : tout le modèle ; vue de dessin,
    /// feuille, légende : ses éléments 2D ; vue 3D : modèle entier. Les éléments 2D de la
    /// vue sont affichés.
    fn activer_vue(&mut self, i: usize) {
        if self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)).is_none() {
            return;
        }
        if self.vue_active == Some(i) && self.vues_ouvertes.contains(&Some(i)) {
            return;
        }
        self.memoriser_vue();
        if self.etats_vues.contains_key(&Some(i)) {
            self.restaurer_vue(Some(i));
            return;
        }
        self.vues_ouvertes.push(Some(i));
        self.nomenclature_ouverte = None;
        self.boite = None;
        self.boite_active = false;
        self.coupe = false;
        self.coupe_niveau = None;
        self.vol = Vol::default();
        self.point_clique = None;
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
            "Jeu de feuilles" => {
                self.vue_active = Some(i);
                self.memoriser_vue();
                return;
            }
            "Nomenclature" => {
                if let Some(e) = v.entite {
                    self.select(Some(e), false);
                }
                self.nomenclature_ouverte = Some(i);
                self.vue_active = Some(i);
                self.memoriser_vue();
                return;
            }
            _ => (annot.clone(), annot.clone()),
        };
        let (d, haut, entite) = (v.direction.as_vec3(), v.haut.as_vec3(), v.entite);
        let genre_feuille = v.genre == "Feuille";
        let coupe_vue = v.coupe;
        let coupe_verticale = v.plan_coupe.is_some();
        // mode dessin pour les plans, coupes et élévations ; coupe à hauteur sur le niveau
        // d'un plan d'étage
        let (genre, etage, echelle) = (v.genre.clone(), v.etage, v.echelle);
        if let Some(d) = NiveauDetail::depuis_revit(v.detail) {
            self.detail = d;
        }
        match genre.as_str() {
            "Plan d'étage" | "Plan de plafond" | "Coupe" | "Élévation" => {
                // dessin au trait des plans, coupes et élévations
                self.style = StyleVisuel::Graphique;
                self.faces_blanches = true;
                self.coupe = genre == "Plan d'étage" || coupe_verticale;
                if self.coupe && !coupe_verticale {
                    self.coupe_niveau = etage;
                    // hauteur du plan de coupe de la plage de vue
                    if let Some(h) = coupe_vue {
                        self.coupe_hauteur = h as f32;
                    }
                }
            }
            // vue 3D : le style en cours, faces colorées en sortant d'un dessin
            "Vue 3D" | "Feuille" => self.faces_blanches = false,
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
        self.memoriser_vue();
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

    /// Graphismes d'un matériau du projet natif, comme dans Revit (sans ombrage) : motifs
    /// de surface et de coupe, chacun en premier plan et en arrière-plan (motif, couleur).
    /// L'arrière-plan est uni ou hachuré ; « Motifs… » ouvre l'éditeur de hachures.
    fn fenetre_materiau(&mut self, ctx: &egui::Context) {
        use crate::modele::MotifCoupe;
        let Some(nom) = self.materiau_edite.clone() else { return };
        let Some((projet, _)) = &self.natif else { self.materiau_edite = None; return };
        let Some(mut m) = projet.materiau(&nom).cloned() else { self.materiau_edite = None; return };
        let avant = m.clone();
        let hachures: Vec<MotifCoupe> = projet.noms_motifs().iter().map(|n| MotifCoupe::hachures(n)).collect();
        let mut ouvert = true;
        let mut editeur = false;
        egui::Window::new(format!("Matériau · {nom}")).id(egui::Id::unique("fenetre_materiau")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            let choix = |ui: &mut egui::Ui, id: &str, x: &mut MotifCoupe, speciaux: &[MotifCoupe]| {
                egui::ComboBox::from_id_salt(id).width(130.0).selected_text(x.libelle().to_string()).show_ui(ui, |ui| {
                    for t in speciaux.iter().chain(hachures.iter()) {
                        let l = t.libelle().to_string();
                        ui.selectable_value(x, t.clone(), l);
                    }
                });
            };
            let couleur = |ui: &mut egui::Ui, c: &mut Option<[u8; 3]>, defaut: [u8; 3]| {
                let mut v = c.unwrap_or(defaut);
                if ui.color_edit_button_srgb(&mut v).changed() {
                    *c = Some(v);
                }
            };
            egui::Grid::new("graphismes_materiau").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new("Motifs de surface").strong());
                ui.end_row();
                ui.label("Premier plan");
                let mut p = m.surface_motif.clone().unwrap_or(MotifCoupe::Vide);
                choix(ui, "surface_premier", &mut p, &[MotifCoupe::Vide, MotifCoupe::Uni]);
                m.surface_motif = (p != MotifCoupe::Vide).then_some(p);
                let d = m.encre_surface();
                couleur(ui, &mut m.surface_couleur, d);
                ui.end_row();
                ui.label("Arrière-plan");
                let mut f = m.fond_surface();
                choix(ui, "surface_arriere", &mut f, &[MotifCoupe::Uni]);
                m.surface_fond_motif = (f != MotifCoupe::Uni).then_some(f);
                ui.color_edit_button_srgb(&mut m.couleur).on_hover_text("Couleur des faces (arrière-plan uni) ; aussi celle du style Réaliste");
                ui.end_row();
                ui.label(RichText::new("Motifs de coupe").strong());
                ui.end_row();
                ui.label("Premier plan");
                choix(ui, "coupe_premier", &mut m.motif, &[MotifCoupe::Vide, MotifCoupe::Uni, MotifCoupe::Isolation]);
                let d = m.encre_coupe();
                couleur(ui, &mut m.coupe_couleur, d);
                ui.end_row();
                ui.label("Arrière-plan");
                let (mut f, teinte) = m.fond_coupe();
                choix(ui, "coupe_arriere", &mut f, &[MotifCoupe::Uni]);
                m.coupe_fond_motif = (f != MotifCoupe::Uni).then_some(f);
                couleur(ui, &mut m.coupe_fond, teinte);
                ui.end_row();
            });
            ui.add_space(4.0);
            if ui.button("Motifs de hachures…").on_hover_text("Créer et modifier les motifs").clicked() {
                editeur = true;
            }
        });
        if editeur {
            self.editeur_motifs = true;
        }
        if !ouvert {
            self.materiau_edite = None;
        }
        if m != avant {
            let mut p = projet.clone();
            if let Some(x) = p.materiaux.iter_mut().find(|x| x.nom == nom) {
                *x = m;
            }
            if let Err(e) = self.appliquer_projet(p) {
                self.error = Some(e);
            }
        }
    }

    /// Éditeur de hachures : motifs du projet, du gabarit et de base ; nom, familles de
    /// traits (angle, pas, glissement, origine, tirets en mm du papier) et aperçu. Un motif
    /// du gabarit ou de base modifié est copié dans le projet ; renommer un motif met à
    /// jour les matériaux qui s'en servent.
    /// Fenêtre « Épaisseurs de traits » : épaisseur de chaque plume (mm du papier),
    /// aperçu, couches qui l'utilisent ; table du projet, ou celle du gabarit rétablie.
    fn fenetre_plumes(&mut self, ctx: &egui::Context) {
        let Some(mut table) = self.editeur_plumes.take() else { return };
        let Some((projet, _)) = &self.natif else { return };
        let gabarit = crate::modele::Projet::default().epaisseurs_plumes();
        let actuelle = projet.epaisseurs_plumes();
        // couches (types de murs et de dalles) par plume
        let mut usages = [0usize; 12];
        for c in projet.types_murs.iter().chain(&projet.types_dalles).flat_map(|t| &t.couches) {
            usages[(c.plume().clamp(1, 12) - 1) as usize] += 1;
        }
        let (mut ouvert, mut fermer, mut appliquer) = (true, false, false);
        egui::Window::new("Épaisseurs de traits").id(egui::Id::unique("fenetre_plumes")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.label(RichText::new("Épaisseur de chaque plume en mm du papier : un trait de 0,35 mm mesure 35 mm réels au 1:100.").small().weak());
            ui.add_space(4.0);
            egui::Grid::new("grille_plumes").num_columns(4).spacing([12.0, 4.0]).striped(true).show(ui, |ui| {
                ui.label(RichText::new("Plume").strong());
                ui.label(RichText::new("Épaisseur").strong());
                ui.label(RichText::new("Aperçu").strong());
                ui.label(RichText::new("Couches").strong());
                ui.end_row();
                for (i, e) in table.iter_mut().enumerate() {
                    let n = i + 1;
                    let defaut = match n { 5 => " (structure)", 3 => " (isolation)", 2 => " (autres)", _ => "" };
                    ui.label(format!("{n}{defaut}"));
                    ui.add(egui::DragValue::new(e).speed(0.005).range(0.01..=5.0).max_decimals(2).suffix(" mm"));
                    // aperçu à l'échelle 1:1 du papier (environ 3,8 pixels par mm)
                    let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 14.0), Sense::hover());
                    ui.painter().line_segment([r.left_center(), r.right_center()], egui::Stroke::new((*e as f32 * 3.78).max(0.5), ui.visuals().text_color()));
                    ui.label(if usages[i] > 0 { usages[i].to_string() } else { "—".into() }).on_hover_text("Couches de types de murs et de dalles qui utilisent cette plume (réglée ou par défaut)");
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let change = table.iter().zip(&actuelle).any(|(a, b)| (a - b).abs() > 1e-9);
                if ui.add_enabled(change, egui::Button::new("Appliquer")).clicked() {
                    appliquer = true;
                }
                if ui.add_enabled(table.iter().zip(&gabarit).any(|(a, b)| (a - b).abs() > 1e-9), egui::Button::new("Valeurs du gabarit"))
                    .on_hover_text("Reprendre les épaisseurs du gabarit (Appliquer pour valider)")
                    .clicked()
                {
                    table = gabarit.clone();
                }
                if ui.button("Fermer").clicked() {
                    fermer = true;
                }
            });
        });
        if appliquer {
            let mut p = projet.clone();
            // table identique à celle du gabarit : le projet suit le gabarit
            p.plumes = if table.iter().zip(&gabarit).all(|(a, b)| (a - b).abs() < 1e-9) { Vec::new() } else { table.clone() };
            match self.appliquer_projet(p) {
                Ok(()) => {
                    self.coupe_dirty = true;
                    self.status = "Épaisseurs de traits appliquées".into();
                }
                Err(e) => self.error = Some(e),
            }
        }
        if ouvert && !fermer {
            self.editeur_plumes = Some(table);
        }
    }

    fn fenetre_motifs(&mut self, ctx: &egui::Context) {
        use crate::modele::{DefinitionMotif, LigneMotif, MotifCoupe};
        if !self.editeur_motifs {
            return;
        }
        let Some((projet, _)) = &self.natif else { self.editeur_motifs = false; return };
        let noms = projet.noms_motifs();
        let choisi = self.motif_choisi.clone().filter(|n| noms.contains(n)).or_else(|| noms.first().cloned());
        let Some(choisi) = choisi else { return };
        let Some(mut d) = projet.definition_motif(&MotifCoupe::hachures(&choisi)) else { return };
        let avant = d.clone();
        let du_projet = projet.motifs.iter().any(|x| x.nom == choisi);
        let de_base = DefinitionMotif::base().iter().any(|x| x.nom == choisi) || crate::gabarit::actif().motifs.iter().any(|x| x.nom == choisi);
        let utilise = projet.materiaux.iter().any(|m| [Some(&m.motif), m.surface_motif.as_ref(), m.surface_fond_motif.as_ref(), m.coupe_fond_motif.as_ref()].into_iter().flatten().any(|x| x.nom_hachures() == Some(choisi.as_str())));
        if self.tirets_saisis.0 != choisi {
            self.tirets_saisis = (choisi.clone(), d.lignes.iter().map(|l| l.tirets.iter().map(|t| crate::step::format_real(*t)).collect::<Vec<_>>().join(" ")).collect());
            self.nom_motif_saisi = choisi.clone();
        }
        let mut ouvert = true;
        let mut action: Option<&str> = None;
        let mut nouveau_choix: Option<String> = None;
        egui::Window::new("Motifs de hachures").id(egui::Id::unique("fenetre_motifs")).open(&mut ouvert).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                // liste des motifs, avec un aperçu
                ui.vertical(|ui| {
                    ui.set_width(190.0);
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for n in &noms {
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(22.0, 16.0), Sense::hover());
                                if let Some(x) = projet.definition_motif(&MotifCoupe::hachures(n)) {
                                    apercu_motif(ui.painter(), r, &x, 2.5);
                                }
                                if ui.selectable_label(*n == choisi, n).clicked() {
                                    nouveau_choix = Some(n.clone());
                                }
                            });
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Nouveau").clicked() { action = Some("nouveau"); }
                        if ui.button("Dupliquer").clicked() { action = Some("dupliquer"); }
                    });
                    let libelle = if de_base && du_projet { "Rétablir" } else { "Supprimer" };
                    let possible = du_projet && (de_base || !utilise);
                    if ui.add_enabled(possible, egui::Button::new(libelle))
                        .on_disabled_hover_text(if !du_projet { "Motif du gabarit ou de base, inchangé" } else { "Motif utilisé par un matériau" })
                        .clicked()
                    {
                        action = Some("supprimer");
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Nom");
                        let r = ui.add(egui::TextEdit::singleline(&mut self.nom_motif_saisi).desired_width(180.0));
                        if r.lost_focus() && self.nom_motif_saisi.trim() != choisi && !self.nom_motif_saisi.trim().is_empty() {
                            action = Some("renommer");
                        }
                    });
                    ui.add_space(4.0);
                    let mut retirer = None;
                    egui::Grid::new("lignes_motif").num_columns(7).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                        for t in ["Angle", "Pas", "Glissement", "Origine x", "Origine y", "Tirets (trait, blanc… mm)", ""] {
                            ui.label(RichText::new(t).small().weak());
                        }
                        ui.end_row();
                        for (k, l) in d.lignes.iter_mut().enumerate() {
                            ui.add(egui::DragValue::new(&mut l.angle).speed(1.0).range(-180.0..=180.0).suffix(" °"));
                            ui.add(egui::DragValue::new(&mut l.pas).speed(0.05).range(0.1..=100.0).max_decimals(2).suffix(" mm"));
                            ui.add(egui::DragValue::new(&mut l.glissement).speed(0.05).max_decimals(2));
                            ui.add(egui::DragValue::new(&mut l.origine[0]).speed(0.05).max_decimals(2));
                            ui.add(egui::DragValue::new(&mut l.origine[1]).speed(0.05).max_decimals(2));
                            if let Some(texte) = self.tirets_saisis.1.get_mut(k) {
                                if ui.add(egui::TextEdit::singleline(texte).desired_width(110.0).hint_text("continu")).changed() {
                                    let v: Option<Vec<f64>> = texte.replace(',', ".").split_whitespace().map(|x| x.parse::<f64>().ok().filter(|x| x.is_finite() && *x != 0.0)).collect();
                                    if let Some(v) = v { l.tirets = v; }
                                }
                            }
                            if ui.add_enabled(k > 0, egui::Button::new("Retirer").small()).on_hover_text("Retirer la famille").clicked() {
                                retirer = Some(k);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(k) = retirer {
                        d.lignes.remove(k);
                        self.tirets_saisis.1.remove(k);
                    }
                    if ui.button("+ Famille de traits").clicked() {
                        d.lignes.push(LigneMotif { angle: 0.0, pas: 2.0, glissement: 0.0, origine: [0.0, 0.0], tirets: Vec::new() });
                        self.tirets_saisis.1.push(String::new());
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new("Aperçu (3 × 3 cm du papier)").small().weak());
                    let (r, _) = ui.allocate_exact_size(egui::vec2(150.0, 150.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                    apercu_motif(ui.painter(), r, &d, 5.0);
                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
                });
            });
        });
        if !ouvert {
            self.editeur_motifs = false;
        }
        if let Some(n) = nouveau_choix {
            self.motif_choisi = Some(n);
            return;
        }
        let mut p = projet.clone();
        let mut change = false;
        let libre = |p: &crate::modele::Projet, base: &str| {
            let mut k = 1;
            loop {
                let n = if k == 1 { base.to_string() } else { format!("{base} {k}") };
                if !p.noms_motifs().contains(&n) { break n; }
                k += 1;
            }
        };
        match action {
            Some("nouveau") => {
                let n = libre(&p, "Motif");
                p.motifs.push(DefinitionMotif { nom: n.clone(), lignes: vec![LigneMotif { angle: 45.0, pas: 2.0, glissement: 0.0, origine: [0.0, 0.0], tirets: Vec::new() }] });
                self.motif_choisi = Some(n);
                change = true;
            }
            Some("dupliquer") => {
                let n = libre(&p, &format!("{} (copie)", d.nom));
                p.motifs.push(DefinitionMotif { nom: n.clone(), ..d.clone() });
                self.motif_choisi = Some(n);
                change = true;
            }
            Some("supprimer") => {
                p.motifs.retain(|x| x.nom != choisi);
                self.tirets_saisis.0.clear();
                change = true;
            }
            Some("renommer") => {
                let n = self.nom_motif_saisi.trim().to_string();
                if p.noms_motifs().contains(&n) {
                    self.error = Some(format!("Un motif s'appelle déjà « {n} »."));
                } else {
                    // copie renommée ; les matériaux qui se servaient de l'ancien suivent
                    p.motifs.retain(|x| x.nom != choisi);
                    p.motifs.push(DefinitionMotif { nom: n.clone(), ..d.clone() });
                    let (ancien, neuf) = (MotifCoupe::hachures(&choisi), MotifCoupe::hachures(&n));
                    for m in &mut p.materiaux {
                        for x in [&mut m.motif].into_iter().chain([&mut m.surface_motif, &mut m.surface_fond_motif, &mut m.coupe_fond_motif].into_iter().flatten()) {
                            if *x == ancien { *x = neuf.clone(); }
                        }
                    }
                    self.motif_choisi = Some(n);
                    change = true;
                }
            }
            _ if d != avant => {
                match p.motifs.iter_mut().find(|x| x.nom == choisi) {
                    Some(x) => *x = d,
                    None => p.motifs.push(d),
                }
                change = true;
            }
            _ => {}
        }
        if change && let Err(e) = self.appliquer_projet(p) {
            self.error = Some(e);
        }
    }

    /// Propriétés de l'élément sélectionné (panneau de gauche, comme dans Revit).
    fn right_panel(&mut self, ui: &mut egui::Ui) {
        self.fenetre_materiau(ui.ctx());
        self.fenetre_motifs(ui.ctx());
        self.fenetre_position_plan(ui.ctx());
        self.fenetre_plumes(ui.ctx());
        self.entete_panneau(ui, Panneau::Proprietes, "Propriétés");
        // sélecteur de type de l'élément sélectionné, en tête comme dans Revit
        if self.jeu_actif().is_none() {
            if let Some(projet) = self.selecteur_type_selection(ui) {
                if let Err(e) = self.appliquer_projet(projet) { self.error = Some(e); }
            }
        }
        if let Some(jeu) = self.jeu_actif() {
            egui::ScrollArea::vertical().show(ui, |ui| self.proprietes_jeu(ui, &jeu));
            return;
        }
        if self.natif.is_some() {
            let ids = self.selection_bim();
            if ids.is_empty() {
                let mut contexte = self.natif.as_ref().unwrap().0.clone();
                if self.ruban_edition.contexte_creation(ui, &mut contexte) {
                    if let Err(e) = self.appliquer_projet(contexte) { self.error = Some(e); }
                }
            } else { self.contexte_selection(ui, &ids); }
            ui.separator();
        }
        if self.panneau_bim(ui) { return; }
        if self.natif.is_some() {
            let projet = &self.natif.as_ref().unwrap().0;
            // éditeur de type de mur : un type déjà porté par des murs les met à jour
            let mut copie = if self.ruban_edition.editeur_type_ouvert() { projet.clone() } else { crate::modele::Projet::default() };
            if self.ruban_edition.editeur_type_ouvert() && self.ruban_edition.editeur_type_mur(ui.ctx(), &mut copie) {
                if let Err(e) = self.appliquer_projet(copie) { self.error = Some(e); }
            } else if let Some(t) = self.ruban_edition.prendre_type_gabarit()
                && let Some((projet, _)) = &self.natif
            {
                let mut copie = projet.clone();
                let r = self.ruban_edition.definir_type_mur(&mut copie, &t.nom, t.couches).and_then(|_| self.appliquer_projet(copie));
                if let Err(e) = r { self.error = Some(e); }
            }
        }
        if self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.proprietes_mur(ui, self.natif.as_ref().map(|(p, _)| p)) {
            return;
        }
        if self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.proprietes_surfaces(ui, self.natif.as_ref().map(|(p, _)| p)) {
            return;
        }
        if self.onglet_ruban == OngletRuban::Annoter
            && let Some(choix) = self.ruban_edition.proprietes_vues(ui, self.natif.as_ref().map(|(p, _)| p))
        {
            if let Some((niveau, hauteur)) = choix {
                self.creer_vue(|p| p.ajouter_vue_plan(&niveau, hauteur), true);
            }
            return;
        }
        // sans navigation IFC, un modèle Revit ou natif n'expose pas ses données IFC
        let ifc = self.options.navigation_ifc || self.scene.as_ref().is_some_and(|s| s.navigateur.is_none());
        if let (Some(s), Some(id)) = (&self.scene, self.selected) {
            // en-tête façon Revit : classe (IFC) et type de l'élément
            let ty = s.model.type_of.get(&id).map(|t| s.model.name(*t)).filter(|t| !t.is_empty());
            let classe = pretty_type(s.model.ty(id)).trim_start_matches("Ifc").to_string();
            let entete = match (ifc, ty) {
                (true, Some(t)) => Some(format!("{classe} : {t}")),
                (true, None) => Some(classe),
                (false, t) => t,
            };
            // projet natif : le type est dans le sélecteur en tête du panneau
            if let Some(e) = entete.filter(|_| self.natif.is_none() || ifc) {
                ui.label(RichText::new(e).color(Color32::from_gray(70)));
                ui.separator();
            }
        }
        let element_selectionne = self.selected.is_some() || !self.selection_bim().is_empty();
        if self.natif.is_some() && !element_selectionne {
            self.proprietes_vue_native(ui);
        }
        if !element_selectionne && self.scene.is_some() && (self.coupe || !self.vue_3d()) {
            self.plage_de_vue(ui);
            ui.separator();
        }
        if self.boite_active {
            self.proprietes_boite(ui);
            ui.separator();
        }
        let (Some(_), Some(id)) = (&self.scene, self.selected) else {
            if self.bim.selection.is_empty() {
                ui.label(RichText::new("Sélectionner un élément dans la vue ou l'arborescence.").weak());
            } else if self.onglet_ruban != OngletRuban::Modifier {
                ui.label(RichText::new(format!("{} éléments sélectionnés", self.bim.selection.len())).weak());
            }
            return;
        };
        if self.natif.is_some() {
            if let Some(projet) = self.proprietes_natif(ui, id) {
                if let Err(e) = self.appliquer_projet(projet) { self.error = Some(e); }
                return;
            }
        }
        let Some(s) = &self.scene else { return };
        let model = &s.model;
        let has_obj = s.object_of.contains_key(&id);
        let under = self.objects_under(id);
        if !has_obj && under.is_empty() {
            ui.label(RichText::new("Pas de géométrie affichable.").weak());
        }
        ui.add_space(4.0);
        let mut attrs = model.attributes(id);
        if !ifc {
            attrs.retain(|(k, _)| !matches!(k.as_str(), "Classe" | "Id STEP" | "GlobalId"));
        }
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
    }

    /// Cadrer, Isoler ou Masquer (ruban Modifier) : objets de la sélection courante.
    fn visibilite_selection(&mut self, o: crate::ruban::Outil) {
        use crate::ruban::Outil;
        let mut objets: Vec<u32> = self.selected.into_iter().chain(self.selection_multiple.iter().copied()).flat_map(|id| self.objects_under(id)).collect();
        objets.sort_unstable();
        objets.dedup();
        match o {
            Outil::Cadrer if objets.is_empty() => self.fit_all(),
            Outil::Cadrer => self.frame_objects(&objets),
            _ if objets.is_empty() => self.status = "Sélectionner d'abord un élément.".into(),
            Outil::Isoler => self.isolate(objets),
            _ => self.hide(objets),
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
                self.fermer();
            }
        }
    }

    /// Tableau d'une nomenclature Revit, recomposé depuis les éléments lus ; les colonnes
    /// que Revit calcule (formules, pièces d'une porte…) sont signalées.
    fn vue_nomenclature(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.nomenclature_ouverte else { return };
        let Some(v) = self.scene.as_ref().and_then(|s| s.navigateur.as_ref()).and_then(|n| n.vues.get(i)) else { return };
        let Some((titres, lignes, nc, tris)) = &v.tableau else { return };
        ui.vertical(|ui| {
            ui.heading(&v.nom);
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
        let mode_mur = self.onglet_ruban == OngletRuban::Modeliser && self.ruban_edition.mur_actif() && self.natif.is_some() && self.loading.is_none();
        // dalle (ou sol) : contour cliqué sur le plan de son dessus
        let dalle = if self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.loading.is_none() { self.ruban_edition.dalle_active() } else { None };
        // coupe : ligne à deux clics, sur le plan du niveau de la vue ouverte
        let outil_coupe = if self.onglet_ruban == OngletRuban::Annoter && self.natif.is_some() && self.loading.is_none() { self.ruban_edition.coupe_active() } else { None };
        // une coupe se trace dans une vue en plan (ou une vue de dessus)
        let coupe_hors_plan = outil_coupe.is_some() && !self.vue_en_plan();
        let outil_coupe = outil_coupe.filter(|_| !coupe_hors_plan);
        if coupe_hors_plan {
            ui.painter().text(rect.left_top() + egui::vec2(12.0, 34.0), egui::Align2::LEFT_TOP, "Coupe / élévation : ouvrir une vue en plan pour tracer la ligne", egui::FontId::proportional(13.0), Color32::from_rgb(55, 130, 200));
        }
        if outil_coupe.is_none() { self.coupe_depart = None; }
        let altitude_coupe = self.altitude_vue_native();
        let structure = if self.onglet_ruban == OngletRuban::Modeliser && self.natif.is_some() && self.loading.is_none() {
            match self.ruban_edition.outil() {
                Some(crate::ruban::Outil::Poteau) => Some(crate::modele::GenreBarre::Poteau),
                Some(crate::ruban::Outil::Poutre) => Some(crate::modele::GenreBarre::Poutre),
                _ => None,
            }
        } else { None };
        if structure != Some(crate::modele::GenreBarre::Poutre) { self.bim.depart_structure = None; }
        let altitude_structure = structure.and_then(|_| {
            let p = &self.natif.as_ref()?.0;
            Some(p.niveau(&self.bim.barre.niveau)?.altitude + self.bim.barre.debut[2])
        });
        let mode_trace = mode_mur || dalle.is_some() || outil_coupe.is_some() || structure.is_some();
        // axe du mur sélectionné : une extrémité saisie se déplace (accrochages du tracé)
        let axe = if mode_trace { None } else { self.axe_selection() };
        if let Some((id, a, b, z)) = &axe {
            let (ea, eb) = (self.ecran_plan(rect, *a, *z), self.ecran_plan(rect, *b, *z));
            if resp.drag_started_by(PointerButton::Primary)
                && let Some(q) = ui.input(|i| i.pointer.press_origin())
            {
                let proche = |e: Option<egui::Pos2>| e.is_some_and(|e| e.distance(q) <= 9.0);
                if proche(eb) {
                    self.poignee_axe = Some((id.clone(), true, *b));
                } else if proche(ea) {
                    self.poignee_axe = Some((id.clone(), false, *a));
                }
            }
            if let (Some((_, fin, _)), Some(q)) = (&self.poignee_axe, resp.interact_pointer_pos().or(resp.hover_pos())) {
                let fixe = if *fin { *a } else { *b };
                let shift = ui.input(|i| i.modifiers.shift);
                if let Some((p, _)) = self.point_tracage(q, rect, shift, Some(fixe), *z)
                    && let Some(x) = &mut self.poignee_axe
                {
                    x.2 = p;
                }
            }
        } else {
            self.poignee_axe = None;
        }
        if resp.drag_stopped() && let Some((id, fin, p)) = self.poignee_axe.take() {
            if let Some((projet, _)) = &self.natif {
                let mut copie = projet.clone();
                match copie.deplacer_extremite(&id, fin, p).and_then(|_| self.appliquer_projet(copie)) {
                    Ok(()) => self.status = "Mur modifié — Ctrl+S pour enregistrer.".into(),
                    Err(e) => self.error = Some(e),
                }
            }
        }
        let mode_trace = mode_trace || self.poignee_axe.is_some();
        if mode_trace && resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair); }
        let ndc = |p: egui::Pos2| {
            Vec2::new((p.x - rect.left()) / rect.width() * 2.0 - 1.0, 1.0 - (p.y - rect.top()) / rect.height() * 2.0)
        };

        let repere_plan = self.scene.is_some() && self.vue_en_plan();
        let sur_repere_plan = repere_plan && ui.input(|i| {
            let zone = crate::orientation_plan::zone(rect);
            i.pointer.hover_pos().is_some_and(|p| zone.contains(p))
                || (i.pointer.any_down() && i.pointer.press_origin().is_some_and(|p| zone.contains(p)))
        });
        if self.scene.is_some() && !sur_repere_plan {
            let pick = |app: &Self, p: egui::Pos2| -> Option<(u32, Vec3)> {
                let s = app.scene.as_ref()?;
                let n = ndc(p);
                let (o, d) = app.camera.ray(n.x, n.y, aspect);
                s.pick(o, d, &app.visible, app.plan_coupe(), app.boite_appliquee()).map(|(obj, t)| (obj, o + d * t))
            };

            if mode_trace { self.fenetre_selection = None; }
            if !mode_trace && self.fenetre_selection.is_none() && resp.drag_started_by(PointerButton::Primary) {
                if let Some(debut) = ui.input(|i| i.pointer.press_origin()) {
                    if pick(self, debut).is_none() {
                        self.fenetre_selection = Some(FenetreSelection { debut, fin: debut, glissement: true, ajouter: ui.input(|i| i.modifiers.ctrl || i.modifiers.command) });
                    }
                }
            }
            let rectangle_actif = self.fenetre_selection.is_some();
            // Orbite autour du point visé (vue 3D seulement).
            if !mode_trace && !rectangle_actif && resp.drag_started_by(PointerButton::Primary) && !ui.input(|i| i.modifiers.shift) && self.vue_3d() {
                if let Some((_, p)) = resp.interact_pointer_pos().and_then(|pp| pick(self, pp)) {
                    self.camera.set_pivot(p);
                }
            }
            let shift = ui.input(|i| i.modifiers.shift);
            if !mode_trace && !rectangle_actif { self.voler(ui, &resp); } else { self.vol.actif = false; }
            for b in [PointerButton::Primary, PointerButton::Secondary, PointerButton::Middle] {
                if (mode_trace || rectangle_actif) && b != PointerButton::Middle { continue; }
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

            if rectangle_actif {
                let curseur = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
                if let Some(p) = curseur {
                    if let Some(f) = &mut self.fenetre_selection { f.fin = p.clamp(rect.min, rect.max); }
                }
                let annuler = ui.input(|i| i.key_pressed(egui::Key::Escape)) || resp.clicked_by(PointerButton::Secondary);
                if annuler { self.fenetre_selection = None; }
                else if self.fenetre_selection.is_some_and(|f| (f.glissement && resp.drag_stopped_by(PointerButton::Primary)) || (!f.glissement && resp.clicked())) {
                    if let Some(f) = self.fenetre_selection.take() { self.selectionner_rectangle(rect, f); }
                }
            } else if self.outil_dessin().is_some() {
                self.souris_dessin(ui, rect, &resp);
            } else if self.alignement_plan {
                // Aligner (repère du plan) : l'arête visée (surlignée) devient horizontale ;
                // sans arête, la direction principale de l'élément
                let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
                let hit = curseur.and_then(|p| pick(self, p).map(|(obj, _)| (p, obj)));
                self.arete_alignement = hit.and_then(|(p, obj)| self.arete_visee(p, rect, obj));
                if resp.clicked() {
                    let d = match self.arete_alignement {
                        Some([a, b]) => Some(glam::Vec2::new(b.x - a.x, b.y - a.y)),
                        None => hit.and_then(|(_, obj)| self.direction_principale(obj)),
                    };
                    match d.and_then(|d| crate::orientation_plan::angle_pour_horizontale(&self.camera, d)) {
                        Some(a) => {
                            crate::orientation_plan::orienter(&mut self.camera, a);
                            self.memoriser_vue();
                            self.alignement_plan = false;
                            self.arete_alignement = None;
                        }
                        None => self.status = "Aligner : viser une arête du plan (Échap : annuler)".into(),
                    }
                }
            } else if let Some(genre) = structure {
                if resp.clicked_by(PointerButton::Secondary) { self.bim.depart_structure = None; }
                if resp.clicked() {
                    let point = altitude_structure.and_then(|alt| resp.interact_pointer_pos().and_then(|q| self.point_tracage(q, rect, shift, self.bim.depart_structure, alt)));
                    if let Some((point, _)) = point {
                        match genre {
                            crate::modele::GenreBarre::Poteau => {
                                if let Err(e) = self.placer_structure(point, None) { self.error = Some(e); }
                            }
                            crate::modele::GenreBarre::Poutre => {
                                if let Some(debut) = self.bim.depart_structure {
                                    match self.placer_structure(debut, Some(point)) {
                                        Ok(()) => self.bim.depart_structure = None,
                                        Err(e) => self.error = Some(e),
                                    }
                                } else { self.bim.depart_structure = Some(point); }
                            }
                        }
                    } else {
                        self.error = Some(if altitude_structure.is_none() { "Choisissez un niveau pour la structure." } else { "Le plan de placement est parallèle à la vue. Utilisez une vue de dessus ou une vue 3D inclinée." }.into());
                    }
                }
            } else if let Some(prm) = &outil_coupe {
                if resp.clicked_by(PointerButton::Secondary) { self.coupe_depart = None; }
                if resp.clicked()
                    && let Some((p, _)) = resp.interact_pointer_pos().and_then(|q| self.point_tracage(q, rect, shift, self.coupe_depart, altitude_coupe))
                {
                    match self.coupe_depart.take() {
                        Some(a) => self.creer_vue(|projet| projet.ajouter_vue_coupe(a, p, prm.profondeur, prm.elevation, &prm.type_vue, prm.echelle), false),
                        None => self.coupe_depart = Some(p),
                    }
                }
            } else if resp.double_clicked() {
                match resp.interact_pointer_pos().and_then(|p| pick(self, p)) {
                    Some((obj, _)) => self.frame_objects(&[obj]),
                    None => self.fit_all(),
                }
            } else if let Some((c, k)) = resp.clicked().then(|| resp.interact_pointer_pos()).flatten().and_then(|q| {
                self.aretes_pente(rect).into_iter().find(|a| a.2.distance(q) <= 9.0).map(|a| (a.0, a.1))
            }) {
                // repère d'arête de l'élément sélectionné : la pente est ajoutée ou retirée
                self.basculer_pente(c, k);
            } else if resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| self.rectangles_annotations(rect).iter().any(|(_, r)| r.expand(4.0).contains(q))) {
                let q = resp.interact_pointer_pos().unwrap();
                let id = self.rectangles_annotations(rect).into_iter().find(|(_, r)| r.expand(4.0).contains(q)).unwrap().0;
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let mut selection = if ctrl { self.selection_bim() } else { Vec::new() };
                if let Some(i) = selection.iter().position(|x| x == &id) { selection.remove(i); } else { selection.push(id); }
                self.select(None, false);
                self.bim.selection = selection;
            } else if resp.clicked() && self.vue_en_plan() && resp.interact_pointer_pos().and_then(|q| self.repere_coupe_proche(rect, q, altitude_coupe)).is_some() {
                let id = self.repere_coupe_proche(rect, resp.interact_pointer_pos().unwrap(), altitude_coupe).unwrap();
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                let mut selection = if ctrl { self.selection_bim() } else { Vec::new() };
                if let Some(i) = selection.iter().position(|x| x == &id) { selection.remove(i); } else { selection.push(id); }
                self.select(None, false);
                self.bim.selection = selection;
            } else if resp.clicked() {
                let hit = resp.interact_pointer_pos().and_then(|p| pick(self, p));
                let id = hit.map(|(obj, _)| self.cible_clic(self.scene.as_ref().unwrap().objects[obj as usize].id));
                self.point_clique = hit.map(|(_, p)| p.as_dvec3() + self.scene.as_ref().unwrap().origin);
                if self.natif.is_some() && self.ruban_edition.outil()==Some(crate::ruban::Outil::Ouverture) {
                    if let (Some(id),Some(p))=(id,self.point_clique) { self.placer_ouverture(id,[p.x*1000.0,p.y*1000.0]); }
                } else if id.is_none() {
                    // clic dans le vide : désélectionne (comme Revit), puis un second clic
                    // ferme une fenêtre de sélection
                    let ajouter = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    if !ajouter {
                        self.select(None, false);
                    }
                    if let Some(debut) = resp.interact_pointer_pos() {
                        self.fenetre_selection = Some(FenetreSelection { debut, fin: debut, glissement: false, ajouter });
                    }
                } else {
                    if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) && self.natif.is_none() {
                        let mut ids = self.selection_multiple.clone();
                        if ids.is_empty() { if let Some(sel) = self.selected { ids.push(sel); } }
                        if let Some(id) = id { if let Some(pos) = ids.iter().position(|x| *x == id) { ids.remove(pos); } else { ids.push(id); } }
                        self.select(ids.first().copied(), true);
                        self.selection_multiple = ids;
                    } else if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
                        let mut selection=self.selection_bim();
                        if let Some(cle)=id.and_then(|id|self.cle_native(id)) {
                            if selection.contains(&cle){selection.retain(|s|s!=&cle);}else{selection.push(cle);}
                        }
                        self.select(if selection.is_empty(){None}else{id},true);
                        self.bim.selection=selection;
                    } else {self.select(id,true);}
                }
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
                if esc && !rectangle_actif {
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
        if let Some(f) = self.fenetre_selection {
            let r = egui::Rect::from_two_pos(f.debut, f.fin);
            let croisement = f.fin.x < f.debut.x;
            let couleur = if croisement { Color32::from_rgb(35, 160, 75) } else { Color32::from_rgb(35, 110, 220) };
            let painter = ui.painter().with_clip_rect(rect);
            painter.rect_filled(r, 0.0, couleur.gamma_multiply(0.18));
            if croisement {
                let points = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.2, couleur), 5.0, 4.0));
            } else { painter.rect_stroke(r, 0.0, egui::Stroke::new(1.2, couleur), egui::StrokeKind::Inside); }
        }
        if self.natif.is_some() && self.vue_en_plan() {
            self.reperes_coupes(ui, rect, altitude_coupe);
        }
        if let Some(prm) = &outil_coupe {
            let point = resp.hover_pos().and_then(|q| self.point_tracage(q, rect, ui.input(|i| i.modifiers.shift), self.coupe_depart, altitude_coupe));
            self.apercu_coupe(ui, rect, point, altitude_coupe, prm);
        }
        if let Some(genre) = structure {
            let painter = ui.painter().with_clip_rect(rect);
            let couleur = Color32::from_rgb(55, 130, 200);
            let texte = match (genre, self.bim.depart_structure) {
                (crate::modele::GenreBarre::Poteau, _) => "Poteau : cliquer pour placer · molette : zoom",
                (_, None) => "Poutre : cliquer le départ · molette : zoom",
                (_, Some(_)) => "Poutre : cliquer l'arrivée · Maj : orthogonal · Échap ou clic droit : annuler",
            };
            painter.text(rect.left_top()+egui::vec2(12.0,34.0), egui::Align2::LEFT_TOP, texte, egui::FontId::proportional(13.0), couleur);
            if let Some(alt) = altitude_structure {
                let point = resp.hover_pos().and_then(|q| self.point_tracage(q, rect, ui.input(|i| i.modifiers.shift), self.bim.depart_structure, alt));
                if let Some((p, accroche)) = point {
                    if let Some(q) = self.ecran_plan(rect, p, alt) { marqueur(&painter, q, accroche, couleur, Color32::from_rgb(230, 120, 20)); }
                    if let Some(a) = self.bim.depart_structure {
                        if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, alt), self.ecran_plan(rect, p, alt)) {
                            painter.line_segment([x, y], egui::Stroke::new(2.0, couleur));
                            painter.text(x.lerp(y, 0.5) + egui::vec2(0.0, -16.0), egui::Align2::CENTER_CENTER, crate::options::unite().afficher((p[0]-a[0]).hypot(p[1]-a[1])), egui::FontId::proportional(13.0), couleur);
                        }
                    }
                }
            }
        }
        if dalle.is_none() && !mode_mur && outil_coupe.is_none() && structure.is_none() {
            self.dessiner_aretes_pente(ui, rect, resp.hover_pos());
            self.dessiner_axe(ui, rect, resp.hover_pos());
        }
        if self.alignement_plan {
            self.dessiner_arete_alignement(ui, rect);
        }
        if self.outil_dessin().is_some() {
            self.dessiner_dessin2d(ui, rect);
        }
        if self.usage_dessin().is_some_and(|(u, _)| u != dessin2d_ui::Usage::Annotation) {
            egui::Area::new(egui::Id::unique("options_trace_mur"))
                .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0))
                .constrain_to(rect)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| self.options_trace(ui));
                });
        }
        if self.a_options_modifier() {
            egui::Area::new(egui::Id::unique("options_modifier"))
                .fixed_pos(rect.left_top() + egui::vec2(8.0, 8.0))
                .constrain_to(rect)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| self.options_modifier(ui));
                });
        }
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
        if repere_plan {
            let origine = self.orientation_initiale_plan();
            let positions = self.positions_plan_liste();
            let r = crate::orientation_plan::afficher(ui, rect, &mut self.camera, origine, self.alignement_plan, &positions);
            if r.aligner {
                self.alignement_plan = !self.alignement_plan;
                self.arete_alignement = None;
            }
            if r.nouvelle {
                self.nom_position_plan = Some(format!("Position {}", positions.len() + 1));
            }
            if let Some(i) = r.supprimer {
                self.supprimer_position_plan(i);
            }
            if r.change {
                self.memoriser_vue();
            }
        } else if self.scene.is_some() && self.vue_3d() {
            self.cube_orientation(ui, rect);
        }

        if self.scene.is_none() {
            let msg = if self.loading.is_some() { self.status.clone() } else { "Glisser un fichier .ifc ou .rvt ici\nou Ctrl+O".to_string() };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(20.0), Color32::from_gray(110));
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
            selected: self.objets_selectionnes(),
            background: if self.style.papier() { [1.0, 1.0, 1.0] } else { [0.925, 0.933, 0.945] },
            style: self.style,
            faces_blanches: self.faces_blanches,
            eclairage: self.eclairage,
            mm: (self.echelle / 1000.0) as f32,
            table_motifs: self.scene.as_ref().map(|s| s.table_motifs.clone()).unwrap_or_default(),
            soleil: self.ombres_portees.then(|| self.soleil(aspect)).flatten(),
            // occultation : rayon de l'ordre du mètre, réduit de près, élargi de loin
            ao: self.ombre_ambiante.then(|| (self.camera.proj(aspect), (self.camera.dist * 0.04).clamp(0.15, 1.5))),
            coupe: self.plan_coupe(),
            boite: self.boite_appliquee(),
            cadrage: self.cadrage_vue(),
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
            ("enregistrement", self.sauvegarde.is_some().to_string()),
            ("sortie_v0", self.natif.as_ref().map(|n| &n.1).or(self.rvt_sortie.as_ref()).map_or("null".into(), |p| texte(&p.to_string_lossy()))),
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
                ("actif", self.plan_coupe().is_some().to_string()),
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
        if self.natif.is_some() {
            p.push(("selection_native", liste(self.selection_bim().iter().map(|id| texte(id)))));
        }
        if !self.selection_multiple.is_empty() {
            p.push(("selections", liste(self.selection_multiple.iter().map(|id| self.decrire(*id)))));
        }
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
            // famille_enregistrer|famille_ouvrir|famille_charger <chemin> : fichier de famille
            "famille_enregistrer" | "famille_ouvrir" | "famille_charger" => {
                let chemin = std::path::Path::new(argument.trim());
                let r = match commande {
                    "famille_enregistrer" => self.enregistrer_famille_fichier(chemin).map(|_| String::new()),
                    "famille_ouvrir" => self.ouvrir_famille_fichier(chemin).map(|_| String::new()),
                    _ => self.charger_famille_fichier(chemin),
                };
                match r { Ok(id) => succes(&[("id", texte(&id))]), Err(e) => erreur(&e) }
            }
            "editer" => {
                let resultat=serde_json::from_str::<crate::modele::commandes::Commande>(argument).map_err(|e|e.to_string()).and_then(|c|self.executer_bim(c));
                match resultat { Ok(ids)=>succes(&[("ids",serde_json::to_string(&ids).unwrap())]), Err(e)=>erreur(&e) }
            }
            "annuler" | "retablir" => {
                self.error=None; self.parcourir_historique(commande=="retablir");
                match &self.error { Some(e)=>erreur(e),None=>self.etat() }
            }
            "projet_natif" => match &self.natif {Some((p,_))=>match p.json() {Ok(s)=>succes(&[("projet",s)]),Err(e)=>erreur(&e)},None=>erreur("Aucun projet natif.")},
            "outil" => {
                match argument {
                    "mur" => { self.onglet_ruban = OngletRuban::Modeliser; self.ruban_edition.activer_mur(); }
                    "poteau" | "poutre" => { self.onglet_ruban = OngletRuban::Modeliser; self.ruban_edition.activer_structure(argument == "poutre"); }
                    "coupe" => { self.onglet_ruban = OngletRuban::Annoter; self.ruban_edition.activer_coupe(); }
                    "dalle" | "sol" | "plafond" | "toiture" => {
                        use crate::modele::SorteDalle;
                        let sorte = match argument { "sol" => SorteDalle::Sol, "plafond" => SorteDalle::Plafond, "toiture" => SorteDalle::Toiture, _ => SorteDalle::Dalle };
                        self.onglet_ruban = OngletRuban::Modeliser;
                        self.ruban_edition.activer_surface(sorte);
                    }
                    "ligne" | "polyligne" | "spline" | "cercle" | "arc" => {
                        use crate::ruban::Outil;
                        let o = match argument { "ligne" => Outil::Ligne, "polyligne" => Outil::Polyligne, "spline" => Outil::Spline, "cercle" => Outil::Cercle, _ => Outil::Arc };
                        self.onglet_ruban = OngletRuban::Annoter;
                        self.ruban_edition.activer_outil(o);
                    }
                    "aucun" => { self.ruban_edition.annuler(); }
                    _ => return erreur("usage : outil mur|poteau|poutre|dalle|sol|coupe|ligne|polyligne|spline|cercle|arc|aucun"),
                }
                self.etat()
            }
            "vue_plan" => {
                // vue_plan <nom ou id du niveau> [hauteur de coupe mm]
                let (nom, h) = argument.rsplit_once(' ').and_then(|(n, h)| Some((n, h.parse::<f64>().ok()?))).unwrap_or((argument, 1200.0));
                let Some(niveau) = self.natif.as_ref().and_then(|(p, _)| p.niveaux.iter().find(|n| n.nom == nom.trim() || n.id == nom.trim())).map(|n| n.id.clone()) else { return erreur("niveau introuvable") };
                self.creer_vue(|p| p.ajouter_vue_plan(&niveau, h), true);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "coupe_vue" => {
                // coupe_vue <x0 y0 x1 y1> [profondeur mm] [elevation] (mm)
                let elevation = argument.contains("elevation");
                let Ok(v) = argument.split_whitespace().filter(|m| *m != "elevation").map(str::parse::<f64>).collect::<Result<Vec<_>, _>>() else { return erreur("usage : coupe_vue x0 y0 x1 y1 [profondeur] [elevation]") };
                if v.len() != 4 && v.len() != 5 { return erreur("usage : coupe_vue x0 y0 x1 y1 [profondeur] [elevation]") }
                let p = v.get(4).copied().unwrap_or(if elevation { 30_000.0 } else { 15_000.0 });
                let (nom, echelle) = if elevation { ("Élévation extérieure", 100.0) } else { ("Coupe de bâtiment", 100.0) };
                self.creer_vue(|projet| projet.ajouter_vue_coupe([v[0], v[1]], [v[2], v[3]], p, elevation, nom, echelle), false);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "dalle" => {
                // dalle <x y> <x y> <x y> … (mm) : type, niveau et décalage de l'outil Dalle / Sol
                let Ok(v) = argument.split_whitespace().map(str::parse::<f64>).collect::<Result<Vec<_>, _>>() else { return erreur("coordonnées de la dalle invalides") };
                if v.len() < 6 || v.len() % 2 != 0 { return erreur("usage : dalle <x0 y0> <x1 y1> <x2 y2> … (mm)") }
                let pts: Vec<[f64; 2]> = v.chunks(2).map(|c| [c[0], c[1]]).collect();
                self.onglet_ruban = OngletRuban::Modeliser;
                let Some(parametres) = self.ruban_edition.dalle_active().or_else(|| { self.ruban_edition.activer_dalle(); self.ruban_edition.dalle_active() }) else { return erreur("outil Dalle indisponible") };
                self.creer_dalle(&pts, &parametres);
                match &self.error { Some(e) => erreur(e), None => self.etat() }
            }
            "surface" => {
                // surface type <nom> | surface pente oui|non | surface <paramètre> <valeur> (outil actif)
                let (cle, reste) = argument.split_once(' ').map_or((argument, ""), |(a, b)| (a, b.trim()));
                let r = match cle {
                    "type" => self.ruban_edition.choisir_type_surface(reste),
                    "pente" => { self.ruban_edition.definir_pente_toiture(reste != "non"); Ok(()) }
                    _ => match reste.parse::<f64>() {
                        Ok(v) => self.ruban_edition.regler_surface(cle, v),
                        Err(_) => Err("usage : surface type <nom> | surface pente oui|non | surface <paramètre> <valeur>".into()),
                    },
                };
                match r { Ok(()) => self.etat(), Err(e) => erreur(&e) }
            }
            "pente" => {
                // pente <id|dernier> <arête 1…n|toutes> <degrés|aucune>
                let v: Vec<&str> = argument.split_whitespace().collect();
                let [cible, arete, valeur] = v[..] else { return erreur("usage : pente <id|dernier> <arête|toutes> <degrés|aucune>") };
                let Some((projet, _)) = &self.natif else { return erreur("pas de projet natif ouvert") };
                let mut projet = projet.clone();
                let Some(d) = (if cible == "dernier" { projet.dalles.last_mut() } else { projet.dalles.iter_mut().find(|d| d.id == cible) }) else { return erreur("élément horizontal introuvable") };
                let pente = if valeur == "aucune" { None } else {
                    match valeur.parse::<f64>() { Ok(a) if a > 0.0 && a < 89.0 => Some(a), _ => return erreur("pente : degrés entre 0 et 89, ou « aucune »") }
                };
                let n = d.contours.first().map_or(0, |c| c.len());
                let aretes: Vec<usize> = if arete == "toutes" { (0..n).collect() } else {
                    match arete.parse::<usize>() { Ok(k) if k >= 1 && k <= n => vec![k - 1], _ => return erreur("numéro d'arête invalide") }
                };
                for k in aretes { d.definir_pente(0, k, pente); }
                match self.appliquer_projet(projet) { Ok(()) => self.etat(), Err(e) => erreur(&e) }
            }
            "type_mur" => {
                // type_mur <nom>|<matériau>:<épaisseur mm>[:structure|isolation|finition]|… (extérieur → intérieur)
                use crate::modele::{Couche, FonctionCouche};
                let mut parties = argument.split('|');
                let nom = parties.next().unwrap_or("").trim().to_string();
                let couches: Option<Vec<Couche>> = parties.map(|c| {
                    let mut x = c.split(':');
                    let materiau = x.next()?.trim().to_string();
                    let epaisseur = x.next()?.trim().parse().ok()?;
                    let fonction = match x.next().map(str::trim) {
                        Some("structure") => FonctionCouche::Structure,
                        Some("isolation") => FonctionCouche::Isolation,
                        Some("finition") => FonctionCouche::Finition,
                        _ => FonctionCouche::Autre,
                    };
                    Some(Couche { materiau: Some(materiau), epaisseur, fonction, plume: None })
                }).collect();
                let (Some(couches), false) = (couches, nom.is_empty()) else { return erreur("usage : type_mur <nom>|<matériau>:<épaisseur>[:fonction]|…") };
                let Some((projet, _)) = &self.natif else { return erreur("pas de projet natif ouvert") };
                let mut projet = projet.clone();
                match self.ruban_edition.definir_type_mur(&mut projet, &nom, couches) {
                    Ok(id) => match self.appliquer_projet(projet) {
                        Ok(()) => succes(&[("type", texte(&id))]),
                        Err(e) => erreur(&e),
                    },
                    Err(e) => erreur(&e),
                }
            }
            "mur" => {
                let Ok(v) = argument.split_whitespace().map(str::parse::<f64>).collect::<Result<Vec<_>, _>>() else { return erreur("coordonnées du mur invalides") };
                if v.len() != 4 && v.len() != 6 { return erreur("usage : mur <x0 mm> <y0 mm> <x1 mm> <y1 mm> [hauteur mm epaisseur mm]") }
                let mut options = self.ruban_edition.parametres_mur();
                if v.len() == 6 { options.hauteur = v[4]; options.epaisseur = v[5]; }
                match self.ajouter_mur_natif([v[0],v[1]],[v[2],v[3]],&options) {
                    Ok(id) => succes(&[("mur",texte(&id))]),
                    Err(e) => erreur(&e),
                }
            }
            "enregistrer" => {
                if self.natif.is_none() && self.rvt_source.is_some() {
                    let sortie = if argument.trim().is_empty() {
                        let Some(p) = self.rvt_sortie.clone() else { return erreur("usage : enregistrer <chemin.modele.json> pour le premier enregistrement d’un RVT") };
                        p
                    } else { PathBuf::from(argument.trim().trim_matches('"')) };
                    return match self.demarrer_enregistrement_rvt(ctx,sortie) {
                        Ok(()) => self.etat(), Err(e) => erreur(&e),
                    };
                }
                if self.natif.is_none() { return erreur("aucun projet natif ou Revit ouvert") }
                if !argument.trim().is_empty() {
                    let chemin=PathBuf::from(argument.trim().trim_matches('"'));
                    let (p,ancien)=self.natif.as_ref().unwrap();
                    if let Err(e)=crate::modele::enregistrement::enregistrer_natif(p,&chemin){return erreur(&e);}
                    crate::modele::enregistrement::effacer_recuperation(ancien);
                    let _=self.historique.enregistrer(p);
                    self.finaliser_enregistrement_sous(chemin);
                } else {self.enregistrer_projet(&ctx);}
                if let Some(e) = &self.error { erreur(e) } else { self.etat() }
            }
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
                    ("off", _) => self.activer_boite(false),
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
            "traits_fins" => {
                // traits_fins oui|non
                self.traits_fins = argument != "non";
                self.coupe_dirty = true;
                self.etat()
            }
            "dessin" => {
                // dessin oui : graphique au trait (faces blanches) ; non : faces colorées
                self.style = StyleVisuel::Graphique;
                self.faces_blanches = argument != "non";
                self.visibility_dirty = true;
                self.etat()
            }
            "ombres" => {
                // ombres portees|ambiante oui|non
                let (quoi, valeur) = argument.split_once(' ').unwrap_or((argument, "oui"));
                let oui = valeur.trim() != "non";
                match quoi {
                    "portees" | "portées" => self.ombres_portees = oui,
                    "ambiante" => self.ombre_ambiante = oui,
                    _ => return erreur("usage : ombres portees|ambiante oui|non"),
                }
                self.etat()
            }
            "style" => {
                // style graphique|realiste ; anciens noms : lignes_cachees, ombre, couleurs, rendu
                let (style, blanches, eclairage, ombres) = match argument {
                    "graphique" | "ombre" => (StyleVisuel::Graphique, false, true, None),
                    "lignes_cachees" => (StyleVisuel::Graphique, true, true, None),
                    "couleurs" => (StyleVisuel::Graphique, false, false, None),
                    "realiste" => (StyleVisuel::Realiste, false, true, None),
                    "rendu" => (StyleVisuel::Realiste, false, true, Some(true)),
                    _ => return erreur("usage : style graphique|realiste (lignes_cachees, couleurs, rendu)"),
                };
                self.style = style;
                self.faces_blanches = blanches;
                self.eclairage = eclairage;
                if let Some(o) = ombres {
                    self.ombres_portees = o;
                    self.ombre_ambiante = o;
                }
                self.visibility_dirty = true;
                self.coupe_dirty = true;
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
                    self.style = StyleVisuel::Graphique;
                    self.faces_blanches = true;
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
            "exporter_jeu" => {
                // exporter_jeu <chemin.pdf> [jeu] : jeu ouvert, sinon le premier
                let (chemin, jeu) = argument.trim().split_once(' ').map_or((argument.trim(), None), |(c, j)| (c, Some(j.trim().to_string())));
                let jeu = jeu.or_else(|| self.jeu_actif()).or_else(|| self.natif.as_ref().and_then(|(p, _)| crate::modele::documentation::jeux(p).into_iter().next()));
                let Some(jeu) = jeu else { return erreur("aucun jeu de feuilles") };
                if !chemin.to_lowercase().ends_with(".pdf") {
                    return erreur("usage : exporter_jeu <chemin.pdf> [jeu]");
                }
                match self.exporter_jeu(&jeu, std::path::Path::new(chemin)) {
                    Ok(n) => succes(&[("chemin", texte(chemin)), ("jeu", texte(&jeu)), ("pages", n.to_string())]),
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
                self.memoriser_vue();
                self.hidden_objects.clear();
                self.hidden_storeys.clear();
                self.isolated = None;
                self.vue_active = None;
                self.nomenclature_ouverte = None;
                if !self.vues_ouvertes.contains(&None) {
                    self.vues_ouvertes.push(None);
                }
                self.memoriser_vue();
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
/// Repère d'accrochage du tracé : carré (extrémité), triangle (milieu), losange
/// (angle d'un mur), cercle sinon.
fn marqueur(painter: &egui::Painter, q: egui::Pos2, acc: AccrocheMur, couleur: Color32, accent: Color32) {
    let t = egui::Stroke::new(2.0, accent);
    match acc {
        AccrocheMur::Extremite => {
            painter.rect_stroke(egui::Rect::from_center_size(q, egui::vec2(11.0, 11.0)), 0.0, t, egui::StrokeKind::Middle);
        }
        AccrocheMur::Milieu => {
            let h = 7.0;
            painter.add(egui::Shape::closed_line(vec![q + egui::vec2(0.0, -h), q + egui::vec2(h, h * 0.8), q + egui::vec2(-h, h * 0.8)], t));
        }
        AccrocheMur::Coin => {
            let h = 7.0;
            painter.add(egui::Shape::closed_line(vec![q + egui::vec2(0.0, -h), q + egui::vec2(h, 0.0), q + egui::vec2(0.0, h), q + egui::vec2(-h, 0.0)], t));
        }
        _ => {
            painter.circle_stroke(q, 4.0, egui::Stroke::new(1.5, couleur));
        }
    }
}

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
        self.valider_saisie(&ctx);
        self.onglet_selon_selection();
        self.historique.geste(ctx.input(|i|i.pointer.any_down()));
        self.poll_sauvegarde();
        self.fenetre_documentation(&ctx);
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z)) { self.parcourir_historique(true); }
            else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) { self.parcourir_historique(false); }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.sauvegarde.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.error = Some("Un enregistrement est en cours ; attendez sa fin avant de quitter.".into());
        } else if ctx.input(|i| i.viewport().close_requested()) && (self.natif_modifie || self.documents.values().any(|d| d.modifie)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirmer_fermeture = Some((true, self.document_actif));
        }
        if self.selected.is_some() && !ctx.egui_wants_keyboard_input() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
            self.tab_selection();
            // Tab sert ici à la sélection, pas au passage du focus entre les champs
            ctx.memory_mut(|m| {
                m.move_focus(egui::FocusDirection::None);
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
        self.clavier_dessin(&ctx);
        if self.alignement_plan && !self.saisie_active && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.alignement_plan = false;
            self.arete_alignement = None;
        }
        if self.ruban_edition.a_un_outil() && !self.saisie_active && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            // Échap : la saisie, puis le mur ou la dalle en cours, puis l'outil
            if self.bim.depart_structure.take().is_some() {
                // Le premier clic d'une poutre est abandonné sans modifier le projet.
            } else if self.coupe_depart.is_some() {
                self.coupe_depart = None;
            } else {
                self.ruban_edition.annuler();
            }
        }
        self.poll_loading();
        self.piloter(&ctx);

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.open(p, &ctx);
        }
        if std::mem::take(&mut self.bim.supprimer) { self.demander_suppression(); }
        if !ctx.egui_wants_keyboard_input() && self.scene.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) && self.rect_vue.is_positive() {
                let f = FenetreSelection { debut: egui::pos2(-100_000_000.0, -100_000_000.0), fin: egui::pos2(100_000_000.0, 100_000_000.0), glissement: false, ajouter: false };
                self.selectionner_rectangle(self.rect_vue, f);
            }
            // Suppr, ou la touche « supprimer » des claviers Mac (Retour arrière) hors tracé
            let trace = self.trace_en_cours() || self.coupe_depart.is_some() || self.bim.depart_structure.is_some();
            if self.natif.is_some() && !trace && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete) || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)) { self.demander_suppression(); }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { self.raccourci_en_attente = None; }
        }
        if !ctx.egui_wants_keyboard_input() && self.natif.is_some() && !self.trace_en_cours() && self.coupe_depart.is_none() && self.bim.depart_structure.is_none() {
            let (temps, textes) = ctx.input(|i| (i.time, if i.modifiers.ctrl || i.modifiers.alt || i.modifiers.command { Vec::new() } else { i.events.iter().filter_map(|e| if let egui::Event::Text(s) = e { Some(s.clone()) } else { None }).collect::<Vec<_>>() }));
            if self.raccourci_en_attente.is_some_and(|(_, t)| temps - t > 2.0) { self.raccourci_en_attente = None; }
            for c in textes.iter().flat_map(|s| s.chars()).filter(|c| c.is_ascii_alphabetic()).map(|c| c.to_ascii_uppercase()) {
                if let Some((premier, _)) = self.raccourci_en_attente.take() {
                    use crate::ruban::Outil;
                    let outil = match (premier, c) { ('M','U') => Some((Outil::Mur, OngletRuban::Modeliser)), ('P','O') => Some((Outil::Poteau, OngletRuban::Modeliser)), ('P','P') => Some((Outil::Poutre, OngletRuban::Modeliser)), ('O','U') => Some((Outil::Ouverture, OngletRuban::Modeliser)), ('M','V') => Some((Outil::Deplacer, OngletRuban::Modifier)), ('C','O') => Some((Outil::Copier, OngletRuban::Modifier)), ('R','O') => Some((Outil::Tourner, OngletRuban::Modifier)), ('A','L') => Some((Outil::Aligner, OngletRuban::Modifier)), ('A','J') => Some((Outil::Ajuster, OngletRuban::Modifier)), _ => None };
                    if (premier, c) == ('C','S') { if let Err(e) = self.creer_identique() { self.error = Some(e); } }
                    else if let Some((outil, onglet)) = outil { self.onglet_ruban = onglet; self.ruban_edition.activer_outil(outil); }
                } else if ['C','M','P','O','R','A'].contains(&c) { self.raccourci_en_attente = Some((c, temps)); }
            }
        } else { self.raccourci_en_attente = None; }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(&ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) && self.loading.is_none() {
            self.creer_projet();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            // onglet Famille : charger la famille dans le projet
            if self.famille_active && self.famille.is_some() {
                let nouveau = self.famille.as_ref().is_some_and(|e| e.nouvelle());
                self.action_famille = Some(famille_ui::ActionFamille::Charger(nouveau));
            } else {
                self.enregistrer_projet(&ctx);
            }
        }

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        self.zone_panneaux = ui.available_rect_before_wrap();
        self.deplacer_panneau(&ctx);
        if self.rvt.is_some() && self.rvt_sheet {
            egui::Panel::top("onglets_vues").show(ui, |ui| self.onglets_vues(ui));
            egui::CentralPanel::default().show(ui, |ui| self.revit_view(ui));
        } else {
            // disposition de Revit par défaut : propriétés à gauche, arborescence du projet
            // à droite ; chacun s'ancre d'un côté ou flotte (ancrages relevés en début
            // d'image, changés pour la suivante)
            let ancrages = self.ancrages;
            // onglet Famille : l'éditeur a ses propres panneaux, ceux du projet se retirent
            let famille = self.famille_active && self.famille.is_some();
            for p in Panneau::TOUS {
                let id = egui::IdSalt::new((p.nom(), ancrages[p as usize].nom()));
                let panneau = match ancrages[p as usize] {
                    Ancrage::Gauche => egui::Panel::left(id),
                    Ancrage::Droite => egui::Panel::right(id),
                    Ancrage::Flottant => continue,
                };
                let reponse = panneau.resizable(true).default_size(self.largeurs_panneaux[p as usize]).show(ui, |ui| self.panneau(ui, p));
                self.largeurs_panneaux[p as usize] = reponse.response.rect.width();
            }
            // sous la vue seulement, entre les panneaux ancrés
            if self.scene.is_some() {
                egui::Panel::top("onglets_vues").show(ui, |ui| self.onglets_vues(ui));
            }
            if !famille {
                egui::Panel::bottom("barre_vue").show(ui, |ui| self.barre_vue(ui));
            }
            egui::CentralPanel::no_frame().show(ui, |ui| {
                if self.famille_active && self.famille.is_some() {
                    self.vue_famille(ui);
                } else if self.scene.is_some() && self.vues_ouvertes.is_empty() {
                    ui.centered_and_justified(|ui| ui.label("Ouvrir une vue depuis l’arborescence du projet."));
                } else if self.nomenclature_ouverte.is_some() {
                    self.vue_nomenclature(ui);
                } else if self.jeu_actif().is_some() {
                    self.vue_jeu(ui);
                } else {
                    self.viewport(ui);
                }
            });
            for p in Panneau::TOUS.into_iter().filter(|p| ancrages[*p as usize] == Ancrage::Flottant) {
                let i = p as usize;
                let mut fenetre = egui::Window::new(p.nom()).id(egui::Id::unique(("flottant", p.nom()))).title_bar(false).movable(false).resizable(true).default_size([self.largeurs_panneaux[i], 520.0]).min_width(220.0);
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
        // vue ou feuille glissée dans l'arborescence : son nom suit le curseur
        let glissee = egui::DragAndDrop::payload::<feuilles::VueGlissee>(&ctx)
            .and_then(|g| self.natif.as_ref().and_then(|(p, _)| p.vues.iter().find(|v| v.id == g.0)).map(|v| format!("Placer : {}", v.nom)))
            .or_else(|| egui::DragAndDrop::payload::<feuilles::NomenclatureGlissee>(&ctx).and_then(|g| self.natif.as_ref().and_then(|(p, _)| p.nomenclatures.iter().find(|n| n.id == g.0)).map(|n| format!("Poser : {}", n.nom))))
            .or_else(|| egui::DragAndDrop::payload::<feuilles::FeuilleGlissee>(&ctx).and_then(|g| self.natif.as_ref().and_then(|(p, _)| p.feuilles.iter().find(|f| f.id == g.0)).map(|f| format!("Déplacer : {} - {}", f.numero, f.nom))));
        if let (Some(nom), Some(q)) = (glissee, ctx.pointer_hover_pos()) {
            let peintre = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("vue_glissee")));
            let texte = peintre.layout_no_wrap(nom, egui::FontId::proportional(13.0), Color32::WHITE);
            let cadre = egui::Rect::from_min_size(q + egui::vec2(14.0, 10.0), texte.size() + egui::vec2(12.0, 6.0));
            peintre.rect_filled(cadre, 4.0, Color32::from_rgb(0, 120, 215));
            peintre.galley(cadre.min + egui::vec2(6.0, 3.0), texte, Color32::WHITE);
        }
        self.traiter_action_famille();
        self.confirmation_famille(&ctx);
        self.unsupported_window(&ctx);
        self.confirmation_modifications(&ctx);
        self.apercu_ancrage(&ctx);
        self.bords_fenetre(&ctx);
    }

    /// Appelé même fenêtre réduite : relève les commandes du pilote et demande alors le
    /// tracé complet, que `ui` les traite et qu'une capture ait des pixels.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_sauvegarde();
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

/// Aperçu d'un motif de hachures dans `r` : traits du motif, `px_mm` pixels par mm du papier.
fn apercu_motif(p: &egui::Painter, r: egui::Rect, d: &crate::modele::DefinitionMotif, px_mm: f32) {
    use crate::rvt::vues::{hachures_plan, GrilleMotif, Motif};
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
