//! Ce que le Visualiseur dit du niveau au-delà de la valeur de l'instant :
//! la moyenne des dernières secondes, et la crête la plus haute atteinte.

use std::collections::VecDeque;

/// Niveau sous lequel on considère qu'il n'y a pas de son.
const SILENCE_DB: f32 = -70.0;

/// Garde les derniers niveaux mesurés pour en tirer une moyenne et une crête maximale.
#[derive(Debug, Clone)]
pub struct SuiviNiveau {
    /// Puissances des dernières mesures. Une moyenne de décibels n'a pas de sens :
    /// on moyenne les puissances, puis on repasse en décibels.
    puissances: VecDeque<f32>,
    capacite: usize,
    crete_max_db: f32,
}

impl SuiviNiveau {
    /// `capacite` : nombre de mesures gardées (30 par seconde × 30 secondes = 900).
    pub fn nouveau(capacite: usize) -> Self {
        Self {
            puissances: VecDeque::with_capacity(capacite.max(1)),
            capacite: capacite.max(1),
            crete_max_db: SILENCE_DB,
        }
    }

    /// Ajoute une mesure. Les silences ne comptent pas dans la moyenne :
    /// une pause dans la musique ne doit pas la faire chuter.
    pub fn ajouter(&mut self, niveau_db: f32, crete_db: f32) {
        if niveau_db > SILENCE_DB {
            if self.puissances.len() == self.capacite {
                self.puissances.pop_front();
            }
            self.puissances.push_back(10f32.powf(niveau_db / 10.0));
        }
        if crete_db > self.crete_max_db {
            self.crete_max_db = crete_db;
        }
    }

    /// Moyenne des dernières mesures, en dB. `None` tant qu'aucun son n'a été capté.
    pub fn moyenne_db(&self) -> Option<f32> {
        if self.puissances.is_empty() {
            return None;
        }
        let somme: f32 = self.puissances.iter().sum();
        Some(10.0 * (somme / self.puissances.len() as f32).log10())
    }

    /// Crête la plus haute depuis la dernière remise à zéro. `None` s'il n'y a rien eu.
    pub fn crete_max_db(&self) -> Option<f32> {
        (self.crete_max_db > SILENCE_DB).then_some(self.crete_max_db)
    }

    pub fn remettre_a_zero(&mut self) {
        self.puissances.clear();
        self.crete_max_db = SILENCE_DB;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn sans_son_il_n_y_a_ni_moyenne_ni_crete() {
        let mut suivi = SuiviNiveau::nouveau(10);
        suivi.ajouter(-120.0, -120.0);
        assert_eq!(suivi.moyenne_db(), None);
        assert_eq!(suivi.crete_max_db(), None);
    }

    #[test]
    fn la_moyenne_est_celle_des_puissances_pas_des_decibels() {
        let mut suivi = SuiviNiveau::nouveau(10);
        suivi.ajouter(-10.0, -8.0);
        suivi.ajouter(-20.0, -18.0);
        // (0,1 + 0,01) / 2 = 0,055, soit −12,6 dB, et non −15.
        let moyenne = suivi.moyenne_db().unwrap();
        assert!((moyenne - -12.596).abs() < 0.01, "{moyenne}");
    }

    #[test]
    fn un_silence_ne_fait_pas_chuter_la_moyenne() {
        let mut suivi = SuiviNiveau::nouveau(10);
        suivi.ajouter(-12.0, -6.0);
        suivi.ajouter(-120.0, -120.0);
        assert!((suivi.moyenne_db().unwrap() - -12.0).abs() < 0.001);
    }

    #[test]
    fn seules_les_dernieres_mesures_comptent() {
        let mut suivi = SuiviNiveau::nouveau(2);
        suivi.ajouter(-3.0, -1.0);
        suivi.ajouter(-30.0, -28.0);
        suivi.ajouter(-30.0, -28.0);
        assert!((suivi.moyenne_db().unwrap() - -30.0).abs() < 0.001);
    }

    #[test]
    fn la_crete_max_reste_jusqu_a_la_remise_a_zero() {
        let mut suivi = SuiviNiveau::nouveau(10);
        suivi.ajouter(-12.0, -2.5);
        suivi.ajouter(-20.0, -15.0);
        assert_eq!(suivi.crete_max_db(), Some(-2.5));
        suivi.remettre_a_zero();
        assert_eq!(suivi.crete_max_db(), None);
        assert_eq!(suivi.moyenne_db(), None);
    }
}
