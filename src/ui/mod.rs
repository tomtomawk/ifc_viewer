//! Interface egui, disposée comme Revit : propriétés à gauche, vue au centre,
//! arborescence du projet à droite (vues, nomenclatures, feuilles, familles, groupes et
//! liens d'un fichier Revit ; arborescence spatiale et classes d'un IFC). Les deux
//! panneaux s'ancrent d'un côté ou flottent ; sous la vue, la boîte de visibilité par
//! catégorie de modèle et d'annotation.

mod bim;
mod communes;
mod types_traits;
mod feuilles;
mod dessin_plan;
mod editeur_famille;
mod accrochage;
mod gestion_contextes;
mod gabarits_vues;
mod profils;
pub mod ruban;
use accrochage::{AccrocheMur, marqueur};
mod collaboration;
mod pilotage;
mod parametres;
mod ajuster;
mod volets;
mod murs_courbes;
mod aides;
mod transformer;
mod menu_vue;
use aides::*;
mod documents;
mod coupe;
mod vue_native;
mod selection;
mod selection_famille;
mod garde_corps;
mod familles_planes;
mod cartouche;
mod proprietes;
mod saisie_plan;
mod barre_haute;
mod panneaux;
mod navigateur;
mod vues;
mod graphismes;
mod soleil;
mod panneau_droit;
mod vue_revit;
mod viewport;
mod commandes_pilote;
use crate::config::options::Dimensions;
use crate::rendu::camera::{Camera, MAX_PITCH};
use crate::dessin::{self, Poche};
use crate::formats::ifc::semantique::pretty_type;
use crate::pilote::{self, Requete};
use crate::rendu::gpu::{FrameParams, ParamsPasse, Renderer, StyleVisuel};
use crate::formats::rvt::{self, RvtInfo};
use crate::rendu::scene::{self, Scene};
use crate::noyau::trace2d;
use crate::formats::ifc::step::Id;
use eframe::egui::{self, Align, Color32, PointerButton, RichText, Sense};
use glam::{DMat4, Mat4, Vec2, Vec3, Vec4};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;

enum LoadMsg {
    Partage(Result<(Box<Scene>, crate::modele::Projet, PathBuf, crate::modele::collaboration::session::Session), String>, Option<usize>),
    Natif(Result<(Box<Scene>, crate::modele::Projet, PathBuf), String>),
    Progress(String),
    Done(Result<Box<Scene>, String>),
    /// Métadonnées, puis scène des murs (ou raison de son absence).
    Revit(Result<(Box<RvtInfo>, Result<Box<Scene>, String>, crate::modele::echanges::enregistrement::SourceRvt), String>),
}

enum SaveMsg {
    Progress(String),
    Done(Result<crate::modele::echanges::enregistrement::Bilan, String>),
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
            LeftTab::Liens => "Liens",
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
    SupprimerVue(usize),
    DupliquerVue(String),
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
    Externe,
}

impl Ancrage {
    const TOUS: [Ancrage; 4] = [Ancrage::Gauche, Ancrage::Droite, Ancrage::Flottant, Ancrage::Externe];

