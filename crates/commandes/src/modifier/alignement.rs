//! Calcul des transformations pour aligner des segments et des faces.
use glam::DVec3;

#[derive(Clone, Copy)]
pub enum Geometrie {
    Segment([DVec3; 2]),
    Face([DVec3; 3]),
}

/// Rotation minimale autour de Z et translation normale pour superposer les supports.
/// Le modèle natif permet les rotations en plan et les translations sur les trois axes.
pub fn transformation(
    reference: Geometrie,
    source: Geometrie,
) -> Result<(DVec3, f64, DVec3), &'static str> {
    match (reference, source) {
        (Geometrie::Segment([a, b]), Geometrie::Segment([p, q])) => {
            let u = (b - a)
                .truncate()
                .try_normalize()
                .ok_or("Segment de référence trop court.")?;
            let v = (q - p)
                .truncate()
                .try_normalize()
                .ok_or("Segment à aligner trop court.")?;
            let u = if u.dot(v) < 0.0 { -u } else { u };
            let angle = v.perp_dot(u).atan2(v.dot(u));
            let n = u.perp();
            Ok((p, angle, (n * (a - p).truncate().dot(n)).extend(0.0)))
        }
        (Geometrie::Face([a, b, c]), Geometrie::Face([p, q, r])) => {
            let n = (b - a)
                .cross(c - a)
                .try_normalize()
                .ok_or("Face de référence dégénérée.")?;
            let m = (q - p)
                .cross(r - p)
                .try_normalize()
                .ok_or("Face à aligner dégénérée.")?;
            if n.cross(m).length() <= 1e-5 {
                return Ok((p, 0.0, n * (a - p).dot(n)));
            }
            let u = m.truncate();
            let angle = [n, -n]
                .into_iter()
                .filter(|v| {
                    (v.z - m.z).abs() <= 1e-5 && u.length() > 1e-5 && v.truncate().length() > 1e-5
                })
                .map(|v| u.perp_dot(v.truncate()).atan2(u.dot(v.truncate())))
                .min_by(|a, b| a.abs().total_cmp(&b.abs()))
                .ok_or("Ces faces demandent une rotation hors du plan horizontal.")?;
            Ok((p, angle, n * (a - p).dot(n)))
        }
        _ => Err("Choisissez deux segments en plan ou deux faces en 3D."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_oblique_aligne_par_rotation_et_translation() {
        let p = DVec3::new(3.0, 2.0, 0.0);
        let reference = Geometrie::Segment([DVec3::ZERO, DVec3::X]);
        let (pivot, angle, delta) =
            transformation(reference, Geometrie::Segment([p, p + DVec3::Y])).unwrap();
        let direction = glam::DQuat::from_rotation_z(angle) * DVec3::Y;
        assert!(direction.cross(DVec3::X).length() < 1e-9);
        assert!((pivot + delta).y.abs() < 1e-9);
    }

    #[test]
    fn faces_horizontales_translatees_en_z() {
        let face = [DVec3::ZERO, DVec3::X, DVec3::Y];
        let cible = face.map(|p| p + 2.0 * DVec3::Z);
        let (_, angle, delta) =
            transformation(Geometrie::Face(face), Geometrie::Face(cible)).unwrap();
        assert_eq!(angle, 0.0);
        assert_eq!(delta, -2.0 * DVec3::Z);
    }

    #[test]
    fn faces_verticales_orientees_puis_deplacees() {
        let p = DVec3::new(3.0, 2.0, 0.0);
        let reference = Geometrie::Face([DVec3::ZERO, DVec3::X, DVec3::Z]);
        let (_, angle, delta) =
            transformation(reference, Geometrie::Face([p, p + DVec3::Y, p + DVec3::Z])).unwrap();
        assert!((angle.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(delta, -2.0 * DVec3::Y);
        assert!(
            transformation(reference, Geometrie::Face([p, p + DVec3::X, p + DVec3::Y])).is_err()
        );
    }
}
