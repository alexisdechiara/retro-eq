//! Une courbe d'égaliseur dessinée pour `EcranCourbe` : un chemin SVG dans une boîte de
//! 1000 × 240. En x, la fréquence sur une échelle logarithmique (20 Hz à gauche, 20 kHz à
//! droite) ; en y, 10 unités par dB autour du milieu (120), donc ±12 dB visibles.

use retro_dsp::{frequences_log, Courbe};

const LARGEUR: f64 = 1000.0;
const HAUTEUR: f64 = 240.0;
const UNITES_PAR_DB: f64 = 10.0;
const POINTS: usize = 160;

/// Le chemin SVG de la forme de la courbe, **sans** le préampli : il décale tout vers le
/// bas sans changer la forme, et c'est la forme qu'on veut lire.
pub fn chemin_svg(courbe: &Courbe) -> String {
    let frequences = frequences_log(POINTS);
    let mut chemin = String::with_capacity(POINTS * 14);
    for (i, f) in frequences.iter().enumerate() {
        let x = LARGEUR * i as f64 / (POINTS - 1) as f64;
        let y = (HAUTEUR / 2.0 - UNITES_PAR_DB * courbe.reponse_db(*f)).clamp(0.0, HAUTEUR);
        chemin.push_str(if i == 0 { "M " } else { " L " });
        chemin.push_str(&format!("{x:.1} {y:.1}"));
    }
    chemin
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_core::courbe_10_bandes;

    fn points(chemin: &str) -> Vec<(f64, f64)> {
        let nombres: Vec<f64> = chemin
            .split_whitespace()
            .filter_map(|m| m.parse().ok())
            .collect();
        nombres.chunks(2).map(|c| (c[0], c[1])).collect()
    }

    #[test]
    fn une_courbe_plate_est_une_ligne_au_milieu() {
        let p = points(&chemin_svg(&Courbe::default()));
        assert_eq!(p.len(), 160);
        assert_eq!(p[0], (0.0, 120.0));
        assert_eq!(p[159], (1000.0, 120.0));
        assert!(p.iter().all(|&(_, y)| y == 120.0));
    }

    #[test]
    fn un_gain_positif_monte_et_le_preampli_ne_compte_pas() {
        let mut gains = [0.0; 10];
        gains[5] = 6.0; // 1 kHz
        let courbe = courbe_10_bandes(gains).avec_preampli_auto();
        assert!(courbe.preampli_db < -5.0);
        let p = points(&chemin_svg(&courbe));
        let (x, y) = p
            .iter()
            .copied()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0.0, 0.0));
        // 1 kHz est à log(50)/log(1000) = 56,6 % de la largeur ; +6 dB monte de 60 unités.
        assert!((x - 566.0).abs() < 8.0, "{x}");
        assert!((y - 60.0).abs() < 1.0, "{y}");
    }

    #[test]
    fn une_courbe_trop_forte_reste_dans_la_boite() {
        let courbe = courbe_10_bandes([12.0, 12.0, 12.0, 12.0, -12.0, -12.0, -12.0, -12.0, 12.0, 12.0]);
        assert!(points(&chemin_svg(&courbe))
            .iter()
            .all(|&(x, y)| (0.0..=1000.0).contains(&x) && (0.0..=240.0).contains(&y)));
    }
}
