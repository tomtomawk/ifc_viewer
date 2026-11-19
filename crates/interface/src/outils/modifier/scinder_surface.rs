use crate::*;
use commandes::Commande;

impl Application {
    pub(super) fn surface_scindable(&self, id: &str) -> bool {
        self.natif.as_ref().is_some_and(|(p, _)| {
            p.dalles.iter().any(|d| d.id == id)
                || p.terrains.iter().any(|t| t.id == id)
                || p.limites_propriete.iter().any(|l| l.id == id)
        })
    }

    pub(crate) fn valider_coupe_surface(&mut self) -> bool {
        let Some(id) = self.transfo.element.clone() else {
            return false;
        };
        let trace = self.transfo.points.clone();
        match self.executer_bim(Commande::ScinderSurface { id, trace }) {
            Ok(ids) => {
                self.status = format!("Surface scindée en {} éléments.", ids.len());
                self.transfo.reinitialiser();
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
}
