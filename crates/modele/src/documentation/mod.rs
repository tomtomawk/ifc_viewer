//! Documentation native : références, annotations par vue, feuilles et nomenclatures.

mod feuilles;
pub use feuilles::*;
mod cotation;
pub use cotation::*;
mod nomenclatures;
pub use nomenclatures::*;
#[cfg(test)]
mod tests;
pub use super::*;
use noyau::trace2d::{Forme, Primitive, Role, Style};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Quadrillage {
    pub id: String,
    pub nom: String,
    pub debut: [f64; 2],
    pub fin: [f64; 2],
}
/// Type de trait du dessin 2D : épaisseur et motif de tirets, en mm du papier (même
/// aspect à toutes les échelles).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TypeTrait {
    pub id: String,
    pub nom: String,
    /// Épaisseur, mm du papier.
    pub epaisseur: f64,
    /// Longueurs alternées trait, blanc, trait… (mm du papier) ; vide : trait continu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tirets: Vec<f64>,
}

impl TypeTrait {
    /// Trait d'un segment sans type : continu fin.
    pub fn standard() -> TypeTrait {
        TypeTrait { id: String::new(), nom: "Continu fin".into(), epaisseur: 0.18, tirets: Vec::new() }
    }

    pub fn valider(&self) -> Result<(), String> {
        if self.nom.trim().is_empty() {
            return Err("Type de trait sans nom.".into());
        }
        if !(self.epaisseur > 0.0 && self.epaisseur <= 5.0) {
            return Err(format!("{} : épaisseur entre 0 et 5 mm.", self.nom));
        }
        if self.tirets.iter().any(|t| !(t.is_finite() && *t > 0.0)) || (!self.tirets.is_empty() && self.tirets.len() % 2 == 1) {
            return Err(format!("{} : tirets en paires trait, blanc de longueurs positives.", self.nom));
        }
        Ok(())
    }

    /// Parties tracées du segment `a`–`b` (mm du modèle), `echelle` : mm du modèle par
    /// mm du papier ; `phase` : avancée dans le motif, reportée au segment suivant.
    pub fn decouper(&self, a: DVec2, b: DVec2, echelle: f64, phase: &mut f64) -> Vec<[DVec2; 2]> {
        let l = a.distance(b);
        let periode: f64 = self.tirets.iter().sum::<f64>() * echelle;
        if self.tirets.is_empty() || !(periode > 1e-9) || l < 1e-9 {
            return vec![[a, b]];
        }
        let u = (b - a) / l;
        let mut out = Vec::new();
        let mut s = 0.0;
        // nombre de pas borné : un motif trop fin pour la longueur reste continu
        if l / periode * self.tirets.len() as f64 > 20_000.0 {
            return vec![[a, b]];
        }
        while s < l {
            let mut t = phase.rem_euclid(periode);
            let mut k = 0;
            while t >= self.tirets[k] * echelle {
                t -= self.tirets[k] * echelle;
                k = (k + 1) % self.tirets.len();
            }
            let reste = self.tirets[k] * echelle - t;
            let pas = reste.min(l - s);
            if k % 2 == 0 {
                out.push([a + u * s, a + u * (s + pas)]);
            }
            s += pas;
            *phase += pas;
        }
        out
    }
}

/// Types de trait du gabarit par défaut (sa section `types_traits`, contrôlée par test).
#[cfg_attr(not(test), allow(dead_code))]
pub fn types_traits_standard() -> Vec<TypeTrait> {
    let t = |id: &str, nom: &str, e: f64, tirets: &[f64]| TypeTrait { id: id.into(), nom: nom.into(), epaisseur: e, tirets: tirets.to_vec() };
    vec![
        t("trait-continu-fin", "Continu fin", 0.18, &[]),
        t("trait-continu-moyen", "Continu moyen", 0.35, &[]),
        t("trait-continu-fort", "Continu fort", 0.50, &[]),
        t("trait-tirets", "Tirets", 0.18, &[3.0, 1.5]),
        t("trait-pointilles", "Pointillés", 0.18, &[0.5, 1.0]),
        t("trait-axe", "Axe", 0.13, &[6.0, 1.0, 1.0, 1.0]),
        t("trait-cache", "Caché", 0.25, &[2.0, 1.0]),
    ]
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "ancrage", rename_all = "snake_case")]
pub enum PointCote {
    Fixe { point: [f64; 2] },
    Mur { id: String, fin: bool },
    /// Arête de référence d'un élément (`cotation::aretes_reference`), à l'abscisse
    /// relative `t` : la cote suit l'élément.
    Arete { id: String, arete: usize, t: f64 },
}

/// Arête de référence d'une branche de cote angulaire.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RefArete {
    pub id: String,
    pub arete: usize,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "forme", rename_all = "snake_case")]
pub enum AnnotationForme {
    Texte {
        point: [f64; 2],
        texte: String,
    },
    Ligne {
        debut: [f64; 2],
        fin: [f64; 2],
    },
    Zone {
        contour: Vec<[f64; 2]>,
    },
    /// Dessin 2D : polyligne (segments droits ou arcs), spline (passant par ses points),
    /// cercle, arc par trois points (début, passage, fin) ; mm dans le plan de la vue.
    Polyligne {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        fermee: bool,
        /// Courbure de chaque segment (segment k : du sommet k au suivant ; le dernier
        /// d'une polyligne fermée revient au premier) : tangente du quart de l'angle au
        /// centre de l'arc, positive dans le sens trigonométrique, 1 pour un demi-cercle ;
        /// 0 : segment droit. Absent ou vide : tous les segments sont droits.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        courbures: Vec<f64>,
        /// Polyligne fermée : remplissage (motif d'un matériau ou choisi).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remplissage: Option<RemplissageContour>,
        /// Type de trait de chaque segment (id d'un type du projet) ; absent ou vide :
        /// continu fin.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        traits: Vec<String>,
    },
    Spline {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        fermee: bool,
    },
    Cercle {
        centre: [f64; 2],
        rayon: f64,
    },
    Arc {
        points: [[f64; 2]; 3],
    },
    Cote {
        debut: PointCote,
        fin: PointCote,
        decalage: f64,
    },
    /// Cote angulaire : sommet, un point sur chaque branche ; l'arc de cote passe à
    /// `rayon` du sommet et mesure l'angle saillant (≤ 180°).
    CoteAngulaire {
        sommet: [f64; 2],
        debut: [f64; 2],
        fin: [f64; 2],
        rayon: f64,
        /// Arêtes cotées (première et seconde branche) : l'angle suit les éléments.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        aretes: Option<[RefArete; 2]>,
    },
    /// Cote de diamètre : centre et un point du cercle ; la ligne de cote traverse le
    /// cercle par le centre.
    CoteDiametre {
        centre: [f64; 2],
        point: [f64; 2],
    },
    /// Cote altimétrique : point visé et position du texte (ligne de rappel entre les
    /// deux). Altitude du projet (0 du projet) ou relative au niveau sous le point.
    Altimetrie {
        point: [f64; 2],
        repere: [f64; 2],
        #[serde(default)]
        relative_niveau: bool,
        #[serde(default = "deux_decimales")]
        decimales: u8,
    },
    Etiquette {
        element: String,
        point: [f64; 2],
    },
}
fn deux_decimales() -> u8 {
    2
}

