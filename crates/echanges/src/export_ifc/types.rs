//! Types IFC (`IfcWallType`, `IfcSlabType`, `IfcDoorType`…) des types du projet
//! effectivement utilisés, reliés à leurs occurrences par `IfcRelDefinesByType`.
use super::step::{guid, refs, texte, Step};
use std::collections::BTreeMap;

/// Un type IFC : nom, type prédéfini, jeu de couches éventuel, occurrences.
struct TypeIfc {
    nom: String,
    predefini: &'static str,
    jeu: Option<usize>,
    objets: Vec<usize>,
}

/// Types rencontrés pendant l'export, par (classe IFC du type, identifiant natif) : un
/// même type natif de dalle peut donner un `IfcSlabType` et un `IfcCoveringType`.
#[derive(Default)]
pub(super) struct Types {
    liens: BTreeMap<(&'static str, String), TypeIfc>,
}

impl Types {
    /// Rattache l'occurrence `objet` au type natif `id` (nom `nom`) sous la classe `classe`
    /// (`IFCWALLTYPE`…). `jeu` : `IfcMaterialLayerSet` du type, associé au type.
    pub(super) fn lier(&mut self, classe: &'static str, id: &str, nom: &str, predefini: &'static str, jeu: Option<usize>, objet: usize) {
        self.liens
            .entry((classe, id.to_string()))
            .or_insert_with(|| TypeIfc { nom: nom.to_string(), predefini, jeu, objets: Vec::new() })
            .objets
            .push(objet);
    }

    /// Écrit les types, leurs relations et le matériau des types à couches.
    pub(super) fn ecrire(self, w: &mut Step) {
        for ((classe, id), t) in self.liens {
            // portes et fenêtres : sens d'ouverture et partition non connus
            let suite = match classe {
                "IFCDOORTYPE" | "IFCWINDOWTYPE" => ",.NOTDEFINED.,$,$",
                _ => "",
            };
            let ty = w.add(format!(
                "{classe}('{}',$,{},$,$,$,$,$,$,.{}.{suite})",
                guid(&format!("type:{classe}:{id}"), None),
                texte(&t.nom),
                t.predefini
            ));
            w.add(format!(
                "IFCRELDEFINESBYTYPE('{}',$,$,$,{},#{ty})",
                guid(&format!("type-rel:{classe}:{id}"), None),
                refs(&t.objets)
            ));
            if let Some(jeu) = t.jeu {
                w.add(format!(
                    "IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{ty}),#{jeu})",
                    guid(&format!("type-mat:{classe}:{id}"), None)
                ));
            }
        }
    }
}