    /// Nom pour le pilote (commande `panneau`).
    fn nom(self) -> &'static str {
        match self {
            Ancrage::Gauche => "gauche",
            Ancrage::Droite => "droite",
            Ancrage::Flottant => "flottant",
            Ancrage::Externe => "externe",
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
    Debut,
    Modeliser,
    Modifier,
    Annoter,
    Sortie,
    /// Ruban de l'éditeur de familles : outils de création.
    FamilleCreer,
    /// Ruban de l'éditeur de familles : chargement dans le projet et fichiers.
    FamilleSortie,
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
    soleil_jour: u32,
    soleil_mois: u32,
    soleil_heure: f32,
    ciel: crate::modele::soleil::ReglagesCiel,
    coupe: bool,
    coupe_niveau: Option<Id>,
    coupe_hauteur: f32,
    poche: Poche,
    echelle: f64,
    detail: NiveauDetail,
    boite: Option<(Vec3, Vec3)>,
    boite_active: bool,
}

/// Fenêtre de sélection, commune aux vues du projet et à l'éditeur de familles : de
/// gauche à droite, fenêtre (éléments entièrement dedans) ; de droite à gauche,
/// croisement (éléments touchés). Ctrl : ajouter à la sélection.
#[derive(Clone, Copy)]
struct FenetreSelection { debut: egui::Pos2, fin: egui::Pos2, ajouter: bool }

impl FenetreSelection {
    fn rectangle(&self) -> egui::Rect {
        egui::Rect::from_two_pos(self.debut, self.fin)
    }
    fn croisement(&self) -> bool {
        self.fin.x < self.debut.x
    }
    /// Croisement : vert en tirets ; fenêtre : bleu plein.
    fn dessiner(&self, painter: &egui::Painter) {
        let r = self.rectangle();
        let couleur = if self.croisement() { Color32::from_rgb(35, 160, 75) } else { Color32::from_rgb(35, 110, 220) };
        painter.rect_filled(r, 0.0, couleur.gamma_multiply(0.18));
        if self.croisement() {
            let points = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            painter.add(egui::Shape::dashed_line(&points, egui::Stroke::new(1.2, couleur), 5.0, 4.0));
        } else {
            painter.rect_stroke(r, 0.0, egui::Stroke::new(1.2, couleur), egui::StrokeKind::Inside);
        }
    }
}

/// Fichier conservé en mémoire quand un autre document devient actif.
struct DocumentOuvert {
    natif: Option<(crate::modele::Projet, PathBuf)>,
    historique: crate::modele::edition::historique::Historique,
    collaboration: Option<crate::modele::collaboration::session::Session>,
    modifie: bool,
    scene: Option<Scene>,
    rvt: Option<RvtInfo>,
    rvt_source: Option<crate::modele::echanges::enregistrement::SourceRvt>,
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
    dessin2d: dessin_plan::EtatDessin2D,
    /// Éditeur de famille de fenêtre ouvert.
    famille: Option<editeur_famille::EditeurFamille>,
    /// Onglet de l'éditeur de familles au premier plan.
    famille_active: bool,
    /// Autres familles ouvertes (onglets à l'arrière-plan).
    familles_inactives: Vec<editeur_famille::EditeurFamille>,
    familles_planes: BTreeMap<usize, familles_planes::EditeurPlan>,
    famille_plane_active: Option<usize>,
    prochain_ordre_famille: usize,
    /// Action des panneaux de l'éditeur de familles, traitée après leur tracé.
    action_famille: Option<editeur_famille::ActionFamille>,
    /// Fermeture de l'onglet Famille à confirmer (modifications non chargées).
    confirmer_famille: bool,
    natif: Option<(crate::modele::Projet, PathBuf)>,
    historique: crate::modele::edition::historique::Historique,
    collaboration: Option<crate::modele::collaboration::session::Session>,
    natif_modifie: bool,
    /// Repère du plan : le prochain clic choisit l'arête à mettre à l'horizontale.
    alignement_plan: bool,
    /// Choix d'un trait donnant la direction du nord du projet.
    orientation_nord_en_cours: bool,
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
    copie_presse: Option<(usize,Vec<String>)>,
    copie_niveau: Option<String>,
    /// Début de la ligne de coupe en cours de tracé (mm).
    coupe_depart: Option<[f64; 2]>,
    raccourci_en_attente: Option<(String, f64)>,
    confirmer_fermeture: Option<(bool, usize)>, // application entière, ou identifiant du document
    documents: BTreeMap<usize, DocumentOuvert>,
    document_actif: usize,
    /// Registre des changements de document actif (projets et familles), lu par le
    /// pilote : le plus récent en dernier.
    journal_documents: Vec<serde_json::Value>,
    prochain_document: usize,
    renderer: Option<Renderer>,
    renderer_famille: Option<Renderer>,
    scene: Option<Scene>,
    /// Métadonnées du fichier Revit ouvert.
    rvt: Option<RvtInfo>,
    rvt_source: Option<crate::modele::echanges::enregistrement::SourceRvt>,
    rvt_sortie: Option<PathBuf>,
    sauvegarde: Option<Sauvegarde>,
    /// Fiche Revit affichée à la place de la vue 3D.
    rvt_sheet: bool,
    /// Lecture des murs du fichier Revit : résumé si la scène en vient, sinon raison de l'échec.
    rvt_geom: Result<String, String>,
    rvt_preview: Option<egui::TextureHandle>,
    loading: Option<Receiver<LoadMsg>>,
    partage_en_cours: bool,
    status: String,
    /// Message de confirmation affiché quelques secondes en bas de la vue.
    confirmation: Option<(String, std::time::Instant)>,
    error: Option<String>,
    camera: Camera,
    selected: Option<Id>,
    selection_multiple: Vec<Id>,
    famille_selectionnee: Option<selection_famille::FamilleCatalogue>,
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
    /// Soleil de la vue : date sans année et heure légale (heures décimales).
    soleil_jour: u32,
    soleil_mois: u32,
    soleil_heure: f32,
    /// Ciel physique de la vue (réglages) et fenêtre Ciel ouverte.
    ciel: crate::modele::soleil::ReglagesCiel,
    ciel_ouvert: bool,
    /// Vue piéton : œil à hauteur d'homme, la marche suit le sol ; caméra d'avant.
    pieton: bool,
    camera_avant_pieton: Option<Camera>,
    /// Pente donnée à une arête cochée (degrés), la dernière saisie.
    pente_defaut: f64,
    /// Options de l'application (menu Début > Options).
    options: crate::config::options::Options,
    parametres: Option<parametres::Parametres>,
    ajustement: Option<ajuster::EtatAjustement>,
    volets: [volets::FenetreVolet;2],
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
    prochaine_tentative_pilote: std::time::Instant,
    requetes: Vec<Requete>,
    saisies_pilote: std::collections::VecDeque<egui::Event>,
    saisie_pilote_image: bool,
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
    /// Niveau du plan ouvert à la dernière image : les outils de création en héritent.
    niveau_plan_precedent: Option<String>,
    ruban_edition: crate::ui::ruban::RubanEdition,
    editeur_garde_corps: Option<garde_corps::EditeurGardeCorps>,
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
    /// Dernière poignée déplacée et cote modifiable de sa ligne d'implantation.
    cote_axe: Option<(String, bool, String)>,
    /// Dernier mur courbe modifié : poignée, rayon et longueur d'arc éditables.
    cote_arc: Option<(String, usize, String, String)>,
    gestion_contextes: Option<gestion_contextes::GestionContextes>,
    editeur_gabarit_vue: Option<gabarits_vues::EditeurGabaritVue>,
    filtres_contextes: crate::modele::edition::contextes::FiltresContextes,
    /// Dernière variante de création dont l'affichage a été synchronisé.
    variante_creation_suivie: Option<String>,
    /// Matériau du projet natif dont les graphismes sont ouverts.
    materiau_edite: Option<String>,
    /// Onglet de la fenêtre Matériau : Graphique (faux) ou Apparence (vrai).
    materiau_apparence: bool,
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
    deplacement_panneau: Option<(Panneau, egui::Vec2, egui::ViewportId)>,
    zone_panneaux: egui::Rect,
    largeurs_panneaux: [f32; 2],
    /// Cadenas affichés à la dernière image : élément natif et position écran.
    cadenas_ecran: Vec<(String, egui::Pos2)>,
    /// Objets masqués par l'utilisateur (Masquer temporaire), distincts des variantes.
    masques_utilisateur: HashSet<u32>,
    /// Isoler a été demandé (visibilité temporaire) : la croix du cadre le lève.
    isolement_utilisateur: bool,
    /// Centre à l'écran de la croix du cadre de visibilité temporaire (pilote, tests).
    croix_temporaire: Option<egui::Pos2>,
    /// Mode « Afficher les éléments masqués » : masqués rendus en magenta, clic = réafficher.
    afficher_masques: bool,
    /// Objets masqués rendus visibles par ce mode (dernier calcul de visibilité).
    objets_masques: HashSet<u32>,
    /// Commande de transformation en cours (Déplacer, Copier, Tourner, Aligner).
    transfo: transformer::Transformation,
    /// Dernière commande de modification utilisée : Entrée ou clic droit la relance.
    derniere_commande: Option<crate::ui::ruban::Outil>,
    /// Poignée du milieu du mur en cours de glissement : mur, départ, position courante.
    glisse_milieu: Option<(String, [f64; 2], [f64; 2])>,
    /// Captures de fenêtre demandées par le pilote et pas encore reçues : egui ne livre
    /// l'image qu'à une image suivante, il faut donc continuer à redessiner.
    captures_en_attente: Arc<std::sync::atomic::AtomicUsize>,
}

impl ViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut error = None;
        let renderer_famille = cc.wgpu_render_state.as_ref().and_then(|rs| Renderer::new(rs).ok());
        let renderer = match cc.wgpu_render_state.as_ref().map(Renderer::new) {
            Some(Ok(r)) => Some(r),
            Some(Err(e)) => {
                error = Some(e);
                None
            }
            None => {
                error = Some("Rendu wgpu indisponible".into());
                None
            }
        };
        let mut app = ViewerApp {
            bim: Default::default(),
            feuilles: Default::default(),
            natif: None,
            historique: Default::default(),
            collaboration: None,
            natif_modifie: false,
            raccourci_en_attente: None,
            coupe_depart: None,
            nom_vue: None,
            copie_presse: None,
            copie_niveau: None,
            alignement_plan: false,
            orientation_nord_en_cours: false,
            dessin2d: Default::default(),
            famille: None,
            famille_active: false,
            familles_inactives: Vec::new(),
            familles_planes: BTreeMap::new(),
            famille_plane_active: None,
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
            journal_documents: Vec::new(),
            prochain_document: 1,
            renderer,
            renderer_famille,
            scene: None,
            rvt: None,
            rvt_source: None,
            rvt_sortie: None,
            sauvegarde: None,
            rvt_sheet: false,
            rvt_geom: Err(String::new()),
            rvt_preview: None,
            loading: None,
            partage_en_cours: false,
            status: "Ouvrir un fichier IFC ou Revit (Ctrl+O), ou le glisser dans la fenêtre.".into(),
            confirmation: None,
            error,
            camera: Camera::default(),
            selected: None,
            selection_multiple: Vec::new(),
            famille_selectionnee: None,
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
            soleil_jour: 21,
            soleil_mois: 6,
            soleil_heure: 15.0,
            ciel: Default::default(),
            ciel_ouvert: false,
            pieton: false,
            camera_avant_pieton: None,
            faces_blanches: false,
            traits_fins: false,
            eclairage: true,
            parametres: None,
            ajustement: None,
            volets: Default::default(),
            options: {
                let o = crate::config::options::Options::charger();
                crate::config::options::definir_unite(o.unite);
                o.theme_interface.appliquer(&cc.egui_ctx);
                // gabarit choisi : lu au démarrage, défaut s'il est illisible
                if let Some(c) = &o.gabarit {
                    match crate::config::gabarit::Gabarit::charger(c) {
                        Ok(g) => crate::config::gabarit::definir(g),
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
            prochaine_tentative_pilote: std::time::Instant::now(),
            requetes: Vec::new(),
            saisies_pilote: Default::default(),
            saisie_pilote_image: false,
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
            niveau_plan_precedent: None,
            ruban_edition: Default::default(),
            editeur_garde_corps: None,
            boite: None,
            boite_active: false,
            coupe_dirty: true,
            liste_niveaux: Vec::new(),
            liste_materiaux: Vec::new(),
            materiau_edite: None,
            materiau_apparence: false,
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
            cote_axe: None,
            cote_arc: None,
            gestion_contextes: None,
            editeur_gabarit_vue: None,
            filtres_contextes: Default::default(),
            variante_creation_suivie: None,
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
            cadenas_ecran: Vec::new(),
            masques_utilisateur: HashSet::new(),
            isolement_utilisateur: false,
            croix_temporaire: None,
            afficher_masques: false,
            objets_masques: HashSet::new(),
            transfo: Default::default(),
            derniere_commande: None,
            glisse_milieu: None,
            captures_en_attente: Default::default(),
        };
        if let Some(p) = path {
            app.open(p, &cc.egui_ctx);
        }
        app
    }

}

impl eframe::App for ViewerApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let terminer = self.saisie_pilote_image && self.saisies_pilote.is_empty();
        self.saisie_pilote_image = !self.saisies_pilote.is_empty();
        // Une étape par image : egui observe bien l'appui puis le relâchement.
        if let Some(e) = self.saisies_pilote.pop_front() {
            let modifiers = match &e {
                egui::Event::PointerButton { modifiers, .. } | egui::Event::Key { modifiers, .. } | egui::Event::MouseWheel { modifiers, .. } => *modifiers,
                _ => egui::Modifiers::NONE,
            };
            raw_input.events.push(egui::Event::ModifiersChanged(modifiers));
            raw_input.events.push(e);
            ctx.request_repaint();
        }
        else if terminer {
            // Ne pas laisser Ctrl/Maj artificiellement enfoncé après la dernière entrée.
            // Les événements physiques du système conservent la priorité.
            if raw_input.events.is_empty() { raw_input.events.push(egui::Event::ModifiersChanged(egui::Modifiers::NONE)); }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // tampon refusé par le GPU (modèle trop lourd pour la carte) : dit plutôt que planter
        if let Some(alerte) = self.renderer.as_mut().and_then(|r| r.alerte_memoire()) {
            self.error = Some(alerte);
        }
        // une capture de fenêtre arrive à une image ultérieure : sans souris ni clavier,
        // egui ne redessinerait plus et la capture n'aboutirait jamais
        if self.captures_en_attente.load(std::sync::atomic::Ordering::Acquire) > 0 {
            ctx.request_repaint();
        }
        if self.pilote.is_none() {
            if std::time::Instant::now() >= self.prochaine_tentative_pilote {
                self.pilote = pilote::demarrer(ctx.clone());
                self.prochaine_tentative_pilote = std::time::Instant::now() + std::time::Duration::from_secs(2);
            }
            if self.pilote.is_none() { ctx.request_repaint_after(std::time::Duration::from_secs(2)); }
        }
        // registre des instances : fichier ouvert et partage, pour que le pont MCP choisisse
        pilote::instances::publier(
            self.natif.as_ref().map(|(_, p)| p.to_string_lossy().into_owned()).or_else(|| self.scene.as_ref().map(|s| s.file_name.clone())),
            // le .bimpartage commun, pas la copie locale propre à chaque instance
            self.collaboration.as_ref().and_then(|c| c.copie.partage.as_ref()).map(|p| p.to_string_lossy().into_owned()),
        );
        self.valider_saisie(&ctx);
        self.recevoir_collaboration();
        self.niveau_creation_courant();
        self.onglet_selon_selection();
        self.historique.geste(ctx.input(|i|i.pointer.any_down()) || self.volets.iter().any(|v|v.interaction));
        self.poll_sauvegarde();
        self.fenetre_documentation(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) && self.sauvegarde.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.error = Some("Un enregistrement est en cours ; attendez sa fin avant de quitter.".into());
        } else if ctx.input(|i| i.viewport().close_requested()) && let Some(ordre) = self.edition_plane_modifiee(None) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.activer_famille(ordre);
            self.demander_fermeture_famille();
        } else if ctx.input(|i| i.viewport().close_requested()) && let Some(ordre) = self.famille.iter().chain(&self.familles_inactives).find(|ed| ed.modifiee()).map(|ed| ed.ordre) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.activer_famille(ordre);
            self.demander_fermeture_famille();
        } else if ctx.input(|i| i.viewport().close_requested()) && (self.natif_modifie || self.documents.values().any(|d| d.modifie)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirmer_fermeture = Some((true, self.document_actif));
        }
        let projet_actif = self.famille_plane_active.is_none();
        if projet_actif {
            self.clavier_dessin(&ctx);
            self.clavier_transformation(&ctx);
            self.repeter_commande(&ctx);
        }
        if projet_actif && self.alignement_plan && !self.saisie_active && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.alignement_plan = false;
            self.arete_alignement = None;
        }
        if projet_actif && self.orientation_nord_en_cours && !self.saisie_active && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.orientation_nord_en_cours = false;
            self.arete_alignement = None;
            self.status = "Orientation du nord annulée.".into();
        }
        if projet_actif && self.afficher_masques && !self.saisie_active && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.basculer_afficher_masques();
        }
        if projet_actif && self.ruban_edition.a_un_outil() && !self.saisie_active && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            // Échap : la saisie, puis le mur ou la dalle en cours, puis l'outil
            if self.ruban_edition.outil()==Some(crate::ui::ruban::Outil::Ajuster) && self.ajustement.as_ref().is_some_and(|a|a.reference.is_some()) {
                self.ajustement=Some(Default::default());
                self.status="Ajuster / prolonger : choisir une nouvelle limite, ou Échap pour terminer.".into();
            } else if self.transfo.outil.is_some() && self.transfo.reculer() {
                // Déplacer, Copier, Tourner, Aligner : une étape en arrière
            } else if self.bim.depart_structure.take().is_some() {
                // Le premier clic d'une poutre est abandonné sans modifier le projet.
            } else if self.coupe_depart.is_some() {
                self.coupe_depart = None;
            } else {
                self.ruban_edition.annuler();
            }
        }
        self.poll_loading();
        let mut t_image = std::time::Instant::now();
        self.piloter(&ctx);
        scene::chrono_lent("image : commandes du pilote", &mut t_image);

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.open(p, &ctx);
        }
        if std::mem::take(&mut self.bim.supprimer) { self.demander_suppression(); }
        self.clavier_raccourcis(&ctx);

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        scene::chrono_lent("image : ruban", &mut t_image);
        self.fenetre_garde_corps(&ctx);
        self.fenetre_gestion_contextes(&ctx);
        self.fenetre_gabarit_vue(&ctx);
        self.fenetre_parametres(&ctx);
        self.fenetre_coller_niveau(&ctx);
        self.fenetres_collaboration(&ctx);
        self.zone_panneaux = ui.available_rect_before_wrap();
        self.deplacer_panneau(&ctx);
        if self.rvt.is_some() && self.rvt_sheet && !self.mode_famille() {
            egui::Panel::top("onglets_vues").show(ui, |ui| self.onglets_vues(ui));
            egui::CentralPanel::default().show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                self.revit_view(ui);
                self.annotation_erreur(ui, rect, 0.0);
            });
        } else {
            // disposition de Revit par défaut : propriétés à gauche, arborescence du projet
            // à droite ; chacun s'ancre d'un côté ou flotte (ancrages relevés en début
            // d'image, changés pour la suivante)
            let ancrages = self.ancrages;
            // onglet Famille : l'éditeur a ses propres panneaux, ceux du projet se retirent
            let famille = self.mode_famille();
            for p in Panneau::TOUS {
                let id = egui::IdSalt::new((p.nom(), ancrages[p as usize].nom()));
                let panneau = match ancrages[p as usize] {
                    Ancrage::Gauche => egui::Panel::left(id),
                    Ancrage::Droite => egui::Panel::right(id),
                    Ancrage::Flottant | Ancrage::Externe => continue,
                };
                let reponse = panneau.resizable(true).min_size(24.0).default_size(self.largeurs_panneaux[p as usize]).show(ui, |ui| self.panneau(ui, p));
                self.largeurs_panneaux[p as usize] = reponse.response.rect.width();
            }
            // sous la vue seulement, entre les panneaux ancrés
            if self.scene.is_some() || !self.onglets_familles().is_empty() {
                egui::Panel::top("onglets_vues").show(ui, |ui| self.onglets_vues(ui));
            }
            if !famille {
                egui::Panel::bottom("barre_vue").show(ui, |ui| self.barre_vue(ui));
            } else if let Some(cartouche) = self.cartouche_actif() {
                egui::Panel::bottom("barre_vue_cartouche").show(ui, |ui| cartouche.barre_vue(ui));
            }
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                let mut bas_barre = 0.0;
                if self.famille_plane_active.is_some() {
                    self.vue_famille_plane(ui);
                } else if self.famille_active && self.famille.is_some() {
                    self.vue_famille(ui);
                } else if self.scene.is_some() && self.vues_ouvertes.is_empty() {
                    ui.centered_and_justified(|ui| ui.label("Ouvrir une vue depuis l’arborescence du projet."));
                } else if self.nomenclature_ouverte.is_some() {
                    self.vue_nomenclature(ui);
                } else if self.jeu_actif().is_some() {
                    self.vue_jeu(ui);
                } else {
                    self.viewport(ui);
                    if self.scene.is_some() { bas_barre = self.dessin2d.bas_barre; }
                }
                self.annotation_erreur(ui, rect, bas_barre);
            });
            for p in Panneau::TOUS.into_iter().filter(|p| ancrages[*p as usize] == Ancrage::Flottant) {
                let i = p as usize;
                let mut fenetre = egui::Window::new(p.nom()).id(egui::Id::unique(("flottant", p.nom()))).title_bar(false).movable(false).resizable(true).default_size([self.largeurs_panneaux[i], 520.0]).min_width(24.0);
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
            self.fenetres_volets(&ctx);
            self.fenetre_visibilite(&ctx);
        }
        scene::chrono_lent("image : panneaux et vue", &mut t_image);
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
        let geste = ctx.input(|i| i.pointer.any_down()) || ctx.egui_wants_keyboard_input();
        if let Some(ed) = self.famille.as_mut() {
            ed.suivre_historique(geste);
        }
        self.suivre_document_actif();
        self.unsupported_window(&ctx);
        self.confirmation_modifications(&ctx);
        self.apercu_ancrage(&ctx);
        self.bords_fenetre(&ctx);
        scene::chrono_lent("image : fin", &mut t_image);
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
}

#[cfg(test)]
mod tests_axe_accroche {
    use super::*;
    #[test]
    fn implantation_reste_exactement_sur_la_cible_apres_rotation() {
        for fin in [false,true] {
            for ecart in [-150.0,0.0,150.0] {
                let cible=glam::DVec2::new(3000.0,2000.0);
                let fixe=glam::DVec2::new(0.0,150.0);
                let median=glam::DVec2::from_array(ViewerApp::extremite_mediane(cible.to_array(),fixe.to_array(),fin,ecart).unwrap());
                let u=if fin{(median-fixe).normalize()}else{(fixe-median).normalize()};
                let retrouve=median+glam::DVec2::new(-u.y,u.x)*ecart;
                assert!(retrouve.distance(cible)<1e-8);
            }
        }
        assert!(ViewerApp::extremite_mediane([0.0,0.0],[0.0,0.0],true,150.0).is_err());
    }
}
