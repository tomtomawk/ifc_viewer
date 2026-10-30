//! Présentation des commandes d'édition. Le contrôleur du viewer consomme les
//! paramètres des outils opérationnels ; les outils à venir sont désactivés.
use crate::options::Dimensions;
use eframe::egui::{self, Align, Color32, RichText, Sense};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Modeliser,
    Modifier,
    Annoter,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) enum Outil {
    Niveau,
    Quadrillage,
    Mur,
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
    Deplacer,
    Copier,
    Tourner,
    Aligner,
    Ajuster,
    CreerIdentique,
    Supprimer,
    Cadrer,
    Isoler,
    Masquer,
    Cote,
    Altimetrie,
    Texte,
    Etiquette,
    Ligne,
    Arc,
    Polyligne,
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
    fn nom(self) -> &'static str {
        match self {
            Self::Niveau => "Niveau",
            Self::Quadrillage => "Quadrillage",
            Self::Mur => "Mur",
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
            Self::Deplacer => "Déplacer",
            Self::Copier => "Copier",
            Self::Tourner => "Tourner",
            Self::Aligner => "Aligner",
            Self::Ajuster => "Ajuster",
            Self::CreerIdentique => "Créer à l'identique",
            Self::Supprimer => "Supprimer",
            Self::Cadrer => "Cadrer",
            Self::Isoler => "Isoler",
            Self::Masquer => "Masquer",
            Self::Cote => "Cote",
            Self::Altimetrie => "Altimétrie",
            Self::Texte => "Texte",
            Self::Etiquette => "Étiquette",
            Self::Ligne => "Ligne",
            Self::Arc => "Arc",
            Self::Polyligne => "Polyligne",
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
            Self::Mur | Self::Deplacer | Self::Cote => 3,
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
fn types_coupe() -> Vec<crate::gabarit::TypeVue> {
    crate::gabarit::actif().types_vues.clone()
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
    crate::gabarit::actif().surfaces(sorte).into_iter().map(|t| (t.nom.clone(), t.couches.clone())).collect()
}

/// Composition d'un élément horizontal en coupe : couches empilées du haut vers le bas,
/// hauteur proportionnelle à l'épaisseur (2 px au moins), couleur du matériau.
fn pile_couches(ui: &mut egui::Ui, r: egui::Rect, couches: &[crate::modele::Couche]) {
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
        let m = crate::modele::assemblage::materiau_par_defaut(c.materiau.as_deref().unwrap_or(""));
        let [cr, cg, cb] = m.couleur;
        let bande = egui::Rect::from_min_size(egui::pos2(r.left(), y), egui::vec2(r.width(), h));
        p.rect_filled(bande, 0.0, Color32::from_rgb(cr, cg, cb));
        p.rect_stroke(bande, 0.0, egui::Stroke::new(0.5, Color32::from_gray(90)), egui::StrokeKind::Inside);
        y += h;
    }
}

/// Projection indicative des limites du mur, bornée à la place disponible dans la palette.
fn limites_croquis_mur(parametres: &crate::modele::edition::ParametresMur, projet: &crate::modele::Projet, haut_reference: f32, bas_reference: f32) -> (f32, f32) {
    let base = parametres.niveau.as_ref().and_then(|id| projet.niveau(id))
        .or_else(|| projet.niveaux.first()).map_or(0.0, |n| n.altitude);
    let bas = base + parametres.decalage;
    let haut = parametres.niveau_haut.as_ref().and_then(|id| projet.niveau(id))
        .map_or(bas + parametres.hauteur, |n| n.altitude + parametres.decalage_haut);
    let echelle = (bas_reference - haut_reference) as f64 / 2800.0;
    let projeter = |z: f64, repli: f32| {
        let y = (bas_reference as f64 - (z - base) * echelle) as f32;
        if y.is_finite() { y } else { repli }
    };
    let marge = (bas_reference - haut_reference) * 0.38;
    let yh = projeter(haut, haut_reference).clamp(haut_reference - marge, haut_reference + marge);
    let yb = projeter(bas, bas_reference).clamp(bas_reference - marge, bas_reference + marge);
    (yh, yb)
}

fn groupes(menu: Menu) -> &'static [(&'static str, &'static [Outil])] {
    use Outil::*;
    match menu {
        Menu::Modeliser => &[
            ("Construire", &[Mur, Surfaces]),
            ("Structure et accès", &[Poteau, Poutre, Escalier, GardeCorps]),
            ("Placer", &[Ouverture, Composant]),
            ("Références", &[Niveau, Quadrillage]),
        ],
        Menu::Modifier => &[
            ("Transformer", &[Deplacer, Copier, Tourner]),
            ("Ajuster", &[Aligner, Ajuster]),
            ("Édition", &[CreerIdentique, Supprimer]),
            ("Visibilité", &[Cadrer, Isoler, Masquer]),
        ],
        Menu::Annoter => &[
            ("Vues", &[VuePlan, VueCoupe]),
            ("Cotes et textes", &[Cote, Texte, Etiquette, Altimetrie]),
            ("Dessin 2D", &[Ligne, Polyligne, Spline, Cercle, Arc, Zone]),
            ("Bibliothèque", &[Symbole, Legende]),
            ("Documents", &[Feuilles, Nomenclatures]),
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

fn largeur_libelle(ui: &egui::Ui, outil: Outil) -> f32 {
    largeur_texte(ui, outil.nom(), 12.0)
}

fn colonnes(ui: &egui::Ui, outils: &[Outil], grands: &[Outil]) -> Vec<Colonne> {
    let mut resultat: Vec<Colonne> = Vec::new();
    for &outil in outils {
        if grands.contains(&outil) {
            resultat.push(Colonne {
                outils: vec![outil],
                grande: true,
                largeur: (largeur_libelle(ui, outil) + 20.0).max(62.0),
            });
        } else {
            let largeur = largeur_libelle(ui, outil) + 34.0;
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

fn largeur_commandes(ui: &egui::Ui, menu: Menu, grands: &[Outil]) -> f32 {
    groupes(menu)
        .iter()
        .map(|(nom, outils)| largeur_groupe(ui, nom, &colonnes(ui, outils, grands)))
        .sum::<f32>()
        + ESPACE_GROUPES * groupes(menu).len().saturating_sub(1) as f32
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
    selection: Option<Outil>,
    derniere_surface: Option<Outil>,
    valeurs: BTreeMap<(Outil, usize), Valeur>,
    pub niveau_mur: Option<String>,
    pub niveau_haut_mur: Option<String>,
    pub decalage_haut_mur: f64,
    pub phase_creation: Option<String>,
    pub sous_projet_creation: Option<String>,
    pub variante_creation: Option<String>,
    pub chainer_murs: bool,
    /// Forme tracée par l'outil Mur (moteur du dessin 2D) : ligne, rectangle, arc, cercle.
    pub forme_mur: crate::dessin2d::OutilDessin,
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
    /// Niveau des prochaines dalles (None : le premier).
    pub niveau_dalle: Option<String>,
    /// Type de mur du gabarit choisi, à copier dans le projet.
    type_mur_gabarit: Option<crate::gabarit::TypeGabarit>,
    /// Toiture : arêtes tracées sans pente (toiture plate).
    toiture_plate: bool,
    /// Tracé des dalles par deux coins (sinon polygone).
    /// Forme tracée par les éléments horizontaux : polygone (défaut), rectangle, cercle.
    pub forme_dalle: crate::dessin2d::OutilDessin,
    /// Type prédéfini choisi pour chaque sorte d'élément horizontal (indice).
    type_surface: BTreeMap<Outil, usize>,
    surface_identique: Option<(Outil, crate::modele::TypeCompose)>,
    /// Type composé en cours d'édition (fenêtre « Type de mur »).
    editeur_type: Option<EditeurType>,
}

/// Brouillon d'un type de mur composé : rien n'est écrit dans le projet avant
/// « Enregistrer ».
struct EditeurType {
    /// None : nouveau type.
    id: Option<String>,
    nom: String,
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
        Surfaces | Niveau | CreerIdentique | Supprimer | Cadrer | Isoler | Masquer | Feuilles | Nomenclatures => vec![],
        Quadrillage => vec![Saisie("Repère", "A"), Choix("Tracé", &["Droit", "Arc"])],
        Mur => vec![
            Choix(
                "Implantation",
                &["Axe", "Face extérieure", "Face intérieure", "Axe du porteur"],
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
            Choix("Forme", &["Droit", "Quart tournant", "Demi tournant"]),
            Nombre("Largeur", 1000.0, " mm"),
            Nombre("Hauteur à franchir", 2800.0, " mm"),
            Nombre("Marches", 16.0, ""),
        ],
        GardeCorps => vec![
            Nombre("Hauteur", 1000.0, " mm"),
            Nombre("Décalage", 0.0, " mm"),
        ],
        Ouverture => vec![
            Choix("Ouverture", &["Porte", "Fenêtre", "Libre"]),
            Nombre("Largeur", 900.0, " mm"),
            Nombre("Hauteur", 2100.0, " mm"),
            Nombre("Allège", 900.0, " mm"),
        ],
        Composant => vec![
            Choix(
                "Composant",
                &["Mobilier", "Équipement", "Élément générique"],
            ),
            Nombre("Rotation", 0.0, " °"),
        ],
        Deplacer => vec![
            Nombre("Distance", 1000.0, " mm"),
            Choix("Contrainte", &["Libre", "Horizontale", "Verticale"]),
        ],
        Copier => vec![Nombre("Copies", 1.0, ""), Nombre("Distance", 1000.0, " mm")],
        Tourner => vec![Nombre("Angle", 90.0, " °")],
        Aligner => vec![Choix("Référence", &["Face", "Axe"])],
        Ajuster => vec![Choix("Mode", &["Raccorder", "Prolonger"])],
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
        Ligne | Arc | Polyligne | Spline | Cercle => vec![],
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
        egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (vignette, _) = ui.allocate_exact_size(egui::vec2(52.0, 52.0), Sense::hover());
                ui.painter().rect_filled(vignette, 3.0, ui.visuals().faint_bg_color);
                let elevation_affichee = self.coupe_identique.as_ref().map_or(courant.elevation, |c| c.2);
                let nom_affiche = self.coupe_identique.as_ref().map_or(courant.nom.as_str(), |c| c.0.as_str());
                icone_vue(ui.painter(), egui::Rect::from_center_size(vignette.center(), egui::vec2(34.0, 34.0)), elevation_affichee, ui.visuals().text_color());
                ui.vertical(|ui| {
                    ui.label(RichText::new("Coupe ou élévation").small().weak());
                    egui::ComboBox::from_id_salt("type_coupe")
                        .width(ui.available_width())
                        .height(400.0)
                        .truncate()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .selected_text(RichText::new(format!("{} · {}", sorte(elevation_affichee), nom_affiche)).strong())
                        .show_ui(ui, |ui| {
                            ui.set_min_width(280.0);
                            ui.horizontal(|ui| {
                                for e in [false, true] {
                                    let (r, rep) = ui.allocate_exact_size(egui::vec2(120.0, 46.0), Sense::click());
                                    let actif = courant.elevation == e;
                                    if actif || rep.hovered() {
                                        ui.painter().rect_filled(r, 4.0, if actif { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                    }
                                    icone_vue(ui.painter(), egui::Rect::from_center_size(r.center_top() + egui::vec2(0.0, 15.0), egui::vec2(24.0, 24.0)), e, ui.visuals().text_color());
                                    ui.painter().text(r.center_bottom() - egui::vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, sorte(e), egui::FontId::proportional(12.0), ui.visuals().text_color());
                                    if rep.clicked() && !actif {
                                        choix = types.iter().position(|t| t.elevation == e);
                                        ui.ctx().request_repaint();
                                    }
                                }
                            });
                            ui.separator();
                            for (i, t) in types.iter().enumerate().filter(|(_, t)| t.elevation == courant.elevation) {
                                let (r, rep) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), Sense::click());
                                if i == k || rep.hovered() {
                                    ui.painter().rect_filled(r, 3.0, if i == k { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                }
                                ui.painter().text(r.min + egui::vec2(8.0, 11.0), egui::Align2::LEFT_CENTER, &t.nom, egui::FontId::proportional(12.0), ui.visuals().text_color());
                                ui.painter().text(r.min + egui::vec2(8.0, 26.0), egui::Align2::LEFT_CENTER, &t.aide, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                                ui.painter().text(r.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, format!("1:{} · {} m", t.echelle, crate::step::format_real(t.profondeur / 1000.0)), egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                                if rep.clicked() {
                                    choix = Some(i);
                                    ui.close();
                                }
                            }
                        });
                    ui.label(RichText::new(&courant.aide).small().weak());
                });
            });
        });
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
            type_dalle: self.surface_identique.as_ref().filter(|(o,_)| *o == outil).map(|(_,t)| t.clone()).unwrap_or_else(|| crate::modele::TypeCompose { id: String::new(), nom, couches, couches_detaillees: true, origine: None }),
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

    pub(crate) fn activer_outil(&mut self, outil: Outil) { if outil == Outil::Ouverture { self.ouverture_type_identique = None; } self.selection = Some(outil); }
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
            Some(Valeur::Texte(v)) if v == "Face extérieure" => Implantation::Exterieure,
            Some(Valeur::Texte(v)) if v == "Face intérieure" => Implantation::Interieure,
            Some(Valeur::Texte(v)) if v == "Axe du porteur" => Implantation::AxePorteur,
            _ => Implantation::Axe,
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
        }
    }

    pub fn afficher(
        &mut self,
        ui: &mut egui::Ui,
        menu: Menu,
        projet: Option<&crate::modele::Projet>,
    ) -> Option<Outil> {
        let mut action = None;
        let largeur = ui.available_width();
        let outil_options = self.selection.filter(|o| {
            groupes(menu)
                .iter()
                .any(|(_, outils)| outils.contains(&o.commande()))
        });
        // Les surfaces se règlent dans l'encart en tête de Propriétés.
        // Sans réglage (dessin 2D, actions), pas d'encart.
        let outil_options = outil_options.filter(|o| o.commande() != Outil::Surfaces && !champs(*o).is_empty());
        let selection_options = outil_options.map(Outil::commande);
        let compact = largeur_commandes(ui, menu, &[]);
        let options_inline =
            selection_options.is_some() && compact + LARGEUR_OPTIONS + ESPACE_COLONNES <= largeur;
        let largeur_options = if selection_options.is_none() {
            0.0
        } else if options_inline {
            LARGEUR_OPTIONS + ESPACE_COLONNES
        } else {
            LARGEUR_REGLAGES + ESPACE_COLONNES
        };
        let budget = largeur - largeur_options;
        let mut grands = Vec::new();
        // Les références et les commandes occasionnelles restent toujours empilées.
        // En réduisant la fenêtre, les outils les moins fréquents se replient d'abord.
        let mut candidats: Vec<_> = groupes(menu)
            .iter()
            .flat_map(|(_, outils)| outils.iter().copied())
            .filter(|o| o.priorite() > 0 && largeur >= 1500.0 - 200.0 * o.priorite() as f32)
            .collect();
        candidats.sort_by_key(|o| std::cmp::Reverse(o.priorite()));
        for outil in candidats {
            let mut essai = grands.clone();
            essai.push(outil);
            if largeur_commandes(ui, menu, &essai) <= budget {
                grands = essai;
            }
        }
        // Défilement de secours pour les très petites surfaces / grandes polices.
        // À la taille minimale de l'application, toutes les commandes restent visibles.
        egui::ScrollArea::horizontal()
            .id_salt("ruban_commandes")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = ESPACE_GROUPES;
                ui.horizontal_top(|ui| {
                    for (indice, (nom, outils)) in groupes(menu).iter().enumerate() {
                        let colonnes = colonnes(ui, outils, &grands);
                        let contient_selection =
                            selection_options.is_some_and(|o| outils.contains(&o));
                        let largeur_groupe = largeur_groupe(ui, nom, &colonnes)
                            + if contient_selection {
                                largeur_options
                            } else {
                                0.0
                            };
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(largeur_groupe, HAUTEUR_GROUPE),
                            Sense::hover(),
                        );
                        if indice > 0 {
                            let x = rect.left() - ESPACE_GROUPES / 2.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(x, rect.top() + 4.0),
                                    egui::pos2(x, rect.bottom() - 4.0),
                                ],
                                ui.visuals().widgets.noninteractive.bg_stroke,
                            );
                        }
                        let mut x = rect.left();
                        for colonne in colonnes {
                            for (ligne, &outil) in colonne.outils.iter().enumerate() {
                                let hauteur = if colonne.grande {
                                    HAUTEUR_OUTILS
                                } else {
                                    HAUTEUR_OUTILS / 3.0
                                };
                                let bouton = egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top() + ligne as f32 * hauteur),
                                    egui::vec2(colonne.largeur, hauteur),
                                );
                                let actif = self.selection.map(Outil::commande) == Some(outil);
                                // éléments horizontaux : un bouton ; la sorte se choisit dans le
                                // sélecteur de type de Propriétés
                                let disponible = matches!(outil, Outil::Mur|Outil::Surfaces|Outil::Poteau|Outil::Poutre|Outil::Niveau|Outil::Quadrillage|Outil::Ouverture|Outil::Deplacer|Outil::Copier|Outil::Tourner|Outil::Aligner|Outil::Ajuster|Outil::CreerIdentique|Outil::Supprimer|Outil::Cadrer|Outil::Isoler|Outil::Masquer|Outil::VuePlan|Outil::VueCoupe|Outil::Texte|Outil::Cote|Outil::Ligne|Outil::Polyligne|Outil::Spline|Outil::Cercle|Outil::Arc|Outil::Zone|Outil::Etiquette|Outil::Feuilles|Outil::Nomenclatures);
                                let mut bouton_ui=ui.new_child(egui::UiBuilder::new().max_rect(bouton));
                                if !disponible {bouton_ui.disable();}
                                let reponse=bouton_outil(&mut bouton_ui,bouton,outil,colonne.grande,actif);
                                if reponse.on_disabled_hover_text("Outil prévu, pas encore disponible").clicked() {
                                    if matches!(outil, Outil::CreerIdentique | Outil::Supprimer | Outil::Cadrer | Outil::Isoler | Outil::Masquer | Outil::Feuilles | Outil::Nomenclatures) {
                                        action = Some(outil);
                                        continue;
                                    }
                                    if outil == Outil::VueCoupe { self.coupe_identique = None; }
                                    if outil == Outil::Surfaces { self.surface_identique = None; }
                                    if outil == Outil::Ouverture { self.ouverture_type_identique = None; }
                                    self.selection = Some(if outil == Outil::Surfaces {
                                        let surface = self.derniere_surface.unwrap_or(Outil::Dalle);
                                        self.derniere_surface = Some(surface);
                                        surface
                                    } else {
                                        outil
                                    });
                                }
                            }
                            x += colonne.largeur + ESPACE_COLONNES;
                            // Réglages directement à droite de la colonne de l'outil actif.
                            if colonne.outils.iter().any(|o| Some(*o) == selection_options) {
                                let outil = outil_options.unwrap();
                                let reglages = egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top()),
                                    egui::vec2(
                                        if options_inline {
                                            LARGEUR_OPTIONS
                                        } else {
                                            LARGEUR_REGLAGES
                                        },
                                        HAUTEUR_OUTILS,
                                    ),
                                );
                                let mut options_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .id_salt(("reglages", outil.commande()))
                                        .max_rect(reglages)
                                        .layout(egui::Layout::top_down(Align::Min)),
                                );
                                {
                                    let ui = &mut options_ui;
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if options_inline {
                                        self.options_outil(ui, outil, projet);
                                    } else {
                                        let r = ui
                                            .add_sized(
                                                [LARGEUR_REGLAGES, HAUTEUR_OUTILS],
                                                egui::Button::new("..."),
                                            )
                                            .on_hover_text(format!("Réglages — {}", outil.nom()));
                                        egui::Popup::menu(&r)
                                            .close_behavior(
                                                egui::PopupCloseBehavior::CloseOnClickOutside,
                                            )
                                            .show(|ui| {
                                                ui.set_width(LARGEUR_OPTIONS);
                                                ui.label(
                                                    RichText::new(format!(
                                                        "Réglages — {}",
                                                        outil.nom()
                                                    ))
                                                    .strong(),
                                                );
                                                ui.separator();
                                                self.options_outil(ui, outil, projet);
                                            });
                                    }
                                }
                                x += largeur_options;
                            }
                        }
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - 8.0),
                            egui::Align2::CENTER_CENTER,
                            *nom,
                            egui::FontId::proportional(11.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                });
            });
        action
    }

    fn proposer_ajout_creation(
        &mut self,
        ui: &mut egui::Ui,
        liste: crate::modele::edition::ListeCreation,
    ) {
        ui.separator();
        if ui
            .button(format!("+ Ajouter : {}…", liste.libelle()))
            .clicked()
        {
            self.ajout_creation = Some(liste);
            self.nom_creation.clear();
            self.jeu_creation.clear();
            self.erreur_creation = None;
            ui.close();
        }
    }

    pub fn reinitialiser_contexte_creation(&mut self) {
        self.phase_creation = None;
        self.sous_projet_creation = None;
        self.variante_creation = None;
        self.ajout_creation = None;
        self.erreur_creation = None;
    }

    /// Contexte commun aux créations, affiché indépendamment de l'outil actif.
    /// Seuls les ajouts modifient le document ; choisir une valeur règle les prochaines créations.
    pub fn contexte_creation(
        &mut self,
        ui: &mut egui::Ui,
        projet: &mut crate::modele::Projet,
    ) -> bool {
        use crate::modele::edition::ListeCreation;
        let phases = if projet.phases.is_empty() {
            vec!["Projet".into()]
        } else {
            projet.phases.clone()
        };
        let sous_projets = if projet.sous_projets.is_empty() {
            vec!["A".into()]
        } else {
            projet.sous_projets.clone()
        };
        if self
            .phase_creation
            .as_ref()
            .is_none_or(|v| !phases.contains(v))
        {
            self.phase_creation = phases.last().cloned();
        }
        if self
            .sous_projet_creation
            .as_ref()
            .is_none_or(|v| !sous_projets.contains(v))
        {
            self.sous_projet_creation = sous_projets.first().cloned();
        }
        if self
            .variante_creation
            .as_ref()
            .is_some_and(|id| !projet.variantes.iter().any(|v| &v.id == id))
        {
            self.variante_creation = None;
        }
        let largeur = ((ui.available_width() - 8.0) / 3.0).max(30.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Phase").small()).truncate());
                egui::ComboBox::from_id_salt("phase_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(self.phase_creation.as_deref().unwrap_or("Projet"))
                    .show_ui(ui, |ui| {
                        for nom in &phases {
                            ui.selectable_value(&mut self.phase_creation, Some(nom.clone()), nom);
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::Phase);
                    })
                    .response
                    .on_hover_text(format!(
                        "Phase des prochaines créations : {}",
                        self.phase_creation.as_deref().unwrap_or("Projet")
                    ));
            });
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Sous-projet").small()).truncate());
                egui::ComboBox::from_id_salt("sous_projet_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(
                        self.sous_projet_creation
                            .as_deref()
                            .unwrap_or("A"),
                    )
                    .show_ui(ui, |ui| {
                        for nom in &sous_projets {
                            ui.selectable_value(
                                &mut self.sous_projet_creation,
                                Some(nom.clone()),
                                nom,
                            );
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::SousProjet);
                    })
                    .response
                    .on_hover_text(format!(
                        "Sous-projet des prochaines créations : {}",
                        self.sous_projet_creation
                            .as_deref()
                            .unwrap_or("A")
                    ));
            });
            let nom = self
                .variante_creation
                .as_ref()
                .and_then(|id| projet.variantes.iter().find(|v| &v.id == id))
                .map_or_else(|| "Principale".into(), |v| v.libelle());
            ui.vertical(|ui| {
                ui.set_width(largeur);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(RichText::new("Variante").small()).truncate());
                egui::ComboBox::from_id_salt("variante_creation")
                    .width(largeur)
                    .truncate()
                    .selected_text(&nom)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.variante_creation, None, "Principale");
                        for v in &projet.variantes {
                            ui.selectable_value(
                                &mut self.variante_creation,
                                Some(v.id.clone()),
                                v.libelle(),
                            );
                        }
                        self.proposer_ajout_creation(ui, ListeCreation::Variante);
                    })
                    .response
                    .on_hover_text(format!("Variante des prochaines créations : {nom}"));
            });
        });
        let mut modifie = false;
        if let Some(liste) = self.ajout_creation {
            let mut ouvert = true;
            let mut valider = false;
            let mut annuler = false;
            egui::Window::new(format!("Ajouter : {}", liste.libelle()))
                .id(egui::Id::unique("ajout_contexte_creation"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .default_width(280.0)
                .open(&mut ouvert)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    egui::Grid::new("grille_ajout_creation").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Nom");
                        ui.add(egui::TextEdit::singleline(&mut self.nom_creation).desired_width(180.0).hint_text(liste.libelle()));
                        ui.end_row();
                        if liste == ListeCreation::Variante {
                            ui.label("Jeu");
                            ui.add(egui::TextEdit::singleline(&mut self.jeu_creation).desired_width(180.0).hint_text("Jeu de variantes"));
                            ui.end_row();
                        }
                    });
                    if let Some(erreur) = &self.erreur_creation {
                        ui.colored_label(ui.visuals().error_fg_color, erreur);
                    }
                    ui.add_space(4.0);
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(Align::Center), |ui| {
                        annuler = ui.button("Annuler").clicked();
                        valider = ui.add_enabled(!self.nom_creation.trim().is_empty(), egui::Button::new(RichText::new("Ajouter").strong())).clicked();
                    });
                    // touches consommées : Échap ne doit pas aussi quitter l'outil en cours
                    let (entree, echap) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)));
                    valider |= entree && !self.nom_creation.trim().is_empty();
                    annuler |= echap;
                });
            if valider {
                match projet.ajouter_choix_creation(liste, &self.nom_creation, &self.jeu_creation) {
                    Ok(valeur) => {
                        match liste {
                            ListeCreation::Phase => self.phase_creation = Some(valeur),
                            ListeCreation::SousProjet => self.sous_projet_creation = Some(valeur),
                            ListeCreation::Variante => self.variante_creation = Some(valeur),
                        }
                        modifie = true;
                        self.ajout_creation = None;
                    }
                    Err(e) => self.erreur_creation = Some(e),
                }
            }
            if !ouvert || annuler {
                self.ajout_creation = None;
            }
        }
        modifie
    }

    /// Palette de création du mur : croquis des deux limites (haut, bas) avec leur
    /// réglage sur la même ligne, puis les propriétés du type en grille.
    pub fn proprietes_mur(
        &mut self,
        ui: &mut egui::Ui,
        projet: Option<&crate::modele::Projet>,
    ) -> bool {
        if !self.mur_actif() {
            return false;
        }
        ui.label(RichText::new("Créer un mur").strong());
        let Some(projet) = projet else {
            ui.label("Créez ou ouvrez un projet natif pour dessiner un mur.");
            return true;
        };
        // Un changement de document ne doit jamais réutiliser une référence étrangère.
        for niveau in [&mut self.niveau_mur, &mut self.niveau_haut_mur] {
            if niveau
                .as_ref()
                .is_some_and(|id| projet.niveau(id).is_none())
            {
                *niveau = None;
            }
        }
        if self.type_mur.as_ref().is_some_and(|id| projet.type_mur(id).is_none()) {
            self.type_mur = None;
        }
        egui::ScrollArea::vertical()
            .id_salt("creation_mur")
            .show(ui, |ui| {
                self.selecteur_type_mur(ui, projet);
                ui.add_space(2.0);
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        self.croquis_mur_creation(ui, projet);
                    });
                if let Err(e) = self.parametres_mur().bornes(projet) {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.add_space(4.0);
                egui::Grid::new("grille_creation_mur")
                    .num_columns(2)
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        ui.label("Épaisseur");
                        match self.type_mur.as_ref().and_then(|id| projet.type_mur(id)) {
                            Some(t) => {
                                ui.label(crate::options::unite().afficher(t.epaisseur()))
                                    .on_hover_text("Épaisseur du type (somme des couches)");
                            }
                            None => self.nombre_mur(ui, 2, 300.0),
                        }
                        ui.end_row();
                        ui.label("Implantation");
                        let v = self
                            .valeurs
                            .entry((Outil::Mur, 0))
                            .or_insert_with(|| Valeur::Texte("Axe".into()));
                        if let Valeur::Texte(v) = v {
                            egui::ComboBox::from_id_salt("implantation_proprietes_mur")
                                .width(130.0)
                                .selected_text(v.as_str())
                                .show_ui(ui, |ui| {
                                    for nom in ["Axe", "Face extérieure", "Face intérieure", "Axe du porteur"] {
                                        ui.selectable_value(v, nom.into(), nom);
                                    }
                                });
                        }
                        ui.end_row();
                        ui.label("Chaîner");
                        ui.checkbox(&mut self.chainer_murs, "")
                            .on_hover_text("Le mur suivant part de la fin du précédent");
                        ui.end_row();
                    });
            });
        true
    }

    /// Sélecteur de type en tête de la palette, comme dans Revit : générique (épaisseur
    /// saisie) ou un type du projet ; composition en bande sous la liste ; Modifier /
    /// Nouveau ouvrent l'éditeur de type.
    fn selecteur_type_mur(&mut self, ui: &mut egui::Ui, projet: &crate::modele::Projet) {
        let nom = self.type_mur.as_ref().and_then(|id| projet.type_mur(id)).map_or("Générique (épaisseur libre)", |t| t.nom.as_str());
        // largeur du panneau mesurée hors de la ligne, qui grandirait avec la liste
        let largeur = ui.available_width();
        let mut types: Vec<_> = projet.types_murs.iter().collect();
        // composés d'abord, puis par nom
        types.sort_by(|a, b| (a.couches.len() < 2, &a.nom).cmp(&(b.couches.len() < 2, &b.nom)));
        // boutons à droite d'abord, la liste prend la place restante (sans élargir le panneau)
        ui.allocate_ui_with_layout(egui::vec2(largeur, 22.0), egui::Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("+").on_hover_text("Nouveau type de mur composé").clicked() {
                self.ouvrir_editeur_type(projet, None);
            }
            if ui.small_button("Modifier").on_hover_text("Modifier les couches du type (les murs déjà tracés gardent les leurs)").clicked() {
                self.ouvrir_editeur_type(projet, self.type_mur.clone());
            }
            egui::ComboBox::from_id_salt("type_mur_creation")
                .width(ui.available_width().max(60.0) - 8.0)
                .truncate()
                .selected_text(RichText::new(nom).strong())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.type_mur, None, "Générique (épaisseur libre)");
                    for t in types {
                        let texte = if t.couches.len() > 1 { format!("{} — {} couches, {}", t.nom, t.couches.len(), crate::options::unite().afficher(t.epaisseur())) } else { t.nom.clone() };
                        ui.selectable_value(&mut self.type_mur, Some(t.id.clone()), texte);
                    }
                    // types du gabarit absents du projet : copiés dans le projet au choix
                    let gabarit = crate::gabarit::actif();
                    let absents: Vec<_> = gabarit.types_murs.iter().filter(|t| !projet.types_murs.iter().any(|p| p.nom == t.nom)).collect();
                    if !absents.is_empty() {
                        ui.separator();
                        ui.label(RichText::new(format!("Gabarit · {}", gabarit.nom)).small().weak());
                        for t in absents {
                            let e: f64 = t.couches.iter().map(|c| c.epaisseur).sum();
                            if ui.selectable_label(false, format!("{} — {}", t.nom, crate::options::unite().afficher(e))).clicked() {
                                self.type_mur_gabarit = Some(t.clone());
                            }
                        }
                    }
                })
                .response
                .on_hover_text("Type des prochains murs. Un type à plusieurs couches crée un mur par couche, regroupés en assemblage.");
        });
        if let Some(t) = self.type_mur.as_ref().and_then(|id| projet.type_mur(id)) {
            ui.scope(|ui| {
                ui.set_max_width(largeur);
                bande_couches(ui, projet, &t.couches, 12.0);
            });
        }
    }

    /// Type de mur du gabarit choisi dans le sélecteur, à ajouter au projet.
    pub fn prendre_type_gabarit(&mut self) -> Option<crate::gabarit::TypeGabarit> {
        self.type_mur_gabarit.take()
    }

    pub fn editeur_type_ouvert(&self) -> bool {
        self.editeur_type.is_some()
    }

    fn ouvrir_editeur_type(&mut self, projet: &crate::modele::Projet, id: Option<String>) {
        use crate::modele::{Couche, FonctionCouche};
        let existant = id.as_ref().and_then(|id| projet.type_mur(id));
        let (nom, couches) = match existant {
            Some(t) => (t.nom.clone(), t.couches.clone()),
            None => {
                // générique en cours : sa seule couche ; sinon un mur isolé type
                let mut n = projet.types_murs.len() + 1;
                while projet.types_murs.iter().any(|t| t.nom == format!("Mur composé {n}")) {
                    n += 1;
                }
                let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None };
                (format!("Mur composé {n}"), vec![c("Enduit", 15.0, FonctionCouche::Finition), c("Béton", 200.0, FonctionCouche::Structure), c("Laine minérale", 120.0, FonctionCouche::Isolation), c("Plâtre", 13.0, FonctionCouche::Finition)])
            }
        };
        let mut materiaux = projet.materiaux.clone();
        for c in &couches {
            if let Some(m) = &c.materiau
                && !materiaux.iter().any(|x| &x.nom == m)
            {
                materiaux.push(crate::modele::assemblage::materiau_par_defaut(m));
            }
        }
        self.editeur_type = Some(EditeurType { id: existant.map(|t| t.id.clone()), nom, couches, materiaux, erreur: None, glisse: None });
    }

    /// Fenêtre d'édition d'un type de mur composé : une ligne par couche, de
    /// l'extérieur (en haut) vers l'intérieur. Retourne vrai si le projet a changé.
    pub fn editeur_type_mur(&mut self, ctx: &egui::Context, projet: &mut crate::modele::Projet) -> bool {
        use crate::modele::{Couche, FonctionCouche, MotifCoupe};
        let Some(ed) = &mut self.editeur_type else { return false };
        let (mut ouvert, mut enregistrer, mut annuler) = (true, false, false);
        egui::Window::new(if ed.id.is_some() { "Modifier le type de mur" } else { "Nouveau type de mur" })
            .id(egui::Id::unique("editeur_type_mur"))
            .open(&mut ouvert)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Nom");
                    ui.add(egui::TextEdit::singleline(&mut ed.nom).desired_width(300.0));
                });
                ui.add_space(4.0);
                bande_couches_brouillon(ui, &ed.materiaux, &ed.couches, 18.0);
                ui.label(RichText::new("Extérieur").small().weak());
                let mut action: Option<(usize, i32)> = None; // (couche, 0 supprimer)
                // glisser-déposer : lignes (hauteur) et relâchement de la poignée
                let mut lignes: Vec<egui::Rangef> = Vec::new();
                let mut lacher = false;
                egui::Grid::new("couches_type_mur").num_columns(8).spacing([6.0, 4.0]).striped(true).show(ui, |ui| {
                    for t in ["", "Fonction", "Matériau", "", "Motif", "Épaisseur", "Plume", ""] {
                        ui.label(RichText::new(t).small().weak());
                    }
                    ui.end_row();
                    let n = ed.couches.len();
                    for (k, c) in ed.couches.iter_mut().enumerate() {
                        ui.push_id(k, |ui| {
                            // poignée : glisser la ligne vers une autre position
                            let (r, poignee) = ui.allocate_exact_size(egui::vec2(18.0, 20.0), Sense::drag());
                            let v = ui.visuals();
                            let encre = if poignee.hovered() || ed.glisse == Some(k) { v.strong_text_color() } else { v.weak_text_color() };
                            for dy in [-4.0, 0.0, 4.0] {
                                ui.painter().line_segment([r.center() + egui::vec2(-5.0, dy), r.center() + egui::vec2(5.0, dy)], egui::Stroke::new(1.3, encre));
                            }
                            if poignee.drag_started() {
                                ed.glisse = Some(k);
                            }
                            if poignee.drag_stopped() {
                                lacher = true;
                            }
                            lignes.push(r.y_range());
                            poignee.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text("Glisser pour déplacer la couche");
                        });
                        egui::ComboBox::from_id_salt(("fonction", k)).width(84.0).selected_text(c.fonction.libelle()).show_ui(ui, |ui| {
                            for f in FonctionCouche::TOUTES {
                                ui.selectable_value(&mut c.fonction, f, f.libelle());
                            }
                        });
                        // matériau : nom libre, ou choisi dans ceux du projet
                        let mut nom = c.materiau.clone().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.add(egui::TextEdit::singleline(&mut nom).desired_width(130.0).hint_text("Matériau"));
                            let liste = petit_bouton(ui, !ed.materiaux.is_empty(), Glyphe::Liste).on_hover_text("Matériaux du projet");
                            egui::Popup::menu(&liste).show(|ui| {
                                for m in &ed.materiaux {
                                    if ui.button(&m.nom).clicked() {
                                        nom = m.nom.clone();
                                        ui.close();
                                    }
                                }
                            });
                        });
                        let nom = nom.trim().to_string();
                        c.materiau = (!nom.is_empty()).then_some(nom.clone());
                        if !nom.is_empty() && !ed.materiaux.iter().any(|m| m.nom == nom) {
                            ed.materiaux.push(crate::modele::assemblage::materiau_par_defaut(&nom));
                        }
                        match ed.materiaux.iter_mut().find(|m| m.nom == nom) {
                            Some(m) => {
                                ui.color_edit_button_srgb(&mut m.couleur).on_hover_text("Couleur du matériau (tous ses usages)");
                                egui::ComboBox::from_id_salt(("motif", k)).width(80.0).selected_text(m.motif.libelle()).show_ui(ui, |ui| {
                                    for x in MotifCoupe::TOUS {
                                        let l = x.libelle().to_string();
                                        ui.selectable_value(&mut m.motif, x, l);
                                    }
                                });
                            }
                            None => {
                                ui.label("");
                                ui.label("");
                            }
                        }
                        ui.add(egui::DragValue::new(&mut c.epaisseur).speed(1.0).range(1.0..=2000.0).max_decimals(1).dimensions());
                        // plume du contour de coupe : numéro et épaisseur de la table
                        let plumes = projet.epaisseurs_plumes();
                        let mut n_plume = c.plume();
                        egui::ComboBox::from_id_salt(("plume", k)).width(70.0).selected_text(format!("{n_plume} · {}", crate::step::format_real(plumes[(n_plume - 1) as usize]))).show_ui(ui, |ui| {
                            for (i, e) in plumes.iter().enumerate() {
                                ui.selectable_value(&mut n_plume, i as u8 + 1, format!("{} · {} mm", i + 1, crate::step::format_real(*e)));
                            }
                        }).response.on_hover_text("Plume du contour de coupe de la couche");
                        if n_plume != c.plume() {
                            c.plume = Some(n_plume);
                        }
                        if petit_bouton(ui, n > 1, Glyphe::Croix).on_hover_text("Supprimer la couche").clicked() { action = Some((k, 0)); }
                        ui.end_row();
                    }
                });
                ui.label(RichText::new("Intérieur").small().weak());
                // position d'insertion sous le pointeur : nombre de lignes dont le milieu est au-dessus
                if let (Some(k), Some(y)) = (ed.glisse, ui.input(|i| i.pointer.interact_pos().map(|p| p.y))) {
                    let cible = lignes.iter().filter(|l| l.center() < y).count();
                    if let (Some(premiere), Some(derniere)) = (lignes.first(), lignes.last()) {
                        let yl = if cible == 0 { premiere.min - 2.0 } else { lignes[cible - 1].max + 2.0 }.clamp(premiere.min - 2.0, derniere.max + 2.0);
                        let x = ui.min_rect().x_range();
                        ui.painter().line_segment([egui::pos2(x.min, yl), egui::pos2(x.max, yl)], egui::Stroke::new(2.0, ui.visuals().selection.stroke.color));
                    }
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    if lacher {
                        let couche = ed.couches.remove(k);
                        let cible = if cible > k { cible - 1 } else { cible };
                        ed.couches.insert(cible.min(ed.couches.len()), couche);
                        ed.glisse = None;
                    }
                }
                if lacher {
                    ed.glisse = None;
                }
                if let Some((k, 0)) = action {
                    ed.couches.remove(k);
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Couche").clicked() {
                        ed.couches.push(Couche { materiau: None, epaisseur: 50.0, fonction: FonctionCouche::Autre, plume: None });
                    }
                    let total: f64 = ed.couches.iter().map(|c| c.epaisseur).sum();
                    ui.label(format!("Épaisseur totale : {}", crate::options::unite().afficher(total)));
                });
                if let Some(e) = &ed.erreur {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.label(RichText::new("Chaque couche sera tracée comme un mur indépendant, regroupé avec les autres en assemblage. Les murs déjà tracés gardent leurs couches.").small().weak());
                ui.separator();
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), egui::Layout::right_to_left(Align::Center), |ui| {
                    annuler = ui.button("Annuler").clicked();
                    enregistrer = ui.button(RichText::new("Enregistrer").strong()).clicked();
                });
            });
        if !ouvert || annuler {
            self.editeur_type = None;
            return false;
        }
        if !enregistrer {
            return false;
        }
        let ed = self.editeur_type.as_mut().unwrap();
        let nom = ed.nom.trim().to_string();
        if nom.is_empty() {
            ed.erreur = Some("Donnez un nom au type.".into());
            return false;
        }
        if ed.couches.is_empty() || ed.couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            ed.erreur = Some("Chaque couche doit avoir une épaisseur positive.".into());
            return false;
        }
        if projet.types_murs.iter().any(|t| t.nom == nom && Some(&t.id) != ed.id.as_ref()) {
            ed.erreur = Some("Un autre type porte déjà ce nom.".into());
            return false;
        }
        let id = match &ed.id {
            Some(id) => {
                let t = projet.types_murs.iter_mut().find(|t| &t.id == id).unwrap();
                t.nom = nom;
                t.couches = ed.couches.clone();
                t.couches_detaillees = true;
                id.clone()
            }
            None => {
                let mut k = projet.types_murs.len() + 1;
                let id = loop {
                    let id = crate::modele::identifiant("type-mur", &format!("compose-{k}"));
                    if projet.types_murs.iter().all(|t| t.id != id) { break id; }
                    k += 1;
                };
                projet.types_murs.push(crate::modele::TypeCompose { id: id.clone(), nom, couches: ed.couches.clone(), couches_detaillees: true, origine: None });
                id
            }
        };
        // matériaux utilisés : ajoutés ou mis à jour (couleur, motif)
        for m in &ed.materiaux {
            if !ed.couches.iter().any(|c| c.materiau.as_ref() == Some(&m.nom)) && projet.materiau(&m.nom).is_none() {
                continue;
            }
            match projet.materiaux.iter_mut().find(|x| x.nom == m.nom) {
                Some(x) => *x = m.clone(),
                None => projet.materiaux.push(m.clone()),
            }
        }
        self.type_mur = Some(id);
        self.editeur_type = None;
        true
    }

    /// Crée (ou remplace) un type composé et le choisit pour les prochains murs (pilote).
    pub fn definir_type_mur(&mut self, projet: &mut crate::modele::Projet, nom: &str, couches: Vec<crate::modele::Couche>) -> Result<String, String> {
        if couches.is_empty() || couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            return Err("Chaque couche doit avoir une épaisseur positive.".into());
        }
        let id = match projet.types_murs.iter_mut().find(|t| t.nom == nom) {
            Some(t) => {
                t.couches = couches;
                t.id.clone()
            }
            None => {
                let id = crate::modele::identifiant("type-mur", &format!("compose:{nom}"));
                projet.types_murs.push(crate::modele::TypeCompose { id: id.clone(), nom: nom.into(), couches, couches_detaillees: true, origine: None });
                id
            }
        };
        projet.completer_materiaux();
        self.type_mur = Some(id.clone());
        Ok(id)
    }

    /// Croquis de saisie : mur en perspective à gauche, ses deux limites à droite, sur
    /// une ligne chacune (niveau, puis décalage, ou hauteur si le haut est libre),
    /// rappelées par un pointillé depuis l'arête correspondante du mur.
    fn croquis_mur_creation(&mut self, ui: &mut egui::Ui, projet: &crate::modele::Projet) {
        use egui::{Align2, FontId, Stroke, pos2, vec2};
        let bleu = Color32::from_rgb(18, 158, 205);
        let largeur_croquis = 64.0;
        let origine = ui.cursor().min;
        let mut centres = [0.0f32; 2];
        let mut gauche_champs = 0.0f32;
        // Réserver de la place autour du croquis pour les décalages des limites.
        ui.add_space(14.0);
        ui.horizontal_top(|ui| {
            ui.add_space(largeur_croquis);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                for haut in [true, false] {
                    let ligne = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_sized(
                            [30.0, 20.0],
                            egui::Label::new(RichText::new(if haut { "Haut" } else { "Bas" }).color(bleu)),
                        );
                        let niveau = if haut {
                            &mut self.niveau_haut_mur
                        } else {
                            &mut self.niveau_mur
                        };
                        let nom = niveau
                            .as_ref()
                            .and_then(|id| projet.niveau(id))
                            .or_else(|| if haut { None } else { projet.niveaux.first() })
                            .map_or(if haut { "Libre" } else { "Niveau 0" }, |n| n.nom.as_str());
                        let champ = 72.0;
                        egui::ComboBox::from_id_salt(("limite_creation_mur", haut))
                            .width((ui.available_width() - champ - 4.0).max(60.0))
                            .truncate()
                            .selected_text(nom)
                            .show_ui(ui, |ui| {
                                if haut || projet.niveaux.is_empty() {
                                    ui.selectable_value(
                                        niveau,
                                        None,
                                        if haut { "Libre (hauteur)" } else { "Niveau 0" },
                                    );
                                }
                                for n in &projet.niveaux {
                                    ui.selectable_value(niveau, Some(n.id.clone()), &n.nom);
                                }
                            })
                            .response
                            .on_hover_text(if haut {
                                "Limite haute : un niveau, ou libre (hauteur saisie)"
                            } else {
                                "Limite basse : niveau de base du mur"
                            });
                        if haut && self.niveau_haut_mur.is_none() {
                            // haut libre : la valeur est la hauteur, sans décalage
                            self.decalage_haut_mur = 0.0;
                            let v = self
                                .valeurs
                                .entry((Outil::Mur, 1))
                                .or_insert(Valeur::Nombre(2800.0));
                            if let Valeur::Nombre(v) = v {
                                ui.add_sized(
                                    [champ, 20.0],
                                    egui::DragValue::new(v).speed(10.0).range(1.0..=f64::MAX).dimensions(),
                                )
                                .on_hover_text("Hauteur du mur");
                            }
                        } else {
                            let v = if haut {
                                &mut self.decalage_haut_mur
                            } else {
                                match self.valeurs.entry((Outil::Mur, 3)).or_insert(Valeur::Nombre(0.0)) {
                                    Valeur::Nombre(v) => v,
                                    Valeur::Texte(_) => unreachable!(),
                                }
                            };
                            ui.add_sized(
                                [champ, 20.0],
                                egui::DragValue::new(v)
                                    .speed(10.0)
                                    .dimensions(),
                            )
                            .on_hover_text("Décalage par rapport au niveau");
                        }
                    });
                    gauche_champs = ligne.response.rect.left();
                    centres[if haut { 0 } else { 1 }] = ligne.response.rect.center().y;
                }
            });
        });
        ui.add_space(14.0);
        let p = ui.painter();
        let bord = Stroke::new(1.0, bleu);
        let profondeur = vec2(4.5, -3.0);
        // mur vu de face (largeur), son chant à droite et son dessus
        let x1 = origine.x + largeur_croquis - 20.0;
        let x0 = origine.x + 16.0;
        let (yh, yb) = limites_croquis_mur(&self.parametres_mur(), projet, centres[0] + 3.0, centres[1] + 3.0);
        let (a, h, b, bb) = (pos2(x0, yh), pos2(x1, yh), pos2(x1, yb), pos2(x0, yb));
        p.add(egui::Shape::convex_polygon(vec![a, h, b, bb], Color32::from_rgb(246, 251, 253), bord));
        p.add(egui::Shape::convex_polygon(
            vec![h, h + profondeur, b + profondeur, b],
            Color32::from_rgb(222, 242, 249),
            bord,
        ));
        p.add(egui::Shape::convex_polygon(
            vec![a, a + profondeur, h + profondeur, h],
            Color32::from_rgb(236, 248, 252),
            bord,
        ));
        // rappels : de l'arête du mur à la ligne de sa limite
        let rappel = Stroke::new(0.8, bleu.gamma_multiply(0.8));
        for (depuis, y) in [(h + profondeur, centres[0]), (b + profondeur, centres[1])] {
            let fin = gauche_champs - 3.0;
            let mut x = depuis.x + 3.0;
            while x < fin {
                p.line_segment([pos2(x, y), pos2((x + 2.5).min(fin), y)], rappel);
                x += 5.0;
            }
            p.line_segment([depuis, pos2(depuis.x + 2.0, y)], rappel);
            p.circle_filled(depuis, 1.5, bleu);
        }
        // hauteur résultante, le long du mur
        if let Ok((bas_z, haut_z)) = self.parametres_mur().bornes(projet) {
            p.text(
                pos2((x0 + x1) * 0.5, (yh + yb) * 0.5),
                Align2::CENTER_CENTER,
                crate::options::unite().afficher(haut_z - bas_z),
                FontId::proportional(8.0),
                ui.visuals().weak_text_color(),
            );
        }
    }

    fn nombre_mur(&mut self, ui: &mut egui::Ui, cle: usize, defaut: f64) {
        let v = self
            .valeurs
            .entry((Outil::Mur, cle))
            .or_insert(Valeur::Nombre(defaut));
        if let Valeur::Nombre(v) = v {
            ui.add_sized([130.0, 20.0], egui::DragValue::new(v).speed(10.0).range(1.0..=f64::MAX).dimensions());
        }
    }

    /// Sélecteur de création placé en tête de la palette Propriétés, comme un
    /// sélecteur de type : vignette, choix explicite, puis paramètres de l'outil.
    /// Retourne false pour laisser la palette afficher la sélection du modèle.
    pub fn proprietes_surfaces(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) -> bool {
        let Some(mut surface) = self.selection.filter(|o| o.commande() == Outil::Surfaces) else {
            return false;
        };
        ui.add_space(6.0);
        egui::Frame::group(ui.style())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let (vignette, _) =
                        ui.allocate_exact_size(egui::vec2(52.0, 58.0), Sense::hover());
                    ui.painter()
                        .rect_filled(vignette, 3.0, ui.visuals().faint_bg_color);
                    icone(
                        ui.painter(),
                        egui::Rect::from_center_size(vignette.center(), egui::vec2(36.0, 36.0)),
                        surface,
                        ui.visuals().text_color(),
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Éléments horizontaux").small().weak());
                        let types = types_surface(surface);
                        let k = self.type_surface.get(&surface).copied().unwrap_or(0).min(types.len().saturating_sub(1));
                        let nom_type = self.surface_identique.as_ref().filter(|(o,_)| *o == surface).map(|(_, t)| t.nom.as_str()).unwrap_or_else(|| types.get(k).map_or("", |t| t.0.as_str()));
                        let mut choix_type: Option<(Outil, usize)> = None;
                        egui::ComboBox::from_id_salt("surface_proprietes")
                            .width(ui.available_width())
                            .height(400.0)
                            // les onglets de sorte gardent la liste ouverte ; un type la ferme
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .truncate()
                            .selected_text(RichText::new(format!("{} · {nom_type}", surface.nom())).strong())
                            .show_ui(ui, |ui| {
                                ui.set_min_width(300.0);
                                // hauteur fixe (sorte la plus fournie) : la liste ne saute pas d'un onglet à l'autre
                                let rangs = SURFACES.iter().map(|(o, _)| types_surface(*o).len()).max().unwrap_or(0);
                                ui.set_min_height(86.0 + rangs as f32 * 27.0);
                                // onglets des sortes : choisir une sorte garde la liste ouverte
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for &(choix, _) in SURFACES {
                                        let (r, reponse) = ui.allocate_exact_size(egui::vec2(74.0, 50.0), Sense::click());
                                        let actif = surface == choix;
                                        if actif || reponse.hovered() {
                                            ui.painter().rect_filled(r, 4.0, if actif { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                        }
                                        icone(ui.painter(), egui::Rect::from_center_size(r.center_top() + egui::vec2(0.0, 17.0), egui::vec2(26.0, 26.0)), choix, ui.visuals().text_color());
                                        ui.painter().text(r.center_bottom() - egui::vec2(0.0, 9.0), egui::Align2::CENTER_CENTER, choix.nom(), egui::FontId::proportional(12.0), ui.visuals().text_color());
                                        if reponse.clicked() && !actif {
                                            choix_type = Some((choix, self.type_surface.get(&choix).copied().unwrap_or(0)));
                                            // liste redimensionnée à l'image suivante
                                            ui.ctx().request_repaint();
                                        }
                                    }
                                });
                                let aide = SURFACES.iter().find(|(o, _)| *o == surface).map_or("", |(_, a)| *a);
                                ui.label(RichText::new(aide).small().weak());
                                ui.separator();
                                // types de la sorte : composition, nom, épaisseur
                                for (i, (nom, couches)) in types.iter().enumerate() {
                                    let (r, reponse) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), Sense::click());
                                    if i == k || reponse.hovered() {
                                        ui.painter().rect_filled(r, 3.0, if i == k { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.bg_fill });
                                    }
                                    let e: f64 = couches.iter().map(|c| c.epaisseur).sum();
                                    pile_couches(ui, egui::Rect::from_min_size(r.min + egui::vec2(8.0, 4.0), egui::vec2(26.0, 16.0)), couches);
                                    ui.painter().text(r.min + egui::vec2(42.0, 12.0), egui::Align2::LEFT_CENTER, nom, egui::FontId::proportional(12.0), ui.visuals().text_color());
                                    ui.painter().text(r.right_center() - egui::vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, crate::options::unite().afficher(e), egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                                    if reponse.clicked() {
                                        choix_type = Some((surface, i));
                                        ui.close();
                                    }
                                }
                            });
                        if let Some((sorte, i)) = choix_type {
                            surface = sorte;
                            self.type_surface.insert(sorte, i);
                            self.surface_identique = None;
                        }
                        let aide = SURFACES
                            .iter()
                            .find(|(o, _)| *o == surface)
                            .map_or("", |(_, aide)| *aide);
                        ui.add(egui::Label::new(RichText::new(aide).small().weak()).wrap());
                    });
                });
                // composition du type choisi, du haut vers le bas
                let types = types_surface(surface);
                if let Some((_, couches)) = types.get(self.type_surface.get(&surface).copied().unwrap_or(0)) {
                    ui.add_space(4.0);
                    for c in couches {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 12.0), Sense::hover());
                            pile_couches(ui, r, std::slice::from_ref(c));
                            ui.label(RichText::new(format!("{} · {}", c.materiau.as_deref().unwrap_or("Couche"), crate::options::unite().afficher(c.epaisseur))).small());
                            ui.label(RichText::new(c.fonction.libelle()).small().weak());
                        });
                    }
                }
            });
        self.selection = Some(surface);
        self.derniere_surface = Some(surface);
        ui.add_space(8.0);
        ui.label(RichText::new("Paramètres").strong());
        ui.separator();
        // tracé dans la vue (projet natif), niveau et mode de tracé
        {
            match projet {
                Some(projet) => {
                    if self.niveau_dalle.as_ref().is_some_and(|id| projet.niveau(id).is_none()) {
                        self.niveau_dalle = None;
                    }
                    egui::Grid::new("dalle_trace").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Niveau");
                        let nom = self.niveau_dalle.as_ref().and_then(|id| projet.niveau(id)).or(projet.niveaux.first()).map_or("Niveau 0", |n| n.nom.as_str());
                        egui::ComboBox::from_id_salt("niveau_dalle").width(130.0).selected_text(nom).show_ui(ui, |ui| {
                            for n in &projet.niveaux {
                                ui.selectable_value(&mut self.niveau_dalle, Some(n.id.clone()), &n.nom);
                            }
                        });
                        ui.end_row();
                        if surface == Outil::Toiture {
                            ui.label("Pente");
                            let mut pente = !self.toiture_plate;
                            ui.checkbox(&mut pente, "Toutes les arêtes la définissent")
                                .on_hover_text("Arêtes tracées en pente (croupes). Décocher pour une toiture plate ; chaque arête se règle ensuite dans les propriétés de la toiture.");
                            self.toiture_plate = !pente;
                            ui.end_row();
                        }
                    });
                }
                None => {
                    ui.label(RichText::new(format!("Créez ou ouvrez un projet natif pour dessiner : {}.", surface.nom().to_lowercase())).weak());
                }
            }
        }
        egui::Grid::new(("proprietes_surface", surface))
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let epaisseur_type: Option<f64> = self.surface_identique.as_ref().filter(|(o,_)| *o == surface).map(|(_, t)| t.epaisseur()).or_else(|| types_surface(surface)
                    .get(self.type_surface.get(&surface).copied().unwrap_or(0))
                    .map(|(_, cs)| cs.iter().map(|c| c.epaisseur).sum()));
                for (indice, champ) in champs(surface).into_iter().enumerate() {
                    if surface == Outil::Toiture && nom_champ(&champ) == "Pente" && self.toiture_plate {
                        continue;
                    }
                    if let Champ::Nombre(nom, defaut, suffixe) = champ {
                        ui.label(nom);
                        // épaisseur : celle du type choisi
                        if let (Some(e), "Épaisseur") = (epaisseur_type, nom) {
                            ui.label(crate::options::unite().afficher(e)).on_hover_text("Épaisseur du type (somme des couches)");
                            ui.end_row();
                            continue;
                        }
                        let valeur = self
                            .valeurs
                            .entry((surface, indice))
                            .or_insert(Valeur::Nombre(defaut));
                        if let Valeur::Nombre(valeur) = valeur {
                            ui.add_sized(
                                [90.0, 22.0],
                                if suffixe == " mm" { egui::DragValue::new(valeur).speed(10.0).dimensions() }
                                else { egui::DragValue::new(valeur).speed(0.1).max_decimals(2).suffix(suffixe) },
                            );
                        }
                        ui.end_row();
                    }
                }
            });
        true
    }

    fn options_outil(
        &mut self,
        ui: &mut egui::Ui,
        outil: Outil,
        projet: Option<&crate::modele::Projet>,
    ) {
        let fenetre = matches!(self.valeurs.get(&(Outil::Ouverture, 0)), Some(Valeur::Texte(t)) if t == "Fenêtre" || t == "Libre");
        let champs: Vec<_> = champs(outil)
            .into_iter()
            .enumerate()
            .filter(|(k, _)| outil != Outil::Ouverture || *k != 3 || fenetre)
            // mur : décalage bas dans Propriétés ; hauteur seulement si le haut est libre
            // et l'épaisseur seulement pour un mur générique (sans type)
            .filter(|(k, _)| outil != Outil::Mur || (*k != 3 && (*k != 1 || self.niveau_haut_mur.is_none()) && (*k != 2 || self.type_mur.is_none())))
            .collect();
        let mut changement_ouverture = false;
        for ligne in champs.chunks(2) {
            ui.horizontal(|ui| {
                for (k, champ) in ligne {
                    ui.push_id(("option_ruban", outil, *k), |ui| {
                        ui.horizontal(|ui| match champ {
                            Champ::Choix(nom, choix) => {
                                ui.label(*nom);
                                let v = self
                                    .valeurs
                                    .entry((outil, *k))
                                    .or_insert_with(|| Valeur::Texte(choix[0].into()));
                                if let Valeur::Texte(v) = v {
                                    egui::ComboBox::from_id_salt("choix")
                                        .width(110.0)
                                        .selected_text(&*v)
                                        .show_ui(ui, |ui| {
                                            for choix in *choix {
                                                if ui.selectable_value(v, (*choix).into(), *choix).changed() && outil == Outil::Ouverture && *k == 0 {
                                                    changement_ouverture = true;
                                                }
                                            }
                                        });
                                }
                            }
                            Champ::Nombre(nom, defaut, suffixe) => {
                                ui.label(*nom);
                                let v = self
                                    .valeurs
                                    .entry((outil, *k))
                                    .or_insert(Valeur::Nombre(*defaut));
                                if let Valeur::Nombre(v) = v {
                                    let pas = if *suffixe == " mm" { 10.0 } else { 0.1 };
                                    let champ = egui::DragValue::new(v).speed(pas);
                                    ui.add(if *suffixe == " mm" && !nom.contains("papier") { champ.dimensions() } else { champ.max_decimals(2).suffix(*suffixe) });
                                }
                            }
                            Champ::Saisie(nom, defaut) => {
                                ui.label(*nom);
                                let v = self
                                    .valeurs
                                    .entry((outil, *k))
                                    .or_insert_with(|| Valeur::Texte((*defaut).into()));
                                if let Valeur::Texte(v) = v {
                                    ui.add(egui::TextEdit::singleline(v).desired_width(100.0));
                                }
                            }
                        });
                    });
                }
            });
        }
        if changement_ouverture { self.ouverture_type_identique = None; }
        if outil == Outil::Mur {
            self.options_mur(ui, projet);
        }
    }

    fn options_mur(&mut self, ui: &mut egui::Ui, projet: Option<&crate::modele::Projet>) {
        if !self.mur_actif() {
            return;
        }
        let Some(projet) = projet else {
            ui.label(RichText::new("Créez ou ouvrez un projet natif pour dessiner un mur.").weak());
            return;
        };
        if self
            .niveau_mur
            .as_ref()
            .is_some_and(|id| projet.niveau(id).is_none())
        {
            self.niveau_mur = None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Niveau de base");
            let defaut = projet.niveaux.first();
            let nom = self
                .niveau_mur
                .as_ref()
                .and_then(|id| projet.niveau(id))
                .or(defaut)
                .map_or("Niveau 0", |n| n.nom.as_str());
            egui::ComboBox::from_id_salt("niveau_mur")
                .selected_text(nom)
                .show_ui(ui, |ui| {
                    if projet.niveaux.is_empty() {
                        ui.selectable_value(&mut self.niveau_mur, None, "Niveau 0");
                    }
                    for n in &projet.niveaux {
                        ui.selectable_value(&mut self.niveau_mur, Some(n.id.clone()), &n.nom);
                    }
                });
            ui.checkbox(&mut self.chainer_murs, "Chaîner");
        });
    }
}

