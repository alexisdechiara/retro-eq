//! Les types partagés par tout le workspace : un filtre, et une courbe faite de filtres.

use serde::{Deserialize, Serialize};

/// La forme d'un filtre d'égaliseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeFiltre {
    /// Cloche centrée sur une fréquence (« PK » chez Equalizer APO).
    Cloche,
    /// Plateau qui monte ou baisse tout ce qui est sous la fréquence (« LSC »).
    PlateauBas,
    /// Plateau qui monte ou baisse tout ce qui est au-dessus de la fréquence (« HSC »).
    PlateauHaut,
}

/// Un filtre d'égaliseur paramétrique.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Filtre {
    pub type_filtre: TypeFiltre,
    pub frequence_hz: f64,
    pub gain_db: f64,
    /// Finesse du filtre : plus Q est grand, plus le filtre est étroit.
    pub q: f64,
}

/// Un réglage d'égaliseur complet : un préampli et une liste de filtres.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Courbe {
    pub preampli_db: f64,
    pub filtres: Vec<Filtre>,
}

use crate::biquad::{
    Coefficients, FREQUENCES_ECHANTILLONNAGE_COURANTES, FREQUENCE_ECHANTILLONNAGE,
};

/// Plus basse fréquence de la bande audible que l'on regarde.
pub const FREQUENCE_MIN_HZ: f64 = 20.0;
/// Plus haute fréquence de la bande audible que l'on regarde.
pub const FREQUENCE_MAX_HZ: f64 = 20_000.0;
/// Densité de la grille qui cherche le gain maximum.
const POINTS_PAR_OCTAVE: f64 = 96.0;

/// `n` fréquences de 20 Hz à 20 kHz, espacées régulièrement en échelle logarithmique
/// (celle de l'oreille : chaque octave compte autant).
pub fn frequences_log(n: usize) -> Vec<f64> {
    match n {
        0 => Vec::new(),
        1 => vec![FREQUENCE_MIN_HZ],
        _ => {
            let rapport = FREQUENCE_MAX_HZ / FREQUENCE_MIN_HZ;
            (0..n)
                .map(|i| FREQUENCE_MIN_HZ * rapport.powf(i as f64 / (n - 1) as f64))
                .collect()
        }
    }
}

impl Filtre {
    /// Gain du filtre (dB) à `f_hz`, d'après le biquad du Cookbook à 48 kHz : c'est ce que
    /// le moteur fait réellement entendre.
    pub fn gain_db_a(&self, f_hz: f64) -> f64 {
        Coefficients::depuis(self, FREQUENCE_ECHANTILLONNAGE)
            .gain_db_a(f_hz, FREQUENCE_ECHANTILLONNAGE)
    }
}

impl Courbe {
    /// Réponse des filtres seuls (dB) à `f_hz`, sans le préampli : les gains en dB s'additionnent.
    pub fn reponse_db(&self, f_hz: f64) -> f64 {
        self.filtres.iter().map(|f| f.gain_db_a(f_hz)).sum()
    }

    /// Réponse complète (dB) à `f_hz`, préampli compris : ce qui sort vraiment du moteur.
    pub fn reponse_totale_db(&self, f_hz: f64) -> f64 {
        self.preampli_db + self.reponse_db(f_hz)
    }

