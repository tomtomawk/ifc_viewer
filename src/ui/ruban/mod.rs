//! Présentation des commandes d'édition. Le contrôleur du viewer consomme les
//! paramètres des outils opérationnels ; les outils à venir sont désactivés.

mod affichage;
mod murs;
mod options;
mod ouvertures;
pub(crate) use ouvertures::nom_genre;
mod icones;
use icones::*;
pub(crate) mod selecteur;
mod barre_commandes;
pub use barre_commandes::*;
#[cfg(test)]
mod tests;
use crate::config::options::Dimensions;
use eframe::egui::{self, Align, Color32, RichText, Sense};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Modeliser,
    Modifier,
    Annoter,
    Cartouche,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) enum Outil {
    Niveau,
    Quadrillage,
    Mur,
    Terrain,
    LimitePropriete,
    Surfaces,
    Dalle,
    Sol,
    Toiture,
    Plafond,
    Poteau,
    Poutre,
    Escalier,
    GardeCorps,
    Ouverture,
    Composant,
    Piece,
    Arbre,
    Deplacer,
    Copier,
    Tourner,
    Symetrie,
    Reseau,
    Echelle,
    Decaler,
    Scinder,
    Demolir,
    Reproduire,
    ChangerHote,
    Inverser,
    Aligner,
    Ajuster,
    ModifierContour,
    CopierNiveau,
    Jonctions,
    AttacherToiture,
    CreerIdentique,
    Supprimer,
    Cadrer,
    Isoler,
    Masquer,
    MasquerVue,
    Afficher,
    Verrouiller,
    Deverrouiller,
    Cote,
    Altimetrie,
    Texte,
    Etiquette,
    Ligne,
    Arc,
    Polyligne,
    Rectangle,
    Spline,
    Cercle,
    Zone,
    Symbole,
    Legende,
    VuePlan,
    VueCoupe,
    Feuilles,
    Nomenclatures,
}

impl Outil {
    pub fn nom(self) -> &'static str {
        match self {
            Self::Niveau => "Niveau",
            Self::Quadrillage => "Quadrillage",
            Self::Mur => "Mur",
            Self::Terrain => "Terrain",
            Self::LimitePropriete => "Limite de propriété",
            Self::Surfaces => "Éléments horizontaux",
            Self::Dalle => "Dalle",
            Self::Sol => "Sol",
            Self::Toiture => "Toiture",
            Self::Plafond => "Plafond",
            Self::Poteau => "Poteau",
            Self::Poutre => "Poutre",
            Self::Escalier => "Escalier",
            Self::GardeCorps => "Garde-corps",
            Self::Ouverture => "Ouverture",
            Self::Composant => "Composant",
            Self::Piece => "Pièce",
            Self::Arbre => "Arbre",
            Self::Deplacer => "Déplacer",
            Self::Copier => "Copier",
            Self::Tourner => "Tourner",
            Self::Symetrie => "Symétrie",
            Self::Reseau => "Réseau",
            Self::Echelle => "Échelle",
            Self::Decaler => "Décaler",
            Self::Scinder => "Scinder",
            Self::Demolir => "Démolir",
            Self::Reproduire => "Pinceau",
            Self::ChangerHote => "Hôte",
            Self::Inverser => "Inverser",
            Self::Aligner => "Aligner",
            Self::Ajuster => "Ajuster",
            Self::ModifierContour => "Modifier",
            Self::CopierNiveau => "Sur niveaux",
            Self::Jonctions => "Jonctions",
            Self::AttacherToiture => "Attacher toit",
            Self::CreerIdentique => "Identique",
            Self::Supprimer => "Supprimer",
            Self::Cadrer => "Cadre",
            Self::Isoler => "Isoler",
            Self::Masquer => "Masquer",
            Self::MasquerVue => "Masquer vue",
            Self::Afficher => "Afficher",
            Self::Verrouiller => "Verrou",
            Self::Deverrouiller => "Déverrouiller",
            Self::Cote => "Cote",
            Self::Altimetrie => "Altimétrie",
            Self::Texte => "Texte",
            Self::Etiquette => "Étiquette",
            Self::Ligne => "Ligne",
            Self::Arc => "Arc",
            Self::Polyligne => "Polyligne",
            Self::Rectangle => "Rectangle",
            Self::Spline => "Spline",
            Self::Cercle => "Cercle",
            Self::Zone => "Zone remplie",
            Self::Symbole => "Symbole",
            Self::Legende => "Légende",
            Self::VuePlan => "Vue en plan",
            Self::VueCoupe => "Coupe / élévation",
            Self::Feuilles => "Feuilles",
            Self::Nomenclatures => "Nomenclatures",
        }
    }

    /// Fréquence présumée d'usage : seul le premier rang gagne un grand bouton.
    /// L'ordre permet de conserver Mur / Déplacer / Cote en grand le plus longtemps.
    fn priorite(self) -> u8 {
        match self {
            Self::Mur | Self::Deplacer | Self::Cote | Self::ModifierContour => 3,
            Self::Ouverture | Self::Copier | Self::Texte | Self::VueCoupe => 2,
            Self::VuePlan => 1,
            Self::Surfaces | Self::Aligner => 1,
            _ => 0,
        }
    }

    fn commande(self) -> Self {
        match self {
            Self::Dalle | Self::Sol | Self::Plafond | Self::Toiture => Self::Surfaces,
            _ => self,
        }
    }

    pub(in crate::ui) fn forme_dessin(self) -> Option<crate::dessin::dao::OutilDessin> {
        use crate::dessin::dao::OutilDessin as D;
        Some(match self {
            Self::Ligne => D::Ligne,
            Self::Polyligne => D::Polyligne,
            Self::Rectangle => D::Rectangle,
            Self::Spline => D::Spline,
            Self::Cercle => D::Cercle,
            Self::Arc => D::Arc,
            _ => return None,
        })
    }
}