fn bouton_outil(
    ui: &mut egui::Ui,
    r: egui::Rect,
    outil: Outil,
    grand: bool,
    actif: bool,
) -> egui::Response {
    let resp = ui.interact(r, ui.scope_id().with(("outil", outil)), Sense::click());
    if actif || resp.hovered() {
        ui.painter().rect_filled(
            r.shrink(1.0),
            4.0,
            if actif {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    let couleur = if actif {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    let (centre, taille, texte, ancrage) = if grand {
        (
            egui::pos2(r.center().x, r.top() + 24.0),
            32.0,
            egui::pos2(r.center().x, r.bottom() - 12.0),
            egui::Align2::CENTER_CENTER,
        )
    } else {
        (
            egui::pos2(r.left() + 12.0, r.center().y),
            17.0,
            egui::pos2(r.left() + 27.0, r.center().y),
            egui::Align2::LEFT_CENTER,
        )
    };
    icone(
        ui.painter(),
        egui::Rect::from_center_size(centre, egui::vec2(taille, taille)),
        outil,
        couleur,
    );
    ui.painter().text(
        texte,
        ancrage,
        outil.nom(),
        egui::FontId::proportional(12.0),
        couleur,
    );
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), actif, outil.nom())
    });
    resp.on_hover_text(match outil {
        Outil::CreerIdentique => "Créer à l'identique (CS)",
        Outil::Supprimer => "Supprimer (Suppr)",
        Outil::Cadrer => "Cadrer la sélection, sinon tout le modèle (F)",
        Outil::Isoler => "Isoler la sélection (I) ; Échap dans la vue : tout réafficher",
        Outil::Masquer => "Masquer la sélection (H)",
        Outil::Feuilles => "Feuilles : mise en page des vues, cartouche, export SVG/PDF",
        Outil::Nomenclatures => "Nomenclatures : quantités par catégorie et par type, export CSV",
        _ => outil.nom(),
    })
}

