//! Rendu « dessin » des plans et élévations, à la manière d'un logiciel de DAO (CADCraft) :
//! fond blanc, faces blanches qui masquent ce qu'elles cachent (tampon de profondeur),
//! arêtes vives des maillages en trait fin, et coupe par un plan : poché (plein, hachuré ou
//! vide) et contour en trait fort. Épaisseurs et pas des hachures en mm du papier,
//! multipliés par l'échelle du dessin.

mod motifs;
pub use motifs::*;
mod feuilles;
pub use feuilles::{dessin_vue, dwg_feuille, dxf_feuille, pdf, pdf_jeu, pdf_pages, svg_feuille, vue_placee};
mod dxf;
pub use dxf::dxf;
pub mod dwg;
mod plans;
pub use plans::{aretes_plage_haute, dans_plage_haute, fond_plan, limite_haute_plan, pointilles_plage_haute};

pub mod dao;
mod coupe;
pub use coupe::{coupe, coupe_avec_cache, CacheCoupe};
mod vue;
mod symboles_caches;
mod masques;
pub use masques::masquer_traits;
pub use symboles_caches::decouper_symboles_caches;
pub use vue::{cadrer_primitives, sommets, sommets_boite, styler_traits, vu};
use noyau::motif::{GrilleMotif, Motif, hachures_plan};
use scene::{Scene, Vertex};
use modele::famille::modelee::Affichage;
use noyau::trace2d::{Forme, Primitive, Role, Style};
use glam::{DVec3, Vec3, Vec4};
use rayon::prelude::*;
use std::collections::HashMap;
use noyau::aretes::{NOIR, PAS, Polyligne, TRAIT_FORT_MM, aretes, chainer};

/// Remplissage des parties coupées.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Poche {
    /// Gris clair hachuré à 45°.
    Hachure,
    /// Gris foncé uni.
    Plein,
    /// Blanc : contour seul.
    Vide,
    /// Couleur du matériau (style Réaliste).
    Materiau,
}

impl Poche {
    pub fn nom(self) -> &'static str {
        match self {
            Poche::Hachure => "hachure",
            Poche::Plein => "plein",
            Poche::Vide => "vide",
            Poche::Materiau => "materiau",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Poche::Hachure => "Hachuré",
            Poche::Plein => "Plein",
            Poche::Vide => "Vide",
            Poche::Materiau => "Matériau",
        }
    }

    pub fn depuis(nom: &str) -> Option<Poche> {
        [Poche::Hachure, Poche::Plein, Poche::Vide, Poche::Materiau].into_iter().find(|p| p.nom() == nom)
    }
}

/// Trait fin des pièces translucides coupées (vitrages), mm du papier.
pub const TRAIT_FIN_MM: f64 = 0.05;
/// Pas des hachures du poché, mm du papier.
pub const PAS_HACHURE_MM: f64 = 1.5;

/// Partie d'un triangle du côté gardé du plan (n·p + d ≥ 0) : polygone convexe.
fn decouper(t: &[DVec3], n: DVec3, d: f64) -> Vec<DVec3> {
    let mut out = Vec::with_capacity(4);
    for k in 0..t.len() {
        let (a, b) = (t[k], t[(k + 1) % t.len()]);
        let (sa, sb) = (n.dot(a) + d, n.dot(b) + d);
        if sa >= 0.0 {
            out.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            out.push(a + (b - a) * (sa / (sa - sb)));
        }
    }
    out
}

/// Segment d'intersection d'un triangle et du plan (n, d), s'il le traverse.
fn section(p: [DVec3; 3], n: DVec3, d: f64) -> Option<[DVec3; 2]> {
    // sommet exactement sur le plan : compté du côté gardé
    let s = p.map(|q| {
        let s = n.dot(q) + d;
        if s == 0.0 { 1e-12 } else { s }
    });
    if s.iter().all(|&x| x > 0.0) || s.iter().all(|&x| x < 0.0) {
        return None;
    }
    let mut pts = Vec::with_capacity(2);
    for k in 0..3 {
        let (a, b) = (k, (k + 1) % 3);
        if (s[a] > 0.0) != (s[b] > 0.0) {
            let t = s[a] / (s[a] - s[b]);
            pts.push(p[a] + (p[b] - p[a]) * t);
        }
    }
    (pts.len() == 2 && pts[0].distance_squared(pts[1]) > 1e-14).then(|| [pts[0], pts[1]])
}

#[cfg(test)]
mod tests;

/// Affichage des tests : tout élément de famille sans restriction est montré.
#[cfg(test)]
const TOUT: Affichage = Affichage { detail: modele::famille::modelee::NiveauDetail::Moyen, vue: modele::famille::modelee::TypeVue::Plan };

