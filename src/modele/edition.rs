//! Création paramétrique d'éléments natifs. Les coordonnées sont en millimètres.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Implantation { Axe, Exterieure, Interieure, AxePorteur }

impl Implantation {
    pub fn reference(self) -> super::assemblage::Reference {
        use super::assemblage::Reference;
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
}

impl Default for ParametresMur {
    fn default() -> Self {
        Self { hauteur: 2800.0, epaisseur: 300.0, implantation: Implantation::Axe, niveau: None, decalage: 0.0, niveau_haut: None, decalage_haut: 0.0, phase: None, sous_projet: None, variante: None, type_mur: None }
    }
}

impl ParametresMur {
    /// Type tracé : celui choisi, sinon une couche de l'épaisseur saisie.
    pub fn gabarit(&self, projet: &Projet) -> TypeCompose {
        self.type_mur.as_ref().and_then(|id| projet.type_mur(id)).cloned().unwrap_or_else(|| TypeCompose {
            id: String::new(),
            nom: String::new(),
            couches: vec![Couche { materiau: None, epaisseur: self.epaisseur, ..Default::default() }],
            couches_detaillees: true,
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

/// Paramètres de création d'une dalle (ou d'un sol) : type composé (couches du haut
/// vers le bas), niveau et altitude du dessus par rapport à lui.
#[derive(Clone)]
pub struct ParametresDalle {
    pub type_dalle: TypeCompose,
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

impl Projet {
    /// Identifiant libre pour une nouvelle vue et son nom (« Coupe 3 »).
    fn nouvelle_vue(&self, prefixe: &str) -> (String, usize) {
        let mut k = 1;
        loop {
            let id = identifiant("vue", &format!("{prefixe}-{k}"));
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
        self.vues.push(Vue { id: id.clone(), nom, type_vue: String::new(), echelle: 100.0, genre: GenreVue::Plan { niveau: n.id, coupe } });
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
        self.vues.push(Vue { id: id.clone(), nom: format!("{nom} {k}"), type_vue: type_vue.into(), echelle, genre: GenreVue::Coupe { debut, fin, profondeur, elevation } });
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
        if self.vues.len() == avant { Err("Vue introuvable.".into()) } else { Ok(()) }
    }

    /// Ajoute une dalle de contour `points` ; son type (couches du haut vers le bas) est
    /// repris s'il existe déjà (même nom et mêmes couches), sinon ajouté. Rien ne change
    /// en cas d'erreur.
    pub fn ajouter_dalle(&mut self, points: &[[f64; 2]], p: &ParametresDalle) -> Result<String, String> {
        let contour = contour_valide(points)?;
        if !p.decalage.is_finite() {
            return Err("Le décalage doit être fini.".into());
        }
        let t = &p.type_dalle;
        if t.couches.is_empty() || t.couches.iter().any(|c| !(c.epaisseur > 0.0 && c.epaisseur.is_finite())) {
            return Err("Le type de dalle doit avoir des couches d’épaisseur positive.".into());
        }
        let niveau = match &p.niveau {
            Some(id) => self.niveau(id).ok_or("Niveau introuvable.")?.id.clone(),
            None => self.niveaux.first().map(|n| n.id.clone()).unwrap_or_else(|| identifiant("niveau", "niveau-0")),
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
                    let id = identifiant("type-dalle", &format!("{}:{k}", t.nom));
                    if self.types_dalles.iter().all(|x| x.id != id) { break id; }
                    k += 1;
                };
                self.types_dalles.push(TypeCompose { id: id.clone(), nom: t.nom.clone(), couches: t.couches.clone(), couches_detaillees: true, origine: None });
                id
            }
        };
        let mut k = self.dalles.len() + 1;
        let id = loop {
            let id = identifiant("dalle", &format!("natif-{k}"));
            if self.dalles.iter().all(|d| d.id != id) { break id; }
            k += 1;
        };
        if self.phases.is_empty() { self.phases.push(p.phase.clone().unwrap_or_else(|| "Projet".into())); }
        if self.sous_projets.is_empty() { self.sous_projets.push(p.sous_projet.clone().unwrap_or_else(|| "Sous-projet 1".into())); }
        self.dalles.push(Dalle {
            variante: p.variante.clone(),
            phase: p.phase.clone().or_else(|| self.phases.last().cloned()),
            sous_projet: p.sous_projet.clone().or_else(|| self.sous_projets.first().cloned()),
            id: id.clone(),
            nom: format!("{} {k}", t.nom),
            type_dalle,
            niveau,
            decalage: p.decalage,
            contours: vec![contour],
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
            let id = identifiant("variante", &format!("{}:{jeu}:{nom}", jeu.len()));
            if self.variantes.iter().any(|v| v.id == id) { return Err("Identifiant de variante déjà utilisé.".into()); }
            self.variantes.push(Variante { id: id.clone(), nom: nom.into(), jeu: jeu.into() });
            return Ok(id);
        }
        let (choix, defaut) = match liste { ListeCreation::Phase => (&mut self.phases, "Projet"), _ => (&mut self.sous_projets, "Sous-projet 1") };
        if choix.iter().any(|v| v.eq_ignore_ascii_case(nom)) || (choix.is_empty() && defaut.eq_ignore_ascii_case(nom)) {
            return Err("Ce nom existe déjà.".into());
        }
        if choix.is_empty() { choix.push(defaut.into()); }
        choix.push(nom.into());
        Ok(nom.into())
    }

    /// Valide entièrement la commande avant d'ajouter le niveau, le type et le mur.
    pub fn ajouter_mur(&mut self, debut: [f64; 2], fin: [f64; 2], p: &ParametresMur) -> Result<String, String> {
        if !debut.iter().chain(&fin).all(|x| x.is_finite()) || ![p.hauteur, p.epaisseur, p.decalage].iter().all(|x| x.is_finite()) {
            return Err("Les coordonnées et dimensions du mur doivent être finies.".into());
        }
        let longueur = (fin[0] - debut[0]).hypot(fin[1] - debut[1]);
        if !longueur.is_finite() || longueur < 1.0 { return Err("Le mur doit avoir une longueur finie d’au moins 1 mm.".into()); }
        if p.hauteur <= 0.0 || p.epaisseur <= 0.0 { return Err("La hauteur et l’épaisseur doivent être positives.".into()); }
        let niveau = match &p.niveau {
            Some(id) => self.niveau(id).ok_or("Niveau de base introuvable.")?.id.clone(),
            None => self.niveaux.first().map(|n| n.id.clone()).unwrap_or_else(|| identifiant("niveau", "niveau-0")),
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
            loop { let id = identifiant("type-mur", &format!("natif-{k}")); if self.types_murs.iter().all(|t| t.id != id) { break id; } k += 1; }
        });
        let mut k = self.murs.len() + self.assemblages.len() + 1;
        let id = loop { let id = identifiant("mur", &format!("natif-{k}")); if self.murs.iter().all(|m| m.id != id) && self.assemblages.iter().all(|a| a.id != id) { break id; } k += 1; };
        // ligne tracée → milieu de l'épaisseur totale : décalage de la référence vers l'intérieur
        let gabarit = p.gabarit(self);
        let decalage = gabarit.decalage_reference(p.implantation.reference());
        let n = [-(fin[1] - debut[1]) / longueur, (fin[0] - debut[0]) / longueur];
        let deplacer = |q: [f64; 2]| [q[0] - n[0] * decalage, q[1] - n[1] * decalage];
        if self.niveaux.is_empty() {
            self.niveaux.push(Niveau { id: niveau.clone(), nom: "Niveau 0".into(), altitude: 0.0, origine: None });
        }
        if existant.is_none() {
            self.types_murs.push(TypeCompose { id: type_mur.clone(), nom: format!("Mur · {} mm", p.epaisseur), couches: vec![Couche { materiau: None, epaisseur: p.epaisseur, ..Default::default() }], couches_detaillees: true, origine: None });
        }
        let compose = compose.is_some_and(|t| t.couches.len() > 1);
        if self.phases.is_empty() { self.phases.push(p.phase.clone().unwrap_or_else(|| "Projet".into())); }
        if self.sous_projets.is_empty() { self.sous_projets.push(p.sous_projet.clone().unwrap_or_else(|| "Sous-projet 1".into())); }
        let contrainte = match &p.niveau_haut {
            Some(niveau) => Haut::Niveau { niveau: niveau.clone(), decalage: p.decalage_haut },
            None => Haut::Libre { hauteur: haut - bas },
        };
        self.murs.push(Mur { variante: p.variante.clone(), phase: p.phase.clone().or_else(|| self.phases.last().cloned()), sous_projet: p.sous_projet.clone().or_else(|| self.sous_projets.first().cloned()), id: id.clone(), nom: format!("Mur {k}"), type_mur, assemblage: None, niveau, decalage_bas: p.decalage,
            haut: contrainte, axe: Axe::Droit { debut: deplacer(debut), fin: deplacer(fin) }, origine: None });
        // type à plusieurs couches : un mur par couche, regroupés (même identifiant)
        if compose {
            self.eclater_mur(&id)?;
        }
        self.arrondir();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let scene = crate::scene::depuis_natif(&relu, std::path::Path::new("test.json")).unwrap();
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
        assert_eq!(projet.phases, ["Projet", "Livraison"]);
        projet.ajouter_choix_creation(ListeCreation::SousProjet, "Structure", "").unwrap();
        assert_eq!(projet.sous_projets, ["Sous-projet 1", "Structure"]);
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
        let t = TypeCompose { id: String::new(), nom: "Dalle BA 230 + chape".into(), couches: vec![c("Chape ciment", 50.0), c("Béton armé", 230.0)], couches_detaillees: true, origine: None };
        let o = ParametresDalle { type_dalle: t, niveau: None, decalage: 0.0, phase: None, sous_projet: None, variante: None };
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
        let s = crate::scene::depuis_natif(&relu, std::path::Path::new("dalle.modele.json")).unwrap();
        assert_eq!(s.objects.len(), 2);
        let volume: f64 = s.objects.iter().map(|o| s.vertices[o.opaque.start as usize..(o.opaque.start + o.opaque.count) as usize].chunks_exact(3).map(|t| {
            let p = [0, 1, 2].map(|k| glam::Vec3::from(t[k].pos).as_dvec3());
            p[0].dot(p[1].cross(p[2])) / 6.0
        }).sum::<f64>()).sum();
        assert!((volume - (20.0 + 2.0) * 0.28).abs() < 1e-4, "Volume du maillage : {volume}");
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
        let s = crate::scene::depuis_natif(&relu, std::path::Path::new("vues.modele.json")).unwrap();
        let nav = s.navigateur.as_ref().unwrap();
        let genres: Vec<&str> = nav.vues.iter().map(|v| v.genre.as_str()).collect();
        assert_eq!(genres, ["Vue 3D", "Plan d'étage", "Plan d'étage", "Coupe", "Élévation"]);
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
        let s = crate::scene::depuis_natif(&relu, std::path::Path::new("essai.modele.json")).unwrap();
        assert_eq!(s.objects.len(), 2);
        assert_eq!(s.triangles, 24);
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
