//! Export pour Wavelet : une ligne `GraphicEQ:` sur la grille de 127 fréquences d'AutoEQ.
//!
//! Wavelet (Android) lit la ligne `GraphicEQ: 20 -1.2; 21 -1.1; …` d'Equalizer APO.
//! Comme AutoEQ, on y plie le préampli, et on empêche le premier point de monter
//! au-dessus de 0 dB (pas de renfort sous 20 Hz).

/// Les 127 fréquences (Hz) de la grille GraphicEQ d'AutoEQ.
///
/// Source : `eqapo_graphic_eq` d'AutoEQ, pas de 1,0563 depuis 20 Hz, valeurs
/// tronquées à l'entier : la dernière est 19871 Hz.
pub const FREQUENCES_HZ: [u32; 127] = [
    20, 21, 22, 23, 24, 26, 27, 29, 30, 32, 34, 36, 38, 40, 43, 45, 48, 50, 53, 56, 59, 63, 66, 70,
    74, 78, 83, 87, 92, 97, 103, 109, 115, 121, 128, 136, 143, 151, 160, 169, 178, 188, 199, 210,
    222, 235, 248, 262, 277, 292, 309, 326, 345, 364, 385, 406, 429, 453, 479, 506, 534, 565, 596,
    630, 665, 703, 743, 784, 829, 875, 924, 977, 1032, 1090, 1151, 1216, 1284, 1357, 1433, 1514,
    1599, 1689, 1784, 1885, 1991, 2103, 2221, 2347, 2479, 2618, 2766, 2921, 3086, 3260, 3443, 3637,
    3842, 4058, 4287, 4528, 4783, 5052, 5337, 5637, 5955, 6290, 6644, 7018, 7414, 7831, 8272, 8738,
    9230, 9749, 10298, 10878, 11490, 12137, 12821, 13543, 14305, 15110, 15961, 16860, 17809, 18812,
    19871,
];

/// Écrit la ligne `GraphicEQ:` d'une courbe.
///
/// `reponse_db(f)` donne la réponse de la courbe à `f` Hz, préampli exclu ;
/// le préampli est ajouté à chaque point. Gains avec une décimale, une seule
/// ligne terminée par un retour à la ligne.
pub fn exporter(reponse_db: impl Fn(f64) -> f64, preampli_db: f64) -> String {
    let paires: Vec<String> = FREQUENCES_HZ
        .iter()
        .enumerate()
        .map(|(rang, &f)| {
            let mut gain = reponse_db(f64::from(f)) + preampli_db;
            if rang == 0 && gain > 0.0 {
                gain = 0.0;
            }
            format!("{f} {}", un_chiffre(gain))
        })
        .collect();
    format!("GraphicEQ: {}\n", paires.join("; "))
}

/// Une décimale, sans « -0.0 ».
fn un_chiffre(gain: f64) -> String {
    let texte = format!("{gain:.1}");
    if texte == "-0.0" {
        "0.0".to_string()
    } else {
        texte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_grille_a_127_points_croissants_de_20_a_19871() {
        assert_eq!(FREQUENCES_HZ.len(), 127);
        assert_eq!(FREQUENCES_HZ[0], 20);
        assert_eq!(FREQUENCES_HZ[126], 19871);
        assert!(FREQUENCES_HZ.windows(2).all(|p| p[0] < p[1]));
    }

    #[test]
    fn la_grille_debute_comme_celle_d_autoeq() {
        assert_eq!(
            FREQUENCES_HZ[..22],
            [
                20, 21, 22, 23, 24, 26, 27, 29, 30, 32, 34, 36, 38, 40, 43, 45, 48, 50, 53, 56, 59,
                63
            ]
        );
    }

    #[test]
    fn la_grille_suit_la_formule_d_autoeq() {
        // AutoEQ : 20 * 1.0563^k, tronqué à l'entier, doublons retirés.
        let mut attendu: Vec<u32> = (0..)
            .map(|k| 20.0 * 1.0563_f64.powi(k))
            .take_while(|f| *f < 20000.0)
            .map(|f| f as u32)
            .collect();
        attendu.dedup();
        assert_eq!(FREQUENCES_HZ.to_vec(), attendu);
    }

    #[test]
    fn une_seule_ligne_avec_127_paires() {
        let texte = exporter(|_| 0.0, 0.0);
        assert_eq!(texte.lines().count(), 1);
        let ligne = texte.lines().next().expect("une ligne");
        let corps = ligne.strip_prefix("GraphicEQ: ").expect("préfixe");
        assert_eq!(corps.split("; ").count(), 127);
        assert!(corps.starts_with("20 0.0; 21 0.0; 22 0.0;"));
        assert!(corps.ends_with("19871 0.0"));
    }

    #[test]
    fn le_preampli_est_ajoute_et_une_decimale() {
        let texte = exporter(|f| if f < 100.0 { -1.0 } else { -2.04 }, -3.0);
        assert!(texte.starts_with("GraphicEQ: 20 -4.0; 21 -4.0;"));
        assert!(texte.contains("; 1032 -5.0;"));
    }

    #[test]
    fn pas_de_moins_zero() {
        let texte = exporter(|_| -0.01, 0.0);
        assert!(!texte.contains("-0.0"));
    }

    #[test]
    fn le_premier_point_ne_monte_pas_au_dessus_de_zero_comme_autoeq() {
        let texte = exporter(|f| if f < 21.0 { 5.0 } else { 1.0 }, 0.0);
        assert!(texte.starts_with("GraphicEQ: 20 0.0; 21 1.0;"));
    }
}
