//! Site IFC : lieu géographique, cote NGF et nord du projet, quand le modèle les porte.
use super::step::{guid, reel, texte, Step};
use modele::Projet;

/// Angle en degrés décimaux -> `IfcCompoundPlaneAngleMeasure` (degrés, minutes,
/// secondes, millionièmes de seconde, tous du signe de l'angle).
fn angle_compose(degres: f64) -> String {
    let signe = if degres < 0.0 { -1 } else { 1 };
    let micro = (degres.abs() * 3600.0 * 1e6).round() as i64;
    let (d, reste) = (micro / 3_600_000_000, micro % 3_600_000_000);
    let (m, reste) = (reste / 60_000_000, reste % 60_000_000);
    let (s, u) = (reste / 1_000_000, reste % 1_000_000);
    format!("({},{},{},{})", signe * d, signe * m, signe * s, signe * u)
}

/// `IfcSite` placé à l'origine. Latitude et longitude seulement si le projet porte un
/// lieu (`Projet::lieu` ; absent, le soleil prend Rennes par défaut, ce n'est pas une
/// donnée du projet) ; altitude de référence = cote NGF de l'origine si elle est
/// renseignée. Les coordonnées E/N de l'origine ne sont pas écrites : le système de
/// coordonnées (`IfcProjectedCRS`) n'est pas connu du modèle.
pub(super) fn site(w: &mut Step, p: &Projet, placement: usize) -> usize {
    let (lat, lon) = match &p.lieu {
        Some(l) if l.latitude.is_finite() && l.longitude.is_finite() => (angle_compose(l.latitude), angle_compose(l.longitude)),
        _ => ("$".into(), "$".into()),
    };
    let altitude = if p.ngf() != 0.0 && p.ngf().is_finite() { reel(p.ngf()) } else { "$".into() };
    w.add(format!(
        "IFCSITE('{}',$,{},$,$,#{placement},$,$,.ELEMENT.,{lat},{lon},{altitude},$,$)",
        guid(&format!("site:{}", p.nom), None),
        texte(&p.nom)
    ))
}

/// Direction du nord géographique dans le plan XY du projet (`TrueNorth`) : l'axe Y
/// tourné de `orientation_nord` (sens trigonométrique, comme `modele::soleil::direction`).
/// None si le nord est l'axe Y.
pub(super) fn nord(p: &Projet) -> Option<String> {
    let a = p.orientation_nord;
    (a != 0.0 && a.is_finite()).then(|| format!("IFCDIRECTION(({},{}))", reel(-a.sin()), reel(a.cos())))
}

#[cfg(test)]
mod tests {
    #[test]
    fn angles_composes() {
        assert_eq!(super::angle_compose(48.1173), "(48,7,2,280000)");
        assert_eq!(super::angle_compose(-1.6778), "(-1,-40,-40,-80000)");
    }
}
