//! Jeux de propriétés (`IfcPropertySet`) et de quantités (`IfcElementQuantity`) d'un
//! objet et de son type : lecture structurée (valeur, unité SI de base) et affichage.
//!
//! Les mesures sont converties en unités SI de base : longueurs en m, aires en m²,
//! volumes en m³ (unité propre de la quantité si elle en a une, sinon `LENGTHUNIT`,
//! `AREAUNIT`, `VOLUMEUNIT` du projet), masses en kg.
use crate::semantique::{IfcModel, PropertyGroup};
use crate::step::{format_real, Entity, Id, Value};

/// Propriété lue : nom, valeur en texte, unité, et valeur numérique si c'en est une.
#[derive(Debug, Clone, PartialEq)]
pub struct Propriete {
    pub nom: String,
    pub valeur: String,
    pub unite: Option<&'static str>,
    pub nombre: Option<f64>,
}

impl Propriete {
    /// Valeur affichée : nombre arrondi (4 décimales) et unité.
    pub fn libelle(&self) -> String {
        let v = self.nombre.map(format_real).unwrap_or_else(|| self.valeur.clone());
        match self.unite {
            Some(u) => format!("{v} {u}"),
            None => v,
        }
    }
}

/// Jeu de propriétés ou de quantités d'un objet (`du_type` : porté par son type).
#[derive(Debug, Clone, PartialEq)]
pub struct Jeu {
    pub nom: String,
    pub quantites: bool,
    pub du_type: bool,
    pub proprietes: Vec<Propriete>,
}

/// Grandeur d'une mesure : (unité affichée, exposant de la longueur ; 0 : sans échelle).
fn grandeur(mesure: &str) -> Option<(&'static str, i32)> {
    Some(match mesure {
        "IFCLENGTHMEASURE" | "IFCPOSITIVELENGTHMEASURE" | "IFCNONNEGATIVELENGTHMEASURE" | "IFCQUANTITYLENGTH" => ("m", 1),
        "IFCAREAMEASURE" | "IFCQUANTITYAREA" => ("m²", 2),
        "IFCVOLUMEMEASURE" | "IFCQUANTITYVOLUME" => ("m³", 3),
        "IFCMASSMEASURE" | "IFCQUANTITYWEIGHT" => ("kg", 0),
        "IFCTIMEMEASURE" | "IFCQUANTITYTIME" => ("s", 0),
        _ => return None,
    })
}

/// Texte d'un nombre lu, sans le bruit des conversions (11 chiffres significatifs).
fn texte_nombre(v: f64) -> String {
    let arrondi: f64 = format!("{v:.10e}").parse().unwrap_or(v);
    format!("{arrondi}")
}

impl IfcModel {
    /// Facteur d'une grandeur (exposant 1, 2, 3) : unité propre `unite` si elle est
    /// donnée, sinon celle du projet.
    fn echelle(&self, exposant: i32, unite: Option<Id>) -> f64 {
        if let Some(s) = unite.and_then(|u| self.unit_scale(u, exposant, 0)) {
            return s;
        }
        match exposant {
            1 => self.length_scale,
            2 => self.area_scale,
            3 => self.volume_scale,
            _ => 1.0,
        }
    }

    /// Valeur simple d'une propriété (mesure typée convertie en unité SI de base).
    fn valeur(&self, v: &Value) -> (String, Option<&'static str>, Option<f64>) {
        let numerique = matches!(v, Value::Real(_) | Value::Int(_)) || matches!(v, Value::Typed(_, x) if matches!(**x, Value::Real(_) | Value::Int(_)));
        let nombre = v.as_f64().filter(|_| numerique);
        match (v, nombre) {
            (Value::Typed(t, _), Some(x)) => match grandeur(t) {
                Some((u, e)) => {
                    let x = x * self.echelle(e, None);
                    (texte_nombre(x), Some(u), Some(x))
                }
                None => (texte_nombre(x), None, Some(x)),
            },
            (_, Some(x)) => (texte_nombre(x), None, Some(x)),
            _ => (v.display(), None, None),
        }
    }

