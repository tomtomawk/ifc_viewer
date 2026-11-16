//! Mode console `--images` : images et vignettes d'un fichier Revit.

/// Écrit les images (types d'image) et les vignettes des apparences d'un fichier Revit.
pub(crate) fn rvt_images(path: &std::path::Path, dossier: &std::path::Path) -> i32 {
    let m = match formats::rvt::maquette::lire(path, &|_| {}) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    if let Err(e) = std::fs::create_dir_all(dossier) {
        eprintln!("Erreur : {e}");
        return 1;
    }
    let propre = |s: &str| s.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect::<String>();
    let mut n = 0;
    let mut ecrire = |nom: String, r: &formats::rvt::medias::Raster| {
        let f = dossier.join(format!("{}.{}", propre(&nom), r.format));
        match std::fs::write(&f, &r.donnees) {
            Ok(()) => {
                println!("{} ({} × {} px, {} octets)", f.display(), r.largeur_px, r.hauteur_px, r.donnees.len());
                n += 1;
            }
            Err(e) => eprintln!("{} : {e}", f.display()),
        }
    };
    for t in &m.types_images {
        if let Some(r) = m.rasters.get(&t.donnees) {
            ecrire(format!("image {} {}", t.id, t.nom), r);
        }
    }
    for (id, a) in &m.apparences {
        if let Some(r) = &a.vignette {
            ecrire(format!("apparence {id} {}", a.nom), r);
        }
    }
    // aperçus des types (composants de légende d'aperçu)
    for l in &m.legendes {
        if let Some(r) = &l.image {
            ecrire(format!("apercu {} {}", l.type_id, m.nom_element(-1_002_052, l.type_id).unwrap_or_default()), r);
        }
    }
    println!("{n} fichiers écrits");
    0
}
