//! Coordonnées d'un projet Revit : emplacements de projet, point de base du projet,
//! point topographique, lieu géographique (docs/LISEZMOI-RVT.md § 7 « Coordonnées » ;
//! portage de `tools/rvt-research/coordonnees.py`).
//!
//! Un emplacement de projet (`ProjectLocation` de l'API) est un `GeoLocation` : nom
//! `m_name`, transformation `m_pInstanceInfo.m_Trf` (lignes `m_3x3`, origine `m_or`),
//! lieu `m_pInstanceInfo.m_symbolId` → `GeoSite` (latitude et longitude en radians,
//! altitude, fuseau, nom du lieu, station météo). La transformation va des coordonnées
//! partagées vers le repère interne (`GetTotalTransform`) : son origine est le point
//! topographique, ses axes sont les colonnes des lignes enregistrées.
//! `ActiveGeoLocationTrackingElement` désigne l'emplacement actif et un emplacement
//! caché dont l'origine est le point de base du projet. `BasePoint` : point
//! topographique (`m_locationType` = 1) ou point de base du projet, position dans le
//! repère interne `m_oGPoint` → `m_coord`.

use super::champs::{Record, Val};
use super::geometrie::FT;
use glam::DVec3;
use std::collections::HashMap;

/// Emplacement de projet, longueurs en mm.
#[derive(Clone, Debug)]
pub struct Emplacement {
    pub id: i64,
    pub nom: String,
    /// Lignes de la matrice enregistrée (`m_3x3`).
    pub lignes: [DVec3; 3],
    /// Point topographique de l'emplacement dans le repère interne (`m_or`), mm.
    pub origine: DVec3,
    /// Lieu géographique (`GeoSite`).
    pub lieu: i64,
}

impl Emplacement {
    /// Point du repère interne (mm) → coordonnées partagées (mm) : la base
    /// (`GetTotalTransform`) a pour colonnes les lignes enregistrées, son inverse
    /// applique donc les lignes à p − origine.
    pub fn vers_partage(&self, p: DVec3) -> DVec3 {
        let d = p - self.origine;
        let col = |j: usize| DVec3::new(self.lignes[0][j], self.lignes[1][j], self.lignes[2][j]);
        DVec3::new(col(0).dot(d), col(1).dot(d), col(2).dot(d))
    }

    /// Angle par rapport au nord géographique (`ProjectPosition.Angle`), radians.
    pub fn angle(&self) -> f64 {
        -self.lignes[1][0].atan2(self.lignes[0][0])
    }
}

/// Lieu géographique (`SiteLocation`).
#[derive(Clone, Debug, Default)]
pub struct Lieu {
    /// Degrés.
    pub latitude: f64,
    pub longitude: f64,
    /// `SiteLocation.Elevation`, valeur enregistrée.
    pub altitude: f64,
    /// Heures par rapport à UTC.
    pub fuseau: f64,
    pub nom: String,
    pub station: String,
}

/// Coordonnées du document.
#[derive(Clone, Debug, Default)]
pub struct Coordonnees {
    pub emplacements: Vec<Emplacement>,
    pub lieux: HashMap<i64, Lieu>,
    /// Emplacement actif et emplacement caché du point de base du projet.
    pub actif: i64,
    pub projet: i64,
    /// Positions dans le repère interne, mm.
    pub point_base: Option<DVec3>,
    pub point_topo: Option<DVec3>,
}

impl Coordonnees {
    pub fn actif(&self) -> Option<&Emplacement> {
        self.emplacements.iter().find(|e| e.id == self.actif)
    }

    /// Emplacements de `Document.ProjectLocations` : ceux du lieu de l'emplacement actif
    /// (`ProjectLocation.Create` les rattache à un lieu). L'emplacement caché du point de
    /// base du projet et les emplacements d'autres lieux (« DefaultLocation » de Kerreut,
    /// avec son propre `GeoSite`) n'y sont pas.
    pub fn nommes(&self) -> impl Iterator<Item = &Emplacement> {
        let lieu = self.actif().map(|a| a.lieu);
        self.emplacements.iter().filter(move |e| e.id != self.projet && Some(e.lieu) == lieu)
    }

