//! Conversion des représentations IFC en triangles (coordonnées monde, mètres).

use crate::csg::{self, Op, Tri};
use crate::model::IfcModel;
use crate::step::{Entity, Id, Value};
use glam::{DMat4, DVec2, DVec3, DVec4};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::sync::Mutex;

pub type Rgba = [f32; 4];

#[derive(Clone)]
pub struct Part {
    pub color: Rgba,
    pub tris: Vec<Tri>,
}

/// Traits d'une couleur, tracés en lignes d'un pixel.
pub struct Ligne {
    pub color: Rgba,
    pub segs: Vec<[DVec3; 2]>,
}

pub struct ProductGeom {
    pub id: Id,
    pub parts: Vec<Part>,
    pub lignes: Vec<Ligne>,
    /// Couleur de rendu (apparence du matériau) de chaque partie, parallèle à `parts` ;
    /// vide ou None : sa couleur d'affichage.
    pub rendu: Vec<Option<Rgba>>,
    /// Motif de surface de chaque partie, parallèle à `parts` : [motif (1 diagonales,
    /// 2 croisées, 3 uni), r, g, b] ; vide ou None : aucun.
    pub motifs: Vec<Option<[u8; 4]>>,
}

/// Compteurs d'entités géométriques non prises en charge.
#[derive(Default)]
pub struct GeomStats {
    pub unsupported: Mutex<HashMap<String, usize>>,
}

impl GeomStats {
    fn miss(&self, ty: &str) {
        *self.unsupported.lock().unwrap().entry(ty.to_string()).or_default() += 1;
    }
}

struct Profile {
    outer: Vec<DVec2>,
    holes: Vec<Vec<DVec2>>,
}

const CIRCLE_SEGMENTS: f64 = 32.0;
/// Limites au-delà desquelles on ne calcule pas les ouvertures (performance).
const CSG_MAX_HOST_TRIS: usize = 6000;
const CSG_MAX_OPENING_TRIS: usize = 3000;

pub struct Geom<'a> {
    m: &'a IfcModel,
    stats: &'a GeomStats,
    /// Tolérance CSG en unités du fichier.
    eps: f64,
}

pub fn product_geometry(m: &IfcModel, stats: &GeomStats, id: Id) -> Option<ProductGeom> {
    let g = Geom { m, stats, eps: 1e-6 / m.length_scale };
    let e = m.get(id)?;
    let placement = g.placement(e.arg(5).as_ref(), 0);
    let default = default_color(m, id, &e.ty);

    let mut parts: Vec<Part> = Vec::new();
    for item in g.body_items(e.arg(6).as_ref()?) {
        g.item(item, DMat4::IDENTITY, None, default, &mut parts, 0);
    }
    if parts.is_empty() {
        return None;
    }

    // Ouvertures : soustraction dans le repère local du produit.
    if let Some(openings) = m.openings.get(&id) {
        let inv = placement.inverse();
        let mut cutters: Vec<Vec<Tri>> = Vec::new();
        for &o in openings {
            let Some(oe) = m.get(o) else { continue };
            let op = inv * g.placement(oe.arg(5).as_ref(), 0);
            let Some(rep) = oe.arg(6).as_ref() else { continue };
            let mut tris = Vec::new();
            for item in g.body_items(rep) {
                if let Some(t) = g.solid(item, op, 0) {
                    tris.extend(t);
                }
            }
            if !tris.is_empty() && tris.len() <= CSG_MAX_OPENING_TRIS {
                csg::fix_orientation(&mut tris);
                cutters.push(tris);
            }
        }
        if !cutters.is_empty() {
            for part in &mut parts {
                if part.tris.len() > CSG_MAX_HOST_TRIS {
                    continue;
                }
                let (bmin, bmax) = bbox(&part.tris);
                for c in &cutters {
                    let (cmin, cmax) = bbox(c);
                    if cmin.cmpgt(bmax).any() || cmax.cmplt(bmin).any() {
                        continue;
                    }
                    if let Some(r) = csg::boolean(&part.tris, c, Op::Difference, g.eps) {
                        part.tris = r;
                    }
                }
            }
        }
    }

    let world = DMat4::from_scale(DVec3::splat(m.length_scale)) * placement;
    for p in &mut parts {
        for t in &mut p.tris {
            for v in t.iter_mut() {
                *v = world.transform_point3(*v);
            }
        }
    }
    parts.retain(|p| !p.tris.is_empty());
    Some(ProductGeom { id, parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() })
}

/// Points (m, repère du projet) de la représentation « Axis » d'un produit : l'axe d'un
/// mur (arcs facettés).
pub fn axe(m: &IfcModel, id: Id) -> Option<Vec<DVec3>> {
    let stats = GeomStats::default();
    let g = Geom { m, stats: &stats, eps: 1e-6 / m.length_scale };
    let e = m.get(id)?;
    let pds = g.get(e.arg(6).as_ref()?)?;
    let rep = pds.arg(2).refs().filter_map(|r| g.get(r)).find(|r| r.arg(1).as_str() == Some("Axis"))?;
    let pts = g.curve(rep.arg(3).refs().next()?, 0);
    if pts.len() < 2 {
        return None;
    }
    let monde = DMat4::from_scale(DVec3::splat(m.length_scale)) * g.placement(e.arg(5).as_ref(), 0);
    Some(pts.into_iter().map(|p| monde.transform_point3(p)).collect())
}

fn bbox(tris: &[Tri]) -> (DVec3, DVec3) {
    let mut mn = DVec3::splat(f64::MAX);
    let mut mx = DVec3::splat(f64::MIN);
    for t in tris {
        for v in t {
            mn = mn.min(*v);
            mx = mx.max(*v);
        }
    }
    (mn, mx)
}

fn default_color(m: &IfcModel, id: Id, ty: &str) -> Rgba {
    if let Some(c) = m.material_of.get(&id).and_then(|mat| m.material_rgba(*mat)) {
        return c;
    }
    class_color(ty)
}

pub fn class_color(ty: &str) -> Rgba {
    match ty {
        "IFCWALL" | "IFCWALLSTANDARDCASE" | "IFCWALLELEMENTEDCASE" => [0.86, 0.85, 0.82, 1.0],
        "IFCSLAB" | "IFCSLABSTANDARDCASE" | "IFCSLABELEMENTEDCASE" => [0.72, 0.72, 0.72, 1.0],
        "IFCROOF" => [0.62, 0.36, 0.30, 1.0],
        "IFCWINDOW" | "IFCWINDOWSTANDARDCASE" => [0.55, 0.75, 0.90, 0.45],
        "IFCDOOR" | "IFCDOORSTANDARDCASE" => [0.62, 0.47, 0.33, 1.0],
        "IFCCOLUMN" | "IFCCOLUMNSTANDARDCASE" | "IFCBEAM" | "IFCBEAMSTANDARDCASE" | "IFCMEMBER"
        | "IFCMEMBERSTANDARDCASE" | "IFCFOOTING" | "IFCPILE" => [0.60, 0.62, 0.66, 1.0],
        "IFCPLATE" | "IFCPLATESTANDARDCASE" => [0.60, 0.78, 0.88, 0.5],
        "IFCCURTAINWALL" => [0.60, 0.78, 0.88, 0.5],
        "IFCSTAIR" | "IFCSTAIRFLIGHT" | "IFCRAMP" | "IFCRAMPFLIGHT" => [0.66, 0.64, 0.60, 1.0],
        "IFCRAILING" => [0.40, 0.40, 0.42, 1.0],
        "IFCSPACE" => [0.35, 0.60, 0.90, 0.22],
        "IFCOPENINGELEMENT" | "IFCOPENINGSTANDARDCASE" | "IFCVOIDINGFEATURE" => [0.95, 0.35, 0.30, 0.30],
        "IFCFURNISHINGELEMENT" | "IFCFURNITURE" => [0.78, 0.63, 0.45, 1.0],
        "IFCSITE" | "IFCGEOGRAPHICELEMENT" => [0.48, 0.62, 0.38, 1.0],
        "IFCCOVERING" => [0.90, 0.88, 0.80, 1.0],
        "IFCFLOWSEGMENT" | "IFCPIPESEGMENT" | "IFCDUCTSEGMENT" | "IFCFLOWFITTING" | "IFCPIPEFITTING"
        | "IFCDUCTFITTING" => [0.45, 0.62, 0.72, 1.0],
        _ => [0.78, 0.78, 0.76, 1.0],
    }
}

