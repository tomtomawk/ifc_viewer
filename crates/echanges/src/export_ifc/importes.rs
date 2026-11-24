//! Éléments importés : classe IFC d'origine quand elle est sûre (sinon
//! `IfcBuildingElementProxy`), jeux de propriétés et de quantités lus à l'import,
//! matériaux et couleur.
use super::step::{guid, ref_, refs, texte, Step, Tri};
use modele::importes::{JeuProprietes, ProprieteImportee, TypeLogique};
use modele::ElementImporte;
use std::collections::HashMap;

/// Classes d'éléments physiques IFC4 dont les attributs sont ceux d'`IfcBuildingElementProxy`
/// (9 attributs, `PredefinedType` en dernier, `NOTDEFINED` admis).
const CLASSES: &[&str] = &[
    "IFCBEAM", "IFCBUILDINGELEMENTPART", "IFCBUILDINGELEMENTPROXY", "IFCCHIMNEY", "IFCCOLUMN", "IFCCOVERING", "IFCCURTAINWALL",
    "IFCFOOTING", "IFCFURNITURE", "IFCGEOGRAPHICELEMENT", "IFCMEMBER", "IFCPLATE", "IFCRAILING", "IFCRAMP", "IFCRAMPFLIGHT",
    "IFCROOF", "IFCSHADINGDEVICE", "IFCSLAB", "IFCSTAIR", "IFCSYSTEMFURNITUREELEMENT", "IFCWALL",
];

/// Classe IFC à écrire : la classe d'origine si elle est connue (les `…StandardCase`
/// et `…ElementedCase` ramenés à leur classe), sinon le proxy.
pub(super) fn classe(e: &ElementImporte) -> &'static str {
    let Some(o) = e.origine.as_ref() else { return "IFCBUILDINGELEMENTPROXY" };
    let c = o.classe.to_uppercase();
    let c = c.strip_suffix("STANDARDCASE").or_else(|| c.strip_suffix("ELEMENTEDCASE")).unwrap_or(&c);
    CLASSES.iter().find(|x| **x == c).copied().unwrap_or("IFCBUILDINGELEMENTPROXY")
}

/// `PredefinedType` lu à l'import, si la classe écrite est la classe d'origine (une
/// valeur d'énumération ne vaut que pour sa classe).
fn type_predefini(e: &ElementImporte) -> Option<&str> {
    let o = e.origine.as_ref()?;
    let c = o.classe.to_uppercase();
    let c = c.strip_suffix("STANDARDCASE").or_else(|| c.strip_suffix("ELEMENTEDCASE")).unwrap_or(&c);
    (classe(e) == c).then_some(e.donnees.type_predefini.as_deref()).flatten()
}

/// Réel STEP sans perte (mantisse avec point, exposant).
fn reel(v: f64) -> String {
    let s = format!("{v:E}");
    let (m, e) = s.split_once('E').unwrap_or((&s, "0"));
    if m.contains('.') { format!("{m}E{e}") } else { format!("{m}.E{e}") }
}

/// Valeur d'une propriété : mesure typée selon l'unité (longueurs remises en mm, unité du
/// fichier), `IfcReal` pour un autre nombre, `IfcLabel` (`IfcText` au-delà de 255
/// caractères) pour un texte ; `IfcBoolean` ou `IfcLogical` pour une valeur typée booléenne.
fn valeur(p: &ProprieteImportee) -> String {
    if let Some((t, c)) = p.logique.and_then(|t| Some((t, t.step(&p.valeur)?))) {
        return format!("{}(.{c}.)", if t == TypeLogique::Booleen { "IFCBOOLEAN" } else { "IFCLOGICAL" });
    }
    match (p.nombre(), p.unite.as_deref()) {
        (Some(v), Some("m")) => format!("IFCLENGTHMEASURE({})", reel(v * 1000.0)),
        (Some(v), Some("m²")) => format!("IFCAREAMEASURE({})", reel(v)),
        (Some(v), Some("m³")) => format!("IFCVOLUMEMEASURE({})", reel(v)),
        (Some(v), Some("kg")) => format!("IFCMASSMEASURE({})", reel(v)),
        (Some(v), Some("s")) => format!("IFCTIMEMEASURE({})", reel(v)),
        (Some(v), _) => format!("IFCREAL({})", reel(v)),
        (None, _) if p.valeur.chars().count() > 255 => format!("IFCTEXT({})", texte(&p.valeur)),
        (None, _) => format!("IFCLABEL({})", texte(&p.valeur)),
    }
}

/// Quantité d'un jeu de quantités, selon son unité.
fn quantite(p: &ProprieteImportee) -> Option<String> {
    let v = p.nombre().filter(|v| v.is_finite() && *v >= 0.0)?;
    let (entite, v) = match p.unite.as_deref() {
        Some("m") => ("IFCQUANTITYLENGTH", v * 1000.0),
        Some("m²") => ("IFCQUANTITYAREA", v),
        Some("m³") => ("IFCQUANTITYVOLUME", v),
        Some("kg") => ("IFCQUANTITYWEIGHT", v),
        Some("s") => ("IFCQUANTITYTIME", v),
        _ => ("IFCQUANTITYCOUNT", v),
    };
    Some(format!("{entite}({},$,$,{},$)", texte(&p.nom), reel(v)))
}

