//! Commandes non graphiques du modèle natif (utilisables aussi en CI).
use crate::dessin;
use crate::modele::{self, Projet};
use std::path::Path;
pub fn executer(args: &[String]) -> Option<Result<(), String>> {
    let commande = args.first()?.as_str();
    if !matches!(
        commande,
        "--editer" | "--export-ifc" | "--export-vue" | "--export-feuille" | "--quantites" | "--mesurer-scene"
    ) {
        return None;
    }
    Some((|| {
        let entree = args.get(1).ok_or("Fichier .modele.json requis.")?;
        let p = Projet::lire_json(&std::fs::read_to_string(entree).map_err(|e| e.to_string())?)?;
        p.verifier()?;
        if commande == "--mesurer-scene" {
            // construction complète, puis reconstruction après le déplacement d'un mur ;
            // résultats sur la sortie d'erreur avec les étapes (BIMEDIT_CHRONO)
            let chemin = Path::new(entree);
            let debut = std::time::Instant::now();
            crate::rendu::scene::depuis_natif(&p, chemin)?;
            eprintln!("scène complète : {:.0} ms", debut.elapsed().as_secs_f64() * 1000.0);
            let repetitions: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(1);
            let mut courant = p;
            for k in 0..repetitions {
                let Some(id) = courant.murs.get(k * 7 % courant.murs.len().max(1)).map(|m| m.assemblage.clone().unwrap_or_else(|| m.id.clone())) else { break };
                let debut = std::time::Instant::now();
                let (suivant, _) = courant.executer(crate::edition::commandes::Commande::Deplacer { ids: vec![id], delta: [0.0, 50.0, 0.0] })?;
                let t_commande = debut.elapsed().as_secs_f64() * 1000.0;
                let debut = std::time::Instant::now();
                crate::rendu::scene::depuis_natif(&suivant, chemin)?;
                eprintln!("mur déplacé : commande {:.0} ms, scène {:.0} ms", t_commande, debut.elapsed().as_secs_f64() * 1000.0);
                courant = suivant;
            }
            return Ok(());
        }
        let arg = args.get(2).ok_or("Argument manquant.")?;
        match commande {
            "--editer" => {
                if Path::new(entree).with_extension("collaboration.json").exists() {
                    if args.get(3).is_some_and(|s| s != entree) { return Err("En collaboration, utilisez une copie locale distincte pour Enregistrer sous".into()); }
                    return crate::collaboration::cli::executer(&["--collaboration".into(),"editer".into(),entree.clone(),arg.clone()]).ok_or("Routage collaboratif absent")?;
                }
                let json = std::fs::read_to_string(arg).map_err(|e| e.to_string())?;
                let c = serde_json::from_str(&json).map_err(|e| e.to_string())?;
                let (p, ids) = p.executer(c)?;
                crate::rendu::scene::depuis_natif(&p, Path::new(entree))?;
                let sortie = args.get(3).map_or(entree, |s| s);
                crate::echanges::enregistrement::enregistrer_natif(&p, Path::new(sortie))?;
                println!("{}", serde_json::to_string(&ids).unwrap());
            }
            "--export-ifc" => crate::echanges::enregistrement::ecrire_atomique(
                Path::new(arg),
                crate::echanges::export_ifc::exporter(&p)?.as_bytes(),
            )?,
            "--quantites" => {
                let n = modele::documentation::Nomenclature {
                    id: "rapport".into(),
                    nom: "Quantités".into(),
                    categorie: args.get(3).cloned().unwrap_or_default(),
                    regrouper: true,
                    colonnes: Vec::new(),
                    parametres: Vec::new(),
                };
                let lignes = modele::documentation::tableau(&p, &n)?;
                crate::echanges::enregistrement::ecrire_atomique(
                    Path::new(arg),
                    modele::documentation::csv(&lignes).as_bytes(),
                )?;
            }
            _ => {
                let sortie = Path::new(args.get(3).ok_or("Chemin de sortie SVG ou PDF requis.")?);
                let svg = if commande == "--export-feuille" {
                    let f = p
                        .feuilles
                        .iter()
                        .find(|f| f.id == *arg || f.numero == *arg)
                        .ok_or("Feuille absente.")?;
                    dessin::svg_feuille(&p, f)?
                } else {
                    dessin::dessin_vue(&p, arg)?.svg(arg)
                };
                let b = match sortie.extension().and_then(|s| s.to_str()) {
                    Some("pdf") => dessin::pdf(&svg)?,
                    Some("svg") => svg.into_bytes(),
                    _ => return Err("Extension attendue : .svg ou .pdf".into()),
                };
                crate::echanges::enregistrement::ecrire_atomique(sortie, &b)?;
            }
        };
        Ok(())
    })())
}
