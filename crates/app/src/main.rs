#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


mod modes;

use std::path::PathBuf;

fn main() {
    // Pont MCP (stdio) : rien d'autre ne doit écrire sur la sortie standard avant lui
    if std::env::args().nth(1).as_deref() == Some("--mcp") {
        pilote::mcp::servir();
        return;
    }
    modele::unite::definir_unite(config::options::Options::charger().0.unite);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(resultat)=collaboration::cli::executer(&args){match resultat{Ok(())=>std::process::exit(0),Err(e)=>{eprintln!("{e}");std::process::exit(2);}}}
    if let Some(resultat) = pilote::cli::executer(&args) {
        match resultat { Ok(())=>std::process::exit(0),Err(e)=>{eprintln!("{e}");std::process::exit(2);} }
    }

    // Mode console : bimedit --images fichier.rvt dossier (images et vignettes enregistrées)
    if args.first().map(|s| s.as_str()) == Some("--images") {
        let (Some(path), Some(dossier)) = (args.get(1), args.get(2)) else {
            eprintln!("Usage : bimedit --images <fichier.rvt|.rfa|.rte> <dossier>");
            std::process::exit(2);
        };
        std::process::exit(modes::rvt_images::rvt_images(&PathBuf::from(path), &PathBuf::from(dossier)));
    }
    // Mode console : bimedit --decodage fichier.rvt|.rfa|.rte (part décodée, JSON)
    if args.first().map(|s| s.as_str()) == Some("--decodage") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : bimedit --decodage <fichier.rvt|.rfa|.rte>");
            std::process::exit(2);
        };
        std::process::exit(modes::rvt_decodage::rvt_decodage(&PathBuf::from(path)));
    }
    // Mode console : bimedit --nomenclatures fichier.rvt (contenu calculé, JSON par ligne)
    if args.first().map(|s| s.as_str()) == Some("--nomenclatures") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : bimedit --nomenclatures <fichier.rvt>");
            std::process::exit(2);
        };
        std::process::exit(modes::rvt_nomenclatures::rvt_nomenclatures(&PathBuf::from(path)));
    }
    // Mode console : bimedit --parametres fichier.rvt (valeurs brutes, unités internes)
    if args.first().map(|s| s.as_str()) == Some("--parametres") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : bimedit --parametres <fichier.rvt|.rfa|.rte>");
            std::process::exit(2);
        };
        std::process::exit(modes::rvt_parametres::rvt_parametres(&PathBuf::from(path)));
    }
    // Mode console : bimedit --modele fichier.rvt|.ifc [sortie.bem]
    if args.first().map(|s| s.as_str()) == Some("--modele") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : bimedit --modele <fichier.rvt|.rfa|.ifc> [sortie.bem]");
            std::process::exit(2);
        };
        let entree = PathBuf::from(path);
        let sortie = args.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(path).with_extension(echanges::enregistrement::EXTENSION_PROJET));
        std::process::exit(modes::modele::modele(entree, sortie));
    }
    // Mode console : bimedit --stats fichier.ifc
    if args.first().map(|s| s.as_str()) == Some("--stats") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : bimedit --stats <fichier.ifc|.rvt|.rfa|.rte>");
            std::process::exit(2);
        };
        let path = PathBuf::from(path);
        let code = if formats::rvt::is_revit_path(&path) { modes::rvt_stats::rvt_stats(&path) } else { modes::stats::stats(path) };
        std::process::exit(code);
    }

    let path = args.first().map(PathBuf::from);
    let resultat = interface::socle::fenetre::lancer(path);
    // plus joignable : le pont MCP ne doit plus la proposer
    pilote::instances::retirer();
    if let Err(e) = resultat {
        eprintln!("Erreur : {e}");
        std::process::exit(1);
    }
}
