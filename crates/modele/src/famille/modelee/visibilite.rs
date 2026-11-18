//! Visibilité des éléments d'une famille (volumes, formes à profil, éléments 2D) selon
//! le niveau de détail et le type de la vue qui les montre, comme les réglages de
//! visibilité des géométries d'une famille Revit. La visibilité paramétrique (« Visible
//! si », paramètre oui/non) s'y ajoute : un élément est montré si les trois l'admettent.
use super::VueEsquisse;
use serde::{Deserialize, Serialize};

/// Niveau de détail d'une vue, comme dans Revit.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum NiveauDetail {
    Faible,
    #[default]
    Moyen,
    Eleve,
}

impl NiveauDetail {
    pub const TOUS: [NiveauDetail; 3] = [NiveauDetail::Faible, NiveauDetail::Moyen, NiveauDetail::Eleve];

    pub fn libelle(self) -> &'static str {
        match self {
            NiveauDetail::Faible => "Faible",
            NiveauDetail::Moyen => "Moyen",
            NiveauDetail::Eleve => "Élevé",
        }
    }

    /// Clé du pilote et du JSON (`faible`, `moyen`, `eleve`).
    pub fn cle(self) -> &'static str {
        match self {
            NiveauDetail::Faible => "faible",
            NiveauDetail::Moyen => "moyen",
            NiveauDetail::Eleve => "eleve",
        }
    }

    pub fn depuis_cle(cle: &str) -> Option<Self> {
        Self::TOUS.into_iter().find(|d| d.cle() == cle)
    }

    /// Valeur du paramètre VIEW_DETAIL_LEVEL (`ViewDetailLevel` de l'API : 1, 2, 3).
    pub fn depuis_revit(v: i64) -> Option<Self> {
        match v {
            1 => Some(NiveauDetail::Faible),
            2 => Some(NiveauDetail::Moyen),
            3 => Some(NiveauDetail::Eleve),
            _ => None,
        }
    }

    pub fn revit(self) -> i64 {
        self as i64 + 1
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Type de la vue qui montre un élément : plan, coupe ou élévation, 3D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeVue {
    Plan,
    Coupe,
    Vue3d,
}

impl TypeVue {
    pub const TOUS: [TypeVue; 3] = [TypeVue::Plan, TypeVue::Coupe, TypeVue::Vue3d];

    pub fn libelle(self) -> &'static str {
        match self {
            TypeVue::Plan => "Plan",
            TypeVue::Coupe => "Coupe / élévation",
            TypeVue::Vue3d => "3D",
        }
    }

    /// Type de la vue d'esquisse d'un élément 2D ou d'une vue plane de l'éditeur.
    pub fn depuis_esquisse(vue: VueEsquisse) -> Self {
        match vue {
            VueEsquisse::Plan => TypeVue::Plan,
            VueEsquisse::Elevation | VueEsquisse::Coupe => TypeVue::Coupe,
        }
    }

    fn bit(self) -> u8 {
        8 << self as u8
    }
}

/// Réglage d'affichage d'une vue : son niveau de détail et son type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Affichage {
    pub detail: NiveauDetail,
    pub vue: TypeVue,
}

impl Affichage {
    /// Un élément de masque `masque` (voir [`VisibiliteElement::masque`]) est montré.
    pub fn montre(self, masque: u8) -> bool {
        masque & self.detail.bit() != 0 && masque & self.vue.bit() != 0
    }
}

fn oui() -> bool {
    true
}

/// Visibilité d'un élément par niveau de détail et par type de vue ; tout coché par
/// défaut (champ absent du JSON : visible partout).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VisibiliteElement {
    #[serde(default = "oui")]
    pub faible: bool,
    #[serde(default = "oui")]
    pub moyen: bool,
    #[serde(default = "oui")]
    pub eleve: bool,
    #[serde(default = "oui")]
    pub plan: bool,
    #[serde(default = "oui")]
    pub coupe: bool,
    #[serde(default = "oui")]
    pub vue_3d: bool,
}

impl Default for VisibiliteElement {
    fn default() -> Self {
        Self { faible: true, moyen: true, eleve: true, plan: true, coupe: true, vue_3d: true }
    }
}

impl VisibiliteElement {
    /// Visible partout (valeur omise à l'écriture).
    pub fn partout(&self) -> bool {
        *self == Self::default()
    }

    /// Masque d'un octet : bits 0 à 2 faible, moyen, élevé ; bits 3 à 5 plan, coupe, 3D ;
    /// [`PARTOUT`] pour un élément visible partout.
    pub fn masque(&self) -> u8 {
        if self.partout() { return PARTOUT; }
        [self.faible, self.moyen, self.eleve, self.plan, self.coupe, self.vue_3d].iter().enumerate().fold(0, |m, (k, b)| if *b { m | 1 << k } else { m })
    }

    pub fn montre(&self, a: Affichage) -> bool {
        a.montre(self.masque())
    }

    pub fn detail_mut(&mut self, d: NiveauDetail) -> &mut bool {
        match d {
            NiveauDetail::Faible => &mut self.faible,
            NiveauDetail::Moyen => &mut self.moyen,
            NiveauDetail::Eleve => &mut self.eleve,
        }
    }

    pub fn vue_mut(&mut self, v: TypeVue) -> &mut bool {
        match v {
            TypeVue::Plan => &mut self.plan,
            TypeVue::Coupe => &mut self.coupe,
            TypeVue::Vue3d => &mut self.vue_3d,
        }
    }
}

/// Masque d'un élément visible partout (tous les bits).
pub const PARTOUT: u8 = u8::MAX;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaut_serde_tout_visible_et_omis() {
        #[derive(Serialize, Deserialize)]
        struct E {
            #[serde(default, skip_serializing_if = "VisibiliteElement::partout")]
            visibilite: VisibiliteElement,
        }
        let e: E = serde_json::from_str("{}").unwrap();
        assert!(e.visibilite.partout() && e.visibilite.masque() == PARTOUT);
        assert_eq!(serde_json::to_string(&e).unwrap(), "{}");
        let e: E = serde_json::from_str(r#"{"visibilite":{"faible":false,"plan":false}}"#).unwrap();
        assert!(!e.visibilite.faible && e.visibilite.moyen && !e.visibilite.plan && e.visibilite.vue_3d);
        assert!(serde_json::from_str::<E>(r#"{"visibilite":{"inconnu":true}}"#).is_err());
        let relu: E = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(relu.visibilite, e.visibilite);
    }

    #[test]
    fn affichage_par_detail_et_type_de_vue() {
        let eleve_seul = VisibiliteElement { faible: false, moyen: false, ..Default::default() };
        let a = |detail, vue| Affichage { detail, vue };
        assert!(eleve_seul.montre(a(NiveauDetail::Eleve, TypeVue::Vue3d)));
        assert!(!eleve_seul.montre(a(NiveauDetail::Faible, TypeVue::Vue3d)));
        assert!(!eleve_seul.montre(a(NiveauDetail::Moyen, TypeVue::Plan)));
        let sans_plan = VisibiliteElement { plan: false, ..Default::default() };
        assert!(!sans_plan.montre(a(NiveauDetail::Moyen, TypeVue::Plan)));
        assert!(sans_plan.montre(a(NiveauDetail::Moyen, TypeVue::Coupe)));
        assert_eq!(NiveauDetail::depuis_revit(NiveauDetail::Eleve.revit()), Some(NiveauDetail::Eleve));
        assert_eq!(NiveauDetail::depuis_cle("faible"), Some(NiveauDetail::Faible));
    }
}