impl<'a> Geom<'a> {
    fn get(&self, id: Id) -> Option<&'a Entity> {
        self.m.get(id)
    }

    fn deref(&self, e: &Entity, i: usize) -> Option<&'a Entity> {
        self.get(e.arg(i).as_ref()?)
    }

    // ------------------------------------------------------------ Repères

    fn point(&self, id: Option<Id>) -> Option<DVec3> {
        let e = self.get(id?)?;
        let c = e.arg(0).as_list();
        Some(DVec3::new(
            c.first()?.as_f64()?,
            c.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
            c.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0),
        ))
    }

    fn direction(&self, id: Option<Id>) -> Option<DVec3> {
        let d = self.point(id)?;
        let l = d.length();
        (l > 1e-12).then(|| d / l)
    }

    /// IfcAxis2Placement3D / IfcAxis2Placement2D -> matrice.
    fn axis2(&self, id: Option<Id>) -> DMat4 {
        let Some(e) = id.and_then(|i| self.get(i)) else { return DMat4::IDENTITY };
        let loc = self.point(e.arg(0).as_ref()).unwrap_or_default();
        let (z, x) = if e.is("IFCAXIS2PLACEMENT2D") {
            (DVec3::Z, self.direction(e.arg(1).as_ref()).unwrap_or(DVec3::X))
        } else {
            (
                self.direction(e.arg(1).as_ref()).unwrap_or(DVec3::Z),
                self.direction(e.arg(2).as_ref()).unwrap_or(DVec3::X),
            )
        };
        frame(loc, z, x)
    }

    /// IfcLocalPlacement (récursif).
    fn placement(&self, id: Option<Id>, depth: u32) -> DMat4 {
        let Some(e) = id.and_then(|i| self.get(i)) else { return DMat4::IDENTITY };
        if depth > 64 {
            return DMat4::IDENTITY;
        }
        match &*e.ty {
            "IFCLOCALPLACEMENT" => {
                let parent = self.placement(e.arg(0).as_ref(), depth + 1);
                parent * self.axis2(e.arg(1).as_ref())
            }
            _ => {
                self.stats.miss(&e.ty);
                DMat4::IDENTITY
            }
        }
    }

    /// IfcCartesianTransformationOperator3D (uniforme ou non) et 2D.
    fn transform_operator(&self, id: Option<Id>) -> DMat4 {
        let Some(e) = id.and_then(|i| self.get(i)) else { return DMat4::IDENTITY };
        let x = self.direction(e.arg(0).as_ref()).unwrap_or(DVec3::X);
        let origin = self.point(e.arg(2).as_ref()).unwrap_or_default();
        let s = e.arg(3).as_f64().unwrap_or(1.0);
        let is3d = e.ty.contains("3D");
        let z = if is3d { self.direction(e.arg(4).as_ref()).unwrap_or(DVec3::Z) } else { DVec3::Z };
        let (s2, s3) = if e.ty.ends_with("NONUNIFORM") {
            (e.arg(5).as_f64().unwrap_or(s), e.arg(6).as_f64().unwrap_or(s))
        } else {
            (s, s)
        };
        let mut f = frame(origin, z, x);
        if !is3d {
            if let Some(y) = self.direction(e.arg(1).as_ref()) {
                // Repère 2D éventuellement miroir.
                if f.y_axis.truncate().dot(y) < 0.0 {
                    f.y_axis = -f.y_axis;
                }
            }
        }
        f * DMat4::from_scale(DVec3::new(s, s2, s3))
    }

    // ------------------------------------------------------------ Représentations

    /// Items de la représentation "Body" (ou à défaut 3D) d'un IfcProductDefinitionShape.
    fn body_items(&self, pds: Id) -> Vec<Id> {
        let Some(pds) = self.get(pds) else { return vec![] };
        let reps: Vec<&Entity> = pds.arg(2).refs().filter_map(|r| self.get(r)).collect();
        let ident = |r: &Entity| r.arg(1).as_str().unwrap_or("").to_string();
        let kind = |r: &Entity| r.arg(2).as_str().unwrap_or("").to_string();
        let mut chosen: Vec<&Entity> = reps.iter().copied().filter(|r| ident(r) == "Body").collect();
        if chosen.is_empty() {
            chosen = reps.iter().copied().filter(|r| ident(r) == "Facetation").collect();
        }
        if chosen.is_empty() {
            const SOLID: &[&str] = &[
                "SweptSolid", "AdvancedSweptSolid", "Brep", "AdvancedBrep", "SolidModel", "SurfaceModel",
                "Tessellation", "Clipping", "CSG", "MappedRepresentation",
            ];
            chosen = reps.iter().copied().filter(|r| SOLID.contains(&kind(r).as_str())).collect();
        }
        chosen.iter().flat_map(|r| r.arg(3).refs()).collect()
    }

    fn item(&self, id: Id, m: DMat4, inherited: Option<Rgba>, default: Rgba, parts: &mut Vec<Part>, depth: u32) {
        if depth > 16 {
            return;
        }
        let Some(e) = self.get(id) else { return };
        let color = self.m.item_color.get(&id).copied().or(inherited);
        if e.is("IFCMAPPEDITEM") {
            let Some(src) = self.deref(e, 0) else { return };
            let target = self.transform_operator(e.arg(1).as_ref());
            let origin = self.axis2(src.arg(0).as_ref());
            let mm = m * target * origin;
            let Some(rep) = self.deref(src, 1) else { return };
            for it in rep.arg(3).refs() {
                self.item(it, mm, color, default, parts, depth + 1);
            }
            return;
        }
        let Some(tris) = self.solid(id, m, depth) else { return };
        if tris.is_empty() {
            return;
        }
        let c = color.unwrap_or(default);
        match parts.iter_mut().find(|p| p.color == c) {
            Some(p) => p.tris.extend(tris),
            None => parts.push(Part { color: c, tris }),
        }
    }

    /// Géométrie solide/surfacique d'un item, dans le repère `m`.
    fn solid(&self, id: Id, m: DMat4, depth: u32) -> Option<Vec<Tri>> {
        if depth > 16 {
            return None;
        }
        let e = self.get(id)?;
        let mut out: Vec<Tri> = Vec::new();
        match &*e.ty {
            "IFCEXTRUDEDAREASOLID" | "IFCEXTRUDEDAREASOLIDTAPERED" => {
                let profs = self.profile(e.arg(0).as_ref()?, 0);
                let pos = m * self.axis2(e.arg(1).as_ref());
                let dir = self.direction(e.arg(2).as_ref()).unwrap_or(DVec3::Z);
                let depth = e.arg(3).as_f64()?;
                for p in &profs {
                    extrude(p, pos, dir * depth, &mut out);
                }
                csg::fix_orientation(&mut out);
            }
            "IFCREVOLVEDAREASOLID" | "IFCREVOLVEDAREASOLIDTAPERED" => {
                let profs = self.profile(e.arg(0).as_ref()?, 0);
                let pos = m * self.axis2(e.arg(1).as_ref());
                let ax = self.get(e.arg(2).as_ref()?)?;
                let ap = self.point(ax.arg(0).as_ref()).unwrap_or_default();
                let ad = self.direction(ax.arg(1).as_ref()).unwrap_or(DVec3::Z);
                let angle = e.arg(3).as_f64()? * self.m.angle_scale;
                for p in &profs {
                    revolve(p, pos, ap, ad, angle, &mut out);
                }
                csg::fix_orientation(&mut out);
            }
            "IFCFACETEDBREP" | "IFCFACETEDBREPWITHVOIDS" | "IFCADVANCEDBREP" | "IFCADVANCEDBREPWITHVOIDS" => {
                self.shell(e.arg(0).as_ref()?, m, &mut out);
                csg::fix_orientation(&mut out);
            }
            "IFCCLOSEDSHELL" | "IFCOPENSHELL" | "IFCCONNECTEDFACESET" => self.shell(id, m, &mut out),
            "IFCSHELLBASEDSURFACEMODEL" | "IFCFACEBASEDSURFACEMODEL" => {
                for s in e.arg(0).refs() {
                    self.shell(s, m, &mut out);
                }
            }
            "IFCTRIANGULATEDFACESET" | "IFCTRIANGULATEDIRREGULARNETWORK" => self.tri_face_set(e, m, &mut out),
            "IFCPOLYGONALFACESET" => self.poly_face_set(e, m, &mut out),
            "IFCBOOLEANRESULT" | "IFCBOOLEANCLIPPINGRESULT" => return self.boolean(e, m, depth),
            "IFCCSGSOLID" => return self.solid(e.arg(0).as_ref()?, m, depth + 1),
            "IFCBLOCK" => {
                let pos = m * self.axis2(e.arg(0).as_ref());
                let (x, y, z) = (e.arg(1).as_f64()?, e.arg(2).as_f64()?, e.arg(3).as_f64()?);
                let p = Profile { outer: vec![DVec2::ZERO, DVec2::new(x, 0.0), DVec2::new(x, y), DVec2::new(0.0, y)], holes: vec![] };
                extrude(&p, pos, DVec3::new(0.0, 0.0, z), &mut out);
                csg::fix_orientation(&mut out);
            }
            "IFCRIGHTCIRCULARCYLINDER" => {
                let pos = m * self.axis2(e.arg(0).as_ref());
                let (h, r) = (e.arg(1).as_f64()?, e.arg(2).as_f64()?);
                let p = Profile { outer: circle(r, r, CIRCLE_SEGMENTS as usize), holes: vec![] };
                extrude(&p, pos, DVec3::new(0.0, 0.0, h), &mut out);
                csg::fix_orientation(&mut out);
            }
            "IFCSPHERE" => {
                let pos = m * self.axis2(e.arg(0).as_ref());
                sphere(e.arg(1).as_f64()?, pos, &mut out);
            }
            "IFCSWEPTDISKSOLID" | "IFCSWEPTDISKSOLIDPOLYGONAL" => {
                let pts = self.curve(e.arg(0).as_ref()?, 0);
                let r = e.arg(1).as_f64()?;
                let pts: Vec<DVec3> = pts.iter().map(|p| m.transform_point3(*p)).collect();
                let scale = m.x_axis.truncate().length();
                sweep_disk(&pts, r * scale, &mut out);
            }
            "IFCMAPPEDITEM" => {
                let src = self.deref(e, 0)?;
                let mm = m * self.transform_operator(e.arg(1).as_ref()) * self.axis2(src.arg(0).as_ref());
                for it in self.deref(src, 1)?.arg(3).refs() {
                    if let Some(t) = self.solid(it, mm, depth + 1) {
                        out.extend(t);
                    }
                }
            }
            // Courbes, annotations, ensembles géométriques : non volumiques.
            "IFCPOLYLINE" | "IFCINDEXEDPOLYCURVE" | "IFCCOMPOSITECURVE" | "IFCTRIMMEDCURVE" | "IFCGEOMETRICSET"
            | "IFCGEOMETRICCURVESET" | "IFCANNOTATIONFILLAREA" | "IFCTEXTLITERAL" | "IFCTEXTLITERALWITHEXTENT"
            | "IFCCARTESIANPOINT" | "IFCLINE" | "IFCCIRCLE" | "IFCBOUNDINGBOX" | "IFCPOLYLOOP" => return None,
            _ => {
                self.stats.miss(&e.ty);
                return None;
            }
        }
        Some(out)
    }

    fn boolean(&self, e: &Entity, m: DMat4, depth: u32) -> Option<Vec<Tri>> {
        let op = match e.arg(0).as_enum() {
            Some("UNION") => Op::Union,
            Some("INTERSECTION") => Op::Intersection,
            _ => Op::Difference,
        };
        let mut first = self.solid(e.arg(1).as_ref()?, m, depth + 1)?;
        if first.is_empty() || first.len() > CSG_MAX_HOST_TRIS {
            return Some(first);
        }
        let Some(sid) = e.arg(2).as_ref() else { return Some(first) };
        let Some(se) = self.get(sid) else { return Some(first) };
        let second = match &*se.ty {
            "IFCHALFSPACESOLID" | "IFCPOLYGONALBOUNDEDHALFSPACE" | "IFCBOXEDHALFSPACE" => {
                self.half_space(se, m, &first)
            }
            _ => self.solid(sid, m, depth + 1),
        };
        let Some(mut second) = second else { return Some(first) };
        if second.is_empty() || second.len() > CSG_MAX_OPENING_TRIS {
            return Some(first);
        }
        csg::fix_orientation(&mut first);
        csg::fix_orientation(&mut second);
        let (amin, amax) = bbox(&first);
        let (bmin, bmax) = bbox(&second);
        let disjoint = bmin.cmpgt(amax).any() || bmax.cmplt(amin).any();
        if disjoint {
            return Some(match op {
                Op::Difference => first,
                Op::Intersection => vec![],
                Op::Union => {
                    first.extend(second);
                    first
                }
            });
        }
        Some(csg::boolean(&first, &second, op, self.eps).unwrap_or(first))
    }

    /// Demi-espace matérialisé par un grand bloc (borné par un prisme le cas échéant).
    fn half_space(&self, e: &Entity, m: DMat4, operand: &[Tri]) -> Option<Vec<Tri>> {
        let plane = self.deref(e, 0)?;
        if !plane.is("IFCPLANE") {
            self.stats.miss(&plane.ty);
            return None;
        }
        let pm = m * self.axis2(plane.arg(0).as_ref());
        let agreement = e.arg(1).as_bool().unwrap_or(true);
        let inv = pm.inverse();
        // Taille : dépasse largement l'opérande dans le repère du plan.
        let (mut mn, mut mx) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for t in operand {
            for v in t {
                let l = inv.transform_point3(*v);
                mn = mn.min(l);
                mx = mx.max(l);
            }
        }
        let size = (mx - mn).length().max(1e-6) * 2.0 + 1.0 / self.m.length_scale;
        let c = (mn + mx) * 0.5;
        let (z0, z1) = if agreement { (-size - c.z.abs(), 0.0) } else { (0.0, size + c.z.abs()) };
        let rect = Profile {
            outer: vec![
                DVec2::new(c.x - size, c.y - size),
                DVec2::new(c.x + size, c.y - size),
                DVec2::new(c.x + size, c.y + size),
                DVec2::new(c.x - size, c.y + size),
            ],
            holes: vec![],
        };
        let mut block = Vec::new();
        extrude(&rect, pm * DMat4::from_translation(DVec3::new(0.0, 0.0, z0)), DVec3::new(0.0, 0.0, z1 - z0), &mut block);
        csg::fix_orientation(&mut block);
        if !e.is("IFCPOLYGONALBOUNDEDHALFSPACE") {
            return Some(block);
        }
        // Prisme de la frontière polygonale, extrudé selon Z de Position.
        let pos = m * self.axis2(e.arg(2).as_ref());
        let pts = self.curve(e.arg(3).as_ref()?, 0);
        let outer = clean_ring(pts.iter().map(|p| p.truncate()).collect());
        if outer.len() < 3 {
            return Some(block);
        }
        let mut prism = Vec::new();
        let big = size * 4.0;
        extrude(
            &Profile { outer, holes: vec![] },
            pos * DMat4::from_translation(DVec3::new(0.0, 0.0, -big)),
            DVec3::new(0.0, 0.0, 2.0 * big),
            &mut prism,
        );
        csg::fix_orientation(&mut prism);
        csg::boolean(&prism, &block, Op::Intersection, self.eps)
    }

    // ------------------------------------------------------------ Faces

    fn shell(&self, id: Id, m: DMat4, out: &mut Vec<Tri>) {
        let Some(e) = self.get(id) else { return };
        for f in e.arg(0).refs() {
            let Some(face) = self.get(f) else { continue };
            let mut outer: Option<Vec<DVec3>> = None;
            let mut holes: Vec<Vec<DVec3>> = Vec::new();
            for b in face.arg(0).refs() {
                let Some(bound) = self.get(b) else { continue };
                let mut pts = self.loop_points(bound.arg(0).as_ref());
                if bound.arg(1).as_bool() == Some(false) {
                    pts.reverse();
                }
                if pts.len() < 3 {
                    continue;
                }
                let pts: Vec<DVec3> = pts.iter().map(|p| m.transform_point3(*p)).collect();
                if bound.is("IFCFACEOUTERBOUND") && outer.is_none() {
                    outer = Some(pts);
                } else {
                    holes.push(pts);
                }
            }
            let outer = match outer {
                Some(o) => o,
                None if !holes.is_empty() => holes.remove(0),
                None => continue,
            };
            // IfcFaceSurface/IfcAdvancedFace.SameSense
            if (face.is("IFCADVANCEDFACE") || face.is("IFCFACESURFACE")) && face.arg(2).as_bool() == Some(false) {
                let mut o = outer;
                o.reverse();
                triangulate_3d(&o, &holes, out);
            } else {
                triangulate_3d(&outer, &holes, out);
            }
        }
    }

    fn loop_points(&self, id: Option<Id>) -> Vec<DVec3> {
        let Some(e) = id.and_then(|i| self.get(i)) else { return vec![] };
        match &*e.ty {
            "IFCPOLYLOOP" => e.arg(0).refs().filter_map(|p| self.point(Some(p))).collect(),
            "IFCEDGELOOP" => {
                let mut pts = Vec::new();
                for oe in e.arg(0).refs() {
                    let Some(oe) = self.get(oe) else { continue };
                    let Some(edge) = self.deref(oe, 2) else { continue };
                    let forward = oe.arg(3).as_bool().unwrap_or(true);
                    let mut seg = self.edge_points(edge);
                    if !forward {
                        seg.reverse();
                    }
                    if !seg.is_empty() {
                        seg.pop();
                    }
                    pts.extend(seg);
                }
                pts
            }
            _ => {
                self.stats.miss(&e.ty);
                vec![]
            }
        }
    }

    /// Points d'une IfcEdgeCurve, du début à la fin (sans courbure si la géométrie est inconnue).
    fn edge_points(&self, edge: &Entity) -> Vec<DVec3> {
        let vp = |i: usize| self.deref(edge, i).and_then(|v| self.point(v.arg(0).as_ref()));
        let (Some(a), Some(b)) = (vp(0), vp(1)) else { return vec![] };
        if edge.is("IFCEDGECURVE") {
            if let Some(c) = edge.arg(2).as_ref() {
                let ty = self.m.ty(c);
                if matches!(ty, "IFCPOLYLINE" | "IFCBSPLINECURVEWITHKNOTS" | "IFCRATIONALBSPLINECURVEWITHKNOTS" | "IFCINDEXEDPOLYCURVE") {
                    let mut pts = self.curve(c, 0);
                    if edge.arg(3).as_bool() == Some(false) {
                        pts.reverse();
                    }
                    if pts.len() >= 2 {
                        let n = pts.len();
                        pts[0] = a;
                        pts[n - 1] = b;
                        return pts;
                    }
                }
                if ty == "IFCCIRCLE" || ty == "IFCELLIPSE" {
                    if let Some(pts) = self.conic_between(c, a, b, edge.arg(3).as_bool().unwrap_or(true)) {
                        return pts;
                    }
                }
            }
        }
        vec![a, b]
    }

    fn tri_face_set(&self, e: &Entity, m: DMat4, out: &mut Vec<Tri>) {
        let Some(coords) = self.deref(e, 0) else { return };
        let pts: Vec<DVec3> = coords.arg(0).as_list().iter().map(|c| m.transform_point3(list_vec3(c))).collect();
        // PnIndex (IFC4 ADD2) : liste d'entiers ; sinon index direct.
        let pn: Vec<usize> = e.arg(4).as_list().iter().filter_map(|v| v.as_i64()).map(|v| v as usize).collect();
        let get = |i: i64| -> Option<DVec3> {
            let i = i as usize;
            let k = if pn.is_empty() { i } else { *pn.get(i.checked_sub(1)?)? };
            pts.get(k.checked_sub(1)?).copied()
        };
        for t in e.arg(3).as_list() {
            let l = t.as_list();
            if l.len() < 3 {
                continue;
            }
            if let (Some(a), Some(b), Some(c)) = (
                l[0].as_i64().and_then(get),
                l[1].as_i64().and_then(get),
                l[2].as_i64().and_then(get),
            ) {
                out.push([a, b, c]);
            }
        }
    }

    fn poly_face_set(&self, e: &Entity, m: DMat4, out: &mut Vec<Tri>) {
        let Some(coords) = self.deref(e, 0) else { return };
        let pts: Vec<DVec3> = coords.arg(0).as_list().iter().map(|c| m.transform_point3(list_vec3(c))).collect();
        let pn: Vec<usize> = e.arg(3).as_list().iter().filter_map(|v| v.as_i64()).map(|v| v as usize).collect();
        let resolve = |l: &[Value]| -> Vec<DVec3> {
            l.iter()
                .filter_map(|v| {
                    let i = v.as_i64()? as usize;
                    let k = if pn.is_empty() { i } else { *pn.get(i.checked_sub(1)?)? };
                    pts.get(k.checked_sub(1)?).copied()
                })
                .collect()
        };
        for f in e.arg(2).refs() {
            let Some(face) = self.get(f) else { continue };
            let outer = resolve(face.arg(0).as_list());
            let holes: Vec<Vec<DVec3>> = if face.is("IFCINDEXEDPOLYGONALFACEWITHVOIDS") {
                face.arg(1).as_list().iter().map(|h| resolve(h.as_list())).collect()
            } else {
                vec![]
            };
            triangulate_3d(&outer, &holes, out);
        }
    }

    // ------------------------------------------------------------ Profils

    fn profile(&self, id: Id, depth: u32) -> Vec<Profile> {
        let Some(e) = self.get(id) else { return vec![] };
        if depth > 8 {
            return vec![];
        }
        let f = |i: usize| e.arg(i).as_f64();
        let pos = || self.axis2(e.arg(2).as_ref());
        let simple = |outer: Vec<DVec2>, holes: Vec<Vec<DVec2>>| -> Vec<Profile> {
            let m = pos();
            let tr = |r: Vec<DVec2>| r.into_iter().map(|p| m.transform_point3(p.extend(0.0)).truncate()).collect();
            vec![Profile { outer: tr(outer), holes: holes.into_iter().map(tr).collect() }]
        };
        match &*e.ty {
            "IFCRECTANGLEPROFILEDEF" | "IFCROUNDEDRECTANGLEPROFILEDEF" => {
                let (Some(x), Some(y)) = (f(3), f(4)) else { return vec![] };
                simple(rect(x, y), vec![])
            }
            "IFCRECTANGLEHOLLOWPROFILEDEF" => {
                let (Some(x), Some(y), Some(t)) = (f(3), f(4), f(5)) else { return vec![] };
                let mut inner = rect(x - 2.0 * t, y - 2.0 * t);
                inner.reverse();
                simple(rect(x, y), vec![inner])
            }
            "IFCCIRCLEPROFILEDEF" => {
                let Some(r) = f(3) else { return vec![] };
                simple(circle(r, r, CIRCLE_SEGMENTS as usize), vec![])
            }
            "IFCCIRCLEHOLLOWPROFILEDEF" => {
                let (Some(r), Some(t)) = (f(3), f(4)) else { return vec![] };
                let mut inner = circle(r - t, r - t, CIRCLE_SEGMENTS as usize);
                inner.reverse();
                simple(circle(r, r, CIRCLE_SEGMENTS as usize), vec![inner])
            }
            "IFCELLIPSEPROFILEDEF" => {
                let (Some(a), Some(b)) = (f(3), f(4)) else { return vec![] };
                simple(circle(a, b, CIRCLE_SEGMENTS as usize), vec![])
            }
            "IFCISHAPEPROFILEDEF" | "IFCASYMMETRICISHAPEPROFILEDEF" => {
                let (Some(w), Some(d), Some(tw), Some(tf)) = (f(3), f(4), f(5), f(6)) else { return vec![] };
                let (x, y) = (w / 2.0, d / 2.0);
                let t = tw / 2.0;
                simple(
                    pts(&[
                        (-x, -y), (x, -y), (x, -y + tf), (t, -y + tf), (t, y - tf), (x, y - tf),
                        (x, y), (-x, y), (-x, y - tf), (-t, y - tf), (-t, -y + tf), (-x, -y + tf),
                    ]),
                    vec![],
                )
            }
            "IFCLSHAPEPROFILEDEF" => {
                let (Some(d), Some(t)) = (f(3), f(5)) else { return vec![] };
                let w = f(4).unwrap_or(d);
                let (x, y) = (w / 2.0, d / 2.0);
                simple(pts(&[(-x, -y), (x, -y), (x, -y + t), (-x + t, -y + t), (-x + t, y), (-x, y)]), vec![])
            }
            "IFCUSHAPEPROFILEDEF" => {
                let (Some(d), Some(b), Some(tw), Some(tf)) = (f(3), f(4), f(5), f(6)) else { return vec![] };
                let (x, y) = (b / 2.0, d / 2.0);
                simple(
                    pts(&[(-x, -y), (x, -y), (x, -y + tf), (-x + tw, -y + tf), (-x + tw, y - tf), (x, y - tf), (x, y), (-x, y)]),
                    vec![],
                )
            }
            "IFCTSHAPEPROFILEDEF" => {
                let (Some(d), Some(b), Some(tw), Some(tf)) = (f(3), f(4), f(5), f(6)) else { return vec![] };
                let (x, y, t) = (b / 2.0, d / 2.0, tw / 2.0);
                simple(
                    pts(&[(-t, -y), (t, -y), (t, y - tf), (x, y - tf), (x, y), (-x, y), (-x, y - tf), (-t, y - tf)]),
                    vec![],
                )
            }
            "IFCCSHAPEPROFILEDEF" => {
                let (Some(d), Some(w), Some(t), Some(g)) = (f(3), f(4), f(5), f(6)) else { return vec![] };
                let (x, y) = (w / 2.0, d / 2.0);
                simple(
                    pts(&[
                        (-x, -y), (x, -y), (x, -y + g), (x - t, -y + g), (x - t, -y + t), (-x + t, -y + t),
                        (-x + t, y - t), (x - t, y - t), (x - t, y - g), (x, y - g), (x, y), (-x, y),
                    ]),
                    vec![],
                )
            }
            "IFCZSHAPEPROFILEDEF" => {
                let (Some(d), Some(b), Some(tw), Some(tf)) = (f(3), f(4), f(5), f(6)) else { return vec![] };
                let (y, t) = (d / 2.0, tw / 2.0);
                simple(
                    pts(&[(-t, -y), (b - t, -y), (b - t, -y + tf), (t, -y + tf), (t, y), (-b + t, y), (-b + t, y - tf), (-t, y - tf)]),
                    vec![],
                )
            }
            "IFCARBITRARYCLOSEDPROFILEDEF" | "IFCARBITRARYPROFILEDEFWITHVOIDS" => {
                let outer = clean_ring(self.curve(e.arg(2).as_ref().unwrap_or(0), 0).iter().map(|p| p.truncate()).collect());
                if outer.len() < 3 {
                    return vec![];
                }
                let holes = e
                    .arg(3)
                    .refs()
                    .map(|h| clean_ring(self.curve(h, 0).iter().map(|p| p.truncate()).collect()))
                    .filter(|h| h.len() >= 3)
                    .collect();
                vec![Profile { outer, holes }]
            }
            "IFCDERIVEDPROFILEDEF" => {
                let op = self.transform_operator(e.arg(3).as_ref());
                let mut profs = self.profile(e.arg(2).as_ref().unwrap_or(0), depth + 1);
                for p in &mut profs {
                    let tr = |v: &mut DVec2| *v = op.transform_point3(v.extend(0.0)).truncate();
                    p.outer.iter_mut().for_each(tr);
                    p.holes.iter_mut().flatten().for_each(tr);
                }
                profs
            }
            "IFCCOMPOSITEPROFILEDEF" => e.arg(2).refs().flat_map(|p| self.profile(p, depth + 1)).collect(),
            _ => {
                self.stats.miss(&e.ty);
                vec![]
            }
        }
    }

    // ------------------------------------------------------------ Courbes

    /// Points d'une courbe (2D -> z = 0) dans son repère.
    fn curve(&self, id: Id, depth: u32) -> Vec<DVec3> {
        let Some(e) = self.get(id) else { return vec![] };
        if depth > 16 {
            return vec![];
        }
        match &*e.ty {
            "IFCPOLYLINE" => e.arg(0).refs().filter_map(|p| self.point(Some(p))).collect(),
            "IFCINDEXEDPOLYCURVE" => {
                let Some(pl) = self.deref(e, 0) else { return vec![] };
                let all: Vec<DVec3> = pl.arg(0).as_list().iter().map(list_vec3).collect();
                let segs = e.arg(1).as_list();
                if segs.is_empty() {
                    return all;
                }
                let mut out: Vec<DVec3> = Vec::new();
                let at = |i: &Value| i.as_i64().and_then(|i| all.get((i as usize).checked_sub(1)?).copied());
                for s in segs {
                    let (name, idx) = match s {
                        Value::Typed(n, v) => (n.as_str(), v.as_list()),
                        _ => continue,
                    };
                    let p: Vec<DVec3> = idx.iter().filter_map(at).collect();
                    let seg = if name == "IFCARCINDEX" && p.len() == 3 { arc3(p[0], p[1], p[2]) } else { p };
                    push_points(&mut out, seg);
                }
                out
            }
            "IFCCOMPOSITECURVE" | "IFCCOMPOSITECURVEONSURFACE" => {
                let mut out = Vec::new();
                for s in e.arg(0).refs() {
                    let Some(seg) = self.get(s) else { continue };
                    let mut p = self.curve(seg.arg(2).as_ref().unwrap_or(0), depth + 1);
                    if seg.arg(1).as_bool() == Some(false) {
                        p.reverse();
                    }
                    push_points(&mut out, p);
                }
                out
            }
            "IFCTRIMMEDCURVE" => self.trimmed(e).unwrap_or_default(),
            "IFCCIRCLE" | "IFCELLIPSE" => {
                let m = self.axis2(e.arg(0).as_ref());
                let a = e.arg(1).as_f64().unwrap_or(0.0);
                let b = if e.is("IFCELLIPSE") { e.arg(2).as_f64().unwrap_or(a) } else { a };
                circle(a, b, CIRCLE_SEGMENTS as usize).iter().map(|p| m.transform_point3(p.extend(0.0))).collect()
            }
            "IFCBSPLINECURVEWITHKNOTS" | "IFCRATIONALBSPLINECURVEWITHKNOTS" => self.bspline(e).unwrap_or_default(),
            _ => {
                self.stats.miss(&e.ty);
                vec![]
            }
        }
    }

    fn trimmed(&self, e: &Entity) -> Option<Vec<DVec3>> {
        let basis = self.deref(e, 0)?;
        let sense = e.arg(3).as_bool().unwrap_or(true);
        let trim_point = |v: &Value| v.as_list().iter().find_map(|t| t.as_ref()).and_then(|r| self.point(Some(r)));
        let trim_param = |v: &Value| v.as_list().iter().find_map(|t| match t {
            Value::Typed(n, x) if n == "IFCPARAMETERVALUE" => x.as_f64(),
            Value::Real(x) => Some(*x),
            _ => None,
        });
        let prefer_cart = e.arg(4).as_enum() != Some("PARAMETER");
        match &*basis.ty {
            "IFCCIRCLE" | "IFCELLIPSE" => {
                let m = self.axis2(basis.arg(0).as_ref());
                let a = basis.arg(1).as_f64()?;
                let b = if basis.is("IFCELLIPSE") { basis.arg(2).as_f64()? } else { a };
                let inv = m.inverse();
                let angle = |v: &Value| -> Option<f64> {
                    let cart = trim_point(v).map(|p| {
                        let l = inv.transform_point3(p);
                        (l.y / b).atan2(l.x / a)
                    });
                    let par = trim_param(v).map(|t| t * self.m.angle_scale);
                    if prefer_cart { cart.or(par) } else { par.or(cart) }
                };
                let (t1, mut t2) = (angle(e.arg(1))?, angle(e.arg(2))?);
                if sense {
                    while t2 <= t1 + 1e-9 {
                        t2 += TAU;
                    }
                } else {
                    while t2 >= t1 - 1e-9 {
                        t2 -= TAU;
                    }
                }
                let n = (((t2 - t1).abs() / TAU) * CIRCLE_SEGMENTS).ceil().max(2.0) as usize;
                Some(
                    (0..=n)
                        .map(|i| {
                            let t = t1 + (t2 - t1) * i as f64 / n as f64;
                            m.transform_point3(DVec3::new(a * t.cos(), b * t.sin(), 0.0))
                        })
                        .collect(),
                )
            }
            "IFCLINE" => {
                let p0 = self.point(basis.arg(0).as_ref())?;
                let vec = self.deref(basis, 1)?;
                let d = self.direction(vec.arg(0).as_ref())? * vec.arg(1).as_f64().unwrap_or(1.0);
                let at = |v: &Value| -> Option<DVec3> {
                    let cart = trim_point(v);
                    let par = trim_param(v).map(|t| p0 + d * t);
                    if prefer_cart { cart.or(par) } else { par.or(cart) }
                };
                Some(vec![at(e.arg(1))?, at(e.arg(2))?])
            }
            _ => {
                let mut p = self.curve(basis_id(e)?, 1);
                if !sense {
                    p.reverse();
                }
                Some(p)
            }
        }
    }

    /// Arc de cercle/ellipse entre deux sommets (arêtes de B-rep avancé).
    fn conic_between(&self, c: Id, a: DVec3, b: DVec3, same_sense: bool) -> Option<Vec<DVec3>> {
        let ce = self.get(c)?;
        let m = self.axis2(ce.arg(0).as_ref());
        let ra = ce.arg(1).as_f64()?;
        let rb = if ce.is("IFCELLIPSE") { ce.arg(2).as_f64()? } else { ra };
        let inv = m.inverse();
        let ang = |p: DVec3| {
            let l = inv.transform_point3(p);
            (l.y / rb).atan2(l.x / ra)
        };
        let (t1, mut t2) = (ang(a), ang(b));
        if same_sense {
            while t2 <= t1 + 1e-9 {
                t2 += TAU;
            }
        } else {
            while t2 >= t1 - 1e-9 {
                t2 -= TAU;
            }
        }
        let n = (((t2 - t1).abs() / TAU) * CIRCLE_SEGMENTS).ceil().max(1.0) as usize;
        let mut pts: Vec<DVec3> = (0..=n)
            .map(|i| {
                let t = t1 + (t2 - t1) * i as f64 / n as f64;
                m.transform_point3(DVec3::new(ra * t.cos(), rb * t.sin(), 0.0))
            })
            .collect();
        pts[0] = a;
        pts[n] = b;
        Some(pts)
    }

    fn bspline(&self, e: &Entity) -> Option<Vec<DVec3>> {
        let degree = e.arg(0).as_i64()? as usize;
        let ctrl: Vec<DVec3> = e.arg(1).refs().filter_map(|p| self.point(Some(p))).collect();
        let mults: Vec<usize> = e.arg(5).as_list().iter().filter_map(|v| v.as_i64()).map(|v| v as usize).collect();
        let kv: Vec<f64> = e.arg(6).as_list().iter().filter_map(|v| v.as_f64()).collect();
        let weights: Vec<f64> = if e.is("IFCRATIONALBSPLINECURVEWITHKNOTS") {
            e.arg(8).as_list().iter().filter_map(|v| v.as_f64()).collect()
        } else {
            vec![]
        };
        let mut knots = Vec::new();
        for (k, mlt) in kv.iter().zip(&mults) {
            knots.extend(std::iter::repeat_n(*k, *mlt));
        }
        let n = ctrl.len();
        if n < 2 || knots.len() != n + degree + 1 {
            return Some(ctrl);
        }
        let (t0, t1) = (knots[degree], knots[n]);
        let samples = (n * 8).clamp(8, 256);
        let w = |i: usize| weights.get(i).copied().unwrap_or(1.0);
        let mut out = Vec::with_capacity(samples + 1);
        for s in 0..=samples {
            let t = t0 + (t1 - t0) * s as f64 / samples as f64;
            // Intervalle de nœuds
            let mut k = degree;
            while k < n - 1 && knots[k + 1] <= t {
                k += 1;
            }
            // de Boor en coordonnées homogènes
            let mut d: Vec<DVec4> = (0..=degree)
                .map(|j| {
                    let i = (j + k).saturating_sub(degree).min(n - 1);
                    (ctrl[i] * w(i)).extend(w(i))
                })
                .collect();
            for r in 1..=degree {
                for j in (r..=degree).rev() {
                    let i = j + k - degree;
                    let den = knots[i + degree + 1 - r] - knots[i];
                    let a = if den.abs() < 1e-14 { 0.0 } else { (t - knots[i]) / den };
                    d[j] = d[j - 1] * (1.0 - a) + d[j] * a;
                }
            }
            let p = d[degree];
            out.push(if p.w.abs() > 1e-14 { p.truncate() / p.w } else { p.truncate() });
        }
        Some(out)
    }
}

