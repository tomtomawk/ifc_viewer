//! Escaliers et garde-corps natifs.
//!
//! Un escalier suit sa ligne de foulée en plan (polyligne, du départ à l'arrivée) :
//! volées droites, palier carré (de la largeur de l'escalier) à chaque sommet
//! intermédiaire, d'où les formes droite, quart tournant ou demi-tournant. La hauteur à
//! franchir va de la base (niveau et décalage) au niveau supérieur, ou vaut la hauteur
//! libre. Le nombre de contremarches découle de la hauteur de marche visée ; un palier
//! compte comme une marche ; le giron partage la longueur des volées entre les marches
//! restantes (la dernière contremarche arrive au plancher haut). Paillasse crantée en
//! béton sous les marches.
//!
//! Un garde-corps suit un tracé en plan : main courante, lisse basse, montants (aux
//! sommets et tous les 1,5 m au plus) et remplissage (barreaudage, panneau plein ou
//! vitré). Hébergé par un escalier, sa base suit la hauteur des marches.
use super::*;
use glam::{DVec2, DVec3};

pub type Tri = [DVec3; 3];

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Escalier {
    pub id: String,
    pub nom: String,
    pub niveau: String,
    /// Niveau d'arrivée ; None : hauteur libre `hauteur`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub niveau_haut: Option<String>,
    #[serde(default)]
    pub decalage_bas: f64,
    /// Hauteur à franchir sans niveau d'arrivée (mm).
    pub hauteur: f64,
    /// Ligne de foulée en plan (mm), du départ à l'arrivée.
    pub trace: Vec<[f64; 2]>,
    /// Emmarchement (mm).
    pub largeur: f64,
    /// Hauteur de marche visée (mm).
    pub hauteur_marche: f64,
    /// Épaisseur de la paillasse (mm).
    pub paillasse: f64,
}

/// Remplissage d'un garde-corps.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Remplissage {
    Barreaudage,
    Plein,
    Vitre,
}

