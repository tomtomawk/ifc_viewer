//! Scène triangulée : tampons de rendu, objets, navigateur de projet, picking.

use ifc_base::semantique::IfcModel;
use ifc_base::step::IdMap;
use glam::{DMat4, DVec3, DVec4, Vec3, Vec4};
use noyau::Id;
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [i8; 4],
    pub color: [u8; 4],
    pub obj: u32,
}

#[derive(Clone, Copy)]
pub struct Range {
    pub start: u32,
    pub count: u32,
}

pub struct SceneObject {
    pub id: Id,
    pub class: Arc<str>,
    pub storey: Option<Id>,
    pub opaque: Range,
    pub transparent: Range,
    /// Sommets des traits (paires, tracés en lignes).
    pub lines: Range,
    /// Sommets des arêtes vives (paires), tracées en mode dessin.
    pub aretes: Range,
    pub min: Vec3,
    pub max: Vec3,
}

/// Arborescence du projet d'un fichier Revit, comme le navigateur de projet de Revit :
/// vues, nomenclatures, feuilles, familles et groupes.
#[derive(Default)]
pub struct Navigateur {
    /// Phases dans l'ordre (ids Revit).
    pub phases: Vec<i64>,
    /// Filtres de phase : présentation par état de phase (`ElementOnPhaseStatus` :
    /// aucun, passé, existant, démoli, nouveau, temporaire, futur), 0 masqué, 1 par
    /// catégorie, 2 remplacé.
    pub filtres_phase: std::collections::HashMap<i64, [u8; 7]>,
    /// Phases de création et de démolition des objets de la scène.
    pub phases_objets: std::collections::HashMap<Id, (i64, i64)>,
    /// Objets d'une option de variante secondaire : masqués comme dans Revit, qui ne
    /// montre que les options principales hors vues réglées autrement.
    pub variantes_secondaires: std::collections::HashSet<Id>,
    pub vues: Vec<NavVue>,
    pub familles: Vec<NavFamille>,
    pub groupes: Vec<String>,
    pub materiaux: Vec<NavMateriau>,
}

/// Matériau et éléments qui le portent (entités de la scène).
#[derive(Clone)]
pub struct NavMateriau {
    pub nom: String,
    pub couleur: [u8; 3],
    /// 0 opaque, 1 transparent.
    pub transparence: f64,
    pub elements: Vec<Id>,
}

/// Contenu d'une nomenclature : titres, lignes, colonnes non calculées, tri en clair.
pub type Tableau = (Vec<String>, Vec<Vec<String>>, Vec<usize>, String);

/// Vue, nomenclature ou feuille.
pub struct NavVue {
    pub revit_id: i64,
    /// Phase de la vue et filtre de phase (VIEW_PHASE, VIEW_PHASE_FILTER ; −1 : aucun).
    pub phase: i64,
    pub filtre_phase: i64,
    /// Hauteur du plan de coupe d'un plan au-dessus de son niveau, m (plage de vue).
    pub coupe: Option<f64>,
    /// Altitude absolue (m) de la limite basse d'un plan natif.
    pub limite_basse: Option<f64>,
    /// Plan de coupe vertical d'une coupe native (n, d ; n·p + d ≥ 0 gardé, m, repère
    /// du fichier).
    pub plan_coupe: Option<DVec4>,
    /// Cadrage d'une coupe native : profondeur et côtés (plans gardés, m, repère du
    /// fichier).
    pub cadrage: Vec<DVec4>,
    /// Contenu d'une nomenclature : titres, lignes, colonnes non calculées, tri en clair.
    pub tableau: Option<Tableau>,
    /// Entité de la vue dans le modèle (propriétés ; ses éléments 2D en enfants).
    pub entite: Option<Id>,
    /// Genre (« Plan d'étage », « Élévation », « Feuille »…).
    pub genre: String,
    /// Nom du type de vue (« Élévation 1 »), vide s'il n'y en a pas.
    pub type_vue: String,
    pub nom: String,
    /// Dénominateur de l'échelle.
    pub echelle: f64,
    /// Niveau de détail (VIEW_DETAIL_LEVEL : 1 faible, 2 moyen, 3 élevé ; 0 : non lu).
    pub detail: i64,
    /// Numéro d'une feuille.
    pub numero: String,
    /// Orientation de la vue : haut et direction vers l'observateur.
    pub haut: DVec3,
    pub direction: DVec3,
    /// Étage d'un plan.
    pub etage: Option<Id>,
    /// Vues placées sur une feuille (ids Revit).
    pub placees: Vec<i64>,
    /// Fenêtres de vue d'une feuille, dans l'ordre de `placees`.
    pub fenetres: Vec<NavFenetre>,
    /// Éléments 2D de la vue (traits, régions, textes, cotes, symboles) en primitives,
    /// repère du fichier (m), pour l'export.
    pub primitives: Vec<noyau::trace2d::Primitive>,
}