fn basis_id(e: &Entity) -> Option<Id> {
    e.arg(0).as_ref()
}

// ---------------------------------------------------------------- Fonctions géométriques

/// Repère orthonormé à partir de Z et d'une direction X approchée.
fn frame(loc: DVec3, z: DVec3, x: DVec3) -> DMat4 {
    let z = z.normalize_or(DVec3::Z);
    let mut x = x - z * x.dot(z);
    if x.length_squared() < 1e-12 {
        x = if z.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        x -= z * x.dot(z);
    }
    let x = x.normalize();
    let y = z.cross(x);
    DMat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), loc.extend(1.0))
}

fn list_vec3(v: &Value) -> DVec3 {
    let l = v.as_list();
    let g = |i: usize| l.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0);
    DVec3::new(g(0), g(1), g(2))
}

fn pts(p: &[(f64, f64)]) -> Vec<DVec2> {
    p.iter().map(|&(x, y)| DVec2::new(x, y)).collect()
}

fn rect(x: f64, y: f64) -> Vec<DVec2> {
    let (a, b) = (x / 2.0, y / 2.0);
    pts(&[(-a, -b), (a, -b), (a, b), (-a, b)])
}

fn circle(a: f64, b: f64, n: usize) -> Vec<DVec2> {
    (0..n).map(|i| {
        let t = TAU * i as f64 / n as f64;
        DVec2::new(a * t.cos(), b * t.sin())
    }).collect()
}

