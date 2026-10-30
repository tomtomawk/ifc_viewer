//! Famille paramétrique de fenêtre (prototype d'éditeur de familles) : dormant, vantaux,
//! imposte, soubassement, vitrage et appui, définis par quelques cotes. La géométrie est
//! une liste de boîtes dans le repère de la baie : x le long du mur (milieu de la baie
//! en 0), y à travers le mur (extérieur positif, axe du mur en 0), z depuis le bas de la
//! baie. Remplacée par la famille modélisée (`famille_modelee`) ; conservée pour les
//! types qui la portent encore.
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Fonctionnement d'un vantail, vu de l'intérieur.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Vantail {
    Fixe,
    /// Paumelles à gauche.
    BattantGauche,
    /// Paumelles à droite.
    BattantDroite,
    OscilloBattantGauche,
    OscilloBattantDroite,
    Coulissant,
}

impl Vantail {
    pub fn ouvrant(self) -> bool {
        self != Self::Fixe
    }
}

/// Nu de référence de la menuiserie dans l'épaisseur du mur.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Nu {
    Exterieur,
    Milieu,
    Interieur,
}

/// Paramètres d'une fenêtre (mm, couleur RVB).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct FamilleFenetre {
    /// Largeur vue du dormant.
    pub dormant: f64,
    /// Épaisseur (profondeur) du dormant.
    pub profondeur: f64,
    pub nu: Nu,
    /// Retrait depuis le nu choisi, vers l'autre face (au milieu : vers l'intérieur).
    pub retrait: f64,
    /// Largeur vue des ouvrants (et des traverses de soubassement).
    pub ouvrant: f64,
    /// De gauche à droite, vus de l'intérieur ; largeurs égales.
    pub vantaux: Vec<Vantail>,
    /// Hauteur vitrée de l'imposte fixe ; 0 : sans imposte.
    pub imposte: f64,
    /// Hauteur du panneau plein en bas des vantaux ; 0 : sans.
    pub soubassement: f64,
    pub vitrage: f64,
    /// Débord extérieur de la pièce d'appui ; 0 : sans.
    pub appui: f64,
    pub couleur: [u8; 3],
}

impl Default for FamilleFenetre {
    fn default() -> Self {
        Self {
            dormant: 60.0,
            profondeur: 70.0,
            nu: Nu::Exterieur,
            retrait: 0.0,
            ouvrant: 55.0,
            vantaux: vec![Vantail::BattantGauche, Vantail::BattantDroite],
            imposte: 0.0,
            soubassement: 0.0,
            vitrage: 28.0,
            appui: 40.0,
            couleur: [235, 235, 230],
        }
    }
}

/// Rôle d'une pièce : couleur et transparence au rendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Dormant,
    Ouvrant,
    Panneau,
    Vitrage,
    Appui,
}

/// Pièce de la fenêtre : boîte alignée sur le repère de la baie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Boite {
    pub min: DVec3,
    pub max: DVec3,
    pub role: Role,
}

/// Case d'un vantail : bornes en x et z (repère de la baie) et fonctionnement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Case {
    pub x: [f64; 2],
    pub z: [f64; 2],
    pub vantail: Vantail,
}

