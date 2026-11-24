//! Génère `samples/essai_isolants.bem` : projet d'essai du dessin des isolants (ligne
//! en S) en plan et en coupe, amorcé par le gabarit par défaut.
//!
//! Murs béton + doublage isolé en L et en T, mur percé de baies, mur oblique à 30°, mur
//! courbe, murs à ossature bois ; dalle à chape isolée et toiture tuiles isolée. Vues : plan
//! du rez-de-chaussée, coupe perpendiculaire aux murs, coupe oblique (à 30° de la perpendiculaire au mur oblique), coupe presque
//! parallèle à un mur (repli sur le calcul d'après la forme), feuille A2.
//!
//! `cargo run --release -p echanges --example essai_isolants [sortie.bem]`
use commandes::{Commande, Execution};
use modele::creation::{Implantation, ParametresDalle, ParametresMur};
use modele::*;
use serde_json::{json, Value};

type R<T> = Result<T, String>;

fn cmd(p: &mut Projet, c: Value) -> R<Vec<String>> {
    let c: Commande = serde_json::from_value(c.clone()).map_err(|e| format!("{e} : {c}"))?;
    let (q, ids) = p.executer(c)?;
    *p = q;
    Ok(ids)
}

fn type_mur(p: &Projet, nom: &str) -> R<String> {
    p.types_murs.iter().find(|t| t.nom == nom).map(|t| t.id.clone()).ok_or(format!("type de mur absent : {nom}"))
}

fn type_dalle(p: &Projet, nom: &str) -> R<TypeCompose> {
    p.types_dalles.iter().find(|t| t.nom == nom).cloned().ok_or(format!("type de dalle absent : {nom}"))
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

fn construire() -> R<Projet> {
    let mut p = Projet::depuis_gabarit("Essai des isolants", &modele::gabarit::Gabarit::defaut());
    let rdc = p.niveaux.first().ok_or("gabarit sans niveau")?.id.clone();
    let (ba, ossature) = (type_mur(&p, "Mur BA 200 + doublage isolé")?, type_mur(&p, "Mur ossature bois 145")?);
    let mur = |p: &mut Projet, axe: Axe, t: &str, implantation: Implantation| -> R<String> {
        let prm = ParametresMur { hauteur: 2700.0, implantation, niveau: Some(rdc.clone()), type_mur: Some(t.into()), ..Default::default() };
        p.ajouter_mur_axe(axe, &prm)
    };
    let droit = |a: [f64; 2], b: [f64; 2]| Axe::Droit { debut: a, fin: b };
    use Implantation::{Axe as A, Exterieure as E};
    // L béton + doublage (extérieur à gauche : tracé horaire)
    mur(&mut p, droit([0.0, 0.0], [0.0, 6000.0]), &ba, E)?;
    let percé = mur(&mut p, droit([0.0, 6000.0], [10000.0, 6000.0]), &ba, E)?;
    // T : refend isolé contre le mur percé
    mur(&mut p, droit([5000.0, 5652.0], [5000.0, 1000.0]), &ba, A)?;
    // mur oblique à 30°
    let (c30, s30) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    mur(&mut p, droit([12000.0, 0.0], [12000.0 + 6000.0 * c30, 6000.0 * s30]), &ba, E)?;
    // mur courbe
    mur(&mut p, Axe::Arc { debut: [0.0, -2000.0], fin: [-4000.0, 2000.0], centre: [-4000.0, -2000.0], direct: true }, &ba, E)?;
    // ossature bois en L
    mur(&mut p, droit([12000.0, 9000.0], [12000.0, 14000.0]), &ossature, E)?;
    mur(&mut p, droit([12000.0, 14000.0], [18000.0, 14000.0]), &ossature, E)?;
    // baies dans le mur percé
    let hote = modele::volumes::solides::hote(&p, &percé)?.0;
    let Axe::Droit { debut, .. } = hote else { return Err("axe".into()) };
    for x in [2000.0, 3600.0, 7500.0] {
        cmd(&mut p, json!({"action": "ouverture", "mur": percé, "genre": "libre", "type_ouverture": null, "abscisse": (x - debut[0]).abs(), "largeur": 900.0, "hauteur": 2100.0, "allege": 0.0}))?;
    }
    // dalle à chape isolée et toiture isolée
    let dalle = |p: &mut Projet, c: Vec<[f64; 2]>, sorte: SorteDalle, t: &str, decalage: f64, pente: Option<f64>| -> R<String> {
        let prm = ParametresDalle { sorte, type_dalle: type_dalle(p, t)?, pente, niveau: Some(rdc.clone()), decalage, phase: None, sous_projet: None, variante: None };
        p.ajouter_dalle(&c, &prm)
    };
    dalle(&mut p, rect(0.0, 0.0, 10000.0, 6000.0), SorteDalle::Dalle, "Dalle BA 230 + chape isolée", 0.0, None)?;
    dalle(&mut p, rect(-300.0, -300.0, 10300.0, 6300.0), SorteDalle::Toiture, "Toiture tuiles isolée", 2700.0, Some(30.0))?;
    // vues
    let plan = p.vues.iter().find(|v| matches!(v.genre, GenreVue::Plan { .. })).map(|v| v.id.clone()).ok_or("plan absent")?;
    p.renommer_vue(&plan, "Plan des isolants")?;
    let mut coupes = Vec::new();
    for (nom, a, b) in [
        ("Coupe perpendiculaire", [2500.0, -1500.0], [2500.0, 8000.0]),
        ("Coupe oblique 30°", [11000.0, 4500.0], [11000.0 + 9000.0 * (30f64.to_radians()).cos(), 4500.0 - 9000.0 * (30f64.to_radians()).sin()]),
        ("Coupe presque parallèle", [-500.0, 5720.0], [10500.0, 5900.0]),
    ] {
        let id = p.ajouter_vue_coupe(a, b, 4000.0, false, "Coupe de bâtiment", 50.0)?;
        p.renommer_vue(&id, nom)?;
        coupes.push(id);
    }
    cmd(&mut p, json!({"action": "feuille", "feuille": {"id": "", "numero": "I01", "nom": "Isolants", "largeur": 594.0, "hauteur": 420.0, "cartouche": "Essai des isolants",
        "vues": [{"vue": plan, "position": [200.0, 250.0], "echelle": 100.0}, {"vue": coupes[0], "position": [430.0, 300.0], "echelle": 50.0},
                 {"vue": coupes[1], "position": [430.0, 120.0], "echelle": 50.0}, {"vue": coupes[2], "position": [150.0, 70.0], "echelle": 50.0}]}}))?;
    p.verifier()?;
    Ok(p)
}

fn main() {
    let sortie = std::env::args().nth(1).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/essai_isolants.bem").into());
    let r = construire().and_then(|p| {
        maillage::depuis_natif(&p, std::path::Path::new(&sortie))?;
        echanges::enregistrement::enregistrer_natif(&p, std::path::Path::new(&sortie))?;
        Ok(p)
    });
    match r {
        Ok(p) => println!("{sortie} : {} murs, {} vues", p.murs.len(), p.vues.len()),
        Err(e) => {
            eprintln!("Erreur : {e}");
            std::process::exit(1);
        }
    }
}
