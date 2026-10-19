//! Opérations booléennes sur maillages par arbres BSP (portage de csg.js, Evan Wallace, MIT).
//!
//! Les maillages doivent être fermés et orientés vers l'extérieur.

use glam::DVec3;

pub type Tri = [DVec3; 3];

#[derive(Clone, Copy)]
struct Plane {
    n: DVec3,
    w: f64,
}

impl Plane {
    fn from_points(a: DVec3, b: DVec3, c: DVec3) -> Option<Plane> {
        let n = (b - a).cross(c - a);
        let len = n.length();
        if !(len > 1e-12) {
            return None;
        }
        let n = n / len;
        Some(Plane { n, w: n.dot(a) })
    }
    fn flip(&mut self) {
        self.n = -self.n;
        self.w = -self.w;
    }
}

#[derive(Clone)]
struct Polygon {
    v: Vec<DVec3>,
    plane: Plane,
}

impl Polygon {
    fn flip(&mut self) {
        self.v.reverse();
        self.plane.flip();
    }
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

struct Ctx {
    eps: f64,
    /// Garde-fou contre l'explosion combinatoire.
    budget: usize,
}

fn split(
    ctx: &mut Ctx,
    plane: &Plane,
    poly: Polygon,
    cop_front: &mut Vec<Polygon>,
    cop_back: &mut Vec<Polygon>,
    front: &mut Vec<Polygon>,
    back: &mut Vec<Polygon>,
) {
    let mut ptype = 0u8;
    let types: Vec<u8> = poly
        .v
        .iter()
        .map(|v| {
            let t = plane.n.dot(*v) - plane.w;
            let ty = if t < -ctx.eps {
                BACK
            } else if t > ctx.eps {
                FRONT
            } else {
                COPLANAR
            };
            ptype |= ty;
            ty
        })
        .collect();
    match ptype {
        COPLANAR => {
            if plane.n.dot(poly.plane.n) > 0.0 {
                cop_front.push(poly)
            } else {
                cop_back.push(poly)
            }
        }
        FRONT => front.push(poly),
        BACK => back.push(poly),
        _ => {
            ctx.budget = ctx.budget.saturating_sub(1);
            let n = poly.v.len();
            let mut f = Vec::with_capacity(n + 1);
            let mut b = Vec::with_capacity(n + 1);
            for i in 0..n {
                let j = (i + 1) % n;
                let (ti, tj) = (types[i], types[j]);
                let (vi, vj) = (poly.v[i], poly.v[j]);
                if ti != BACK {
                    f.push(vi);
                }
                if ti != FRONT {
                    b.push(vi);
                }
                if (ti | tj) == SPANNING {
                    let d = plane.n.dot(vj - vi);
                    let t = (plane.w - plane.n.dot(vi)) / d;
                    let v = vi.lerp(vj, t);
                    f.push(v);
                    b.push(v);
                }
            }
            if f.len() >= 3 {
                front.push(Polygon { v: f, plane: poly.plane });
            }
            if b.len() >= 3 {
                back.push(Polygon { v: b, plane: poly.plane });
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polygons: Vec<Polygon>,
}

impl Node {
    fn new(ctx: &mut Ctx, polys: Vec<Polygon>) -> Node {
        let mut n = Node::default();
        n.build(ctx, polys);
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polygons {
            p.flip();
        }
        if let Some(p) = &mut self.plane {
            p.flip();
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polygons(&self, ctx: &mut Ctx, polys: Vec<Polygon>) -> Vec<Polygon> {
        let Some(plane) = self.plane else { return polys };
        if ctx.budget == 0 {
            return polys;
        }
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            split(ctx, &plane, p, &mut cf, &mut cb, &mut front, &mut back);
            front.append(&mut cf);
            back.append(&mut cb);
        }
        let mut front = match &self.front {
            Some(f) => f.clip_polygons(ctx, front),
            None => front,
        };
        let back = match &self.back {
            Some(b) => b.clip_polygons(ctx, back),
            None => Vec::new(),
        };
        front.extend(back);
        front
    }

    fn clip_to(&mut self, ctx: &mut Ctx, bsp: &Node) {
        let polys = std::mem::take(&mut self.polygons);
        self.polygons = bsp.clip_polygons(ctx, polys);
        if let Some(f) = &mut self.front {
            f.clip_to(ctx, bsp);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(ctx, bsp);
        }
    }

    fn all_polygons(&self, out: &mut Vec<Polygon>) {
        out.extend(self.polygons.iter().cloned());
        if let Some(f) = &self.front {
            f.all_polygons(out);
        }
        if let Some(b) = &self.back {
            b.all_polygons(out);
        }
    }

    fn build(&mut self, ctx: &mut Ctx, polys: Vec<Polygon>) {
        if polys.is_empty() {
            return;
        }
        if ctx.budget == 0 {
            self.polygons.extend(polys);
            return;
        }
        let plane = *self.plane.get_or_insert(polys[0].plane);
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            split(ctx, &plane, p, &mut cf, &mut cb, &mut front, &mut back);
            self.polygons.append(&mut cf);
            self.polygons.append(&mut cb);
        }
        if !front.is_empty() {
            self.front.get_or_insert_with(Default::default).build(ctx, front);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(Default::default).build(ctx, back);
        }
    }
}

fn to_polys(tris: &[Tri]) -> Vec<Polygon> {
    tris.iter()
        .filter_map(|t| Some(Polygon { v: t.to_vec(), plane: Plane::from_points(t[0], t[1], t[2])? }))
        .collect()
}

fn to_tris(polys: Vec<Polygon>) -> Vec<Tri> {
    let mut out = Vec::with_capacity(polys.len() * 2);
    for p in polys {
        for i in 1..p.v.len() - 1 {
            out.push([p.v[0], p.v[i], p.v[i + 1]]);
        }
    }
    out
}

#[derive(Clone, Copy)]
pub enum Op {
    Difference,
    Intersection,
    Union,
}

/// Applique `op` ; renvoie None si le calcul dépasse le budget (on garde alors la géométrie d'origine).
pub fn boolean(a: &[Tri], b: &[Tri], op: Op, eps: f64) -> Option<Vec<Tri>> {
    let mut ctx = Ctx { eps, budget: 20_000 + 20 * (a.len() + b.len()) };
    let mut na = Node::new(&mut ctx, to_polys(a));
    let mut nb = Node::new(&mut ctx, to_polys(b));
    match op {
        Op::Difference => {
            na.invert();
            na.clip_to(&mut ctx, &nb);
            nb.clip_to(&mut ctx, &na);
            nb.invert();
            nb.clip_to(&mut ctx, &na);
            nb.invert();
            let mut pb = Vec::new();
            nb.all_polygons(&mut pb);
            na.build(&mut ctx, pb);
            na.invert();
        }
        Op::Intersection => {
            na.invert();
            nb.clip_to(&mut ctx, &na);
            nb.invert();
            na.clip_to(&mut ctx, &nb);
            nb.clip_to(&mut ctx, &na);
            let mut pb = Vec::new();
            nb.all_polygons(&mut pb);
            na.build(&mut ctx, pb);
            na.invert();
        }
        Op::Union => {
            na.clip_to(&mut ctx, &nb);
            nb.clip_to(&mut ctx, &na);
            nb.invert();
            nb.clip_to(&mut ctx, &na);
            nb.invert();
            let mut pb = Vec::new();
            nb.all_polygons(&mut pb);
            na.build(&mut ctx, pb);
        }
    }
    if ctx.budget == 0 {
        return None;
    }
    let mut out = Vec::new();
    na.all_polygons(&mut out);
    Some(to_tris(out))
}

/// Volume signé (positif si les normales pointent vers l'extérieur).
pub fn signed_volume(tris: &[Tri]) -> f64 {
    let c = tris.first().map(|t| t[0]).unwrap_or_default();
    tris.iter().map(|t| (t[0] - c).dot((t[1] - c).cross(t[2] - c))).sum::<f64>() / 6.0
}

pub fn fix_orientation(tris: &mut [Tri]) {
    if signed_volume(tris) < 0.0 {
        for t in tris.iter_mut() {
            t.swap(1, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(min: DVec3, max: DVec3) -> Vec<Tri> {
        let p = |x: usize, y: usize, z: usize| {
            DVec3::new(
                if x == 0 { min.x } else { max.x },
                if y == 0 { min.y } else { max.y },
                if z == 0 { min.z } else { max.z },
            )
        };
        let quads = [
            [p(0, 0, 0), p(0, 1, 0), p(1, 1, 0), p(1, 0, 0)],
            [p(0, 0, 1), p(1, 0, 1), p(1, 1, 1), p(0, 1, 1)],
            [p(0, 0, 0), p(1, 0, 0), p(1, 0, 1), p(0, 0, 1)],
            [p(0, 1, 0), p(0, 1, 1), p(1, 1, 1), p(1, 1, 0)],
            [p(0, 0, 0), p(0, 0, 1), p(0, 1, 1), p(0, 1, 0)],
            [p(1, 0, 0), p(1, 1, 0), p(1, 1, 1), p(1, 0, 1)],
        ];
        quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
    }

    #[test]
    fn subtract_hole() {
        let wall = cube(DVec3::ZERO, DVec3::new(4.0, 0.2, 3.0));
        let hole = cube(DVec3::new(1.0, -0.1, 1.0), DVec3::new(2.0, 0.3, 2.0));
        assert!((signed_volume(&wall) - 2.4).abs() < 1e-9);
        let r = boolean(&wall, &hole, Op::Difference, 1e-7).unwrap();
        assert!((signed_volume(&r) - (2.4 - 0.2)).abs() < 1e-6);
    }
}
