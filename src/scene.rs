//! Chargement complet d'un fichier IFC et assemblage des tampons de rendu.

use crate::geom::{self, GeomStats, ProductGeom};
use crate::model::IfcModel;
use crate::step::{Id, IdMap, StepFile};
use glam::{DMat4, DVec3, DVec4, Vec3, Vec4};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
mod natif;
pub use natif::{depuis_natif, id_natif, numero_vue};

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
    pub elements: Vec<crate::step::Id>,
}

/// Vue, nomenclature ou feuille.
pub struct NavVue {
    pub revit_id: i64,
    /// Phase de la vue et filtre de phase (VIEW_PHASE, VIEW_PHASE_FILTER ; −1 : aucun).
    pub phase: i64,
    pub filtre_phase: i64,
    /// Hauteur du plan de coupe d'un plan au-dessus de son niveau, m (plage de vue).
    pub coupe: Option<f64>,
    /// Plan de coupe vertical d'une coupe native (n, d ; n·p + d ≥ 0 gardé, m, repère
    /// du fichier).
    pub plan_coupe: Option<DVec4>,
    /// Cadrage d'une coupe native : profondeur et côtés (plans gardés, m, repère du
    /// fichier).
    pub cadrage: Vec<DVec4>,
    /// Contenu d'une nomenclature : titres, lignes, colonnes non calculées, tri en clair.
    pub tableau: Option<(Vec<String>, Vec<Vec<String>>, Vec<usize>, String)>,
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
    pub primitives: Vec<crate::trace2d::Primitive>,
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

pub struct Scene {
    pub model: IfcModel,
    pub vertices: Vec<Vertex>,
    /// Couleur de rendu (apparence du matériau) de chaque sommet, alpha 0 : aucune
    /// (style Rendu).
    pub couleurs_rendu: Vec<[u8; 4]>,
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
}

pub struct CoucheCoupe {
    /// Région de la couche : demi-espaces `n · x + w >= 0`.
    pub plans: Vec<Vec4>,
    pub couleur: [u8; 4],
    pub motif: Option<Arc<crate::rvt::vues::Motif>>,
    pub couleur_motif: [u8; 4],
    pub hachure_defaut: bool,
}

/// Vue de départ d'un projet natif vide, sans inventer de géométrie ni de source IFC.
#[cfg(test)]
pub fn projet_vide(projet: &crate::modele::Projet, path: &Path) -> Result<Scene, String> {
    depuis_natif(projet, path)
}

pub fn load(path: &Path, progress: &dyn Fn(String)) -> Result<Scene, String> {
    let t0 = Instant::now();
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    progress(format!("Lecture de {file_name}…"));
    let data = std::fs::read(path).map_err(|e| format!("Lecture impossible : {e}"))?;
    if data.starts_with(b"PK") {
        return Err("Fichier IFC-ZIP (.ifczip) non pris en charge : décompresser d'abord.".into());
    }
    progress(format!("Analyse STEP ({:.1} Mo)…", data.len() as f64 / 1e6));
    let file = StepFile::parse(&data)?;
    drop(data);
    let n_ent = file.entities.len();
    progress(format!("Indexation de {n_ent} entités…"));
    let model = IfcModel::new(file);

    progress(format!("Géométrie de {} produits…", model.products.len()));
    let stats = GeomStats::default();
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(64 << 20)
        .build()
        .map_err(|e| e.to_string())?;
    let mut geoms: Vec<ProductGeom> = pool.install(|| {
        model
            .products
            .par_iter()
            .filter_map(|&id| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| geom::product_geometry(&model, &stats, id)))
                    .ok()
                    .flatten()
            })
            .collect()
    });
    geoms.sort_by_key(|g| g.id);

    progress("Assemblage des tampons…".into());
    let mut scene = assemble(model, geoms, file_name);
    let mut unsupported: Vec<(String, usize)> = stats.unsupported.into_inner().unwrap().into_iter().collect();
    unsupported.sort_by(|a, b| b.1.cmp(&a.1));
    scene.unsupported = unsupported;
    scene.load_ms = t0.elapsed().as_millis();
    Ok(scene)
}