fn push_points(out: &mut Vec<DVec3>, seg: Vec<DVec3>) {
    for p in seg {
        if out.last().is_none_or(|l| l.distance_squared(p) > 1e-18) {
            out.push(p);
        }
    }
}

/// Arc passant par trois points.
fn arc3(p1: DVec3, p2: DVec3, p3: DVec3) -> Vec<DVec3> {
    let a = p1 - p3;
    let b = p2 - p3;
    let axb = a.cross(b);
    let den = 2.0 * axb.length_squared();
    if den < 1e-18 {
        return vec![p1, p2, p3];
    }
    let c = p3 + (b * a.length_squared() - a * b.length_squared()).cross(axb) / den;
    let r = (p1 - c).length();
    // Normale selon l'ordre p1 -> p2 -> p3 : le parcours direct depuis p1 passe par p2 avant p3.
    let n = (p2 - p1).cross(p3 - p1).normalize();
    let u = (p1 - c) / r;
    let v = n.cross(u);
    let ang = |p: DVec3| {
        let d = p - c;
        let t = d.dot(v).atan2(d.dot(u));
        if t < 0.0 { t + TAU } else { t }
    };
    let total = ang(p3);
    let steps = ((total / TAU) * CIRCLE_SEGMENTS).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let t = total * i as f64 / steps as f64;
            c + (u * t.cos() + v * t.sin()) * r
        })
        .collect()
}