    /// Jeux de propriétés et de quantités de l'objet, puis ceux de son type.
    pub fn jeux(&self, id: Id) -> Vec<Jeu> {
        let mut out: Vec<Jeu> = self.property_defs.get(&id).into_iter().flatten().filter_map(|&d| self.jeu(d, false)).collect();
        if let Some(&t) = self.type_of.get(&id) {
            // IfcTypeObject.HasPropertySets (index 5), puis relations vers le type
            let du_type = self.get(t).map(|te| te.arg(5).refs().collect::<Vec<_>>()).unwrap_or_default();
            let rel = self.property_defs.get(&t).cloned().unwrap_or_default();
            out.extend(du_type.into_iter().chain(rel).filter_map(|d| self.jeu(d, true)));
        }
        out
    }

    /// Jeux de propriétés et de quantités pour l'affichage, y compris ceux du type
    /// (suffixe « (type) »).
    pub fn property_groups(&self, id: Id) -> Vec<PropertyGroup> {
        self.jeux(id)
            .into_iter()
            .map(|j| PropertyGroup { name: if j.du_type { format!("{} (type)", j.nom) } else { j.nom }, props: j.proprietes.iter().map(|p| (p.nom.clone(), p.libelle())).collect() })
            .collect()
    }

    fn jeu(&self, id: Id, du_type: bool) -> Option<Jeu> {
        let e = self.get(id)?;
        let nom = e.arg(2).as_str().unwrap_or("?").to_string();
        let mut proprietes = Vec::new();
        let quantites = match &*e.ty {
            "IFCPROPERTYSET" => {
                for p in e.arg(4).refs() {
                    self.propriete(p, "", &mut proprietes, 0);
                }
                false
            }
            "IFCELEMENTQUANTITY" => {
                for q in e.arg(5).refs() {
                    self.quantite(q, &mut proprietes, 0);
                }
                true
            }
            _ => return None,
        };
        Some(Jeu { nom, quantites, du_type, proprietes })
    }

    fn propriete(&self, id: Id, prefixe: &str, out: &mut Vec<Propriete>, profondeur: u32) {
        let Some(e) = self.get(id) else { return };
        let nom = format!("{prefixe}{}", e.arg(0).as_str().unwrap_or("?"));
        let (valeur, unite, nombre) = match &*e.ty {
            "IFCPROPERTYSINGLEVALUE" => self.valeur(e.arg(2)),
            "IFCPROPERTYENUMERATEDVALUE" | "IFCPROPERTYLISTVALUE" | "IFCPROPERTYREFERENCEVALUE" => (e.arg(2).display(), None, None),
            "IFCPROPERTYBOUNDEDVALUE" => (format!("{} … {}", e.arg(3).display(), e.arg(2).display()), None, None),
            "IFCPROPERTYTABLEVALUE" => (format!("{} → {}", e.arg(2).display(), e.arg(3).display()), None, None),
            "IFCCOMPLEXPROPERTY" => {
                if profondeur < 4 {
                    for p in e.arg(3).refs() {
                        self.propriete(p, &format!("{nom}."), out, profondeur + 1);
                    }
                }
                return;
            }
            _ => (String::new(), None, None),
        };
        out.push(Propriete { nom, valeur, unite, nombre });
    }

    fn quantite(&self, id: Id, out: &mut Vec<Propriete>, profondeur: u32) {
        let Some(e) = self.get(id) else { return };
        if e.is("IFCPHYSICALCOMPLEXQUANTITY") {
            if profondeur < 4 {
                for q in e.arg(2).refs() {
                    self.quantite(q, out, profondeur + 1);
                }
            }
            return;
        }
        out.push(self.mesure(e));
    }

