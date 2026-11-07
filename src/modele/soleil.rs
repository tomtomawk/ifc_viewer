//! Lieu du projet et position du soleil (algorithme du NOAA, précision de l'ordre de la
//! minute d'arc entre 1950 et 2050). La date n'a pas d'année : une année de référence
//! non bissextile est utilisée.

use serde::{Deserialize, Serialize};

/// Année de référence des dates sans année (non bissextile).
const ANNEE: i32 = 2026;

/// Lieu du projet : coordonnées géographiques (degrés décimaux, nord et est positifs),
/// fuseau horaire (heures par rapport à UTC, hors heure d'été) et heure d'été européenne.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Lieu {
    #[serde(default)]
    pub nom: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default = "Lieu::fuseau_defaut")]
    pub fuseau: f64,
    #[serde(default = "Lieu::vrai")]
    pub heure_ete: bool,
}

impl Lieu {
    fn fuseau_defaut() -> f64 { 1.0 }
    fn vrai() -> bool { true }
    /// Rennes, lieu par défaut d'un projet sans coordonnées.
    pub fn rennes() -> Self {
        Lieu { nom: "Rennes".into(), latitude: 48.1173, longitude: -1.6778, fuseau: 1.0, heure_ete: true }
    }
}

impl Default for Lieu {
    fn default() -> Self { Self::rennes() }
}

/// Villes proposées dans le choix du lieu : nom, latitude, longitude, fuseau.
pub const VILLES: &[(&str, f64, f64, f64)] = &[
    ("Rennes", 48.1173, -1.6778, 1.0),
    ("Nantes", 47.2184, -1.5536, 1.0),
    ("Brest", 48.3904, -4.4861, 1.0),
    ("Saint-Malo", 48.6493, -2.0257, 1.0),
    ("Vannes", 47.6582, -2.7608, 1.0),
    ("Lorient", 47.7483, -3.3700, 1.0),
    ("Quimper", 47.9960, -4.1024, 1.0),
    ("Saint-Brieuc", 48.5141, -2.7603, 1.0),
    ("Angers", 47.4784, -0.5632, 1.0),
    ("Le Mans", 48.0061, 0.1996, 1.0),
    ("Caen", 49.1829, -0.3707, 1.0),
    ("Paris", 48.8566, 2.3522, 1.0),
    ("Lille", 50.6292, 3.0573, 1.0),
    ("Strasbourg", 48.5734, 7.7521, 1.0),
    ("Lyon", 45.7640, 4.8357, 1.0),
    ("Bordeaux", 44.8378, -0.5792, 1.0),
    ("Toulouse", 43.6047, 1.4442, 1.0),
    ("Montpellier", 43.6108, 3.8767, 1.0),
    ("Marseille", 43.2965, 5.3698, 1.0),
    ("Nice", 43.7102, 7.2620, 1.0),
];

/// Jour de l'année (1 à 365) d'une date sans année.
pub fn jour_annee(jour: u32, mois: u32) -> u32 {
    const CUMUL: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    CUMUL[(mois.clamp(1, 12) - 1) as usize] + jour.clamp(1, jours_du_mois(mois))
}

pub fn jours_du_mois(mois: u32) -> u32 {
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(mois.clamp(1, 12) - 1) as usize]
}

/// Jour de l'année du dernier dimanche d'un mois de l'année de référence.
fn dernier_dimanche(mois: u32) -> u32 {
    let fin = jour_annee(jours_du_mois(mois), mois);
    // 1er janvier de l'année de référence : jour de la semaine (0 lundi … 6 dimanche)
    let premier = jour_semaine_1er_janvier(ANNEE);
    let semaine = (premier + fin - 1) % 7;
    fin - (semaine + 1) % 7
}

fn jour_semaine_1er_janvier(annee: i32) -> u32 {
    // Gauss : 0 dimanche … 6 samedi, ramené à 0 lundi
    let a = annee - 1;
    let d = (1 + 5 * (a % 4) + 4 * (a % 100) + 6 * (a % 400)) % 7;
    ((d + 6) % 7) as u32
}