/// Supprime les doublons consécutifs et le point de fermeture.
fn clean_ring(mut r: Vec<DVec2>) -> Vec<DVec2> {
    r.dedup_by(|a, b| a.distance_squared(*b) < 1e-18);
    while r.len() > 1 && r[0].distance_squared(*r.last().unwrap()) < 1e-18 {
        r.pop();
    }
    r
}

fn signed_area(r: &[DVec2]) -> f64 {
    let n = r.len();
    (0..n).map(|i| r[i].perp_dot(r[(i + 1) % n])).sum::<f64>() * 0.5
}

/// Triangulation 2D (anneau extérieur + trous) : indices dans [outer, trous...], triangles CCW.
fn triangulate_2d_idx(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[usize; 3]> {
    let all: Vec<DVec2> = outer.iter().chain(holes.iter().flatten()).copied().collect();
    let orient = |mut t: [usize; 3]| {
        if (all[t[1]] - all[t[0]]).perp_dot(all[t[2]] - all[t[0]]) < 0.0 {
            t.swap(1, 2);
        }
        t
    };
    if outer.len() == 3 && holes.is_empty() {
        return vec![orient([0, 1, 2])];
    }
    let mut flat = Vec::with_capacity(all.len() * 2);
    for p in &all {
        flat.push(p.x);
        flat.push(p.y);
    }
    let mut hole_idx = Vec::new();
    let mut k = outer.len();
    for h in holes {
        hole_idx.push(k);
        k += h.len();
    }
    let Ok(idx) = earcutr::earcut(&flat, &hole_idx, 2) else { return vec![] };
    idx.chunks_exact(3).map(|c| orient([c[0], c[1], c[2]])).collect()
}

fn triangulate_2d(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[DVec2; 3]> {
    let all: Vec<DVec2> = outer.iter().chain(holes.iter().flatten()).copied().collect();
    triangulate_2d_idx(outer, holes).into_iter().map(|t| [all[t[0]], all[t[1]], all[t[2]]]).collect()
}

/// Extrusion d'un profil de `v` (repère local du profil), transformée par `m`.
fn extrude(p: &Profile, m: DMat4, v: DVec3, out: &mut Vec<Tri>) {
    let mut outer = clean_ring(p.outer.clone());
    if outer.len() < 3 {
        return;
    }
    if signed_area(&outer) < 0.0 {
        outer.reverse();
    }
    let holes: Vec<Vec<DVec2>> = p
        .holes
        .iter()
        .map(|h| {
            let mut h = clean_ring(h.clone());
            if signed_area(&h) > 0.0 {
                h.reverse();
            }
            h
        })
        .filter(|h| h.len() >= 3)
        .collect();
    let b = |q: DVec2| m.transform_point3(q.extend(0.0));
    let t = |q: DVec2| m.transform_point3(q.extend(0.0) + v);
    for tri in triangulate_2d(&outer, &holes) {
        out.push([b(tri[0]), b(tri[2]), b(tri[1])]);
        out.push([t(tri[0]), t(tri[1]), t(tri[2])]);
    }
    for ring in std::iter::once(&outer).chain(holes.iter()) {
        let n = ring.len();
        for i in 0..n {
            let (p0, p1) = (ring[i], ring[(i + 1) % n]);
            let (a0, b0, a1, b1) = (b(p0), b(p1), t(p0), t(p1));
            out.push([a0, b0, b1]);
            out.push([a0, b1, a1]);
        }
    }
}

/// Révolution d'un profil autour d'un axe (repère du profil), transformée par `m`.
fn revolve(p: &Profile, m: DMat4, axis_p: DVec3, axis_d: DVec3, angle: f64, out: &mut Vec<Tri>) {
    let angle = if angle.abs() < 1e-9 { TAU } else { angle };
    let full = (angle.abs() - TAU).abs() < 1e-6;
    let steps = ((angle.abs() / TAU) * CIRCLE_SEGMENTS).ceil().max(3.0) as usize;
    let rot = |k: usize| {
        let a = angle * k as f64 / steps as f64;
        m * DMat4::from_translation(axis_p) * DMat4::from_axis_angle(axis_d, a) * DMat4::from_translation(-axis_p)
    };
    let mats: Vec<DMat4> = (0..=steps).map(rot).collect();
    let outer = clean_ring(p.outer.clone());
    let holes: Vec<Vec<DVec2>> = p.holes.iter().map(|h| clean_ring(h.clone())).collect();
    for ring in std::iter::once(&outer).chain(holes.iter()) {
        let n = ring.len();
        for k in 0..steps {
            for i in 0..n {
                let (p0, p1) = (ring[i].extend(0.0), ring[(i + 1) % n].extend(0.0));
                let a0 = mats[k].transform_point3(p0);
                let b0 = mats[k].transform_point3(p1);
                let a1 = mats[k + 1].transform_point3(p0);
                let b1 = mats[k + 1].transform_point3(p1);
                out.push([a0, b0, b1]);
                out.push([a0, b1, a1]);
            }
        }
    }
    if !full {
        for tri in triangulate_2d(&outer, &holes) {
            let s = |q: DVec2| mats[0].transform_point3(q.extend(0.0));
            let e = |q: DVec2| mats[steps].transform_point3(q.extend(0.0));
            out.push([s(tri[0]), s(tri[2]), s(tri[1])]);
            out.push([e(tri[0]), e(tri[1]), e(tri[2])]);
        }
    }
}

fn sphere(r: f64, m: DMat4, out: &mut Vec<Tri>) {
    let (nu, nv) = (24usize, 12usize);
    let p = |i: usize, j: usize| {
        let u = TAU * i as f64 / nu as f64;
        let v = PI * j as f64 / nv as f64 - PI / 2.0;
        m.transform_point3(DVec3::new(r * v.cos() * u.cos(), r * v.cos() * u.sin(), r * v.sin()))
    };
    for i in 0..nu {
        for j in 0..nv {
            let (a, b, c, d) = (p(i, j), p(i + 1, j), p(i + 1, j + 1), p(i, j + 1));
            out.push([a, b, c]);
            out.push([a, c, d]);
        }
    }
}

/// Tube de rayon `r` le long d'une polyligne (repères à transport parallèle).
fn sweep_disk(path: &[DVec3], r: f64, out: &mut Vec<Tri>) {
    let mut path = path.to_vec();
    path.dedup_by(|a, b| a.distance_squared(*b) < 1e-18);
    if path.len() < 2 || r <= 0.0 {
        return;
    }
    let seg = 12usize;
    let n = path.len();
    let tangent = |i: usize| -> DVec3 {
        let a = if i > 0 { (path[i] - path[i - 1]).normalize_or_zero() } else { DVec3::ZERO };
        let b = if i + 1 < n { (path[i + 1] - path[i]).normalize_or_zero() } else { DVec3::ZERO };
        (a + b).normalize_or(b.normalize_or(a))
    };
    let mut t_prev = tangent(0);
    let mut normal = t_prev.any_orthonormal_vector();
    let mut rings: Vec<Vec<DVec3>> = Vec::with_capacity(n);
    for i in 0..n {
        let t = tangent(i);
        // Transport parallèle de la normale.
        let axis = t_prev.cross(t);
        if axis.length_squared() > 1e-18 {
            let ang = t_prev.dot(t).clamp(-1.0, 1.0).acos();
            normal = glam::DQuat::from_axis_angle(axis.normalize(), ang) * normal;
        }
        t_prev = t;
        let bin = t.cross(normal);
        rings.push(
            (0..seg)
                .map(|k| {
                    let a = TAU * k as f64 / seg as f64;
                    path[i] + (normal * a.cos() + bin * a.sin()) * r
                })
                .collect(),
        );
    }
    for i in 0..n - 1 {
        for k in 0..seg {
            let k1 = (k + 1) % seg;
            let (a0, b0, a1, b1) = (rings[i][k], rings[i][k1], rings[i + 1][k], rings[i + 1][k1]);
            out.push([a0, b0, b1]);
            out.push([a0, b1, a1]);
        }
    }
    for (ring, c) in [(&rings[0], path[0]), (&rings[n - 1], path[n - 1])] {
        for k in 0..seg {
            out.push([c, ring[(k + 1) % seg], ring[k]]);
        }
    }
}

/// Triangulation d'un polygone 3D plan (avec trous) en conservant son orientation.
fn triangulate_3d(outer: &[DVec3], holes: &[Vec<DVec3>], out: &mut Vec<Tri>) {
    let n = outer.len();
    if n < 3 {
        return;
    }
    if n == 3 && holes.is_empty() {
        out.push([outer[0], outer[1], outer[2]]);
        return;
    }
    // Normale de Newell
    let mut nrm = DVec3::ZERO;
    for i in 0..n {
        let (a, b) = (outer[i], outer[(i + 1) % n]);
        nrm += DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
    }
    if nrm.length_squared() < 1e-24 {
        return;
    }
    let nrm = nrm.normalize();
    if n == 4 && holes.is_empty() {
        let (a, b, c, d) = (outer[0], outer[1], outer[2], outer[3]);
        // Diagonale qui garde les deux triangles dans le sens de la normale.
        if (b - a).cross(c - a).dot(nrm) > 0.0 && (c - a).cross(d - a).dot(nrm) > 0.0 {
            out.push([a, b, c]);
            out.push([a, c, d]);
        } else {
            out.push([a, b, d]);
            out.push([b, c, d]);
        }
        return;
    }
    let u = nrm.any_orthonormal_vector();
    let v = nrm.cross(u);
    let o = outer[0];
    let to2 = |p: &DVec3| DVec2::new((*p - o).dot(u), (*p - o).dot(v));
    let o2: Vec<DVec2> = outer.iter().map(to2).collect();
    let h2: Vec<Vec<DVec2>> = holes.iter().filter(|h| h.len() >= 3).map(|h| h.iter().map(to2).collect()).collect();
    let all3: Vec<DVec3> = outer.iter().chain(holes.iter().filter(|h| h.len() >= 3).flatten()).copied().collect();
    for t in triangulate_2d_idx(&o2, &h2) {
        out.push([all3[t[0]], all3[t[1]], all3[t[2]]]);
    }
}