impl AnnotationForme {
    /// Points de la forme (repère de la vue, mm) : déplacements et transformations.
    pub fn points_mesure_mut(&mut self) -> Vec<&mut [f64; 2]> {
        match self {
            Self::CoteAngulaire { sommet, debut, fin, .. } => vec![sommet, debut, fin],
            Self::CoteDiametre { centre, point } => vec![centre, point],
            Self::Altimetrie { point, repere, .. } => vec![point, repere],
            _ => Vec::new(),
        }
    }

    /// Forme du dessin 2D (ligne, polyligne, spline, cercle, arc), None pour les autres.
    pub fn forme_2d(&self) -> Option<noyau::forme2d::Forme2D> {
        use noyau::forme2d::Forme2D as F;
        let v = |p: &[f64; 2]| DVec2::from_array(*p);
        Some(match self {
            Self::Ligne { debut, fin } => F::Ligne([v(debut), v(fin)]),
            Self::Polyligne { points, fermee, courbures, .. } => F::Polyligne { points: points.iter().map(v).collect(), fermee: *fermee, courbures: courbures.clone() },
            Self::Spline { points, fermee } => F::Spline { points: points.iter().map(v).collect(), fermee: *fermee },
            Self::Cercle { centre, rayon } => F::Cercle { centre: v(centre), rayon: *rayon },
            Self::Arc { points } => F::Arc(points.map(|p| v(&p))),
            _ => return None,
        })
    }

    /// Annotation d'une forme du dessin 2D.
    pub fn depuis_forme_2d(f: &noyau::forme2d::Forme2D) -> AnnotationForme {
        use noyau::forme2d::Forme2D as F;
        let a = |p: &DVec2| p.to_array();
        match f {
            F::Ligne([d, e]) => Self::Ligne { debut: a(d), fin: a(e) },
            F::Polyligne { points, fermee, courbures } => Self::Polyligne { points: points.iter().map(a).collect(), fermee: *fermee, courbures: courbures.clone(), remplissage: None, traits: Vec::new() },
            F::Spline { points, fermee } => Self::Spline { points: points.iter().map(a).collect(), fermee: *fermee },
            F::Cercle { centre, rayon } => Self::Cercle { centre: a(centre), rayon: *rayon },
            F::Arc(p) => Self::Arc { points: p.map(|q| q.to_array()) },
        }
    }

    /// Contour développé d'une polyligne (arcs remplacés par leurs points, sans retour au
    /// premier sommet) : tracé du remplissage et des régions. None pour une autre forme.
    pub fn contour_developpe(&self) -> Option<Vec<[f64; 2]>> {
        let Self::Polyligne { points, fermee, courbures, .. } = self else { return None };
        if courbures.iter().all(|k| *k == 0.0) {
            return Some(points.clone());
        }
        let sommets: Vec<DVec2> = points.iter().map(|p| DVec2::from_array(*p)).collect();
        let mut l = noyau::forme2d::developper(&sommets, *fermee, courbures);
        if *fermee && l.len() > 1 {
            l.pop();
        }
        Some(l.into_iter().map(|p| p.to_array()).collect())
    }

    /// Nombre de segments d'une polyligne de `points` sommets.
    pub fn segments_polyligne(points: usize, fermee: bool) -> usize {
        points.saturating_sub(1) + usize::from(fermee && points > 2)
    }
}

/// Remplissage d'une polyligne fermée : motif de coupe d'un matériau (premier plan, encre
/// et arrière-plan du matériau), ou motif choisi (hachures du projet ou aplat) à l'encre
/// gris foncé.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum RemplissageContour {
    Materiau { materiau: String },
    Motif { motif: MotifCoupe },
}

impl RemplissageContour {
    /// Encre d'un motif choisi.
    pub const ENCRE: [u8; 3] = [70, 70, 70];

    pub fn libelle(&self) -> String {
        match self {
            Self::Materiau { materiau } => format!("Matériau : {materiau}"),
            Self::Motif { motif } => format!("Motif : {}", motif.libelle()),
        }
    }
}

