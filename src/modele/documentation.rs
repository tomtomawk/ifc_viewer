//! Documentation native : références, annotations par vue, feuilles et nomenclatures.
use super::*;
use crate::trace2d::{Dessin, Forme, Primitive, Role, Style};
use glam::{DVec2, DVec3};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Quadrillage {
    pub id: String,
    pub nom: String,
    pub debut: [f64; 2],
    pub fin: [f64; 2],
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "ancrage", rename_all = "snake_case")]
pub enum PointCote {
    Fixe { point: [f64; 2] },
    Mur { id: String, fin: bool },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
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
    /// Dessin 2D : polyligne, spline (passant par ses points), cercle, arc par trois
    /// points (début, passage, fin) ; mm dans le plan de la vue.
    Polyligne {
        points: Vec<[f64; 2]>,
        #[serde(default)]
        fermee: bool,
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
    Etiquette {
        element: String,
        point: [f64; 2],
    },
}
impl AnnotationForme {
    /// Forme du dessin 2D (ligne, polyligne, spline, cercle, arc), None pour les autres.
    pub fn forme_2d(&self) -> Option<crate::dessin2d::Forme2D> {
        use crate::dessin2d::Forme2D as F;
        let v = |p: &[f64; 2]| DVec2::from_array(*p);
        Some(match self {
            Self::Ligne { debut, fin } => F::Ligne([v(debut), v(fin)]),
            Self::Polyligne { points, fermee } => F::Polyligne { points: points.iter().map(v).collect(), fermee: *fermee },
            Self::Spline { points, fermee } => F::Spline { points: points.iter().map(v).collect(), fermee: *fermee },
            Self::Cercle { centre, rayon } => F::Cercle { centre: v(centre), rayon: *rayon },
            Self::Arc { points } => F::Arc(points.map(|p| v(&p))),
            _ => return None,
        })
    }