impl Remplissage {
    pub const TOUS: [Remplissage; 3] = [Self::Barreaudage, Self::Plein, Self::Vitre];
    pub fn nom(self) -> &'static str {
        match self {
            Self::Barreaudage => "Barreaudage",
            Self::Plein => "Panneau plein",
            Self::Vitre => "Vitrage",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GardeCorps {
    pub id: String,
    pub nom: String,
    pub niveau: String,
    /// Base au-dessus du niveau (mm), sans escalier hôte.
    #[serde(default)]
    pub decalage: f64,
    pub trace: Vec<[f64; 2]>,
    pub hauteur: f64,
    pub remplissage: Remplissage,
    /// Escalier hôte : la base suit ses marches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalier: Option<String>,
}

/// Caractéristiques calculées d'un escalier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalculEscalier {
    /// Altitude du départ (mm).
    pub base: f64,
    pub hauteur: f64,
    pub contremarches: usize,
    /// Hauteur de marche réelle (mm).
    pub h: f64,
    pub giron: f64,
    /// Formule de Blondel : 2h + g (600 à 640 mm confortable).
    pub blondel: f64,
    pub paliers: usize,
    /// Longueur des volées (sans paliers).
    pub volees: f64,
}

impl CalculEscalier {
    pub fn confortable(&self) -> bool {
        (600.0..=650.0).contains(&self.blondel) && self.h <= 200.0 && self.giron >= 220.0
    }
}

fn pt(p: [f64; 2]) -> DVec2 {
    DVec2::from_array(p)
}

/// Hexaèdre (bas : 4 sommets, haut : 4 sommets, même ordre) en triangles orientés vers
/// l'extérieur.
pub fn hexaedre(mut c: [DVec3; 8]) -> Vec<Tri> {
    // bas dans le sens trigonométrique vu de dessus
    let aire: f64 = (0..4).map(|i| { let (a, b) = (c[i], c[(i + 1) % 4]); a.x * b.y - b.x * a.y }).sum();
    if aire < 0.0 {
        c.swap(1, 3);
        c.swap(5, 7);
    }
    let mut t = vec![[c[0], c[2], c[1]], [c[0], c[3], c[2]], [c[4], c[5], c[6]], [c[4], c[6], c[7]]];
    for i in 0..4 {
        let j = (i + 1) % 4;
        t.push([c[i], c[j], c[j + 4]]);
        t.push([c[i], c[j + 4], c[i + 4]]);
    }
    t
}

/// Barre de section `largeur` (horizontale, perpendiculaire) × `epaisseur` (verticale)
/// de `a` à `b` (axe au centre de la section) ; verticale si `a` et `b` sont superposés.
pub fn barre_3d(a: DVec3, b: DVec3, largeur: f64, epaisseur: f64) -> Vec<Tri> {
    let d = DVec2::new(b.x - a.x, b.y - a.y);
    if d.length() < 1e-6 {
        // montant vertical : section carrée en plan
        let (u, v) = (DVec2::X * largeur / 2.0, DVec2::Y * epaisseur / 2.0);
        let q = DVec2::new(a.x, a.y);
        let coins = [q - u - v, q + u - v, q + u + v, q - u + v];
        let (z0, z1) = (a.z.min(b.z), a.z.max(b.z));
        return hexaedre([coins[0].extend(z0), coins[1].extend(z0), coins[2].extend(z0), coins[3].extend(z0), coins[0].extend(z1), coins[1].extend(z1), coins[2].extend(z1), coins[3].extend(z1)]);
    }
    let n = d.normalize().perp() * largeur / 2.0;
    let e = epaisseur / 2.0;
    let p = |q: DVec3, s: f64, dz: f64| DVec3::new(q.x + n.x * s, q.y + n.y * s, q.z + dz);
    hexaedre([p(a, -1.0, -e), p(b, -1.0, -e), p(b, 1.0, -e), p(a, 1.0, -e), p(a, -1.0, e), p(b, -1.0, e), p(b, 1.0, e), p(a, 1.0, e)])
}

impl Escalier {
    /// Hauteur à franchir et caractéristiques.
    pub fn calcul(&self, p: &Projet) -> Result<CalculEscalier, String> {
        let n0 = p.niveau(&self.niveau).ok_or("Niveau de l'escalier absent.")?;
        let base = n0.altitude + self.decalage_bas;
        let hauteur = match &self.niveau_haut {
            Some(id) => p.niveau(id).ok_or("Niveau d'arrivée de l'escalier absent.")?.altitude - base,
            None => self.hauteur,
        };
        if !(hauteur > 0.0 && hauteur.is_finite()) {
            return Err("L'escalier doit monter : niveau d'arrivée au-dessus du départ.".into());
        }
        if self.trace.len() < 2 || !self.trace.iter().flatten().all(|x| x.is_finite()) {
            return Err("Ligne de foulée : au moins deux points.".into());
        }
        if !(self.largeur > 0.0 && self.hauteur_marche > 50.0 && self.paillasse > 0.0) {
            return Err("Largeur, hauteur de marche et paillasse doivent être positives.".into());
        }
        let contremarches = ((hauteur / self.hauteur_marche).round() as usize).max(2);
        let h = hauteur / contremarches as f64;
        let paliers = self.trace.len() - 2;
        let volees = self.volees().iter().map(|(a, b)| a.distance(*b)).sum::<f64>();
        let marches = contremarches as i64 - 1 - paliers as i64;
        if marches < 1 {
            return Err("Trop de paliers pour la hauteur à franchir.".into());
        }
        if self.volees().iter().any(|(a, b)| a.distance(*b) < 1.0) {
            return Err("Volée trop courte : allonger la ligne de foulée ou réduire la largeur.".into());
        }
        let giron = volees / marches as f64;
        Ok(CalculEscalier { base, hauteur, contremarches, h, giron, blondel: 2.0 * h + giron, paliers, volees })
    }

    /// Volées en plan : segments de la ligne de foulée, raccourcis de la demi-largeur
    /// du palier à chaque sommet intermédiaire.
    pub fn volees(&self) -> Vec<(DVec2, DVec2)> {
        let n = self.trace.len();
        (0..n.saturating_sub(1))
            .map(|i| {
                let (a, b) = (pt(self.trace[i]), pt(self.trace[i + 1]));
                let d = (b - a).normalize_or_zero();
                let a = if i > 0 { a + d * self.largeur / 2.0 } else { a };
                let b = if i + 2 < n { b - d * self.largeur / 2.0 } else { b };
                (a, b)
            })
            .collect()
    }