/// Écrit chaque jeu de l'élément (`IfcPropertySet` ou `IfcElementQuantity`) et sa relation.
fn jeux(w: &mut Step, cle: &str, objet: usize, jeux: &[JeuProprietes]) {
    for j in jeux {
        let ids: Vec<usize> = if j.quantites {
            j.proprietes.iter().filter_map(quantite).map(|q| w.add(q)).collect()
        } else {
            j.proprietes.iter().map(|p| w.add(format!("IFCPROPERTYSINGLEVALUE({},$,{},$)", texte(&p.nom), valeur(p)))).collect()
        };
        if ids.is_empty() {
            continue;
        }
        let jeu = if j.quantites {
            w.add(format!("IFCELEMENTQUANTITY('{}',$,{},$,$,{})", guid(&format!("qto:{}:{cle}", j.nom), None), texte(&j.nom), refs(&ids)))
        } else {
            w.add(format!("IFCPROPERTYSET('{}',$,{},$,{})", guid(&format!("pset:{}:{cle}", j.nom), None), texte(&j.nom), refs(&ids)))
        };
        w.add(format!("IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#{objet}),#{jeu})", guid(&format!("pset-rel:{}:{cle}", j.nom), None)));
    }
}

/// Jeux lus sur l'`IfcSpace` d'une pièce importée, sauf `Qto_SpaceBaseQuantities`,
/// recalculé du contour natif (`quantites::piece`).
pub(super) fn jeux_piece(w: &mut Step, x: &modele::Piece, objet: usize) {
    let lus: Vec<JeuProprietes> = x.donnees.proprietes.iter().filter(|j| j.nom != "Qto_SpaceBaseQuantities").cloned().collect();
    jeux(w, &x.id, objet, &lus);
}

/// Matériaux et styles des éléments importés, réutilisés d'un élément à l'autre.
#[derive(Default)]
pub(super) struct Importes {
    materiaux: HashMap<String, usize>,
    styles: HashMap<[u8; 4], usize>,
}

impl Importes {
    fn materiau(&mut self, w: &mut Step, nom: &str) -> usize {
        *self.materiaux.entry(nom.to_string()).or_insert_with(|| w.add(format!("IFCMATERIAL({},$,$)", texte(nom))))
    }

    fn style(&mut self, w: &mut Step, [r, g, b, a]: [u8; 4]) -> usize {
        *self.styles.entry([r, g, b, a]).or_insert_with(|| {
            let c = |x: u8| format!("{:.6}", x as f64 / 255.0);
            let couleur = w.add(format!("IFCCOLOURRGB($,{},{},{})", c(r), c(g), c(b)));
            let transparence = if a < 255 { format!("{:.6}", 1.0 - a as f64 / 255.0) } else { "$".into() };
            let ombrage = w.add(format!("IFCSURFACESTYLESHADING(#{couleur},{transparence})"));
            w.add(format!("IFCSURFACESTYLE('Import {r} {g} {b}',.BOTH.,(#{ombrage}))"))
        })
    }

    /// Élément importé : produit de sa classe, forme colorée, jeux, matériaux. Rend
    /// l'entité du produit.
    pub(super) fn ecrire(&mut self, w: &mut Step, contexte: usize, placement: usize, e: &ElementImporte, tris: &[Tri]) -> usize {
        let faces: Vec<usize> = w.faces(tris).into_iter().collect();
        if let (Some(&f), Some(c)) = (faces.first(), e.donnees.couleur) {
            let s = self.style(w, c);
            w.add(format!("IFCSTYLEDITEM(#{f},(#{s}),$)"));
        }
        let rep = w.forme(contexte, &faces, &[]);
        let id = w.add(format!(
            "{}('{}',$,{},{},{},#{placement},{},$,.{}.)",
            classe(e),
            guid(&e.id, e.origine.as_ref()),
            texte(&e.nom),
            texte(&e.raison),
            texte(&e.categorie),
            ref_(rep),
            type_predefini(e).unwrap_or("NOTDEFINED")
        ));
        jeux(w, &e.id, id, &e.donnees.proprietes);
        if let Some(m) = &e.donnees.materiau {
            let noms: Vec<&str> = m.split(" | ").map(str::trim).filter(|n| !n.is_empty()).collect();
            let ids: Vec<usize> = noms.iter().map(|n| self.materiau(w, n)).collect();
            let cible = match ids.as_slice() {
                [] => 0,
                [un] => *un,
                _ => w.add(format!("IFCMATERIALLIST({})", refs(&ids))),
            };
            if cible != 0 {
                w.add(format!("IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{id}),#{cible})", guid(&format!("mat:{}", e.id), None)));
            }
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reels_sans_perte() {
        assert_eq!(reel(0.1667), "1.667E-1");
        assert_eq!(reel(3.0), "3.E0");
        assert_eq!(reel(1.5e-5).parse::<f64>().unwrap(), 1.5e-5);
    }
}
