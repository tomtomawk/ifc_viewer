//! Mode console `--decodage` : part décodée d'un fichier Revit, en JSON.

pub(crate) fn rvt_decodage(path: &std::path::Path) -> i32 {
    match formats::rvt::maquette::decodage(path) {
        Ok(d) => {
            println!(
                "{{\"non_interpretees\":[{}],\"flux\":[{},{}],\"non_lus\":[{}],\"elements\":[{},{},{}],\"octets_elements\":[{},{},{}],\"octets_nommes\":{},\"geometries\":[{},{}],\"octets_geometries\":[{},{}]}}",
                d.non_interpretees.iter().map(|(c, n, o)| format!("[{},{n},{o}]", pilote::texte(c))).collect::<Vec<_>>().join(","),
                d.flux.0,
                d.flux.1,
                d.non_lus.iter().map(|(p, n)| format!("[{},{n}]", pilote::texte(p))).collect::<Vec<_>>().join(","), d.elements.0, d.elements.1, d.elements.2, d.octets_elements.0, d.octets_elements.1, d.octets_elements.2, d.octets_nommes, d.geometries.0, d.geometries.1, d.octets_geometries.0, d.octets_geometries.1
            );
            0
        }
        Err(e) => {
            eprintln!("Erreur : {e}");
            1
        }
    }
}