/// Fenêtre de vue d'une feuille : la vue placée et le passage du modèle à la feuille.
/// Coordonnées en mètres, repère du fichier (avant soustraction de `Scene::origin`).
pub struct NavFenetre {
    /// Vue placée (id Revit).
    pub vue: i64,
    /// Repère affine du modèle vers la feuille : x, y sur le papier, z = profondeur
    /// réduite à l'échelle (vers l'observateur).
    pub vers_feuille: DMat4,
    /// Cadrage du modèle sur la feuille (xmin, ymin, xmax, ymax), None : vue non cadrée.
    pub cadre: Option<[f64; 4]>,
    /// Cadrage des annotations de la vue sur la feuille, None : non découpées.
    pub cadre_annotations: Option<[f64; 4]>,
    /// Plans de découpe en profondeur dans le modèle (n, d) : n·p + d ≥ 0 est gardé.
    pub plans: Vec<DVec4>,
    /// Direction de la vue (vers l'observateur), pour l'éclairage.
    pub direction: DVec3,
    /// Éléments masqués dans la vue placée (ids Revit).
    pub masques: Vec<i64>,
}

/// Famille (chargeable ou système) et ses types (nom, entité du type s'il est utilisé).
pub struct NavFamille {
    pub categorie: String,
    pub nom: String,
    pub types: Vec<(String, Option<Id>)>,
}

/// Coordonnées partagées d'un document Revit placé (emplacement actif) : point du
/// repère du fichier (m) → coordonnées partagées (m), colonnes appliquées à p − origine.
#[derive(Clone, Debug)]
pub struct ReperePartage {
    /// Nom de l'emplacement.
    pub nom: String,
    /// Point topographique dans le repère du fichier, m.
    pub origine: DVec3,
    pub colonnes: [DVec3; 3],
}

impl ReperePartage {
    pub fn partage(&self, p: DVec3) -> DVec3 {
        let d = p - self.origine;
        DVec3::new(self.colonnes[0].dot(d), self.colonnes[1].dot(d), self.colonnes[2].dot(d))
    }
}

impl Scene {
    /// Le sommet `k` est montré dans une vue d'affichage `a` (niveau de détail et type).
    pub fn montre(&self, k: usize, a: modele::famille::modelee::Affichage) -> bool {
        self.visibilite.get(k).is_none_or(|m| a.montre(*m))
    }

    /// Primitives de `n` sommets de `r` (3 : triangles, 2 : traits) montrées avec `a`.
    pub fn primitives_montrees(&self, r: Range, n: usize, a: modele::famille::modelee::Affichage) -> impl Iterator<Item = &[Vertex]> {
        let debut = r.start as usize;
        self.vertices[debut..debut + r.count as usize].chunks_exact(n).enumerate().filter(move |(k, _)| self.montre(debut + k * n, a)).map(|(_, t)| t)
    }

    /// Indices des sommets de `r` montrés avec `a`, ajoutés à `out` (les sommets d'un
    /// même triangle ou trait partagent leur masque).
    pub fn indices_montres(&self, r: Range, a: modele::famille::modelee::Affichage, out: &mut Vec<u32>) {
        let plage = r.start..r.start + r.count;
        if self.visibilite.is_empty() { out.extend(plage) } else { out.extend(plage.filter(|&k| self.montre(k as usize, a))) }
    }

