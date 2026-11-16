//! Mode console `--parametres` : valeurs brutes des paramètres d'un fichier Revit.

/// Valeurs de paramètres décodées, une par ligne : élément, paramètre, stockage, valeur
/// brute (unités internes de Revit), pour comparaison avec l'API.
pub(crate) fn rvt_parametres(path: &std::path::Path) -> i32 {
    use formats::rvt::parametres::{Brut, Stockage, integres};
    let m = match formats::rvt::maquette::lire(path, &|_| {}) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    let mut ids: Vec<&i64> = m.parametres.keys().collect();
    ids.sort();
    for e in ids {
        for (p, b) in &m.parametres[e] {
            let st = m.definitions.get(p).map(|d| d.stockage).or_else(|| integres().get(p).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
            let (s, v) = match b.selon(st) {
                Brut::Reel(x) => ("Double", format!("{x:?}")),
                Brut::Entier(i) => ("Integer", i.to_string()),
                Brut::Id(i) => ("ElementId", i.to_string()),
                Brut::Texte(t) => ("String", t.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")),
                Brut::Nomme { .. } => ("?", String::new()),
            };
            println!("{e}\t{p}\t{s}\t{v}");
        }
    }
    // IfcGUID calculés (paramètres IFC_GUID et IFC_TYPE_GUID de l'API)
    let mut ids: Vec<&i64> = m.episode_de.keys().collect();
    ids.sort();
    for e in ids {
        if let Some(g) = m.guid_ifc(*e) {
            println!("{e}\tifcguid\tString\t{g}");
        }
    }
    0
}