const SURFACES: &[(Outil, &str)] = &[
    (Outil::Dalle, "Dalle structurelle porteuse"),
    (Outil::Sol, "Revêtement et finition de sol"),
    (Outil::Plafond, "Plafond ou faux plafond"),
    (Outil::Toiture, "Couverture et toiture"),
];

/// Paramètres d'une coupe ou d'une élévation à tracer.
pub struct ParametresCoupe {
    pub elevation: bool,
    pub type_vue: String,
    pub echelle: f64,
    pub profondeur: f64,
}

/// Types de coupe et d'élévation du gabarit (échelle, profondeur par défaut).
fn types_coupe() -> Vec<crate::config::gabarit::TypeVue> {
    crate::config::gabarit::actif().types_vues.clone()
}

/// Pictogramme d'une coupe (trait, têtes et flèches) ou d'une élévation (cercle et
/// flèche de visée).
fn icone_vue(p: &egui::Painter, r: egui::Rect, elevation: bool, c: Color32) {
    if !elevation {
        icone(p, r, Outil::VueCoupe, c);
        return;
    }
    let t = egui::Stroke::new(1.3, c);
    let m = r.center() + egui::vec2(0.0, r.height() * 0.08);
    let rayon = r.width() * 0.22;
    p.circle_stroke(m, rayon, t);
    p.add(egui::Shape::convex_polygon(vec![m + egui::vec2(-rayon, -rayon * 0.2), m + egui::vec2(rayon, -rayon * 0.2), m + egui::vec2(0.0, -rayon * 2.1)], c, egui::Stroke::NONE));
    p.line_segment([r.left_bottom() + egui::vec2(r.width() * 0.05, -r.height() * 0.12), r.right_bottom() + egui::vec2(-r.width() * 0.05, -r.height() * 0.12)], egui::Stroke::new(1.0, c));
}

/// Sorte d'élément horizontal d'un outil.
fn sorte_surface(o: Outil) -> Option<crate::modele::SorteDalle> {
    use crate::modele::SorteDalle;
    match o {
        Outil::Dalle => Some(SorteDalle::Dalle),
        Outil::Sol => Some(SorteDalle::Sol),
        Outil::Plafond => Some(SorteDalle::Plafond),
        Outil::Toiture => Some(SorteDalle::Toiture),
        _ => None,
    }
}

/// Types d'éléments horizontaux du gabarit, couches du haut vers le bas.
fn types_surface(o: Outil) -> Vec<(String, Vec<crate::modele::Couche>)> {
    let Some(sorte) = sorte_surface(o) else { return Vec::new() };
    crate::config::gabarit::actif().surfaces(sorte).into_iter().map(|t| (t.nom.clone(), t.couches.clone())).collect()
}

/// Pictogramme d'une catégorie de mur, vu en plan : un trait épais (mur), plusieurs
/// bandes (mur composé), un trait fin (cloison).
fn icone_categorie_mur(p: &egui::Painter, r: egui::Rect, c: crate::modele::CategorieMur, encre: Color32) {
    use crate::modele::CategorieMur;
    let contour = egui::Stroke::new(1.2, encre);
    let (l, h) = (r.width(), r.height());
    let bande = |haut: f32, bas: f32| egui::Rect::from_min_max(egui::pos2(r.left(), r.top() + h * haut), egui::pos2(r.right(), r.top() + h * bas));
    match c {
        CategorieMur::Mur => {
            p.rect_filled(bande(0.3, 0.7), 0.0, encre.gamma_multiply(0.35));
            p.rect_stroke(bande(0.3, 0.7), 0.0, contour, egui::StrokeKind::Inside);
        }
        CategorieMur::Compose => {
            for (a, b, f) in [(0.15, 0.3, 0.15), (0.3, 0.6, 0.45), (0.6, 0.8, 0.25), (0.8, 0.88, 0.15)] {
                p.rect_filled(bande(a, b), 0.0, encre.gamma_multiply(f));
            }
            p.rect_stroke(bande(0.15, 0.88), 0.0, contour, egui::StrokeKind::Inside);
        }
        CategorieMur::Cloison => {
            p.rect_filled(bande(0.42, 0.58), 0.0, encre.gamma_multiply(0.2));
            p.rect_stroke(bande(0.42, 0.58), 0.0, contour, egui::StrokeKind::Inside);
            let _ = l;
        }
    }
}

/// Composition d'un élément horizontal en coupe : couches empilées du haut vers le bas,
/// hauteur proportionnelle à l'épaisseur (2 px au moins), couleur du matériau.
pub(crate) fn pile_couches(ui: &mut egui::Ui, r: egui::Rect, projet: &crate::modele::Projet, couches: &[crate::modele::Couche]) {
    let total: f64 = couches.iter().map(|c| c.epaisseur).sum();
    if total <= 0.0 {
        return;
    }
    let p = ui.painter();
    let n = couches.len() as f32;
    let reste = (r.height() - 2.0 * n).max(0.0);
    let mut y = r.top();
    for c in couches {
        let h = 2.0 + reste * (c.epaisseur / total) as f32;
        let m = c.materiau.as_ref().and_then(|n| projet.materiau(n)).cloned().unwrap_or_else(|| crate::modele::murs::assemblage::materiau_par_defaut(c.materiau.as_deref().unwrap_or("Mur générique")));
        let bande = egui::Rect::from_min_size(egui::pos2(r.left(), y), egui::vec2(r.width(), h));
        peindre_materiau_coupe(p, bande, projet, &m, false);
        p.rect_stroke(bande, 0.0, egui::Stroke::new(0.5, Color32::from_gray(90)), egui::StrokeKind::Inside);
        y += h;
    }
}