impl FamilleFenetre {
    /// Contrôle des cotes avant enregistrement.
    pub fn valider(&self, largeur: f64, hauteur: f64) -> Result<(), String> {
        let cotes = [self.dormant, self.profondeur, self.retrait, self.ouvrant, self.imposte, self.soubassement, self.vitrage, self.appui];
        if !cotes.iter().all(|c| c.is_finite() && *c >= 0.0) {
            return Err("Les cotes de la famille doivent être positives ou nulles.".into());
        }
        if self.dormant <= 0.0 || self.profondeur <= 0.0 || self.ouvrant <= 0.0 || self.vitrage <= 0.0 {
            return Err("Dormant, ouvrant et vitrage doivent avoir une épaisseur.".into());
        }
        if self.vantaux.is_empty() || self.vantaux.len() > 6 {
            return Err("Une fenêtre compte de 1 à 6 vantaux.".into());
        }
        let n = self.vantaux.len() as f64;
        let utile = largeur - 2.0 * self.dormant;
        if utile / n < 2.0 * self.ouvrant + 50.0 {
            return Err("Vantaux trop étroits pour la largeur de la baie.".into());
        }
        let bas = hauteur - 2.0 * self.dormant - if self.imposte > 0.0 { self.imposte + self.dormant } else { 0.0 };
        if bas < 2.0 * self.ouvrant + self.soubassement + 50.0 {
            return Err("Imposte ou soubassement trop haut pour la hauteur de la baie.".into());
        }
        Ok(())
    }

    /// Centre de la menuiserie dans l'épaisseur `e` du mur (y, extérieur positif).
    pub fn centre(&self, e: f64) -> f64 {
        let p = self.profondeur.min(e);
        match self.nu {
            Nu::Exterieur => e / 2.0 - self.retrait - p / 2.0,
            Nu::Milieu => -self.retrait,
            Nu::Interieur => -e / 2.0 + self.retrait + p / 2.0,
        }
    }

    /// Cases des vantaux et de l'imposte, dans le clair du dormant.
    pub fn cases(&self, largeur: f64, hauteur: f64) -> Vec<Case> {
        let d = self.dormant;
        let (x0, x1) = (-largeur / 2.0 + d, largeur / 2.0 - d);
        let haut = hauteur - d;
        let bas_haut = if self.imposte > 0.0 { haut - self.imposte - d } else { haut };
        let n = self.vantaux.len().max(1);
        let pas = (x1 - x0) / n as f64;
        let mut cases = Vec::new();
        for (k, &v) in self.vantaux.iter().enumerate() {
            // montant (meneau) entre deux cases dont l'une est fixe
            let fixe_a = |j: usize| self.vantaux.get(j).is_some_and(|v| !v.ouvrant());
            let gauche = if k > 0 && (fixe_a(k) || fixe_a(k - 1)) { d / 2.0 } else { 0.0 };
            let droite = if k + 1 < n && (fixe_a(k) || fixe_a(k + 1)) { d / 2.0 } else { 0.0 };
            cases.push(Case { x: [x0 + pas * k as f64 + gauche, x0 + pas * (k + 1) as f64 - droite], z: [d, bas_haut], vantail: v });
        }
        if self.imposte > 0.0 {
            cases.push(Case { x: [x0, x1], z: [haut - self.imposte, haut], vantail: Vantail::Fixe });
        }
        cases
    }

