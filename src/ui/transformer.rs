//! Socle graphique des commandes de modification (voir docs/MODIFICATION.md) :
//! Déplacer, Copier, Tourner, Symétrie, Réseau, Échelle, Aligner, Décaler et Scinder se
//! font entièrement à la souris dans la vue. Chaque commande suit le même cycle :
//! consigne au curseur, survol de la cible, aperçu fantôme, cotes temporaires, valeur
//! tapée facultative (Entrée), une seule transaction à la validation, Échap ou clic
//! droit pour revenir d'une étape.
use super::*;
use crate::modele::edition::commandes::Commande;
use crate::modele::Axe;
use crate::ui::ruban::Outil;

/// Nombre d'éléments d'un réseau quand aucun n'est réglé.
pub(super) const NOMBRE_RESEAU: usize = 4;

/// État de la commande de modification en cours.
#[derive(Default)]
pub(super) struct Transformation {
    /// Points cliqués (mm, plan du projet), dans l'ordre des étapes de la commande.
    pub points: Vec<[f64; 2]>,
    /// Tourner, réseau polaire : centre ; absent, celui de la sélection.
    pub centre: Option<[f64; 2]>,
    /// Le prochain clic place le centre.
    pub placer_centre: bool,
    /// Aligner : élément de référence (mur ou mur composé).
    pub reference: Option<String>,
    /// Décaler : élément choisi.
    pub element: Option<String>,
    /// Déplacer, Tourner, Symétrie, Décaler : la transformation porte sur une copie.
    pub copie: bool,
    /// Copier : la commande reste sur le même point de base après chaque copie.
    pub multiple: bool,
    /// Réseau : nombre d'éléments, original compris (0 : valeur par défaut).
    pub nombre: usize,
    /// Réseau : polaire (autour d'un centre) plutôt que linéaire.
    pub polaire: bool,
    /// Scinder : écart laissé entre les deux parties (mm).
    pub ecart: f64,
    /// Valeur tapée au clavier (distance en unité d'affichage, angle en degrés, facteur).
    pub saisie: String,
    /// Dernière position du curseur dans le plan (mm), pour une valeur tapée.
    pub curseur: Option<[f64; 2]>,
    /// Outil auquel l'état se rapporte : un changement d'outil le remet à zéro.
    pub outil: Option<Outil>,
    /// Élément natif sous le curseur : surligné avant le clic.
    pub cible: Option<String>,
}

impl Transformation {
    /// Échap : revient d'une étape ; faux s'il n'y a plus rien à défaire (quitter l'outil).
    pub fn reculer(&mut self) -> bool {
        if !self.saisie.is_empty() {
            self.saisie.clear();
        } else if self.placer_centre {
            self.placer_centre = false;
        } else if self.points.pop().is_some() {
        } else if self.element.take().is_some() {
        } else if self.reference.take().is_some() {
        } else {
            return false;
        }
        true
    }
    /// Remet l'état à zéro en gardant les options.
    pub fn reinitialiser(&mut self) {
        let (copie, multiple, outil, nombre, polaire, ecart) = (self.copie, self.multiple, self.outil, self.nombre, self.polaire, self.ecart);
        *self = Self { copie, multiple, outil, nombre, polaire, ecart, ..Default::default() };
    }
    /// Nouvel outil : options par défaut de la commande (Symétrie et Décaler copient).
    pub fn commencer(&mut self, outil: Option<Outil>) {
        let (nombre, polaire, ecart) = (self.nombre, self.polaire, self.ecart);
        *self = Self { outil, nombre, polaire, ecart, copie: matches!(outil, Some(Outil::Symetrie | Outil::Decaler)), ..Default::default() };
    }
    pub fn nombre_reseau(&self) -> usize {
        if self.nombre >= 2 { self.nombre } else { NOMBRE_RESEAU }
    }
    fn polaire_avec_centre(&self, outil: Outil) -> bool {
        outil == Outil::Tourner || (outil == Outil::Reseau && self.polaire)
    }
    /// Valeur tapée attendue à l'étape courante.
    pub fn attend_valeur(&self, outil: Outil) -> bool {
        match outil {
            Outil::Deplacer | Outil::Copier => self.points.len() == 1,
            o if self.polaire_avec_centre(o) => !self.placer_centre,
            Outil::Reseau | Outil::Echelle => self.points.len() == 1,
            Outil::Decaler => self.element.is_some(),
            _ => false,
        }
    }
    /// Consigne de l'étape courante, affichée au curseur et sous la barre de commande.
    pub fn consigne(&self, outil: Outil) -> String {
        let valeur = if self.saisie.is_empty() { String::new() } else { format!(" · valeur : {} (Entrée)", self.saisie) };
        let n = self.nombre_reseau();
        match outil {
            Outil::Deplacer | Outil::Copier if self.points.is_empty() => "Cliquer le point de base".into(),
            Outil::Deplacer | Outil::Copier => format!("Cliquer la destination, ou taper la distance{valeur} · Maj : orthogonal"),
            o if self.polaire_avec_centre(o) && self.placer_centre => "Cliquer le nouveau centre".into(),
            o if self.polaire_avec_centre(o) && self.points.is_empty() => format!("Cliquer la direction de départ, ou taper l'angle{valeur} · clic sur le centre : le déplacer"),
            Outil::Tourner => format!("Cliquer la direction d'arrivée, ou taper l'angle{valeur} · Maj : pas de 15°"),
            Outil::Reseau if self.polaire => format!("Cliquer la direction du 2ᵉ élément, ou taper l'angle entre éléments{valeur} · {n} éléments"),
            Outil::Reseau if self.points.is_empty() => format!("Réseau linéaire de {n} éléments : cliquer le point de base"),
            Outil::Reseau => format!("Cliquer la position du 2ᵉ élément, ou taper le pas{valeur} · Maj : orthogonal"),
            Outil::Symetrie if self.points.is_empty() => "Cliquer le premier point de l'axe de symétrie".into(),
            Outil::Symetrie => "Cliquer le second point de l'axe · Maj : orthogonal".into(),
            Outil::Echelle if self.points.is_empty() => "Cliquer le point de base (fixe)".into(),
            Outil::Echelle if self.points.len() == 1 => format!("Cliquer la longueur de référence, ou taper le facteur{valeur}"),
            Outil::Echelle => "Cliquer la nouvelle longueur".into(),
            Outil::Aligner if self.reference.is_none() => "Cliquer la référence (axe d'un mur)".into(),
            Outil::Aligner => "Cliquer les éléments à aligner · Échap : changer de référence".into(),
            Outil::Decaler if self.element.is_none() => "Cliquer le mur ou la dalle à décaler".into(),
            Outil::Decaler => format!("Cliquer le côté et la distance, ou taper la distance{valeur}"),
            Outil::Scinder => "Cliquer le mur ou la poutre au point de coupure".into(),
            Outil::Demolir => "Cliquer les éléments à démolir (clic sur un élément démoli : annuler)".into(),
            Outil::Reproduire if self.element.is_none() => "Pipette : cliquer l'élément source".into(),
            Outil::Reproduire => "Pinceau : cliquer les éléments à mettre à jour · Échap : changer de source".into(),
            Outil::ChangerHote if self.element.is_none() => "Cliquer l'ouverture à déplacer".into(),
            Outil::ChangerHote => "Cliquer le nouveau mur hôte à l'emplacement voulu".into(),
            Outil::Inverser => "Cliquer un mur (sens) ou une ouverture (côté ; Maj : paumelles)".into(),
            _ => String::new(),
        }
    }
}