/// Projection indicative des limites du mur, bornée à la place disponible dans la palette.
fn limites_croquis_mur(parametres: &crate::modele::edition::ParametresMur, _projet: &crate::modele::Projet, haut_reference: f32, bas_reference: f32) -> (f32, f32) {
    let deplacement = |v: f64| if v > 0.0 { -10.0 } else if v < 0.0 { 10.0 } else { 0.0 };
    let bas = deplacement(parametres.decalage);
    let haut = if parametres.niveau_haut.is_some() { deplacement(parametres.decalage_haut) } else { bas };
    (haut_reference + haut, bas_reference + bas)
}

fn groupes(menu: Menu) -> &'static [(&'static str, &'static [Outil])] {
    use Outil::*;
    const DESSIN_2D: (&str, &[Outil]) = ("Dessin 2D", &[Ligne, Polyligne, Spline, Cercle, Arc]);
    match menu {
        Menu::Modeliser => &[
            ("Construire", &[Mur, Surfaces]),
            ("Site", &[Terrain, LimitePropriete, Arbre]),
            ("Structure et accès", &[Poteau, Poutre, Escalier, GardeCorps]),
            ("Placer", &[Ouverture, Composant, Piece]),
            ("Références", &[Niveau, Quadrillage]),
        ],
        Menu::Modifier => &[
            ("Transformer", &[Deplacer, Copier, Tourner, Symetrie, Reseau, Echelle]),
            ("Ajuster", &[Aligner, Decaler, Ajuster, Scinder, CreerIdentique, Supprimer]),
            ("Éléments", &[ModifierContour, CopierNiveau, Jonctions, AttacherToiture, Demolir, Reproduire, ChangerHote]),
            // verrou (bascule) et visibilité ; Déverrouiller : DC, clic sur le cadenas, menu
            ("Verrou et visibilité", &[Inverser, Verrouiller, Isoler, Masquer, MasquerVue, Afficher]),
        ],
        Menu::Annoter => &[
            ("Vues", &[VuePlan, VueCoupe]),
            ("Cotes et textes", &[Cote, Texte, Etiquette, Altimetrie]),
            // les régions remplies sont des polylignes fermées (Remplissage dans Propriétés)
            DESSIN_2D,
            ("Bibliothèque", &[Symbole, Legende]),
            ("Documents", &[Feuilles, Nomenclatures]),
        ],
        Menu::Cartouche => &[
            ("Cotes et textes", &[Cote, Texte, Etiquette]),
            DESSIN_2D,
            ("Papier", &[Rectangle, Zone]),
        ],
    }
}

const HAUTEUR_OUTILS: f32 = 66.0;
const HAUTEUR_GROUPE: f32 = 86.0;
const ESPACE_COLONNES: f32 = 4.0;
const ESPACE_GROUPES: f32 = 16.0;
const LARGEUR_OPTIONS: f32 = 440.0;
const LARGEUR_REGLAGES: f32 = 34.0;

struct Colonne {
    outils: Vec<Outil>,
    grande: bool,
    largeur: f32,
}

fn largeur_texte(ui: &egui::Ui, texte: &str, taille: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            texte.into(),
            egui::FontId::proportional(taille),
            ui.visuals().text_color(),
        )
        .size()
        .x
}

fn colonnes(ui: &egui::Ui, outils: &[Outil], grands: &[Outil], serre: bool) -> Vec<Colonne> {
    let mut resultat: Vec<Colonne> = Vec::new();
    for &outil in outils {
        if grands.contains(&outil) {
            resultat.push(Colonne {
                outils: vec![outil],
                grande: true,
                largeur: (largeur_texte(ui, outil.nom(), 12.0) + 20.0).max(62.0),
            });
        } else {
            let (police, marge) = if serre { (11.0, 24.0) } else { (12.0, 34.0) };
            let largeur = largeur_texte(ui, outil.nom(), police) + marge;
            if let Some(colonne) = resultat
                .last_mut()
                .filter(|c| !c.grande && c.outils.len() < 3)
            {
                colonne.outils.push(outil);
                colonne.largeur = colonne.largeur.max(largeur);
            } else {
                resultat.push(Colonne {
                    outils: vec![outil],
                    grande: false,
                    largeur,
                });
            }
        }
    }
    resultat
}

fn largeur_groupe(ui: &egui::Ui, nom: &str, colonnes: &[Colonne]) -> f32 {
    (colonnes.iter().map(|c| c.largeur).sum::<f32>()
        + ESPACE_COLONNES * colonnes.len().saturating_sub(1) as f32)
        .max(largeur_texte(ui, nom, 11.0) + 8.0)
}

fn largeur_commandes(ui: &egui::Ui, menu: Menu, grands: &[Outil], serre: bool) -> f32 {
    groupes(menu)
        .iter()
        .map(|(nom, outils)| largeur_groupe(ui, nom, &colonnes(ui, outils, grands, serre)))
        .sum::<f32>()
        + espace_groupes(serre) * groupes(menu).len().saturating_sub(1) as f32
}

fn espace_groupes(serre: bool) -> f32 {
    if serre { 8.0 } else { ESPACE_GROUPES }
}