#[cfg(test)]
mod mesure {
    /// Sphère de 500 000 triangles : durée du calcul des arêtes.
    #[test]
    #[ignore]
    fn sphere_500k() {
        use glam::DVec3;
        let (nu, nv) = (500, 500);
        let p = |i: usize, j: usize| {
            let (u, v) = (i as f64 / nu as f64 * std::f64::consts::TAU, j as f64 / nv as f64 * std::f64::consts::PI);
            DVec3::new(v.sin() * u.cos(), v.sin() * u.sin(), v.cos()) * 10.0
        };
        let mut t = Vec::new();
        for i in 0..nu {
            for j in 0..nv {
                let (a, b, c, d) = (p(i, j), p((i + 1) % nu, j), p((i + 1) % nu, j + 1), p(i, j + 1));
                t.push([a, b, c]);
                t.push([a, c, d]);
            }
        }
        let t0 = std::time::Instant::now();
        let a = super::aretes(&t);
        println!("{} triangles : {} vives, {} douces en {:?}", t.len(), a.vives.len(), a.douces.len(), t0.elapsed());
    }

    /// Deux murs de même type à un angle quelconque : la coupe en plan ne garde que le
    /// contour extérieur, aucun reste du trait d'onglet.
    #[test]
    fn angle_quelconque_sans_reste_d_onglet() {
        use modele::{creation::ParametresMur, Projet};
        for deg in [90.0_f64, 70.0, 55.0, 35.0, 123.0] {
            let mut p = Projet::nouveau("A");
            let o = ParametresMur { epaisseur: 200.0, ..Default::default() };
            let r = deg.to_radians();
            let c = [5000.0 + 4000.0 * r.cos(), 4000.0 * r.sin()];
            p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &o).unwrap();
            p.ajouter_mur([5000.0, 0.0], c, &o).unwrap();
            let s = maillage::depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
            let visible = vec![true; s.objects.len()];
            let prims = crate::coupe(&s, &visible, crate::TOUT, glam::Vec4::new(0.0, 0.0, 1.0, -1.0), 100.0, crate::Poche::Hachure);
            let mut segs: Vec<[glam::DVec3; 2]> = Vec::new();
            for pr in prims.iter().filter(|p| p.role == noyau::trace2d::Role::Coupe) {
                if let noyau::trace2d::Forme::Segments(v) = &pr.forme { segs.extend(v.iter().copied()); }
            }
            // contour d'un angle plein : 6 côtés (18,4 m), sans reste du trait d'onglet
            let total: f64 = segs.iter().map(|s| s[0].distance(s[1])).sum();
            assert_eq!(segs.len(), 6, "{deg}° : {segs:?}");
            assert!((total - 18.4).abs() < 1e-3, "{deg}° : {total}");
        }
    }
}

#[cfg(test)]
mod test_isolant_arc {
    use modele::*;
    use modele::creation::ParametresMur;

    /// Mur courbe isolé passé par toute la chaîne (projet, scène, coupe) : extrémités non
    /// radiales comprises, le motif d'isolant couvre tout l'arc.
    #[test]
    fn isolant_d_un_mur_courbe_reel() {
        let mut p = Projet::nouveau("Arc");
        let c = |m: &str, e: f64, f| Couche { materiau: Some(m.into()), epaisseur: e, fonction: f, plume: None, retour: false, ossature: None };
        p.types_murs.push(TypeCompose { sorte: None, id: "t".into(), nom: "Mur".into(), couches: vec![c("Enduit", 20.0, FonctionCouche::Finition), c("Isolant", 100.0, FonctionCouche::Isolation), c("Béton", 200.0, FonctionCouche::Structure)], categorie: None, couches_detaillees: true, origine: None });
        let o = ParametresMur { type_mur: Some("t".into()), ..Default::default() };
        p.ajouter_mur_axe(Axe::Arc { debut: [5000.0, 0.0], fin: [0.0, 5000.0], centre: [0.0, 0.0], direct: true }, &o).unwrap();
        let s = maillage::depuis_natif(&p, std::path::Path::new("t.json")).unwrap();
        let prims = super::coupe(&s, &vec![true; s.objects.len()], crate::TOUT, glam::Vec3::NEG_Z.extend(1.0), 100.0, super::Poche::Hachure);
        // la ligne d'isolant (série de boucles : la plus fournie des hachures) : elle doit couvrir l'arc
        // (corde de 5 m × √2 ≈ 6,9 m), pas un morceau droit
        let isolant = prims.iter().filter(|p| p.role == super::Role::Hachure).filter_map(|p| match &p.forme {
            super::Forme::Segments(v) => Some(v),
            _ => None,
        }).max_by_key(|v| v.len()).expect("ligne d'isolant");
        let pts: Vec<_> = isolant.iter().flatten().collect();
        let etendue = pts.iter().flat_map(|a| pts.iter().map(move |b| a.distance(**b))).fold(0.0, f64::max);
        assert!(etendue > 6.0, "motif d'isolant limité à {etendue:.2} m");
    }
}