    /// `IfcQuantityLength`, `Area`, `Volume`, `Weight`, `Count`, `Time` : valeur (index 3)
    /// dans l'unité propre de la quantité (index 2) ou celle du projet.
    fn mesure(&self, e: &Entity) -> Propriete {
        let nom = e.arg(0).as_str().unwrap_or("?").to_string();
        match (grandeur(&e.ty), e.arg(3).as_f64()) {
            (Some((u, exposant)), Some(v)) => {
                let x = v * self.echelle(exposant, e.arg(2).as_ref());
                Propriete { nom, valeur: texte_nombre(x), unite: Some(u), nombre: Some(x) }
            }
            (None, Some(v)) => Propriete { nom, valeur: texte_nombre(v), unite: None, nombre: Some(v) },
            _ => Propriete { nom, valeur: e.arg(3).display(), unite: None, nombre: None },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::semantique::IfcModel;
    use crate::step::StepFile;

    fn modele(unites: &str, quantites: &str) -> IfcModel {
        let texte = format!(
            "ISO-10303-21;HEADER;ENDSEC;DATA;\n#1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'P',$,$,$,$,$,#10);\n#10=IFCUNITASSIGNMENT(({unites}));\n#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n#12=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);\n#13=IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.);\n#14=IFCSIUNIT(*,.AREAUNIT.,.MILLI.,.SQUARE_METRE.);\n#20=IFCWALL('1YvctVUKr0kugbFTf53O9L',$,'Mur',$,$,$,$,$,$);\n#21=IFCRELDEFINESBYPROPERTIES('2YvctVUKr0kugbFTf53O9L',$,$,$,(#20),#22);\n#22=IFCELEMENTQUANTITY('3YvctVUKr0kugbFTf53O9L',$,'Qto_WallBaseQuantities',$,$,({quantites}));\n#30=IFCQUANTITYLENGTH('Length',$,$,5000.,$);\n#31=IFCQUANTITYAREA('GrossSideArea',$,$,15.,$);\n#32=IFCQUANTITYVOLUME('NetVolume',$,$,2.76,$);\n#33=IFCQUANTITYAREA('Propre',$,#14,2000000.,$);\n#34=IFCQUANTITYCOUNT('Nombre',$,$,3.,$);\n#23=IFCRELDEFINESBYPROPERTIES('4YvctVUKr0kugbFTf53O9L',$,$,$,(#20),#24);\n#24=IFCPROPERTYSET('5YvctVUKr0kugbFTf53O9L',$,'Pset_WallCommon',$,(#25,#26,#27));\n#25=IFCPROPERTYSINGLEVALUE('Reference',$,IFCIDENTIFIER('M-1'),$);\n#26=IFCPROPERTYSINGLEVALUE('Width',$,IFCPOSITIVELENGTHMEASURE(200.),$);\n#27=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$);\nENDSEC;END-ISO-10303-21;\n"
        );
        IfcModel::new(StepFile::parse(texte.as_bytes()).unwrap())
    }

    /// Longueurs en mm, aires et volumes en m² et m³ explicites (export natif) : les
    /// quantités sont lues en m, m², m³, sans être divisées par 10⁶ et 10⁹ (A6b).
    #[test]
    fn quantites_selon_les_unites_du_projet() {
        let m = modele("#11,#12,#13", "#30,#31,#32,#33,#34");
        assert_eq!((m.length_scale, m.area_scale, m.volume_scale), (1e-3, 1.0, 1.0));
        let jeux = m.jeux(20);
        let q = jeux.iter().find(|j| j.quantites).unwrap();
        let v: Vec<_> = q.proprietes.iter().map(|p| (p.nom.as_str(), p.valeur.as_str(), p.unite)).collect();
        assert_eq!(v, vec![("Length", "5", Some("m")), ("GrossSideArea", "15", Some("m²")), ("NetVolume", "2.76", Some("m³")), ("Propre", "2", Some("m²")), ("Nombre", "3", None)]);
        let g = m.property_groups(20);
        assert_eq!(g[0].props[1], ("GrossSideArea".to_string(), "15 m²".to_string()));
        // propriétés : mesure convertie, identifiant et booléen en texte
        let p = jeux.iter().find(|j| !j.quantites).unwrap();
        let v: Vec<_> = p.proprietes.iter().map(|p| (p.nom.as_str(), p.valeur.as_str(), p.unite)).collect();
        assert_eq!(v, vec![("Reference", "M-1", None), ("Width", "0.2", Some("m")), ("IsExternal", "Vrai", None)]);
    }

    /// Sans `AREAUNIT` ni `VOLUMEUNIT` : unité de longueur au carré, au cube.
    #[test]
    fn quantites_sans_unite_d_aire() {
        let m = modele("#11", "#31,#32");
        assert!((m.area_scale - 1e-6).abs() < 1e-18 && (m.volume_scale - 1e-9).abs() < 1e-21);
        let q = &m.jeux(20)[0];
        assert_eq!(q.proprietes[0].valeur, "0.000015");
    }
}