    /// Annotation d'une forme du dessin 2D.
    pub fn depuis_forme_2d(f: &crate::dessin2d::Forme2D) -> AnnotationForme {
        use crate::dessin2d::Forme2D as F;
        let a = |p: &DVec2| p.to_array();
        match f {
            F::Ligne([d, e]) => Self::Ligne { debut: a(d), fin: a(e) },
            F::Polyligne { points, fermee } => Self::Polyligne { points: points.iter().map(a).collect(), fermee: *fermee },
            F::Spline { points, fermee } => Self::Spline { points: points.iter().map(a).collect(), fermee: *fermee },
            F::Cercle { centre, rayon } => Self::Cercle { centre: a(centre), rayon: *rayon },
            F::Arc(p) => Self::Arc { points: p.map(|q| q.to_array()) },
        }
    }

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
    pub echelle: f64,
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
    }
}
fn texte(s: &str, pos: DVec3, u: DVec3, v: DVec3, hauteur: f64, couleur: [u8; 4]) -> Primitive {
    let t = crate::texte::ligne(s, "Arial", hauteur, false);
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
    }
}
pub fn annotations(p: &Projet, v: &Vue) -> Vec<Primitive> {
    let (o, u, h) = repere(p, v);
    let mut out = Vec::new();
    let pt = |p: DVec2| (o + u * p.x + h * p.y) * 0.001;
    for a in p.annotations.iter().filter(|a| a.vue == v.id) {
        let debut = out.len();
        let taille = a.hauteur * v.echelle * 0.001;
        match &a.forme {
            AnnotationForme::Texte { point, texte: s } => out.push(texte(
                s,
                pt(DVec2::from_array(*point)),
                u,
                h,
                taille,
                [30, 30, 30, 255],
            )),
            AnnotationForme::Ligne { debut, fin } => out.push(ligne(
                pt(DVec2::from_array(*debut)),
                pt(DVec2::from_array(*fin)),
            )),
            AnnotationForme::Polyligne { .. } | AnnotationForme::Spline { .. } | AnnotationForme::Cercle { .. } | AnnotationForme::Arc { .. } => {
                for l in a.forme.forme_2d().map(|f| f.polylignes()).unwrap_or_default() {
                    out.push(Primitive {
                        forme: Forme::Polyligne { points: l.into_iter().map(pt).collect(), fermee: false },
                        style: Style { couleur: [30, 30, 30, 255], epaisseur: 0.18 },
                        role: Role::Annotation,
                        element: None,
                    });
                }
            }
            AnnotationForme::Zone { contour } => out.push(Primitive {
                forme: Forme::Surface(vec![
                    contour.iter().map(|p| pt(DVec2::from_array(*p))).collect(),
                ]),
                style: Style {
                    couleur: [215, 215, 210, 255],
                    epaisseur: 0.0,
                },
                role: Role::Annotation,
                element: None,
            }),
            AnnotationForme::Cote {
                debut,
                fin,
                decalage,
            } => match (point_cote(p, v, debut), point_cote(p, v, fin)) {
                (Some(a), Some(b)) => {
                    let n = (b - a).normalize_or_zero().perp() * decalage;
                    out.extend([
                        ligne(pt(a), pt(a + n)),
                        ligne(pt(b), pt(b + n)),
                        ligne(pt(a + n), pt(b + n)),
                    ]);
                    out.push(texte(
                        &crate::options::unite().afficher((b - a).length()),
                        pt((a + b) / 2.0 + n),
                        u,
                        h,
                        taille,
                        [30, 30, 30, 255],
                    ));
                }
                _ => out.push(texte(
                    "Cote : référence perdue",
                    o * 0.001,
                    u,
                    h,
                    taille,
                    [190, 35, 35, 255],
                )),
            },
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
        }
        for primitive in &mut out[debut..] {
            primitive.element = Some(crate::scene::id_natif("annotation", &a.id));
        }
    }
    if matches!(v.genre, GenreVue::Plan { .. }) {
        for g in &p.quadrillages {
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
    out.extend(symboles_familles(p, v));
    out
}

/// Symboles 2D des familles d'ouvertures : en plan, ceux des ouvertures du niveau de
/// la vue ; en coupe ou élévation, ceux des ouvertures vues de face (mur parallèle à
/// la ligne de vue, dans sa profondeur et sa longueur), projetés sur le plan de vue.
fn symboles_familles(p: &Projet, v: &Vue) -> Vec<Primitive> {
    use super::famille_modelee::VueEsquisse;
    let (o, u, h) = repere(p, v);
    let polyligne = |points: Vec<DVec3>| Primitive {
        forme: Forme::Polyligne { points: points.into_iter().map(|q| q * 0.001).collect(), fermee: false },
        style: Style { couleur: [30, 30, 30, 255], epaisseur: 0.18 },
        role: Role::Annotation,
        element: None,
    };
    let mut out = Vec::new();
    // escaliers : ligne de foulée, rond au départ, flèche à l'arrivée
    if let GenreVue::Plan { niveau, .. } = &v.genre {
        for e in p.escaliers.iter().filter(|e| &e.niveau == niveau && e.trace.len() >= 2) {
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
    for ouv in &p.ouvertures {
        let Ok((axe, _, niveau)) = super::solides::hote(p, &ouv.mur) else { continue };
        match &v.genre {
            GenreVue::Plan { niveau: n, .. } if *n == niveau => {
                for l in super::solides::symboles_ouverture(p, ouv, VueEsquisse::Plan) {
                    out.push(polyligne(l.into_iter().map(|q| q.with_z(o.z)).collect()));
                }
            }
            GenreVue::Coupe { debut, fin, profondeur, .. } => {
                let (c, uw) = super::solides::repere(&axe, ouv.abscisse);
                let d = DVec2::new(u.x, u.y).perp();
                let rel = c - DVec2::new(o.x, o.y);
                let longueur = DVec2::from_array(*fin).distance(DVec2::from_array(*debut));
                let profondeur_c = rel.dot(d);
                let lateral = rel.dot(DVec2::new(u.x, u.y));
                if uw.dot(DVec2::new(u.x, u.y)).abs() < 0.98 || !(-1.0..=*profondeur).contains(&profondeur_c) || !(0.0..=longueur).contains(&lateral) {
                    continue;
                }
                for l in super::solides::symboles_ouverture(p, ouv, VueEsquisse::Elevation) {
                    out.push(polyligne(l.into_iter().map(|q| o + u * (q - o).dot(u) + h * (q - o).dot(h)).collect()));
                }
            }
            _ => {}
        }
    }
    out
}

pub fn dessin_vue(p: &Projet, id: &str) -> Result<Dessin, String> {
    let v = p.vues.iter().find(|v| v.id == id).ok_or("Vue absente.")?;
    let s = crate::scene::depuis_natif(p, std::path::Path::new("projet.modele.json"))?;
    let (o, u, h) = repere(p, v);
    let oeil = u.cross(h);
    let normale = -oeil;
    let plan = normale.extend(-normale.dot(o * 0.001 - s.origin)).as_vec4();
    let elevation = matches!(
        v.genre,
        GenreVue::Coupe {
            elevation: true,
            ..
        }
    );
    let mut visible = vec![true; s.objects.len()];
    if let GenreVue::Coupe {
        debut,
        fin,
        profondeur,
        ..
    } = &v.genre
    {
        let a = DVec2::from_array(*debut);
        let d = DVec2::from_array(*fin) - a;
        let u = d.normalize();
        let n = u.perp();
        for (i, obj) in s.objects.iter().enumerate() {
            let corners = (0..8).map(|k| {
                DVec3::new(
                    if k & 1 == 0 { obj.min.x } else { obj.max.x } as f64,
                    if k & 2 == 0 { obj.min.y } else { obj.max.y } as f64,
                    if k & 4 == 0 { obj.min.z } else { obj.max.z } as f64,
                )
            });
            let points: Vec<_> = corners
                .map(|q| (q + s.origin).truncate() * 1000.0 - a)
                .collect();
            visible[i] = points.iter().any(|q| q.dot(u) >= 0.0)
                && points.iter().any(|q| q.dot(u) <= d.length())
                && points.iter().any(|q| q.dot(n) >= 0.0)
                && points.iter().any(|q| q.dot(n) <= *profondeur);
        }
    }
    let mut primitives = if elevation {
        Vec::new()
    } else {
        crate::dessin::coupe(&s, &visible, plan, v.echelle, crate::dessin::Poche::Hachure)
    };
    primitives.extend(crate::dessin::vu(
        &s,
        &visible,
        u,
        h,
        oeil,
        Some(plan),
        4096,
    ));
    // Revenir au repère absolu, comme les annotations.
    let mut primitives: Vec<_> = primitives
        .into_iter()
        .map(|pr| pr.translater(s.origin))
        .collect();
    primitives.extend(annotations(p, v));
    Ok(Dessin {
        droite: u,
        haut: h,
        echelle: v.echelle,
        primitives,
    })
}

/// Jeux de feuilles du projet, dans l'ordre de leur première feuille.
pub fn jeux(p: &Projet) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for f in &p.feuilles {
        if !v.contains(&f.jeu) {
            v.push(f.jeu.clone());
        }
    }
    v
}

/// Déplace la feuille `id` dans le jeu `jeu`, juste avant la feuille `avant` ; sans
/// `avant` (ou s'il n'est pas dans ce jeu), après la dernière feuille du jeu.
pub fn deplacer_feuille(p: &mut Projet, id: &str, jeu: &str, avant: Option<&str>) -> Result<(), String> {
    let k = p.feuilles.iter().position(|f| f.id == id).ok_or("Feuille absente.")?;
    let mut f = p.feuilles.remove(k);
    f.jeu = jeu.into();
    let rang = avant
        .filter(|a| *a != id)
        .and_then(|a| p.feuilles.iter().position(|g| g.id == a && g.jeu == jeu))
        .or_else(|| p.feuilles.iter().rposition(|g| g.jeu == jeu).map(|r| r + 1))
        .unwrap_or(p.feuilles.len());
    p.feuilles.insert(rang, f);
    Ok(())
}

/// Formats papier courants (paysage), mm.
pub const FORMATS: [(&str, f64, f64); 5] = [("A0", 1189.0, 841.0), ("A1", 841.0, 594.0), ("A2", 594.0, 420.0), ("A3", 420.0, 297.0), ("A4", 297.0, 210.0)];

/// Marge du cadre et taille du cartouche (mm).
pub const MARGE: f64 = 10.0;
pub const CARTOUCHE: [f64; 2] = [180.0, 40.0];

fn pt_papier(x: f64, y: f64) -> DVec3 {
    DVec3::new(x / 1000.0, y / 1000.0, 0.0)
}

fn trait_papier(points: Vec<DVec3>, fermee: bool, epaisseur: f64) -> Primitive {
    Primitive { forme: Forme::Polyligne { points, fermee }, style: Style { couleur: [20, 20, 20, 255], epaisseur }, role: Role::Annotation, element: None }
}

/// Cadre et cartouche d'une feuille, en mètres du papier (origine en bas à gauche) :
/// bord, cadre intérieur, cartouche en bas à droite (projet, cartouche libre, titre,
/// numéro, échelles des vues placées).
pub fn cadre_feuille(p: &Projet, f: &Feuille) -> Vec<Primitive> {
    let (l, h, m) = (f.largeur, f.hauteur, MARGE);
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64, e: f64| trait_papier(vec![pt_papier(x0, y0), pt_papier(x1, y0), pt_papier(x1, y1), pt_papier(x0, y1)], true, e);
    let mut v = vec![rect(0.0, 0.0, l, h, 0.0), rect(m, m, l - m, h - m, 0.5)];
    let [cl, ch] = CARTOUCHE;
    let (x0, y0, x1, y1) = (l - m - cl, m, l - m, m + ch);
    if x0 > m && y1 < h - m {
        v.push(rect(x0, y0, x1, y1, 0.35));
        let xn = x1 - 40.0;
        v.push(trait_papier(vec![pt_papier(xn, y0), pt_papier(xn, y1)], false, 0.25));
        v.push(trait_papier(vec![pt_papier(x0, y0 + 14.0), pt_papier(xn, y0 + 14.0)], false, 0.18));
        v.push(trait_papier(vec![pt_papier(x0, y1 - 10.0), pt_papier(xn, y1 - 10.0)], false, 0.18));
        let noir = [20, 20, 20, 255];
        v.push(texte(&p.nom, pt_papier(x0 + 3.0, y1 - 7.0), DVec3::X, DVec3::Y, 0.0035, noir));
        v.push(texte(&f.nom, pt_papier(x0 + 3.0, y0 + 20.0), DVec3::X, DVec3::Y, 0.005, noir));
        if !f.cartouche.is_empty() {
            v.push(texte(&f.cartouche, pt_papier(x0 + 3.0, y0 + 9.0), DVec3::X, DVec3::Y, 0.0025, noir));
        }
        let mut echelles: Vec<String> = f.vues.iter().map(|x| format!("1:{}", x.echelle)).collect();
        echelles.dedup();
        if !echelles.is_empty() {
            v.push(texte(&format!("Échelle {}", echelles.join(", ")), pt_papier(x0 + 3.0, y0 + 3.5), DVec3::X, DVec3::Y, 0.0025, noir));
        }
        v.push(texte(&f.numero, pt_papier(xn + 4.0, y0 + 15.0), DVec3::X, DVec3::Y, 0.009, noir));
    }
    v
}

/// Vue placée sur une feuille : dessin de la vue à l'échelle de la feuille, en mètres du
/// papier (origine en bas à gauche), suivi de son titre (nom, échelle) souligné ; et
/// l'étendue du dessin seul, en mm du papier [x0, y0, x1, y1].
pub fn vue_placee(p: &Projet, v: &VuePlacee) -> Result<(Vec<Primitive>, [f64; 4]), String> {
    let mut projet = p.clone();
    if let Some(vue) = projet.vues.iter_mut().find(|x| x.id == v.vue) {
        vue.echelle = v.echelle;
    }
    let dessin = dessin_vue(&projet, &v.vue)?;
    let vue = p.vues.iter().find(|x| x.id == v.vue).ok_or("Vue absente.")?;
    let (origine, _, _) = repere(p, vue);
    let proj = |q: DVec3| {
        let q = q - origine * 0.001;
        pt_papier(v.position[0] + q.dot(dessin.droite) * 1000.0 / v.echelle, v.position[1] + q.dot(dessin.haut) * 1000.0 / v.echelle)
    };
    let mut primitives = Vec::new();
    let mut e = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for mut pr in dessin.primitives {
        pr.forme = match pr.forme {
            Forme::Polyligne { points, fermee } => Forme::Polyligne { points: points.into_iter().map(proj).collect(), fermee },
            Forme::Segments(s) => Forme::Segments(s.into_iter().map(|s| s.map(proj)).collect()),
            Forme::Triangles(t) => Forme::Triangles(t.into_iter().map(|t| t.map(proj)).collect()),
            Forme::Surface(c) => Forme::Surface(c.into_iter().map(|c| c.into_iter().map(proj).collect()).collect()),
        };
        let points: Box<dyn Iterator<Item = &DVec3> + '_> = match &pr.forme {
            Forme::Polyligne { points, .. } => Box::new(points.iter()),
            Forme::Segments(s) => Box::new(s.iter().flatten()),
            Forme::Triangles(t) => Box::new(t.iter().flatten()),
            Forme::Surface(c) => Box::new(c.iter().flatten()),
        };
        for q in points {
            e = [e[0].min(q.x * 1000.0), e[1].min(q.y * 1000.0), e[2].max(q.x * 1000.0), e[3].max(q.y * 1000.0)];
        }
        primitives.push(pr);
    }
    if e[0] > e[2] {
        e = [v.position[0], v.position[1], v.position[0] + 10.0, v.position[1] + 10.0];
    }
    // titre sous le dessin : nom, échelle, trait
    let y = e[1] - 8.0;
    let noir = [20, 20, 20, 255];
    primitives.push(texte(&vue.nom, pt_papier(e[0], y), DVec3::X, DVec3::Y, 0.004, noir));
    primitives.push(texte(&format!("1:{}", v.echelle), pt_papier(e[0], y - 5.0), DVec3::X, DVec3::Y, 0.0025, noir));
    primitives.push(trait_papier(vec![pt_papier(e[0], y - 1.2), pt_papier(e[0] + 60.0_f64.min(e[2] - e[0]).max(30.0), y - 1.2)], false, 0.35));
    Ok((primitives, e))
}

/// Tableau d'une nomenclature posée sur une feuille, en mètres du papier : titre
/// (nom), ligne d'en-tête, lignes ; colonnes à la largeur de leur plus long texte. Et
/// son étendue en mm du papier [x0, y0, x1, y1].
pub fn nomenclature_placee(p: &Projet, n: &NomenclaturePlacee) -> Result<(Vec<Primitive>, [f64; 4]), String> {
    let nom = p.nomenclatures.iter().find(|x| x.id == n.nomenclature).ok_or("Nomenclature absente.")?;
    let lignes = tableau(p, nom)?;
    // mm du papier : texte 2,5, en-tête et titre plus grands
    let (texte_h, ligne_h, marge, titre_h) = (2.5, 6.0, 2.0, 3.5);
    let largeur = |s: &str, h: f64| crate::texte::ligne(s, "Arial", h, false).largeur;
    let ncol = lignes.iter().map(Vec::len).max().unwrap_or(0);
    let mut colonnes = vec![12.0_f64; ncol];
    for l in &lignes {
        for (i, c) in l.iter().enumerate() {
            colonnes[i] = colonnes[i].max(largeur(c, texte_h) + 2.0 * marge);
        }
    }
    let total: f64 = colonnes.iter().sum::<f64>().max(largeur(&nom.nom, titre_h) + 2.0 * marge);
    let [x0, y0] = n.position;
    let noir = [20, 20, 20, 255];
    let mut prims = Vec::new();
    prims.push(texte(&nom.nom, pt_papier(x0 + marge, y0 - titre_h - 1.5), DVec3::X, DVec3::Y, titre_h / 1000.0, noir));
    let haut = y0 - titre_h - 4.0;
    let bas = haut - ligne_h * lignes.len() as f64;
    // cadre, lignes, colonnes ; en-tête souligné plus fort
    prims.push(trait_papier(vec![pt_papier(x0, haut), pt_papier(x0 + total, haut), pt_papier(x0 + total, bas), pt_papier(x0, bas)], true, 0.35));
    for k in 1..lignes.len() {
        let y = haut - ligne_h * k as f64;
        prims.push(trait_papier(vec![pt_papier(x0, y), pt_papier(x0 + total, y)], false, if k == 1 { 0.35 } else { 0.13 }));
    }
    let mut x = x0;
    for w in colonnes.iter().take(ncol.saturating_sub(1)) {
        x += w;
        prims.push(trait_papier(vec![pt_papier(x, haut), pt_papier(x, bas)], false, 0.13));
    }
    for (k, l) in lignes.iter().enumerate() {
        let y = haut - ligne_h * (k as f64 + 1.0) + (ligne_h - texte_h) * 0.5;
        let mut x = x0;
        for (i, c) in l.iter().enumerate() {
            if !c.is_empty() {
                prims.push(texte(c, pt_papier(x + marge, y), DVec3::X, DVec3::Y, texte_h / 1000.0, noir));
            }
            x += colonnes[i];
        }
    }
    Ok((prims, [x0, bas, x0 + total, y0]))
}

pub fn feuille(p: &Projet, f: &Feuille) -> Result<Dessin, String> {
    let mut primitives = cadre_feuille(p, f);
    for v in &f.vues {
        primitives.extend(vue_placee(p, v)?.0);
    }
    for n in &f.nomenclatures {
        primitives.extend(nomenclature_placee(p, n)?.0);
    }
    Ok(Dessin { droite: DVec3::X, haut: DVec3::Y, echelle: 1.0, primitives })
}

/// Quantités nettes : murs percés, dalles avec trémies. Pas de double compte des assemblages.
pub fn tableau(p: &Projet, n: &Nomenclature) -> Result<Vec<Vec<String>>, String> {
    if !n.colonnes.is_empty() || !n.parametres.is_empty() {
        return tableau_colonnes(p, n);
    }
    let mut groupes: BTreeMap<(String, String), (usize, f64)> = BTreeMap::new();
    let jonctions = jonctions::coupes(p);
    let mut ajouter = |cat: &str, nom: String, v: f64| {
        if !n.categorie.is_empty() && !cat.to_lowercase().contains(&n.categorie.to_lowercase()) {
            return;
        }
        let e = groupes.entry((cat.into(), nom)).or_default();
        e.0 += 1;
        e.1 += v;
    };
    for m in &p.murs {
        ajouter(
            "Murs",
            if n.regrouper {
                p.type_mur(&m.type_mur).unwrap().nom.clone()
            } else {
                format!("{} ({})", m.nom, m.id)
            },
            geometrie::volume(&solides::mur(p, m, &jonctions)?) / 1e9,
        );
    }
    for d in &p.dalles {
        ajouter(
            d.sorte.nom(),
            if n.regrouper {
                p.type_dalle(&d.type_dalle).unwrap().nom.clone()
            } else {
                format!("{} ({})", d.nom, d.id)
            },
            p.volume_dalle(d).unwrap_or(0.0) / 1e9,
        );
    }
    for b in &p.barres {
        let categorie = match b.genre {
            GenreBarre::Poteau => "Poteaux",
            GenreBarre::Poutre => "Poutres",
        };
        ajouter(
            categorie,
            if n.regrouper {
                b.nom.clone()
            } else {
                format!("{} ({})", b.nom, b.id)
            },
            geometrie::volume(&solides::barre(p, b)?) / 1e9,
        );
    }
    for o in &p.ouvertures {
        ajouter(
            match o.genre {
                GenreOuverture::Porte => "Portes",
                GenreOuverture::Fenetre => "Fenêtres",
                GenreOuverture::Libre => "Ouvertures libres",
            },
            o.nom.clone(),
            0.0,
        );
    }
    for e in &p.importes {
        ajouter(
            &e.categorie,
            if n.regrouper {
                e.nom.clone()
            } else {
                format!("{} ({})", e.nom, e.id)
            },
            geometrie::volume(&solides::maillage(&e.maillage)?) / 1e9,
        );
    }
    let mut lignes = vec![vec![
        "Catégorie".into(),
        "Type / élément".into(),
        "Nombre".into(),
        "Volume net (m³)".into(),
    ]];
    for ((cat, nom), (nb, v)) in groupes {
        lignes.push(vec![cat, nom, nb.to_string(), format!("{v:.6}")]);
    }
    Ok(lignes)
}
pub fn csv(lignes: &[Vec<String>]) -> String {
    lignes
        .iter()
        .map(|l| {
            l.iter()
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}

/// Feuille au format papier exact, sans la marge automatique des exports de vues.
pub fn svg_feuille(p: &Projet, f: &Feuille) -> Result<String, String> {
    let mut svg = feuille(p, f)?.svg(&format!("{} — {}", f.numero, f.nom));
    let a = svg.find("<svg ").ok_or("SVG absent")?;
    let b = a + svg[a..].find('>').ok_or("SVG invalide")? + 1;
    svg.replace_range(a..b,&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 -{} {} {}\">",f.largeur,f.hauteur,f.hauteur,f.largeur,f.hauteur));
    Ok(svg)
}
pub fn pdf(svg: &str) -> Result<Vec<u8>, String> {
    let tree = svg2pdf::usvg::Tree::from_str(svg, &svg2pdf::usvg::Options::default())
        .map_err(|e| format!("SVG : {e}"))?;
    // usvg interprète les mm avec 96 px/pouce ; conserver la taille physique du papier.
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions { dpi: 96.0 },
    )
    .map_err(|e| format!("PDF : {e}"))
}

/// Tableau d'une nomenclature à colonnes choisies : une ligne par élément, ou par
/// groupe (valeurs des colonnes non totalisées) si elle regroupe.
fn tableau_colonnes(p: &Projet, n: &Nomenclature) -> Result<Vec<Vec<String>>, String> {
    let jonctions = jonctions::coupes(p);
    let nom_niveau = |id: &str| p.niveau(id).map_or(String::new(), |x| x.nom.clone());
    let mut lignes: Vec<LigneQuantite> = Vec::new();
    for m in &p.murs {
        let t = p.type_mur(&m.type_mur);
        let (l, h) = (m.axe.longueur(), p.bas_haut(m).map(|(a, b)| b - a));
        lignes.push(LigneQuantite {
            categorie: "Murs".into(),
            type_nom: t.map_or(String::new(), |t| t.nom.clone()),
            element: m.nom.clone(),
            niveau: nom_niveau(&m.niveau),
            longueur: Some(l),
            hauteur: h,
            largeur: None,
            epaisseur: t.map(|t| t.epaisseur()),
            surface: h.map(|h| l * h / 1e6),
            volume: Some(geometrie::volume(&solides::mur(p, m, &jonctions)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for d in &p.dalles {
        let t = p.type_dalle(&d.type_dalle);
        lignes.push(LigneQuantite {
            categorie: d.sorte.nom().into(),
            type_nom: t.map_or(String::new(), |t| t.nom.clone()),
            element: d.nom.clone(),
            niveau: nom_niveau(&d.niveau),
            longueur: None,
            hauteur: None,
            largeur: None,
            epaisseur: t.map(|t| t.epaisseur()),
            surface: Some(geometrie::aire_pair_impair(&d.contours) / 1e6),
            volume: p.volume_dalle(d).map(|v| v / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for b in &p.barres {
        let l = DVec3::from_array(b.fin).distance(DVec3::from_array(b.debut));
        lignes.push(LigneQuantite {
            categorie: match b.genre {
                GenreBarre::Poteau => "Poteaux",
                GenreBarre::Poutre => "Poutres",
            }
            .into(),
            type_nom: b.nom.clone(),
            element: format!("{} ({})", b.nom, b.id),
            niveau: nom_niveau(&b.niveau),
            longueur: Some(l),
            hauteur: Some(b.hauteur),
            largeur: Some(b.largeur),
            epaisseur: None,
            surface: None,
            volume: Some(geometrie::volume(&solides::barre(p, b)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for o in &p.ouvertures {
        // niveau du mur hôte (ou de la première couche de son assemblage)
        let hote = p.murs.iter().find(|m| m.id == o.mur || m.assemblage.as_deref() == Some(o.mur.as_str()));
        lignes.push(LigneQuantite {
            categorie: match o.genre {
                GenreOuverture::Porte => "Portes",
                GenreOuverture::Fenetre => "Fenêtres",
                GenreOuverture::Libre => "Ouvertures libres",
            }
            .into(),
            type_nom: o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t)).map_or(o.nom.clone(), |t| t.nom.clone()),
            element: o.nom.clone(),
            niveau: hote.map_or(String::new(), |m| nom_niveau(&m.niveau)),
            longueur: None,
            hauteur: Some(o.hauteur),
            largeur: Some(o.largeur),
            epaisseur: None,
            surface: Some(o.largeur * o.hauteur / 1e6),
            volume: None,
            parametres: solides::parametres_ouverture(p, o),
        });
    }
    for e in &p.escaliers {
        let c = e.calcul(p).ok();
        lignes.push(LigneQuantite {
            categorie: "Escaliers".into(),
            type_nom: e.nom.clone(),
            element: e.nom.clone(),
            niveau: nom_niveau(&e.niveau),
            longueur: c.map(|c| c.volees),
            hauteur: c.map(|c| c.hauteur),
            largeur: Some(e.largeur),
            epaisseur: None,
            surface: None,
            volume: Some(geometrie::volume(&e.solide(p)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    for g in &p.garde_corps {
        lignes.push(LigneQuantite {
            categorie: "Garde-corps".into(),
            type_nom: format!("Garde-corps {} · {}", g.remplissage.nom(), crate::options::unite().afficher(g.hauteur)),
            element: g.nom.clone(),
            niveau: nom_niveau(&g.niveau),
            longueur: Some(g.longueur()),
            hauteur: Some(g.hauteur),
            largeur: None,
            epaisseur: None,
            surface: None,
            volume: None,
            parametres: BTreeMap::new(),
        });
    }
    for e in &p.importes {
        lignes.push(LigneQuantite {
            categorie: e.categorie.clone(),
            type_nom: e.nom.clone(),
            element: format!("{} ({})", e.nom, e.id),
            niveau: e.niveau.as_deref().map_or(String::new(), nom_niveau),
            longueur: None,
            hauteur: None,
            largeur: None,
            epaisseur: None,
            surface: None,
            volume: Some(geometrie::volume(&solides::maillage(&e.maillage)?) / 1e9),
            parametres: BTreeMap::new(),
        });
    }
    let filtre = n.categorie.to_lowercase();
    lignes.retain(|l| filtre.is_empty() || l.categorie.to_lowercase().contains(&filtre));
    // valeur affichée d'une colonne non totalisée, et valeur d'une colonne totalisée
    let texte = |l: &LigneQuantite, c: Colonne| -> String {
        let mm = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:.0}"));
        match c {
            Colonne::Categorie => l.categorie.clone(),
            Colonne::Type => l.type_nom.clone(),
            Colonne::Element => l.element.clone(),
            Colonne::Niveau => l.niveau.clone(),
            Colonne::Hauteur => mm(l.hauteur),
            Colonne::Largeur => mm(l.largeur),
            Colonne::Epaisseur => mm(l.epaisseur),
            _ => String::new(),
        }
    };
    let somme = |l: &LigneQuantite, c: Colonne| -> Option<f64> {
        match c {
            Colonne::Nombre => Some(1.0),
            Colonne::Longueur => l.longueur,
            Colonne::Surface => l.surface,
            Colonne::Volume => l.volume,
            _ => None,
        }
    };
    let format_somme = |c: Colonne, v: Option<f64>| match (c, v) {
        (_, None) => String::new(),
        (Colonne::Nombre, Some(v)) => format!("{v:.0}"),
        (Colonne::Longueur, Some(v)) => format!("{v:.0}"),
        (Colonne::Surface, Some(v)) => format!("{v:.2}"),
        (_, Some(v)) => format!("{v:.3}"),
    };
    // sans colonnes choisies : catégorie, type ou élément, nombre, volume
    let defaut = [Colonne::Categorie, if n.regrouper { Colonne::Type } else { Colonne::Element }, Colonne::Nombre, Colonne::Volume];
    let cols: &[Colonne] = if n.colonnes.is_empty() { &defaut } else { &n.colonnes };
    let valeur_parametre = |l: &LigneQuantite, nom: &String| l.parametres.get(nom).cloned().unwrap_or_default();
    // regroupement : clé = valeurs des colonnes non totalisées (et l'élément si on ne
    // regroupe pas)
    let mut groupes: BTreeMap<Vec<String>, Vec<Option<f64>>> = BTreeMap::new();
    for (k, l) in lignes.iter().enumerate() {
        let mut cle: Vec<String> = cols.iter().filter(|c| !c.totalisee()).map(|&c| texte(l, c)).collect();
        cle.extend(n.parametres.iter().map(|nom| valeur_parametre(l, nom)));
        if !n.regrouper {
            cle.push(format!("{:08}", k));
        }
        let e = groupes.entry(cle).or_insert_with(|| vec![None; cols.len()]);
        for (i, &c) in cols.iter().enumerate() {
            if let Some(v) = somme(l, c) {
                e[i] = Some(e[i].unwrap_or(0.0) + v);
            }
        }
    }
    let mut sortie = vec![cols.iter().map(|c| c.titre().to_string()).chain(n.parametres.iter().cloned()).collect::<Vec<_>>()];
    for (cle, sommes) in groupes {
        let mut fixes = cle.into_iter();
        let mut ligne: Vec<String> = cols.iter().enumerate().map(|(i, &c)| if c.totalisee() { format_somme(c, sommes[i]) } else { fixes.next().unwrap_or_default() }).collect();
        // les valeurs des paramètres suivent les colonnes fixes dans la clé
        ligne.extend(fixes.take(n.parametres.len()));
        sortie.push(ligne);
    }
    Ok(sortie)
}

/// PDF de plusieurs pages, une par SVG (chacune à son format, comme `pdf`).
pub fn pdf_pages(svgs: &[String]) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
    if svgs.is_empty() {
        return Err("Aucune page à exporter.".into());
    }
    let mut alloc = Ref::new(1);
    let (catalogue, arbre) = (alloc.bump(), alloc.bump());
    let mut pages = Vec::new();
    let mut morceaux = Vec::new();
    for svg in svgs {
        let tree = svg2pdf::usvg::Tree::from_str(svg, &svg2pdf::usvg::Options::default()).map_err(|e| format!("SVG : {e}"))?;
        let (morceau, x) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default()).map_err(|e| format!("PDF : {e}"))?;
        let mut renum = std::collections::HashMap::new();
        let morceau = morceau.renumber(|ancien| *renum.entry(ancien).or_insert_with(|| alloc.bump()));
        let x = *renum.get(&x).ok_or("PDF : objet de page absent")?;
        // usvg lit les mm à 96 px/pouce ; points PDF à 72 par pouce
        let (l, h) = (tree.size().width() * 0.75, tree.size().height() * 0.75);
        pages.push((alloc.bump(), alloc.bump(), x, l, h));
        morceaux.push(morceau);
    }
    let mut pdf = Pdf::new();
    pdf.catalog(catalogue).pages(arbre);
    pdf.pages(arbre).kids(pages.iter().map(|p| p.0)).count(pages.len() as i32);
    for &(page, contenu, x, l, h) in &pages {
        let mut pg = pdf.page(page);
        pg.media_box(Rect::new(0.0, 0.0, l, h));
        pg.parent(arbre);
        pg.contents(contenu);
        let mut ressources = pg.resources();
        ressources.x_objects().pair(Name(b"F"), x);
        ressources.finish();
        pg.finish();
        let mut c = Content::new();
        c.transform([l, 0.0, 0.0, h, 0.0, 0.0]).x_object(Name(b"F"));
        pdf.stream(contenu, &c.finish());
    }
    for m in &morceaux {
        pdf.extend(m);
    }
    Ok(pdf.finish())
}

/// PDF d'un jeu de feuilles : ses feuilles dans l'ordre, une page chacune.
pub fn pdf_jeu(p: &Projet, jeu: &str) -> Result<Vec<u8>, String> {
    let svgs = p.feuilles.iter().filter(|f| f.jeu == jeu).map(|f| svg_feuille(p, f)).collect::<Result<Vec<_>, _>>()?;
    pdf_pages(&svgs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_compatibles_v0_references_et_pdf() {
        let (mut p, _) =
            depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
                .unwrap();
        let vue = p
            .ajouter_vue_plan(&p.niveaux[0].id.clone(), 1200.0)
            .unwrap();
        p.annotations.push(Annotation {
            id: "annotation-test".into(),
            vue: vue.clone(),
            hauteur: 2.5,
            forme: AnnotationForme::Cote {
                debut: PointCote::Mur {
                    id: "supprime".into(),
                    fin: false,
                },
                fin: PointCote::Fixe {
                    point: [1000.0, 0.0],
                },
                decalage: 500.0,
            },
        });
        let f = Feuille {
            id: "f".into(),
            numero: "A01".into(),
            nom: "Plan".into(),
            jeu: JEU_DEFAUT.into(),
            largeur: 420.0,
            hauteur: 297.0,
            cartouche: "Agence & associés".into(),
            vues: vec![VuePlacee {
                vue,
                position: [30.0, 80.0],
                echelle: 100.0,
            }],
            nomenclatures: Vec::new(),
        };
        p.feuilles.push(f.clone());
        p.verifier().unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.feuilles.len(), 1);
        let svg = svg_feuille(&p, &f).unwrap();
        assert!(svg.contains("width=\"420mm\""));
        assert!(svg.contains("height=\"297mm\""));
        let pdf = pdf(&svg).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        // jeu de deux feuilles (A3 et A4 portrait) : un PDF de deux pages à leurs formats
        let mut g = f.clone();
        g.id = "g".into();
        g.numero = "A02".into();
        (g.largeur, g.hauteur) = (210.0, 297.0);
        p.feuilles.push(g);
        let jeu = pdf_jeu(&p, JEU_DEFAUT).unwrap();
        let texte = String::from_utf8_lossy(&jeu);
        assert!(jeu.starts_with(b"%PDF-"));
        assert!(texte.contains("/Count 2"), "pages");
        assert_eq!(texte.matches("/MediaBox").count(), 2);
        assert!(texte.contains("1190.55") && texte.contains("595.27"), "formats A3 et A4 en points");
        p.feuilles.truncate(1);
        p.supprimer_vue(&f.vues[0].vue).unwrap();
        assert!(p.annotations.is_empty());
        assert!(p.feuilles[0].vues.is_empty());
    }
    #[test]
    fn feuilles_par_jeu_et_anciens_fichiers() {
        // feuille enregistrée avant les jeux : jeu par défaut
        let f: Feuille = serde_json::from_str(r#"{"id":"f","numero":"A01","nom":"Plan","largeur":420,"hauteur":297,"cartouche":"","vues":[]}"#).unwrap();
        assert_eq!(f.jeu, JEU_DEFAUT);
        let mut p = Projet::nouveau("J");
        let mut g = f.clone();
        g.id = "g".into();
        g.jeu = "Détails".into();
        let mut h = f.clone();
        h.id = "h".into();
        p.feuilles = vec![f, g, h];
        assert_eq!(jeux(&p), vec![JEU_DEFAUT.to_string(), "Détails".to_string()]);
        // réordonner : h avant f, puis f à la fin du jeu Détails
        deplacer_feuille(&mut p, "h", JEU_DEFAUT, Some("f")).unwrap();
        let ordre = |p: &Projet| p.feuilles.iter().map(|f| format!("{}:{}", f.id, f.jeu)).collect::<Vec<_>>().join(" ");
        assert_eq!(ordre(&p), "h:Plans f:Plans g:Détails");
        deplacer_feuille(&mut p, "f", "Détails", None).unwrap();
        assert_eq!(ordre(&p), "h:Plans g:Détails f:Détails");
        deplacer_feuille(&mut p, "f", "Détails", Some("g")).unwrap();
        assert_eq!(ordre(&p), "h:Plans f:Détails g:Détails");
        assert!(deplacer_feuille(&mut p, "x", "Plans", None).is_err());
        // cadre et cartouche dans la feuille
        let cadre = cadre_feuille(&p, &p.feuilles[0]);
        assert!(cadre.len() > 4);
    }

    #[test]
    fn nomenclature_parametres_de_famille() {
        use crate::modele::famille_modelee::FamilleModelee;
        let mut p = Projet::nouveau("Familles");
        p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        p.types_murs.push(TypeCompose { id: "t".into(), nom: "Mur".into(), couches: vec![Couche { epaisseur: 200.0, ..Default::default() }], couches_detaillees: true, origine: None });
        p.murs.push(Mur { id: "m".into(), nom: "Mur".into(), type_mur: "t".into(), niveau: "n".into(), decalage_bas: 0.0, haut: Haut::Libre { hauteur: 3000.0 }, axe: Axe::Droit { debut: [0.0, 0.0], fin: [8000.0, 0.0] }, assemblage: None, phase: None, sous_projet: None, variante: None, origine: None });
        let mut famille = FamilleModelee::gabarit_fenetre();
        famille.parametres.iter_mut().find(|x| x.nom == "MateriauAppui").unwrap().formule = "Pierre".into();
        p.types_ouvertures.push(TypeOuverture { id: "f".into(), nom: "F1".into(), genre: GenreOuverture::Fenetre, largeur: 1200.0, hauteur: 1350.0, famille: None, modele: Some(famille), origine: None });
        for (k, avec) in [(0, true), (1, false), (2, true)] {
            p.ouvertures.push(Ouverture {
                id: format!("o{k}"), nom: format!("F{k}"), genre: GenreOuverture::Fenetre, mur: "m".into(), type_ouverture: Some("f".into()),
                abscisse: 1500.0 + 2500.0 * k as f64, largeur: 1200.0, hauteur: 1350.0, allege: 900.0, representation: None,
                valeurs: if avec { BTreeMap::new() } else { BTreeMap::from([("AvecAppui".to_string(), 0.0)]) },
                materiaux: BTreeMap::new(), retournee: false, inversee: false, origine: None,
            });
        }
        p.verifier().unwrap();
        let n = Nomenclature { id: "n".into(), nom: "Fenêtres".into(), categorie: "Fenêtres".into(), regrouper: true, colonnes: vec![Colonne::Type, Colonne::Nombre], parametres: vec!["AvecAppui".into(), "MateriauAppui".into()] };
        let t = tableau(&p, &n).unwrap();
        assert_eq!(t[0], ["Type", "Nombre", "AvecAppui", "MateriauAppui"]);
        // l'occurrence sans appui forme son propre groupe
        assert!(t.contains(&vec!["F1".to_string(), "2".into(), "oui".into(), "Pierre".into()]), "{t:?}");
        assert!(t.contains(&vec!["F1".to_string(), "1".into(), "non".into(), "Pierre".into()]), "{t:?}");
    }

    #[test]
    fn nomenclature_csv_et_quantites_sans_double_compte() {
        let (p, _) =
            depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
                .unwrap();
        let n = Nomenclature {
            id: "n".into(),
            nom: "Murs".into(),
            categorie: "Murs".into(),
            regrouper: true,
            colonnes: Vec::new(),
            parametres: Vec::new(),
        };
        // colonnes choisies : type, niveau, nombre, longueur, volume ; regroupées par type
        // et niveau, le nombre total reste celui des murs
        let choisie = Nomenclature { colonnes: vec![Colonne::Type, Colonne::Niveau, Colonne::Nombre, Colonne::Longueur, Colonne::Volume], ..n.clone() };
        let t = tableau(&p, &choisie).unwrap();
        assert_eq!(t[0], ["Type", "Niveau", "Nombre", "Longueur (mm)", "Volume net (m³)"]);
        let total: usize = t.iter().skip(1).map(|r| r[2].parse::<usize>().unwrap()).sum();
        assert_eq!(total, p.murs.len());
        let detail = tableau(&p, &Nomenclature { regrouper: false, ..choisie.clone() }).unwrap();
        assert_eq!(detail.len() - 1, p.murs.len());
        assert_eq!(Colonne::depuis_titre("Famille et type"), Some(Colonne::Type));
        assert_eq!(Colonne::depuis_titre("Commentaires"), None);
        // tableau posé sur une feuille : titre, une ligne par groupe, sous sa position
        let mut q = p.clone();
        q.nomenclatures.push(choisie.clone());
        let posee = NomenclaturePlacee { nomenclature: choisie.id.clone(), position: [20.0, 200.0] };
        let (prims, e) = nomenclature_placee(&q, &posee).unwrap();
        assert!(!prims.is_empty() && e[3] == 200.0 && e[1] < 200.0 - 6.0 * t.len() as f64 && e[2] > 20.0, "{e:?}");
        let f = Feuille { id: "f".into(), numero: "A01".into(), nom: "Quantités".into(), jeu: JEU_DEFAUT.into(), largeur: 420.0, hauteur: 297.0, cartouche: String::new(), vues: Vec::new(), nomenclatures: vec![posee] };
        q.feuilles.push(f.clone());
        q.verifier().unwrap();
        assert!(svg_feuille(&q, &f).unwrap().len() > svg_feuille(&q, &Feuille { nomenclatures: Vec::new(), ..f.clone() }).unwrap().len());
        // nomenclature supprimée : retirée de la feuille
        let (q, _) = q.executer(crate::modele::commandes::Commande::Supprimer { ids: vec![choisie.id.clone()] }).unwrap();
        assert!(q.feuilles[0].nomenclatures.is_empty());
        let t = tableau(&p, &n).unwrap();
        let count: usize = t
            .iter()
            .skip(1)
            .map(|r| r[2].parse::<usize>().unwrap())
            .sum();
        assert_eq!(count, p.murs.len());
        assert_eq!(
            csv(&[vec!["A;B".into(), "Il dit \"oui\"".into()]]),
            "\"A;B\";\"Il dit \"\"oui\"\"\"\r\n"
        );
    }
}
