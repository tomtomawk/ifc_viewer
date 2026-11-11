//! Mémoire des opérations booléennes : le résultat d'une opération est repris tel quel
//! quand les deux maillages, l'opération et la tolérance sont identiques à un calcul
//! précédent. C'est ce qui rend le recalcul d'un projet partiel : après une modification,
//! seuls les éléments dont les opérandes ont réellement changé (l'élément modifié, ses
//! voisins en contact, ce qu'il perce ou porte) repassent par le calcul.
use super::csg::{boolean, Op, Tri};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Empreintes des deux maillages, opération, tolérance.
type Cle = ([u64; 2], [u64; 2], u8, u64);

#[derive(Default)]
struct Memoire {
    generation: u64,
    resultats: HashMap<Cle, (u64, Option<Arc<Vec<Tri>>>)>,
}

/// Nombre de calculs d'ensemble pendant lesquels un résultat inutilisé est gardé
/// (annuler puis rétablir retrouvent ainsi leurs résultats).
const GENERATIONS_GARDEES: u64 = 4;

fn memoire() -> &'static Mutex<Memoire> {
    static MEMOIRE: OnceLock<Mutex<Memoire>> = OnceLock::new();
    MEMOIRE.get_or_init(Default::default)
}

/// Empreinte de 128 bits d'un maillage : coordonnées exactes, dans l'ordre.
fn empreinte(tris: &[Tri]) -> [u64; 2] {
    let (mut a, mut b) = (0x9E37_79B9_7F4A_7C15u64 ^ tris.len() as u64, 0xC2B2_AE3D_27D4_EB4Fu64);
    for v in tris.iter().flatten() {
        for x in v.to_array() {
            let k = x.to_bits();
            a = (a ^ k).wrapping_mul(0x0000_0100_0000_01B3).rotate_left(23);
            b = (b.rotate_left(29) ^ k).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        }
    }
    [a, b]
}

/// `csg::boolean` avec reprise d'un résultat déjà calculé pour les mêmes opérandes.
pub fn boolean_memoire(a: &[Tri], b: &[Tri], op: Op, eps: f64) -> Option<Vec<Tri>> {
    let rang = match op {
        Op::Difference => 0,
        Op::Intersection => 1,
        Op::Union => 2,
    };
    let cle: Cle = (empreinte(a), empreinte(b), rang, eps.to_bits());
    // le verrou ne couvre que la recherche : la copie du résultat se fait après
    let trouve = {
        let mut m = memoire().lock().unwrap_or_else(|e| e.into_inner());
        let generation = m.generation;
        m.resultats.get_mut(&cle).map(|(vu, resultat)| {
            *vu = generation;
            resultat.clone()
        })
    };
    if let Some(resultat) = trouve {
        return resultat.map(|r| r.as_ref().clone());
    }
    let resultat = boolean(a, b, op, eps);
    let mut m = memoire().lock().unwrap_or_else(|e| e.into_inner());
    let generation = m.generation;
    m.resultats.insert(cle, (generation, resultat.clone().map(Arc::new)));
    resultat
}

/// Fin d'un calcul d'ensemble : oublie les résultats qui n'ont servi à aucun des
/// derniers calculs.
pub fn fin_de_calcul() {
    let mut m = memoire().lock().unwrap_or_else(|e| e.into_inner());
    m.generation += 1;
    let generation = m.generation;
    m.resultats.retain(|_, (vu, _)| *vu + GENERATIONS_GARDEES > generation);
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn cube(o: DVec3, c: f64) -> Vec<Tri> {
        let s = |x: f64, y: f64, z: f64| o + DVec3::new(x, y, z) * c;
        let faces = [
            [s(0., 0., 0.), s(0., 1., 0.), s(1., 1., 0.), s(1., 0., 0.)],
            [s(0., 0., 1.), s(1., 0., 1.), s(1., 1., 1.), s(0., 1., 1.)],
            [s(0., 0., 0.), s(1., 0., 0.), s(1., 0., 1.), s(0., 0., 1.)],
            [s(1., 0., 0.), s(1., 1., 0.), s(1., 1., 1.), s(1., 0., 1.)],
            [s(1., 1., 0.), s(0., 1., 0.), s(0., 1., 1.), s(1., 1., 1.)],
            [s(0., 1., 0.), s(0., 0., 0.), s(0., 0., 1.), s(0., 1., 1.)],
        ];
        faces.iter().flat_map(|f| [[f[0], f[1], f[2]], [f[0], f[2], f[3]]]).collect()
    }

    #[test]
    fn resultat_repris_identique_au_calcul_direct() {
        let (a, b) = (cube(DVec3::ZERO, 10.0), cube(DVec3::splat(5.0), 10.0));
        for op in [Op::Difference, Op::Intersection, Op::Union] {
            let direct = boolean(&a, &b, op, 1e-5);
            assert!(direct.as_ref().is_some_and(|t| !t.is_empty()));
            assert_eq!(boolean_memoire(&a, &b, op, 1e-5), direct);
            fin_de_calcul();
            assert_eq!(boolean_memoire(&a, &b, op, 1e-5), direct);
        }
        // un sommet déplacé : autre empreinte, autre résultat
        let c = cube(DVec3::splat(4.0), 10.0);
        assert_ne!(empreinte(&b), empreinte(&c));
        assert_ne!(boolean_memoire(&a, &c, Op::Difference, 1e-5), boolean_memoire(&a, &b, Op::Difference, 1e-5));
    }
}
