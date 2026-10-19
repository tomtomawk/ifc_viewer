//! Chargement complet d'un fichier IFC et assemblage des tampons de rendu.

use crate::geom::{self, GeomStats, ProductGeom};
use crate::model::IfcModel;
use crate::step::{Id, IdMap, StepFile};
use glam::{DVec3, Vec3};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

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
    pub min: Vec3,
    pub max: Vec3,
}

pub struct Scene {
    pub model: IfcModel,
    pub vertices: Vec<Vertex>,
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

fn assemble(model: IfcModel, geoms: Vec<ProductGeom>, file_name: String) -> Scene {
    let mut gmin = DVec3::splat(f64::MAX);
    let mut gmax = DVec3::splat(f64::MIN);
    for g in &geoms {
        for p in &g.parts {
            for t in &p.tris {
                for v in t {
                    gmin = gmin.min(*v);
                    gmax = gmax.max(*v);
                }
            }
        }
    }
    let origin = if gmin.x <= gmax.x { (gmin + gmax) * 0.5 } else { DVec3::ZERO };

    let mut opaque: Vec<Vertex> = Vec::new();
    let mut transp: Vec<Vertex> = Vec::new();
    let mut objects = Vec::with_capacity(geoms.len());
    let mut triangles = 0;

    for g in &geoms {
        let obj = objects.len() as u32;
        let (o0, t0) = (opaque.len() as u32, transp.len() as u32);
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for p in &g.parts {
            let color = p.color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            let dst = if p.color[3] < 0.99 { &mut transp } else { &mut opaque };
            for t in &p.tris {
                let n = (t[1] - t[0]).cross(t[2] - t[0]);
                let len = n.length();
                if !(len > 1e-14) {
                    continue;
                }
                let n = n / len;
                let normal = [(n.x * 127.0) as i8, (n.y * 127.0) as i8, (n.z * 127.0) as i8, 0];
                for v in t {
                    let p = (*v - origin).as_vec3();
                    mn = mn.min(p);
                    mx = mx.max(p);
                    dst.push(Vertex { pos: p.to_array(), normal, color, obj });
                }
                triangles += 1;
            }
        }
        if mn.x > mx.x {
            continue;
        }
        objects.push(SceneObject {
            id: g.id,
            class: model.get(g.id).map(|e| e.ty.clone()).unwrap_or_else(|| Arc::from("?")),
            storey: model.storey_of(g.id),
            opaque: Range { start: o0, count: opaque.len() as u32 - o0 },
            transparent: Range { start: t0, count: transp.len() as u32 - t0 },
            min: mn,
            max: mx,
        });
    }

    // Tampon unique : opaques puis transparents.
    let shift = opaque.len() as u32;
    for o in &mut objects {
        o.transparent.start += shift;
    }
    opaque.extend(transp);
    let vertices = opaque;

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
        objects,
        object_of,
        origin,
        min,
        max,
        triangles,
        unsupported: vec![],
        load_ms: 0,
        file_name,
    }
}

impl Scene {
    /// Objet visible le plus proche le long du rayon.
    pub fn pick(&self, origin: Vec3, dir: Vec3, visible: &[bool]) -> Option<(u32, f32)> {
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
                        if best.is_none_or(|b| d < b.1) {
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
