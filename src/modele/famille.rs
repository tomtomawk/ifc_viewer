//! Famille paramétrique de fenêtre (prototype d'éditeur de familles) : dormant, vantaux,
//! imposte, soubassement, vitrage et appui, définis par quelques cotes. La géométrie est
//! une liste de boîtes dans le repère de la baie : x le long du mur (milieu de la baie
//! en 0), y à travers le mur (extérieur positif, axe du mur en 0), z depuis le bas de la
//! baie. Les symboles d'élévation suivent l'usage français : la pointe du triangle
//! indique le côté des paumelles.
use glam::{DVec2, DVec3};
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
    pub const TOUS: [Vantail; 6] = [Self::Fixe, Self::BattantGauche, Self::BattantDroite, Self::OscilloBattantGauche, Self::OscilloBattantDroite, Self::Coulissant];

    pub fn nom(self) -> &'static str {
        match self {
            Self::Fixe => "Fixe",
            Self::BattantGauche => "Battant, paumelles à gauche",
            Self::BattantDroite => "Battant, paumelles à droite",
            Self::OscilloBattantGauche => "Oscillo-battant, paumelles à gauche",
            Self::OscilloBattantDroite => "Oscillo-battant, paumelles à droite",
            Self::Coulissant => "Coulissant",
        }
    }

    pub fn ouvrant(self) -> bool {
        self != Self::Fixe
    }

    /// Côté des paumelles : -1 gauche, 1 droite, 0 sans.
    fn paumelles(self) -> f64 {
        match self {
            Self::BattantGauche | Self::OscilloBattantGauche => -1.0,
            Self::BattantDroite | Self::OscilloBattantDroite => 1.0,
            _ => 0.0,
        }
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

impl Nu {
    pub const TOUS: [Nu; 3] = [Self::Exterieur, Self::Milieu, Self::Interieur];

    pub fn nom(self) -> &'static str {
        match self {
            Self::Exterieur => "Nu extérieur",
            Self::Milieu => "Milieu du mur",
            Self::Interieur => "Nu intérieur",
        }
    }
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

    /// Symboles d'ouverture en élévation intérieure (x, z) : triangle pointé vers les
    /// paumelles, second triangle pointé vers le bas pour l'oscillo-battant, flèche
    /// pour le coulissant.
    pub fn symboles(&self, largeur: f64, hauteur: f64) -> Vec<[DVec2; 2]> {
        let mut s = Vec::new();
        for c in self.cases(largeur, hauteur).iter().take(self.vantaux.len()) {
            let ([x0, x1], [z0, z1]) = (c.x, c.z);
            let zm = (z0 + z1) / 2.0;
            let p = c.vantail.paumelles();
            if p != 0.0 {
                let (charniere, oppose) = if p < 0.0 { (x0, x1) } else { (x1, x0) };
                s.push([DVec2::new(charniere, zm), DVec2::new(oppose, z0)]);
                s.push([DVec2::new(charniere, zm), DVec2::new(oppose, z1)]);
            }
            if matches!(c.vantail, Vantail::OscilloBattantGauche | Vantail::OscilloBattantDroite) {
                let xm = (x0 + x1) / 2.0;
                s.push([DVec2::new(xm, z0), DVec2::new(x0, z1)]);
                s.push([DVec2::new(xm, z0), DVec2::new(x1, z1)]);
            }
            if c.vantail == Vantail::Coulissant {
                let (a, b) = (DVec2::new(x0 + (x1 - x0) * 0.2, zm), DVec2::new(x1 - (x1 - x0) * 0.2, zm));
                let t = (x1 - x0) * 0.08;
                s.push([a, b]);
                s.push([b, b + DVec2::new(-t, t)]);
                s.push([b, b + DVec2::new(-t, -t)]);
            }
        }
        s
    }

    /// Débattements en plan (repère de la baie, y extérieur positif) : vantail ouvert à
    /// 90° vers l'intérieur et arc de rotation, par vantail battant.
    pub fn debattements(&self, largeur: f64, hauteur: f64, e: f64) -> Vec<Vec<DVec2>> {
        let y = self.centre(e) - self.profondeur.min(e) * 0.4;
        let mut v = Vec::new();
        for c in self.cases(largeur, hauteur).iter().take(self.vantaux.len()) {
            let p = c.vantail.paumelles();
            if p == 0.0 {
                continue;
            }
            let (charniere, oppose) = if p < 0.0 { (c.x[0], c.x[1]) } else { (c.x[1], c.x[0]) };
            let r = (oppose - charniere).abs();
            let pivot = DVec2::new(charniere, y);
            v.push(vec![pivot, pivot - DVec2::Y * r]);
            v.push((0..=16).map(|k| {
                let a = std::f64::consts::FRAC_PI_2 * k as f64 / 16.0;
                pivot + DVec2::new((oppose - charniere).signum() * r * a.cos(), -r * a.sin())
            }).collect());
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_cases_et_symboles() {
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
        // chaque battant a son triangle, pointé vers ses paumelles
        let s = f.symboles(1200.0, 2150.0);
        assert_eq!(s.len(), 4);
        assert!((s[0][0].x - cases[0].x[0]).abs() < 1e-9);
        assert_eq!(f.debattements(1200.0, 2150.0, 300.0).len(), 4);
        // un fixe entre deux battants crée des meneaux
        let g = FamilleFenetre { vantaux: vec![Vantail::BattantGauche, Vantail::Fixe, Vantail::BattantDroite], ..Default::default() };
        let c = g.cases(1800.0, 1350.0);
        assert!(c[1].x[0] - c[0].x[1] > 1.0);
        assert!(g.valider(400.0, 1350.0).is_err());
    }
}