    /// Marches et paliers : contour en plan, altitude du dessus ; et le profil de la
    /// ligne de foulée (abscisse curviligne, altitude du nez de marche).
    fn marches(&self, c: &CalculEscalier) -> (Vec<([DVec2; 4], f64)>, Vec<(DVec2, f64)>) {
        let w = self.largeur / 2.0;
        let volees = self.volees();
        let total: f64 = volees.iter().map(|(a, b)| a.distance(*b)).sum();
        let marches = c.contremarches - 1 - c.paliers;
        let mut out = Vec::new();
        let mut profil = vec![(pt(self.trace[0]), c.base)];
        let mut k = 0usize; // marches et paliers posés
        let mut cumul = 0.0;
        let mut faites = 0usize; // marches de volée posées
        for (i, (a, b)) in volees.iter().enumerate() {
            let d = (*b - *a).normalize_or_zero();
            let nrm = d.perp() * w;
            if i > 0 {
                // palier au sommet, dans l'axe de la volée qui arrive
                let s = pt(self.trace[i]);
                let din = (s - pt(self.trace[i - 1])).normalize_or_zero();
                let nin = din.perp() * w;
                let z = c.base + (k + 1) as f64 * c.h;
                out.push(([s - din * w - nin, s + din * w - nin, s + din * w + nin, s - din * w + nin], z));
                profil.push((s, z));
                k += 1;
            }
            cumul += a.distance(*b);
            // marches de cette volée : partage au prorata de la longueur cumulée
            let jusqua = ((cumul / total) * marches as f64).round() as usize;
            let m = jusqua.saturating_sub(faites).max(if i + 1 == volees.len() { marches - faites } else { 0 });
            let g = a.distance(*b) / m.max(1) as f64;
            for j in 0..m {
                let (p0, p1) = (*a + d * (j as f64 * g), *a + d * ((j + 1) as f64 * g));
                let z = c.base + (k + 1) as f64 * c.h;
                out.push(([p0 - nrm, p1 - nrm, p1 + nrm, p0 + nrm], z));
                profil.push((p0, z));
                k += 1;
            }
            faites += m;
        }
        profil.push((pt(*self.trace.last().unwrap()), c.base + c.hauteur));
        (out, profil)
    }

    /// Solide de l'escalier : marches pleines sur paillasse crantée.
    pub fn solide(&self, p: &Projet) -> Result<Vec<Tri>, String> {
        let c = self.calcul(p)?;
        let (marches, _) = self.marches(&c);
        let mut t = Vec::new();
        for (q, z) in marches {
            let bas = (z - c.h - self.paillasse).max(c.base - self.paillasse);
            t.extend(hexaedre([q[0].extend(bas), q[1].extend(bas), q[2].extend(bas), q[3].extend(bas), q[0].extend(z), q[1].extend(z), q[2].extend(z), q[3].extend(z)]));
        }
        Ok(t)
    }

    /// Altitude des marches au droit d'un point en plan : projection sur la ligne de
    /// foulée, interpolée entre nez de marche.
    pub fn altitude_en(&self, p: &Projet, q: DVec2) -> Result<f64, String> {
        let c = self.calcul(p)?;
        let (_, profil) = self.marches(&c);
        // abscisse curviligne de la projection sur la ligne de foulée
        let mut meilleur = (f64::INFINITY, 0.0);
        let mut s0 = 0.0;
        for w in self.trace.windows(2) {
            let (a, b) = (pt(w[0]), pt(w[1]));
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            let d = q.distance(a + ab * t);
            if d < meilleur.0 {
                meilleur = (d, s0 + ab.length() * t);
            }
            s0 += ab.length();
        }
        let s = meilleur.1;
        // abscisses du profil
        let mut acc = Vec::new();
        let mut s1 = 0.0;
        let mut seg = 0;
        for (pp, z) in &profil {
            while seg + 1 < self.trace.len() {
                let (a, b) = (pt(self.trace[seg]), pt(self.trace[seg + 1]));
                let ab = b - a;
                let t = (*pp - a).dot(ab) / ab.length_squared().max(1e-12);
                if t <= 1.0 + 1e-9 {
                    acc.push((s1 + ab.length() * t.clamp(0.0, 1.0), *z));
                    break;
                }
                s1 += ab.length();
                seg += 1;
            }
        }
        acc.sort_by(|a, b| a.0.total_cmp(&b.0));
        let i = acc.iter().position(|(x, _)| *x >= s).unwrap_or(acc.len() - 1);
        if i == 0 {
            return Ok(acc[0].1);
        }
        let ((x0, z0), (x1, z1)) = (acc[i - 1], acc[i]);
        Ok(if x1 - x0 > 1e-9 { z0 + (z1 - z0) * (s - x0) / (x1 - x0) } else { z1 })
    }

