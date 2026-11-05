//! Création paramétrique d'éléments natifs. Les coordonnées sont en millimètres.
pub mod commandes;
pub mod historique;
pub(crate) mod validation;
use crate::modele::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Implantation { Axe, Exterieure, Interieure, AxePorteur }

impl Implantation {
    pub fn reference(self) -> crate::modele::murs::assemblage::Reference {
        use crate::modele::murs::assemblage::Reference;
        match self {
            Self::Axe => Reference::Axe,
            Self::Exterieure => Reference::FaceExterieure,
            Self::Interieure => Reference::FaceInterieure,
            Self::AxePorteur => Reference::AxePorteur,
        }
    }
}

#[derive(Clone)]
pub struct ParametresMur {
    pub hauteur: f64,
    pub epaisseur: f64,
    pub implantation: Implantation,
    pub niveau: Option<String>,
    pub decalage: f64,
    pub niveau_haut: Option<String>,
    pub decalage_haut: f64,
    pub phase: Option<String>,
    pub sous_projet: Option<String>,
    pub variante: Option<String>,
    /// Type de mur existant ; None : type monocouche de l'épaisseur `epaisseur`. Un type
    /// à plusieurs couches crée un assemblage (un mur par couche).
    pub type_mur: Option<String>,
    /// Sens inversé : le mur est tracé de l'autre côté de la ligne (extérieur et intérieur
    /// échangés).
    pub inverse: bool,
}

impl Default for ParametresMur {
    fn default() -> Self {
        Self { hauteur: 2800.0, epaisseur: 300.0, implantation: Implantation::Axe, niveau: None, decalage: 0.0, niveau_haut: None, decalage_haut: 0.0, phase: None, sous_projet: None, variante: None, type_mur: None, inverse: false }
    }
}

impl ParametresMur {
    /// Type tracé : celui choisi, sinon une couche de l'épaisseur saisie.
    pub fn gabarit(&self, projet: &Projet) -> TypeCompose {
        self.type_mur.as_ref().and_then(|id| projet.type_mur(id)).cloned().unwrap_or_else(|| TypeCompose {
            id: String::new(),
            nom: String::new(),
            couches: vec![Couche { materiau: None, epaisseur: self.epaisseur, ..Default::default() }],
            categorie: None, couches_detaillees: true,
            origine: None,
        })
    }

    /// Altitudes résolues pour la validation, le schéma et l'aperçu 3D (mm).
    pub fn bornes(&self, projet: &Projet) -> Result<(f64, f64), String> {
        let base = match &self.niveau {
            Some(id) => projet.niveau(id).ok_or("Limite basse introuvable.")?.altitude,
            None => projet.niveaux.first().map_or(0.0, |n| n.altitude),
        };
        let bas = base + self.decalage;
        let haut = match &self.niveau_haut {
            Some(id) => projet.niveau(id).ok_or("Limite haute introuvable.")?.altitude + self.decalage_haut,
            None => bas + self.hauteur + self.decalage_haut,
        };
        if ![base, bas, haut, self.decalage, self.decalage_haut].iter().all(|x| x.is_finite()) || haut <= bas {
            return Err("La limite haute du mur doit être au-dessus de sa limite basse ; les valeurs doivent être finies.".into());
        }
        Ok((bas, haut))
    }
}

/// Paramètres de création d'un élément horizontal : sorte, type composé (couches du
/// haut vers le bas), niveau et altitude de la référence (dessus, ou sous-face d'un
/// plafond et d'une toiture) par rapport à lui ; pente donnée à toutes les arêtes.
#[derive(Clone)]
pub struct ParametresDalle {
    pub sorte: SorteDalle,
    pub type_dalle: TypeCompose,
    pub pente: Option<f64>,
    pub niveau: Option<String>,
    pub decalage: f64,
    pub phase: Option<String>,
    pub sous_projet: Option<String>,
    pub variante: Option<String>,
}

/// Contour fermé simple : au moins trois sommets distincts, sans arête qui en croise
/// une autre (sommets en double et point de fermeture retirés), orienté dans le sens
/// trigonométrique.
pub fn contour_valide(points: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, String> {
    let mut c: Vec<[f64; 2]> = Vec::new();
    for &p in points {
        if !p.iter().all(|x| x.is_finite()) {
            return Err("Les coordonnées du contour doivent être finies.".into());
        }
        if c.last().is_none_or(|q: &[f64; 2]| (q[0] - p[0]).hypot(q[1] - p[1]) > 0.5) {
            c.push(p);
        }
    }
    while c.len() > 1 && (c[0][0] - c[c.len() - 1][0]).hypot(c[0][1] - c[c.len() - 1][1]) <= 0.5 {
        c.pop();
    }
    if c.len() < 3 {
        return Err("Le contour doit avoir au moins trois sommets.".into());
    }
    let aire = geometrie::aire_pair_impair(std::slice::from_ref(&c));
    if aire < 10_000.0 {
        return Err("Le contour est trop petit (moins de 0,01 m²).".into());
    }
    let n = c.len();
    let croise = |a: [f64; 2], b: [f64; 2], p: [f64; 2], q: [f64; 2]| {
        let o = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let (d1, d2, d3, d4) = (o(p, q, a), o(p, q, b), o(a, b, p), o(a, b, q));
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if croise(c[i], c[(i + 1) % n], c[j], c[(j + 1) % n]) {
                return Err("Le contour se recoupe.".into());
            }
        }
    }
    let signe: f64 = (0..n).map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1]).sum();
    if signe < 0.0 {
        c.reverse();
    }
    Ok(c)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListeCreation { Phase, SousProjet, Variante }

impl ListeCreation {
    pub fn libelle(self) -> &'static str {
        match self { Self::Phase => "Phase", Self::SousProjet => "Sous-projet", Self::Variante => "Variante" }
    }
}

fn identifiant_natif(espace: &str, _cle: &str) -> String { format!("{espace}-{}", crate::modele::collaboration::uuid()) }