    /// Altitude du dessus le plus haut sous (x, y) sans dépasser `z_max` (repère de la
    /// scène) : sol, dalle, marche ou terrain, pour la marche en vue piéton. Les
    /// annotations et les ouvertures sont ignorées.
    pub fn sol_sous(&self, x: f32, y: f32, z_max: f32) -> Option<f32> {
        let mut meilleur: Option<f32> = None;
        for o in &self.objects {
            if matches!(&*o.class, "IFCANNOTATION" | "IFCOPENINGELEMENT" | "IFCSPACE" | "IFCVIRTUALELEMENT") {
                continue;
            }
            let r = o.opaque;
            let fin = (r.start + r.count) as usize;
            if fin > self.vertices.len() || o.min.x > x || o.max.x < x || o.min.y > y || o.max.y < y || o.min.z > z_max {
                continue;
            }
            for t in self.vertices[r.start as usize..fin].as_chunks::<3>().0 {
                let [a, b, c] = [t[0].pos, t[1].pos, t[2].pos];
                let d = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
                if d.abs() < 1e-9 {
                    continue;
                }
                let u = ((x - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (y - a[1])) / d;
                let v = ((b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1])) / d;
                if u < 0.0 || v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let z = a[2] + u * (b[2] - a[2]) + v * (c[2] - a[2]);
                if z <= z_max && meilleur.is_none_or(|m| z > m) {
                    meilleur = Some(z);
                }
            }
        }
        meilleur
    }

    /// Distance du premier obstacle (face opaque ou vitrée) le long du rayon `o + t·d`
    /// (d unitaire), jusqu'à `max` : collision de la marche en vue piéton.
    pub fn obstacle(&self, o: Vec3, d: Vec3, max: f32) -> Option<f32> {
        let fin_rayon = o + d * max;
        let (bmin, bmax) = (o.min(fin_rayon), o.max(fin_rayon));
        let mut proche: Option<f32> = None;
        for ob in &self.objects {
            if matches!(&*ob.class, "IFCANNOTATION" | "IFCOPENINGELEMENT" | "IFCSPACE" | "IFCVIRTUALELEMENT") {
                continue;
            }
            if ob.min.cmpgt(bmax).any() || ob.max.cmplt(bmin).any() {
                continue;
            }
            for r in [ob.opaque, ob.transparent] {
                let fin = (r.start + r.count) as usize;
                if fin > self.vertices.len() {
                    continue;
                }
                for t in self.vertices[r.start as usize..fin].as_chunks::<3>().0 {
                    let [a, b, c] = [Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos)];
                    // Möller-Trumbore
                    let (e1, e2) = (b - a, c - a);
                    let p = d.cross(e2);
                    let det = e1.dot(p);
                    if det.abs() < 1e-9 {
                        continue;
                    }
                    let s = o - a;
                    let u = s.dot(p) / det;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let q = s.cross(e1);
                    let v = d.dot(q) / det;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let tt = e2.dot(q) / det;
                    if tt > 0.0 && tt <= max && proche.is_none_or(|m| tt < m) {
                        proche = Some(tt);
                    }
                }
            }
        }
        proche
    }
}

pub struct Scene {
    pub model: IfcModel,
    pub vertices: Vec<Vertex>,
    /// Couleur de rendu (apparence du matériau) de chaque sommet, alpha 0 : aucune
    /// (style Rendu).
    pub couleurs_rendu: Vec<[u8; 4]>,
    /// Motif de surface de chaque sommet : [premier plan | arrière-plan << 4, r, g, b].
    pub motifs_surface: Vec<[u8; 4]>,
    /// Origine projetée (mètres), cosinus et sinus par sommet des faces placées.
    pub placements_surface: Vec<[f32; 4]>,
    /// Définitions des motifs de surface, dans le même ordre que table_motifs.
    pub definitions_surface: Vec<modele::DefinitionMotif>,
    /// Masque de visibilité de chaque sommet par niveau de détail et type de vue
    /// (`VisibiliteElement::masque`, éléments des familles) ; vide : tout visible.
    pub visibilite: Vec<u8>,
    /// Motifs de surface de la scène, indexés par les codes des sommets (moins un) :
    /// indice de la première famille dans `familles_motifs` et nombre de familles.
    pub table_motifs: Vec<[f32; 4]>,
    /// Familles de traits des motifs de surface, trois `vec4` chacune (voir
    /// `maillage::materiaux::familles_gpu`).
    pub familles_motifs: Vec<[f32; 4]>,
    /// Apparences de rendu des matériaux, indexées par l'alpha des couleurs de rendu
    /// (128 + rang).
    pub apparences: Vec<modele::Apparence>,
    pub objects: Vec<SceneObject>,
    pub object_of: IdMap<u32>,
    /// Origine (mètres, repère du fichier) soustraite aux coordonnées f32.
    pub origin: DVec3,
    pub min: Vec3,
    pub max: Vec3,
    pub triangles: usize,
    pub unsupported: Vec<(String, usize)>,
    pub load_ms: u128,
    pub file_name: String,
    /// Arborescence du projet (fichiers Revit).
    pub navigateur: Option<Navigateur>,
    /// Coordonnées partagées (fichier Revit placé).
    pub partage: Option<ReperePartage>,
    /// Bandes de composition dont la position dans un mur droit est vérifiée.
    pub couches_coupe: IdMap<Vec<CoucheCoupe>>,
    /// Styles d'objets : contours coupés de l'objet ; épaisseur (0 : celle des bandes ou
    /// le trait fort) et tirets.
    pub traits_coupe: IdMap<TraitVu>,
    /// Styles d'objets : lignes vues de l'objet (épaisseur et tirets).
    pub traits_vus: IdMap<TraitVu>,
}