/// Angle signé (degrés, sens trigonométrique) de la direction centre→a à centre→b.
pub(super) fn angle_deg(centre: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (u, v) = ([a[0] - centre[0], a[1] - centre[1]], [b[0] - centre[0], b[1] - centre[1]]);
    (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]).to_degrees()
}

/// Angle arrondi au multiple de `pas` (degrés) le plus proche.
pub(super) fn accrocher_angle(angle: f64, pas: f64) -> f64 {
    (angle / pas).round() * pas
}

/// Point à `distance` de `base` dans la direction du curseur.
pub(super) fn point_a_distance(base: [f64; 2], curseur: [f64; 2], distance: f64) -> Option<[f64; 2]> {
    let (dx, dy) = (curseur[0] - base[0], curseur[1] - base[1]);
    let l = dx.hypot(dy);
    (l > 1e-9).then(|| [base[0] + dx / l * distance, base[1] + dy / l * distance])
}

/// Rotation d'un point autour de `centre` (degrés).
pub(super) fn tourner_point(p: [f64; 2], centre: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.to_radians().sin_cos();
    let (x, y) = (p[0] - centre[0], p[1] - centre[1]);
    [centre[0] + x * c - y * s, centre[1] + x * s + y * c]
}

/// Image de `p` par la symétrie d'axe (a, b) ; `p` si l'axe est nul.
pub(super) fn miroir_point(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = ux * ux + uy * uy;
    if l2 < 1e-18 {
        return p;
    }
    let t = ((p[0] - a[0]) * ux + (p[1] - a[1]) * uy) / l2;
    let h = [a[0] + ux * t, a[1] + uy * t];
    [2.0 * h[0] - p[0], 2.0 * h[1] - p[1]]
}

/// Distance signée de `p` à la droite (a, b) : positive à gauche du sens a→b.
pub(super) fn distance_signee(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
    let l = ux.hypot(uy).max(1e-12);
    (ux * (p[1] - a[1]) - uy * (p[0] - a[0])) / l
}

