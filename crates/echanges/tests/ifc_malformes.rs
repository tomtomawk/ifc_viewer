//! T1 : balayage aléatoire (mutations déterministes) des IFC d'exemple. Un fichier
//! malformé doit donner une erreur ou une scène partielle, jamais une panique : lecture
//! STEP, sémantique, géométrie et conversion en projet natif.
use std::path::Path;

/// Générateur pseudo-aléatoire (xorshift64*), graine fixe : mêmes mutations à chaque
/// exécution.
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

/// Une mutation : octet remplacé, jeton STEP inséré, plage supprimée ou dupliquée,
/// nombre remplacé par une valeur extrême, fichier tronqué.
fn muter(source: &[u8], a: &mut Alea) -> Vec<u8> {
    let mut d = source.to_vec();
    for _ in 0..1 + a.jusqua(4) {
        let n = d.len().max(1);
        let i = a.jusqua(n);
        match a.jusqua(6) {
            0 => if i < d.len() { d[i] = (a.suivant() & 0xff) as u8 },
            1 => {
                const JETONS: [&[u8]; 12] = [b"(", b")", b",", b"#", b"$", b"*", b"'", b"#999999", b"-1", b"1E308", b"NaN", b"((((((((("];
                let j = JETONS[a.jusqua(JETONS.len())];
                d.splice(i.min(d.len())..i.min(d.len()), j.iter().copied());
            }
            2 => { let fin = (i + a.jusqua(200)).min(d.len()); d.drain(i.min(fin)..fin); }
            3 => { let fin = (i + a.jusqua(400)).min(d.len()); let copie = d[i.min(fin)..fin].to_vec(); d.splice(fin..fin, copie); }
            4 => {
                // un nombre devient extrême
                if let Some(p) = d[i.min(d.len())..].iter().position(|c| c.is_ascii_digit()) {
                    let p = i + p;
                    let fin = p + d[p..].iter().take_while(|c| c.is_ascii_digit() || **c == b'.').count();
                    let v: &[u8] = [b"99999999999" as &[u8], b"0.", b"1E300", b"-99999999"][a.jusqua(4)];
                    d.splice(p..fin, v.iter().copied());
                }
            }
            _ => d.truncate(i),
        }
    }
    d
}

#[test]
fn ifc_mutes_sans_panique() {
    // toute panique compte, même rattrapée (géométrie d'un produit, triangulation)
    static PANIQUES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let defaut = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        PANIQUES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        defaut(info);
    }));
    let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let sortie = std::env::temp_dir().join(format!("bimedit-ifc-malformes-{}", std::process::id()));
    std::fs::create_dir_all(&sortie).unwrap();
    let mut a = Alea(0x9e37_79b9_7f4a_7c15);
    let mut essais = 0;
    for nom in ["test_ifc4.ifc", "test_ifc4_murs.ifc", "test_ifc4_espaces.ifc", "test_ifc4_proprietes.ifc"] {
        let source = std::fs::read(dossier.join(nom)).unwrap();
        for k in 0..150 {
            let chemin = sortie.join(format!("{k}-{nom}"));
            std::fs::write(&chemin, muter(&source, &mut a)).unwrap();
            // erreur acceptée ; une panique fait échouer le test (fichier gardé pour l'analyse)
            let _ = formats::ifc::assemblage::load(&chemin, &|_| {});
            let _ = echanges::depuis_ifc::convertir(&chemin, &|_| {});
            std::fs::remove_file(&chemin).unwrap();
            essais += 1;
        }
    }
    let _ = std::fs::remove_dir(&sortie);
    assert_eq!(essais, 600);
    assert_eq!(PANIQUES.load(std::sync::atomic::Ordering::Relaxed), 0, "paniques pendant la lecture des fichiers mutés");
}