/// Composition d'un type en bande horizontale : une case par couche, largeur
/// proportionnelle à l'épaisseur, couleur du matériau ; extérieur à gauche.
pub fn bande_couches(ui: &mut egui::Ui, projet: &crate::modele::Projet, couches: &[crate::modele::Couche], hauteur: f32) {
    bande_couches_brouillon(ui, &projet.materiaux, couches, hauteur);
}

fn bande_couches_brouillon(ui: &mut egui::Ui, materiaux: &[crate::modele::Materiau], couches: &[crate::modele::Couche], hauteur: f32) {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), hauteur), Sense::hover());
    let total: f64 = couches.iter().map(|c| c.epaisseur.max(0.0)).sum();
    if total <= 0.0 {
        return;
    }
    let p = ui.painter();
    let mut x = r.left();
    let mut survol = None;
    for c in couches {
        let l = (c.epaisseur.max(0.0) / total) as f32 * r.width();
        let case = egui::Rect::from_min_size(egui::pos2(x, r.top()), egui::vec2(l, hauteur));
        let m = c.materiau.as_ref().and_then(|n| materiaux.iter().find(|m| &m.nom == n));
        let [cr, cg, cb] = m.map_or([200, 200, 196], |m| m.couleur);
        p.rect_filled(case, 0.0, Color32::from_rgb(cr, cg, cb));
        p.rect_stroke(case, 0.0, egui::Stroke::new(0.6, Color32::from_gray(90)), egui::StrokeKind::Inside);
        if resp.hover_pos().is_some_and(|q| case.contains(q)) {
            survol = Some(c);
        }
        x += l;
    }
    if let Some(c) = survol {
        resp.on_hover_text(format!("{} · {} · {}", c.materiau.as_deref().unwrap_or("Sans matériau"), c.fonction.libelle(), crate::options::unite().afficher(c.epaisseur)));
    }
}