/// Primitives du remplissage d'un contour (mm du plan de la vue) : arrière-plan (aplat
/// s'il n'est pas blanc, ou hachures) puis premier plan, hachures à l'échelle de la vue.
fn primitives_remplissage(p: &Projet, r: &RemplissageContour, contour: &[[f64; 2]], echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    use noyau::motif::{GrilleMotif, Motif, hachures_plan};
    let (motif, encre, fond) = match r {
        RemplissageContour::Materiau { materiau } => match p.materiau(materiau) {
            Some(m) => (m.motif.clone(), m.encre_coupe(), Some(m.fond_coupe())),
            None => return Vec::new(),
        },
        RemplissageContour::Motif { motif } => (motif.clone(), RemplissageContour::ENCRE, None),
    };
    let contours = vec![contour.iter().map(|q| [q[0] as f32, q[1] as f32]).collect()];
    let tris: Vec<[[f64; 2]; 3]> = noyau::texte::trianguler(contours).iter().map(|t| t.map(|q| [q[0] as f64, q[1] as f64])).collect();
    let couleur = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    let aplat = |c: [u8; 3]| Primitive {
        forme: Forme::Triangles(tris.iter().map(|t| t.map(|q| pt(DVec2::from_array(q)))).collect()),
        style: Style { couleur: couleur(c), epaisseur: 0.0 },
        role: Role::Hachure,
        element: None,
    };
    let hachures = |m: &MotifCoupe, c: [u8; 3]| -> Option<Primitive> {
        let d = p.definition_motif(m)?;
        let motif = Motif {
            nom: d.nom.clone(),
            modele: false,
            grilles: d.lignes.iter().map(|l| GrilleMotif { angle: l.angle.to_radians(), origine: l.origine, glissement: l.glissement, decalage: l.pas, tirets: l.tirets.iter().map(|t| t.abs()).collect() }).collect(),
        };
        let segments = hachures_plan(&tris, &motif, echelle, 20_000);
        Some(Primitive { forme: Forme::Segments(segments.into_iter().map(|s| s.map(|q| pt(DVec2::from_array(q)))).collect()), style: Style { couleur: couleur(c), epaisseur: 0.0 }, role: Role::Hachure, element: None })
    };
    let mut out = Vec::new();
    match fond {
        Some((MotifCoupe::Uni, c)) if c != [255, 255, 255] => out.push(aplat(c)),
        Some((m, c)) if m != MotifCoupe::Uni => out.extend(hachures(&m, c)),
        _ => {}
    }
    match motif {
        MotifCoupe::Uni => out.push(aplat(encre)),
        MotifCoupe::Vide | MotifCoupe::Isolation => {}
        m => out.extend(hachures(&m, encre)),
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Annotation {
    pub id: String,
    pub vue: String,
    pub hauteur: f64,
    #[serde(flatten)]
    pub forme: AnnotationForme,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VuePlacee {
    pub vue: String,
    pub position: [f64; 2],
    /// Valeur historique conservée dans les fichiers de projet.
    pub echelle: f64,
}

impl VuePlacee {
    pub fn echelle_sur_feuille(&self, projet: &Projet) -> f64 {
        projet.vues.iter().find(|vue| vue.id == self.vue).map_or(self.echelle, |vue| vue.echelle)
    }
}
/// Jeu des feuilles enregistrées sans jeu.
pub const JEU_DEFAUT: &str = "Plans";
fn jeu_defaut() -> String {
    JEU_DEFAUT.into()
}

/// Feuille : format papier (mm), cartouche, vues placées ; les feuilles d'un même jeu
/// se suivent dans l'ordre du projet.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Feuille {
    pub id: String,
    pub numero: String,
    pub nom: String,
    #[serde(default = "jeu_defaut")]
    pub jeu: String,
    pub largeur: f64,
    pub hauteur: f64,
    pub cartouche: String,
    pub vues: Vec<VuePlacee>,
    /// Nomenclatures posées sur la feuille (tableaux).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nomenclatures: Vec<NomenclaturePlacee>,
}

/// Nomenclature posée sur une feuille : tableau dont le coin haut gauche est à
/// `position` (mm du papier, origine en bas à gauche).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NomenclaturePlacee {
    pub nomenclature: String,
    pub position: [f64; 2],
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Nomenclature {
    pub id: String,
    pub nom: String,
    pub categorie: String,
    pub regrouper: bool,
    /// Colonnes choisies ; vide : catégorie, type ou élément, nombre, volume net.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colonnes: Vec<Colonne>,
    /// Colonnes de paramètres (familles : type, occurrence, partagés), après les
    /// colonnes choisies ; elles distinguent les groupes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parametres: Vec<String>,
}

/// Colonne d'une nomenclature, calculée sur le modèle natif. Regroupée par type, les
/// colonnes Nombre, Longueur, Surface et Volume sont totalisées ; les autres
/// distinguent les groupes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Colonne {
    Categorie,
    Type,
    Element,
    Niveau,
    Nombre,
    Longueur,
    Hauteur,
    Largeur,
    Epaisseur,
    Surface,
    Volume,
}

impl Colonne {
    pub const TOUTES: [Colonne; 11] = [Self::Categorie, Self::Type, Self::Element, Self::Niveau, Self::Nombre, Self::Longueur, Self::Hauteur, Self::Largeur, Self::Epaisseur, Self::Surface, Self::Volume];

    pub fn titre(self) -> &'static str {
        match self {
            Self::Categorie => "Catégorie",
            Self::Type => "Type",
            Self::Element => "Élément",
            Self::Niveau => "Niveau",
            Self::Nombre => "Nombre",
            Self::Longueur => "Longueur (mm)",
            Self::Hauteur => "Hauteur (mm)",
            Self::Largeur => "Largeur (mm)",
            Self::Epaisseur => "Épaisseur (mm)",
            Self::Surface => "Surface (m²)",
            Self::Volume => "Volume net (m³)",
        }
    }

    fn totalisee(self) -> bool {
        matches!(self, Self::Nombre | Self::Longueur | Self::Surface | Self::Volume)
    }

    /// Colonne d'après le titre d'un champ de nomenclature Revit (« Famille et type »,
    /// « Niveau de base »…) ; None si le modèle natif ne la calcule pas.
    pub fn depuis_titre(titre: &str) -> Option<Colonne> {
        let t = titre.trim().to_lowercase();
        Some(match t.as_str() {
            "catégorie" => Self::Categorie,
            "type" | "famille et type" | "nom du type" | "famille" => Self::Type,
            "nom" | "repère" => Self::Element,
            "niveau" | "niveau de base" | "contrainte inférieure" | "niveau de référence" => Self::Niveau,
            "nombre" => Self::Nombre,
            "longueur" | "longueur de coupe" => Self::Longueur,
            "hauteur" | "hauteur non contrainte" => Self::Hauteur,
            "largeur" => Self::Largeur,
            "épaisseur" | "largeur du mur" | "épaisseur par défaut" => Self::Epaisseur,
            "surface" | "aire" => Self::Surface,
            "volume" => Self::Volume,
            _ => return None,
        })
    }
}

/// Élément d'une nomenclature : valeurs de ses colonnes (longueurs en mm, surface en
/// m², volume en m³).
struct LigneQuantite {
    categorie: String,
    type_nom: String,
    element: String,
    niveau: String,
    longueur: Option<f64>,
    hauteur: Option<f64>,
    largeur: Option<f64>,
    epaisseur: Option<f64>,
    surface: Option<f64>,
    volume: Option<f64>,
    /// Valeurs affichées des paramètres de famille (ouvertures).
    parametres: BTreeMap<String, String>,
}

pub fn repere(p: &Projet, v: &Vue) -> (DVec3, DVec3, DVec3) {
    match &v.genre {
        GenreVue::Plan { niveau, coupe, .. } => (
            DVec3::new(
                0.0,
                0.0,
                p.niveau(niveau).map_or(0.0, |n| n.altitude) + coupe,
            ),
            DVec3::X,
            DVec3::Y,
        ),
        GenreVue::Coupe { debut, fin, .. } => {
            let a = DVec2::from_array(*debut);
            let u = (DVec2::from_array(*fin) - a).normalize_or_zero();
            (a.extend(0.0), u.extend(0.0), DVec3::Z)
        }
        // vue caméra : plan perpendiculaire au regard, à l'œil
        GenreVue::Camera { oeil, cible, .. } => {
            let o = DVec3::from_array(*oeil);
            let d = (DVec3::from_array(*cible) - o).normalize_or(DVec3::Y);
            let u = d.cross(DVec3::Z).normalize_or(DVec3::X);
            (o, u, u.cross(d))
        }
    }
}
/// Arbres du niveau d'un plan : contour du houppier (festonné pour un feuillu, étoilé
/// pour un conifère, cercle pour un fastigié), tronc et croix au pied. Élément : l'arbre
/// (`id_natif("arbre", id)`), sélectionnable en cliquant le symbole.
pub fn primitives_arbres(p: &Projet, v: &Vue) -> Vec<Primitive> {
    use crate::elements::arbre::Essence;
    let GenreVue::Plan { niveau, .. } = &v.genre else { return Vec::new() };
    let z = p.niveau(niveau).map_or(0.0, |n| n.altitude) + 5.0;
    let mut out = Vec::new();
    for a in p.arbres.iter().filter(|a| &a.niveau == niveau && !v.elements_masques.contains(&a.id)) {
        // au pied de l'arbre (sur un talus, au-dessus du terrain)
        let c = DVec3::new(a.position[0], a.position[1], z + a.decalage.max(0.0));
        let r = a.diametre * 0.5;
        let n = 48;
        let rayon = |k: usize| {
            let t = k as f64 / n as f64;
            match a.essence {
                // festons : 8 lobes
                Essence::Feuillu => r * (0.9 + 0.1 * (t * 8.0 * std::f64::consts::TAU).cos().abs()),
                // dents : 16 pointes
                Essence::Conifere => r * if k % 3 == 0 { 1.0 } else { 0.86 },
                Essence::Fastigie => r,
            }
        };
        let point = |k: usize, rr: f64| {
            let ang = k as f64 / n as f64 * std::f64::consts::TAU;
            (c + DVec3::new(ang.cos() * rr, ang.sin() * rr, 0.0)) * 0.001
        };
        let mut segs: Vec<[DVec3; 2]> = (0..n).map(|k| [point(k, rayon(k)), point(k + 1, rayon((k + 1) % n))]).collect();
        let rt = (a.diametre * 0.035).clamp(60.0, 400.0);
        segs.extend((0..16).map(|k| [point(k * 3, rt), point(k * 3 + 3, rt)]));
        let croix = r * 0.12;
        segs.push([(c - DVec3::X * croix) * 0.001, (c + DVec3::X * croix) * 0.001]);
        segs.push([(c - DVec3::Y * croix) * 0.001, (c + DVec3::Y * croix) * 0.001]);
        out.push(Primitive {
            forme: Forme::Segments(segs),
            style: Style { couleur: [60, 110, 50, 255], epaisseur: 0.18 },
            role: Role::Annotation,
            element: Some(crate::identifiants::id_natif("arbre", &a.id)),
        });
    }
    out
}

