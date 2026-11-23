//! Feuilles : jeux, cartouches, vues et nomenclatures placées, export SVG et PDF.
use super::*;

const TAILLE_CARTOUCHE_STANDARD: [f64; 2] = [180.0, 40.0];
pub const NOM_CARTOUCHE_STANDARD: &str = "Cartouche standard";

impl Default for FamilleCartouche {
    fn default() -> Self {
        use crate::famille::modelee::{CategorieFamille, Element2DFamille, FamilleModelee, FichierFamille, VueEsquisse, FORMAT_FAMILLE};
        let [largeur, hauteur] = TAILLE_CARTOUCHE_STANDARD;
        let traits = [[[0.0,0.0],[180.0,0.0]], [[180.0,0.0],[180.0,40.0]], [[180.0,40.0],[0.0,40.0]], [[0.0,40.0],[0.0,0.0]], [[140.0,0.0],[140.0,40.0]], [[0.0,14.0],[140.0,14.0]], [[0.0,30.0],[140.0,30.0]]];
        let textes = [("{projet}", [3.0,33.0], 3.5), ("{feuille}", [3.0,20.0], 5.0), ("{cartouche}", [3.0,9.0], 2.5), ("{echelles}", [3.0,3.5], 2.5), ("{numero}", [144.0,15.0], 9.0)];
        let mut famille = FamilleModelee::vide(CategorieFamille::Cartouche);
        famille.elements_2d.extend(traits.iter().map(|t| Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Ligne { debut: t[0], fin: t[1] }, 2.5)));
        famille.elements_2d.extend(textes.iter().map(|(t, point, h)| Element2DFamille::fixe(VueEsquisse::Plan, AnnotationForme::Texte { point: *point, texte: (*t).into() }, *h)));
        Self {
            document_famille: Box::new(FichierFamille { format: FORMAT_FAMILLE.into(), nom: NOM_CARTOUCHE_STANDARD.into(), largeur, hauteur, profondeur: 300.0, famille }),
            id: noyau::uuid::uuid(), nom: NOM_CARTOUCHE_STANDARD.into(), largeur, hauteur,
        }
    }
}

/// Jeux de feuilles du projet, dans l'ordre de leur première feuille.
pub fn jeux(p: &Projet) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for f in &p.feuilles {
        if !v.contains(&f.jeu) {
            v.push(f.jeu.clone());
        }
    }
    v
}

/// Déplace la feuille `id` dans le jeu `jeu`, juste avant la feuille `avant` ; sans
/// `avant` (ou s'il n'est pas dans ce jeu), après la dernière feuille du jeu.
pub fn deplacer_feuille(p: &mut Projet, id: &str, jeu: &str, avant: Option<&str>) -> Result<(), String> {
    let k = p.feuilles.iter().position(|f| f.id == id).ok_or("Feuille absente.")?;
    let mut f = p.feuilles.remove(k);
    f.jeu = jeu.into();
    let rang = avant
        .filter(|a| *a != id)
        .and_then(|a| p.feuilles.iter().position(|g| g.id == a && g.jeu == jeu))
        .or_else(|| p.feuilles.iter().rposition(|g| g.jeu == jeu).map(|r| r + 1))
        .unwrap_or(p.feuilles.len());
    p.feuilles.insert(rang, f);
    Ok(())
}

pub fn pt_papier(x: f64, y: f64) -> DVec3 {
    DVec3::new(x / 1000.0, y / 1000.0, 0.0)
}

pub fn trait_papier(points: Vec<DVec3>, fermee: bool, epaisseur: f64) -> Primitive {
    Primitive { forme: Forme::Polyligne { points, fermee }, style: Style { couleur: [20, 20, 20, 255], epaisseur }, role: Role::Annotation, element: None }
}

/// Famille de cartouche de nom `nom` (vide : la standard) ; None pour la standard non
/// enregistrée dans le projet.
pub fn famille_cartouche<'a>(p: &'a Projet, nom: &str) -> Option<&'a FamilleCartouche> {
    let nom = if nom.is_empty() { NOM_CARTOUCHE_STANDARD } else { nom };
    p.familles_cartouches.iter().find(|f| f.nom == nom)
}