impl Projet {
    /// Intersection pour Ajuster/Prolonger ; l'extrémité opposée reste fixe.
    pub fn cible_ajustement(&self,id:&str,reference:&str,fin:bool)->Result<[f64;2],String> {
        use glam::DVec2;
        if id==reference {return Err("Choisissez un mur différent de la limite.".into());}
        let (a,b,_)=self.axe_element(id).ok_or("Mur droit requis.")?;
        let (c,d,_)=self.axe_element(reference).ok_or("Référence droite requise.")?;
        let [a,b,c,d]=[a,b,c,d].map(DVec2::from_array);
        let (u,v)=(b-a,d-c);let den=u.perp_dot(v);
        if u.length_squared()<1e-12 || v.length_squared()<1e-12 || den.abs()<1e-9*u.length()*v.length() {return Err("Murs parallèles : aucune intersection.".into());}
        let t=(c-a).perp_dot(v)/den;
        if !t.is_finite() || (fin&&t<=1e-9) || (!fin&&t>=1.0-1e-9) {return Err("La limite dépasserait l'autre extrémité du mur.".into());}
        let point=a+u*t;
        if !point.is_finite(){return Err("Intersection non valide.".into());}
        Ok(point.to_array())
    }
    /// Identifiant libre pour une nouvelle vue et son nom (« Coupe 3 »).
    fn nouvelle_vue(&self, prefixe: &str) -> (String, usize) {
        let mut k = 1;
        loop {
            let id = identifiant_natif("vue", &format!("{prefixe}-{k}"));
            if self.vues.iter().all(|v| v.id != id) {
                return (id, k);
            }
            k += 1;
        }
    }

    /// Ajoute un plan d'étage du niveau `niveau` (nom du niveau, suffixé s'il en existe
    /// déjà un), coupe à `coupe` mm au-dessus du niveau.
    pub fn ajouter_vue_plan(&mut self, niveau: &str, coupe: f64) -> Result<String, String> {
        let n = self.niveau(niveau).ok_or("Niveau introuvable.")?.clone();
        if !coupe.is_finite() {
            return Err("La hauteur du plan de coupe doit être finie.".into());
        }
        let (id, _) = self.nouvelle_vue("plan");
        let deja = self.vues.iter().filter(|v| matches!(&v.genre, GenreVue::Plan { niveau: x, .. } if x == niveau)).count();
        let nom = if deja == 0 { n.nom.clone() } else { format!("{} ({})", n.nom, deja + 1) };
        self.vues.push(Vue { id: id.clone(), nom, type_vue: String::new(), echelle: 100.0, orientation_plan: 0.0, coupe_active: None, poche_vue: None, elements_masques: Vec::new(), contour_cadrage: Vec::new(), fond_plan: None, limite_haute_niveau: None, limite_haute_decalage: 2800.0, genre: GenreVue::Plan { bas_niveau: n.id.clone(), bas_decalage: 0.0, niveau: n.id, coupe } });
        Ok(id)
    }

    /// Ajoute une coupe (ou une élévation) le long de `debut` → `fin` (mm), regardant
    /// vers la gauche, de type `type_vue` à l'échelle 1:`echelle`.
    pub fn ajouter_vue_coupe(&mut self, debut: [f64; 2], fin: [f64; 2], profondeur: f64, elevation: bool, type_vue: &str, echelle: f64) -> Result<String, String> {
        if !debut.iter().chain(&fin).chain([&profondeur]).all(|x| x.is_finite()) || (fin[0] - debut[0]).hypot(fin[1] - debut[1]) < 100.0 {
            return Err("La ligne de coupe doit mesurer au moins 100 mm.".into());
        }
        if profondeur <= 0.0 {
            return Err("La profondeur de la coupe doit être positive.".into());
        }
        if !(echelle >= 1.0 && echelle.is_finite()) {
            return Err("L’échelle doit être au moins 1:1.".into());
        }
        let (prefixe, nom) = if elevation { ("elevation", "Élévation") } else { ("coupe", "Coupe") };
        let (id, _) = self.nouvelle_vue(prefixe);
        // numéro : premier libre parmi les vues du même nom
        let mut k = 1;
        while self.vues.iter().any(|v| v.nom == format!("{nom} {k}")) {
            k += 1;
        }
        self.vues.push(Vue { id: id.clone(), nom: format!("{nom} {k}"), type_vue: type_vue.into(), echelle, orientation_plan: 0.0, coupe_active: None, poche_vue: None, elements_masques: Vec::new(), contour_cadrage: Vec::new(), fond_plan: None, limite_haute_niveau: None, limite_haute_decalage: 2800.0, genre: GenreVue::Coupe { debut, fin, profondeur, elevation } });
        self.arrondir();
        Ok(id)
    }

    /// Renomme une vue ; le nom doit être unique et non vide.
    pub fn renommer_vue(&mut self, id: &str, nom: &str) -> Result<(), String> {
        let nom = nom.trim();
        if nom.is_empty() {
            return Err("Le nom de la vue ne peut pas être vide.".into());
        }
        if self.vues.iter().any(|v| v.nom == nom && v.id != id) {
            return Err(format!("Une vue s’appelle déjà « {nom} »."));
        }
        self.vues.iter_mut().find(|v| v.id == id).ok_or("Vue introuvable.")?.nom = nom.into();
        Ok(())
    }

    /// Retourne une coupe ou une élévation : elle regarde de l'autre côté de sa ligne.
    pub fn retourner_vue(&mut self, id: &str) -> Result<(), String> {
        match &mut self.vues.iter_mut().find(|v| v.id == id).ok_or("Vue introuvable.")?.genre {
            GenreVue::Coupe { debut, fin, .. } => {
                std::mem::swap(debut, fin);
                Ok(())
            }
            GenreVue::Plan { .. } => Err("Un plan ne se retourne pas.".into()),
        }
    }

    pub fn supprimer_vue(&mut self, id: &str) -> Result<(), String> {
        let avant = self.vues.len();
        self.vues.retain(|v| v.id != id);
        if self.vues.len() == avant { Err("Vue introuvable.".into()) } else {
            self.annotations.retain(|a| a.vue!=id);
            for f in &mut self.feuilles {f.vues.retain(|v|v.vue!=id);}
            Ok(())
        }
    }

    /// Axe d'un mur ou d'un assemblage (moyenne des axes de ses couches) : début, fin
    /// (mm) et altitude de la base.
    pub fn axe_element(&self, id: &str) -> Option<([f64; 2], [f64; 2], f64)> {
        let murs: Vec<&Mur> = match self.assemblage(id) {
            Some(_) => self.couches_assemblage(id),
            None => self.murs.iter().filter(|m| m.id == id).collect(),
        };
        let (mut a, mut b) = (glam::DVec2::ZERO, glam::DVec2::ZERO);
        for m in &murs {
            let Axe::Droit { debut, fin } = m.axe else { return None };
            a += glam::DVec2::from_array(debut);
            b += glam::DVec2::from_array(fin);
        }
        let n = murs.len() as f64;
        let premier = murs.first()?;
        let z = self.niveau(&premier.niveau)?.altitude + premier.decalage_bas;
        Some(((a / n).to_array(), (b / n).to_array(), z))
    }

