//! Granularité des relations, réservations et lectures spatiales du protocole 2.
use super::*;

pub fn carte(cle: &str) -> Option<(&str, &str)> {
    let reste = cle.strip_prefix("@/")?;
    let (nom, id) = reste.split_once('/')?;
    carte_objet(nom).then_some((nom, id))
}
pub fn carte_objet(nom: &str) -> bool {
    matches!(
        nom,
        "implantations"
            | "contextes"
            | "contextes_vues"
            | "gabarits_vue_affectes"
            | "verrous"
            | "demolitions"
            | "priorites_jonction"
            | "attaches_toitures"
            | "cartouches_feuilles"
    )
}
pub fn valeur<'a>(o: &'a Objets, cle: &str) -> Option<&'a Value> {
    if let Some((nom, id)) = carte(cle) {
        o.get(&format!("@/{nom}"))?.get(id)
    } else {
        o.get(cle)
    }
}
pub fn decomposer(changements: Vec<Changement>) -> Vec<Changement> {
    let mut out = Vec::new();
    for c in changements {
        if c.cle.strip_prefix("@/").is_some_and(carte_objet) {
            let a = c.avant.as_ref().and_then(Value::as_object);
            let b = c.apres.as_ref().and_then(Value::as_object);
            let ids: BTreeSet<_> = a
                .into_iter()
                .flat_map(|m| m.keys())
                .chain(b.into_iter().flat_map(|m| m.keys()))
                .collect();
            for id in ids {
                let avant = a.and_then(|m| m.get(id));
                let apres = b.and_then(|m| m.get(id));
                if avant != apres {
                    out.push(Changement {
                        cle: format!("{}/{id}", c.cle),
                        avant: avant.cloned(),
                        apres: apres.cloned(),
                    });
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
pub fn affecter(o: &mut Objets, cle: &str, valeur: Option<Value>) {
    if let Some((nom, id)) = carte(cle) {
        let parent = format!("@/{nom}");
        let m = o
            .entry(parent.clone())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(m) = m.as_object_mut() {
            match valeur {
                Some(v) => {
                    m.insert(id.into(), v);
                }
                None => {
                    m.remove(id);
                }
            }
            if m.is_empty() && nom != "implantations" {
                o.remove(&parent);
            }
        }
    } else {
        match valeur {
            Some(v) => {
                o.insert(cle.into(), v);
            }
            None => {
                o.remove(cle);
            }
        }
    }
}
fn cle_id(o: &Objets, id: &str) -> Option<String> {
    o.iter()
        .find(|(_, v)| v.get("id").and_then(Value::as_str) == Some(id))
        .map(|(k, _)| k.clone())
}
fn physique(k: &str) -> bool {
    matches!(
        k.split('/').next().unwrap_or(""),
        "murs"
            | "assemblages"
            | "ouvertures"
            | "dalles"
            | "terrains"
            | "barres"
            | "pieces"
            | "composants"
            | "types_composants"
            | "escaliers"
            | "garde_corps"
            | "importes"
            | "limites_propriete"
    )
}
impl Transaction {
    pub fn cles_reservation(&self, o: &Objets) -> BTreeSet<String> {
        if self.protocole == 1 {
            return self
                .changements
                .iter()
                .filter(|c| sensible(&c.cle))
                .map(|c| c.cle.clone())
                .collect();
        }
        let mut nouveaux = o.clone();
        for c in &self.changements {
            affecter(&mut nouveaux, &c.cle, c.apres.clone());
        }
        let mut cles = BTreeSet::new();
        for c in &self.changements {
            if let Some((_, id)) = carte(&c.cle) {
                cles.insert(
                    cle_id(&nouveaux, id)
                        .or_else(|| cle_id(o, id))
                        .unwrap_or_else(|| format!("objet/{id}")),
                );
            } else if sensible(&c.cle)
                || physique(&c.cle)
                || c.cle.starts_with("annotations/")
                || c.cle.starts_with("quadrillages/")
                || (c.cle.starts_with("vues/")
                    && match (&c.avant, &c.apres) {
                        (Some(Value::Object(a)), Some(Value::Object(b))) => {
                            a.keys().chain(b.keys()).any(|k| {
                                !matches!(k.as_str(), "nom" | "type_vue") && a.get(k) != b.get(k)
                            })
                        }
                        _ => true,
                    })
            {
                cles.insert(c.cle.clone());
            }
            // Une ouverture ou un garde-corps hébergé réserve aussi son hôte.
            for v in c.avant.iter().chain(c.apres.iter()) {
                for champ in ["mur", "assemblage", "escalier"] {
                    if let Some(id) = v.get(champ).and_then(Value::as_str) {
                        if let Some(k) = cle_id(&nouveaux, id).or_else(|| cle_id(o, id)) {
                            cles.insert(k);
                        }
                    }
                }
            }
        }
        cles
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Zone([f64; 4]);
impl Zone {
    fn points(points: impl Iterator<Item = [f64; 2]>) -> Option<Self> {
        let mut z = Self([
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ]);
        let mut n = 0;
        for p in points {
            if !p.iter().all(|x| x.is_finite()) {
                return None;
            }
            for i in 0..2 {
                z.0[i] = z.0[i].min(p[i]);
                z.0[i + 2] = z.0[i + 2].max(p[i]);
            }
            n += 1;
        }
        (n > 0).then_some(z)
    }
    fn dilater(self, d: f64) -> Self {
        let a = self.0;
        Self([a[0] - d, a[1] - d, a[2] + d, a[3] + d])
    }
    fn touche(self, b: Self) -> bool {
        self.0[0] <= b.0[2] && b.0[0] <= self.0[2] && self.0[1] <= b.0[3] && b.0[1] <= self.0[3]
    }
}
fn points(v: &Value, out: &mut Vec<[f64; 2]>) {
    if let Some(a) = v.as_array() {
        if a.len() >= 2 && a[0].is_number() && a[1].is_number() {
            if let (Some(x), Some(y)) = (a[0].as_f64(), a[1].as_f64()) {
                out.push([x, y]);
            }
        } else {
            for v in a {
                points(v, out);
            }
        }
    }
}
fn largeur(m: &Value, o: &Objets) -> Option<f64> {
    let id = m.get("type_mur")?.as_str()?;
    let t = o.get(&format!("types_murs/{id}"))?;
    let mut e = 0.0;
    for c in t.get("couches")?.as_array()? {
        e += c.get("epaisseur")?.as_f64()?;
    }
    Some(e)
}
fn emprise(k: &str, v: &Value, o: &Objets) -> Option<Zone> {
    let collection = k.split('/').next()?;
    let mut p = Vec::new();
    let mut marge = 5.0;
    match collection {
        "murs" => {
            let axe = v.get("axe")?;
            points(axe.get("debut")?, &mut p);
            points(axe.get("fin")?, &mut p);
            if axe.get("forme").and_then(Value::as_str) == Some("arc") {
                let centre = axe.get("centre")?.as_array()?;
                let c = [centre[0].as_f64()?, centre[1].as_f64()?];
                let r = (p.first()?[0] - c[0]).hypot(p.first()?[1] - c[1]);
                p = vec![[c[0] - r, c[1] - r], [c[0] + r, c[1] + r]];
            }
            marge += 2.0 * largeur(v, o)? + 10.0;
        }
        "assemblages" => {
            for id in v.get("couches")?.as_array()? {
                let cle = format!("murs/{}", id.as_str()?);
                let m = o.get(&cle)?;
                let z = emprise(&cle, m, o)?;
                p.extend([[z.0[0], z.0[1]], [z.0[2], z.0[3]]]);
                marge += 2.0 * largeur(m, o)? + 10.0;
            }
        }
        "ouvertures" => {
            // Toute modification d'hôte est observée ; les familles à géométrie
            // arbitraire restent conservatrices tant qu'une emprise n'est pas prouvée.
            if let Some(m) = v.get("representation").filter(|m| !m.is_null()) {
                points(m.get("sommets")?, &mut p);
            } else {
                if let Some(id) = v.get("type_ouverture").and_then(Value::as_str) {
                    if o.get(&format!("types_ouvertures/{id}"))
                        .and_then(|t| t.get("modele"))
                        .is_some_and(|m| !m.is_null())
                    {
                        return None;
                    }
                }
                let hote = v.get("mur")?.as_str()?;
                let cle = cle_id(o, hote)?;
                let z = emprise(&cle, o.get(&cle)?, o)?;
                p.extend([[z.0[0], z.0[1]], [z.0[2], z.0[3]]]);
                marge += v.get("largeur")?.as_f64()?.max(v.get("hauteur")?.as_f64()?);
            }
        }
        "dalles" => points(v.get("contours")?, &mut p),
        "terrains" => {
            points(v.get("contour")?, &mut p);
            if let Some(v) = v.get("points") {
                points(v, &mut p);
            }
        }
        "barres" => {
            points(v.get("debut")?, &mut p);
            points(v.get("fin")?, &mut p);
            marge += v.get("largeur")?.as_f64()?.max(v.get("hauteur")?.as_f64()?);
        }
        "composants" => {
            points(v.get("position")?, &mut p);
            marge += 3000.0;
        }
        "pieces" => {
            points(v.get("point")?, &mut p);
            if let Some(c) = v.get("contour") {
                points(c, &mut p);
            }
        }
        "escaliers" => {
            points(v.get("trace")?, &mut p);
            marge += 2.0 * v.get("largeur")?.as_f64()? + v.get("paillasse")?.as_f64()?;
        }
        "garde_corps" => {
            points(v.get("trace")?, &mut p);
            marge += 1000.0;
        }
        "importes" => points(v.get("maillage")?.get("sommets")?, &mut p),
        "limites_propriete" => {
            for champ in ["points", "contour", "trace"] {
                if let Some(v) = v.get(champ) {
                    points(v, &mut p);
                }
            }
        }
        _ => return None,
    }
    Zone::points(p.into_iter()).and_then(|z| {
        let z = z.dilater(marge);
        z.0.iter().all(|v| v.is_finite()).then_some(z)
    })
}
fn references(index: &Index, cle: &str) -> Vec<String> {
    index
        .utilisateurs
        .iter()
        .filter(|(_, u)| u.contains(cle))
        .map(|(k, _)| k.clone())
        .collect()
}

/// Protocole 3 : bande autour de l'axe droit, plutôt que tout son rectangle.
/// Les autres géométries gardent une enveloppe conservatrice.
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum Voisinage {
    Bande { debut:[f64;2], fin:[f64;2], rayon:f64 },
    Rectangle(Zone),
}
impl Voisinage {
    pub fn valide(&self)->bool {
        match self {
            Self::Bande{debut,fin,rayon}=>debut.iter().chain(fin).all(|x|x.is_finite()) && rayon.is_finite() && *rayon>=0.0,
            Self::Rectangle(z)=>z.0.iter().all(|x|x.is_finite()) && z.0[0]<=z.0[2] && z.0[1]<=z.0[3],
        }
    }
    fn touche(self,b:Self)->bool {
        match (self,b) {
            (Self::Bande{debut:a,fin:b,rayon:r},Self::Bande{debut:c,fin:d,rayon:s})=>distance_segments(a,b,c,d)<=(r+s).powi(2),
            (Self::Bande{debut:a,fin:b,rayon:r},Self::Rectangle(z)) | (Self::Rectangle(z),Self::Bande{debut:a,fin:b,rayon:r})=>{
                let dedans=|p:[f64;2]|p[0]>=z.0[0]&&p[0]<=z.0[2]&&p[1]>=z.0[1]&&p[1]<=z.0[3];
                let coins=[[z.0[0],z.0[1]],[z.0[2],z.0[1]],[z.0[2],z.0[3]],[z.0[0],z.0[3]]];
                dedans(a)||dedans(b)||(0..4).any(|i|distance_segments(a,b,coins[i],coins[(i+1)%4])<=r*r)
            }
            (Self::Rectangle(a),Self::Rectangle(b))=>a.touche(b),
        }
    }
}
fn distance_segments(a:[f64;2],b:[f64;2],c:[f64;2],d:[f64;2])->f64 {
    let [a,b,c,d]=[a,b,c,d].map(glam::DVec2::from_array);
    let ab=b-a;let cd=d-c;
    let croix=ab.perp_dot(cd);
    if croix!=0.0 {
        let t=(c-a).perp_dot(cd)/croix;
        let u=(c-a).perp_dot(ab)/croix;
        if (0.0..=1.0).contains(&t)&&(0.0..=1.0).contains(&u){return 0.0;}
    }
    let point=|p:glam::DVec2,x:glam::DVec2,y:glam::DVec2|{
        let v=y-x;
        let t=if v.length_squared()>0.0{((p-x).dot(v)/v.length_squared()).clamp(0.0,1.0)}else{0.0};
        (p-(x+t*v)).length_squared()
    };
    point(a,c,d).min(point(b,c,d)).min(point(c,a,b)).min(point(d,a,b))
}
fn voisinage(k:&str,v:&Value,o:&Objets)->Option<Voisinage> {
    if k.starts_with("murs/") && v.get("axe")?.get("forme")?.as_str()?=="droit" {
        let axe=&v["axe"];
        let point=|cle:&str|->Option<[f64;2]>{let a=axe.get(cle)?.as_array()?;Some([a.first()?.as_f64()?,a.get(1)?.as_f64()?])};
        let b=Voisinage::Bande{debut:point("debut")?,fin:point("fin")?,rayon:2.0*largeur(v,o)?+15.0};
        return b.valide().then_some(b);
    }
    emprise(k,v,o).map(Voisinage::Rectangle)
}
pub fn empreinte_voisinage(o:&Objets,zones:&[Voisinage])->String {
    let index=Index::construire(o);
    let mut selection:BTreeSet<String>=o.iter().filter(|(k,v)|physique(k)&&voisinage(k,v,o).is_none_or(|b|zones.iter().any(|z|z.touche(b)))).map(|(k,_)|k.clone()).collect();
    completer_selection(o,&index,&mut selection);
    hash(&selection.iter().map(|k|(k,valeur(o,k))).collect::<BTreeMap<_,_>>())
}

pub fn empreinte_zone(o: &Objets, zones: &[Zone]) -> String {
    let index = Index::construire(o);
    let mut selection: BTreeSet<String> = o
        .iter()
        .filter(|(k, v)| {
            physique(k) && emprise(k, v, o).is_none_or(|b| zones.iter().any(|z| z.touche(b)))
        })
        .map(|(k, _)| k.clone())
        .collect();
    completer_selection(o,&index,&mut selection);
    hash(&selection.iter().map(|k| (k, valeur(o, k))).collect::<BTreeMap<_, _>>())
}
fn completer_selection(o:&Objets,index:&Index,selection:&mut BTreeSet<String>) {
    let mut front: Vec<_> = selection.iter().cloned().collect();
    while let Some(k) = front.pop() {
        for r in references(&index, &k) {
            if selection.insert(r.clone()) {
                front.push(r);
            }
        }
    }
    for k in [
        "@/coordonnees_origine",
        "@/orientation_nord",
        "@/lieu",
        "@/parametres_partages",
    ] {
        selection.insert(k.into());
    }
    if selection.iter().any(|k| {
        k.starts_with("terrains/")
            && o.get(k)
                .is_some_and(|v| v.get("type_terrain").and_then(Value::as_str).is_none())
    }) {
        selection.extend(
            o.keys()
                .filter(|k| {
                    k.starts_with("types_terrains/") || k.as_str() == "@ordre/types_terrains"
                })
                .cloned(),
        );
    }
    // Les relations entrantes appartiennent à chaque objet, pas à la carte entière.
    let liens: Vec<_> = selection
        .iter()
        .flat_map(|k| index.utilisateurs.get(k).into_iter().flatten())
        .filter(|k| carte(k).is_some())
        .cloned()
        .collect();
    selection.extend(liens);
}
pub fn empreinte_types_terrain(o: &Objets) -> String {
    hash(
        &o.iter()
            .filter(|(k, _)| {
                k.starts_with("types_terrains/") || k.as_str() == "@ordre/types_terrains"
            })
            .collect::<BTreeMap<_, _>>(),
    )
}
pub fn empreinte_usagers_terrain(o: &Objets) -> String {
    hash(
        &o.iter()
            .filter(|(k, v)| {
                k.starts_with("terrains/")
                    && v.get("type_terrain").and_then(Value::as_str).is_none()
            })
            .collect::<BTreeMap<_, _>>(),
    )
}
fn effet_geometrique(c: &Changement) -> bool {
    if matches!(
        c.cle.split('/').next().unwrap_or(""),
        "materiaux" | "motifs" | "familles_cartouches" | "familles_etiquettes" | "types_traits"
    ) {
        return false;
    }
    if let (Some(Value::Object(a)), Some(Value::Object(b))) = (&c.avant, &c.apres) {
        return a
            .keys()
            .chain(b.keys())
            .any(|k| k != "nom" && a.get(k) != b.get(k));
    }
    true
}
pub fn ajouter_spatial(t: &mut Transaction, a: &Objets) {
    if t.changements.iter().any(|c| {
        c.cle.starts_with("types_terrains/")
            || c.cle.starts_with("terrains/")
                && c.avant
                    .iter()
                    .chain(c.apres.iter())
                    .any(|v| v.get("type_terrain").and_then(Value::as_str).is_none())
    }) {
        t.lectures
            .insert("#types-terrains-defaut".into(), empreinte_types_terrain(a));
        if t.changements
            .iter()
            .any(|c| c.cle.starts_with("types_terrains/"))
        {
            t.lectures.insert(
                "#usagers-terrain-defaut".into(),
                empreinte_usagers_terrain(a),
            );
        }
    }
    let mut b = a.clone();
    for c in &t.changements {
        affecter(&mut b, &c.cle, c.apres.clone());
    }
    let ia = Index::construire(a);
    let ib = Index::construire(&b);
    let mut touches: BTreeSet<String> = t
        .changements
        .iter()
        .filter(|c| effet_geometrique(c) && (physique(&c.cle) || sensible(&c.cle)))
        .map(|c| c.cle.clone())
        .collect();
    for c in &t.changements {
        if carte(&c.cle).is_some_and(|(nom, _)| {
            matches!(
                nom,
                "implantations" | "priorites_jonction" | "attaches_toitures"
            )
        }) {
            for i in [&ia, &ib] {
                touches.extend(references(i, &c.cle));
            }
        }
    }
    let mut front: Vec<_> = touches.iter().cloned().collect();
    while let Some(k) = front.pop() {
        for i in [&ia, &ib] {
            for u in i.utilisateurs.get(&k).into_iter().flatten() {
                if touches.insert(u.clone()) {
                    front.push(u.clone());
                }
            }
        }
    }
    let mut zones = Vec::new();
    let mut voisinages = Vec::new();
    for k in &touches {
        if physique(k) {
            for o in [a, &b] {
                if let Some(v) = o.get(k) {
                    if let Some(z) = emprise(k, v, o) {
                        zones.push(z);
                        voisinages.push(voisinage(k,v,o).unwrap_or(Voisinage::Rectangle(z)));
                    } else {
                        t.lectures.insert("#spatial".into(), spatial(a));
                        return;
                    }
                }
            }
        } else if k.starts_with("@/") && carte(k).is_none() && sensible(k) {
            t.lectures.insert("#spatial".into(), spatial(a));
            return;
        }
    }
    if !zones.is_empty() {
        if t.protocole>=3 {
            let key=format!("#voisinage3/{}",serde_json::to_string(&voisinages).expect("voisinages finis"));
            t.lectures.insert(key,empreinte_voisinage(a,&voisinages));
            return;
        }
        let key = format!(
            "#zones/{}",
            serde_json::to_string(&zones).expect("emprises finies")
        );
        t.lectures.insert(key, empreinte_zone(a, &zones));
    }
}