/// Taille d'un cartouche de la famille `nom` (mm) ; standard si elle est absente.
pub fn taille_cartouche(p: &Projet, nom: &str) -> [f64; 2] {
    famille_cartouche(p, nom).map_or(TAILLE_CARTOUCHE_STANDARD, |c| [c.largeur, c.hauteur])
}

/// Cartouche de la famille `nom` en bas à droite de la feuille, dans la marge.
pub fn cartouche_par_defaut(p: &Projet, f: &Feuille, nom: &str) -> CartouchePlace {
    CartouchePlace { famille: if nom == NOM_CARTOUCHE_STANDARD { String::new() } else { nom.into() }, position: [f.largeur - MARGE - taille_cartouche(p, nom)[0], MARGE] }
}

/// Cadre d'un cartouche posé [x0, y0, x1, y1] (mm du papier).
pub fn cadre_cartouche(p: &Projet, c: &CartouchePlace) -> [f64; 4] {
    let [l, h] = taille_cartouche(p, &c.famille);
    let [x, y] = c.position;
    [x, y, x + l, y + h]
}

pub fn trait_cartouche(t: [[f64; 2]; 2]) -> Primitive {
    trait_papier(t.map(|[x, y]| pt_papier(x, y)).to_vec(), false, 0.25)
}

pub fn texte_cartouche(t: &TexteCartouche, contenu: &str) -> Primitive {
    texte(contenu, pt_papier(t.position[0], t.position[1]), DVec3::X, DVec3::Y, t.hauteur / 1000.0, [20, 20, 20, 255])
}

/// Primitives d'une forme dessinée (ligne, polyligne avec ses types de trait et son
/// remplissage, spline, cercle, arc) : points de la forme passés par `pt`, tirets à
/// l'échelle `echelle` (dénominateur ; 1 : mm du papier).
pub fn primitives_dessin(p: &Projet, forme: &AnnotationForme, echelle: f64, pt: &dyn Fn(DVec2) -> DVec3) -> Vec<Primitive> {
    primitives_forme_dessin(p, forme, echelle, pt)
}

pub fn forme_cartouche(p: &Projet, forme: &AnnotationForme) -> Vec<Primitive> {
    let pt = |v: DVec2| pt_papier(v.x, v.y);
    // cotes : même géométrie que dans les vues, en mm du papier (texte de 2,5 mm)
    let longueur = |x: f64| format!("{x:.1} mm");
    let altitude = |q: [f64; 2], _| q[1];
    let style = StyleCote { oblique: 1.5, taille: 2.5, longueur: &longueur, altitude: &altitude };
    if let Some((traits, textes)) = GenreCote::de_forme(forme).and_then(|_| geometrie(forme, None, &style)) {
        let mut out: Vec<_> = traits.iter().map(|[x, y]| ligne(pt(*x), pt(*y))).collect();
        out.extend(textes.into_iter().map(|(pos, dir, s)| {
            let (du, dv) = (DVec3::new(dir.x, dir.y, 0.0), DVec3::new(-dir.y, dir.x, 0.0));
            let l = noyau::texte::largeur(&s, "Arial", 0.0025, false);
            texte(&s, pt(pos) - du * (l / 2.0), du, dv, 0.0025, [20, 20, 20, 255])
        }));
        out
    } else {
        primitives_forme_dessin(p, forme, 1.0, &pt)
    }
}

/// Cartouches et annotations d'une feuille, en mètres du papier (origine en bas à
/// gauche). Le format de papier ne produit aucun cadre imprimé automatique.
pub fn cadre_feuille(p: &Projet, f: &Feuille) -> Vec<Primitive> {
    let mut v: Vec<Primitive> = f.annotations.iter().flat_map(|a| primitives_annotation_feuille(p, f, a)).collect();
    v.extend(cartouche_feuille(p, f));
    v
}

/// Cartouches d'une feuille, en mètres du papier ; vide sans cartouche.
pub fn cartouche_feuille(p: &Projet, f: &Feuille) -> Vec<Primitive> {
    f.cartouches.iter().flat_map(|c| primitives_cartouche(p, f, c)).collect()
}

