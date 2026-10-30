//! Commandes non graphiques du modèle natif (utilisables aussi en CI).
use crate::modele::{self, Projet};
use std::path::Path;
pub fn executer(args: &[String]) -> Option<Result<(), String>> {
    let commande = args.first()?.as_str();
    if !matches!(
        commande,
        "--editer" | "--export-ifc" | "--export-vue" | "--export-feuille" | "--quantites"
    ) {
        return None;
    }
    Some((|| {
        let entree = args.get(1).ok_or("Fichier .modele.json requis.")?;
        let p = Projet::lire_json(&std::fs::read_to_string(entree).map_err(|e| e.to_string())?)?;
        p.verifier()?;
        let arg = args.get(2).ok_or("Argument manquant.")?;
        match commande {
            "--editer" => {
                let json = std::fs::read_to_string(arg).map_err(|e| e.to_string())?;
                let c = serde_json::from_str(&json).map_err(|e| e.to_string())?;
                let (p, ids) = p.executer(c)?;
                crate::scene::depuis_natif(&p, Path::new(entree))?;
                let sortie = args.get(3).map_or(entree, |s| s);
                modele::enregistrement::enregistrer_natif(&p, Path::new(sortie))?;
                println!("{}", serde_json::to_string(&ids).unwrap());
            }
            "--export-ifc" => modele::enregistrement::ecrire_atomique(
                Path::new(arg),
                modele::export_ifc::exporter(&p)?.as_bytes(),
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
                modele::enregistrement::ecrire_atomique(
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
                    modele::documentation::svg_feuille(&p, f)?
                } else {
                    modele::documentation::dessin_vue(&p, arg)?.svg(arg)
                };
                let b = match sortie.extension().and_then(|s| s.to_str()) {
                    Some("pdf") => modele::documentation::pdf(&svg)?,
                    Some("svg") => svg.into_bytes(),
                    _ => return Err("Extension attendue : .svg ou .pdf".into()),
                };
                modele::enregistrement::ecrire_atomique(sortie, &b)?;
            }
        };
        Ok(())
    })())
}