    /// Pièces de la fenêtre pour une baie `largeur` × `hauteur` dans un mur d'épaisseur
    /// `e`.
    pub fn pieces(&self, largeur: f64, hauteur: f64, e: f64) -> Vec<Boite> {
        let (l, h, d) = (largeur, hauteur, self.dormant);
        let p = self.profondeur.min(e);
        let yc = self.centre(e);
        let boite = |x: [f64; 2], y: [f64; 2], z: [f64; 2], role| Boite { min: DVec3::new(x[0], y[0], z[0]), max: DVec3::new(x[1], y[1], z[1]), role };
        let yd = [yc - p / 2.0, yc + p / 2.0];
        let mut v = vec![
            boite([-l / 2.0, -l / 2.0 + d], yd, [0.0, h], Role::Dormant),
            boite([l / 2.0 - d, l / 2.0], yd, [0.0, h], Role::Dormant),
            boite([-l / 2.0 + d, l / 2.0 - d], yd, [0.0, d], Role::Dormant),
            boite([-l / 2.0 + d, l / 2.0 - d], yd, [h - d, h], Role::Dormant),
        ];
        let cases = self.cases(l, h);
        // traverse sous l'imposte, montants entre cases séparées
        if self.imposte > 0.0 {
            v.push(boite([-l / 2.0 + d, l / 2.0 - d], yd, [h - d - self.imposte - d, h - d - self.imposte], Role::Dormant));
        }
        for w in cases.windows(2).filter(|w| w[0].z == w[1].z && w[1].x[0] - w[0].x[1] > 1e-6) {
            v.push(boite([w[0].x[1], w[1].x[0]], yd, w[0].z, Role::Dormant));
        }
        // ouvrants : cadre légèrement en retrait vers l'intérieur, coulissants décalés
        let o = self.ouvrant;
        let vitrage = self.vitrage;
        for (k, c) in cases.iter().enumerate() {
            let decal = if c.vantail == Vantail::Coulissant { if k % 2 == 0 { p * 0.2 } else { -p * 0.2 } } else { -p * 0.1 };
            let yo = [yc - p * 0.4 + decal, yc + p * 0.4 + decal];
            let yv = [yc + decal - vitrage / 2.0, yc + decal + vitrage / 2.0];
            let [cx0, cx1] = c.x;
            let [cz0, cz1] = c.z;
            let (mut gx, mut gz) = ([cx0, cx1], [cz0, cz1]);
            if c.vantail.ouvrant() {
                v.push(boite([cx0, cx0 + o], yo, [cz0, cz1], Role::Ouvrant));
                v.push(boite([cx1 - o, cx1], yo, [cz0, cz1], Role::Ouvrant));
                v.push(boite([cx0 + o, cx1 - o], yo, [cz0, cz0 + o], Role::Ouvrant));
                v.push(boite([cx0 + o, cx1 - o], yo, [cz1 - o, cz1], Role::Ouvrant));
                gx = [cx0 + o, cx1 - o];
                gz = [cz0 + o, cz1 - o];
            }
            // soubassement des vantaux (pas de l'imposte)
            if self.soubassement > 0.0 && k < self.vantaux.len() {
                let haut = gz[0] + self.soubassement;
                v.push(boite(gx, yv, [gz[0], haut], Role::Panneau));
                v.push(boite(gx, yo, [haut, haut + o], Role::Ouvrant));
                gz[0] = haut + o;
            }
            if gz[1] > gz[0] && gx[1] > gx[0] {
                v.push(boite(gx, yv, gz, Role::Vitrage));
            }
        }
        if self.appui > 0.0 {
            v.push(boite([-l / 2.0 - 20.0, l / 2.0 + 20.0], [yd[1] - 10.0, e / 2.0 + self.appui], [-40.0, 0.0], Role::Appui));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_et_cases() {
        let f = FamilleFenetre { imposte: 400.0, soubassement: 300.0, ..Default::default() };
        f.valider(1200.0, 2150.0).unwrap();
        let cases = f.cases(1200.0, 2150.0);
        // deux vantaux qui se rejoignent (pas de meneau) et l'imposte
        assert_eq!(cases.len(), 3);
        assert!((cases[0].x[1] - cases[1].x[0]).abs() < 1e-9);
        let pieces = f.pieces(1200.0, 2150.0, 300.0);
        let vitrages = pieces.iter().filter(|b| b.role == Role::Vitrage).count();
        assert_eq!(vitrages, 3);
        assert!(pieces.iter().any(|b| b.role == Role::Appui && b.max.y > 150.0));
        // au nu extérieur, le dormant affleure la face extérieure
        let dormant = pieces.iter().find(|b| b.role == Role::Dormant).unwrap();
        assert!((dormant.max.y - 150.0).abs() < 1e-9);
        // un fixe entre deux battants crée des meneaux
        let g = FamilleFenetre { vantaux: vec![Vantail::BattantGauche, Vantail::Fixe, Vantail::BattantDroite], ..Default::default() };
        let c = g.cases(1800.0, 1350.0);
        assert!(c[1].x[0] - c[0].x[1] > 1.0);
        assert!(g.valider(400.0, 1350.0).is_err());
    }
}