/// Vrai si l'heure d'été européenne s'applique ce jour (du dernier dimanche de mars au
/// dernier dimanche d'octobre).
pub fn en_heure_ete(jour: u32, mois: u32) -> bool {
    let n = jour_annee(jour, mois);
    n >= dernier_dimanche(3) && n < dernier_dimanche(10)
}

/// Position du soleil (azimut en degrés depuis le nord, dans le sens horaire ; hauteur
/// en degrés au-dessus de l'horizon) à l'heure légale `heure` (heures décimales).
pub fn position(lieu: &Lieu, jour: u32, mois: u32, heure: f64) -> (f64, f64) {
    let decalage = lieu.fuseau + if lieu.heure_ete && en_heure_ete(jour, mois) { 1.0 } else { 0.0 };
    let n = jour_annee(jour, mois) as f64;
    // fraction de l'année (radians)
    let g = 2.0 * std::f64::consts::PI / 365.0 * (n - 1.0 + (heure - decalage - 12.0) / 24.0);
    let eqt = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin() - 0.002697 * (3.0 * g).cos() + 0.00148 * (3.0 * g).sin();
    // temps solaire vrai (minutes) et angle horaire
    let tsv = heure * 60.0 + eqt + 4.0 * lieu.longitude - 60.0 * decalage;
    let ah = (tsv / 4.0 - 180.0).to_radians();
    let lat = lieu.latitude.to_radians();
    let cos_z = (lat.sin() * decl.sin() + lat.cos() * decl.cos() * ah.cos()).clamp(-1.0, 1.0);
    let zenith = cos_z.acos();
    let hauteur = 90.0 - zenith.to_degrees();
    // azimut depuis le nord, sens horaire
    let az = (decl.sin() - lat.sin() * cos_z) / (lat.cos() * zenith.sin()).max(1e-12);
    let mut azimut = az.clamp(-1.0, 1.0).acos().to_degrees();
    if ah > 0.0 {
        azimut = 360.0 - azimut;
    }
    (azimut, hauteur)
}

/// Direction vers le soleil dans le repère du projet (z vers le haut) : `nord` est
/// l'orientation du nord géographique par rapport à l'axe Y du projet (radians).
pub fn direction(azimut: f64, hauteur: f64, nord: f64) -> [f64; 3] {
    let (a, h) = (azimut.to_radians(), hauteur.to_radians());
    // repère géographique : x est, y nord ; puis rotation du nord
    let (x, y) = (a.sin() * h.cos(), a.cos() * h.cos());
    let (s, c) = nord.sin_cos();
    [c * x - s * y, s * x + c * y, h.sin()]
}

/// Ambiances de ciel, comme les préréglages d'Enscape.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmbianceCiel {
    Clair,
    Voile,
    Nuageux,
    Couvert,
    Brumeux,
}

impl AmbianceCiel {
    pub const TOUTES: [AmbianceCiel; 5] = [Self::Clair, Self::Voile, Self::Nuageux, Self::Couvert, Self::Brumeux];
    pub fn nom(self) -> &'static str {
        match self {
            Self::Clair => "Ciel clair",
            Self::Voile => "Légèrement voilé",
            Self::Nuageux => "Nuageux",
            Self::Couvert => "Couvert",
            Self::Brumeux => "Brumeux",
        }
    }
    /// Réglages de l'ambiance (brume, nuages, intensités), le reste inchangé.
    pub fn appliquer(self, c: &mut ReglagesCiel) {
        let (brume, nuages, soleil, ciel) = match self {
            Self::Clair => (0.15, 0.0, 1.0, 1.0),
            Self::Voile => (0.35, 0.25, 0.85, 1.0),
            Self::Nuageux => (0.3, 0.55, 0.7, 0.9),
            Self::Couvert => (0.45, 0.95, 0.15, 0.75),
            Self::Brumeux => (0.85, 0.2, 0.5, 0.85),
        };
        c.ambiance = self;
        c.brume = brume;
        c.nuages = nuages;
        c.intensite_soleil = soleil;
        c.intensite_ciel = ciel;
    }
}