pub fn assemble(model: IfcModel, geoms: Vec<ProductGeom>, file_name: String) -> Scene {
    let mut gmin = DVec3::splat(f64::MAX);
    let mut gmax = DVec3::splat(f64::MIN);
    for g in &geoms {
        let traits = g.lignes.iter().flat_map(|l| l.segs.iter().flatten());
        for v in g.parts.iter().flat_map(|p| p.tris.iter().flatten()).chain(traits) {
            gmin = gmin.min(*v);
            gmax = gmax.max(*v);
        }
    }
    let origin = if gmin.x <= gmax.x { (gmin + gmax) * 0.5 } else { DVec3::ZERO };

    let mut opaque: Vec<Vertex> = Vec::new();
    let mut transp: Vec<Vertex> = Vec::new();
    let mut lignes: Vec<Vertex> = Vec::new();
    let mut aretes: Vec<Vertex> = Vec::new();
    // couleurs de rendu des sommets opaques et transparents (alpha 0 : aucune)
    let (mut rendu_o, mut rendu_t): (Vec<[u8; 4]>, Vec<[u8; 4]>) = (Vec::new(), Vec::new());
    let mut objects = Vec::with_capacity(geoms.len());
    let mut triangles = 0;
    let annotation = |g: &ProductGeom| model.get(g.id).is_some_and(|e| &*e.ty == "IFCANNOTATION");
    // arêtes vives des objets du modèle (pas des annotations : textes, cotes)
    let vives: Vec<crate::dessin::Aretes> = geoms
        .par_iter()
        .map(|g| {
            if annotation(g) {
                return Default::default();
            }
            let tris: Vec<[DVec3; 3]> = g.parts.iter().flat_map(|p| p.tris.iter().copied()).collect();
            crate::dessin::aretes(&tris)
        })
        .collect();

    // liserés d'un pixel des annotations (cotes, textes, traits) : un ruban plus fin qu'un
    // pixel ne disparaît plus d'une image à l'autre au zoom
    let liseres: Vec<Vec<Vec<[DVec3; 2]>>> = geoms
        .par_iter()
        .map(|g| if annotation(g) { g.parts.iter().map(|p| crate::dessin::aretes(&p.tris).vives).collect() } else { Vec::new() })
        .collect();

    for ((g, vives), liseres) in geoms.iter().zip(&vives).zip(&liseres) {
        let obj = objects.len() as u32;
        let (o0, t0, l0, a0) = (opaque.len() as u32, transp.len() as u32, lignes.len() as u32, aretes.len() as u32);
        // drapeau des annotations (4ᵉ octet de la normale) : jamais découpées par la coupe
        // du mode dessin, ni blanchies
        let drapeau = if annotation(g) { 127 } else { 0 };
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for (k, p) in g.parts.iter().enumerate() {
            let color = p.color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            let rendu = g.rendu.get(k).copied().flatten().map_or([0; 4], |c| [c[0], c[1], c[2], 1.0].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8));
            let (dst, dst_r) = if p.color[3] < 0.99 { (&mut transp, &mut rendu_t) } else { (&mut opaque, &mut rendu_o) };
            for t in &p.tris {
                let n = (t[1] - t[0]).cross(t[2] - t[0]);
                let len = n.length();
                if !(len > 1e-14) {
                    continue;
                }
                let n = n / len;
                let normal = [(n.x * 127.0) as i8, (n.y * 127.0) as i8, (n.z * 127.0) as i8, drapeau];
                for v in t {
                    let p = (*v - origin).as_vec3();
                    mn = mn.min(p);
                    mx = mx.max(p);
                    dst.push(Vertex { pos: p.to_array(), normal, color, obj });
                    dst_r.push(rendu);
                }
                triangles += 1;
            }
        }
        for (p, segs) in g.parts.iter().zip(liseres) {
            let color = p.color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            for v in segs.iter().flatten() {
                lignes.push(Vertex { pos: (*v - origin).as_vec3().to_array(), normal: [0, 0, 0, drapeau], color, obj });
            }
        }
        for l in &g.lignes {
            let color = l.color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            for v in l.segs.iter().flatten() {
                let p = (*v - origin).as_vec3();
                mn = mn.min(p);
                mx = mx.max(p);
                lignes.push(Vertex { pos: p.to_array(), normal: [0, 0, 0, drapeau], color, obj });
            }
        }
        for v in vives.vives.iter().flatten() {
            let p = (*v - origin).as_vec3();
            aretes.push(Vertex { pos: p.to_array(), normal: [0, 0, 0, 0], color: crate::dessin::NOIR, obj });
        }
        // arêtes douces : normale de la 1ʳᵉ facette, celle de la 2ᵉ dans la couleur ;
        // 4ᵉ octet de la normale à −127 (silhouette, voir render.rs)
        for (s, n1, n2) in &vives.douces {
            let normal = [(n1.x * 127.0) as i8, (n1.y * 127.0) as i8, (n1.z * 127.0) as i8, -127];
            let color = [n2.x, n2.y, n2.z].map(|c| ((c * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8);
            for v in s {
                let p = (*v - origin).as_vec3();
                aretes.push(Vertex { pos: p.to_array(), normal, color: [color[0], color[1], color[2], 255], obj });
            }
        }
        if mn.x > mx.x {
            aretes.truncate(a0 as usize);
            continue;
        }
        objects.push(SceneObject {
            id: g.id,
            class: model.get(g.id).map(|e| e.ty.clone()).unwrap_or_else(|| Arc::from("?")),
            storey: model.storey_of(g.id),
            opaque: Range { start: o0, count: opaque.len() as u32 - o0 },
            transparent: Range { start: t0, count: transp.len() as u32 - t0 },
            lines: Range { start: l0, count: lignes.len() as u32 - l0 },
            aretes: Range { start: a0, count: aretes.len() as u32 - a0 },
            min: mn,
            max: mx,
        });
    }

    // Tampon unique : opaques, transparents, sommets des traits, puis des arêtes vives.
    let shift = opaque.len() as u32;
    let shift_l = shift + transp.len() as u32;
    let shift_a = shift_l + lignes.len() as u32;
    for o in &mut objects {
        o.transparent.start += shift;
        o.lines.start += shift_l;
        o.aretes.start += shift_a;
    }
    rendu_o.extend(rendu_t);
    rendu_o.resize(rendu_o.len() + lignes.len() + aretes.len(), [0; 4]);
    opaque.extend(transp);
    opaque.extend(lignes);
    opaque.extend(aretes);
    let vertices = opaque;
    let couleurs_rendu = rendu_o;

    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut object_of = IdMap::default();
    for (i, o) in objects.iter().enumerate() {
        min = min.min(o.min);
        max = max.max(o.max);
        object_of.insert(o.id, i as u32);
    }
    if objects.is_empty() {
        min = Vec3::splat(-1.0);
        max = Vec3::splat(1.0);
    }

    Scene {
        model,
        vertices,
        couleurs_rendu,
        objects,
        object_of,
        origin,
        min,
        max,
        triangles,
        unsupported: vec![],
        load_ms: 0,
        file_name,
        navigateur: None,
        partage: None,
        couches_coupe: IdMap::default(),
    }
}

impl Scene {
    /// Objet visible le plus proche le long du rayon ; avec une coupe (n, d), seuls les
    /// points gardés (n·p + d ≥ 0) des objets du modèle comptent.
    pub fn pick(&self, origin: Vec3, dir: Vec3, visible: &[bool], coupe: Option<Vec4>, boite: Option<(Vec3, Vec3)>) -> Option<(u32, f32)> {
        let inv = dir.recip();
        let mut best: Option<(u32, f32)> = None;
        for (i, o) in self.objects.iter().enumerate() {
            if !visible[i] {
                continue;
            }
            let Some(tmin) = ray_aabb(origin, inv, o.min, o.max) else { continue };
            if best.is_some_and(|b| tmin > b.1) {
                continue;
            }
            for r in [o.opaque, o.transparent] {
                let vs = &self.vertices[r.start as usize..(r.start + r.count) as usize];
                for t in vs.chunks_exact(3) {
                    let (a, b, c) = (Vec3::from(t[0].pos), Vec3::from(t[1].pos), Vec3::from(t[2].pos));
                    if let Some(d) = ray_tri(origin, dir, a, b, c) {
                        let q = origin + dir * d;
                        let garde = t[0].normal[3] > 0 || (coupe.is_none_or(|p| p.truncate().dot(q) + p.w >= 0.0) && boite.is_none_or(|(a, b)| q.cmpge(a).all() && q.cmple(b).all()));
                        if garde && best.is_none_or(|b| d < b.1) {
                            best = Some((i as u32, d));
                        }
                    }
                }
            }
        }
        best
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
