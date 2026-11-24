//! Le spectre : la puissance d'un signal, fréquence par fréquence.

use crate::courbe::{frequences_log, Courbe};

/// Puissance linéaire (pas en dB) d'un signal en une liste de fréquences.
///
/// Les deux vecteurs ont la même longueur ; la grille est le plus souvent logarithmique,
/// donc chaque point pèse autant qu'un autre dans une somme.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectre {
    pub frequences_hz: Vec<f64>,
    pub puissances: Vec<f64>,
}

impl Spectre {
    /// Bruit rose sur `n` points de 20 Hz à 20 kHz : même puissance par octave, donc même
    /// puissance par point sur une grille logarithmique. C'est le spectre par défaut.
    pub fn rose(n: usize) -> Spectre {
        Spectre {
            frequences_hz: frequences_log(n),
            puissances: vec![1.0; n],
        }
    }

    /// Ce spectre débarrassé de l'effet de `courbe` (préampli compris).
    ///
    /// Sert quand la capture se fait après le moteur : on retrouve le signal d'origine.
    pub fn divise_par(&self, courbe: &Courbe) -> Spectre {
        let puissances = self
            .frequences_hz
            .iter()
            .zip(&self.puissances)
            .map(|(&f, &p)| p / 10f64.powf(courbe.reponse_totale_db(f) / 10.0))
            .collect();
        Spectre {
            frequences_hz: self.frequences_hz.clone(),
            puissances,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::courbe::{Filtre, TypeFiltre};

    #[test]
    fn le_bruit_rose_a_la_meme_puissance_par_octave() {
        let s = Spectre::rose(960);
        assert_eq!(s.frequences_hz.len(), 960);
        assert_eq!(s.puissances.len(), 960);
        let somme = |lo: f64, hi: f64| -> f64 {
            s.frequences_hz
                .iter()
                .zip(&s.puissances)
                .filter(|(&f, _)| f >= lo && f < hi)
                .map(|(_, p)| p)
                .sum()
        };
        let a = somme(100.0, 200.0);
        let b = somme(1000.0, 2000.0);
        assert!((a / b - 1.0).abs() < 0.03, "{a} {b}");
    }

    #[test]
    fn diviser_par_la_courbe_annule_son_effet() {
        let courbe = Courbe {
            preampli_db: -3.0,
            filtres: vec![Filtre {
                type_filtre: TypeFiltre::Cloche,
                frequence_hz: 1000.0,
                gain_db: 6.0,
                q: 1.0,
            }],
        };
        let s = Spectre::rose(50);
        let d = s.divise_par(&courbe);
        for ((&f, &p), &q) in d.frequences_hz.iter().zip(&d.puissances).zip(&s.puissances) {
            let db = 10.0 * (q / p).log10();
            assert!((db - courbe.reponse_totale_db(f)).abs() < 1e-9);
        }
    }

    #[test]
    fn diviser_par_une_courbe_plate_ne_change_rien() {
        let s = Spectre::rose(20);
        assert_eq!(s.divise_par(&Courbe::default()), s);
    }
}