enum Champ {
    Choix(&'static str, &'static [&'static str]),
    Nombre(&'static str, f64, &'static str),
    Saisie(&'static str, &'static str),
}
enum Valeur {
    Texte(String),
    Nombre(f64),
}

#[derive(Default)]
pub struct RubanEdition {
    /// Sélection du projet, renseignée par l'application avant `afficher` : active
    /// Copier sur niveaux (éléments) et Jonctions (murs).
    pub selection_elements: bool,
    pub selection_murs: bool,
    /// Bouton Jonctions de cette image : le menu s'ancre dessus.
    pub bouton_jonctions: Option<egui::Response>,
    pub modifier_famille: bool,
    pub ouvrir_famille: bool,
    pub type_garde_corps: Option<String>,
    pub prolonger_angle: bool,
    pub selection_mur: bool,
    selection: Option<Outil>,
    derniere_surface: Option<Outil>,
    valeurs: BTreeMap<(Outil, usize), Valeur>,
    pub niveau_mur: Option<String>,
    pub niveau_haut_mur: Option<String>,
    pub decalage_haut_mur: f64,
    pub phase_creation: Option<String>,
    pub sous_projet_creation: Option<String>,
    pub variante_creation: Option<String>,
    /// Sens inversé des prochains murs (Espace pendant le tracé).
    pub sens_inverse: bool,
    /// Forme tracée par l'outil Mur (moteur du dessin 2D) : ligne, rectangle, arc, cercle.
    pub forme_mur: crate::dessin::dao::OutilDessin,
    ajout_creation: Option<crate::modele::edition::ListeCreation>,
    nom_creation: String,
    jeu_creation: String,
    erreur_creation: Option<String>,
    /// Type des prochains murs ; None : générique, épaisseur saisie.
    pub type_mur: Option<String>,
    /// Type de coupe ou d'élévation choisi (indice dans les types de vues du gabarit).
    type_coupe: usize,
    coupe_identique: Option<(String, f64, bool)>,
    pub(crate) ouverture_type_identique: Option<String>,
    /// Demande d'ouverture de l'éditeur de familles : type à éditer (None : nouveau).
    pub demande_famille: Option<Option<String>>,
    /// Charger une famille d'ouverture depuis un fichier (demandé par le sélecteur).
    pub demande_charger_famille: bool,
    /// Niveau des prochaines dalles (None : le premier).
    pub niveau_dalle: Option<String>,
    /// Onglet de catégorie ouvert dans le sélecteur de type de mur.
    categorie_mur: Option<crate::modele::CategorieMur>,
    /// Onglet du sélecteur de type d'ouverture (None : celui du type courant).
    categorie_ouverture: Option<crate::modele::GenreOuverture>,
    /// Type de mur du gabarit choisi, à copier dans le projet.
    type_mur_gabarit: Option<crate::config::gabarit::TypeGabarit>,
    /// Toiture : arêtes tracées sans pente (toiture plate).
    toiture_plate: bool,
    /// Tracé des dalles par deux coins (sinon polygone).
    /// Forme tracée par les éléments horizontaux : polygone (défaut), rectangle, cercle.
    pub forme_dalle: crate::dessin::dao::OutilDessin,
    /// Type prédéfini choisi pour chaque sorte d'élément horizontal (indice).
    type_surface: BTreeMap<Outil, usize>,
    surface_identique: Option<(Outil, crate::modele::TypeCompose)>,
    /// Type composé en cours d'édition (fenêtre « Type de mur »).
    editeur_type: Option<EditeurType>,
    /// Matériau dont la fenêtre doit s'ouvrir (double-clic dans l'éditeur de type).
    materiau_a_editer: Option<String>,
}

/// Brouillon d'un type de mur composé : rien n'est écrit dans le projet avant
/// « Enregistrer ».
struct EditeurType {
    /// None : nouveau type.
    id: Option<String>,
    nom: String,
    categorie: crate::modele::CategorieMur,
    couches: Vec<crate::modele::Couche>,
    /// Matériaux du projet et ceux ajoutés dans l'éditeur (couleur, motif).
    materiaux: Vec<crate::modele::Materiau>,
    erreur: Option<String>,
    /// Couche en cours de glisser-déposer.
    glisse: Option<usize>,
}

fn nom_champ(c: &Champ) -> &'static str {
    match c {
        Champ::Nombre(n, _, _) | Champ::Saisie(n, _) | Champ::Choix(n, _) => n,
    }
}

fn champs(o: Outil) -> Vec<Champ> {
    use Champ::*;
    use Outil::*;
    match o {
        Surfaces | Niveau | Terrain | LimitePropriete | ModifierContour | CopierNiveau | Jonctions | AttacherToiture | CreerIdentique | Supprimer | Cadrer | Isoler | Masquer | MasquerVue | Afficher | Verrouiller | Deverrouiller | Feuilles | Nomenclatures => vec![],
        Quadrillage => vec![Saisie("Repère", "A"), Choix("Tracé", &["Droit", "Arc"])],
        Mur => vec![
            Choix(
                "Implantation",
                &["Face extérieure", "Axe", "Face intérieure", "Axe du porteur"],
            ),
            Nombre("Hauteur", 2800.0, " mm"),
            Nombre("Épaisseur", 300.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Dalle => vec![
            Nombre("Épaisseur", 200.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Sol => vec![
            Nombre("Épaisseur", 15.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Toiture => vec![
            Nombre("Épaisseur", 300.0, " mm"),
            Nombre("Décalage sous-face", 2800.0, " mm"),
            Nombre("Pente", 35.0, " °"),
        ],
        Plafond => vec![
            Nombre("Épaisseur", 25.0, " mm"),
            Nombre("Hauteur sous plafond", 2500.0, " mm"),
        ],
        Poteau | Poutre => vec![],
        Escalier => vec![
            Nombre("Largeur", 1000.0, " mm"),
            Nombre("Hauteur de marche", 175.0, " mm"),
            Choix("Garde-corps", &["Deux côtés", "Gauche", "Droite", "Aucun"]),
        ],
        GardeCorps => vec![
            Nombre("Hauteur", 1000.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
            Choix("Remplissage", &["Barreaudage", "Panneau plein", "Vitrage"]),
        ],
        Ouverture => vec![
            Choix("Ouverture", &["Porte", "Fenêtre", "Libre"]),
            Nombre("Largeur", 900.0, " mm"),
            Nombre("Hauteur", 2100.0, " mm"),
            Nombre("Allège", 900.0, " mm"),
        ],
        Composant | Piece | Arbre => vec![],
        Deplacer => vec![
            Nombre("Distance", 1000.0, " mm"),
            Choix("Contrainte", &["Libre", "Horizontale", "Verticale"]),
        ],
        Copier => vec![Nombre("Copies", 1.0, ""), Nombre("Distance", 1000.0, " mm")],
        Tourner => vec![Nombre("Angle", 90.0, " °")],
        Symetrie | Reseau | Echelle | Decaler | Scinder | Demolir | Reproduire | ChangerHote | Inverser => vec![],
        Aligner => vec![Choix("Référence", &["Face", "Axe"])],
        Ajuster => vec![Choix("Mode", &["Ajuster ou prolonger", "Prolonger seulement"])],
        Cote => vec![
            Choix("Mode", &["Alignée", "Orthogonale"]),
            Nombre("Décalage papier", 8.0, " mm"),
        ],
        Altimetrie => vec![
            Choix("Référence", &["Projet", "Niveau courant"]),
            Choix("Précision", &["0.00 m", "0.000 m"]),
        ],
        Texte => vec![
            Choix("Ligne de repère", &["Sans flèche", "Avec flèche"]),
            Nombre("Hauteur papier", 2.5, " mm"),
            Choix("Alignement", &["Gauche", "Centre", "Droite"]),
        ],
        Etiquette => vec![
            Choix("Contenu", &["Type", "Repère", "Matériau"]),
            Nombre("Hauteur papier", 2.5, " mm"),
        ],
        Ligne | Arc | Polyligne | Rectangle | Spline | Cercle => vec![],
        Zone => vec![
            Choix("Motif", &["Diagonales 45°", "Béton", "Isolant", "Plein"]),
            Nombre("Pas papier", 2.0, " mm"),
        ],
        Symbole => vec![Choix(
            "Symbole",
            &["Nord", "Repère de coupe", "Repère de détail"],
        )],
        Legende => vec![Choix(
            "Contenu",
            &["Matériaux", "Types de murs", "Symboles"],
        )],
        VuePlan => vec![Nombre("Hauteur de coupe", 1200.0, " mm")],
        VueCoupe => vec![Nombre("Profondeur", 15_000.0, " mm")],
    }
}

impl RubanEdition {
    /// Outil Coupe actif : sorte (coupe ou élévation), type, échelle et profondeur.
    pub fn coupe_active(&self) -> Option<ParametresCoupe> {
        if self.selection != Some(Outil::VueCoupe) {
            return None;
        }
        let types = types_coupe();
        let t = &types[self.type_coupe.min(types.len() - 1)];
        let profondeur = match self.valeurs.get(&(Outil::VueCoupe, 0)) {
            Some(Valeur::Nombre(v)) => *v,
            _ => t.profondeur,
        };
        let (type_vue, echelle, elevation) = self.coupe_identique.clone().unwrap_or_else(|| (t.nom.clone(), t.echelle, t.elevation));
        Some(ParametresCoupe { elevation, type_vue, echelle, profondeur })
    }

    pub fn activer_coupe(&mut self) {
        self.selection = Some(Outil::VueCoupe);
        self.coupe_identique = None;
    }
    pub(crate) fn coupe_identique(&mut self, vue: &crate::modele::Vue) -> Result<(), String> {
        let crate::modele::GenreVue::Coupe { profondeur, elevation, .. } = vue.genre else { return Err("Sélectionnez une coupe ou une élévation.".into()) };
        let types = types_coupe();
        self.type_coupe = types.iter().position(|t| t.nom == vue.type_vue && t.elevation == elevation).or_else(|| types.iter().position(|t| t.elevation == elevation)).unwrap_or(0);
        self.selection = Some(Outil::VueCoupe);
        self.coupe_identique = Some((vue.type_vue.clone(), vue.echelle, elevation));
        self.valeurs.insert((Outil::VueCoupe, 0), Valeur::Nombre(profondeur));
        Ok(())
    }
    pub(crate) fn quadrillage_identique(&mut self, nom: &str) {
        self.selection = Some(Outil::Quadrillage);
        self.valeurs.insert((Outil::Quadrillage, 0), Valeur::Texte(nom.into()));
    }

    /// Sélecteur de type de l'outil Coupe, comme celui des éléments horizontaux : un
    /// onglet Coupe, un onglet Élévation, leurs types (échelle, profondeur par défaut).
    fn selecteur_coupe(&mut self, ui: &mut egui::Ui) {
        let types = types_coupe();
        let k = self.type_coupe.min(types.len() - 1);
        let courant = &types[k];
        let sorte = |e: bool| if e { "Élévation" } else { "Coupe" };
        let mut choix: Option<usize> = None;
        let elevation_affichee = self.coupe_identique.as_ref().map_or(courant.elevation, |c| c.2);
        let nom_affiche = self.coupe_identique.as_ref().map_or(courant.nom.as_str(), |c| c.0.as_str());
        let echelle = |e: f64, profondeur: f64| format!("1:{e} · {} m", crate::formats::ifc::step::format_real(profondeur / 1000.0));
        let titre = format!("{} · {}", sorte(elevation_affichee), nom_affiche);
        let vignette = |p: &egui::Painter, r: egui::Rect, c: Color32| icone_vue(p, r, elevation_affichee, c);
        let bouton = selecteur::entete(ui, &titre, Some(&format!("1:{}", self.coupe_identique.as_ref().map_or(courant.echelle, |c| c.1))), &format!("{titre}
{}", courant.aide), Some(&vignette));
        let rangs = [false, true].iter().map(|e| types.iter().filter(|t| t.elevation == *e).count()).max().unwrap_or(1);
        let largeur_liste = selecteur::largeur_liste(ui, types.iter().map(|t| t.aide.as_str()), 90.0, 2, 120.0);
        selecteur::liste(&bouton, largeur_liste, selecteur::HAUTEUR_CADRE + rangs as f32 * selecteur::HAUTEUR_LIGNE_DOUBLE, |ui| {
            if let Some(e) = selecteur::onglets(ui, &[false, true], courant.elevation, 120.0, |e| sorte(e).into(), "", |p, r, e, c| icone_vue(p, r, e, c)) {
                choix = types.iter().position(|t| t.elevation == e);
            }
            for (i, t) in types.iter().enumerate().filter(|(_, t)| t.elevation == courant.elevation) {
                let l = selecteur::Ligne { nom: &t.nom, courant: i == k, etiquette: None, sous_titre: Some(&t.aide), detail: Some(echelle(t.echelle, t.profondeur)), actions: &[] };
                let mut v = |ui: &mut egui::Ui, r: egui::Rect| icone_vue(ui.painter(), egui::Rect::from_center_size(r.center(), egui::vec2(20.0, 20.0)), t.elevation, ui.visuals().weak_text_color());
                if selecteur::ligne(ui, &l, Some(&mut v)).choisi {
                    choix = Some(i);
                }
            }
        });
        ui.label(RichText::new(&courant.aide).small().weak());
        if let Some(i) = choix {
            self.type_coupe = i;
            self.coupe_identique = None;
            // profondeur par défaut du type choisi
            self.valeurs.insert((Outil::VueCoupe, 0), Valeur::Nombre(types[i].profondeur));
        }
        ui.add_space(6.0);
        egui::Grid::new("parametres_coupe").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label("Profondeur");
            let v = self.valeurs.entry((Outil::VueCoupe, 0)).or_insert(Valeur::Nombre(types[k].profondeur));
            if let Valeur::Nombre(v) = v {
                ui.add(egui::DragValue::new(v).speed(50.0).range(100.0..=200_000.0).dimensions());
            }
            ui.end_row();
            ui.label("Échelle");
            ui.label(format!("1:{}", self.coupe_identique.as_ref().map_or(types[k].echelle, |c| c.1)));
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Dans une vue en plan, cliquer le début puis la fin de la ligne : la vue regarde vers sa gauche (vers le haut pour une ligne tracée de gauche à droite), sur la longueur de la ligne et la profondeur.").small().weak());
    }

    /// Palette de l'outil Vue en plan : un bouton par niveau (plans existants
    /// indiqués). Retourne le niveau et la hauteur de coupe (mm) du plan à créer.
    pub fn proprietes_vues(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) -> Option<Option<(String, f64)>> {
        match self.selection {
            Some(Outil::VuePlan) => {}
            Some(Outil::VueCoupe) => {
                self.selecteur_coupe(ui);
                return Some(None);
            }
            _ => return None,
        }
        ui.label(RichText::new("Créer une vue en plan").strong());
        let Some(projet) = projet else {
            ui.label(RichText::new("Créez ou ouvrez un projet natif.").weak());
            return Some(None);
        };
        let v = self.valeurs.entry((Outil::VuePlan, 0)).or_insert(Valeur::Nombre(1200.0));
        let Valeur::Nombre(hauteur) = v else { return Some(None) };
        ui.horizontal(|ui| {
            ui.label("Hauteur du plan de coupe");
            ui.add(egui::DragValue::new(hauteur).speed(10.0).dimensions());
        });
        let hauteur = *hauteur;
        let mut choix = None;
        egui::Grid::new("niveaux_vues_plan").num_columns(3).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for n in &projet.niveaux {
                let plans = projet.vues.iter().filter(|v| matches!(&v.genre, crate::modele::GenreVue::Plan { niveau, .. } if niveau == &n.id)).count();
                ui.label(&n.nom);
                ui.label(RichText::new(match plans { 0 => "aucun plan".to_string(), 1 => "1 plan".to_string(), k => format!("{k} plans") }).small().weak());
                if ui.button("Créer").on_hover_text("Créer le plan d'étage de ce niveau et l'ouvrir").clicked() {
                    choix = Some((n.id.clone(), hauteur));
                }
                ui.end_row();
            }
        });
        if projet.niveaux.is_empty() {
            ui.label(RichText::new("Le projet n'a pas encore de niveau (le premier mur en crée un).").weak());
        }
        Some(choix)
    }

    /// Paramètres de l'élément horizontal à tracer (dalle, sol, plafond, toiture), si
    /// cet outil est actif.
    pub fn dalle_active(&self) -> Option<crate::modele::edition::ParametresDalle> {
        let outil = self.selection?;
        let sorte = sorte_surface(outil)?;
        let types = types_surface(outil);
        let (nom, couches) = types.get(self.type_surface.get(&outil).copied().unwrap_or(0))?.clone();
        let nombre = |k| match (self.valeurs.get(&(outil, k)), champs(outil).get(k)) {
            (Some(Valeur::Nombre(v)), _) => *v,
            (_, Some(Champ::Nombre(_, d, _))) => *d,
            _ => 0.0,
        };
        let pente = (outil == Outil::Toiture && !self.toiture_plate).then(|| nombre(2));
        Some(crate::modele::edition::ParametresDalle {
            sorte,
            type_dalle: self.surface_identique.as_ref().filter(|(o,_)| *o == outil).map(|(_,t)| t.clone()).unwrap_or_else(|| crate::modele::TypeCompose { id: String::new(), nom, couches, categorie: None, couches_detaillees: true, origine: None }),
            pente,
            niveau: self.niveau_dalle.clone(),
            decalage: nombre(1),
            phase: self.phase_creation.clone(),
            sous_projet: self.sous_projet_creation.clone(),
            variante: self.variante_creation.clone(),
        })
    }

    pub fn mur_actif(&self) -> bool {
        self.selection == Some(Outil::Mur)
    }
    pub fn outil(&self) -> Option<Outil> { self.selection }
    pub fn nombre_option(&self, outil: Outil, index: usize) -> f64 {
        match self.valeurs.get(&(outil,index)) {
            Some(Valeur::Nombre(v)) => *v,
            _ => match champs(outil).get(index) { Some(Champ::Nombre(_,v,_)) => *v, _=>0.0 },
        }
    }
    pub fn definir_nombre_option(&mut self, outil: Outil, index: usize, valeur: f64) {
        self.valeurs.insert((outil, index), Valeur::Nombre(valeur));
    }
    pub fn definir_texte_option(&mut self, outil: Outil, index: usize, valeur: &str) {
        self.valeurs.insert((outil, index), Valeur::Texte(valeur.into()));
    }
    pub fn texte_option(&self, outil: Outil, index: usize) -> String {
        match self.valeurs.get(&(outil,index)) {
            Some(Valeur::Texte(v)) => v.clone(),
            _ => match champs(outil).get(index) { Some(Champ::Saisie(_,v)) => (*v).into(), Some(Champ::Choix(_,v)) => v[0].into(), _=>String::new() },
        }
    }
    pub fn a_un_outil(&self) -> bool {
        self.selection.is_some()
    }
    /// Outil d'élément horizontal de la sorte, type courant de la sorte.
    pub fn activer_surface(&mut self, sorte: crate::modele::SorteDalle) {
        use crate::modele::SorteDalle;
        self.surface_identique = None;
        let o = match sorte {
            SorteDalle::Dalle => Outil::Dalle,
            SorteDalle::Sol => Outil::Sol,
            SorteDalle::Plafond => Outil::Plafond,
            SorteDalle::Toiture => Outil::Toiture,
        };
        self.selection = Some(o);
        self.derniere_surface = Some(o);
    }

    pub(crate) fn surface_identique(&mut self, p: &crate::modele::Projet, d: &crate::modele::Dalle) -> Result<(), String> {
        let type_dalle = p.type_dalle(&d.type_dalle).ok_or("Type de l'élément introuvable.")?.clone();
        let pentes: Vec<_> = d.pentes.iter().flatten().copied().collect();
        if !pentes.is_empty() && pentes.iter().any(|x| *x != pentes[0]) { return Err("Créer à l'identique ne prend pas encore en charge les pentes différentes par arête.".into()); }
        let pente = pentes.first().copied().flatten();
        if pente.is_some() && d.sorte != crate::modele::SorteDalle::Toiture { return Err("Créer à l'identique ne prend pas encore en charge cette pente.".into()); }
        self.activer_surface(d.sorte);
        let outil = self.selection.unwrap();
        self.surface_identique = Some((outil, type_dalle));
        self.niveau_dalle = Some(d.niveau.clone());
        self.valeurs.insert((outil, 1), Valeur::Nombre(d.decalage));
        if let Some(pente) = pente { self.valeurs.insert((outil, 2), Valeur::Nombre(pente)); }
        self.toiture_plate = pente.is_none();
        self.phase_creation = d.phase.clone(); self.sous_projet_creation = d.sous_projet.clone(); self.variante_creation = d.variante.clone();
        Ok(())
    }

    pub fn activer_dalle(&mut self) {
        self.activer_surface(crate::modele::SorteDalle::Dalle);
    }

    /// Choisit le type de la sorte active par son nom (gabarit).
    pub fn choisir_type_surface(&mut self, nom: &str) -> Result<(), String> {
        let o = self.selection.filter(|o| sorte_surface(*o).is_some()).ok_or("Aucun outil d'élément horizontal actif.")?;
        let i = types_surface(o).iter().position(|(n, _)| n == nom).ok_or_else(|| format!("Type « {nom} » absent du gabarit."))?;
        self.type_surface.insert(o, i);
        self.surface_identique = None;
        Ok(())
    }

    /// Règle un paramètre numérique de l'outil d'élément horizontal actif (par son nom).
    pub fn regler_surface(&mut self, champ: &str, valeur: f64) -> Result<(), String> {
        let o = self.selection.filter(|o| sorte_surface(*o).is_some()).ok_or("Aucun outil d'élément horizontal actif.")?;
        let k = champs(o).iter().position(|c| matches!(c, Champ::Nombre(n, _, _) if n.to_lowercase().starts_with(&champ.to_lowercase()))).ok_or_else(|| format!("Paramètre « {champ} » inconnu."))?;
        self.valeurs.insert((o, k), Valeur::Nombre(valeur));
        Ok(())
    }

    /// Toiture : toutes les arêtes tracées définissent la pente.
    pub fn definir_pente_toiture(&mut self, oui: bool) {
        self.toiture_plate = !oui;
    }

    pub(crate) fn activer_outil(&mut self, outil: Outil) { self.selection = Some(outil); }
    pub(crate) fn mur_identique(&mut self, projet: &crate::modele::Projet, mur: &crate::modele::Mur) -> Result<(), String> {
        let (bas, haut) = projet.bas_haut(mur).ok_or("Contraintes du mur invalides.")?;
        self.selection = Some(Outil::Mur);
        self.type_mur = Some(mur.assemblage.as_ref().and_then(|id| projet.assemblage(id)).map_or_else(|| mur.type_mur.clone(), |a| a.type_mur.clone()));
        self.niveau_mur = Some(mur.niveau.clone());
        self.valeurs.insert((Outil::Mur, 1), Valeur::Nombre(haut-bas));
        self.valeurs.insert((Outil::Mur, 3), Valeur::Nombre(mur.decalage_bas));
        match &mur.haut { crate::modele::Haut::Niveau { niveau, decalage } => { self.niveau_haut_mur = Some(niveau.clone()); self.decalage_haut_mur = *decalage; }, crate::modele::Haut::Libre { .. } => { self.niveau_haut_mur = None; self.decalage_haut_mur = 0.0; } }
        self.phase_creation = mur.phase.clone(); self.sous_projet_creation = mur.sous_projet.clone(); self.variante_creation = mur.variante.clone();
        Ok(())
    }
    pub(crate) fn ouverture_identique(&mut self, o: &crate::modele::Ouverture) -> Result<(), String> {
        if o.representation.is_some() { return Err("Cette menuiserie importée ne peut pas encore être créée à l'identique.".into()); }
        self.ouverture_type_identique = o.type_ouverture.clone();
        self.selection = Some(Outil::Ouverture);
        self.valeurs.insert((Outil::Ouverture, 0), Valeur::Texte(match o.genre { crate::modele::GenreOuverture::Porte => "Porte", crate::modele::GenreOuverture::Fenetre => "Fenêtre", crate::modele::GenreOuverture::Libre => "Libre" }.into()));
        for (i,v) in [(1,o.largeur), (2,o.hauteur), (3,o.allege)] { self.valeurs.insert((Outil::Ouverture,i), Valeur::Nombre(v)); }
        Ok(())
    }

    pub(crate) fn activer_structure(&mut self, poutre: bool) {
        self.selection = Some(if poutre { Outil::Poutre } else { Outil::Poteau });
    }
    pub fn activer_mur(&mut self) {
        self.selection = Some(Outil::Mur);
    }
    pub fn annuler(&mut self) {
        self.sens_inverse = false;
        self.coupe_identique = None;
        self.surface_identique = None;
        self.ouverture_type_identique = None;
        self.selection = None;
    }
    pub fn parametres_mur(&self) -> crate::modele::edition::ParametresMur {
        use crate::modele::edition::{Implantation, ParametresMur};
        let nombre = |k, defaut| match self.valeurs.get(&(Outil::Mur, k)) {
            Some(Valeur::Nombre(v)) => *v,
            _ => defaut,
        };
        let implantation = match self.valeurs.get(&(Outil::Mur, 0)) {
            Some(Valeur::Texte(v)) if v == "Axe" => Implantation::Axe,
            Some(Valeur::Texte(v)) if v == "Face intérieure" => Implantation::Interieure,
            Some(Valeur::Texte(v)) if v == "Axe du porteur" => Implantation::AxePorteur,
            _ => Implantation::Exterieure,
        };
        ParametresMur {
            hauteur: nombre(1, 2800.0),
            epaisseur: nombre(2, 300.0),
            implantation,
            niveau: self.niveau_mur.clone(),
            decalage: nombre(3, 0.0),
            niveau_haut: self.niveau_haut_mur.clone(),
            decalage_haut: self.decalage_haut_mur,
            phase: self.phase_creation.clone(),
            sous_projet: self.sous_projet_creation.clone(),
            variante: self.variante_creation.clone(),
            type_mur: self.type_mur.clone(),
            inverse: self.sens_inverse,
        }
    }

}

#[derive(Clone, Copy)]
enum Glyphe {
    Croix,
    Liste,
}

/// Pictogramme générique d'une commande des bandeaux Début et Sortie.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icone {
    Collaborer,
    Synchro,
    Nouveau,
    Ouvrir,
    Fermer,
    Importer,
    Fiche,
    Options,
    Nord,
    /// Origine et cote NGF : repère X, Y, Z.
    Coordonnees,
    /// Lieu géographique : globe et repère.
    Lieu,
    Enregistrer,
    EnregistrerSous,
    Exporter(&'static str),
    /// Charger la famille dans le projet : document et flèche qui y entre.
    Charger,
    /// Pictogramme d'un outil de Documenter.
    /// Outils et vues de l'éditeur de familles, par leur clé.
    Famille(&'static str),
}

/// Commande d'un bandeau à icônes (Début, Sortie).
pub struct Commande {
    pub cle: &'static str,
    pub nom: &'static str,
    pub icone: Icone,
    pub aide: &'static str,
    pub actif: bool,
    /// Bouton enfoncé (bascule).
    pub enfonce: bool,
}