    /// Gain maximum des filtres (dB) entre 20 Hz et 20 kHz, cherché sur une grille dense, et
    /// pris sur la plus haute des fréquences d'échantillonnage courantes
    /// ([`FREQUENCES_ECHANTILLONNAGE_COURANTES`]) : le préampli qui en découle évite la
    /// saturation quelle que soit celle de la sortie audio. La courbe affichée, elle, reste
    /// celle de 48 kHz ([`Courbe::reponse_db`]).
    pub fn gain_max_db(&self) -> f64 {
        let octaves = (FREQUENCE_MAX_HZ / FREQUENCE_MIN_HZ).log2();
        let n = (octaves * POINTS_PAR_OCTAVE).ceil() as usize + 1;
        let grille = frequences_log(n);
        FREQUENCES_ECHANTILLONNAGE_COURANTES
            .iter()
            .map(|&fe| {
                let coefficients: Vec<Coefficients> = self
                    .filtres
                    .iter()
                    .map(|f| Coefficients::depuis(f, fe))
                    .collect();
                let reponse = |hz: f64| -> f64 {
                    coefficients
                        .iter()
                        .map(|c| c.gain_db_a(hz, fe))
                        .sum::<f64>()
                };
                let (rang, max_grille) = grille.iter().map(|&hz| reponse(hz)).enumerate().fold(
                    (0, f64::NEG_INFINITY),
                    |m, (i, g)| {
                        if g > m.1 {
                            (i, g)
                        } else {
                            m
                        }
                    },
                );
                // Une crête étroite près de Nyquist peut tomber entre deux points de la
                // grille : on l'affine entre les deux voisins du meilleur point.
                let (mut bas, mut haut) = (
                    grille[rang.saturating_sub(1)],
                    grille[(rang + 1).min(grille.len() - 1)],
                );
                for _ in 0..60 {
                    let (m1, m2) = (bas + (haut - bas) / 3.0, haut - (haut - bas) / 3.0);
                    if reponse(m1) < reponse(m2) {
                        bas = m1;
                    } else {
                        haut = m2;
                    }
                }
                max_grille.max(reponse((bas + haut) / 2.0))
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Le plus petit préampli (dB, négatif ou nul) qui évite que le son sature.
    pub fn preampli_auto_db(&self) -> f64 {
        -self.gain_max_db().max(0.0)
    }

    /// Cette courbe avec son préampli remplacé par le préampli automatique.
    pub fn avec_preampli_auto(mut self) -> Courbe {
        self.preampli_db = self.preampli_auto_db();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(t: TypeFiltre, hz: f64, g: f64, q: f64) -> Filtre {
        Filtre {
            type_filtre: t,
            frequence_hz: hz,
            gain_db: g,
            q,
        }
    }

    fn courbe(filtres: Vec<Filtre>) -> Courbe {
        Courbe {
            preampli_db: 0.0,
            filtres,
        }
    }

    #[test]
    fn la_cloche_atteint_son_gain_au_centre_et_rien_loin() {
        let c = f(TypeFiltre::Cloche, 1000.0, 6.0, 1.41);
        assert!((c.gain_db_a(1000.0) - 6.0).abs() < 1e-6);
        assert!(c.gain_db_a(20.0).abs() < 0.1);
        assert!(c.gain_db_a(20_000.0).abs() < 0.5);
    }

    #[test]
    fn le_plateau_bas_atteint_son_gain_dans_les_graves_et_rien_dans_les_aigus() {
        let c = f(TypeFiltre::PlateauBas, 200.0, 6.0, 0.71);
        assert!((c.gain_db_a(20.0) - 6.0).abs() < 0.2);
        assert!((c.gain_db_a(1.0) - 6.0).abs() < 0.01);
        assert!(c.gain_db_a(10_000.0).abs() < 0.1);
        // Au milieu de la pente, le plateau donne la moitié du gain.
        assert!((c.gain_db_a(200.0) - 3.0).abs() < 0.2);
    }

    #[test]
    fn le_plateau_haut_atteint_son_gain_dans_les_aigus_et_rien_dans_les_graves() {
        let c = f(TypeFiltre::PlateauHaut, 4000.0, -6.0, 0.71);
        assert!((c.gain_db_a(20_000.0) + 6.0).abs() < 0.8);
        assert!(c.gain_db_a(100.0).abs() < 0.1);
    }

    #[test]
    fn plateau_haut_symetrique_du_plateau_bas() {
        let b = f(TypeFiltre::PlateauBas, 1000.0, 5.0, 0.71);
        let h = f(TypeFiltre::PlateauHaut, 1000.0, 5.0, 0.71);
        // Loin de Nyquist, le plateau haut en 2 kHz vaut le plateau bas en 500 Hz inversé.
        assert!((b.gain_db_a(100.0) - 5.0).abs() < 0.5);
        assert!((h.gain_db_a(10_000.0) - 5.0).abs() < 0.8);
    }

    #[test]
    fn courbe_plate_donne_zero_partout() {
        let c = Courbe::default();
        for hz in frequences_log(50) {
            assert_eq!(c.reponse_db(hz), 0.0);
            assert_eq!(c.reponse_totale_db(hz), 0.0);
        }
        assert_eq!(c.preampli_auto_db(), 0.0);
    }

    #[test]
    fn la_reponse_additionne_les_filtres_et_totale_ajoute_le_preampli() {
        let a = f(TypeFiltre::Cloche, 1000.0, 3.0, 1.0);
        let b = f(TypeFiltre::Cloche, 1000.0, 2.0, 1.0);
        let c = Courbe {
            preampli_db: -4.0,
            filtres: vec![a, b],
        };
        assert!((c.reponse_db(1000.0) - 5.0).abs() < 1e-6);
        assert!((c.reponse_totale_db(1000.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn preampli_auto_compense_un_gain_et_ignore_une_coupe() {
        let haut = courbe(vec![f(TypeFiltre::Cloche, 1000.0, 5.0, 1.0)]);
        assert!((haut.gain_max_db() - 5.0).abs() < 0.01);
        assert!((haut.preampli_auto_db() + 5.0).abs() < 0.01);
        let bas = courbe(vec![f(TypeFiltre::Cloche, 1000.0, -5.0, 1.0)]);
        assert_eq!(bas.preampli_auto_db(), 0.0);
    }

    #[test]
    fn avec_preampli_auto_evite_de_depasser_zero_db() {
        let c = courbe(vec![
            f(TypeFiltre::PlateauBas, 80.0, 4.0, 0.71),
            f(TypeFiltre::Cloche, 3000.0, 3.0, 1.0),
        ])
        .avec_preampli_auto();
        assert!(c.preampli_db < -3.9);
        for hz in frequences_log(500) {
            assert!(c.reponse_totale_db(hz) <= 0.01);
        }
    }

    /// Réponse (dB, préampli compris) de la courbe calculée à la fréquence d'échantillonnage `fe`.
    fn reponse_totale_a(c: &Courbe, f_hz: f64, fe: f64) -> f64 {
        c.preampli_db
            + c.filtres
                .iter()
                .map(|filtre| Coefficients::depuis(filtre, fe).gain_db_a(f_hz, fe))
                .sum::<f64>()
    }

    #[test]
    fn le_preampli_auto_evite_la_saturation_a_44_1_48_et_96_khz() {
        // Mesuré : à 48 kHz la crête vaut 11,18 dB, à 44,1 kHz 11,54 dB. Equalizer APO calcule
        // ses filtres à la vraie fréquence de la sortie audio : le préampli doit couvrir les deux.
        let c = courbe(vec![
            f(TypeFiltre::PlateauHaut, 14_000.0, 6.0, 0.5),
            f(TypeFiltre::Cloche, 19_500.0, 6.0, 2.0),
        ])
        .avec_preampli_auto();
        assert!((c.preampli_db + 11.546).abs() < 0.01, "{}", c.preampli_db);
        for fe in [44_100.0, 48_000.0, 96_000.0] {
            for hz in frequences_log(2000) {
                let r = reponse_totale_a(&c, hz, fe);
                assert!(r <= 1e-3, "{r} dB à {hz} Hz pour {fe} Hz");
            }
        }
    }

    #[test]
    fn la_courbe_affichee_reste_celle_de_48_khz() {
        let c = courbe(vec![
            f(TypeFiltre::PlateauHaut, 14_000.0, 6.0, 0.5),
            f(TypeFiltre::Cloche, 19_500.0, 6.0, 2.0),
        ]);
        for hz in frequences_log(100) {
            assert_eq!(c.reponse_db(hz), reponse_totale_a(&c, hz, 48_000.0));
        }
    }

    #[test]
    fn frequences_log_va_de_20_hz_a_20_khz_regulierement() {
        let v = frequences_log(100);
        assert_eq!(v.len(), 100);
        assert!((v[0] - 20.0).abs() < 1e-9);
        assert!((v[99] - 20_000.0).abs() < 1e-6);
        let r = v[1] / v[0];
        assert!(v.windows(2).all(|w| (w[1] / w[0] - r).abs() < 1e-9));
        assert!(frequences_log(0).is_empty());
        assert_eq!(frequences_log(1), vec![20.0]);
    }

    #[test]
    fn la_cloche_correspond_a_la_formule_du_cookbook() {
        // Valeur de référence calculée à la main : +6 dB, 1 kHz, Q 1 → à 500 Hz ≈ +1,6 dB.
        let c = f(TypeFiltre::Cloche, 1000.0, 6.0, 1.0);
        let g = c.gain_db_a(500.0);
        assert!(g > 1.0 && g < 2.5, "{g}");
    }
}