/// Lignes vues d'un objet : épaisseur (mm du papier ; 0 : trait par défaut) et tirets
/// (longueurs alternées trait, blanc… en mm du papier ; vide : continu).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TraitVu {
    pub epaisseur: f64,
    pub tirets: Vec<f64>,
}

#[derive(Clone)]
pub struct CoucheCoupe {
    /// Région de la couche : demi-espaces `n · x + w >= 0`.
    pub plans: Vec<Vec4>,
    /// Couleur du matériau (poché du style Réaliste).
    pub couleur: [u8; 4],
    /// Arrière-plan du poché (style Graphique) : couleur et motif (None : aucun, blanc ;
    /// grilles vides : uni).
    pub fond: [u8; 4],
    pub motif_fond: Option<Arc<noyau::motif::Motif>>,
    /// Ligne d'isolant en boucles au milieu de la couche (premier plan).
    pub isolation: bool,
    /// Centre du mur courbe dans le repère du modèle, en mètres.
    pub centre_isolation: Option<[f64; 2]>,
    /// Épaisseur du contour de coupe (mm du papier, plume de la couche).
    pub trait_mm: f64,
    pub motif: Option<Arc<noyau::motif::Motif>>,
    pub couleur_motif: [u8; 4],
    pub hachure_defaut: bool,
}

impl Scene {
    /// Objet visible le plus proche le long du rayon ; avec une coupe (n, d), seuls les
    /// points gardés (n·p + d ≥ 0) des objets du modèle comptent. Sous une coupe (plan,
    /// coupe verticale), la topographie n'est prise que faute d'autre objet : elle passe
    /// sous les murs qu'elle recoupe au lieu de les masquer.
    pub fn pick(&self, origin: Vec3, dir: Vec3, visible: &[bool], coupe: Option<Vec4>, boite: Option<(Vec3, Vec3)>) -> Option<(u32, f32)> {
        self.pick_triangle(origin, dir, visible, coupe, boite, None)
            .map(|(objet, distance, _)| (objet, distance))
    }

