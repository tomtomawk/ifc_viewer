//! Mesure (ignorée par défaut) : coûts de l'historique sur l'immeuble d'essai.
#[test]
#[ignore]
fn mesurer() {
    let p = modele::Projet::lire_json(include_str!("../../../samples/immeuble_essai_murs/immeuble_essai_murs.modele.json")).unwrap();
    let t = std::time::Instant::now();
    let json = p.json().unwrap();
    eprintln!("instantané : {} octets, {:.1} ms", json.len(), t.elapsed().as_secs_f64() * 1e3);
    let t = std::time::Instant::now();
    let _a: serde_json::Value = serde_json::from_str(&json).unwrap();
    eprintln!("lecture en Value : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    use commandes::{Commande, Execution};
    let mur = p.murs[0].id.clone();
    let q = p.executer(Commande::Deplacer { ids: vec![mur], delta: [10.0, 0.0, 0.0] }).unwrap().0;
    let t = std::time::Instant::now();
    let va = serde_json::to_value(&p).unwrap();
    eprintln!("état en Value : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let vb = serde_json::to_value(&q).unwrap();
    let t = std::time::Instant::now();
    let d = commandes::differences::calculer(&va, &vb).unwrap();
    eprintln!("différence : {:.1} ms, {} octets en JSON", t.elapsed().as_secs_f64() * 1e3, commandes::differences::taille(&d));
    let mut h: commandes::historique::Historique = Default::default();
    let t = std::time::Instant::now();
    for _ in 0..10 { h.pousser(&p, &q).unwrap(); h.pousser(&q, &p).unwrap(); }
    eprintln!("étape d'historique (différence, libellé, compression) : {:.1} ms ; mémoire pour {} étapes : {} octets", t.elapsed().as_secs_f64() * 1e3 / 20.0, h.libelles(false).len(), h.memoire());
    let t = std::time::Instant::now();
    let r = h.cible(&p, false).unwrap();
    eprintln!("annuler (application de la différence, relecture) : {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    assert_eq!(r.json().unwrap(), q.json().unwrap());
    for niveau in [1u8, 3, 6] {
        let t = std::time::Instant::now();
        let c = miniz_oxide::deflate::compress_to_vec(json.as_bytes(), niveau);
        let tc = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let d = miniz_oxide::inflate::decompress_to_vec(&c).unwrap();
        eprintln!("compression niveau {niveau} : {} octets ({:.1} %), {tc:.1} ms ; décompression {:.1} ms", c.len(), 100.0 * c.len() as f64 / json.len() as f64, t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(d.len(), json.len());
    }
}
