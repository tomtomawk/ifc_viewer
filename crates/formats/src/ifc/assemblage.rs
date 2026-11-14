//! Chargement complet d'un fichier IFC et assemblage des tampons de rendu.

use crate::ifc::geom::{self, GeomStats, ProductGeom};
use ifc_base::semantique::IfcModel;
use ifc_base::step::{Id, IdMap, StepFile};
use scene::{Range, Scene, SceneObject, Vertex};
use glam::{DVec3, Vec3};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

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
    assemble_avec(model, geoms, file_name, std::collections::HashMap::new())
}

/// Comme [`assemble`], les arêtes de certains objets étant données (jonctions entre
/// éléments d'un même matériau, calculées ensemble).
pub fn assemble_avec(model: IfcModel, geoms: Vec<ProductGeom>, file_name: String, mut imposees: std::collections::HashMap<Id, noyau::aretes::Aretes>) -> Scene {
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
    // motifs de surface des sommets opaques et transparents (motif 0 : aucun)
    let (mut motifs_o, mut motifs_t): (Vec<[u8; 4]>, Vec<[u8; 4]>) = (Vec::new(), Vec::new());
    let mut objects = Vec::with_capacity(geoms.len());
    let mut triangles = 0;
    let annotation = |g: &ProductGeom| model.get(g.id).is_some_and(|e| &*e.ty == "IFCANNOTATION");
    // arêtes vives des objets du modèle (pas des annotations : textes, cotes)
    let vives: Vec<noyau::aretes::Aretes> = geoms
        .iter()
        .map(|g| imposees.remove(&g.id))
        .collect::<Vec<_>>()
        .into_par_iter()
        .zip(geoms.par_iter())
        .map(|(imposee, g)| {
            if let Some(a) = imposee {
                return a;
            }
            if annotation(g) {
                return Default::default();
            }
            let tris: Vec<[DVec3; 3]> = g.parts.iter().flat_map(|p| p.tris.iter().copied()).collect();
            noyau::aretes::aretes(&tris)
        })
        .collect();

    // liserés d'un pixel des annotations (cotes, textes, traits) : un ruban plus fin qu'un
    // pixel ne disparaît plus d'une image à l'autre au zoom
    let liseres: Vec<Vec<Vec<[DVec3; 2]>>> = geoms
        .par_iter()
        .map(|g| if annotation(g) { g.parts.iter().map(|p| noyau::aretes::aretes(&p.tris).vives).collect() } else { Vec::new() })
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
            // alpha au-delà de 1 : rang d'apparence codé (128 + rang), sinon 255
            let rendu = g.rendu.get(k).copied().flatten().map_or([0; 4], |c| {
                let [r, v, b] = [c[0], c[1], c[2]].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
                [r, v, b, if c[3] > 1.5 { c[3].clamp(128.0, 254.0) as u8 } else { 255 }]
            });
            let motif = g.motifs.get(k).copied().flatten().unwrap_or([0; 4]);
            let (dst, dst_r, dst_m) = if p.color[3] < 0.99 { (&mut transp, &mut rendu_t, &mut motifs_t) } else { (&mut opaque, &mut rendu_o, &mut motifs_o) };
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
                    dst_m.push(motif);
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
            aretes.push(Vertex { pos: p.to_array(), normal: [0, 0, 0, 0], color: noyau::aretes::NOIR, obj });
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
    motifs_o.extend(motifs_t);
    motifs_o.resize(rendu_o.len(), [0; 4]);
    opaque.extend(transp);
    opaque.extend(lignes);
    opaque.extend(aretes);
    let vertices = opaque;
    let couleurs_rendu = rendu_o;
    let motifs_surface = motifs_o;

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
        motifs_surface,
        table_motifs: Vec::new(),
        apparences: Vec::new(),
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