/// Pièces du niveau d'un plan : aplat clair (rôle poché, sous les traits) sur le sol du
/// niveau, et étiquette centrée sur le point (nom, numéro, surface). Élément :
/// `id_natif("annotation", id)`, comme les annotations (sélection, surbrillance).
pub fn primitives_pieces(p: &Projet, v: &Vue) -> Vec<Primitive> {
    let GenreVue::Plan { niveau, .. } = &v.genre else { return Vec::new() };
    let (o, u, h) = repere(p, v);
    let sol = p.niveau(niveau).map_or(0.0, |n| n.altitude) + 2.0;
    let mut out = Vec::new();
    for x in p.pieces.iter().filter(|x| &x.niveau == niveau && !v.elements_masques.contains(&x.id)) {
        let element = Some(crate::identifiants::id_natif("annotation", &x.id));
        if x.fermee() {
            out.push(Primitive {
                forme: Forme::Surface(vec![x.contour.iter().map(|q| DVec3::new(q[0], q[1], sol) * 0.001).collect()]),
                style: Style { couleur: [226, 236, 248, 255], epaisseur: 0.0 },
                role: Role::Poche,
                element,
            });
        }
        // étiquette : famille de la pièce, en mm du papier autour de son point
        let famille = p.famille_etiquette(x);
        let (formes, textes) = famille.document_famille.as_ref().map_or_else(|| (famille.formes.clone(), famille.textes.clone()), |f| contenu_plan_famille(f, p));
        // mm du papier → m du modèle
        let papier = v.echelle * 0.001;
        let centre = (o + DVec3::new(x.point[0], x.point[1], 0.0)) * 0.001;
        let couleur = if x.fermee() { [30, 30, 30, 255] } else { [190, 35, 35, 255] };
        for t in &textes {
            let l = FamilleEtiquette::remplir(&t.contenu, p, x);
            if l.trim().is_empty() { continue; }
            let taille = t.hauteur * papier;
            let largeur = noyau::texte::ligne(&l, "Arial", taille, false).largeur;
            let pos = centre + (u * t.position[0] + h * t.position[1]) * papier - u * largeur / 2.0;
            let mut prim = texte(&l, pos, u, h, taille, couleur);
            prim.element = element;
            out.push(prim);
        }
        for forme in &formes {
            for mut prim in forme_cartouche(p, forme) {
                let transformer = |q: DVec3| centre + (u * q.x + h * q.y) * v.echelle;
                prim.forme = match prim.forme {
                    Forme::Polyligne { points, fermee } => Forme::Polyligne { points: points.into_iter().map(transformer).collect(), fermee },
                    Forme::Segments(segments) => Forme::Segments(segments.into_iter().map(|s| s.map(transformer)).collect()),
                    Forme::Triangles(triangles) => Forme::Triangles(triangles.into_iter().map(|t| t.map(transformer)).collect()),
                    Forme::Surface(contours) => Forme::Surface(contours.into_iter().map(|c| c.into_iter().map(transformer).collect()).collect()),
                };
                prim.element = element;
                out.push(prim);
            }
        }
        for t in famille.traits.iter().filter(|_| famille.document_famille.is_none()) {
            let [a, b] = t.map(|q| centre + (u * q[0] + h * q[1]) * papier);
            let mut prim = ligne(a, b);
            prim.style.couleur = couleur;
            prim.element = element;
            out.push(prim);
        }
    }
    out
}
pub fn texte(s: &str, pos: DVec3, u: DVec3, v: DVec3, hauteur: f64, couleur: [u8; 4]) -> Primitive {
    let t = noyau::texte::ligne(s, "Arial", hauteur, false);
    Primitive {
        forme: Forme::Triangles(
            t.triangles
                .into_iter()
                .map(|t| t.map(|p| pos + u * p[0] + v * p[1]))
                .collect(),
        ),
        style: Style {
            couleur,
            epaisseur: 0.0,
        },
        role: Role::Annotation,
        element: None,
    }
}
fn ligne(a: DVec3, b: DVec3) -> Primitive {
    Primitive {
        forme: Forme::Segments(vec![[a, b]]),
        style: Style {
            couleur: [30, 30, 30, 255],
            epaisseur: 0.18,
        },
        role: Role::Annotation,
        element: None,
    }
}
fn point_cote(p: &Projet, v: &Vue, point: &PointCote) -> Option<DVec2> {
    match point {
        PointCote::Fixe { point } => Some(DVec2::from_array(*point)),
        PointCote::Mur { id, fin } => {
            let (a, b, z) = p.axe_element(id)?;
            let pt = DVec2::from_array(if *fin { b } else { a }).extend(z);
            let (o, u, h) = repere(p, v);
            let d = pt - o;
            Some(DVec2::new(d.dot(u), d.dot(h)))
        }
        PointCote::Arete { id, arete, t } => {
            let [a, b] = cotation::arete_vue(p, v, id, *arete)?;
            Some(a + (b - a) * *t)
        }
    }
}
/// Traits d'une polyligne : segments consécutifs de même type regroupés (raccords
/// continus), tirets reportés d'un segment à l'autre.
fn primitives_polyligne(p: &Projet, points: &[[f64; 2]], fermee: bool, courbures: &[f64], traits: &[String], echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    let pts: Vec<DVec2> = points.iter().map(|q| DVec2::from_array(*q)).collect();
    // chaque segment : ses deux sommets, ou les points de son arc
    let segments: Vec<Vec<DVec2>> = noyau::forme2d::segments_polyligne(&pts, fermee, courbures).into_iter().map(|(a, b, k)| noyau::forme2d::points_segment(a, b, k)).collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < segments.len() {
        let id = traits.get(k).map_or("", |s| s.as_str());
        let mut fin = k + 1;
        while fin < segments.len() && traits.get(fin).map_or("", |s| s.as_str()) == id {
            fin += 1;
        }
        let t = p.type_trait(id);
        let style = Style { couleur: [30, 30, 30, 255], epaisseur: t.epaisseur };
        if t.tirets.is_empty() {
            let mut l: Vec<DVec3> = segments[k..fin].iter().flat_map(|s| &s[..s.len() - 1]).map(|q| pt(*q)).collect();
            let ferme = k == 0 && fin == segments.len() && fermee && pts.len() > 2;
            if !ferme {
                l.push(pt(*segments[fin - 1].last().unwrap()));
            }
            out.push(Primitive { forme: Forme::Polyligne { points: l, fermee: ferme }, style, role: Role::Annotation, element: None });
        } else {
            let mut phase = 0.0;
            let mut s: Vec<[DVec3; 2]> = Vec::new();
            for w in segments[k..fin].iter().flat_map(|s| s.windows(2)) {
                s.extend(t.decouper(w[0], w[1], echelle, &mut phase).into_iter().map(|[a, b]| [pt(a), pt(b)]));
            }
            out.push(Primitive { forme: Forme::Segments(s), style, role: Role::Annotation, element: None });
        }
        k = fin;
    }
    out
}

