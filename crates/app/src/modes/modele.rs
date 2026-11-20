//! Mode console `--modele` : conversion vers le modèle natif (projet JSON et rapport).

use std::path::PathBuf;

/// Conversion vers le modèle v0 : projet JSON, et rapport (Markdown) à côté et sur la
/// sortie standard.
pub(crate) fn modele(entree: PathBuf, sortie: PathBuf) -> i32 {
    let t0 = std::time::Instant::now();
    let e = entree.clone();
    let handle = match std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let progress = |s: String| eprintln!("{s}");
            if formats::rvt::is_revit_path(&e) { echanges::depuis_rvt::convertir(&e, &progress) } else { echanges::depuis_ifc::convertir(&e, &progress) }
        }) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Erreur : impossible de lancer la conversion ({e})");
            return 1;
        }
    };
    let (projet, mesures) = match handle.join() {
        Ok(Ok(x)) => x,
        Ok(Err(e)) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
        Err(_) => {
            eprintln!("Erreur : la conversion a échoué (panique)");
            return 1;
        }
    };
    let rapport = echanges::rapport::markdown(&projet, &mesures, t0.elapsed().as_secs_f64());
    let json = match projet.json() {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    // aller-retour : le fichier écrit se relit à l'identique
    match modele::Projet::lire_json(&json) {
        Ok(p) if p.murs.len() == projet.murs.len() && p.importes.len() == projet.importes.len() => {}
        Ok(_) => eprintln!("Attention : le projet relu diffère du projet écrit"),
        Err(e) => eprintln!("Attention : projet écrit illisible ({e})"),
    }
    let chemin_rapport = sortie.with_extension("rapport.md");
    if let Err(e) = std::fs::write(&sortie, json).and_then(|_| std::fs::write(&chemin_rapport, &rapport)) {
        eprintln!("Écriture impossible : {e}");
        return 1;
    }
    println!("{rapport}");
    eprintln!("Projet : {}\nRapport : {}", sortie.display(), chemin_rapport.display());
    0
}
