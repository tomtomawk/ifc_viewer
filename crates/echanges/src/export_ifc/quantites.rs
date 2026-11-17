//! Quantités de base (`Qto_*BaseQuantities`, IFC4 ADD2 TC1). Une seule source de calcul :
//! les fonctions de `modele` qu'appelle `modele::documentation::tableau` (nomenclatures
//! et `--quantites`), avec les mêmes conventions. Longueurs en mm (unité du fichier),
//! aires en m², volumes en m³ (`IfcUnitAssignment`).
use super::step::{guid, refs, texte, Step, Tri};
use modele::volumes::geometrie;
use modele::*;

pub(super) enum Quantite {
    Longueur(&'static str, f64),
    Aire(&'static str, f64),
    Volume(&'static str, f64),
}

impl Quantite {
    fn step(&self) -> Option<String> {
        let (entite, nom, v) = match *self {
            Quantite::Longueur(n, v) => ("IFCQUANTITYLENGTH", n, v),
            Quantite::Aire(n, v) => ("IFCQUANTITYAREA", n, v),
            Quantite::Volume(n, v) => ("IFCQUANTITYVOLUME", n, v),
        };
        // les quantités IFC sont positives ou nulles
        (v.is_finite() && v >= 0.0).then(|| format!("{entite}({},$,$,{v:.9},$)", texte(nom)))
    }
}

/// Écrit le jeu de quantités `nom` de l'objet `objet` (clé native `cle`) s'il en a.
pub(super) fn ecrire(w: &mut Step, nom: &str, cle: &str, objet: usize, quantites: Vec<Quantite>) {
    let ids: Vec<usize> = quantites.iter().filter_map(Quantite::step).map(|q| w.add(q)).collect();
    if ids.is_empty() {
        return;
    }
    let jeu = w.add(format!("IFCELEMENTQUANTITY('{}',$,{},$,'BaseQuantities',{})", guid(&format!("qto:{nom}:{cle}"), None), texte(nom), refs(&ids)));
    w.add(format!("IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#{objet}),#{jeu})", guid(&format!("qto-rel:{nom}:{cle}"), None)));
}

/// Mur : longueur de la ligne d'implantation, hauteur, épaisseur du type, surface
/// (longueur × hauteur, brute) et volume net du solide exporté (percé, raccordé).
pub(super) fn mur(w: &mut Step, p: &Projet, m: &Mur, solide: &[Tri], objet: usize) {
    use Quantite::*;
    let l = p.trajectoire_justification(m.assemblage.as_deref().unwrap_or(&m.id)).map_or_else(|| m.axe.longueur(), |(axe, _)| axe.longueur());
    let h = p.bas_haut(m).map(|(a, b)| b - a);
    let mut q = vec![Longueur("Length", l)];
    if let Some(t) = p.type_mur(&m.type_mur) {
        q.push(Longueur("Width", t.epaisseur()));
    }
    if let Some(h) = h {
        q.push(Longueur("Height", h));
        q.push(Aire("GrossSideArea", l * h / 1e6));
    }
    q.push(Volume("NetVolume", geometrie::volume(solide) / 1e9));
    ecrire(w, "Qto_WallBaseQuantities", &m.id, objet, q);
}

/// Dalle, sol, plafond ou toiture : épaisseur du type, aire des contours (trémies
/// déduites, en plan), volume net. L'aire en plan n'est l'aire nette que sans pente ;
/// en toiture, c'est l'aire projetée.
pub(super) fn dalle(w: &mut Step, p: &Projet, d: &Dalle, classe: &str, objet: usize) {
    use Quantite::*;
    let epaisseur = p.type_dalle(&d.type_dalle).map(|t| t.epaisseur());
    let aire = geometrie::aire_pair_impair(&d.contours) / 1e6;
    let plane = d.pentes.iter().flatten().all(Option::is_none);
    let mut q = Vec::new();
    let nom = match classe {
        "IFCROOF" => {
            q.push(Aire("ProjectedArea", aire));
            "Qto_RoofBaseQuantities"
        }
        _ => {
            q.extend(epaisseur.map(|e| Longueur("Width", e)));
            if plane {
                q.push(Aire("NetArea", aire));
            }
            if classe == "IFCCOVERING" {
                "Qto_CoveringBaseQuantities"
            } else {
                q.extend(p.volume_dalle(d).map(|v| Volume("NetVolume", v / 1e9)));
                "Qto_SlabBaseQuantities"
            }
        }
    };
    ecrire(w, nom, &d.id, objet, q);
}

/// Poutre ou poteau : longueur entre extrémités, volume du solide exporté.
pub(super) fn barre(w: &mut Step, b: &Barre, solide: &[Tri], objet: usize) {
    use Quantite::*;
    let l = glam::DVec3::from_array(b.fin).distance(glam::DVec3::from_array(b.debut));
    let nom = match b.genre {
        GenreBarre::Poteau => "Qto_ColumnBaseQuantities",
        GenreBarre::Poutre => "Qto_BeamBaseQuantities",
    };
    ecrire(w, nom, &b.id, objet, vec![Longueur("Length", l), Volume("NetVolume", geometrie::volume(solide) / 1e9)]);
}

/// Porte ou fenêtre : largeur, hauteur et aire de la baie.
pub(super) fn ouverture(w: &mut Step, o: &Ouverture, porte: bool, objet: usize) {
    use Quantite::*;
    let nom = if porte { "Qto_DoorBaseQuantities" } else { "Qto_WindowBaseQuantities" };
    ecrire(w, nom, &o.id, objet, vec![Longueur("Width", o.largeur), Longueur("Height", o.hauteur), Aire("Area", o.largeur * o.hauteur / 1e6)]);
}

/// Pièce : surface et périmètre de son contour.
pub(super) fn piece(w: &mut Step, x: &Piece, objet: usize) {
    use Quantite::*;
    ecrire(w, "Qto_SpaceBaseQuantities", &x.id, objet, vec![Aire("NetFloorArea", x.aire() / 1e6), Longueur("NetPerimeter", x.perimetre())]);
}

/// Garde-corps : longueur du tracé.
pub(super) fn garde_corps(w: &mut Step, g: &GardeCorps, objet: usize) {
    ecrire(w, "Qto_RailingBaseQuantities", &g.id, objet, vec![Quantite::Longueur("Length", g.longueur())]);
}