#[derive(Clone, Copy)]
enum Glyphe {
    Croix,
    Liste,
}

/// Petit bouton à pictogramme dessiné (la police n'a pas ✕ ▾).
fn petit_bouton(ui: &mut egui::Ui, actif: bool, g: Glyphe) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), if actif { Sense::click() } else { Sense::hover() });
    let v = ui.visuals();
    if actif && resp.hovered() {
        ui.painter().rect_filled(r, 3.0, v.widgets.hovered.bg_fill);
    }
    let c = if actif { v.text_color() } else { v.weak_text_color().gamma_multiply(0.5) };
    let m = r.center();
    let p = ui.painter();
    match g {
        Glyphe::Liste => triangle_bas(p, m, c),
        Glyphe::Croix => {
            let t = egui::Stroke::new(1.3, c);
            p.line_segment([m + egui::vec2(-3.5, -3.5), m + egui::vec2(3.5, 3.5)], t);
            p.line_segment([m + egui::vec2(-3.5, 3.5), m + egui::vec2(3.5, -3.5)], t);
        }
    }
    resp
}

/// Flèche de liste déroulante (la police n'a pas toujours ▾).
fn triangle_bas(p: &egui::Painter, c: egui::Pos2, couleur: Color32) {
    let (l, h) = (4.0, 3.0);
    p.add(egui::Shape::convex_polygon(
        vec![egui::pos2(c.x - l, c.y - h * 0.5), egui::pos2(c.x + l, c.y - h * 0.5), egui::pos2(c.x, c.y + h)],
        couleur,
        egui::Stroke::NONE,
    ));
}