fn primitives_forme_dessin(p: &Projet, forme: &AnnotationForme, echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    match forme {
        AnnotationForme::Ligne { debut, fin } => vec![ligne(pt(DVec2::from_array(*debut)), pt(DVec2::from_array(*fin)))],
        AnnotationForme::Polyligne { points, fermee, courbures, remplissage, traits } => {
            let mut out = Vec::new();
            if let Some(r) = remplissage.as_ref().filter(|_| *fermee && points.len() >= 3) {
                out.extend(primitives_remplissage(p, r, &forme.contour_developpe().unwrap_or_default(), echelle, pt));
            }
            out.extend(primitives_polyligne(p, points, *fermee, courbures, traits, echelle, pt));
            out
        }
        AnnotationForme::Zone { contour } => vec![Primitive {
            forme: Forme::Surface(vec![contour.iter().map(|p| pt(DVec2::from_array(*p))).collect()]),
            style: Style { couleur: [215, 215, 210, 255], epaisseur: 0.0 },
            role: Role::Annotation,
            element: None,
        }],
        _ => forme.forme_2d().into_iter().flat_map(|f| f.polylignes()).map(|l| Primitive {
            forme: Forme::Polyligne { points: l.into_iter().map(pt).collect(), fermee: false },
            style: Style { couleur: [30, 30, 30, 255], epaisseur: 0.18 },
            role: Role::Annotation,
            element: None,
        }).collect(),
    }
}

pub fn annotations(p: &Projet, v: &Vue) -> Vec<Primitive> {
    let (o, u, h) = repere(p, v);
    let mut out = Vec::new();
    let pt = |p: DVec2| (o + u * p.x + h * p.y) * 0.001;
    for a in p.annotations.iter().filter(|a| a.vue == v.id && !v.elements_masques.contains(&a.id)) {
        let debut = out.len();
        let taille = a.hauteur * v.echelle * 0.001;
        // traits obliques des cotes : 0,6 × la hauteur du texte, en mm du modèle
        let oblique = a.hauteur * v.echelle * 0.6;
        match &a.forme {
            AnnotationForme::Texte { point, texte: s } => out.push(texte(
                &remplir_champs(s, &[("{projet}", &p.nom), ("{vue}", &v.nom)]),
                pt(DVec2::from_array(*point)),
                u,
                h,
                taille,
                [30, 30, 30, 255],
            )),
            forme @ (AnnotationForme::Cote { .. } | AnnotationForme::CoteAngulaire { .. } | AnnotationForme::CoteDiametre { .. } | AnnotationForme::Altimetrie { .. }) => {
                let extremites = match forme {
                    AnnotationForme::Cote { debut, fin, .. } => cotation::extremites_cote(p, v, debut, fin),
                    _ => None,
                };
                // angle entre arêtes : sommet et branches recalculés depuis les éléments
                let resolue = cotation::angle_resolu(p, v, forme);
                let perdue = matches!(forme, AnnotationForme::CoteAngulaire { aretes: Some(_), .. }) && resolue.is_none();
                let forme = resolue.as_ref().unwrap_or(forme);
                let unite = crate::unite::unite();
                let longueur = |x: f64| unite.afficher(x);
                let altitude = |q: [f64; 2], relatif: bool| altitude_vue(p, v, q, relatif);
                let style = StyleCote { oblique, taille: taille * 1000.0, longueur: &longueur, altitude: &altitude };
                let lie = matches!(forme, AnnotationForme::Cote { .. });
                match geometrie(forme, extremites, &style).filter(|_| (!lie || extremites.is_some()) && !perdue) {
                    Some((traits, textes)) => {
                        out.extend(traits.iter().map(|[x, y]| ligne(pt(*x), pt(*y))));
                        for (centre, dir, s) in textes {
                            let (du, dv) = (u * dir.x + h * dir.y, u * -dir.y + h * dir.x);
                            let l = noyau::texte::largeur(&s, "Arial", taille, false);
                            out.push(texte(&s, pt(centre) - du * (l / 2.0), du, dv, taille, [30, 30, 30, 255]));
                        }
                    }
                    None => out.push(texte("Cote : référence perdue", o * 0.001, u, h, taille, [190, 35, 35, 255])),
                }
            }
            AnnotationForme::Etiquette { element, point } => {
                let nom = p
                    .murs
                    .iter()
                    .find(|m| &m.id == element)
                    .map(|m| m.nom.as_str())
                    .or_else(|| {
                        p.dalles
                            .iter()
                            .find(|m| &m.id == element)
                            .map(|m| m.nom.as_str())
                    })
                    .or_else(|| {
                        p.barres
                            .iter()
                            .find(|b| &b.id == element)
                            .map(|b| b.nom.as_str())
                    })
                    .or_else(|| {
                        p.importes
                            .iter()
                            .find(|m| &m.id == element)
                            .map(|m| m.nom.as_str())
                    });
                out.push(texte(
                    nom.unwrap_or("Référence perdue"),
                    pt(DVec2::from_array(*point)),
                    u,
                    h,
                    taille,
                    if nom.is_some() {
                        [30, 30, 30, 255]
                    } else {
                        [190, 35, 35, 255]
                    },
                ));
            }
            forme => out.extend(primitives_forme_dessin(p, forme, v.echelle, &pt)),
        }
        for primitive in &mut out[debut..] {
            primitive.element = Some(crate::identifiants::id_natif("annotation", &a.id));
        }
    }
    out.extend(primitives_pieces(p, v));
    out.extend(primitives_arbres(p, v));
    if matches!(v.genre, GenreVue::Plan { .. }) {
        for g in &p.quadrillages {
            if v.elements_masques.contains(&g.id) { continue; }
            let (a, b) = (pt(DVec2::from_array(g.debut)), pt(DVec2::from_array(g.fin)));
            out.push(ligne(a, b));
            out.push(texte(
                &g.nom,
                b,
                u,
                h,
                2.5 * v.echelle * 0.001,
                [70, 70, 70, 255],
            ));
        }
    }
    if matches!(v.genre, GenreVue::Coupe { .. }) {
        out.extend(niveaux_et_limites_en_coupe(p, v));
    }
    out.extend(symboles_familles(p, v));
    out
}

fn trait_couleur(a: DVec3, b: DVec3, couleur: [u8; 4], epaisseur: f64) -> Primitive {
    Primitive {
        forme: Forme::Segments(vec![[a, b]]),
        style: Style { couleur, epaisseur },
        role: Role::Annotation,
        element: None,
    }
}