    /// Coordonnées partagées (mm) d'un point du repère interne (mm), dans l'emplacement
    /// actif.
    pub fn partage(&self, p: DVec3) -> Option<DVec3> {
        Some(self.actif()?.vers_partage(p))
    }

    /// Le document est-il placé (emplacement actif autre que l'identité) ?
    pub fn place(&self) -> bool {
        self.actif().is_some_and(|e| e.origine.length() > 1e-6 || e.angle().abs() > 1e-12)
    }
}

/// Objet lu, réduit à sa part des coordonnées.
pub enum Piece {
    Emplacement(Emplacement),
    Lieu(i64, Lieu),
    /// Point topographique (true) ou point de base du projet, mm.
    Point(bool, DVec3),
    /// Emplacement actif, emplacement du point de base du projet.
    Actif(i64, i64),
}

fn vec3(v: &Val) -> Option<DVec3> {
    match v.reals()?.as_slice() {
        &[x, y, z] => Some(DVec3::new(x, y, z)),
        _ => None,
    }
}

/// Lit un `GeoLocation`, un `GeoSite`, un `BasePoint` ou l'élément de suivi de
/// l'emplacement actif.
pub fn lire(id: i64, r: &Record) -> Option<Piece> {
    let o = &r.root;
    let reel = |k: &str| o.get(k).and_then(Val::real).unwrap_or(0.0);
    let texte = |k: &str| o.get(k).and_then(Val::str).unwrap_or_default().to_string();
    let ident = |k: &str| o.get(k).and_then(Val::element_id).unwrap_or(-1);
    Some(match o.class_name() {
        "GeoLocation" => {
            let info = r.field_body(o, "m_pInstanceInfo")?;
            let t = info.get("m_Trf")?.obj()?;
            let l = t.get("m_3x3")?.list();
            let lignes = [vec3(l.first()?)?, vec3(l.get(1)?)?, vec3(l.get(2)?)?];
            let lieu = info.get("m_symbolId").and_then(Val::element_id).unwrap_or(-1);
            Piece::Emplacement(Emplacement { id, nom: texte("m_name"), lignes, origine: vec3(t.get("m_or")?)? * FT, lieu })
        }
        "GeoSite" => Piece::Lieu(
            id,
            Lieu {
                latitude: reel("m_dLatitude").to_degrees(),
                longitude: reel("m_dLongitude").to_degrees(),
                altitude: reel("m_dElevation"),
                fuseau: reel("m_dTimeZone"),
                nom: texte("m_placeName"),
                station: texte("m_weatherStationName"),
            },
        ),
        "BasePoint" => {
            let p = vec3(r.field_body(o, "m_oGPoint")?.get("m_coord")?)? * FT;
            Piece::Point(o.get("m_locationType").and_then(Val::int) == Some(1), p)
        }
        "ActiveGeoLocationTrackingElement" => Piece::Actif(ident("m_activeGeoLocationId"), ident("m_projectGeoLocationId")),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Emplacement B de l'essai E16 : lignes et origine du fichier, position partagée
    /// de l'origine interne et angle relevés dans Revit 2026 (`GetProjectPosition`).
    #[test]
    fn emplacement_b_e16() {
        let e = Emplacement {
            id: 2797,
            nom: "ZZ_EMPLACEMENT_B".into(),
            lignes: [DVec3::new(0.984_807_753_012_208_1, -0.173_648_177_666_930_33, 0.0), DVec3::new(0.173_648_177_666_930_33, 0.984_807_753_012_208_1, 0.0), DVec3::Z],
            origine: DVec3::new(-209.157_282_702_869_88, -703.170_499_898_735_7, -9.842_519_685_039_37) * FT,
            lieu: 1691,
        };
        let p = e.vers_partage(DVec3::ZERO);
        for (v, att) in [(p.x, 100_000.0), (p.y, 200_000.0), (p.z, 3_000.0)] {
            assert!((v - att).abs() < 1e-6, "{v} au lieu de {att}");
        }
        assert!((e.angle().to_degrees() + 10.0).abs() < 1e-9);
    }
}