fn icone(p: &egui::Painter, r: egui::Rect, o: Outil, c: Color32) {
    use Outil::*;
    let t = egui::Stroke::new(if r.width() < 20.0 { 1.1 } else { 1.4 }, c);
    let pt = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let seg = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], t);
    };
    let cadre = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            egui::Rect::from_min_max(pt(x, y), pt(x + w, y + h)),
            0.0,
            t,
            egui::StrokeKind::Inside,
        );
    };
    let face = |points: &[(f32, f32)], opacite: f32| {
        p.add(egui::Shape::convex_polygon(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            c.gamma_multiply(opacite),
            t,
        ));
    };
    match o {
        Niveau | Altimetrie => {
            seg((0.05, 0.65), (0.95, 0.65));
            seg((0.3, 0.2), (0.3, 0.65));
            seg((0.18, 0.48), (0.3, 0.65));
            seg((0.42, 0.48), (0.3, 0.65));
            if o == Niveau {
                seg((0.05, 0.88), (0.95, 0.88));
            }
        }
        Quadrillage => {
            for n in [0.25, 0.65] {
                seg((n, 0.1), (n, 0.9));
                seg((0.1, n), (0.9, n));
            }
        }
        Mur => {
            // Mur mince en perspective, trait fin ; les couches ne se lisent que sur
            // le chant et le dessus, en trait plus léger.
            let fin = egui::Stroke::new(t.width * 0.6, c);
            let leger = egui::Stroke::new(t.width * 0.45, c.gamma_multiply(0.7));
            let (e, f) = (0.16, -0.10); // épaisseur du mur (profondeur)
            let face = |points: &[(f32, f32)], opacite: f32| {
                p.add(egui::Shape::convex_polygon(
                    points.iter().map(|&(x, y)| pt(x, y)).collect(),
                    c.gamma_multiply(opacite),
                    fin,
                ));
            };
            face(&[(0.10, 0.20), (0.72, 0.40), (0.72, 0.98), (0.10, 0.78)], 0.05);
            face(&[(0.10, 0.20), (0.10 + e, 0.20 + f), (0.72 + e, 0.40 + f), (0.72, 0.40)], 0.10);
            face(&[(0.72, 0.40), (0.72 + e, 0.40 + f), (0.72 + e, 0.98 + f), (0.72, 0.98)], 0.18);
            for couche in [1.0 / 3.0, 2.0 / 3.0] {
                let (dx, dy) = (couche * e, couche * f);
                p.line_segment([pt(0.10 + dx, 0.20 + dy), pt(0.72 + dx, 0.40 + dy)], leger);
                p.line_segment([pt(0.72 + dx, 0.40 + dy), pt(0.72 + dx, 0.98 + dy)], leger);
            }
        }
        Surfaces => {
            // Trois silhouettes espacées : couverture, plafond suspendu et plancher.
            face(
                &[(0.08, 0.22), (0.36, 0.05), (0.86, 0.16), (0.58, 0.32)],
                0.06,
            );
            seg((0.36, 0.05), (0.60, 0.16));
            face(
                &[(0.08, 0.49), (0.49, 0.39), (0.92, 0.50), (0.51, 0.61)],
                0.04,
            );
            seg((0.20, 0.46), (0.20, 0.36));
            seg((0.79, 0.47), (0.79, 0.37));
            face(
                &[(0.08, 0.78), (0.49, 0.68), (0.92, 0.79), (0.51, 0.90)],
                0.10,
            );
            seg((0.08, 0.78), (0.08, 0.86));
            seg((0.08, 0.86), (0.51, 0.98));
            seg((0.51, 0.98), (0.92, 0.87));
            seg((0.92, 0.87), (0.92, 0.79));
            seg((0.51, 0.90), (0.51, 0.98));
        }
        Dalle => {
            // Béton porteur : tranche épaisse, avec hachures dans l'épaisseur.
            face(
                &[(0.06, 0.35), (0.55, 0.15), (0.94, 0.40), (0.45, 0.61)],
                0.04,
            );
            face(
                &[(0.06, 0.35), (0.45, 0.61), (0.45, 0.88), (0.06, 0.62)],
                0.13,
            );
            face(
                &[(0.45, 0.61), (0.94, 0.40), (0.94, 0.67), (0.45, 0.88)],
                0.20,
            );
            for k in [0.0, 0.18, 0.36] {
                seg((0.50 + k, 0.69 - k * 0.42), (0.56 + k, 0.80 - k * 0.42));
            }
        }
        Sol => {
            // Finition mince : carrelage sur le dessus, très faible épaisseur.
            face(
                &[(0.06, 0.44), (0.55, 0.20), (0.94, 0.46), (0.45, 0.70)],
                0.05,
            );
            for k in [0.33, 0.66] {
                seg(
                    (0.06 + 0.49 * k, 0.44 - 0.24 * k),
                    (0.45 + 0.49 * k, 0.70 - 0.24 * k),
                );
                seg(
                    (0.06 + 0.39 * k, 0.44 + 0.26 * k),
                    (0.55 + 0.39 * k, 0.20 + 0.26 * k),
                );
            }
            seg((0.06, 0.50), (0.45, 0.76));
            seg((0.45, 0.76), (0.94, 0.52));
            seg((0.06, 0.44), (0.06, 0.50));
            seg((0.94, 0.46), (0.94, 0.52));
        }
        Toiture => {
            // Deux pans bien distincts autour d'un faîtage, avec débords.
            face(
                &[(0.04, 0.62), (0.32, 0.20), (0.75, 0.08), (0.47, 0.50)],
                0.16,
            );
            face(
                &[(0.32, 0.20), (0.68, 0.68), (0.97, 0.54), (0.75, 0.08)],
                0.04,
            );
            seg((0.32, 0.20), (0.75, 0.08));
            seg((0.04, 0.69), (0.68, 0.80));
            seg((0.68, 0.80), (0.97, 0.65));
            seg((0.68, 0.68), (0.68, 0.80));
            seg((0.19, 0.59), (0.47, 0.17));
        }
        Plafond => {
            // Panneau haut, suspendu à trois tiges ; lecture par le dessous.
            for (x, y) in [(0.13, 0.50), (0.55, 0.30), (0.89, 0.53)] {
                seg((x, y), (x, y - 0.24));
                seg((x - 0.05, y - 0.24), (x + 0.05, y - 0.24));
            }
            face(
                &[(0.06, 0.55), (0.55, 0.34), (0.94, 0.59), (0.45, 0.81)],
                0.13,
            );
            seg((0.06, 0.61), (0.45, 0.87));
            seg((0.45, 0.87), (0.94, 0.65));
            seg((0.25, 0.68), (0.74, 0.47));
        }
        Poteau | Poutre => {
            face(
                &[(0.33, 0.22), (0.57, 0.12), (0.75, 0.22), (0.51, 0.33)],
                0.09,
            );
            face(
                &[(0.33, 0.22), (0.51, 0.33), (0.51, 0.86), (0.33, 0.75)],
                0.06,
            );
            face(
                &[(0.51, 0.33), (0.75, 0.22), (0.75, 0.75), (0.51, 0.86)],
                0.17,
            );
            seg((0.16, 0.82), (0.49, 0.98));
            seg((0.49, 0.98), (0.91, 0.79));
        }
        Escalier => {
            for k in 0..4 {
                let x = 0.07 + k as f32 * 0.13;
                let y = 0.84 - k as f32 * 0.17;
                seg((x, y), (x, y - 0.16));
                seg((x, y - 0.16), (x + 0.13, y - 0.16));
                seg((x, y - 0.16), (x + 0.35, y - 0.31));
                seg((x + 0.35, y - 0.31), (x + 0.48, y - 0.31));
            }
            seg((0.07, 0.84), (0.55, 0.84));
            seg((0.55, 0.84), (0.94, 0.18));
        }
        GardeCorps => {
            // Main courante en perspective, avec retour et platines distinctes.
            seg((0.08, 0.36), (0.73, 0.12));
            seg((0.73, 0.12), (0.94, 0.25));
            seg((0.08, 0.46), (0.73, 0.22));
            seg((0.73, 0.22), (0.94, 0.35));
            seg((0.10, 0.66), (0.73, 0.43));
            seg((0.73, 0.43), (0.92, 0.55));
            for (x, y) in [(0.12, 0.36), (0.42, 0.25), (0.73, 0.14), (0.91, 0.27)] {
                seg((x, y), (x, y + 0.52));
                seg((x - 0.06, y + 0.54), (x + 0.06, y + 0.50));
            }
        }
        Ouverture => {
            cadre(0.15, 0.08, 0.7, 0.84);
            seg((0.5, 0.08), (0.5, 0.92));
            seg((0.15, 0.5), (0.85, 0.5));
        }
        Composant => {
            for (x, y) in [(0.1, 0.1), (0.55, 0.1), (0.1, 0.55), (0.55, 0.55)] {
                cadre(x, y, 0.32, 0.32);
            }
        }
        Deplacer => {
            for (a, b) in [
                ((0.1, 0.5), (0.9, 0.5)),
                ((0.5, 0.1), (0.5, 0.9)),
                ((0.1, 0.5), (0.27, 0.35)),
                ((0.1, 0.5), (0.27, 0.65)),
                ((0.9, 0.5), (0.73, 0.35)),
                ((0.9, 0.5), (0.73, 0.65)),
                ((0.5, 0.1), (0.35, 0.27)),
                ((0.5, 0.1), (0.65, 0.27)),
                ((0.5, 0.9), (0.35, 0.73)),
                ((0.5, 0.9), (0.65, 0.73)),
            ] {
                seg(a, b);
            }
        }
        Copier => {
            cadre(0.1, 0.1, 0.55, 0.55);
            cadre(0.35, 0.35, 0.55, 0.55);
        }
        CreerIdentique => {
            cadre(0.08, 0.12, 0.44, 0.44);
            cadre(0.48, 0.48, 0.44, 0.44);
            seg((0.68, 0.08), (0.68, 0.36));
            seg((0.54, 0.22), (0.82, 0.22));
        }
        Supprimer => {
            seg((0.16, 0.25), (0.84, 0.25));
            seg((0.35, 0.13), (0.65, 0.13));
            seg((0.25, 0.29), (0.30, 0.88));
            seg((0.75, 0.29), (0.70, 0.88));
            seg((0.30, 0.88), (0.70, 0.88));
            seg((0.42, 0.40), (0.42, 0.76));
            seg((0.58, 0.40), (0.58, 0.76));
        }
        Tourner | Arc => {
            let pts = (0..24)
                .map(|k| {
                    let a = 0.2 + k as f32 / 23.0 * 4.4;
                    pt(0.5 + 0.36 * a.cos(), 0.5 + 0.36 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, t));
            if o == Tourner {
                seg((0.85, 0.57), (0.9, 0.28));
                seg((0.85, 0.57), (0.62, 0.52));
            }
        }
        Aligner => {
            seg((0.15, 0.05), (0.15, 0.95));
            cadre(0.25, 0.15, 0.65, 0.23);
            cadre(0.25, 0.6, 0.4, 0.23);
        }
        Ajuster => {
            seg((0.1, 0.8), (0.9, 0.8));
            seg((0.3, 0.15), (0.3, 0.95));
            seg((0.6, 0.2), (0.9, 0.5));
            seg((0.6, 0.5), (0.9, 0.2));
        }
        Cote => {
            seg((0.15, 0.15), (0.15, 0.85));
            seg((0.85, 0.15), (0.85, 0.85));
            seg((0.05, 0.6), (0.95, 0.6));
            seg((0.06, 0.7), (0.24, 0.5));
            seg((0.76, 0.7), (0.94, 0.5));
        }
        Texte => {
            seg((0.12, 0.16), (0.88, 0.16));
            seg((0.5, 0.16), (0.5, 0.88));
            seg((0.32, 0.88), (0.68, 0.88));
        }
        Etiquette => {
            p.add(egui::Shape::closed_line(
                vec![
                    pt(0.08, 0.1),
                    pt(0.6, 0.1),
                    pt(0.93, 0.5),
                    pt(0.5, 0.93),
                    pt(0.08, 0.5),
                ],
                t,
            ));
            p.circle_stroke(pt(0.3, 0.3), 2.0, t);
        }
        Ligne => {
            seg((0.15, 0.85), (0.85, 0.15));
            p.circle_filled(pt(0.15, 0.85), r.width() * 0.06, c);
            p.circle_filled(pt(0.85, 0.15), r.width() * 0.06, c);
        }
        Polyligne => {
            p.add(egui::Shape::line(vec![pt(0.1, 0.85), pt(0.35, 0.3), pt(0.65, 0.65), pt(0.9, 0.15)], t));
            for (x, y) in [(0.1, 0.85), (0.35, 0.3), (0.65, 0.65), (0.9, 0.15)] {
                p.circle_filled(pt(x, y), r.width() * 0.05, c);
            }
        }
        Spline => {
            let pts = (0..=24).map(|k| {
                let x = k as f32 / 24.0;
                pt(0.1 + 0.8 * x, 0.5 - 0.32 * (x * std::f32::consts::TAU).sin())
            }).collect();
            p.add(egui::Shape::line(pts, t));
        }
        Cercle => {
            p.circle_stroke(r.center(), r.width() * 0.38, t);
            p.circle_filled(r.center(), r.width() * 0.05, c);
        }
        Zone => {
            cadre(0.08, 0.08, 0.84, 0.84);
            for k in 0..5 {
                let x = 0.1 + k as f32 * 0.17;
                seg((x, 0.85), (0.85, x));
            }
        }
        Symbole => {
            p.circle_stroke(r.center(), r.width() * 0.38, t);
            seg((0.5, 0.25), (0.5, 0.75));
            seg((0.25, 0.5), (0.75, 0.5));
        }
        Feuilles => {
            // feuille et son cartouche
            cadre(0.12, 0.08, 0.76, 0.84);
            cadre(0.5, 0.68, 0.38, 0.24);
            cadre(0.22, 0.18, 0.5, 0.4);
        }
        Nomenclatures => {
            // tableau : en-tête et lignes
            cadre(0.1, 0.12, 0.8, 0.76);
            face(&[(0.1, 0.12), (0.9, 0.12), (0.9, 0.3), (0.1, 0.3)], 0.35);
            for y in [0.48, 0.68] {
                seg((0.1, y), (0.9, y));
            }
            seg((0.42, 0.12), (0.42, 0.88));
        }
        Cadrer => {
            // coins de cadrage autour d'un objet
            for (x, y, dx, dy) in [(0.08, 0.08, 1.0, 1.0), (0.92, 0.08, -1.0, 1.0), (0.08, 0.92, 1.0, -1.0), (0.92, 0.92, -1.0, -1.0)] {
                seg((x, y), (x + 0.22 * dx, y));
                seg((x, y), (x, y + 0.22 * dy));
            }
            cadre(0.32, 0.32, 0.36, 0.36);
        }
        Isoler => {
            // objet plein, voisins en pointillé
            face(&[(0.32, 0.32), (0.68, 0.32), (0.68, 0.68), (0.32, 0.68)], 0.6);
            for (x, y) in [(0.05, 0.42), (0.8, 0.42)] {
                p.add(egui::Shape::dashed_line(&[pt(x, y), pt(x + 0.15, y), pt(x + 0.15, y + 0.16), pt(x, y + 0.16), pt(x, y)], t, 2.0, 2.0));
            }
        }
        Masquer => {
            // œil barré
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.5 - 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            let pts = (0..=16).map(|k| { let a = std::f32::consts::PI * k as f32 / 16.0; pt(0.5 - 0.42 * a.cos(), 0.5 + 0.22 * a.sin()) }).collect();
            p.add(egui::Shape::line(pts, t));
            p.circle_stroke(r.center(), r.width() * 0.1, t);
            seg((0.15, 0.85), (0.85, 0.15));
        }
        Legende => {
            for y in [0.2, 0.5, 0.8] {
                cadre(0.08, y - 0.07, 0.17, 0.14);
                seg((0.4, y), (0.92, y));
            }
        }
        VuePlan => {
            // plan : contour de pièces vu de dessus
            cadre(0.1, 0.15, 0.8, 0.7);
            seg((0.5, 0.15), (0.5, 0.55));
            seg((0.1, 0.55), (0.35, 0.55));
            seg((0.65, 0.55), (0.9, 0.55));
        }
        VueCoupe => {
            // repère de coupe : trait, têtes rondes et flèches de visée
            seg((0.12, 0.5), (0.88, 0.5));
            for x in [0.12, 0.88] {
                p.circle_stroke(pt(x, 0.5), r.width() * 0.1, t);
                p.add(egui::Shape::convex_polygon(vec![pt(x - 0.1, 0.38), pt(x + 0.1, 0.38), pt(x, 0.2)], c, egui::Stroke::NONE));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn croquis_mur_suit_hauteur_niveaux_et_decalages() {
        let mut projet = crate::modele::Projet::nouveau("Croquis");
        projet.niveaux = vec![crate::modele::Niveau {
            id: "bas".into(), nom: "Rez-de-chaussée".into(), altitude: 0.0, origine: None,
        }];
        projet.niveaux.push(crate::modele::Niveau {
            id: "haut".into(), nom: "Étage".into(), altitude: 2800.0, origine: None,
        });
        let mut params = crate::modele::edition::ParametresMur::default();
        let (haut, bas) = limites_croquis_mur(&params, &projet, 10.0, 46.0);
        params.decalage = -200.0;
        let (haut_decale, bas_decale) = limites_croquis_mur(&params, &projet, 10.0, 46.0);
        assert!(haut_decale > haut && bas_decale > bas); // hauteur libre conservée
        params.decalage = 0.0;
        params.niveau_haut = Some("haut".into());
        params.decalage_haut = -200.0;
        let (haut_decale, bas_inchange) = limites_croquis_mur(&params, &projet, 10.0, 46.0);
        assert!(haut_decale > haut && (bas_inchange - bas).abs() < 0.01);
        params.decalage_haut = 200.0;
        assert!(limites_croquis_mur(&params, &projet, 10.0, 46.0).0 < haut);
        params.niveau_haut = None;
        params.hauteur = 2400.0;
        assert!(limites_croquis_mur(&params, &projet, 10.0, 46.0).0 > haut);
        params.decalage = -100_000.0;
        let (haut, bas) = limites_croquis_mur(&params, &projet, 10.0, 46.0);
        assert!(haut < bas && bas < 60.0);
    }

    #[test]
    fn boutons_modifier_declenchent_les_actions_sans_changer_outil() {
        let ctx = egui::Context::default();
        let mut ruban = RubanEdition::default();
        ruban.activer_mur();
        for outil in [Outil::CreerIdentique, Outil::Supprimer] {
            let mut action = None;
            let mut frame = |events| { let mut output = ctx.run_ui(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1500.0, 200.0))),
                events,
                ..Default::default()
            }, |ui| {
                egui::Panel::top("modifier_test").show(ui, |ui| {
                    if let Some(a) = ruban.afficher(ui, Menu::Modifier, None) { action = Some(a); }
                });
            }); output.textures_delta.clear(); output };
            frame(vec![]);
            let output = frame(vec![]);
            let position = output.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == outil.nom() => Some(t.pos + t.galley.size() / 2.0),
                _ => None,
            }).expect("commande du ruban absente");
            for pressed in [true, false] {
                frame(vec![egui::Event::PointerMoved(position), egui::Event::PointerButton {
                    pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE,
                }]);
            }
            drop(frame);
            assert_eq!(action, Some(outil));
            assert_eq!(ruban.outil(), Some(Outil::Mur));
        }
    }

    fn afficher(
        ctx: &egui::Context,
        ruban: &mut RubanEdition,
        largeur: f32,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut sortie = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(largeur, 520.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                egui::Panel::top("ruban_test")
                    .show(ui, |ui| ruban.afficher(ui, Menu::Modeliser, None));
                egui::Panel::left("proprietes_test")
                    .exact_size(284.0)
                    .show(ui, |ui| {
                        ui.label("Propriétés");
                        ruban.proprietes_surfaces(ui, None);
                    });
                egui::CentralPanel::default().show(ui, |_| {});
            },
        );
        sortie.textures_delta.clear();
        sortie
    }

    fn libelle_correspond(o: Outil, texte: &str) -> bool {
        texte == o.nom()
    }

    fn cliquer(
        ctx: &egui::Context,
        ruban: &mut RubanEdition,
        largeur: f32,
        texte: &str,
    ) -> egui::FullOutput {
        afficher(ctx, ruban, largeur, vec![]);
        let sortie = afficher(ctx, ruban, largeur, vec![]);
        // dernière occurrence : Propriétés est dessinée après le ruban, qui peut
        // afficher le même libellé (sorte d'élément horizontal)
        let position = sortie
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if t.galley.text() == texte =>
                {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .last()
            .unwrap_or_else(|| panic!("commande absente : {texte}"));

        for pressed in [true, false] {
            afficher(
                ctx,
                ruban,
                largeur,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        afficher(ctx, ruban, largeur, vec![])
    }

    #[test]
    fn palette_mur_affiche_contraintes_et_choix_du_document() {
        let ctx = egui::Context::default();
        let mut ruban = RubanEdition::default();
        ruban.activer_mur();
        let mut projet = crate::modele::Projet::nouveau("Mur");
        projet.phases = vec!["Existant".into(), "Construction".into()];
        projet.sous_projets = vec!["Enveloppe".into(), "Structure".into()];
        for largeur in [220.0, 260.0, 300.0, 420.0] {
            let mut sortie = None;
            for _ in 0..3 {
                let mut frame = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(largeur, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            assert!(ruban.proprietes_mur(ui, Some(&projet)));
                        });
                    },
                );
                frame.textures_delta.clear();
                sortie = Some(frame);
            }
            let textes: Vec<_> = sortie
                .unwrap()
                .shapes
                .into_iter()
                .filter_map(|s| match s.shape {
                    egui::Shape::Text(t) => Some((
                        t.galley.text().to_string(),
                        egui::Rect::from_min_size(t.pos, t.galley.size()),
                    )),
                    _ => None,
                })
                .collect();
            for attendu in ["Haut", "Bas", "Libre", "Épaisseur", "Implantation", "Chaîner"] {
                assert!(textes.iter().any(|(t, _)| t == attendu), "Champ absent : {attendu}");
            }
            let position = |nom: &str| textes.iter().find(|(t, _)| t == nom).unwrap().1;
            // limites sur une ligne chacune, le haut au-dessus du bas, puis la grille
            assert!((position("Haut").center().y - position("Libre").center().y).abs() < 3.0);
            assert!(position("Bas").top() > position("Haut").bottom());
            assert!(position("Épaisseur").top() > position("Bas").bottom());
        }
    }

    #[test]
    fn selecteur_surfaces_et_reglages_independants() {
        for largeur in [800.0, 1500.0] {
            let ctx = egui::Context::default();
            let mut ruban = RubanEdition::default();
            let sortie = cliquer(&ctx, &mut ruban, largeur, "Éléments horizontaux");
            assert_eq!(ruban.selection, Some(Outil::Dalle));
            let encart = sortie
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Dalle · Dalle BA 200" => Some(t.pos),
                    _ => None,
                })
                .last()
                .expect("sélecteur Dalle dans Propriétés");
            assert!(
                encart.x < 284.0 && encart.y > HAUTEUR_GROUPE,
                "le sélecteur doit se trouver dans la palette sous le ruban"
            );
            // la liste reste ouverte d'un onglet de sorte à l'autre
            cliquer(&ctx, &mut ruban, largeur, "Dalle · Dalle BA 200");
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Sol",
            );
            assert_eq!(ruban.selection, Some(Outil::Sol));
            assert!(matches!(
                ruban.valeurs.get(&(Outil::Sol, 1)),
                Some(Valeur::Nombre(0.0))
            ));
            ruban.valeurs.insert((Outil::Sol, 1), Valeur::Nombre(21.0));
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Plafond",
            );
            assert_eq!(ruban.selection, Some(Outil::Plafond));
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Dalle",
            );
            assert!(matches!(
                ruban.valeurs.get(&(Outil::Dalle, 1)),
                Some(Valeur::Nombre(0.0))
            ));
            cliquer(
                &ctx,
                &mut ruban,
                largeur,
                "Sol",
            );
            assert!(matches!(
                ruban.valeurs.get(&(Outil::Sol, 1)),
                Some(Valeur::Nombre(21.0))
            ));
            let sortie = cliquer(&ctx, &mut ruban, largeur, "Toiture");
            assert_eq!(ruban.selection, Some(Outil::Toiture));
            assert!(
                sortie.shapes.iter().any(
                    |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Pente")
                )
            );
            cliquer(&ctx, &mut ruban, largeur, "Mur");
            assert!(
                ruban.mur_actif(),
                "le sélecteur ne doit pas réactiver les surfaces après le clic Mur"
            );
            cliquer(&ctx, &mut ruban, largeur, "Éléments horizontaux");
            assert_eq!(
                ruban.selection,
                Some(Outil::Toiture),
                "le groupe retrouve le dernier choix"
            );
        }
    }

    #[test]
    fn reglages_accessibles_en_mode_compact() {
        let ctx = egui::Context::default();
        let mut ruban = RubanEdition::default();
        ruban.activer_mur();
        let mut afficher = |events| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 520.0),
                )),
                events,
                ..Default::default()
            };
            let mut sortie = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default()
                    .show(ui, |ui| ruban.afficher(ui, Menu::Modeliser, None));
            });
            sortie.textures_delta.clear();
            sortie
        };
        let sortie = afficher(vec![]);
        let bouton = sortie
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "..." => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .expect("bouton de réglages compact");
        afficher(vec![
            egui::Event::PointerMoved(bouton),
            egui::Event::PointerButton {
                pos: bouton,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        afficher(vec![egui::Event::PointerButton {
            pos: bouton,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        let sortie = afficher(vec![]);
        assert!(
            sortie.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Implantation")
            ),
            "le clic sur les réglages doit rendre les options Mur accessibles"
        );
    }

    #[test]
    fn commandes_et_options_tiennent_aux_largeurs_du_viewer() {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::light());
        let projet = crate::modele::Projet::nouveau("Ruban");
        for largeur in [800.0, 1000.0, 1280.0, 1500.0] {
            for menu in [Menu::Modeliser, Menu::Modifier, Menu::Annoter] {
                let outils: Vec<_> = groupes(menu)
                    .iter()
                    .flat_map(|(_, outils)| outils.iter().copied())
                    .collect();
                let selections = outils
                    .iter()
                    .copied()
                    .map(|o| {
                        if o == Outil::Surfaces {
                            Outil::Dalle
                        } else {
                            o
                        }
                    })
                    .chain(
                        SURFACES
                            .iter()
                            .map(|&(o, _)| o)
                            .filter(|_| menu == Menu::Modeliser),
                    );
                for selection in std::iter::once(None).chain(selections.map(Some)) {
                    let mut ruban = RubanEdition {
                        selection,
                        ..Default::default()
                    };
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(largeur, 520.0),
                        )),
                        ..Default::default()
                    };
                    let mut sortie = ctx.run_ui(input, |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            let debut = ui.next_widget_position().y;
                            ruban.afficher(ui, menu, Some(&projet));
                            assert!(
                                ui.next_widget_position().y - debut <= 100.0,
                                "ruban trop haut à {largeur}, {selection:?}"
                            );
                        });
                    });
                    sortie.textures_delta.clear();
                    let mut textes = Vec::new();
                    fn relever(forme: &egui::Shape, textes: &mut Vec<(String, egui::Rect)>) {
                        match forme {
                            egui::Shape::Text(t) => textes.push((
                                t.galley.text().into(),
                                egui::Rect::from_min_size(t.pos, t.galley.size()),
                            )),
                            egui::Shape::Vec(formes) => {
                                formes.iter().for_each(|f| relever(f, textes))
                            }
                            _ => {}
                        }
                    }
                    for forme in &sortie.shapes {
                        relever(&forme.shape, &mut textes);
                    }
                    for outil in &outils {
                        let (_, rect) = textes
                            .iter()
                            .find(|(texte, _)| libelle_correspond(*outil, texte))
                            .unwrap_or_else(|| panic!("commande absente : {outil:?}"));
                        assert!(
                            rect.left() >= 0.0 && rect.right() <= largeur,
                            "commande hors écran à {largeur}, {selection:?} : {outil:?} {rect:?}"
                        );
                    }
                    let commandes: Vec<_> = textes
                        .iter()
                        .filter(|(texte, _)| outils.iter().any(|o| libelle_correspond(*o, texte)))
                        .collect();
                    for (indice, (nom, rect)) in commandes.iter().enumerate() {
                        for (autre, voisin) in commandes.iter().skip(indice + 1) {
                            assert!(
                                !rect.shrink(0.5).intersects(*voisin),
                                "commandes superposées à {largeur}, {selection:?} : {nom} / {autre}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn creation_identique_reutilise_types_niveaux_et_vue() {
        let p = crate::modele::Projet::lire_json(&std::fs::read_to_string("samples/maison_native.modele.json").unwrap()).unwrap();
        let d = &p.dalles[0];
        let mut ruban = RubanEdition::default();
        ruban.surface_identique(&p, d).unwrap();
        let options = ruban.dalle_active().unwrap();
        assert_eq!(options.type_dalle.nom, p.type_dalle(&d.type_dalle).unwrap().nom);
        assert_eq!(options.type_dalle.couches, p.type_dalle(&d.type_dalle).unwrap().couches);
        assert_eq!(options.niveau.as_ref(), Some(&d.niveau));
        assert_eq!(options.decalage, d.decalage);
        let vue = p.vues.iter().find(|v| v.id == "coupe").unwrap();
        ruban.coupe_identique(vue).unwrap();
        let c = ruban.coupe_active().unwrap();
        assert_eq!(c.echelle, vue.echelle);
        assert_eq!(c.type_vue, vue.type_vue);
        assert_eq!(c.profondeur, 6000.0);
    }

}


/// Pictogramme générique d'une commande des bandeaux Début et Sortie.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icone {
    Nouveau,
    Ouvrir,
    Fermer,
    Importer,
    Fiche,
    Options,
    Enregistrer,
    EnregistrerSous,
    Exporter(&'static str),
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

impl Commande {
    pub fn new(cle: &'static str, nom: &'static str, icone: Icone, aide: &'static str, actif: bool) -> Commande {
        Commande { cle, nom, icone, aide, actif, enfonce: false }
    }
}

/// Bandeau à grandes icônes, de la hauteur des bandeaux d'outils : groupes séparés,
/// nom du groupe en bas. Rend la réponse de chaque commande (clé, réponse).
pub fn bandeau(ui: &mut egui::Ui, groupes: &[(&str, Vec<Commande>)]) -> Vec<(&'static str, egui::Response)> {
    let mut reponses = Vec::new();
    egui::ScrollArea::horizontal()
        .id_salt("ruban_commandes_fixes")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = ESPACE_GROUPES;
            ui.horizontal_top(|ui| {
                for (indice, (nom, commandes)) in groupes.iter().enumerate() {
                    let largeurs: Vec<f32> = commandes.iter().map(|c| (largeur_texte(ui, c.nom, 12.0) + 20.0).max(62.0)).collect();
                    let largeur = (largeurs.iter().sum::<f32>() + ESPACE_COLONNES * largeurs.len().saturating_sub(1) as f32).max(largeur_texte(ui, nom, 11.0) + 8.0);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(largeur, HAUTEUR_GROUPE), Sense::hover());
                    if indice > 0 {
                        let x = rect.left() - ESPACE_GROUPES / 2.0;
                        ui.painter().line_segment([egui::pos2(x, rect.top() + 4.0), egui::pos2(x, rect.bottom() - 4.0)], ui.visuals().widgets.noninteractive.bg_stroke);
                    }
                    let mut x = rect.left();
                    for (c, l) in commandes.iter().zip(largeurs) {
                        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(l, HAUTEUR_OUTILS));
                        let mut sous = ui.new_child(egui::UiBuilder::new().max_rect(r));
                        if !c.actif {
                            sous.disable();
                        }
                        let resp = sous.interact(r, sous.scope_id().with(("commande", c.cle)), Sense::click());
                        if c.enfonce || (c.actif && resp.hovered()) {
                            sous.painter().rect_filled(r.shrink(1.0), 4.0, if c.enfonce { sous.visuals().selection.bg_fill } else { sous.visuals().widgets.hovered.weak_bg_fill });
                        }
                        let couleur = if !c.actif {
                            sous.visuals().weak_text_color().gamma_multiply(0.6)
                        } else if c.enfonce {
                            sous.visuals().selection.stroke.color
                        } else {
                            sous.visuals().text_color()
                        };
                        icone_commande(sous.painter(), egui::Rect::from_center_size(egui::pos2(r.center().x, r.top() + 24.0), egui::vec2(32.0, 32.0)), c.icone, couleur);
                        sous.painter().text(egui::pos2(r.center().x, r.bottom() - 12.0), egui::Align2::CENTER_CENTER, c.nom, egui::FontId::proportional(12.0), couleur);
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, c.actif, c.enfonce, c.nom));
                        let resp = if c.aide.is_empty() { resp } else { resp.on_hover_text(c.aide) };
                        reponses.push((c.cle, resp));
                        x += l + ESPACE_COLONNES;
                    }
                    ui.painter().text(egui::pos2(rect.center().x, rect.bottom() - 8.0), egui::Align2::CENTER_CENTER, *nom, egui::FontId::proportional(11.0), ui.visuals().weak_text_color());
                }
            });
        });
    reponses
}