    /// Déplace une extrémité (`fin` : la fin, sinon le début) de l'axe d'un mur ou d'un
    /// assemblage ; les couches d'un assemblage gardent leur écart à l'axe.
    pub fn deplacer_extremite(&mut self, id: &str, fin: bool, p: [f64; 2]) -> Result<(), String> {
        use glam::DVec2;
        let (a, b, _) = self.axe_element(id).ok_or("Mur introuvable ou courbe.")?;
        let (a, b) = (DVec2::from_array(a), DVec2::from_array(b));
        let (na, nb) = if fin { (a, DVec2::from_array(p)) } else { (DVec2::from_array(p), b) };
        if !p.iter().all(|x| x.is_finite()) || (nb - na).length() < 1.0 {
            return Err("Le mur doit mesurer au moins 1 mm.".into());
        }
        let normale = |a: DVec2, b: DVec2| (b - a).normalize().perp();
        let (n0, n1) = (normale(a, b), normale(na, nb));
        let couches: Vec<String> = match self.assemblage(id) {
            Some(x) => x.couches.clone(),
            None => vec![id.to_string()],
        };
        for c in couches {
            let Some(m) = self.murs.iter_mut().find(|m| m.id == c) else { continue };
            let Axe::Droit { debut, fin } = m.axe else { continue };
            // écart de la couche à l'axe, gardé
            let ecart = (DVec2::from_array(debut) - a).dot(n0);
            let _ = fin;
            m.axe = Axe::Droit { debut: (na + n1 * ecart).to_array(), fin: (nb + n1 * ecart).to_array() };
        }
        self.arrondir();
        Ok(())
    }

    /// Déplace une poignée de la ligne de justification en gardant l'autre fixe.
    pub fn deplacer_extremite_justification(&mut self,id:&str,fin:bool,point:[f64;2])->Result<(),String> {
        use glam::DVec2;
        self.axe_element(id).ok_or("Mur droit introuvable")?;
        let (ca0,cb0,_)=self.ligne_justification(id).ok_or("Ligne de justification introuvable")?;
        if !point.iter().all(|x|x.is_finite()) {return Err("Coordonnées non finies".into());}
        let (ca0,cb0)=(DVec2::from_array(ca0),DVec2::from_array(cb0));
        let (ca,cb)=if fin {(ca0,DVec2::from_array(point))}else{(DVec2::from_array(point),cb0)};
        if ca.distance(cb)<1.0 {return Err("Le mur doit mesurer au moins 1 mm.".into());}
        // Chaque couche garde son écart à la ligne d'implantation elle-même (et non à une
        // moyenne des axes de couches, qui n'est pas le milieu de l'épaisseur totale).
        let ancien=(cb0-ca0).normalize_or_zero().perp();
        let nouveau=(cb-ca).normalize().perp();
        let couches=self.assemblage(id).map(|a|a.couches.clone()).unwrap_or_else(||vec![id.to_string()]);
        for m in self.murs.iter_mut().filter(|m|couches.contains(&m.id)) {
            let Axe::Droit{debut,..}=m.axe else{return Err("Mur droit requis".into())};
            let d=(DVec2::from_array(debut)-ca0).dot(ancien);
            m.axe=Axe::Droit{debut:(ca+nouveau*d).to_array(),fin:(cb+nouveau*d).to_array()};
        }
        self.arrondir();
        Ok(())
    }

    /// Ajoute une dalle de contour `points` ; son type (couches du haut vers le bas) est
    /// repris s'il existe déjà (même nom et mêmes couches), sinon ajouté. Rien ne change
    /// en cas d'erreur.
    #[cfg(test)]
    pub fn ajouter_dalle(&mut self, points: &[[f64; 2]], p: &ParametresDalle) -> Result<String, String> {
        self.ajouter_dalle_contours(&[points.to_vec()], p)
    }