    /// Tracés des garde-corps des deux côtés (gauche puis droite du sens de montée),
    /// à `retrait` du bord.
    pub fn traces_garde_corps(&self, retrait: f64) -> [Vec<[f64; 2]>; 2] {
        let decale = |s: f64| decaler_polyligne(&self.trace, s * (self.largeur / 2.0 - retrait));
        [decale(1.0), decale(-1.0)]
    }
}

/// Polyligne décalée de `d` vers sa gauche (onglets aux sommets).
pub fn decaler_polyligne(points: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    let p: Vec<DVec2> = points.iter().map(|x| pt(*x)).collect();
    let n = p.len();
    (0..n)
        .map(|i| {
            let avant = (i > 0).then(|| (p[i] - p[i - 1]).normalize_or_zero().perp());
            let apres = (i + 1 < n).then(|| (p[i + 1] - p[i]).normalize_or_zero().perp());
            let q = match (avant, apres) {
                (Some(a), Some(b)) => {
                    let m = (a + b).normalize_or_zero();
                    let c = m.dot(a).max(0.2);
                    p[i] + m * d / c
                }
                (Some(a), None) | (None, Some(a)) => p[i] + a * d,
                (None, None) => p[i],
            };
            q.to_array()
        })
        .collect()
}

impl GardeCorps {
    /// Altitude de la base au droit d'un point (escalier hôte, sinon niveau et décalage).
    fn base(&self, p: &Projet, q: DVec2) -> Result<f64, String> {
        match self.escalier.as_ref().and_then(|id| p.escaliers.iter().find(|e| &e.id == id)) {
            Some(e) => e.altitude_en(p, q),
            None => Ok(p.niveau(&self.niveau).ok_or("Niveau du garde-corps absent.")?.altitude + self.decalage),
        }
    }

