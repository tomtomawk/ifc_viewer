//! Mode console `--nomenclatures` : contenu calculé des nomenclatures d'un fichier Revit.

/// Contenu calculé de chaque nomenclature : une ligne JSON {id, nom, titres, lignes,
/// non_calculees}.
pub(crate) fn rvt_nomenclatures(path: &std::path::Path) -> i32 {
    let m = match formats::rvt::maquette::lire(path, &|_| {}) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    let j = |s: &str| pilote::texte(s);
    for v in &m.vues {
        let Some(d) = &v.nomenclature else { continue };
        let t = formats::rvt::nomenclatures::calculer(&m, d);
        let liste = |l: &[String]| format!("[{}]", l.iter().map(|x| j(x)).collect::<Vec<_>>().join(","));
        let lignes = t.lignes.iter().map(|l| liste(l)).collect::<Vec<_>>().join(",");
        let nc = t.non_calculees.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        // colonnes visibles calculées par une formule
        let formules = d.champs.iter().filter(|c| !c.masque).enumerate().filter(|(_, c)| c.genre == formats::rvt::nomenclatures::GenreChamp::Formule).map(|(k, _)| k.to_string()).collect::<Vec<_>>().join(",");
        println!("{{\"id\":{},\"nom\":{},\"titres\":{},\"lignes\":[{lignes}],\"non_calculees\":[{nc}],\"formules\":[{formules}]}}", v.id, j(&v.nom), liste(&t.titres));
    }
    0
}