/// Coupes et élévations : un repère par niveau (trait sur toute la largeur de la vue,
/// nom et cote) et un repère vertical là où une limite de propriété coupe le plan de
/// la vue.
fn niveaux_et_limites_en_coupe(p: &Projet, v: &Vue) -> Vec<Primitive> {
    let GenreVue::Coupe { debut, fin, .. } = &v.genre else { return Vec::new() };
    let (o, u, h) = repere(p, v);
    let pt = |x: f64, z: f64| (o + u * x + h * z) * 0.001;
    let a = DVec2::from_array(*debut);
    let d = DVec2::from_array(*fin) - a;
    let longueur = d.length();
    if longueur < 1e-6 {
        return Vec::new();
    }
    let (axe_u, axe_n) = (d / longueur, (d / longueur).perp());
    let taille = 2.5 * v.echelle * 0.001;
    let mut out = Vec::new();
    for n in &p.niveaux {
        if v.elements_masques.contains(&n.id) {
            continue;
        }
        let bleu = [18, 120, 170, 255];
        out.push(trait_couleur(pt(-500.0, n.altitude), pt(longueur + 500.0, n.altitude), bleu, 0.13));
        out.push(texte(&format!("{} ({:.2} m)", n.nom, n.altitude / 1000.0), pt(longueur + 700.0, n.altitude), u, h, taille, bleu));
    }
    for l in &p.limites_propriete {
        if v.elements_masques.contains(&l.id) || l.contour.len() < 2 {
            continue;
        }
        let rouge = [190, 40, 40, 255];
        for i in 0..l.contour.len() {
            let rel = |q: [f64; 2]| {
                let r = DVec2::from_array(q) - a;
                DVec2::new(r.dot(axe_u), r.dot(axe_n))
            };
            let (b0, b1) = (rel(l.contour[i]), rel(l.contour[(i + 1) % l.contour.len()]));
            // la limite coupe le plan de la vue : repère vertical à cet endroit
            if (b0.y > 0.0) == (b1.y > 0.0) {
                continue;
            }
            let x = b0.x + (b1.x - b0.x) * (b0.y / (b0.y - b1.y));
            if !(0.0..=longueur).contains(&x) {
                continue;
            }
            out.push(trait_couleur(pt(x, l.altitude), pt(x, l.altitude + 3000.0), rouge, 0.35));
        }
    }
    out
}

/// Symboles 2D des familles d'ouvertures : en plan, ceux des ouvertures du niveau de
/// la vue ; en coupe ou élévation, ceux des ouvertures vues de face (mur parallèle à
/// la ligne de vue, dans sa profondeur et sa longueur), projetés sur le plan de vue.
fn symboles_familles(p: &Projet, v: &Vue) -> Vec<Primitive> {
    use super::famille::modelee::VueEsquisse;
    let (o, u, h) = repere(p, v);
    let polyligne = |points: Vec<DVec3>| Primitive {
        forme: Forme::Polyligne { points: points.into_iter().map(|q| q * 0.001).collect(), fermee: false },
        style: Style { couleur: [30, 30, 30, 255], epaisseur: 0.18 },
        role: Role::Annotation,
        element: None,
    };
    let remplissage_commun = |points: Vec<DVec3>, r: RemplissageContour| -> Vec<Primitive> {
        let contour: Vec<[f64; 2]> = points.iter().map(|q| [(*q-o).dot(u), (*q-o).dot(h)]).collect();
        primitives_remplissage(p, &r, &contour, v.echelle, &|q| (o + u*q.x + h*q.y) * 0.001)
    };
    let mut out = Vec::new();
    // escaliers : ligne de foulée, rond au départ, flèche à l'arrivée
    if let GenreVue::Plan { niveau, .. } = &v.genre {
        for e in p.escaliers.iter().filter(|e| &e.niveau == niveau && e.trace.len() >= 2 && !v.elements_masques.contains(&e.id)) {
            let pts: Vec<DVec3> = e.trace.iter().map(|q| DVec3::new(q[0], q[1], o.z)).collect();
            out.push(polyligne(pts.clone()));
            let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
            let d = (b - a).normalize_or_zero() * 150.0;
            let n = DVec3::new(-d.y, d.x, 0.0) * 0.4;
            out.push(polyligne(vec![b - d + n, b, b - d - n]));
            let c = pts[0];
            out.push(polyligne((0..=16).map(|k| { let t = std::f64::consts::TAU * k as f64 / 16.0; c + DVec3::new(t.cos(), t.sin(), 0.0) * 60.0 }).collect()));
        }
    }
    if let GenreVue::Plan { niveau, .. } = &v.genre {
        for c in p.composants.iter().filter(|c| &c.niveau == niveau && !v.elements_masques.contains(&c.id)) {
            for (points, r) in c.remplissages_plan(p) { out.extend(remplissage_commun(points, r)); }
            for l in c.symboles_plan(p) {
                out.push(polyligne(l.into_iter().map(|q| q.with_z(o.z)).collect()));
            }
            for (point, contenu, hauteur) in c.textes_plan(p) {
                out.push(texte(&remplir_champs(&contenu, &[("{projet}", &p.nom), ("{vue}", &v.nom)]), point.with_z(o.z) * 0.001, u, h, hauteur * v.echelle * 0.001, [30,30,30,255]));
            }
        }
    }
    for ouv in &p.ouvertures {
        if v.elements_masques.contains(&ouv.id) || v.elements_masques.contains(&ouv.mur) { continue; }
        let Ok((axe, _, niveau)) = super::volumes::solides::hote(p, &ouv.mur) else { continue };
        match &v.genre {
            GenreVue::Plan { niveau: n, .. } if *n == niveau => {
                for (points, r) in super::volumes::solides::remplissages_ouverture(p, ouv, VueEsquisse::Plan) { out.extend(remplissage_commun(points, r)); }
                for (l, m) in super::volumes::solides::regions_ouverture(p, ouv, VueEsquisse::Plan) {
                    out.extend(remplissage_commun(l.into_iter().map(|q| q.with_z(o.z)).collect(), m));
                }
                for l in super::volumes::solides::symboles_ouverture(p, ouv, VueEsquisse::Plan) {
                    out.push(polyligne(l.into_iter().map(|q| q.with_z(o.z)).collect()));
                }
                for (point, contenu, hauteur) in super::volumes::solides::textes_ouverture(p, ouv, VueEsquisse::Plan) {
                    out.push(texte(&remplir_champs(&contenu, &[("{projet}", &p.nom), ("{vue}", &v.nom)]), point.with_z(o.z) * 0.001, u, h, hauteur * v.echelle * 0.001, [30,30,30,255]));
                }
            }
            GenreVue::Coupe { debut, fin, profondeur, .. } => {
                let (c, uw) = super::volumes::solides::repere(&axe, ouv.abscisse);
                let d = DVec2::new(u.x, u.y).perp();
                let rel = c - DVec2::new(o.x, o.y);
                let longueur = DVec2::from_array(*fin).distance(DVec2::from_array(*debut));
                let profondeur_c = rel.dot(d);
                let lateral = rel.dot(DVec2::new(u.x, u.y));
                if uw.dot(DVec2::new(u.x, u.y)).abs() < 0.98 || !(-1.0..=*profondeur).contains(&profondeur_c) || !(0.0..=longueur).contains(&lateral) {
                    continue;
                }
                for (points, r) in super::volumes::solides::remplissages_ouverture(p, ouv, VueEsquisse::Elevation) { out.extend(remplissage_commun(points, r)); }
                for (l, m) in super::volumes::solides::regions_ouverture(p, ouv, VueEsquisse::Elevation) {
                    out.extend(remplissage_commun(l.into_iter().map(|q| o + u * (q - o).dot(u) + h * (q - o).dot(h)).collect(), m));
                }
                for l in super::volumes::solides::symboles_ouverture(p, ouv, VueEsquisse::Elevation) {
                    out.push(polyligne(l.into_iter().map(|q| o + u * (q - o).dot(u) + h * (q - o).dot(h)).collect()));
                }
                for (point, contenu, hauteur) in super::volumes::solides::textes_ouverture(p, ouv, VueEsquisse::Elevation) {
                    out.push(texte(&remplir_champs(&contenu, &[("{projet}", &p.nom), ("{vue}", &v.nom)]), point * 0.001, u, h, hauteur * v.echelle * 0.001, [30,30,30,255]));
                }
            }
            _ => {}
        }
    }
    out
}