    /// Solides : (vitrage, triangles) ; main courante, lisse basse, montants,
    /// remplissage.
    pub fn solides(&self, p: &Projet) -> Result<Vec<(bool, Vec<Tri>)>, String> {
        if self.trace.len() < 2 || !self.trace.iter().flatten().all(|x| x.is_finite()) {
            return Err("Garde-corps : au moins deux points.".into());
        }
        if !(self.hauteur > 200.0 && self.hauteur.is_finite()) {
            return Err("Garde-corps : hauteur non valide.".into());
        }
        let h = self.hauteur;
        // échantillons le long du tracé : sommets, tous les 250 mm (pente des marches)
        let mut ech: Vec<(DVec2, bool)> = Vec::new();
        for (i, w) in self.trace.windows(2).enumerate() {
            let (a, b) = (pt(w[0]), pt(w[1]));
            let n = ((a.distance(b) / 250.0).ceil() as usize).max(1);
            for k in 0..n {
                ech.push((a.lerp(b, k as f64 / n as f64), k == 0));
            }
            if i + 2 == self.trace.len() {
                ech.push((b, true));
            }
        }
        let z: Vec<f64> = ech.iter().map(|(q, _)| self.base(p, *q)).collect::<Result<_, _>>()?;
        let point = |i: usize, dz: f64| ech[i].0.extend(z[i] + dz);
        let mut metal = Vec::new();
        let mut verre = Vec::new();
        for i in 0..ech.len() - 1 {
            metal.extend(barre_3d(point(i, h - 25.0), point(i + 1, h - 25.0), 50.0, 50.0));
            metal.extend(barre_3d(point(i, 120.0), point(i + 1, 120.0), 40.0, 40.0));
            match self.remplissage {
                Remplissage::Barreaudage => {}
                r => {
                    // panneau de 20 mm entre lisse et main courante
                    let (a, b) = (ech[i].0, ech[i + 1].0);
                    let n = (b - a).normalize_or_zero().perp() * 10.0;
                    let (za, zb) = (z[i], z[i + 1]);
                    let c = [
                        (a - n).extend(za + 140.0), (b - n).extend(zb + 140.0), (b + n).extend(zb + 140.0), (a + n).extend(za + 140.0),
                        (a - n).extend(za + h - 50.0), (b - n).extend(zb + h - 50.0), (b + n).extend(zb + h - 50.0), (a + n).extend(za + h - 50.0),
                    ];
                    if r == Remplissage::Vitre { verre.extend(hexaedre(c)) } else { metal.extend(hexaedre(c)) }
                }
            }
        }
        // montants : aux sommets et tous les 1,5 m au plus
        let mut depuis = f64::INFINITY;
        let mut prec = ech[0].0;
        for i in 0..ech.len() {
            depuis += ech[i].0.distance(prec);
            prec = ech[i].0;
            if ech[i].1 || depuis >= 1500.0 || i + 1 == ech.len() {
                metal.extend(barre_3d(point(i, 0.0), point(i, h - 50.0), 50.0, 50.0));
                depuis = 0.0;
            }
        }
        // barreaux de 20 mm tous les 110 mm
        if self.remplissage == Remplissage::Barreaudage {
            for w in self.trace.windows(2) {
                let (a, b) = (pt(w[0]), pt(w[1]));
                let n = ((a.distance(b) / 110.0).floor() as usize).max(1);
                for k in 1..n {
                    let q = a.lerp(b, k as f64 / n as f64);
                    let zb = self.base(p, q)?;
                    metal.extend(barre_3d(q.extend(zb + 140.0), q.extend(zb + h - 50.0), 20.0, 20.0));
                }
            }
        }
        Ok([(false, metal), (true, verre)].into_iter().filter(|(_, t)| !t.is_empty()).collect())
    }

