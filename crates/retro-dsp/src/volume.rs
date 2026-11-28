//! Volume perçu : pondération K, effet d'une courbe sur un spectre, égalisation des volumes.

use crate::biquad::Coefficients;
use crate::courbe::Courbe;
use crate::spectre::Spectre;

/// Fréquence d'échantillonnage pour laquelle les coefficients de la norme sont donnés.
const FE_K: f64 = 48_000.0;

/// Pondération K de la norme ITU-R BS.1770 (en dB) à `f_hz` : un « filtre en forme d'oreille »
/// (légère bosse dans les aigus, coupure des très graves), évalué en fréquence.
/// Les deux étages sont ceux de la norme à 48 kHz.
pub fn ponderation_k_db(f_hz: f64) -> f64 {
    let etage_aigus = Coefficients::normalises(
        [1.53512485958697, -2.69169618940638, 1.19839281085285],
        [1.0, -1.69065929318241, 0.73248077421585],
    );
    let etage_graves =
        Coefficients::normalises([1.0, -2.0, 1.0], [1.0, -1.99004745483398, 0.99007225036621]);
    etage_aigus.gain_db_a(f_hz, FE_K) + etage_graves.gain_db_a(f_hz, FE_K)
}

/// Variation de volume perçu (dB) que `courbe` produit sur `spectre`, préampli compris :
/// `10·log10( Σ P·K·|H|² / Σ P·K )`. Chaque fréquence compte selon sa puissance et son poids K.
pub fn volume_percu_db(courbe: &Courbe, spectre: &Spectre) -> f64 {
    let (mut avec, mut sans) = (0.0, 0.0);
    for (&f, &p) in spectre.frequences_hz.iter().zip(&spectre.puissances) {
        let poids = p * 10f64.powf(ponderation_k_db(f) / 10.0);
        avec += poids * 10f64.powf(courbe.reponse_totale_db(f) / 10.0);
        sans += poids;
    }
    if sans > 0.0 && avec > 0.0 {
        10.0 * (avec / sans).log10()
    } else {
        0.0
    }
}

/// Ce qu'il faut ajouter au préampli de `variante` (dB) pour qu'elle sonne aussi fort que
/// `reference` sur ce spectre.
pub fn compensation_db(reference: &Courbe, variante: &Courbe, spectre: &Spectre) -> f64 {
    volume_percu_db(reference, spectre) - volume_percu_db(variante, spectre)
}

/// Met la variante au volume de la référence, puis baisse les deux du même nombre de dB si
/// l'une dépasse 0 dB : le test compare ainsi des sons de même volume, sans saturation.
/// Retourne `(référence, variante)`.
pub fn egaliser_volumes(
    reference: &Courbe,
    variante: &Courbe,
    spectre: &Spectre,
) -> (Courbe, Courbe) {
    let mut reference = reference.clone();
    let mut variante = variante.clone();
    variante.preampli_db += compensation_db(&reference, &variante, spectre);
    let depassement = [&reference, &variante]
        .iter()
        .map(|c| c.gain_max_db() + c.preampli_db)
        .fold(0.0, f64::max);
    reference.preampli_db -= depassement;
    variante.preampli_db -= depassement;
    (reference, variante)
}

/// En dessous de ce volume (dB), une mesure est du silence et ne compte pas : c'est la porte
/// absolue de la mesure LUFS (ITU-R BS.1770). Un silence ne dit rien de l'égaliseur.
pub const PORTE_SILENCE_DB: f64 = -70.0;

/// Le volume perçu **mesuré** sur ce qui joue, cumulé mesure après mesure (ADR-0006 : la
/// capture sert à afficher l'écart mesuré, pas à régler le volume).
///
/// La moyenne se fait sur les puissances, pas sur les dB, comme une mesure LUFS intégrée :
/// une explosion pèse plus qu'un passage calme, ce qui ressemble à ce que l'oreille retient.
/// `Copy` : deux nombres, qu'on peut recopier sans `clone()`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VolumeIntegre {
    somme_puissances: f64,
    mesures: u32,
}

impl VolumeIntegre {
    /// Ajoute une mesure de volume perçu (dB). Le silence (sous [`PORTE_SILENCE_DB`]) et les
    /// valeurs non finies sont ignorés.
    pub fn ajouter(&mut self, volume_db: f64) {
        if !volume_db.is_finite() || volume_db < PORTE_SILENCE_DB {
            return;
        }
        self.somme_puissances += 10f64.powf(volume_db / 10.0);
        self.mesures += 1;
    }

    /// Nombre de mesures retenues.
    pub fn mesures(&self) -> u32 {
        self.mesures
    }

