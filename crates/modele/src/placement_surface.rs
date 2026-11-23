//! Repère d'un motif de modèle et des textures sur une face plane.
use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacementSurface {
    /// Normale orientée de la face et distance signée à l'origine du projet (mm).
    pub normale: [f64; 3],
    pub distance: f64,
    /// Origine dans le plan de projection du matériau (mm) et rotation (radians).
    pub origine: [f64; 2],
    pub angle: f64,
}

impl PlacementSurface {
    pub fn de_face(normale: DVec3, point_mm: DVec3) -> Self {
        Self {
            normale: normale.to_array(),
            distance: normale.dot(point_mm),
            origine: Self::projeter(normale, point_mm).to_array(),
            angle: 0.0,
        }
    }

    pub fn contient(&self, normale: DVec3, point_mm: DVec3) -> bool {
        let n = DVec3::from_array(self.normale);
        n.dot(normale) > 1.0 - 1e-7 && (n.dot(point_mm) - self.distance).abs() < 0.2
    }

    /// Axes de projection historiques du rendu, utilisables même sur une pente.
    pub fn axes(normale: DVec3) -> (DVec3, DVec3) {
        if normale.z.abs() > 0.7 {
            (DVec3::X, DVec3::Y)
        } else {
            (
                DVec3::new(-normale.y + 1e-6, normale.x, 0.0).normalize(),
                DVec3::Z,
            )
        }
    }

    pub fn projeter(normale: DVec3, point: DVec3) -> DVec2 {
        let (u, v) = Self::axes(normale);
        DVec2::new(point.dot(u), point.dot(v))
    }

    pub fn vers_motif(&self, point: DVec2) -> DVec2 {
        DVec2::from_angle(-self.angle).rotate(point - DVec2::from_array(self.origine))
    }

    pub fn depuis_motif(&self, point: DVec2) -> DVec2 {
        DVec2::from_array(self.origine) + DVec2::from_angle(self.angle).rotate(point)
    }

    pub fn dans_face(&self, point: DVec2) -> DVec3 {
        let normale = DVec3::from_array(self.normale);
        let (u, v) = Self::axes(normale);
        let base = u * point.x + v * point.y;
        let direction = u.cross(v);
        base + direction * ((self.distance - normale.dot(base)) / normale.dot(direction))
    }

    pub fn tourner(&mut self, centre: DVec2, angle: f64) {
        let origine = DVec2::from_array(self.origine);
        self.origine = (centre + DVec2::from_angle(angle).rotate(origine - centre)).to_array();
        self.angle = (self.angle + angle).rem_euclid(std::f64::consts::TAU);
    }
}