    /// Ajoute un élément horizontal de plusieurs contours : un contour extérieur d'une
    /// autre partie ajoute une partie, un contour intérieur à un autre est une trémie
    /// (règle pair-impair). Les contours ne se croisent pas.
    pub fn ajouter_dalle_contours(&mut self, contours: &[Vec<[f64; 2]>], p: &ParametresDalle) -> Result<String, String> {
        let contours: Vec<Vec<[f64; 2]>> = contours.iter().map(|c| contour_valide(c)).collect::<Result<_, _>>()?;
        if contours.is_empty() {
            return Err("Aucun contour.".into());
        }
        for (i, a) in contours.iter().enumerate() {
            if contours[i + 1..].iter().any(|b| crate::modele::edition::commandes::croisement(a, b)) {
                return Err("Les contours de l'élément se croisent.".into());
            }
        }
        if !p.decalage.is_finite() {
            return Err("Le décalage doit être fini.".into());
        }
        if p.pente.is_some_and(|a| !(a > 0.0 && a < 89.0)) {
            return Err("La pente doit être comprise entre 0 et 89°.".into());
        }
        let t = &p.type_dalle;
        if t.couches.is_empty() || t.couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            return Err("Le type de dalle doit avoir des couches d’épaisseur positive.".into());
        }
        let niveau = match &p.niveau {
            Some(id) => self.niveau(id).ok_or("Niveau introuvable.")?.id.clone(),
            None => self.niveaux.first().map(|n| n.id.clone()).unwrap_or_else(|| identifiant_natif("niveau", "niveau-0")),
        };
        if p.variante.as_ref().is_some_and(|id| !self.variantes.iter().any(|v| &v.id == id)) {
            return Err("Variante introuvable.".into());
        }
        if self.niveaux.is_empty() {
            self.niveaux.push(Niveau { id: niveau.clone(), nom: "Niveau 0".into(), altitude: 0.0, origine: None });
        }
        let type_dalle = match self.types_dalles.iter().find(|x| x.nom == t.nom && x.couches == t.couches) {
            Some(x) => x.id.clone(),
            None => {
                let mut k = self.types_dalles.len() + 1;
                let id = loop {
                    let id = identifiant_natif("type-dalle", &format!("{}:{k}", t.nom));
                    if self.types_dalles.iter().all(|x| x.id != id) { break id; }
                    k += 1;
                };
                self.types_dalles.push(TypeCompose { id: id.clone(), nom: t.nom.clone(), couches: t.couches.clone(), categorie: None, couches_detaillees: true, origine: None });
                id
            }
        };
        let mut k = self.dalles.len() + 1;
        let id = loop {
            let id = identifiant_natif("dalle", &format!("natif-{k}"));
            if self.dalles.iter().all(|d| d.id != id) { break id; }
            k += 1;
        };
        if self.phases.is_empty() { self.phases.push(p.phase.clone().unwrap_or_else(|| "Projet".into())); }
        if self.sous_projets.is_empty() { self.sous_projets.push(p.sous_projet.clone().unwrap_or_else(|| "A".into())); }
        self.dalles.push(Dalle {
            variante: p.variante.clone(),
            phase: p.phase.clone().or_else(|| self.phases.last().cloned()),
            sous_projet: p.sous_projet.clone().or_else(|| self.sous_projets.first().cloned()),
            id: id.clone(),
            nom: format!("{} {k}", t.nom),
            type_dalle,
            niveau,
            sorte: p.sorte,
            decalage: p.decalage,
            pentes: p.pente.map(|a| contours.iter().map(|c| vec![Some(a); c.len()]).collect()).unwrap_or_default(),
            contours,
            origine: None,
        });
        self.completer_materiaux();
        self.arrondir();
        Ok(id)
    }

    /// Ajout atomique aux listes communes à tous les outils de création.
    pub fn ajouter_choix_creation(&mut self, liste: ListeCreation, nom: &str, jeu: &str) -> Result<String, String> {
        let nom = nom.trim();
        let jeu = jeu.trim();
        if nom.is_empty() { return Err("Saisissez un nom.".into()); }
        if liste == ListeCreation::Variante {
            if jeu.is_empty() { return Err("Saisissez le jeu de variantes.".into()); }
            if self.variantes.iter().any(|v| v.jeu.eq_ignore_ascii_case(jeu) && v.nom.eq_ignore_ascii_case(nom)) {
                return Err("Cette variante existe déjà dans ce jeu.".into());
            }
            let id = identifiant_natif("variante", &format!("{}:{jeu}:{nom}", jeu.len()));
            if self.variantes.iter().any(|v| v.id == id) { return Err("Identifiant de variante déjà utilisé.".into()); }
            self.variantes.push(Variante { id: id.clone(), nom: nom.into(), jeu: jeu.into() });
            return Ok(id);
        }
        let (choix, defaut) = match liste { ListeCreation::Phase => (&mut self.phases, "Projet"), _ => (&mut self.sous_projets, "A") };
        if choix.iter().any(|v| v.eq_ignore_ascii_case(nom)) || (choix.is_empty() && defaut.eq_ignore_ascii_case(nom)) {
            return Err("Ce nom existe déjà.".into());
        }
        if choix.is_empty() { choix.push(defaut.into()); }
        choix.push(nom.into());
        Ok(nom.into())
    }

    /// Valide entièrement la commande avant d'ajouter le niveau, le type et le mur.
    pub fn ajouter_mur(&mut self, debut: [f64; 2], fin: [f64; 2], p: &ParametresMur) -> Result<String, String> {
        self.ajouter_mur_axe(Axe::Droit { debut, fin }, p)
    }

    /// Ajoute un mur droit ou courbe de ligne tracée `trace` (implantation de `p`).
    pub fn ajouter_mur_axe(&mut self, trace: Axe, p: &ParametresMur) -> Result<String, String> {
        // sens inversé : la ligne est parcourue en sens contraire
        let trace = match (p.inverse, trace) {
            (true, Axe::Droit { debut, fin }) => Axe::Droit { debut: fin, fin: debut },
            (true, Axe::Arc { debut, fin, centre, direct }) => Axe::Arc { debut: fin, fin: debut, centre, direct: !direct },
            (_, t) => t,
        };
        let points = match trace {
            Axe::Droit { debut, fin } => vec![debut, fin],
            Axe::Arc { debut, fin, centre, .. } => vec![debut, fin, centre],
        };
        if !points.iter().flatten().all(|x| x.is_finite()) || ![p.hauteur, p.epaisseur, p.decalage].iter().all(|x| x.is_finite()) {
            return Err("Les coordonnées et dimensions du mur doivent être finies.".into());
        }
        let longueur = trace.longueur();
        if !longueur.is_finite() || longueur < 1.0 { return Err("Le mur doit avoir une longueur finie d’au moins 1 mm.".into()); }
        if p.hauteur <= 0.0 || p.epaisseur <= 0.0 { return Err("La hauteur et l’épaisseur doivent être positives.".into()); }
        let niveau = match &p.niveau {
            Some(id) => self.niveau(id).ok_or("Niveau de base introuvable.")?.id.clone(),
            None => self.niveaux.first().map(|n| n.id.clone()).unwrap_or_else(|| identifiant_natif("niveau", "niveau-0")),
        };
        let (bas, haut) = p.bornes(self)?;
        for (valeur, choix, nom) in [(&p.phase, &self.phases, "Phase"), (&p.sous_projet, &self.sous_projets, "Sous-projet")] {
            if let Some(v) = valeur {
                if v.trim().is_empty() || (!choix.is_empty() && !choix.contains(v)) {
                    return Err(format!("{nom} introuvable."));
                }
            }
        }
        if p.variante.as_ref().is_some_and(|id| !self.variantes.iter().any(|v| &v.id == id)) {
            return Err("Variante introuvable.".into());
        }
        let compose = match &p.type_mur {
            Some(id) => Some(self.type_mur(id).ok_or("Type de mur introuvable.")?.clone()),
            None => None,
        };
        if compose.as_ref().is_some_and(|t| t.couches.is_empty() || t.couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite()))) {
            return Err("Le type de mur doit avoir des couches d’épaisseur positive.".into());
        }
        let existant = match &compose {
            Some(t) => Some(t.id.clone()),
            None => self.types_murs.iter().find(|t| (t.epaisseur() - p.epaisseur).abs() < 1e-6 && t.couches.len() == 1).map(|t| t.id.clone()),
        };
        let type_mur = existant.clone().unwrap_or_else(|| {
            let mut k = self.types_murs.len() + 1;
            loop { let id = identifiant_natif("type-mur", &format!("natif-{k}")); if self.types_murs.iter().all(|t| t.id != id) { break id; } k += 1; }
        });
        let mut k = self.murs.len() + self.assemblages.len() + 1;
        let id = loop { let id = identifiant_natif("mur", &format!("natif-{k}")); if self.murs.iter().all(|m| m.id != id) && self.assemblages.iter().all(|a| a.id != id) { break id; } k += 1; };
        // ligne tracée → milieu de l'épaisseur totale : décalage de la référence vers l'intérieur
        let gabarit = p.gabarit(self);
        let decalage = gabarit.decalage_reference(p.implantation.reference());
        if self.niveaux.is_empty() {
            self.niveaux.push(Niveau { id: niveau.clone(), nom: "Niveau 0".into(), altitude: 0.0, origine: None });
        }
        if existant.is_none() {
            self.types_murs.push(TypeCompose { id: type_mur.clone(), nom: format!("Mur · {} mm", p.epaisseur), couches: vec![Couche { materiau: None, epaisseur: p.epaisseur, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None });
        }
        let compose = compose.is_some_and(|t| t.couches.len() > 1);
        if self.phases.is_empty() { self.phases.push(p.phase.clone().unwrap_or_else(|| "Projet".into())); }
        if self.sous_projets.is_empty() { self.sous_projets.push(p.sous_projet.clone().unwrap_or_else(|| "A".into())); }
        let contrainte = match &p.niveau_haut {
            Some(niveau) => Haut::Niveau { niveau: niveau.clone(), decalage: p.decalage_haut },
            None => Haut::Libre { hauteur: haut - bas },
        };
        self.murs.push(Mur { variante: p.variante.clone(), phase: p.phase.clone().or_else(|| self.phases.last().cloned()), sous_projet: p.sous_projet.clone().or_else(|| self.sous_projets.first().cloned()), id: id.clone(), nom: format!("Mur {k}"), type_mur, assemblage: None, niveau, decalage_bas: p.decalage,
            haut: contrainte, axe: crate::modele::murs::assemblage::decaler_axe(&trace, -decalage), origine: None });
        // type à plusieurs couches : un mur par couche, regroupés (même identifiant)
        if compose {
            self.eclater_mur(&id)?;
        }
        if p.implantation != Implantation::Axe {
            self.implantations.insert(id.clone(), p.implantation.reference());
        }
        self.arrondir();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn poignee_garde_fixe_autre_extremite_sur_la_justification() {
        for implantation in [Implantation::Axe,Implantation::Exterieure,Implantation::Interieure,Implantation::AxePorteur] {
            for fin in [false,true] {
                let mut p=Projet::nouveau("Poignée");
                let id=p.ajouter_mur([0.,0.],[4000.,0.],&ParametresMur{implantation,..Default::default()}).unwrap();
                let (a,b,_)=p.ligne_justification(&id).unwrap();
                let fixe=if fin {a}else{b};
                let cible=if fin {[4500.,1800.]}else{[-500.,1800.]};
                p.deplacer_extremite_justification(&id,fin,cible).unwrap();
                let (ap,bp,_)=p.ligne_justification(&id).unwrap();
                let proche=|a:[f64;2],b:[f64;2]|(a[0]-b[0]).hypot(a[1]-b[1])<0.002;
                assert!(proche(if fin {ap}else{bp},fixe));
                assert!(proche(if fin {bp}else{ap},cible));
            }
        }
    }

    #[test]
    fn poignee_mur_compose_garde_les_couches_et_la_ligne() {
        use crate::modele::{Couche,FonctionCouche,TypeCompose};
        let c=|m:&str,e:f64,f|Couche{materiau:Some(m.into()),epaisseur:e,fonction:f,plume:None, retour: false};
        for implantation in [Implantation::Axe,Implantation::Exterieure,Implantation::Interieure] {
            for fin in [false,true] {
                let mut p=Projet::nouveau("Poignée composée");
                p.types_murs.push(TypeCompose{id:"t".into(),nom:"Mur".into(),couches:vec![c("Enduit",20.,FonctionCouche::Finition),c("Béton",200.,FonctionCouche::Structure),c("Isolant",100.,FonctionCouche::Isolation)],categorie:None,couches_detaillees:true,origine:None});
                let id=p.ajouter_mur([0.,0.],[4000.,0.],&ParametresMur{implantation,type_mur:Some("t".into()),..Default::default()}).unwrap();
                let (a,b,_)=p.ligne_justification(&id).unwrap();
                let epaisseurs=|p:&Projet|p.couches_assemblage(&id).iter().map(|m|match m.axe{Axe::Droit{debut,fin}=>(debut[0]-fin[0]).hypot(debut[1]-fin[1]),_=>0.}).collect::<Vec<_>>();
                let fixe=if fin {a}else{b};
                let cible=if fin {[4500.,1800.]}else{[-500.,1800.]};
                p.deplacer_extremite_justification(&id,fin,cible).unwrap();
                let (ap,bp,_)=p.ligne_justification(&id).unwrap();
                let proche=|a:[f64;2],b:[f64;2]|(a[0]-b[0]).hypot(a[1]-b[1])<0.002;
                assert!(proche(if fin {ap}else{bp},fixe),"{implantation:?} fixe");
                assert!(proche(if fin {bp}else{ap},cible),"{implantation:?} cible {ap:?} {bp:?} {cible:?}");
                assert_eq!(epaisseurs(&p).len(),3);
            }
        }
    }

    #[test]
    fn ligne_de_justification_retrouvee_apres_creation() {
        for (implantation, attendu) in [(Implantation::Axe, 0.0), (Implantation::Exterieure, 0.0), (Implantation::Interieure, 0.0)] {
            let mut p = Projet::nouveau("J");
            let options = ParametresMur { implantation, epaisseur: 300.0, ..Default::default() };
            let id = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &options).unwrap();
            let (a, b, ecart) = p.ligne_justification(&id).unwrap();
            // la ligne retrouvée est la ligne tracée
            assert!((a[1] - attendu).abs() < 1e-6 && (b[1] - attendu).abs() < 1e-6, "{implantation:?}: {a:?} {b:?}");
            assert!((ecart - options.gabarit(&p).decalage_reference(implantation.reference())).abs() < 1e-6);
        }
    }

    #[test]
    fn dalle_a_plusieurs_contours_parties_et_tremie() {
        let mut p = Projet::nouveau("D");
        let t = TypeCompose { id: String::new(), nom: "Dalle".into(), couches: vec![Couche { epaisseur: 200.0, ..Default::default() }], categorie: None, couches_detaillees: true, origine: None };
        let o = ParametresDalle { sorte: SorteDalle::Dalle, type_dalle: t, pente: None, niveau: None, decalage: 0.0, phase: None, sous_projet: None, variante: None };
        let rect = |x: f64, y: f64, l: f64| vec![[x, y], [x + l, y], [x + l, y + l], [x, y + l]];
        // partie principale, trémie à l'intérieur, seconde partie à l'extérieur
        let id = p.ajouter_dalle_contours(&[rect(0.0, 0.0, 6000.0), rect(1000.0, 1000.0, 1000.0), rect(8000.0, 0.0, 2000.0)], &o).unwrap();
        assert_eq!(p.dalles.iter().find(|d| d.id == id).unwrap().contours.len(), 3);
        // contours qui se croisent : refusés, rien n'est ajouté
        let avant = p.dalles.len();
        assert!(p.ajouter_dalle_contours(&[rect(0.0, 0.0, 3000.0), rect(2000.0, 2000.0, 3000.0)], &o).is_err());
        assert_eq!(p.dalles.len(), avant);
    }

    #[test]
    fn mur_au_sens_inverse_passe_de_l_autre_cote_de_la_ligne() {
        let mut y = Vec::new();
        for inverse in [false, true] {
            let mut p = Projet::nouveau("S");
            let options = ParametresMur { implantation: Implantation::Exterieure, epaisseur: 300.0, inverse, ..Default::default() };
            let id = p.ajouter_mur([0.0, 0.0], [4000.0, 0.0], &options).unwrap();
            let m = p.murs.iter().find(|m| m.id == id).unwrap();
            let Axe::Droit { debut, .. } = m.axe else { panic!() };
            y.push(debut[1]);
        }
        assert!((y[0] + 150.0).abs() < 1e-6 && (y[1] - 150.0).abs() < 1e-6, "{y:?}");
    }

    #[test]
    fn mur_courbe_trace_et_implante() {
        let mut p = Projet::nouveau("Arc");
        // face extérieure (gauche du tracé, côté centre en parcours direct) sur le cercle
        // de rayon 5 m : le milieu de l'épaisseur est à 5 m + 150 mm
        let options = ParametresMur { implantation: Implantation::Exterieure, epaisseur: 300.0, ..Default::default() };
        let id = p.ajouter_mur_axe(Axe::Arc { debut: [5000.0, 0.0], fin: [0.0, 5000.0], centre: [0.0, 0.0], direct: true }, &options).unwrap();
        let m = p.murs.iter().find(|m| m.id == id).unwrap();
        let Axe::Arc { debut, centre, direct: true, .. } = m.axe else { panic!("{:?}", m.axe) };
        assert_eq!(centre, [0.0, 0.0]);
        assert!((debut[0] - 5150.0).abs() < 1e-6, "{debut:?}");
        assert!((m.axe.longueur() - 5150.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn mur_contraintes_phases_et_sous_projets_persistes() {
        let mut p = Projet::nouveau("Contraintes");
        p.niveaux = vec![
            Niveau { id: "bas".into(), nom: "RDC".into(), altitude: 1000., origine: None },
            Niveau { id: "haut".into(), nom: "Étage".into(), altitude: 4200., origine: None },
        ];
        p.phases = vec!["Existant".into(), "Projet".into()];
        p.sous_projets = vec!["Structure".into(), "Enveloppe".into()];
        let mut options = ParametresMur { niveau: Some("bas".into()), niveau_haut: Some("haut".into()),
            decalage: 150., decalage_haut: -200., phase: Some("Projet".into()), sous_projet: Some("Enveloppe".into()), ..Default::default() };
        assert_eq!(options.bornes(&p).unwrap(), (1150., 4000.));
        p.ajouter_mur([0.,0.], [1000.,0.], &options).unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        let mur = &relu.murs[0];
        assert_eq!(relu.bas_haut(mur), Some((1150.,4000.)));
        assert_eq!(mur.haut, Haut::Niveau { niveau: "haut".into(), decalage: -200. });
        assert_eq!(mur.phase.as_deref(), Some("Projet"));
        assert_eq!(mur.sous_projet.as_deref(), Some("Enveloppe"));
        assert_eq!(relu.sous_projets, p.sous_projets);
        let scene = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("test.json")).unwrap();
        assert!(((scene.max.z-scene.min.z) as f64 - 2.85).abs() < 1e-5);
        let avant = p.json().unwrap();
        for (niveau, decalage) in [("absent", 0.), ("bas", 0.), ("haut", -4000.), ("haut", f64::INFINITY)] {
            options.niveau_haut = Some(niveau.into()); options.decalage_haut = decalage;
            assert!(p.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
            assert_eq!(p.json().unwrap(), avant);
        }
        options.niveau_haut = None; options.decalage_haut = 100.;
        assert_eq!(options.bornes(&p).unwrap(), (1150.,4050.));
        options.phase = Some("Absente".into());
        assert!(p.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
        assert_eq!(p.json().unwrap(), avant);
    }

    #[test]
    fn anciens_modeles_sans_phase_ni_sous_projet_restent_lisibles() {
        let mut p = Projet::nouveau("Ancien");
        p.ajouter_mur([0.,0.], [1000.,0.], &ParametresMur::default()).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&p.json().unwrap()).unwrap();
        let objet = json.as_object_mut().unwrap();
        objet.remove("phases"); objet.remove("sous_projets"); objet.remove("variantes");
        let mur = objet["murs"][0].as_object_mut().unwrap();
        mur.remove("phase"); mur.remove("sous_projet"); mur.remove("variante");
        let relu = Projet::lire_json(&json.to_string()).unwrap();
        assert!(relu.phases.is_empty());
        assert!(relu.variantes.is_empty());
        assert!(relu.murs[0].variante.is_none());
        assert!(relu.murs[0].phase.is_none());
        assert_eq!(relu.bas_haut(&relu.murs[0]), Some((0.,2800.)));
    }

    #[test]
    fn listes_creation_ajout_validation_et_persistance() {
        let mut projet = Projet::nouveau("Listes");
        assert_eq!(projet.ajouter_choix_creation(ListeCreation::Phase, "  Livraison  ", "").unwrap(), "Livraison");
        // un nouveau projet a les phases Existant et Projet
        assert_eq!(projet.phases, ["Existant", "Projet", "Livraison"]);
        projet.ajouter_choix_creation(ListeCreation::SousProjet, "Structure", "").unwrap();
        assert_eq!(projet.sous_projets, ["A", "Structure"]);
        let a = projet.ajouter_choix_creation(ListeCreation::Variante, "Option A", "Façade").unwrap();
        let b = projet.ajouter_choix_creation(ListeCreation::Variante, "Option A", "Structure").unwrap();
        assert_ne!(a,b);
        let avant = projet.json().unwrap();
        for (liste, nom, jeu) in [(ListeCreation::Phase,"livraison",""), (ListeCreation::SousProjet," ",""), (ListeCreation::Variante,"option a","façade"), (ListeCreation::Variante,"Option B","")] {
            assert!(projet.ajouter_choix_creation(liste, nom, jeu).is_err());
            assert_eq!(projet.json().unwrap(), avant);
        }
        let relu = Projet::lire_json(&avant).unwrap();
        assert_eq!(relu.variantes.len(),2);
        assert_eq!(relu.phases, projet.phases);
        assert_eq!(relu.sous_projets, projet.sous_projets);
    }

    #[test]
    fn variante_mur_persiste_et_validee_sans_mutation() {
        let mut projet = Projet::nouveau("Variantes");
        projet.variantes = vec![
            Variante { id: "a".into(), nom: "Option 1".into(), jeu: "Façade".into() },
            Variante { id: "b".into(), nom: "Option 1".into(), jeu: "Structure".into() },
        ];
        let mut options = ParametresMur { variante: Some("b".into()), ..Default::default() };
        projet.ajouter_mur([0.,0.], [1000.,0.], &options).unwrap();
        options.variante = None;
        projet.ajouter_mur([0.,1000.], [1000.,1000.], &options).unwrap();
        let relu = Projet::lire_json(&projet.json().unwrap()).unwrap();
        assert_eq!(relu.murs[0].variante.as_deref(), Some("b"));
        assert_eq!(relu.murs[1].variante, None);
        assert_eq!(relu.variantes[1].libelle(), "Structure — Option 1");
        let avant = projet.json().unwrap();
        options.variante = Some("absente".into());
        assert!(projet.ajouter_mur([0.,0.], [1000.,0.], &options).is_err());
        assert_eq!(projet.json().unwrap(), avant);
    }

    #[test]
    fn dalle_creation_contour_et_relecture() {
        let mut p = Projet::nouveau("Dalle");
        let c = |m: &str, e: f64| Couche { materiau: Some(m.into()), epaisseur: e, ..Default::default() };
        let t = TypeCompose { id: String::new(), nom: "Dalle BA 230 + chape".into(), couches: vec![c("Chape ciment", 50.0), c("Béton armé", 230.0)], categorie: None, couches_detaillees: true, origine: None };
        let o = ParametresDalle { sorte: SorteDalle::Dalle, type_dalle: t, pente: None, niveau: None, decalage: 0.0, phase: None, sous_projet: None, variante: None };
        // contour en sens horaire, point de fermeture répété : réorienté et nettoyé
        let id = p.ajouter_dalle(&[[0.0, 0.0], [0.0, 4000.0], [5000.0, 4000.0], [5000.0, 0.0], [0.0, 0.0]], &o).unwrap();
        let d = p.dalles.iter().find(|d| d.id == id).unwrap();
        assert_eq!(d.contours[0].len(), 4);
        assert!((p.volume_dalle(d).unwrap() - 5000.0 * 4000.0 * 280.0).abs() < 1.0);
        // même type : pas de doublon ; contours invalides refusés sans rien changer
        p.ajouter_dalle(&[[6000.0, 0.0], [8000.0, 0.0], [8000.0, 2000.0]], &o).unwrap();
        assert_eq!(p.types_dalles.len(), 1);
        let avant = p.json().unwrap();
        for mauvais in [vec![[0.0, 0.0], [1000.0, 0.0]], vec![[0.0, 0.0], [1000.0, 1000.0], [1000.0, 0.0], [0.0, 1000.0]], vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]] {
            assert!(p.ajouter_dalle(&mauvais, &o).is_err());
            assert_eq!(p.json().unwrap(), avant);
        }
        let relu = Projet::lire_json(&avant).unwrap();
        assert_eq!(relu.dalles.len(), 2);
        let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("dalle.modele.json")).unwrap();
        assert_eq!(s.objects.len(), 2);
        let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start + o.opaque.count) as usize].chunks_exact(3).map(|t| {
            let p = [0, 1, 2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
            p[0].dot(p[1].cross(p[2])) / 6.0
        }).sum::<f64>()).sum();
        assert!((volume - (20.0 + 2.0) * 0.28).abs() < 1e-4, "Volume du maillage : {volume}");
    }

    #[test]
    fn plafond_et_toiture_en_pente() {
        let g = crate::config::gabarit::Gabarit::defaut();
        let mut p = Projet::depuis_gabarit("Toit", &g);
        assert_eq!(p.types_murs.len(), g.types_murs.len());
        assert_eq!(p.niveaux.len(), 1);
        let type_de = |sorte| {
            let t = g.surfaces(sorte)[0];
            TypeCompose { id: String::new(), nom: t.nom.clone(), couches: t.couches.clone(), categorie: None, couches_detaillees: true, origine: None }
        };
        let rect = [[0.0, 0.0], [10_000.0, 0.0], [10_000.0, 6_000.0], [0.0, 6_000.0]];
        let plafond = ParametresDalle { sorte: SorteDalle::Plafond, type_dalle: type_de(SorteDalle::Plafond), pente: None, niveau: None, decalage: 2500.0, phase: None, sous_projet: None, variante: None };
        let toit = ParametresDalle { sorte: SorteDalle::Toiture, type_dalle: type_de(SorteDalle::Toiture), pente: Some(35.0), decalage: 2800.0, ..plafond.clone() };
        p.ajouter_dalle(&rect, &plafond).unwrap();
        let id = p.ajouter_dalle(&rect, &toit).unwrap();
        assert!(p.ajouter_dalle(&rect, &ParametresDalle { pente: Some(90.0), ..toit.clone() }).is_err());
        let d = p.dalles.iter().find(|d| d.id == id).unwrap().clone();
        assert_eq!(d.sorte, SorteDalle::Toiture);
        assert_eq!((0..4).filter_map(|k| d.pente(0, k)).count(), 4);
        // épaisseur normale aux versants : volume = aire × e / cos 35°
        let e = p.type_dalle(&d.type_dalle).unwrap().epaisseur();
        let v = p.volume_dalle(&d).unwrap();
        assert!((v - 60e6 * e / 35f64.to_radians().cos()).abs() / v < 1e-9, "{v}");
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("toit.modele.json")).unwrap();
        assert_eq!(s.objects.len(), 2);
        let nav = s.navigateur.unwrap();
        assert!(nav.familles.iter().any(|f| f.categorie == "Plafonds") && nav.familles.iter().any(|f| f.categorie == "Toits"));
        // toiture en dessous du faîtage, sous-face à l'égout (2,80 m), plafond sous-face à 2,50 m
        let o = &s.objects[1];
        assert!((o.min.z as f64 + s.origin.z - 2.8).abs() < 1e-3, "{}", o.min.z as f64 + s.origin.z);
        let faitage = 2.8 + 3.0 * 35f64.to_radians().tan() + e / 1000.0 / 35f64.to_radians().cos();
        assert!((o.max.z as f64 + s.origin.z - faitage).abs() < 1e-3);
        assert!((s.objects[0].min.z as f64 + s.origin.z - 2.5).abs() < 1e-3);
    }

    #[test]
    fn vues_plan_et_coupe() {
        let mut p = Projet::nouveau("Vues");
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur::default()).unwrap();
        let n = p.niveaux[0].id.clone();
        p.ajouter_vue_plan(&n, 1200.0).unwrap();
        p.ajouter_vue_plan(&n, 1500.0).unwrap();
        p.ajouter_vue_coupe([0.0, -2000.0], [5000.0, -2000.0], 8000.0, false, "Coupe de bâtiment", 100.0).unwrap();
        let e = p.ajouter_vue_coupe([5000.0, 6000.0], [0.0, 6000.0], 20000.0, true, "Élévation extérieure", 100.0).unwrap();
        assert!(p.ajouter_vue_plan("absent", 1200.0).is_err());
        assert!(p.ajouter_vue_coupe([0.0, 0.0], [10.0, 0.0], 8000.0, false, "", 100.0).is_err());
        let noms: Vec<&str> = p.vues.iter().map(|v| v.nom.as_str()).collect();
        assert_eq!(noms, ["Niveau 0", "Niveau 0 (2)", "Coupe 1", "Élévation 1"]);
        assert!(p.renommer_vue(&e, "Coupe 1").is_err());
        p.renommer_vue(&e, "Façade sud").unwrap();
        p.retourner_vue(&e).unwrap();
        assert!(matches!(p.vues[3].genre, GenreVue::Coupe { debut, .. } if debut == [0.0, 6000.0]));
        p.retourner_vue(&e).unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.vues, p.vues);
        let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("vues.modele.json")).unwrap();
        let nav = s.navigateur.as_ref().unwrap();
        let genres: Vec<&str> = nav.vues.iter().map(|v| v.genre.as_str()).collect();
        assert_eq!(genres, ["Vue 3D", "Plan d'étage", "Plan d'étage", "Coupe", "Élévation"]);
        assert_eq!(nav.vues[0].nom, "Vue 3D");
        assert_eq!(nav.vues.iter().filter(|v| v.genre == "Vue 3D").count(), 1);
        // élévation tracée de droite à gauche au nord : elle regarde vers le sud (−y)
        assert!((nav.vues[4].direction - glam::DVec3::Y).length() < 1e-9, "{:?}", nav.vues[4].direction);
        // cadrage : profondeur et deux côtés
        assert_eq!(nav.vues[3].cadrage.len(), 3);
        let (p0, p1) = (glam::DVec3::new(2.5, 0.0, 0.0), glam::DVec3::new(2.5, 7.0, 0.0));
        let garde = |q: glam::DVec3| nav.vues[3].cadrage.iter().all(|pl| pl.truncate().dot(q) + pl.w >= 0.0);
        assert!(garde(p0) && !garde(p1), "au-delà de la profondeur de 8 m");
        p.supprimer_vue(&e).unwrap();
        assert_eq!(p.vues.len(), 3);
        // coupe : on regarde vers la gauche de la ligne (+y), l'observateur est côté −y
        let c = &nav.vues[3];
        assert!((c.direction - glam::DVec3::NEG_Y).length() < 1e-9, "{:?}", c.direction);
        assert!(c.plan_coupe.is_some());
    }

    #[test]
    fn mur_creation_enregistrement_relecture() {
        let mut p = Projet::nouveau("Essai");
        let id = p.ajouter_mur([0.,0.], [5000.,0.], &ParametresMur::default()).unwrap();
        p.ajouter_mur([5000.,0.], [5000.,3000.], &ParametresMur::default()).unwrap();
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(relu.murs[0].id, id);
        assert_eq!(relu.niveaux.len(), 1);
        assert_eq!(relu.types_murs.len(), 1);
        assert_eq!(relu.murs.len(), 2);
        assert!((relu.volume_mur(&relu.murs[0]).unwrap() - 4_200_000_000.0).abs() < 1.0);
        let s = crate::rendu::scene::depuis_natif(&relu, std::path::Path::new("essai.modele.json")).unwrap();
        assert_eq!(s.objects.len(), 2);
        // Le nombre de triangles dépend de la tessellation ; le volume fermé
        // ci-dessous est l’invariant géométrique utile après changement des identités.
        assert!(s.triangles >= 24);
        let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start+o.opaque.count) as usize].chunks_exact(3).map(|t| {
            let p = [0,1,2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
            p[0].dot(p[1].cross(p[2])) / 6.0
        }).sum::<f64>()).sum();
        assert!((volume - 6.72).abs() < 1e-5, "Volume du maillage : {volume}");
    }
    #[test]
    fn mur_implantations_et_erreurs_sans_mutation() {
        for (implantation, y) in [(Implantation::Axe,0.), (Implantation::Exterieure,-150.), (Implantation::Interieure,150.)] {
            let mut p = Projet::nouveau("Essai");
            let mut options = ParametresMur { implantation, ..Default::default() };
            p.ajouter_mur([0.,0.],[1000.,0.],&options).unwrap();
            assert!(matches!(p.murs[0].axe, Axe::Droit { debut, .. } if debut[1] == y));
            let avant = p.json().unwrap();
            assert!(p.ajouter_mur([0.,0.],[0.,0.],&options).is_err());
            options.hauteur = -1.;
            assert!(p.ajouter_mur([0.,0.],[1000.,0.],&options).is_err());
            assert_eq!(p.json().unwrap(), avant);
        }
    }
}