    pub fn longueur(&self) -> f64 {
        self.trace.windows(2).map(|w| pt(w[0]).distance(pt(w[1]))).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet() -> Projet {
        let mut p = Projet::nouveau("Escalier");
        p.niveaux.push(Niveau { id: "rdc".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        p.niveaux.push(Niveau { id: "r1".into(), nom: "R+1".into(), altitude: 2800.0, origine: None });
        p
    }

    fn escalier(trace: Vec<[f64; 2]>) -> Escalier {
        Escalier { id: "e".into(), nom: "Escalier".into(), niveau: "rdc".into(), niveau_haut: Some("r1".into()), decalage_bas: 0.0, hauteur: 2800.0, trace, largeur: 1000.0, hauteur_marche: 175.0, paillasse: 150.0 }
    }

    #[test]
    fn escalier_droit_et_quart_tournant() {
        let p = projet();
        // droit : 16 contremarches de 175, 15 marches sur 4,05 m → giron 270
        let e = escalier(vec![[0.0, 0.0], [4050.0, 0.0]]);
        let c = e.calcul(&p).unwrap();
        assert_eq!(c.contremarches, 16);
        assert!((c.h - 175.0).abs() < 1e-9 && (c.giron - 270.0).abs() < 1e-9 && c.confortable(), "{c:?}");
        let t = e.solide(&p).unwrap();
        assert!(crate::modele::geometrie::volume(&t) > 0.0);
        let zmax = t.iter().flatten().map(|v| v.z).fold(f64::MIN, f64::max);
        assert!((zmax - 2625.0).abs() < 1e-6, "{zmax}");
        // quart tournant : un palier qui compte pour une marche
        let q = escalier(vec![[0.0, 0.0], [2500.0, 0.0], [2500.0, 2500.0]]);
        let c = q.calcul(&p).unwrap();
        assert_eq!(c.paliers, 1);
        assert!((c.giron - 4000.0 / 14.0).abs() < 1e-9, "{c:?}");
        let marches = q.marches(&c).0;
        assert_eq!(marches.len(), 15);
        // altitude interpolée : mi-hauteur vers le palier, arrivée au plancher
        let z = q.altitude_en(&p, DVec2::new(2500.0, 2500.0)).unwrap();
        assert!((z - 2800.0).abs() < 1e-6);
        assert!(q.altitude_en(&p, DVec2::new(1000.0, 0.0)).unwrap() > 0.0);
        // trop de paliers
        let mauvais = Escalier { niveau_haut: None, hauteur: 400.0, ..q.clone() };
        assert!(mauvais.calcul(&p).is_err());
    }

    #[test]
    fn commandes_escalier_et_garde_corps() {
        use crate::modele::commandes::Commande;
        let p = projet();
        let e = Escalier { id: String::new(), ..escalier(vec![[0.0, 0.0], [4050.0, 0.0]]) };
        let (p, ids) = p.executer(Commande::Escalier { escalier: e.clone() }).unwrap();
        let id = ids[0].clone();
        let [g, _] = p.escaliers[0].traces_garde_corps(50.0);
        let gc = GardeCorps { id: String::new(), nom: "GC".into(), niveau: "rdc".into(), decalage: 0.0, trace: g, hauteur: 1000.0, remplissage: Remplissage::Barreaudage, escalier: Some(id.clone()) };
        let (p, _) = p.executer(Commande::GardeCorps { garde_corps: gc }).unwrap();
        // copie : le garde-corps suit l'escalier copié
        let (q, copies) = p.executer(Commande::Copier { ids: vec![id.clone()], delta: [0.0, 3000.0, 0.0] }).unwrap();
        assert_eq!((q.escaliers.len(), q.garde_corps.len()), (2, 2));
        assert!(q.garde_corps.iter().any(|g| g.escalier.as_ref() == Some(&copies[0])));
        // suppression : les garde-corps hébergés partent avec l'escalier
        let (q, _) = p.executer(Commande::Supprimer { ids: vec![id] }).unwrap();
        assert!(q.escaliers.is_empty() && q.garde_corps.is_empty());
        // escalier impossible refusé
        assert!(p.executer(Commande::Escalier { escalier: Escalier { trace: vec![[0.0, 0.0]], ..e } }).is_err());
        // export IFC : classes d'escalier et de garde-corps
        let ifc = crate::modele::export_ifc::exporter(&p).unwrap();
        assert!(ifc.contains("IFCSTAIR(") && ifc.contains("IFCRAILING("));
    }

    #[test]
    fn garde_corps_sur_escalier_et_plat() {
        let mut p = projet();
        let e = escalier(vec![[0.0, 0.0], [4050.0, 0.0]]);
        let [g, d] = e.traces_garde_corps(50.0);
        assert!((g[0][1] - 450.0).abs() < 1e-9 && (d[0][1] + 450.0).abs() < 1e-9);
        p.escaliers.push(e);
        let gc = GardeCorps { id: "g".into(), nom: "GC".into(), niveau: "rdc".into(), decalage: 0.0, trace: g, hauteur: 1000.0, remplissage: Remplissage::Barreaudage, escalier: Some("e".into()) };
        let s = gc.solides(&p).unwrap();
        let zmax = s.iter().flat_map(|(_, t)| t.iter().flatten()).map(|v| v.z).fold(f64::MIN, f64::max);
        // la main courante monte avec l'escalier
        assert!(zmax > 2800.0 && zmax < 3900.0, "{zmax}");
        let vitre = GardeCorps { escalier: None, remplissage: Remplissage::Vitre, ..gc.clone() };
        let s = vitre.solides(&p).unwrap();
        assert!(s.iter().any(|(v, t)| *v && !t.is_empty()));
        assert!((vitre.longueur() - 4050.0).abs() < 1e-9);
        // tous les hexaèdres sont fermés et bien orientés : volume positif
        let t: Vec<Tri> = s.into_iter().flat_map(|(_, t)| t).collect();
        assert!(crate::modele::geometrie::volume(&t) > 0.0);
    }
}