/// Pictogrammes génériques des commandes de fichier.
fn icone_commande(p: &egui::Painter, r: egui::Rect, i: Icone, c: Color32) {
    let t = egui::Stroke::new(1.4, c);
    let pt = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let seg = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], t);
    };
    // document à coin replié
    let document = || {
        p.add(egui::Shape::closed_line(vec![pt(0.2, 0.06), pt(0.6, 0.06), pt(0.8, 0.26), pt(0.8, 0.94), pt(0.2, 0.94)], t));
        seg((0.6, 0.06), (0.6, 0.26));
        seg((0.6, 0.26), (0.8, 0.26));
    };
    // disquette
    let disquette = || {
        p.add(egui::Shape::closed_line(vec![pt(0.12, 0.12), pt(0.74, 0.12), pt(0.88, 0.26), pt(0.88, 0.88), pt(0.12, 0.88)], t));
        p.rect_stroke(egui::Rect::from_min_max(pt(0.3, 0.12), pt(0.66, 0.34)), 0.0, t, egui::StrokeKind::Inside);
        p.rect_stroke(egui::Rect::from_min_max(pt(0.26, 0.54), pt(0.74, 0.88)), 0.0, t, egui::StrokeKind::Inside);
    };
    match i {
        Icone::Nouveau => {
            document();
            seg((0.5, 0.48), (0.5, 0.78));
            seg((0.35, 0.63), (0.65, 0.63));
        }
        Icone::Ouvrir => {
            p.add(egui::Shape::closed_line(vec![pt(0.06, 0.2), pt(0.36, 0.2), pt(0.44, 0.3), pt(0.86, 0.3), pt(0.86, 0.84), pt(0.06, 0.84)], t));
            p.add(egui::Shape::line(vec![pt(0.06, 0.84), pt(0.22, 0.46), pt(0.96, 0.46), pt(0.86, 0.84)], t));
        }
        Icone::Fermer => {
            document();
            seg((0.36, 0.48), (0.64, 0.76));
            seg((0.64, 0.48), (0.36, 0.76));
        }
        Icone::Importer => {
            p.add(egui::Shape::line(vec![pt(0.14, 0.5), pt(0.14, 0.9), pt(0.86, 0.9), pt(0.86, 0.5)], t));
            seg((0.5, 0.08), (0.5, 0.66));
            p.add(egui::Shape::line(vec![pt(0.32, 0.48), pt(0.5, 0.66), pt(0.68, 0.48)], t));
        }
        Icone::Fiche => {
            document();
            for y in [0.44, 0.6, 0.76] {
                seg((0.32, y), (0.68, y));
            }
        }
        Icone::Options => {
            let m = r.center();
            p.circle_stroke(m, r.width() * 0.2, t);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = egui::vec2(a.cos(), a.sin());
                p.line_segment([m + d * r.width() * 0.3, m + d * r.width() * 0.44], egui::Stroke::new(3.0, c));
            }
            p.circle_stroke(m, r.width() * 0.32, t);
        }
        Icone::Enregistrer => disquette(),
        Icone::EnregistrerSous => {
            disquette();
            p.add(egui::Shape::convex_polygon(vec![pt(0.62, 1.0), pt(1.0, 0.62), pt(1.06, 0.68), pt(0.68, 1.06)], c, egui::Stroke::NONE));
        }
        Icone::Exporter(format) => {
            document();
            p.text(pt(0.5, 0.64), egui::Align2::CENTER_CENTER, format, egui::FontId::proportional(r.width() * 0.26), c);
        }
    }
}