/// Hachures à 45° d'un polygone (règle pair-impair), lignes espacées de `pas`.
/// Géométrie d'une cote, commune aux vues du projet et à l'éditeur de familles.
pub struct GeometrieCote {
    /// Lignes d'attache, ligne de cote et traits obliques.
    pub traits: Vec<[DVec2; 2]>,
    /// Milieu de chaque intervalle sur la ligne de cote, et sa longueur.
    pub textes: Vec<(DVec2, f64)>,
}

/// Cote (ou chaîne de cotes) sur des points alignés : la ligne de cote passe à
/// `decalage` des points, perpendiculairement au premier → dernier ; un trait oblique
/// de longueur `oblique` marque chaque point.
pub fn geometrie_cote(points: &[DVec2], decalage: f64, oblique: f64) -> GeometrieCote {
    let mut g = GeometrieCote { traits: Vec::new(), textes: Vec::new() };
    let (Some(&a), Some(&b)) = (points.first(), points.last()) else { return g };
    let d = (b - a).normalize_or_zero();
    let n = d.perp() * decalage;
    let o = (d + d.perp()).normalize_or_zero() * oblique * 0.5;
    for &p in points {
        g.traits.push([p, p + n]);
        g.traits.push([p + n - o, p + n + o]);
    }
    g.traits.push([a + n, b + n]);
    for w in points.windows(2) {
        g.textes.push(((w[0] + w[1]) / 2.0 + n, (w[1] - w[0]).dot(d).abs()));
    }
    g
}

/// Géométrie d'une cote de mesure (angle, diamètre, altimétrie), commune aux vues du
/// projet et à l'éditeur de familles : traits, et texte (position du milieu de sa ligne
/// de base, direction de la ligne de base, contenu).
pub struct GeometrieMesure {
    pub traits: Vec<[DVec2; 2]>,
    pub texte: (DVec2, DVec2, String),
}

/// Angle saillant (radians, 0..=π) entre les branches sommet → debut et sommet → fin,
/// et angle de départ et balayage (sens trigonométrique) de son arc.
pub fn angle_branches(sommet: DVec2, debut: DVec2, fin: DVec2) -> Option<(f64, f64, f64)> {
    let (a, b) = ((debut - sommet).try_normalize()?, (fin - sommet).try_normalize()?);
    let (ta, tb) = (a.to_angle(), b.to_angle());
    let balayage = (tb - ta).rem_euclid(std::f64::consts::TAU);
    Some(if balayage <= std::f64::consts::PI { (balayage, ta, balayage) } else { (std::f64::consts::TAU - balayage, tb, std::f64::consts::TAU - balayage) })
}

/// Cote angulaire : arc à `rayon` du sommet, prolongement des branches jusqu'à l'arc,
/// traits obliques aux extrémités ; texte en degrés au milieu de l'arc. `marge` : écart
/// du texte (≈ hauteur du texte), `oblique` : longueur des traits obliques.
pub fn geometrie_angle(sommet: DVec2, debut: DVec2, fin: DVec2, rayon: f64, oblique: f64, marge: f64) -> Option<GeometrieMesure> {
    let (angle, depart, balayage) = angle_branches(sommet, debut, fin)?;
    let r = rayon.abs().max(1e-6);
    let n = ((balayage.to_degrees() / 5.0).ceil() as usize).max(2);
    let arc: Vec<DVec2> = (0..=n).map(|i| sommet + DVec2::from_angle(depart + balayage * i as f64 / n as f64) * r).collect();
    let mut traits: Vec<[DVec2; 2]> = arc.windows(2).map(|w| [w[0], w[1]]).collect();
    for (p, t) in [(debut, depart), (fin, depart + balayage)].into_iter().map(|(p, t)| (p, t)) {
        let d = DVec2::from_angle(t);
        let l = (p - sommet).length();
        if l < r { traits.push([sommet + d * l, sommet + d * (r + marge * 0.4)]); }
        let q = sommet + d * r;
        let o = (d + d.perp()).normalize_or_zero() * oblique * 0.5;
        traits.push([q - o, q + o]);
    }
    let milieu = DVec2::from_angle(depart + balayage / 2.0);
    let degres = angle.to_degrees();
    let texte = if (degres - degres.round()).abs() < 0.05 { format!("{:.0}°", degres.round()) } else { format!("{degres:.1}°") };
    // texte tangent à l'arc, lisible (jamais tête en bas)
    let mut dir = milieu.perp();
    if dir.x < -1e-9 || (dir.x.abs() < 1e-9 && dir.y < 0.0) { dir = -dir; }
    Some(GeometrieMesure { traits, texte: (sommet + milieu * (r + marge * 0.4), dir, texte) })
}

/// Cote de diamètre : ligne passant par le centre, d'un bord à l'autre du cercle,
/// traits obliques aux bords, croix de centre ; texte « Ø … » sur la ligne.
pub fn geometrie_diametre(centre: DVec2, point: DVec2, oblique: f64, marge: f64) -> Option<GeometrieMesure> {
    let r = (point - centre).length();
    let d = (point - centre).try_normalize()?;
    let (a, b) = (centre - d * r, centre + d * r);
    let o = (d + d.perp()).normalize_or_zero() * oblique * 0.5;
    let c = oblique * 0.6;
    let traits = vec![[a, b], [a - o, a + o], [b - o, b + o], [centre - DVec2::X * c, centre + DVec2::X * c], [centre - DVec2::Y * c, centre + DVec2::Y * c]];
    let mut dir = d;
    if dir.x < -1e-9 || (dir.x.abs() < 1e-9 && dir.y < 0.0) { dir = -dir; }
    let texte = format!("Ø {}", crate::unite::unite().afficher(2.0 * r));
    Some(GeometrieMesure { traits, texte: (centre + d * r * 0.5 + dir.perp() * marge * 0.4, dir, texte) })
}

/// Cote altimétrique : triangle pointe sur le point visé, ligne de rappel jusqu'au
/// repère, trait horizontal sous le texte. `taille` : hauteur du texte (mm du modèle).
pub fn geometrie_altimetrie(point: DVec2, repere: DVec2, altitude_mm: f64, decimales: u8, taille: f64) -> GeometrieMesure {
    let t = taille * 0.8;
    let (g, dr, h) = (point + DVec2::new(-t * 0.6, t), point + DVec2::new(t * 0.6, t), point);
    let mut traits = vec![[g, dr], [dr, h], [h, g]];
    let texte = texte_altitude(altitude_mm, decimales);
    let largeur = noyau::texte::largeur(&texte, "Arial", taille, false);
    let sens = if repere.x >= point.x { 1.0 } else { -1.0 };
    if repere.distance(point) > 1e-6 { traits.push([point, repere]); }
    traits.push([repere, repere + DVec2::X * sens * largeur]);
    GeometrieMesure { traits, texte: (repere + DVec2::new(sens * largeur / 2.0, taille * 0.3), DVec2::X, texte) }
}

