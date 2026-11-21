//! Mesure et empreinte (ignorées par défaut) des arêtes de la scène sur les grands
//! échantillons : sert à vérifier qu'une optimisation des arêtes ne change rien.
#![allow(clippy::unwrap_used, reason = "tests d'intégration : les fonctions d'aide hors #[test] échouent par panique, comme les tests")]
fn empreinte(chemin: &str) {
    let p = modele::Projet::lire_json(&std::fs::read_to_string(chemin).unwrap()).unwrap();
    let t = std::time::Instant::now();
    let s = maillage::depuis_natif(&p, std::path::Path::new(chemin)).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    // arêtes de chaque objet, triées (l'ordre interne peut changer)
    let mut tous: Vec<(u64, Vec<[i64; 3]>)> = s.objects.iter().map(|o| {
        let r = o.aretes;
        let mut v: Vec<[i64; 3]> = s.vertices[r.start as usize..(r.start + r.count) as usize].chunks(2)
            .flat_map(|c| { let mut a = [c[0].pos, c[1].pos]; a.sort_by(|x, y| x.partial_cmp(y).unwrap()); a })
            .map(|p| p.map(|x| (x as f64 * 1e4).round() as i64)).collect();
        v.sort();
        (o.id, v)
    }).collect();
    tous.sort();
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tous.hash(&mut h);
    let n: usize = tous.iter().map(|t| t.1.len()).sum();
    eprintln!("{chemin} : {} sommets d'arêtes, empreinte {:016x}, scène {ms:.0} ms", n, h.finish());
}

#[test]
#[ignore]
fn empreintes() {
    for f in ["samples/ilot_demo/ilot_demo.bem", "samples/immeuble_essai_murs/immeuble_essai_murs.bem", "samples/maison_native.bem"] {
        empreinte(&format!("{}/../../{f}", env!("CARGO_MANIFEST_DIR")));
    }
}
