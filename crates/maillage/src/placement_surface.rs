//! Transmet le placement des faces planes à leurs sommets de rendu.
use glam::DVec2;
use modele::placement_surface::PlacementSurface;

pub(crate) fn appliquer(scene: &mut scene::Scene, projet: &modele::Projet) {
    if projet.placements_surface.is_empty() {
        return;
    }
    let mut placements = vec![[0.0; 4]; scene.vertices.len()];
    for objet in &scene.objects {
        let Some(faces) = scene
            .model
            .file
            .entities
            .get(&objet.id)
            .and_then(|e| e.args.first())
            .and_then(|a| a.as_str())
            .and_then(|id| projet.placements_surface.get(id))
        else {
            continue;
        };
        for plage in [objet.opaque, objet.transparent] {
            let debut = plage.start as usize;
            let fin = debut + plage.count as usize;
            for (sommets, sortie) in scene.vertices[debut..fin]
                .chunks_exact(3)
                .zip(placements[debut..fin].chunks_exact_mut(3))
            {
                let point = |i: usize| glam::Vec3::from(sommets[i].pos).as_dvec3();
                let normale = (point(1) - point(0))
                    .cross(point(2) - point(0))
                    .normalize_or_zero();
                if let Some(face) = faces
                    .iter()
                    .find(|f| f.contient(normale, (point(0) + scene.origin) * 1000.0))
                {
                    let origine = DVec2::from_array(face.origine) * 0.001
                        - PlacementSurface::projeter(normale, scene.origin);
                    let (sin, cos) = face.angle.sin_cos();
                    sortie.fill([origine.x as f32, origine.y as f32, cos as f32, sin as f32]);
                }
            }
        }
    }
    scene.placements_surface = placements;
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    #[test]
    fn le_placement_est_transmis_a_la_face_visee_et_pas_a_la_sous_face() {
        let mut projet = modele::Projet::nouveau("Revêtement");
        let id = projet
            .ajouter_mur([3000.0, 4000.0], [5000.0, 4000.0], &Default::default())
            .unwrap();
        let point = DVec3::new(3000.0, 4000.0, projet.niveaux[0].altitude + 2800.0);
        let mut face = PlacementSurface::de_face(DVec3::Z, point);
        face.angle = std::f64::consts::FRAC_PI_2;
        projet.placements_surface.insert(id.clone(), vec![face]);
        let scene = crate::depuis_natif(&projet, std::path::Path::new("test.bem")).unwrap();
        let objet =
            &scene.objects[scene.object_of[&modele::identifiants::id_natif("mur", &id)] as usize];
        let r = objet.opaque;
        let mut dessus = 0;
        let mut dessous = 0;
        for i in r.start as usize..(r.start + r.count) as usize {
            match scene.vertices[i].normal[2] {
                127 => {
                    let p = scene.placements_surface[i];
                    assert!((p[0] as f64 + scene.origin.x - 3.0).abs() < 1e-6);
                    assert!((p[1] as f64 + scene.origin.y - 4.0).abs() < 1e-6);
                    assert!(p[2].abs() < 1e-6);
                    assert_eq!(p[3], 1.0);
                    dessus += 1;
                }
                -127 => {
                    assert_eq!(scene.placements_surface[i], [0.0; 4]);
                    dessous += 1;
                }
                _ => {}
            }
        }
        assert!(dessus > 0 && dessous > 0);
    }
}