/// Réglages simples du ciel physique d'une vue (style Réaliste).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ReglagesCiel {
    pub ambiance: AmbianceCiel,
    /// Brume (aérosols) : 0 air pur, 1 très brumeux.
    pub brume: f32,
    /// Couverture nuageuse : 0 à 1.
    pub nuages: f32,
    /// Intensité du soleil et du ciel (1 : valeur physique).
    pub intensite_soleil: f32,
    pub intensite_ciel: f32,
    /// Taille apparente du soleil (1 : réelle, plus grand : ombres plus douces).
    pub taille_soleil: f32,
    /// Couleur du sol vu à l'horizon et renvoyée vers le ciel.
    pub sol: [u8; 3],
    /// Exposition automatique, sinon valeur d'exposition fixe (EV).
    pub exposition_auto: bool,
    pub exposition: f32,
}

impl Default for ReglagesCiel {
    fn default() -> Self {
        let mut c = ReglagesCiel { ambiance: AmbianceCiel::Clair, brume: 0.0, nuages: 0.0, intensite_soleil: 1.0, intensite_ciel: 1.0, taille_soleil: 1.0, sol: [120, 115, 100], exposition_auto: true, exposition: 0.0 };
        AmbianceCiel::Clair.appliquer(&mut c);
        c
    }
}

/// Réglages de rendu d'une vue enregistrés avec elle : style, ombres, soleil (date sans
/// année, heure légale) et ciel physique.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct RenduVue {
    /// Style visuel (« graphique », « realiste »), None : celui du genre de vue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    pub ombres_portees: bool,
    pub ombre_ambiante: bool,
    pub jour: u32,
    pub mois: u32,
    pub heure: f32,
    pub ciel: ReglagesCiel,
}

impl Default for RenduVue {
    fn default() -> Self {
        RenduVue { style: None, ombres_portees: false, ombre_ambiante: false, jour: 21, mois: 6, heure: 15.0, ciel: ReglagesCiel::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hauteur_maximale_rennes() {
        let l = Lieu::rennes();
        // midi solaire au solstice d'été vers 14 h 07 (heure d'été) : 90 - 48,12 + 23,44
        let max = (0..240).map(|k| position(&l, 21, 6, 12.0 + k as f64 / 60.0).1).fold(f64::MIN, f64::max);
        assert!((max - 65.3).abs() < 0.3, "{max}");
        // équinoxe : 90 - 48,12
        let max = (0..240).map(|k| position(&l, 20, 3, 12.0 + k as f64 / 60.0).1).fold(f64::MIN, f64::max);
        assert!((max - 41.9).abs() < 0.6, "{max}");
    }

    #[test]
    fn azimut_matin_et_soir() {
        let l = Lieu::rennes();
        let (a_matin, _) = position(&l, 21, 6, 9.0);
        let (a_soir, _) = position(&l, 21, 6, 19.0);
        assert!(a_matin > 60.0 && a_matin < 120.0, "{a_matin}");
        assert!(a_soir > 240.0 && a_soir < 300.0, "{a_soir}");
        // la nuit : sous l'horizon
        assert!(position(&l, 21, 12, 23.0).1 < 0.0);
    }

    #[test]
    fn heure_ete_2026() {
        // 29 mars et 25 octobre 2026
        assert!(!en_heure_ete(28, 3) && en_heure_ete(29, 3));
        assert!(en_heure_ete(24, 10) && !en_heure_ete(25, 10));
    }

    #[test]
    fn direction_nord_tourne() {
        // soleil au sud, horizon : -Y sans rotation
        let d = direction(180.0, 0.0, 0.0);
        assert!((d[1] + 1.0).abs() < 1e-9 && d[0].abs() < 1e-9);
        // nord tourné de 90° (vers -X) : le sud est vers +X
        let d = direction(180.0, 0.0, std::f64::consts::FRAC_PI_2);
        assert!((d[0] - 1.0).abs() < 1e-9, "{d:?}");
    }
}
