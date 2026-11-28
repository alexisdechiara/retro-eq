//! Moteur factice : il note ce qu'on lui demande, sans toucher au son.

use std::sync::{Arc, Mutex, MutexGuard};

use retro_dsp::Courbe;

use crate::{ErreurMoteur, EtatMoteur, Moteur, SortieAudio};

/// Un appel reçu par le moteur simulé.
#[derive(Debug, Clone, PartialEq)]
pub enum Appel {
    Appliquer(Courbe),
    Couper,
    Retirer,
}

#[derive(Debug)]
struct Journal {
    appels: Vec<Appel>,
    /// Le message d'échec et le nombre d'appels qui doivent encore échouer.
    echec: Option<(String, u32)>,
    etat: EtatMoteur,
    sorties: Vec<SortieAudio>,
}

/// Moteur pour les tests et pour faire tourner l'appli sans Equalizer APO.
///
/// Les clones partagent le même journal (`Arc<Mutex<..>>`) : un test garde un
/// clone pendant que l'appli possède le `Box<dyn Moteur>`.
#[derive(Debug, Clone)]
pub struct MoteurSimule {
    journal: Arc<Mutex<Journal>>,
}

impl Default for MoteurSimule {
    fn default() -> Self {
        Self::nouveau()
    }
}

impl MoteurSimule {
    /// Un moteur « installé mais inactif », avec une seule sortie factice.
    pub fn nouveau() -> Self {
        MoteurSimule {
            journal: Arc::new(Mutex::new(Journal {
                appels: Vec::new(),
                echec: None,
                etat: EtatMoteur::Inactif,
                sorties: vec![SortieAudio {
                    id: "simulee".into(),
                    nom: "Sortie simulée".into(),
                    moteur_installe: true,
                }],
            })),
        }
    }

    // Un test qui panique ne doit pas empoisonner les autres : on reprend le contenu.
    fn journal(&self) -> MutexGuard<'_, Journal> {
        self.journal.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Tous les appels reçus, dans l'ordre.
    pub fn appels(&self) -> Vec<Appel> {
        self.journal().appels.clone()
    }

    /// Les courbes appliquées, dans l'ordre.
    pub fn courbes_appliquees(&self) -> Vec<Courbe> {
        self.journal()
            .appels
            .iter()
            .filter_map(|a| match a {
                Appel::Appliquer(c) => Some(c.clone()),
                _ => None,
            })
            .collect()
    }

    /// Le prochain appel (et seulement lui) échouera avec ce message ; il
    /// n'est pas enregistré.
    pub fn echouer_au_prochain_appel(&self, message: &str) {
        self.echouer_aux_prochains_appels(1, message);
    }

    /// Les `nombre` prochains appels échoueront avec ce message, sans être enregistrés ;
    /// les suivants réussiront.
    pub fn echouer_aux_prochains_appels(&self, nombre: u32, message: &str) {
        self.journal().echec = (nombre > 0).then(|| (message.to_string(), nombre));
    }

    /// Force l'état renvoyé par `etat()` (ex. `Absent`).
    pub fn definir_etat(&self, etat: EtatMoteur) {
        self.journal().etat = etat;
    }

    /// Remplace la liste renvoyée par `sorties()`.
    pub fn definir_sorties(&self, sorties: Vec<SortieAudio>) {
        self.journal().sorties = sorties;
    }

    fn enregistrer(&self, appel: Appel, nouvel_etat: EtatMoteur) -> Result<(), ErreurMoteur> {
        let mut j = self.journal();
        if let Some((message, restants)) = j.echec.take() {
            if restants > 1 {
                j.echec = Some((message.clone(), restants - 1));
            }
            return Err(ErreurMoteur::Simule(message));
        }
        j.appels.push(appel);
        j.etat = nouvel_etat;
        Ok(())
    }
}

impl Moteur for MoteurSimule {
    fn appliquer(&mut self, courbe: &Courbe) -> Result<(), ErreurMoteur> {
        self.enregistrer(Appel::Appliquer(courbe.clone()), EtatMoteur::Actif)
    }

    fn couper(&mut self) -> Result<(), ErreurMoteur> {
        self.enregistrer(Appel::Couper, EtatMoteur::Actif)
    }

    fn etat(&self) -> EtatMoteur {
        self.journal().etat
    }

    fn sorties(&self) -> Result<Vec<SortieAudio>, ErreurMoteur> {
        Ok(self.journal().sorties.clone())
    }

    fn retirer(&mut self) -> Result<(), ErreurMoteur> {
        self.enregistrer(Appel::Retirer, EtatMoteur::Inactif)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn courbe(preampli: f64) -> Courbe {
        Courbe {
            preampli_db: preampli,
            filtres: vec![],
        }
    }

    #[test]
    fn enregistre_les_appels_dans_l_ordre_via_un_clone() {
        let espion = MoteurSimule::nouveau();
        let mut boite: Box<dyn Moteur> = Box::new(espion.clone());
        boite.appliquer(&courbe(-1.0)).unwrap();
        boite.couper().unwrap();
        boite.appliquer(&courbe(-2.0)).unwrap();
        boite.retirer().unwrap();
        assert_eq!(
            espion.appels(),
            vec![
                Appel::Appliquer(courbe(-1.0)),
                Appel::Couper,
                Appel::Appliquer(courbe(-2.0)),
                Appel::Retirer
            ]
        );
        assert_eq!(
            espion.courbes_appliquees(),
            vec![courbe(-1.0), courbe(-2.0)]
        );
        assert_eq!(espion.etat(), EtatMoteur::Inactif);
    }

    #[test]
    fn echec_demande_ne_touche_que_le_prochain_appel() {
        let mut m = MoteurSimule::nouveau();
        m.echouer_au_prochain_appel("boum");
        assert!(matches!(m.couper(), Err(ErreurMoteur::Simule(_))));
        assert!(m.appels().is_empty());
        assert!(m.couper().is_ok());
        assert_eq!(m.appels(), vec![Appel::Couper]);
    }

    #[test]
    fn echec_demande_pour_plusieurs_appels_touche_exactement_ceux_la() {
        let mut m = MoteurSimule::nouveau();
        m.echouer_aux_prochains_appels(3, "verrouillé");
        for _ in 0..3 {
            assert!(matches!(
                m.appliquer(&courbe(-1.0)),
                Err(ErreurMoteur::Simule(_))
            ));
        }
        assert!(m.appels().is_empty());
        assert!(m.appliquer(&courbe(-1.0)).is_ok());
        assert_eq!(m.appels(), vec![Appel::Appliquer(courbe(-1.0))]);
    }

    #[test]
    fn etat_et_sorties_reglables() {
        let m = MoteurSimule::nouveau();
        m.definir_etat(EtatMoteur::Absent);
        assert_eq!(m.etat(), EtatMoteur::Absent);
        m.definir_sorties(vec![]);
        assert!(m.sorties().unwrap().is_empty());
    }
}