/// Valeur tapée : nombre décimal (virgule ou point).
pub(super) fn lire_valeur(saisie: &str) -> Option<f64> {
    saisie.replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

impl ViewerApp {
    /// Outil de modification graphique actif dans l'onglet Modifier, s'il peut s'exécuter.
    pub(super) fn outil_transformation(&self) -> Option<Outil> {
        if self.onglet_ruban != OngletRuban::Modifier || self.natif.is_none() || self.loading.is_some() {
            return None;
        }
        match self.ruban_edition.outil()? {
            o @ (Outil::Deplacer | Outil::Copier | Outil::Tourner | Outil::Symetrie | Outil::Reseau | Outil::Echelle) if !self.selection_bim().is_empty() => Some(o),
            o @ (Outil::Aligner | Outil::Decaler | Outil::Scinder | Outil::Demolir | Outil::Reproduire | Outil::ChangerHote | Outil::Inverser) => Some(o),
            _ => None,
        }
    }

    /// Synchronise l'état avec l'outil actif : un nouvel outil repart de zéro.
    pub(super) fn suivre_transformation(&mut self) {
        let outil = self.outil_transformation();
        if outil != self.transfo.outil {
            self.transfo.commencer(outil);
            if outil.is_some() {
                self.derniere_commande = outil;
            }
        }
    }

    /// Centre de la sélection (milieu de l'emprise des axes et contours), en mm.
    pub(super) fn centre_selection(&self) -> Option<[f64; 2]> {
        let (projet, _) = self.natif.as_ref()?;
        let ids = self.selection_bim();
        let mut pts: Vec<[f64; 2]> = Vec::new();
        for id in &ids {
            if let Some((a, b, _)) = projet.axe_element(id) {
                pts.extend([a, b]);
            }
        }
        for d in projet.dalles.iter().filter(|d| ids.contains(&d.id)) {
            pts.extend(d.contours.iter().flatten().copied());
        }
        for b in projet.barres.iter().filter(|b| ids.contains(&b.id)) {
            pts.extend([[b.debut[0], b.debut[1]], [b.fin[0], b.fin[1]]]);
        }
        for t in projet.terrains.iter().filter(|t| ids.contains(&t.id)) {
            pts.extend(t.contour.iter().map(|p| [p[0], p[1]]));
        }
        if pts.is_empty() {
            return None;
        }
        let (mut mn, mut mx) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in pts {
            for k in 0..2 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        Some([(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0])
    }

    fn centre_rotation(&self) -> Option<[f64; 2]> {
        self.transfo.centre.or_else(|| self.centre_selection())
    }

    /// Angle courant pour une direction d'arrivée `p` (Maj : pas de 15°).
    fn angle_tourner(&self, p: [f64; 2], maj: bool) -> Option<f64> {
        let c = self.centre_rotation()?;
        let depart = *self.transfo.points.first()?;
        let a = angle_deg(c, depart, p);
        Some(if maj { accrocher_angle(a, 15.0) } else { a })
    }

    /// Axe (justification) d'un mur ou mur composé.
    fn axe_mur(&self, id: &str) -> Option<([f64; 2], [f64; 2], f64)> {
        let (p, _) = self.natif.as_ref()?;
        p.axe_element(id)
    }

    /// Distance de décalage (mm) donnée par le curseur `p` pour l'élément `id` : à gauche
    /// du sens d'un mur droit, vers le centre d'un arc trigonométrique, vers l'extérieur
    /// d'un contour de dalle.
    fn distance_decalage(&self, id: &str, p: [f64; 2]) -> Option<f64> {
        let (projet, _) = self.natif.as_ref()?;
        if let Some(d) = projet.dalles.iter().find(|d| d.id == id) {
            let c = d.contours.first()?;
            let n = c.len();
            let dist = (0..n).map(|i| {
                let (a, b) = (c[i], c[(i + 1) % n]);
                let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = (ux * ux + uy * uy).max(1e-9);
                let t = (((p[0] - a[0]) * ux + (p[1] - a[1]) * uy) / l2).clamp(0.0, 1.0);
                (p[0] - a[0] - ux * t).hypot(p[1] - a[1] - uy * t)
            }).fold(f64::MAX, f64::min);
            let mut dedans = false;
            for i in 0..n {
                let (a, b) = (c[i], c[(i + n - 1) % n]);
                if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                    dedans = !dedans;
                }
            }
            return Some(if dedans { -dist } else { dist });
        }
        let couche = projet.assemblage(id).and_then(|a| a.couches.first().cloned()).unwrap_or(id.to_string());
        if let Some(Axe::Arc { debut, centre, direct, .. }) = projet.murs.iter().find(|m| m.id == couche).map(|m| m.axe.clone()) {
            let r = (debut[0] - centre[0]).hypot(debut[1] - centre[1]);
            let rp = (p[0] - centre[0]).hypot(p[1] - centre[1]);
            return Some(if direct { r - rp } else { rp - r });
        }
        let (a, b, _) = self.axe_mur(id)?;
        Some(distance_signee(p, a, b))
    }

    /// Élément décalable : mur (droit, courbe, composé) ou dalle.
    fn decalable(&self, id: &str) -> bool {
        self.axe_mur(id).is_some() || self.natif.as_ref().is_some_and(|(p, _)| p.dalles.iter().any(|d| d.id == id))
    }

    /// Exécute la transformation de la sélection en une seule transaction ; `copie` : sur
    /// une copie de la sélection (copie puis transformation, un seul Ctrl+Z).
    fn appliquer_transformation(&mut self, commande: impl FnOnce(Vec<String>) -> Commande, copie: bool) {
        let ids = self.selection_bim();
        let resultat = if copie {
            self.executer_bim_suite(Commande::Copier { ids, delta: [0.0; 3] }, commande)
        } else {
            self.executer_bim(commande(ids)).map(|_| ())
        };
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }

    /// Clic dans la vue pendant une commande. `cible` : élément natif sous le clic.
    /// Mur (ou mur composé) dont l'épaisseur contient `p` : cible d'un clic qui ne touche
    /// aucune face (bord du mur, vue en plan).
    pub(super) fn mur_proche(&self, p: [f64; 2]) -> Option<String> {
        let (projet, _) = self.natif.as_ref()?;
        let ids = projet.murs.iter().filter(|m| m.assemblage.is_none()).map(|m| m.id.clone()).chain(projet.assemblages.iter().map(|a| a.id.clone()));
        ids.filter_map(|id| {
            let (axe, epaisseur, _) = crate::modele::volumes::solides::hote(projet, &id).ok()?;
            let s = axe.abscisse(p);
            let (q, _) = crate::modele::volumes::solides::repere(&axe, s);
            let d = (q.x - p[0]).hypot(q.y - p[1]);
            (d <= epaisseur / 2.0 + 20.0).then_some((d, id))
        }).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
    }

    pub(super) fn clic_transformation(&mut self, outil: Outil, p: [f64; 2], ecran: egui::Pos2, rect: egui::Rect, altitude: f64, maj: bool, cible: Option<String>) {
        let cible = cible.or_else(|| self.mur_proche(p));
        // centre déplaçable (Tourner, réseau polaire) : clic dessus, puis nouveau centre
        if self.transfo.polaire_avec_centre(outil) {
            let pres_du_centre = self.centre_rotation().and_then(|c| self.ecran_plan(rect, c, altitude)).is_some_and(|c| c.distance(ecran) <= 10.0);
            if self.transfo.placer_centre {
                self.transfo.centre = Some(p);
                self.transfo.placer_centre = false;
                return;
            }
            if self.transfo.points.is_empty() && pres_du_centre {
                self.transfo.placer_centre = true;
                return;
            }
        }
        match outil {
            Outil::Deplacer | Outil::Copier => match self.transfo.points.first().copied() {
                None => self.transfo.points.push(p),
                Some(a) => self.valider_deplacement(outil, [p[0] - a[0], p[1] - a[1], 0.0]),
            },
            Outil::Tourner => {
                if self.transfo.points.is_empty() {
                    self.transfo.points.push(p);
                } else if let Some(angle) = self.angle_tourner(p, maj) {
                    self.valider_rotation(angle);
                }
            }
            Outil::Reseau if self.transfo.polaire => {
                if self.transfo.points.is_empty() {
                    self.transfo.points.push(p);
                } else if let Some(angle) = self.angle_tourner(p, maj) {
                    self.valider_reseau([0.0; 3], angle);
                }
            }
            Outil::Reseau => match self.transfo.points.first().copied() {
                None => self.transfo.points.push(p),
                Some(a) => self.valider_reseau([p[0] - a[0], p[1] - a[1], 0.0], 0.0),
            },
            Outil::Symetrie => match self.transfo.points.first().copied() {
                None => self.transfo.points.push(p),
                Some(a) if (p[0] - a[0]).hypot(p[1] - a[1]) < 1.0 => self.status = "Symétrie : le second point doit être distinct du premier.".into(),
                Some(a) => {
                    let copie = self.transfo.copie;
                    self.appliquer_transformation(|ids| Commande::Symetrie { ids, a, b: p }, copie);
                    self.fin_commande("Symétrie appliquée");
                }
            },
            Outil::Echelle => match self.transfo.points.len() {
                0 | 1 => self.transfo.points.push(p),
                _ => {
                    let (c, r) = (self.transfo.points[0], self.transfo.points[1]);
                    let l1 = (r[0] - c[0]).hypot(r[1] - c[1]);
                    let l2 = (p[0] - c[0]).hypot(p[1] - c[1]);
                    if l1 < 1.0 {
                        self.status = "Échelle : longueur de référence nulle.".into();
                        self.transfo.points.truncate(1);
                    } else {
                        self.valider_echelle(l2 / l1);
                    }
                }
            },
            Outil::Aligner => {
                let Some(id) = cible else {
                    self.status = "Aligner : cliquer sur un élément.".into();
                    return;
                };
                match self.transfo.reference.clone() {
                    None if self.axe_mur(&id).is_some() => self.transfo.reference = Some(id),
                    None => self.status = "Aligner : la référence doit être un mur droit.".into(),
                    Some(r) if r == id => self.status = "Aligner : choisir un autre élément que la référence.".into(),
                    Some(r) => match self.executer_bim(Commande::Aligner { ids: vec![id], reference: r }) {
                        Ok(_) => self.status = "Élément aligné — cliquer le suivant, Ctrl+Z pour annuler.".into(),
                        Err(e) => self.error = Some(e),
                    },
                }
            }
            Outil::Decaler => match self.transfo.element.clone() {
                None => match cible.filter(|id| self.decalable(id)) {
                    Some(id) => self.transfo.element = Some(id),
                    None => self.status = "Décaler : cliquer un mur ou une dalle.".into(),
                },
                Some(id) => {
                    if let Some(d) = self.distance_decalage(&id, p) {
                        self.valider_decalage(&id, d);
                    }
                }
            },
            Outil::Demolir => match cible {
                Some(id) => {
                    let (deja, phase) = self.natif.as_ref().map_or((false, None), |(p, _)| (p.demolitions.contains_key(&id), p.phases.last().cloned()));
                    let phase = if deja { None } else { phase };
                    match self.executer_bim(Commande::Demolir { ids: vec![id], phase }) {
                        Ok(_) => self.status = if deja { "Démolition annulée — Ctrl+Z pour annuler.".into() } else { "Élément démoli — cliquer le suivant, Ctrl+Z pour annuler.".into() },
                        Err(e) => self.error = Some(e),
                    }
                    self.bim.selection.clear();
                }
                None => self.status = "Démolir : cliquer sur un élément.".into(),
            },
            Outil::Reproduire => match (self.transfo.element.clone(), cible) {
                (_, None) => self.status = "Reproduire : cliquer sur un élément.".into(),
                (None, Some(id)) => self.transfo.element = Some(id),
                (Some(source), Some(id)) if source == id => {}
                (Some(source), Some(id)) => match self.executer_bim(Commande::Reproduire { source, cibles: vec![id] }) {
                    Ok(_) => {
                        self.bim.selection.clear();
                        self.status = "Propriétés reproduites — cliquer la cible suivante, Ctrl+Z pour annuler.".into();
                    }
                    Err(e) => self.error = Some(e),
                },
            },
            Outil::ChangerHote => {
                let ouverture = |app: &Self, id: &String| app.natif.as_ref().is_some_and(|(p, _)| p.ouvertures.iter().any(|o| &o.id == id));
                // un clic sur le mur à l'endroit d'une ouverture choisit cette ouverture
                // (le clic peut aussi tomber dans le vide de la baie : aucun objet touché)
                let cible = match (&self.transfo.element, cible) {
                    (None, c) if c.as_ref().is_none_or(|id| !ouverture(self, id)) => self.natif.as_ref().and_then(|(projet, _)| {
                        projet.ouvertures.iter().find(|o| {
                            crate::modele::volumes::solides::hote(projet, &o.mur).is_ok_and(|(axe, epaisseur, _)| {
                                let s = axe.abscisse(p);
                                let (q, _) = crate::modele::volumes::solides::repere(&axe, s);
                                (s - o.abscisse).abs() <= o.largeur / 2.0 && (q.x - p[0]).hypot(q.y - p[1]) <= epaisseur / 2.0 + 100.0
                            })
                        }).map(|o| o.id.clone())
                    }).or(c),
                    (_, c) => c,
                };
                match (self.transfo.element.clone(), cible) {
                    (None, Some(id)) if ouverture(self, &id) => self.transfo.element = Some(id),
                    (None, _) => self.status = "Changer d'hôte : cliquer une porte ou une fenêtre.".into(),
                    (Some(id), Some(mur)) if self.axe_mur(&mur).is_some() => match self.executer_bim(Commande::ChangerHote { id, mur, point: p }) {
                        Ok(_) => self.fin_commande("Ouverture placée sur le nouveau mur"),
                        Err(e) => self.error = Some(e),
                    },
                    (Some(_), _) => self.status = "Changer d'hôte : cliquer un mur.".into(),
                }
            }
            Outil::Inverser => {
                let Some(id) = cible else {
                    self.status = "Inverser : cliquer un mur ou une ouverture.".into();
                    return;
                };
                let ouverture = self.natif.as_ref().is_some_and(|(p, _)| p.ouvertures.iter().any(|o| o.id == id));
                let c = if ouverture { Commande::InverserOuverture { id, paumelles: maj } } else { Commande::InverserMur { id } };
                match self.executer_bim(c) {
                    Ok(_) => self.status = "Inversé — Ctrl+Z pour annuler.".into(),
                    Err(e) => self.error = Some(e),
                }
            }
            Outil::Scinder => match cible.filter(|id| self.axe_mur(id).is_some() || self.natif.as_ref().is_some_and(|(p, _)| p.barres.iter().any(|b| &b.id == id && b.genre == crate::modele::GenreBarre::Poutre))) {
                Some(id) => {
                    let ecart = self.transfo.ecart.max(0.0);
                    match self.executer_bim(Commande::Scinder { id, point: p, ecart }) {
                        Ok(_) => self.status = "Mur scindé — cliquer un autre point, Ctrl+Z pour annuler.".into(),
                        Err(e) => self.error = Some(e),
                    }
                }
                None => self.status = "Scinder : cliquer sur un mur droit ou une poutre.".into(),
            },
            _ => {}
        }
    }

    fn fin_commande(&mut self, message: &str) {
        self.transfo.reinitialiser();
        if self.error.is_none() {
            self.status = format!("{message} — Ctrl+Z pour annuler.");
        }
    }

    fn valider_deplacement(&mut self, outil: Outil, delta: [f64; 3]) {
        let copier = outil == Outil::Copier || self.transfo.copie;
        let ids = self.selection_bim();
        let originaux = ids.clone();
        let commande = if copier { Commande::Copier { ids, delta } } else { Commande::Deplacer { ids, delta } };
        match self.executer_bim(commande) {
            Ok(nouveaux) => {
                if outil == Outil::Copier && self.transfo.multiple {
                    // la sélection reste l'original : copies successives depuis la même base
                    let base = self.transfo.points[0];
                    self.bim.selection = originaux;
                    self.transfo.reinitialiser();
                    self.transfo.points.push(base);
                    self.status = format!("{} copie(s) créée(s) — cliquer la destination suivante, Échap pour terminer.", nouveaux.len());
                } else {
                    self.transfo.reinitialiser();
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn valider_rotation(&mut self, angle: f64) {
        let Some(centre) = self.centre_rotation() else { return };
        let copie = self.transfo.copie;
        self.appliquer_transformation(|ids| Commande::Tourner { ids, centre, angle }, copie);
        self.fin_commande(&format!("Rotation de {angle:.1}° appliquée"));
    }

    fn valider_reseau(&mut self, delta: [f64; 3], angle: f64) {
        let centre = self.centre_rotation().unwrap_or([0.0; 2]);
        let nombre = self.transfo.nombre_reseau();
        let ids = self.selection_bim();
        match self.executer_bim(Commande::Reseau { ids: ids.clone(), nombre, delta, centre, angle }) {
            Ok(_) => {
                self.bim.selection = ids;
                self.fin_commande(&format!("Réseau de {nombre} éléments créé"));
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn valider_echelle(&mut self, facteur: f64) {
        let Some(centre) = self.transfo.points.first().copied() else { return };
        self.appliquer_transformation(|ids| Commande::Echelle { ids, centre, facteur }, false);
        self.fin_commande(&format!("Échelle {facteur:.3} appliquée"));
    }

    fn valider_decalage(&mut self, id: &str, distance: f64) {
        let copie = self.transfo.copie;
        match self.executer_bim(Commande::Decaler { id: id.to_string(), distance, copie }) {
            Ok(_) => self.fin_commande(&format!("Décalage de {} appliqué", crate::config::options::unite().afficher(distance.abs()))),
            Err(e) => self.error = Some(e),
        }
    }

    /// Entrée sans commande active : relance la dernière commande de modification.
    pub(super) fn repeter_commande(&mut self, ctx: &egui::Context) {
        if self.ruban_edition.outil().is_some() || self.saisie_active || ctx.egui_wants_keyboard_input() || self.parametres.is_some() {
            return;
        }
        let Some(o) = self.derniere_commande else { return };
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            self.relancer_commande(o);
        }
    }

    pub(super) fn relancer_commande(&mut self, o: Outil) {
        self.onglet_ruban = OngletRuban::Modifier;
        self.ruban_edition.activer_outil(o);
        self.status = format!("{} : commande relancée.", o.nom());
    }

    /// Clavier : chiffres, virgule, signe moins, Retour arrière et Entrée pour une valeur
    /// tapée. Lu avant les raccourcis, uniquement quand l'étape attend une valeur.
    pub(super) fn clavier_transformation(&mut self, ctx: &egui::Context) {
        self.suivre_transformation();
        let Some(outil) = self.transfo.outil else { return };
        if !self.transfo.attend_valeur(outil) || self.saisie_active {
            return;
        }
        let mut entree = false;
        ctx.input_mut(|i| {
            i.events.retain(|e| match e {
                egui::Event::Text(t) if t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')) => {
                    self.transfo.saisie.push_str(t);
                    false
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } if !self.transfo.saisie.is_empty() => {
                    self.transfo.saisie.pop();
                    false
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } if !self.transfo.saisie.is_empty() => {
                    entree = true;
                    false
                }
                _ => true,
            })
        });
        if !entree {
            return;
        }
        let Some(v) = lire_valeur(&self.transfo.saisie) else {
            self.status = format!("Valeur illisible : {}", self.transfo.saisie);
            self.transfo.saisie.clear();
            return;
        };
        self.transfo.saisie.clear();
        let unite = crate::config::options::unite();
        let vers_curseur = |app: &Self, d: f64| {
            let base = app.transfo.points.first().copied()?;
            let p = point_a_distance(base, app.transfo.curseur?, d)?;
            Some([p[0] - base[0], p[1] - base[1], 0.0])
        };
        match outil {
            Outil::Deplacer | Outil::Copier => {
                if let Some(delta) = vers_curseur(self, unite.vers_mm(v)) {
                    self.valider_deplacement(outil, delta);
                }
            }
            Outil::Tourner => self.valider_rotation(v),
            Outil::Reseau if self.transfo.polaire => self.valider_reseau([0.0; 3], v),
            Outil::Reseau => {
                if let Some(delta) = vers_curseur(self, unite.vers_mm(v)) {
                    self.valider_reseau(delta, 0.0);
                }
            }
            Outil::Echelle if v > 0.0 => self.valider_echelle(v),
            Outil::Echelle => self.status = "Échelle : le facteur doit être positif.".into(),
            Outil::Decaler => {
                let (Some(id), Some(c)) = (self.transfo.element.clone(), self.transfo.curseur) else { return };
                // côté du curseur, distance tapée
                let cote = if self.distance_decalage(&id, c).unwrap_or(1.0) < 0.0 { -1.0 } else { 1.0 };
                self.valider_decalage(&id, cote * unite.vers_mm(v.abs()));
            }
            _ => {}
        }
    }

    /// Aperçu de la commande : consigne au curseur, accrochage, cotes temporaires,
    /// fantôme de la sélection transformée, centre, axe, cible survolée.
    pub(super) fn apercu_transformation(&mut self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>, altitude: f64) {
        let Some(outil) = self.transfo.outil else { return };
        let cible = self.transfo.cible.clone();
        let painter = ui.painter().with_clip_rect(rect);
        let bleu = Color32::from_rgb(55, 130, 200);
        let orange = Color32::from_rgb(230, 120, 20);
        let maj = ui.input(|i| i.modifiers.shift);
        let unite = crate::config::options::unite();
        let orthogonal = maj && matches!(outil, Outil::Deplacer | Outil::Copier | Outil::Symetrie) || maj && outil == Outil::Reseau && !self.transfo.polaire;
        let point = survol.and_then(|q| {
            let depart = match outil {
                Outil::Deplacer | Outil::Copier | Outil::Symetrie | Outil::Echelle => self.transfo.points.first().copied(),
                Outil::Reseau if !self.transfo.polaire => self.transfo.points.first().copied(),
                _ => None,
            };
            self.point_tracage(q, rect, orthogonal, depart, altitude)
        });
        self.transfo.curseur = point.map(|(p, _)| p);
        if let (Some(q), Some((p, acc))) = (survol, point) {
            if let Some(e) = self.ecran_plan(rect, p, altitude) {
                marqueur(&painter, e, acc, bleu, orange);
            }
            painter.text(q + egui::vec2(16.0, 14.0), egui::Align2::LEFT_TOP, self.transfo.consigne(outil), egui::FontId::proportional(12.0), bleu);
        }
        let fantome = |app: &Self, f: &dyn Fn([f64; 2]) -> [f64; 2]| {
            let Some((projet, _)) = &app.natif else { return };
            let ids = app.selection_bim();
            for id in &ids {
                let (Some((_, _, z)), Some((s, e, _))) = (projet.axe_element(id), projet.ligne_justification(id)) else { continue };
                if let (Some(x), Some(y)) = (app.ecran_plan(rect, f(s), z), app.ecran_plan(rect, f(e), z)) {
                    painter.line_segment([x, y], egui::Stroke::new(2.5, bleu.gamma_multiply(0.7)));
                }
            }
            for dalle in projet.dalles.iter().filter(|x| ids.contains(&x.id)) {
                let z = projet.niveau(&dalle.niveau).map_or(0.0, |n| n.altitude) + dalle.decalage;
                for c in &dalle.contours {
                    let pts: Vec<egui::Pos2> = c.iter().chain(c.first()).filter_map(|v| app.ecran_plan(rect, f(*v), z)).collect();
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, bleu.gamma_multiply(0.7))));
                }
            }
        };
        let cote = |a: [f64; 2], b: [f64; 2], texte: String| {
            if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, altitude), self.ecran_plan(rect, b, altitude)) {
                painter.add(egui::Shape::dashed_line(&[x, y], egui::Stroke::new(1.5, bleu), 6.0, 4.0));
                painter.circle_filled(x, 3.5, bleu);
                painter.text(x.lerp(y, 0.5) + egui::vec2(0.0, -16.0), egui::Align2::CENTER_CENTER, texte, egui::FontId::proportional(13.0), bleu);
            }
        };
        let trait_axe = |id: &str, couleur: Color32, epaisseur: f32| {
            if let Some((a, b, z)) = self.axe_mur(id) {
                if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) {
                    painter.add(egui::Shape::dashed_line(&[x, y], egui::Stroke::new(epaisseur, couleur), 8.0, 4.0));
                }
            }
        };
        // centre (Tourner, réseau polaire) : croix cerclée, orange au survol (clic : le déplacer)
        let dessiner_centre = |app: &Self| -> Option<egui::Pos2> {
            let c = app.centre_rotation()?;
            let ce = app.ecran_plan(rect, c, altitude)?;
            let proche = survol.is_some_and(|q| q.distance(ce) <= 10.0) || app.transfo.placer_centre;
            let couleur = if proche { orange } else { bleu };
            painter.circle_stroke(ce, 7.0, egui::Stroke::new(1.8, couleur));
            painter.line_segment([ce - egui::vec2(10.0, 0.0), ce + egui::vec2(10.0, 0.0)], egui::Stroke::new(1.2, couleur));
            painter.line_segment([ce - egui::vec2(0.0, 10.0), ce + egui::vec2(0.0, 10.0)], egui::Stroke::new(1.2, couleur));
            Some(ce)
        };
        // rapporteur : arc et valeur de l'angle à l'écran
        let rapporteur = |ce: egui::Pos2, de: egui::Pos2, ae: egui::Pos2, angle: f64| {
            painter.line_segment([ce, de], egui::Stroke::new(1.2, bleu));
            painter.line_segment([ce, ae], egui::Stroke::new(1.8, orange));
            let r = ce.distance(de).clamp(30.0, 90.0);
            let a0 = (de - ce).angle();
            let mut balayage = (ae - ce).angle() - a0;
            while balayage > std::f32::consts::PI { balayage -= std::f32::consts::TAU; }
            while balayage < -std::f32::consts::PI { balayage += std::f32::consts::TAU; }
            let pts: Vec<egui::Pos2> = (0..=24).map(|k| ce + egui::Vec2::angled(a0 + balayage * k as f32 / 24.0) * r).collect();
            painter.add(egui::Shape::line(pts, egui::Stroke::new(1.5, orange)));
            painter.text(ce + egui::Vec2::angled(a0 + balayage / 2.0) * (r + 16.0), egui::Align2::CENTER_CENTER, format!("{angle:.1}°"), egui::FontId::proportional(13.0), orange);
        };
        match outil {
            Outil::Deplacer | Outil::Copier => {
                let (Some(a), Some((p, _))) = (self.transfo.points.first().copied(), point) else { return };
                let p = lire_valeur(&self.transfo.saisie).and_then(|v| point_a_distance(a, p, unite.vers_mm(v))).unwrap_or(p);
                let d = [p[0] - a[0], p[1] - a[1]];
                cote(a, p, unite.afficher(d[0].hypot(d[1])));
                fantome(self, &|v| [v[0] + d[0], v[1] + d[1]]);
            }
            o if self.transfo.polaire_avec_centre(o) => {
                let Some(ce) = dessiner_centre(self) else { return };
                if self.transfo.placer_centre {
                    return;
                }
                let Some(c) = self.centre_rotation() else { return };
                let Some(depart) = self.transfo.points.first().copied() else {
                    if let Some(e) = point.and_then(|(p, _)| self.ecran_plan(rect, p, altitude)) {
                        painter.add(egui::Shape::dashed_line(&[ce, e], egui::Stroke::new(1.2, bleu), 5.0, 4.0));
                    }
                    return;
                };
                let angle = lire_valeur(&self.transfo.saisie).or_else(|| point.and_then(|(p, _)| self.angle_tourner(p, maj)));
                let Some(angle) = angle else { return };
                let (Some(de), Some(ae)) = (self.ecran_plan(rect, depart, altitude), self.ecran_plan(rect, tourner_point(depart, c, angle), altitude)) else { return };
                rapporteur(ce, de, ae, angle);
                if o == Outil::Tourner {
                    fantome(self, &|v| tourner_point(v, c, angle));
                } else {
                    for k in 1..self.transfo.nombre_reseau() {
                        let a = angle * k as f64;
                        fantome(self, &|v| tourner_point(v, c, a));
                    }
                }
            }
            Outil::Reseau => {
                let (Some(a), Some((p, _))) = (self.transfo.points.first().copied(), point) else { return };
                let p = lire_valeur(&self.transfo.saisie).and_then(|v| point_a_distance(a, p, unite.vers_mm(v))).unwrap_or(p);
                let d = [p[0] - a[0], p[1] - a[1]];
                cote(a, p, format!("pas {}", unite.afficher(d[0].hypot(d[1]))));
                for k in 1..self.transfo.nombre_reseau() {
                    let k = k as f64;
                    fantome(self, &|v| [v[0] + d[0] * k, v[1] + d[1] * k]);
                }
            }
            Outil::Symetrie => {
                let (Some(a), Some((p, _))) = (self.transfo.points.first().copied(), point) else { return };
                if let (Some(x), Some(y)) = (self.ecran_plan(rect, a, altitude), self.ecran_plan(rect, p, altitude)) {
                    // axe prolongé en tirets
                    let v = (y - x).normalized() * 4000.0;
                    painter.add(egui::Shape::dashed_line(&[x - v, y + v], egui::Stroke::new(1.5, orange), 10.0, 5.0));
                }
                if (p[0] - a[0]).hypot(p[1] - a[1]) > 1.0 {
                    fantome(self, &|v| miroir_point(v, a, p));
                }
            }
            Outil::Echelle => {
                let Some(c) = self.transfo.points.first().copied() else { return };
                let Some((p, _)) = point else { return };
                match self.transfo.points.get(1).copied() {
                    None => {
                        if let Some(f) = lire_valeur(&self.transfo.saisie).filter(|f| *f > 0.0) {
                            fantome(self, &|v| [c[0] + (v[0] - c[0]) * f, c[1] + (v[1] - c[1]) * f]);
                        }
                        cote(c, p, format!("référence {}", unite.afficher((p[0] - c[0]).hypot(p[1] - c[1]))));
                    }
                    Some(r) => {
                        let l1 = (r[0] - c[0]).hypot(r[1] - c[1]).max(1e-9);
                        let f = (p[0] - c[0]).hypot(p[1] - c[1]) / l1;
                        cote(c, p, format!("facteur {f:.3}"));
                        fantome(self, &|v| [c[0] + (v[0] - c[0]) * f, c[1] + (v[1] - c[1]) * f]);
                    }
                }
            }
            Outil::Aligner => {
                if let Some(r) = &self.transfo.reference {
                    trait_axe(r, bleu, 2.0);
                }
                if let Some(id) = cible.filter(|id| Some(id) != self.transfo.reference.as_ref()) {
                    trait_axe(&id, if self.transfo.reference.is_some() { orange } else { bleu }, 3.0);
                }
            }
            Outil::Decaler => match self.transfo.element.clone() {
                None => {
                    if let Some(id) = cible {
                        trait_axe(&id, orange, 3.0);
                    }
                }
                Some(id) => {
                    let Some((p, _)) = point else { return };
                    let Some(mut d) = self.distance_decalage(&id, p) else { return };
                    if let Some(v) = lire_valeur(&self.transfo.saisie) {
                        d = d.signum() * unite.vers_mm(v.abs());
                    }
                    // dalle : contour décalé en fantôme
                    if let Some((projet, _)) = &self.natif {
                        if let Some(dalle) = projet.dalles.iter().find(|x| x.id == id) {
                            let z = projet.niveau(&dalle.niveau).map_or(0.0, |n| n.altitude) + dalle.decalage;
                            if let Some(g) = dalle.contours.first().and_then(|c| crate::modele::edition::commandes::decaler_polygone(c, d)) {
                                let pts: Vec<egui::Pos2> = g.iter().chain(g.first()).filter_map(|v| self.ecran_plan(rect, *v, z)).collect();
                                painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0, bleu.gamma_multiply(0.8))));
                            }
                            if let Some(q) = survol {
                                painter.text(q + egui::vec2(16.0, -18.0), egui::Align2::LEFT_BOTTOM, unite.afficher(d.abs()), egui::FontId::proportional(13.0), bleu);
                            }
                            return;
                        }
                    }
                    let Some((a, b, z)) = self.axe_mur(&id) else { return };
                    trait_axe(&id, bleu, 2.0);
                    let _ = distance_signee(p, a, b);
                    if let Some(v) = lire_valeur(&self.transfo.saisie) {
                        d = d.signum() * unite.vers_mm(v.abs());
                    }
                    let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
                    let l = ux.hypot(uy).max(1e-9);
                    let n = [-uy / l * d, ux / l * d];
                    let (a2, b2) = ([a[0] + n[0], a[1] + n[1]], [b[0] + n[0], b[1] + n[1]]);
                    if let (Some(x), Some(y)) = (self.ecran_plan(rect, a2, z), self.ecran_plan(rect, b2, z)) {
                        painter.line_segment([x, y], egui::Stroke::new(2.5, bleu.gamma_multiply(0.7)));
                    }
                    let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                    cote(m, [m[0] + n[0], m[1] + n[1]], unite.afficher(d.abs()));
                }
            },
            Outil::Demolir | Outil::Reproduire | Outil::ChangerHote | Outil::Inverser => {
                if let Some(source) = self.transfo.element.as_ref() {
                    trait_axe(source, bleu, 2.0);
                }
                if let Some(id) = cible {
                    trait_axe(&id, orange, 3.0);
                    if let (Some(q), Some((projet, _))) = (survol, &self.natif) {
                        let nom = projet.murs.iter().find(|m| m.id == id).map(|m| m.nom.clone())
                            .or_else(|| projet.assemblage(&id).map(|a| a.nom.clone()))
                            .or_else(|| projet.ouvertures.iter().find(|o| o.id == id).map(|o| o.nom.clone()))
                            .or_else(|| projet.dalles.iter().find(|d| d.id == id).map(|d| d.nom.clone()))
                            .unwrap_or_default();
                        let etat = projet.demolitions.get(&id).map(|ph| format!(" · démoli ({ph})")).unwrap_or_default();
                        painter.text(q + egui::vec2(16.0, -18.0), egui::Align2::LEFT_BOTTOM, format!("{nom}{etat}"), egui::FontId::proportional(12.0), orange);
                    }
                }
            }
            Outil::Scinder => {
                let Some(id) = cible else { return };
                trait_axe(&id, orange, 3.0);
                let (Some((a, b, z)), Some((p, _))) = (self.axe_mur(&id), point) else { return };
                // marque de coupure : projection du curseur sur l'axe, cotes vers les extrémités
                let (ux, uy) = (b[0] - a[0], b[1] - a[1]);
                let l2 = (ux * ux + uy * uy).max(1e-9);
                let t = (((p[0] - a[0]) * ux + (p[1] - a[1]) * uy) / l2).clamp(0.0, 1.0);
                let q = [a[0] + ux * t, a[1] + uy * t];
                if let Some(e) = self.ecran_plan(rect, q, z) {
                    let n = egui::vec2(-(uy as f32), ux as f32).normalized() * 14.0;
                    let n = egui::vec2(n.x, -n.y);
                    painter.line_segment([e - n, e + n], egui::Stroke::new(2.5, orange));
                }
                let l = l2.sqrt();
                cote(a, q, unite.afficher(l * t));
                cote(q, b, unite.afficher(l * (1.0 - t)));
            }
            _ => {}
        }
    }

    /// État de la commande pour le pilote : étape, consigne, centre à l'écran.
    pub(super) fn etat_transformation(&self) -> serde_json::Value {
        let Some(outil) = self.transfo.outil else { return serde_json::Value::Null };
        let rect = self.rect_vue;
        let altitude = self.altitude_vue_native();
        let centre = self.transfo.polaire_avec_centre(outil).then(|| self.centre_rotation()).flatten();
        serde_json::json!({
            "outil": outil.nom(),
            "consigne": self.transfo.consigne(outil),
            "points": self.transfo.points,
            "centre": centre,
            "centre_ecran": centre.and_then(|c| self.ecran_plan(rect, c, altitude)).map(|e| [e.x, e.y]),
            "placer_centre": self.transfo.placer_centre,
            "reference": self.transfo.reference,
            "element": self.transfo.element,
            "copie": self.transfo.copie,
            "multiple": self.transfo.multiple,
            "nombre": self.transfo.nombre_reseau(),
            "polaire": self.transfo.polaire,
            "ecart": self.transfo.ecart,
            "saisie": self.transfo.saisie,
            "derniere": self.derniere_commande.map(|o| o.nom()),
            "cible": self.transfo.cible,
        })
    }

    /// Réglage d'une option de la commande en cours par le pilote (comme la barre d'options).
    pub(super) fn regler_transformation(&mut self, v: &serde_json::Value) -> Result<(), String> {
        self.suivre_transformation();
        let o = v.as_object().ok_or("options : objet JSON attendu")?;
        for (cle, val) in o {
            match cle.as_str() {
                "copie" => self.transfo.copie = val.as_bool().ok_or("copie : booléen")?,
                "multiple" => self.transfo.multiple = val.as_bool().ok_or("multiple : booléen")?,
                "polaire" => self.transfo.polaire = val.as_bool().ok_or("polaire : booléen")?,
                "nombre" => self.transfo.nombre = val.as_u64().filter(|n| (2..=500).contains(n)).ok_or("nombre : de 2 à 500")? as usize,
                "ecart" => self.transfo.ecart = val.as_f64().filter(|e| *e >= 0.0).ok_or("ecart : mm positif")?,
                _ => return Err(format!("Option inconnue : {cle}")),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_signes_et_accrochage() {
        assert!((angle_deg([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]) - 90.0).abs() < 1e-9);
        assert!((angle_deg([0.0, 0.0], [1.0, 0.0], [0.0, -1.0]) + 90.0).abs() < 1e-9);
        assert!((angle_deg([5.0, 5.0], [6.0, 5.0], [4.0, 5.0]).abs() - 180.0).abs() < 1e-9);
        assert_eq!(accrocher_angle(37.0, 15.0), 30.0);
        assert_eq!(accrocher_angle(-52.0, 15.0), -45.0);
    }

    #[test]
    fn distance_tapee_rotation_miroir_et_cote() {
        let p = point_a_distance([0.0, 0.0], [3.0, 4.0], 10.0).unwrap();
        assert!((p[0] - 6.0).abs() < 1e-9 && (p[1] - 8.0).abs() < 1e-9);
        assert!(point_a_distance([1.0, 1.0], [1.0, 1.0], 5.0).is_none());
        let q = tourner_point([2.0, 0.0], [1.0, 0.0], 90.0);
        assert!((q[0] - 1.0).abs() < 1e-9 && (q[1] - 1.0).abs() < 1e-9);
        assert_eq!(miroir_point([3.0, 2.0], [0.0, 0.0], [0.0, 1.0]), [-3.0, 2.0]);
        assert!((distance_signee([0.0, 2.0], [0.0, 0.0], [5.0, 0.0]) - 2.0).abs() < 1e-9);
        assert!((distance_signee([0.0, -2.0], [0.0, 0.0], [5.0, 0.0]) + 2.0).abs() < 1e-9);
        assert_eq!(lire_valeur("12,5"), Some(12.5));
        assert_eq!(lire_valeur("-"), None);
    }

    #[test]
    fn echap_recule_d_une_etape_puis_quitte() {
        let mut t = Transformation { points: vec![[0.0, 0.0]], saisie: "12".into(), reference: Some("r".into()), copie: true, ..Default::default() };
        assert!(t.reculer());
        assert!(t.saisie.is_empty() && t.points.len() == 1);
        assert!(t.reculer());
        assert!(t.points.is_empty());
        assert!(t.reculer());
        assert!(t.reference.is_none());
        assert!(!t.reculer());
        assert!(t.copie, "les options survivent");
    }

    #[test]
    fn symetrie_et_decaler_copient_par_defaut() {
        let mut t = Transformation::default();
        t.commencer(Some(Outil::Symetrie));
        assert!(t.copie);
        t.commencer(Some(Outil::Tourner));
        assert!(!t.copie);
        assert_eq!(t.nombre_reseau(), NOMBRE_RESEAU);
    }
}