/// Altitude en mètres signée (« +2.50 », « -0.20 », « ±0.00 »).
pub fn texte_altitude(altitude_mm: f64, decimales: u8) -> String {
    let d = decimales.min(4) as usize;
    let m = altitude_mm / 1000.0;
    if format!("{:.*}", d, m.abs()).trim_start_matches(['0', '.']).is_empty() { format!("±{:.*}", d, 0.0) } else { format!("{:+.*}", d, m) }
}

/// Altitude (mm, par rapport au 0 du projet) d'un point d'une vue : niveau du plan, ou
/// hauteur du point en coupe ; `relative_niveau` : par rapport au niveau le plus haut
/// sous ce point.
pub fn altitude_vue(p: &Projet, v: &Vue, point: [f64; 2], relative_niveau: bool) -> f64 {
    let z = match &v.genre {
        GenreVue::Plan { niveau, .. } => p.niveau(niveau).map_or(0.0, |n| n.altitude),
        _ => {
            let (o, u, h) = repere(p, v);
            (o + u * point[0] + h * point[1]).z
        }
    };
    if !relative_niveau { return z; }
    let base = p.niveaux.iter().map(|n| n.altitude).filter(|a| *a <= z + 1e-6).fold(f64::NEG_INFINITY, f64::max);
    if base.is_finite() { z - base } else { z }
}

/// Formats papier courants (paysage), mm.
pub const FORMATS: [(&str, f64, f64); 5] = [("A0", 1189.0, 841.0), ("A1", 841.0, 594.0), ("A2", 594.0, 420.0), ("A3", 420.0, 297.0), ("A4", 297.0, 210.0)];

/// Marge du cadre et taille du cartouche (mm).
pub const MARGE: f64 = 10.0;

/// Géométrie réutilisable d'un cartouche, en millimètres depuis son coin inférieur gauche.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FamilleCartouche {
    /// Document complet de la famille ; les champs graphiques sont son rendu en plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_famille: Option<Box<crate::famille::modelee::FichierFamille>>,
    #[serde(default)]
    pub id: String,
    pub nom: String,
    pub largeur: f64,
    pub hauteur: f64,
    pub traits: Vec<[[f64; 2]; 2]>,
    pub textes: Vec<TexteCartouche>,
    /// Formes tracées dans le repère du papier, en millimètres.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formes: Vec<AnnotationForme>,
}

/// Famille d'étiquette de pièce : traits et textes à champs, en mm du papier autour du
/// point de la pièce (textes centrés sur leur position).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FamilleEtiquette {
    /// Document complet de la famille ; seules les annotations de plan sont rendues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_famille: Option<Box<crate::famille::modelee::FichierFamille>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formes: Vec<AnnotationForme>,
    pub id: String,
    pub nom: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traits: Vec<[[f64; 2]; 2]>,
    pub textes: Vec<TexteCartouche>,
}

/// Prépare le rendu de la seule vue en plan d'une famille documentaire.
pub(super) fn contenu_plan_famille(document: &crate::famille::modelee::FichierFamille, p: &Projet) -> (Vec<AnnotationForme>, Vec<TexteCartouche>) {
    use crate::famille::modelee::VueEsquisse;
    let r = document.famille.resoudre_occurrence(document.largeur, document.hauteur, document.profondeur, &p.parametres_partages, &Default::default());
    let mut formes = Vec::new();
    let mut textes = Vec::new();
    for e in document.famille.elements_2d_resolus(&r, VueEsquisse::Plan) {
        match e.forme {
            AnnotationForme::Texte { point, texte } => textes.push(TexteCartouche { contenu: texte, position: point, hauteur: e.hauteur }),
            forme => formes.push(forme),
        }
    }
    (formes, textes)
}

/// Résout les champs fournis par le contexte de rendu du texte.
pub fn remplir_champs(contenu: &str, champs: &[(&str, &str)]) -> String {
    champs.iter().fold(contenu.to_owned(), |texte, (cle, valeur)| texte.replace(cle, valeur))
}

/// Champs d'une étiquette de pièce.
pub const CHAMPS_ETIQUETTE: &str = "{nom}, {numero}, {surface}, {perimetre}, {niveau}";

impl FamilleEtiquette {
    /// Étiquette standard : nom et numéro, surface dessous.
    pub fn standard() -> FamilleEtiquette {
        let mut etiquette = FamilleEtiquette {
            document_famille: None,
            formes: Vec::new(),
            id: String::new(),
            nom: "Étiquette standard".into(),
            traits: Vec::new(),
            textes: vec![
                TexteCartouche { contenu: "{nom} {numero}".into(), position: [0.0, 0.75], hauteur: 2.5 },
                TexteCartouche { contenu: "{surface}".into(), position: [0.0, -3.0], hauteur: 2.5 },
            ],
        };
        use crate::famille::modelee::{CategorieFamille, Element2DFamille, FamilleModelee, FichierFamille, VueEsquisse, FORMAT_FAMILLE};
        let mut famille = FamilleModelee::vide(CategorieFamille::EtiquettePiece);
        famille.elements_2d.extend(etiquette.textes.iter().map(|t| Element2DFamille { vue: VueEsquisse::Plan, forme: AnnotationForme::Texte { point: t.position, texte: t.contenu.clone() }, hauteur: t.hauteur }));
        etiquette.document_famille = Some(Box::new(FichierFamille { format: FORMAT_FAMILLE.into(), nom: etiquette.nom.clone(), largeur: 60.0, hauteur: 30.0, profondeur: 300.0, famille }));
        etiquette
    }

    /// Texte `contenu` avec les champs de la pièce `x`.
    pub fn remplir(contenu: &str, p: &Projet, x: &crate::Piece) -> String {
        let surface = if x.fermee() { format!("{:.2} m²", x.aire() / 1e6) } else { "Non fermée".into() };
        let perimetre = if x.fermee() { format!("{:.2} m", x.perimetre() / 1e3) } else { String::new() };
        let niveau = p.niveau(&x.niveau).map_or(String::new(), |n| n.nom.clone());
        remplir_champs(contenu, &[("{nom}", &x.nom), ("{numero}", &x.numero), ("{surface}", &surface), ("{perimetre}", &perimetre), ("{niveau}", &niveau)])
    }
}

impl Projet {
    /// Type de trait d'id `id`, sinon le continu fin.
    pub fn type_trait(&self, id: &str) -> TypeTrait {
        self.types_traits.iter().find(|t| !id.is_empty() && t.id == id).cloned().unwrap_or_else(TypeTrait::standard)
    }

    /// Famille d'étiquette d'une pièce : la sienne, sinon la standard.
    pub fn famille_etiquette(&self, x: &crate::Piece) -> FamilleEtiquette {
        self.familles_etiquettes.iter().find(|f| !x.etiquette.is_empty() && f.id == x.etiquette).cloned().unwrap_or_else(FamilleEtiquette::standard)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TexteCartouche {
    /// {projet}, {feuille}, {numero}, {cartouche} ou {echelles}, ou texte libre.
    pub contenu: String,
    pub position: [f64; 2],
    pub hauteur: f64,
}

