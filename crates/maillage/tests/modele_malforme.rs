//! T1 : projets `.bem` malformés. Des valeurs remplacées au hasard (graine fixe) dans les
//! projets d'exemple donnent une erreur de lecture, de `verifier()` ou de construction de
//! la scène, jamais une panique.
#![allow(clippy::unwrap_used, reason = "tests d'intégration : les fonctions d'aide hors #[test] échouent par panique, comme les tests")]
use serde_json::Value;

struct Alea(u64);
impl Alea {
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn jusqua(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }
}

/// Nombre de nœuds de l'arbre JSON.
fn compter(v: &Value) -> usize {
    1 + match v {
        Value::Array(a) => a.iter().map(compter).sum(),
        Value::Object(o) => o.values().map(compter).sum(),
        _ => 0,
    }
}

/// Modifie le nœud n° `k` (parcours en profondeur) en gardant son type : nombre
/// extrême, chaîne vide, inconnue ou reprise d'ailleurs (référence vers un autre objet),
/// tableau vidé ou amputé, champ d'objet retiré.
fn muter(v: &mut Value, k: &mut usize, a: &mut Alea, chaines: &[String]) -> bool {
    if *k == 0 {
        match v {
            Value::Number(_) => *v = serde_json::from_str(["-1", "0", "1e300", "-1e300", "4294967296", "0.000001"][a.jusqua(6)]).unwrap(),
            Value::String(t) => *t = match a.jusqua(3) { 0 => String::new(), 1 => "inconnu".into(), _ => chaines[a.jusqua(chaines.len())].clone() },
            Value::Array(t) => if t.is_empty() || a.jusqua(2) == 0 { t.clear() } else { t.remove(a.jusqua(t.len())); },
            Value::Object(o) => if let Some(cle) = o.keys().nth(a.jusqua(o.len())).cloned() { o.remove(&cle); },
            Value::Bool(b) => *b = !*b,
            Value::Null => {}
        }
        return true;
    }
    *k -= 1;
    match v {
        Value::Array(t) => t.iter_mut().any(|x| muter(x, k, a, chaines)),
        Value::Object(o) => o.values_mut().any(|x| muter(x, k, a, chaines)),
        _ => false,
    }
}

/// Chaînes du fichier (identifiants, noms), réutilisées comme références erronées.
fn chaines(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(t) => out.push(t.clone()),
        Value::Array(t) => t.iter().for_each(|x| chaines(x, out)),
        Value::Object(o) => o.values().for_each(|x| chaines(x, out)),
        _ => {}
    }
}

#[test]
fn modeles_mutes_sans_panique() {
    static PANIQUES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let defaut = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        PANIQUES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        defaut(info);
    }));
    let mut a = Alea(0x51_7cc1_b727_220a);
    let (mut lus, mut verifies, mut scenes) = (0, 0, 0);
    for nom in ["maison_native.bem", "ilot_demo/ilot_demo.bem"] {
        let texte = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/").to_string() + nom).unwrap();
        let source: Value = serde_json::from_str(&texte).unwrap();
        let n = compter(&source);
        let mut ch = Vec::new();
        chaines(&source, &mut ch);
        for _ in 0..200 {
            let mut v = source.clone();
            let mut k = a.jusqua(n);
            muter(&mut v, &mut k, &mut a, &ch);
            let Ok(p) = modele::Projet::lire_json(&v.to_string()) else { continue };
            lus += 1;
            if p.verifier().is_err() { continue }
            verifies += 1;
            if maillage::depuis_natif(&p, std::path::Path::new("t.bem")).is_ok() { scenes += 1; }
        }
    }
    eprintln!("lus {lus}, vérifiés {verifies}, scènes {scenes}");
    assert!(lus > 200 && verifies > 50, "mutations trop destructrices pour éprouver verifier()");
    assert_eq!(PANIQUES.load(std::sync::atomic::Ordering::Relaxed), 0, "paniques sur des projets mutés");
}