    /// Triangle touché en premier, dans le repère recentré de la scène.
    pub fn pick_triangle(&self, origin: Vec3, dir: Vec3, visible: &[bool], coupe: Option<Vec4>, boite: Option<(Vec3, Vec3)>, affichage: Option<modele::famille::modelee::Affichage>) -> Option<(u32, f32, [Vec3; 3])> {
        let inv = dir.recip();
        let mut best: Option<(u32, f32, [Vec3; 3])> = None;
        let mut topo: Option<(u32, f32, [Vec3; 3])> = None;
        for (i, o) in self.objects.iter().enumerate() {
            if !visible[i] {
                continue;
            }
            let Some(tmin) = ray_aabb(origin, inv, o.min, o.max) else { continue };
            let au_fond = coupe.is_some() && &*o.class == "IFCGEOGRAPHICELEMENT";
            let cible = if au_fond { topo } else { best };
            if cible.is_some_and(|b| tmin > b.1) {
                continue;
            }
            for r in [o.opaque, o.transparent] {
                let vs = &self.vertices[r.start as usize..(r.start + r.count) as usize];
                for (k, t) in vs.as_chunks::<3>().0.iter().enumerate() {
                    if affichage.is_some_and(|a| !self.montre(r.start as usize + k * 3, a)) { continue; }
                    let (a, b, c) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
                    if let Some(d) = ray_tri(origin, dir, a, b, c) {
                        let q = origin + dir * d;
                        let garde = t[0].normal[3] > 0 || (coupe.is_none_or(|p| p.truncate().dot(q) + p.w >= 0.0) && boite.is_none_or(|(a, b)| q.cmpge(a).all() && q.cmple(b).all()));
                        if garde {
                            let cible = if au_fond { &mut topo } else { &mut best };
                            if cible.is_none_or(|b| d < b.1) {
                                *cible = Some((i as u32, d, [a, b, c]));
                            }
                        }
                    }
                }
            }
        }
        best.or(topo)
    }
}

impl Scene {
    /// Tous les objets visibles traversés par le rayon, du plus proche au plus lointain
    /// (même règle de coupe et de boîte que `pick`, topographie après les autres) :
    /// candidats du cycle de sélection (Tab).
    pub fn pick_tous(&self, origin: Vec3, dir: Vec3, visible: &[bool], coupe: Option<Vec4>, boite: Option<(Vec3, Vec3)>) -> Vec<(u32, f32)> {
        let inv = dir.recip();
        let mut out: Vec<(u32, f32, bool)> = Vec::new();
        for (i, o) in self.objects.iter().enumerate() {
            if !visible[i] || ray_aabb(origin, inv, o.min, o.max).is_none() {
                continue;
            }
            let au_fond = coupe.is_some() && &*o.class == "IFCGEOGRAPHICELEMENT";
            let mut proche: Option<f32> = None;
            for r in [o.opaque, o.transparent] {
                let vs = &self.vertices[r.start as usize..(r.start + r.count) as usize];
                for t in vs.as_chunks::<3>().0 {
                    let (a, b, c) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
                    let Some(d) = ray_tri(origin, dir, a, b, c) else { continue };
                    let q = origin + dir * d;
                    let garde = t[0].normal[3] > 0 || (coupe.is_none_or(|p| p.truncate().dot(q) + p.w >= 0.0) && boite.is_none_or(|(a, b)| q.cmpge(a).all() && q.cmple(b).all()));
                    if garde && proche.is_none_or(|x| d < x) {
                        proche = Some(d);
                    }
                }
            }
            if let Some(d) = proche {
                out.push((i as u32, d, au_fond));
            }
        }
        out.sort_by(|a, b| a.2.cmp(&b.2).then(a.1.total_cmp(&b.1)));
        out.into_iter().map(|(i, d, _)| (i, d)).collect()
    }
}

fn ray_aabb(o: Vec3, inv: Vec3, mn: Vec3, mx: Vec3) -> Option<f32> {
    let t1 = (mn - o) * inv;
    let t2 = (mx - o) * inv;
    let tmin = t1.min(t2).max_element();
    let tmax = t1.max(t2).min_element();
    (tmax >= tmin.max(0.0)).then_some(tmin.max(0.0))
}

