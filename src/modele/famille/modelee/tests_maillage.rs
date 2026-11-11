//! Invariants géométriques communs aux générateurs de solides de famille.
use glam::DVec3;
use std::collections::BTreeMap;

pub(super) fn verifier_fermeture(tris: &[[DVec3;3]]) {
    assert!(!tris.is_empty());
    let cle = |p: DVec3| p.to_array().map(|x| (x * 1e6).round() as i64);
    let mut aretes = BTreeMap::<_, (usize,i32)>::new();
    for t in tris {
        assert!(t.iter().all(|p| p.is_finite()));
        assert!((t[1]-t[0]).cross(t[2]-t[0]).length_squared() > 1e-16);
        for i in 0..3 {
            let (a,b)=(cle(t[i]),cle(t[(i+1)%3]));
            let (key,sens)=if a<b {((a,b),1)} else {((b,a),-1)};
            let entry=aretes.entry(key).or_default(); entry.0+=1; entry.1+=sens;
        }
    }
    assert!(aretes.values().all(|&(nombre,sens)| nombre==2 && sens==0), "Maillage non fermé ou faces incohérentes");
}

pub(super) fn volume(tris: &[[DVec3;3]]) -> f64 {
    tris.iter().map(|t| t[0].dot(t[1].cross(t[2]))/6.0).sum()
}