    /// Le volume moyen (dB), ou rien si aucune mesure n'a été retenue.
    pub fn moyenne_db(&self) -> Option<f64> {
        (self.mesures > 0).then(|| 10.0 * (self.somme_puissances / f64::from(self.mesures)).log10())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::courbe::{Filtre, TypeFiltre};

    fn filtre(t: TypeFiltre, hz: f64, g: f64, q: f64) -> Filtre {
        Filtre {
            type_filtre: t,
            frequence_hz: hz,
            gain_db: g,
            q,
        }
    }

    #[test]
    fn k_vaut_a_peu_pres_zero_a_1_khz() {
        assert!(
            ponderation_k_db(1000.0).abs() < 0.8,
            "{}",
            ponderation_k_db(1000.0)
        );
    }

    #[test]
    fn k_monte_de_4_db_a_10_khz() {
        let k = ponderation_k_db(10_000.0);
        assert!((k - 4.0).abs() < 0.3, "{k}");
    }

    #[test]
    fn k_coupe_fortement_les_tres_graves() {
        assert!(ponderation_k_db(30.0) < -8.0, "{}", ponderation_k_db(30.0));
        assert!(ponderation_k_db(30.0) < ponderation_k_db(100.0));
    }

    #[test]
    fn un_simple_preampli_change_le_volume_de_autant() {
        let c = Courbe {
            preampli_db: 3.0,
            filtres: vec![],
        };
        let v = volume_percu_db(&c, &Spectre::rose(500));
        assert!((v - 3.0).abs() < 1e-9, "{v}");
        assert!(volume_percu_db(&Courbe::default(), &Spectre::rose(500)).abs() < 1e-9);
    }

    #[test]
    fn monter_les_aigus_est_plus_fort_que_monter_les_infra_graves() {
        let s = Spectre::rose(500);
        let aigus = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 6000.0, 6.0, 1.0)],
        };
        let infra = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 30.0, 6.0, 1.0)],
        };
        assert!(volume_percu_db(&aigus, &s) > volume_percu_db(&infra, &s));
    }

    #[test]
    fn la_compensation_egalise_les_volumes_percus() {
        let s = Spectre::rose(500);
        let reference = Courbe::default();
        let variante = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre(TypeFiltre::PlateauBas, 120.0, 5.0, 0.71)],
        };
        let c = compensation_db(&reference, &variante, &s);
        assert!(c < 0.0);
        let mut v2 = variante.clone();
        v2.preampli_db += c;
        assert!((volume_percu_db(&v2, &s) - volume_percu_db(&reference, &s)).abs() < 1e-9);
    }

    #[test]
    fn egaliser_volumes_egalise_et_ne_depasse_pas_zero_db() {
        let s = Spectre::rose(500);
        let reference = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 2000.0, 2.0, 1.0)],
        };
        let variante = Courbe {
            preampli_db: 0.0,
            filtres: vec![
                filtre(TypeFiltre::Cloche, 2000.0, 2.0, 1.0),
                filtre(TypeFiltre::PlateauHaut, 8000.0, 6.0, 0.71),
            ],
        };
        let (r, v) = egaliser_volumes(&reference, &variante, &s);
        assert!((volume_percu_db(&r, &s) - volume_percu_db(&v, &s)).abs() < 0.01);
        for c in [&r, &v] {
            assert!(c.gain_max_db() + c.preampli_db <= 1e-9);
        }
        // Les filtres ne sont pas touchés.
        assert_eq!(r.filtres, reference.filtres);
        assert_eq!(v.filtres, variante.filtres);
    }

    #[test]
    fn un_volume_integre_vide_n_a_pas_de_moyenne() {
        let v = VolumeIntegre::default();
        assert_eq!(v.mesures(), 0);
        assert_eq!(v.moyenne_db(), None);
    }

    #[test]
    fn le_volume_integre_fait_la_moyenne_des_puissances() {
        let mut v = VolumeIntegre::default();
        v.ajouter(-20.0);
        v.ajouter(-20.0);
        assert!((v.moyenne_db().unwrap() + 20.0).abs() < 1e-9);
        v.ajouter(-30.0);
        // (0,01 + 0,01 + 0,001) / 3 en puissance : la mesure forte pèse plus qu'en dB.
        let attendu = 10.0 * (0.021f64 / 3.0).log10();
        assert!((v.moyenne_db().unwrap() - attendu).abs() < 1e-9);
        assert_eq!(v.mesures(), 3);
    }

    #[test]
    fn le_silence_et_les_valeurs_absurdes_ne_comptent_pas() {
        let mut v = VolumeIntegre::default();
        v.ajouter(-90.0);
        v.ajouter(f64::NAN);
        v.ajouter(f64::NEG_INFINITY);
        assert_eq!(v.mesures(), 0);
        v.ajouter(PORTE_SILENCE_DB + 1.0);
        assert_eq!(v.mesures(), 1);
    }

    #[test]
    fn egaliser_volumes_laisse_deux_courbes_identiques_identiques() {
        let s = Spectre::rose(100);
        let c = Courbe {
            preampli_db: -1.0,
            filtres: vec![filtre(TypeFiltre::Cloche, 500.0, -3.0, 1.0)],
        };
        let (r, v) = egaliser_volumes(&c, &c, &s);
        assert_eq!(r, v);
        assert_eq!(r, c);
    }
}