fn ray_tri(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

/// État d'un élément dans une phase (`Element.GetPhaseStatus`) : 0 aucun, 1 passé,
/// 2 existant, 3 démoli, 4 nouveau, 5 temporaire, 6 futur, d'après l'ordre des phases.
pub fn etat_de_phase(ordre: &[i64], cree: i64, demoli: i64, phase: i64) -> usize {
    let rang = |p: i64| ordre.iter().position(|&x| x == p);
    let (Some(c), Some(p)) = (rang(cree), rang(phase)) else { return 0 };
    let d = rang(demoli);
    if c > p {
        6
    } else if c == p {
        if d == Some(p) { 5 } else { 4 }
    } else {
        match d {
            None => 2,
            Some(d) if d > p => 2,
            Some(d) if d == p => 3,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deux dalles carrées de 10 m, à z = 0 et z = 3 (objets 0 et 1, deux triangles
    /// chacune, normales vers le haut).
    fn scene() -> Scene {
        let model = IfcModel::new(ifc_base::step::StepFile::parse(b"ISO-10303-21;\nDATA;\n#1=IFCPROJECT('0',$,'P',$,$,$,$,$,$);\nENDSEC;\nEND-ISO-10303-21;\n").unwrap());
        let mut vertices = Vec::new();
        let mut objects = Vec::new();
        for (k, z) in [0.0f32, 3.0].into_iter().enumerate() {
            let p = |x: f32, y: f32| Vertex { pos: [x, y, z], normal: [0, 0, 127, 0], color: [200; 4], obj: k as u32 };
            let debut = vertices.len() as u32;
            vertices.extend([p(0., 0.), p(10., 0.), p(10., 10.), p(0., 0.), p(10., 10.), p(0., 10.)]);
            let vide = Range { start: 0, count: 0 };
            objects.push(SceneObject { id: k as Id + 1, class: "IFCSLAB".into(), storey: None, opaque: Range { start: debut, count: 6 }, transparent: vide, lines: vide, aretes: vide, min: Vec3::new(0., 0., z), max: Vec3::new(10., 10., z) });
        }
        Scene {
            model, vertices, couleurs_rendu: Vec::new(), motifs_surface: Vec::new(), placements_surface: Vec::new(), definitions_surface: Vec::new(), visibilite: Vec::new(), table_motifs: Vec::new(), familles_motifs: Vec::new(), apparences: Vec::new(),
            objects, object_of: IdMap::default(), origin: DVec3::ZERO, min: Vec3::ZERO, max: Vec3::new(10., 10., 3.), triangles: 4, unsupported: Vec::new(),
            load_ms: 0, file_name: String::new(), navigateur: None, partage: None, couches_coupe: IdMap::default(),
            traits_coupe: IdMap::default(), traits_vus: IdMap::default(),
        }
    }

    /// T1 : sélection au rayon : dalle haute d'abord ; une coupe horizontale qui la
    /// retire laisse la dalle basse ; un objet masqué n'est pas pris ; tous les objets
    /// traversés, du plus proche au plus lointain.
    #[test]
    fn selection_au_rayon() {
        let s = scene();
        let (o, d) = (Vec3::new(5., 5., 10.), Vec3::NEG_Z);
        assert_eq!(s.pick(o, d, &[true, true], None, None).map(|x| x.0), Some(1));
        // plan de coupe z <= 2 gardé (n · x + w >= 0 avec n = -Z, w = 2)
        assert_eq!(s.pick(o, d, &[true, true], Some(Vec4::new(0., 0., -1., 2.)), None).map(|x| x.0), Some(0));
        assert_eq!(s.pick(o, d, &[true, false], None, None).map(|x| x.0), Some(0));
        assert_eq!(s.pick_tous(o, d, &[true, true], None, None).iter().map(|x| x.0).collect::<Vec<_>>(), [1, 0]);
        assert!(s.pick(Vec3::new(20., 5., 10.), d, &[true, true], None, None).is_none());
    }

    /// Sol sous un point (vue piéton) : le dessus le plus haut sous `z_max`.
    #[test]
    fn sol_sous_le_pieton() {
        let s = scene();
        assert_eq!(s.sol_sous(5., 5., 2.), Some(0.0));
        assert_eq!(s.sol_sous(5., 5., 4.), Some(3.0));
        assert_eq!(s.sol_sous(50., 5., 4.), None);
    }

    /// État de phase (`GetPhaseStatus`) d'après l'ordre des phases.
    #[test]
    fn etats_de_phase() {
        let ordre = [10, 20, 30];
        assert_eq!(etat_de_phase(&ordre, 10, -1, 20), 2); // existant
        assert_eq!(etat_de_phase(&ordre, 10, 20, 20), 3); // démoli dans la phase
        assert_eq!(etat_de_phase(&ordre, 20, -1, 20), 4); // nouveau
        assert_eq!(etat_de_phase(&ordre, 20, 20, 20), 5); // temporaire
        assert_eq!(etat_de_phase(&ordre, 30, -1, 20), 6); // futur
        assert_eq!(etat_de_phase(&ordre, 10, 20, 30), 1); // passé (démoli avant)
        assert_eq!(etat_de_phase(&ordre, 99, -1, 20), 0); // phase inconnue
    }
}