/// Un cartouche posé, en mètres du papier : éléments 2D du plan de sa famille, champs
/// remplis.
pub fn primitives_cartouche(p: &Projet, f: &Feuille, c: &CartouchePlace) -> Vec<Primitive> {
    let mut v = Vec::new();
    let standard;
    let famille = if let Some(famille) = famille_cartouche(p, &c.famille) {
        famille
    } else {
        standard = FamilleCartouche::default();
        &standard
    };
    let origine = pt_papier(c.position[0], c.position[1]);
    let (formes, textes) = super::contenu_plan_famille(&famille.document_famille, p);
    v.extend(formes.iter().flat_map(|forme| forme_cartouche(p, forme)).map(|pr| pr.translater(origine)));
    let mut echelles: Vec<String> = f.vues.iter().map(|x| format!("1:{}", x.echelle_sur_feuille(p))).collect();
    echelles.dedup();
    let ech = if echelles.is_empty() { String::new() } else { format!("Échelle {}", echelles.join(", ")) };
    for t in &textes {
        let contenu = super::remplir_champs(&t.contenu, &[("{projet}", &p.nom), ("{feuille}", &f.nom), ("{numero}", &f.numero), ("{cartouche}", &f.cartouche), ("{echelles}", &ech)]);
        if !contenu.is_empty() {
            v.push(texte_cartouche(t, &contenu).translater(origine));
        }
    }
    v
}

/// Tableau d'une nomenclature posée sur une feuille, en mètres du papier : titre
/// (nom), ligne d'en-tête, lignes ; colonnes à la largeur de leur plus long texte. Et
/// son étendue en mm du papier [x0, y0, x1, y1].
pub fn nomenclature_placee(p: &Projet, n: &NomenclaturePlacee) -> Result<(Vec<Primitive>, [f64; 4]), String> {
    let nom = p.nomenclatures.iter().find(|x| x.id == n.nomenclature).ok_or("Nomenclature absente.")?;
    let lignes = tableau(p, nom)?;
    // mm du papier : texte 2,5, en-tête et titre plus grands
    let (texte_h, ligne_h, marge, titre_h) = (2.5, 6.0, 2.0, 3.5);
    let largeur = |s: &str, h: f64| noyau::texte::ligne(s, "Arial", h, false).largeur;
    let ncol = lignes.iter().map(Vec::len).max().unwrap_or(0);
    let mut colonnes = vec![12.0_f64; ncol];
    for l in &lignes {
        for (i, c) in l.iter().enumerate() {
            colonnes[i] = colonnes[i].max(largeur(c, texte_h) + 2.0 * marge);
        }
    }
    let total: f64 = colonnes.iter().sum::<f64>().max(largeur(&nom.nom, titre_h) + 2.0 * marge);
    let [x0, y0] = n.position;
    let noir = [20, 20, 20, 255];
    let mut prims = Vec::new();
    prims.push(texte(&nom.nom, pt_papier(x0 + marge, y0 - titre_h - 1.5), DVec3::X, DVec3::Y, titre_h / 1000.0, noir));
    let haut = y0 - titre_h - 4.0;
    let bas = haut - ligne_h * lignes.len() as f64;
    // cadre, lignes, colonnes ; en-tête souligné plus fort
    prims.push(trait_papier(vec![pt_papier(x0, haut), pt_papier(x0 + total, haut), pt_papier(x0 + total, bas), pt_papier(x0, bas)], true, 0.35));
    for k in 1..lignes.len() {
        let y = haut - ligne_h * k as f64;
        prims.push(trait_papier(vec![pt_papier(x0, y), pt_papier(x0 + total, y)], false, if k == 1 { 0.35 } else { 0.13 }));
    }
    let mut x = x0;
    for w in colonnes.iter().take(ncol.saturating_sub(1)) {
        x += w;
        prims.push(trait_papier(vec![pt_papier(x, haut), pt_papier(x, bas)], false, 0.13));
    }
    for (k, l) in lignes.iter().enumerate() {
        let y = haut - ligne_h * (k as f64 + 1.0) + (ligne_h - texte_h) * 0.5;
        let mut x = x0;
        for (i, c) in l.iter().enumerate() {
            if !c.is_empty() {
                prims.push(texte(c, pt_papier(x + marge, y), DVec3::X, DVec3::Y, texte_h / 1000.0, noir));
            }
            x += colonnes[i];
        }
    }
    Ok((prims, [x0, bas, x0 + total, y0]))
}

pub fn csv(lignes: &[Vec<String>]) -> String {
    lignes
        .iter()
        .map(|l| {
            l.iter()
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}
