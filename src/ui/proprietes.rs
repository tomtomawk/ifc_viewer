//! Panneau des propriétés de la sélection native.
use super::*;

impl ViewerApp {
    pub(super) fn proprietes_natif(&mut self, ui: &mut egui::Ui, id: Id) -> Option<crate::modele::Projet> {
        use crate::modele::Haut;
        let mut pente_defaut = self.pente_defaut;
        let (projet, _) = self.natif.as_ref()?;
        let mut nouveau: Option<crate::modele::Projet> = None;
        let mut selection: Option<Id> = None;
        if let Some(a) = projet.assemblages.iter().find(|a| scene::id_natif("assemblage", &a.id) == id) {
            ui.label(RichText::new(format!("Mur composé · {}", a.nom)).strong());
            if let Some(t) = projet.type_mur(&a.type_mur) {
                ui.label(RichText::new(&t.nom).weak());
            }
            let couches = projet.couches_assemblage(&a.id);
            egui::Grid::new("assemblage_identite").num_columns(2).striped(true).show(ui,|ui| {
                if let Some(m)=couches.first() {
                    ui.label("Niveau de base");ui.label(projet.niveau(&m.niveau).map_or("",|n|n.nom.as_str()));ui.end_row();
                    ui.label("Contrainte haute");
                    ui.label(match &m.haut {Haut::Libre{hauteur}=>crate::config::options::unite().afficher(*hauteur),Haut::Niveau{niveau,..}=>projet.niveau(niveau).map_or_else(||niveau.clone(),|n|n.nom.clone())});ui.end_row();
                }
                ui.label("Nom");ui.label(&a.nom);ui.end_row();
                if let Some((axe,_))=projet.trajectoire_justification(&a.id) {ui.label("Longueur");ui.label(crate::config::options::unite().afficher(axe.longueur()));ui.end_row();}
                let volume:f64=couches.iter().filter_map(|m|projet.volume_mur(m)).sum();
                ui.label("Volume");ui.label(format!("{:.3} m³",volume/1e9));ui.end_row();
            });
            // le plan des couches est dans le sélecteur de type, juste au-dessus
            ui.label(RichText::new("Couches, de l'extérieur vers l'intérieur (clic : sélectionner ; Tab parcourt)").small().weak());
            egui::Grid::new("couches_assemblage").num_columns(4).striped(true).spacing([8.0, 3.0]).show(ui, |ui| {
                for m in &couches {
                    let c = projet.type_mur(&m.type_mur).and_then(|t| t.couches.first());
                    let nom = c.and_then(|c| c.materiau.clone()).unwrap_or_else(|| m.nom.clone());
                    if ui.link(nom).clicked() {
                        selection = Some(scene::id_natif("mur", &m.id));
                    }
                    ui.label(crate::config::options::unite().afficher(c.map_or(0.0, |c| c.epaisseur)));
                    ui.label(projet.bas_haut(m).map_or(String::new(), |(b, h)| format!("h {:.0}", h - b)));
                    ui.label(RichText::new(m.phase.as_deref().unwrap_or("")).weak());
                    ui.end_row();
                }
            });
            ui.separator();
        } else if let Some(mur) = projet.murs.iter().find(|m| scene::id_natif("mur", &m.id) == id) {
            let mut m = mur.clone();
            let epaisseur = projet.type_mur(&m.type_mur).map_or(0.0, |t| t.epaisseur());
            let materiau = projet.type_mur(&m.type_mur).and_then(|t| (t.couches.len() == 1).then(|| t.couches[0].materiau.clone()).flatten());
            let assemblage = m.assemblage.as_ref().and_then(|a| projet.assemblage(a));
            ui.label(RichText::new(match (assemblage, &materiau) {
                (Some(a), Some(x)) => format!("Couche · {x} · de {}", a.nom),
                (Some(a), None) => format!("Couche de {}", a.nom),
                (None, _) => m.nom.clone(),
            }).strong());
            let mut change = false;
            egui::Grid::new("mur_natif_proprietes").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                let niveaux: Vec<(String, String)> = projet.niveaux.iter().map(|n| (n.id.clone(), n.nom.clone())).collect();
                let nom_niveau = |id: &str| niveaux.iter().find(|n| n.0 == id).map_or(id.to_string(), |n| n.1.clone());
                ui.label("Niveau de base");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("mur_niveau").width(90.0).truncate().selected_text(nom_niveau(&m.niveau)).show_ui(ui, |ui| {
                        for (nid, nom) in &niveaux {
                            change |= ui.selectable_value(&mut m.niveau, nid.clone(), nom).changed();
                        }
                    });
                    change |= ui.add(egui::DragValue::new(&mut m.decalage_bas).speed(10.0).dimensions()).on_hover_text("Décalage de la base par rapport au niveau").changed();
                });
                ui.end_row();
                ui.label("Contrainte haute");
                let libre = matches!(m.haut, Haut::Libre { .. });
                let texte = match &m.haut { Haut::Libre { .. } => "Libre".to_string(), Haut::Niveau { niveau, .. } => nom_niveau(niveau) };
                let (bas, haut) = projet.bas_haut(&m).unwrap_or((0.0, 2800.0));
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("mur_haut").width(90.0).truncate().selected_text(texte).show_ui(ui, |ui| {
                        if ui.selectable_label(libre, "Libre (hauteur)").clicked() && !libre {
                            m.haut = Haut::Libre { hauteur: haut - bas };
                            change = true;
                        }
                        for (nid, nom) in &niveaux {
                            let actif = matches!(&m.haut, Haut::Niveau { niveau, .. } if niveau == nid);
                            if ui.selectable_label(actif, nom).clicked() && !actif {
                                m.haut = Haut::Niveau { niveau: nid.clone(), decalage: 0.0 };
                                change = true;
                            }
                        }
                    });
                    match &mut m.haut {
                        Haut::Libre { hauteur } => {
                            change |= ui.add(egui::DragValue::new(hauteur).speed(10.0).range(1.0..=f64::MAX).dimensions()).on_hover_text("Hauteur du mur").changed();
                        }
                        Haut::Niveau { decalage, .. } => {
                            change |= ui.add(egui::DragValue::new(decalage).speed(10.0).dimensions()).on_hover_text("Décalage du haut par rapport au niveau").changed();
                        }
                    }
                });
                ui.end_row();
                ui.label("Nom");ui.label(&m.nom);ui.end_row();
                let longueur=projet.trajectoire_justification(m.assemblage.as_deref().unwrap_or(&m.id)).map_or_else(||m.axe.longueur(),|(axe,_)|axe.longueur());
                for (nom, valeur) in [
                    ("Longueur", crate::config::options::unite().afficher(longueur)),
                    ("Hauteur résultante", crate::config::options::unite().afficher(haut - bas)),
                    ("Épaisseur", crate::config::options::unite().afficher(epaisseur)),
                    ("Volume", format!("{:.3} m³", projet.volume_mur(&m).unwrap_or(0.0) / 1e9)),
                ] {
                    ui.label(nom);
                    ui.label(valeur);
                    ui.end_row();
                }
            });
            if change {
                let mut p = projet.clone();
                if let Some(x) = p.murs.iter_mut().find(|x| x.id == m.id) {
                    *x = m.clone();
                }
                match p.bas_haut(&m) {
                    Some((b, h)) if h > b => nouveau = Some(p),
                    _ => self.error = Some("La limite haute du mur doit être au-dessus de sa limite basse.".into()),
                }
            }
            ui.separator();
        } else if let Some(arbre) = projet.arbres.iter().find(|a| scene::id_natif("arbre", &a.id) == id) {
            use crate::modele::elements::arbre::Essence;
            let mut a = arbre.clone();
            ui.label(RichText::new(format!("Arbre · {}", a.nom)).strong());
            let mut change = false;
            egui::Grid::new("arbre_proprietes").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Essence");
                egui::ComboBox::from_id_salt("arbre_essence").selected_text(a.essence.nom()).show_ui(ui, |ui| {
                    for e in Essence::TOUTES {
                        change |= ui.selectable_value(&mut a.essence, e, e.nom()).changed();
                    }
                });
                ui.end_row();
                ui.label("Hauteur");
                change |= ui.add(egui::DragValue::new(&mut a.hauteur).speed(50.0).range(500.0..=60_000.0).dimensions()).changed();
                ui.end_row();
                ui.label("Houppier");
                change |= ui.add(egui::DragValue::new(&mut a.diametre).speed(50.0).range(300.0..=40_000.0).dimensions()).changed();
                ui.end_row();
                ui.label("Décalage");
                change |= ui.add(egui::DragValue::new(&mut a.decalage).speed(10.0).dimensions()).changed();
                ui.end_row();
            });
            if change {
                let mut p = projet.clone();
                if let Some(x) = p.arbres.iter_mut().find(|x| x.id == a.id) {
                    *x = a;
                }
                nouveau = Some(p);
            }
            ui.separator();
        } else if let Some(e) = projet.importes.iter().find(|e| scene::id_natif("importe", &e.id)==id) {
            ui.heading(&e.nom); ui.label(&e.categorie); ui.label("Géométrie importée"); ui.label(&e.raison);
            if let Some(n)=e.niveau.as_ref().and_then(|n|projet.niveau(n)) { ui.label(format!("Niveau : {}",n.nom)); }
            if let Some(o)=&e.origine { ui.label(format!("Source : {} · {}",o.source,o.id)); if let Some(g)=&o.guid {ui.label(g);} }
        } else if let Some(o) = projet.ouvertures.iter().find(|o| scene::id_natif("ouverture", &o.id)==id) {
            let mut o=o.clone(); let mut change=false;
            ui.heading(&o.nom);
            for (nom,v) in [("Abscisse",&mut o.abscisse),("Largeur",&mut o.largeur),("Hauteur",&mut o.hauteur),("Allège",&mut o.allege)] {
                ui.horizontal(|ui| { ui.label(nom); change|=ui.add_enabled(o.representation.is_none() || matches!(nom,"Abscisse"|"Allège"),egui::DragValue::new(v).dimensions().speed(10.0)).changed(); });
            }
            // paramètres d'occurrence de la famille : propres à cette ouverture
            if let Some(f) = o.type_ouverture.as_ref().and_then(|t| projet.types_ouvertures.iter().find(|x| &x.id == t)).and_then(|t| t.modele.as_ref()) {
                use crate::modele::famille::modelee::{GenreParametre, Portee};
                let occurrences: Vec<&crate::modele::famille::modelee::Parametre> = f.parametres.iter().filter(|p| p.portee == Portee::Occurrence).collect();
                if !occurrences.is_empty() {
                    let e = crate::modele::volumes::solides::hote(projet, &o.mur).map_or(200.0, |h| h.1);
                    let surcharges = projet.surcharges(&o);
                    let r = f.resoudre_occurrence(o.largeur, o.hauteur, e, &surcharges, &o.materiaux);
                    ui.label(RichText::new("Paramètres d'occurrence").strong());
                    for p in occurrences {
                        let nom = p.nom.as_str();
                        ui.horizontal(|ui| {
                            ui.label(nom);
                            match p.genre {
                                GenreParametre::OuiNon => {
                                    let mut oui = r.valeurs.get(nom).is_none_or(|v| v.abs() > 1e-9);
                                    if ui.checkbox(&mut oui, "").changed() {
                                        o.valeurs.insert(nom.to_string(), if oui { 1.0 } else { 0.0 });
                                        change = true;
                                    }
                                }
                                GenreParametre::Materiau => {
                                    let mut m = r.materiaux.get(nom).cloned().unwrap_or_default();
                                    let avant = m.clone();
                                    egui::ComboBox::from_id_salt(("materiau_occurrence", nom)).width(140.0).selected_text(if m.is_empty() { "(rôle)" } else { m.as_str() }).show_ui(ui, |ui| {
                                        ui.selectable_value(&mut m, String::new(), "(couleur du rôle)");
                                        for x in &projet.materiaux {
                                            ui.selectable_value(&mut m, x.nom.clone(), &x.nom);
                                        }
                                    });
                                    if m != avant {
                                        o.materiaux.insert(nom.to_string(), m);
                                        change = true;
                                    }
                                }
                                GenreParametre::Longueur => {
                                    let mut v = r.valeurs.get(nom).copied().unwrap_or(0.0);
                                    if ui.add(egui::DragValue::new(&mut v).dimensions().speed(5.0)).changed() {
                                        o.valeurs.insert(nom.to_string(), v);
                                        change = true;
                                    }
                                }
                            }
                            if (o.valeurs.contains_key(nom) || o.materiaux.contains_key(nom)) && ui.small_button("Défaut").on_hover_text("Reprendre la valeur de la famille").clicked() {
                                o.valeurs.remove(nom);
                                o.materiaux.remove(nom);
                                change = true;
                            }
                        });
                    }
                }
            }
            if o.type_ouverture.as_ref().and_then(|t| projet.types_ouvertures.iter().find(|x| &x.id == t)).is_some_and(|t| t.modele.is_some()) {
                ui.horizontal(|ui| {
                    change |= ui.checkbox(&mut o.retournee, "Retourner").on_hover_text("Échange intérieur et extérieur : sens d'ouverture").changed();
                    change |= ui.checkbox(&mut o.inversee, "Inverser").on_hover_text("Échange gauche et droite : main de la porte").changed();
                });
            }
            if o.representation.is_some() && ui.button("Remplacer la menuiserie importée par un cadre paramétrique").clicked() {o.representation=None;change=true;}
            if change {
                if o.representation.is_some() { ui.label("La menuiserie importée est conservée ; sa forme n'est pas paramétrique."); }
                let mut p=projet.clone(); *p.ouvertures.iter_mut().find(|x|x.id==o.id).unwrap()=o.clone(); nouveau=Some(p);
            }
        } else if let Some(e) = projet.escaliers.iter().find(|e| scene::id_natif("escalier", &e.id) == id) {
            let mut e = e.clone();
            let mut change = false;
            ui.label(RichText::new(format!("Escalier · {}", e.nom)).strong());
            egui::Grid::new("proprietes_escalier").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Nom");
                change |= ui.text_edit_singleline(&mut e.nom).lost_focus();
                ui.end_row();
                let nom_niveau = |id: &str| projet.niveau(id).map_or(String::new(), |n| n.nom.clone());
                ui.label("Niveau de base");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("escalier_niveau").width(90.0).truncate().selected_text(nom_niveau(&e.niveau)).show_ui(ui, |ui| {
                        for n in &projet.niveaux {
                            change |= ui.selectable_value(&mut e.niveau, n.id.clone(), &n.nom).changed();
                        }
                    });
                    change |= ui.add(egui::DragValue::new(&mut e.decalage_bas).dimensions().speed(5.0)).on_hover_text("Décalage de la base par rapport au niveau").changed();
                });
                ui.end_row();
                ui.label("Niveau d'arrivée");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("escalier_haut").width(90.0).truncate().selected_text(e.niveau_haut.as_deref().map_or("Libre".to_string(), nom_niveau)).show_ui(ui, |ui| {
                        change |= ui.selectable_value(&mut e.niveau_haut, None, "Hauteur libre").changed();
                        for n in &projet.niveaux {
                            change |= ui.selectable_value(&mut e.niveau_haut, Some(n.id.clone()), &n.nom).changed();
                        }
                    });
                    if e.niveau_haut.is_none() {
                        change |= ui.add(egui::DragValue::new(&mut e.hauteur).dimensions().speed(5.0)).on_hover_text("Hauteur libre").changed();
                    } else {
                        change |= ui.add(egui::DragValue::new(&mut e.decalage_haut).dimensions().speed(5.0)).on_hover_text("Décalage de l'arrivée par rapport au niveau").changed();
                    }
                });
                ui.end_row();
                for (nom, v, actif) in [("Largeur", &mut e.largeur, true), ("Hauteur de marche visée", &mut e.hauteur_marche, true), ("Paillasse", &mut e.paillasse, true)] {
                    ui.label(nom);
                    change |= ui.add_enabled(actif, egui::DragValue::new(v).dimensions().speed(5.0)).changed();
                    ui.end_row();
                }
            });
            // caractéristiques calculées et confort (Blondel)
            match e.calcul(projet) {
                Ok(c) => {
                    let u = crate::config::options::unite();
                    ui.label(format!("{} contremarches de {} · giron {} · {} palier(s)", c.contremarches, u.afficher(c.h), u.afficher(c.giron), c.paliers));
                    let texte = format!("Blondel 2h + g = {}", u.afficher(c.blondel));
                    if c.confortable() { ui.label(texte); } else { ui.colored_label(Color32::from_rgb(200, 120, 20), format!("{texte} : hors confort (60 à 65 cm, h ≤ 20 cm, g ≥ 22 cm)")); }
                }
                Err(x) => { ui.colored_label(Color32::from_rgb(200, 40, 40), x); }
            }
            if change {
                let mut p = projet.clone();
                let id = e.id.clone();
                *p.escaliers.iter_mut().find(|x| x.id == id).unwrap() = e;
                nouveau = Some(p);
            }
        } else if let Some(g) = projet.garde_corps.iter().find(|g| scene::id_natif("garde_corps", &g.id) == id) {
            let mut g = g.clone();
            let mut change = false;
            ui.label(RichText::new(format!("Garde-corps · {}", g.nom)).strong());
            egui::Grid::new("proprietes_garde_corps").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Nom");
                change |= ui.text_edit_singleline(&mut g.nom).lost_focus();
                ui.end_row();
                ui.label("Hauteur");
                change |= ui.add(egui::DragValue::new(&mut g.hauteur).dimensions().speed(5.0)).changed();
                ui.end_row();
                ui.label("Remplissage");
                egui::ComboBox::from_id_salt("garde_corps_remplissage").selected_text(g.remplissage.nom()).show_ui(ui, |ui| {
                    for r in crate::modele::Remplissage::TOUS {
                        change |= ui.selectable_value(&mut g.remplissage, r, r.nom()).changed();
                    }
                });
                ui.end_row();
                if g.escalier.is_none() {
                    ui.label("Décalage");
                    change |= ui.add(egui::DragValue::new(&mut g.decalage).dimensions().speed(5.0)).changed();
                    ui.end_row();
                }
            });
            let hote = g.escalier.as_ref().and_then(|id| projet.escaliers.iter().find(|e| &e.id == id)).map(|e| e.nom.clone());
            ui.label(RichText::new(match hote { Some(n) => format!("Suit l'escalier « {n} » · longueur {}", crate::config::options::unite().afficher(g.longueur())), None => format!("Longueur {}", crate::config::options::unite().afficher(g.longueur())) }).weak());
            if change {
                let mut p = projet.clone();
                let id = g.id.clone();
                *p.garde_corps.iter_mut().find(|x| x.id == id).unwrap() = g;
                nouveau = Some(p);
            }
        } else {
            let dalle = projet.dalles.iter().find(|d| scene::id_natif("dalle", &d.id) == id)?;
            let mut d = dalle.clone();
            let t = projet.type_dalle(&d.type_dalle);
            ui.label(RichText::new(format!("{} · {}", d.sorte.nom(), t.map_or("", |t| t.nom.as_str()))).strong());
            if let Some(t) = t {
                for c in &t.couches {
                    ui.horizontal(|ui| {
                        let [r, g, b] = c.materiau.as_ref().and_then(|n| projet.materiau(n)).map_or([200, 200, 196], |m| m.couleur);
                        let (q, _) = ui.allocate_exact_size(egui::vec2(16.0, 12.0), Sense::hover());
                        ui.painter().rect_filled(q, 0.0, Color32::from_rgb(r, g, b));
                        ui.label(RichText::new(format!("{} · {}", c.materiau.as_deref().unwrap_or("Couche"), crate::config::options::unite().afficher(c.epaisseur))).small());
                    });
                }
            }
            let mut change = false;
            egui::Grid::new("dalle_natif_proprietes").num_columns(2).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Niveau");
                let nom = projet.niveau(&d.niveau).map_or(d.niveau.clone(), |n| n.nom.clone());
                egui::ComboBox::from_id_salt("dalle_niveau").width(150.0).selected_text(nom).show_ui(ui, |ui| {
                    for n in &projet.niveaux {
                        change |= ui.selectable_value(&mut d.niveau, n.id.clone(), &n.nom).changed();
                    }
                });
                ui.end_row();
                ui.label("Sorte");
                egui::ComboBox::from_id_salt("dalle_sorte").width(150.0).selected_text(d.sorte.nom()).show_ui(ui, |ui| {
                    for s in crate::modele::SorteDalle::TOUTES {
                        change |= ui.selectable_value(&mut d.sorte, s, s.nom()).changed();
                    }
                })
                .response
                .on_hover_text("Même élément, nom et référence d'altitude différents (dessus ou sous-face)");
                ui.end_row();
                ui.label(match d.sorte {
                    crate::modele::SorteDalle::Plafond => "Hauteur sous plafond",
                    crate::modele::SorteDalle::Toiture => "Décalage (sous-face)",
                    _ => "Décalage (dessus)",
                });
                change |= ui.add(egui::DragValue::new(&mut d.decalage).speed(10.0).dimensions()).changed();
                ui.end_row();
                let aire = crate::modele::volumes::geometrie::aire_pair_impair(&d.contours);
                for (nom, valeur) in [
                    ("Aire", format!("{:.2} m²", aire / 1e6)),
                    ("Épaisseur", crate::config::options::unite().afficher(t.map_or(0.0, |t| t.epaisseur()))),
                    ("Volume", format!("{:.3} m³", projet.volume_dalle(&d).unwrap_or(0.0) / 1e9)),
                ] {
                    ui.label(nom);
                    ui.label(valeur);
                    ui.end_row();
                }
            });
            if d.sorte.pente_possible() {
                ui.add_space(4.0);
                ui.label(RichText::new("Arêtes").strong())
                    .on_hover_text("Une arête cochée définit une pente : le versant monte depuis elle vers l'intérieur. Aussi en cliquant ses repères dans la vue.");
                egui::Grid::new("dalle_aretes").num_columns(3).striped(true).spacing([8.0, 3.0]).show(ui, |ui| {
                    for c in 0..d.contours.len() {
                        let n = d.contours[c].len();
                        for k in 0..n {
                            let (a, b) = (d.contours[c][k], d.contours[c][(k + 1) % n]);
                            let nom = if d.contours.len() > 1 { format!("{}.{}", c + 1, k + 1) } else { format!("{}", k + 1) };
                            ui.label(format!("Arête {nom} · {}", crate::config::options::unite().afficher((b[0] - a[0]).hypot(b[1] - a[1]))));
                            let mut pente = d.pente(c, k);
                            let mut oui = pente.is_some();
                            if ui.checkbox(&mut oui, "Pente").changed() {
                                pente = oui.then_some(pente_defaut);
                            }
                            match &mut pente {
                                Some(a) => {
                                    ui.add(egui::DragValue::new(a).speed(0.5).range(0.5..=85.0).max_decimals(1).suffix(" °"));
                                }
                                None => {
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                            if pente != d.pente(c, k) {
                                if let Some(a) = pente { pente_defaut = a; }
                                d.definir_pente(c, k, pente);
                                change = true;
                            }
                        }
                    }
                });
            }
            if change {
                let mut p = projet.clone();
                if let Some(x) = p.dalles.iter_mut().find(|x| x.id == d.id) {
                    *x = d;
                }
                nouveau = Some(p);
            }
            ui.separator();
        }
        self.pente_defaut = pente_defaut;
        if let Some(sel) = selection {
            self.select(Some(sel), true);
        }
        nouveau
    }

}
