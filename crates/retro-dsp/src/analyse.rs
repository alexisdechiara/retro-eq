//! Analyse d'un signal : niveau RMS, spectre par FFT, regroupement en bandes, lissage.

use crate::courbe::{frequences_log, FREQUENCE_MAX_HZ, FREQUENCE_MIN_HZ};
use crate::spectre::Spectre;
use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::f32::consts::PI;
use std::sync::Arc;

/// Niveau le plus bas représenté (dBFS) : sous ce seuil on parle de silence.
pub const PLANCHER_DB: f32 = -120.0;

/// Niveau efficace (RMS) d'un signal en dBFS ; 0 dBFS = sinus d'amplitude 1 ne vaut que
/// −3,01 dBFS (le RMS d'un sinus est amplitude/√2). Silence ou vide : −120.
pub fn niveau_rms_db(echantillons: &[f32]) -> f32 {
    if echantillons.is_empty() {
        return PLANCHER_DB;
    }
    let somme: f64 = echantillons.iter().map(|&x| f64::from(x).powi(2)).sum();
    let rms = (somme / echantillons.len() as f64).sqrt();
    ((20.0 * rms.log10()) as f32).max(PLANCHER_DB)
}

/// Calcule le spectre d'un bloc d'échantillons par FFT, avec une fenêtre de Hann
/// (qui adoucit les bords du bloc pour éviter les fausses raies).
pub struct Analyseur {
    taille: usize,
    fe: f32,
    fenetre: Vec<f32>,
    gain_fenetre: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    entree: Vec<f32>,
    sortie: Vec<Complex<f32>>,
}

impl Analyseur {
    /// Prépare une FFT de `taille_fft` points (une puissance de 2 est le plus rapide) pour
    /// un signal échantillonné à `frequence_echantillonnage` Hz.
    pub fn new(taille_fft: usize, frequence_echantillonnage: f32) -> Self {
        let taille = taille_fft.max(2);
        let fenetre: Vec<f32> = (0..taille)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / taille as f32).cos())
            .collect();
        let gain_fenetre = fenetre.iter().sum();
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(taille);
        let entree = fft.make_input_vec();
        let sortie = fft.make_output_vec();
        Self {
            taille,
            fe: frequence_echantillonnage,
            fenetre,
            gain_fenetre,
            fft,
            entree,
            sortie,
        }
    }

    /// Nombre d'échantillons analysés d'un coup.
    pub fn taille(&self) -> usize {
        self.taille
    }

    /// Fréquence (Hz) du bin `i` du spectre rendu.
    pub fn frequence_bin(&self, i: usize) -> f32 {
        i as f32 * self.fe / self.taille as f32
    }

    /// Magnitudes (dBFS) des `taille/2 + 1` bins ; un sinus plein de 1 kHz tombant pile sur
    /// un bin donne 0 dBFS. On garde les `taille` derniers échantillons ; s'il y en a moins,
    /// on complète par du silence.
    pub fn spectre(&mut self, echantillons: &[f32]) -> Vec<f32> {
        let utiles = if echantillons.len() > self.taille {
            &echantillons[echantillons.len() - self.taille..]
        } else {
            echantillons
        };
        self.entree.fill(0.0);
        for ((e, &x), &w) in self.entree.iter_mut().zip(utiles).zip(&self.fenetre) {
            *e = x * w;
        }
        // Les tailles sont celles qu'a préparées `new`, donc l'erreur ne peut pas arriver ;
        // en cas de doute, on rend un spectre de silence plutôt que de planter l'appli.
        if self
            .fft
            .process(&mut self.entree, &mut self.sortie)
            .is_err()
        {
            return vec![PLANCHER_DB; self.sortie.len()];
        }
        // ×2 : l'énergie se répartit sur les fréquences positives et négatives.
        let echelle = 2.0 / self.gain_fenetre;
        self.sortie
            .iter()
            .map(|c| (20.0 * (c.norm() * echelle).log10()).max(PLANCHER_DB))
            .collect()
    }

    /// Convertit des magnitudes en dBFS (sortie de `spectre`) en `Spectre` à `n` points
    /// logarithmiques, pour le calcul de compensation.
    ///
    /// Le bin `k` couvre les fréquences `(k−½)·fe/N` à `(k+½)·fe/N` et sa puissance est
    /// répartie entre les bandes qui le recoupent, au prorata du recouvrement. Chaque
    /// puissance n'est ainsi comptée qu'une fois (la puissance totale est conservée), que
    /// la bande soit plus large qu'un bin (aigus) ou plus étroite (graves) : un bruit rose
    /// donne une puissance égale dans toutes les bandes.
    pub fn vers_spectre(&self, magnitudes_db: &[f32], n: usize) -> Spectre {
        let frequences_hz = frequences_log(n);
        let mut puissances = vec![0.0; n];
        if magnitudes_db.is_empty() || n == 0 {
            return Spectre {
                frequences_hz,
                puissances,
            };
        }
        let largeur_bin = f64::from(self.fe) / self.taille as f64;
        // Densité de puissance de chaque bin (puissance par Hz).
        let densite: Vec<f64> = magnitudes_db
            .iter()
            .map(|&m| 10f64.powf(f64::from(m) / 10.0) / largeur_bin)
            .collect();
        let dernier = magnitudes_db.len() - 1;
        // Demi-pas de la grille, pour que les points d'extrémité aient une bande entière.
        let demi_pas = (FREQUENCE_MAX_HZ / FREQUENCE_MIN_HZ).powf(0.5 / (n.max(2) - 1) as f64);
        for (k, &f) in frequences_hz.iter().enumerate() {
            // Une bande va du milieu géométrique avec le point précédent à celui avec le suivant.
            let lo = if k == 0 {
                f / demi_pas
            } else {
                (f * frequences_hz[k - 1]).sqrt()
            };
            let hi = if k + 1 == n {
                f * demi_pas
            } else {
                (f * frequences_hz[k + 1]).sqrt()
            };
            // Seuls les bins dont l'étendue touche [lo, hi] comptent.
            let premier = ((lo / largeur_bin - 0.5).floor().max(0.0)) as usize;
            let dernier_utile = (((hi / largeur_bin + 0.5).ceil()).max(0.0) as usize).min(dernier);
            for i in premier..=dernier_utile {
                let debut = ((i as f64 - 0.5) * largeur_bin).max(lo);
                let fin = ((i as f64 + 0.5) * largeur_bin).min(hi);
                if fin > debut {
                    puissances[k] += puissance_dans_bin(&densite, i, largeur_bin, debut, fin);
                }
            }
        }
        Spectre {
            frequences_hz,
            puissances,
        }
    }
}

/// Puissance du bin `i` qui tombe entre `debut` et `fin` (Hz, dans l'étendue du bin).
///
/// Dans une bande plus étroite qu'un bin, répartir la puissance à plat ferait des marches
/// d'escalier (jusqu'à 1 dB sur un bruit rose dans les graves). On donne donc au bin une
/// densité qui suit la ligne brisée passant par les centres des bins voisins, puis on la
/// remet à l'échelle pour que le bin entier garde exactement sa puissance : rien n'est
/// compté deux fois, rien ne passe dans un bin voisin, et la densité reste positive.
fn puissance_dans_bin(densite: &[f64], i: usize, largeur: f64, debut: f64, fin: f64) -> f64 {
    let centre = densite[i];
    // Aux extrémités du spectre on prolonge à plat.
    let gauche = if i > 0 { densite[i - 1] } else { centre };
    let droite = densite.get(i + 1).copied().unwrap_or(centre);
    let moyenne = (gauche + 6.0 * centre + droite) / 8.0;
    if moyenne <= 0.0 {
        return 0.0;
    }
    let echelle = centre / moyenne;
    let x_centre = i as f64 * largeur;
    // Intégrale de la densité linéaire entre `a` et `b` d'un même côté du centre.
    let morceau = |a: f64, b: f64, voisin: f64| {
        if b <= a {
            return 0.0;
        }
        let d = |x: f64| centre + (voisin - centre) * (x - x_centre).abs() / largeur;
        (d(a) + d(b)) / 2.0 * (b - a)
    };
    let integrale =
        morceau(debut, fin.min(x_centre), gauche) + morceau(debut.max(x_centre), fin, droite);
    echelle * integrale
}

/// Regroupe un spectre en `n_bandes` bandes logarithmiques de 20 Hz à 20 kHz (ou Nyquist)
/// pour l'affichage : chaque bande prend le niveau le plus fort de ses bins (les crêtes
/// restent visibles), ou une interpolation quand elle est plus étroite qu'un bin.
pub fn bandes_log(magnitudes_db: &[f32], fe: f32, n_bandes: usize) -> Vec<f32> {
    if magnitudes_db.len() < 2 || n_bandes == 0 {
        return vec![PLANCHER_DB; n_bandes];
    }
    let dernier = magnitudes_db.len() - 1;
    let largeur_bin = f64::from(fe) / (2.0 * dernier as f64);
    let haut = FREQUENCE_MAX_HZ.min(f64::from(fe) / 2.0);
    let rapport = (haut / FREQUENCE_MIN_HZ).powf(1.0 / n_bandes as f64);
    (0..n_bandes)
        .map(|k| {
            let lo = FREQUENCE_MIN_HZ * rapport.powi(k as i32);
            let hi = lo * rapport;
            let i_lo = (lo / largeur_bin).ceil() as usize;
            let i_hi = ((hi / largeur_bin).floor() as usize).min(dernier);
            if i_lo <= i_hi {
                magnitudes_db[i_lo..=i_hi]
                    .iter()
                    .copied()
                    .fold(PLANCHER_DB, f32::max)
            } else {
                let pos = ((lo * hi).sqrt() / largeur_bin).min(dernier as f64);
                let (i0, i1) = (pos.floor() as usize, (pos.ceil() as usize).min(dernier));
                let t = (pos - pos.floor()) as f32;
                magnitudes_db[i0] * (1.0 - t) + magnitudes_db[i1] * t
            }
        })
        .collect()
}

/// Lissage exponentiel : chaque valeur se rapproche de la nouvelle mesure d'une fraction
/// `alpha`. Évite que l'affichage (ou le spectre de compensation) saute à chaque bloc.
#[derive(Debug, Clone)]
pub struct Lissage {
    alpha: f32,
    etat: Vec<f32>,
}

impl Lissage {
    /// `alpha` entre 0 (figé) et 1 (aucun lissage) : poids de la nouvelle mesure.
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            etat: Vec::new(),
        }
    }

    /// Mélange `valeurs` à l'état lissé et rend le nouvel état. La première mesure (ou
    /// une mesure de taille différente) sert de point de départ.
    pub fn appliquer(&mut self, valeurs: &[f32]) -> &[f32] {
        if self.etat.len() != valeurs.len() {
            self.etat = valeurs.to_vec();
        } else {
            for (e, &v) in self.etat.iter_mut().zip(valeurs) {
                *e += self.alpha * (v - *e);
            }
        }
        &self.etat
    }

    /// Oublie l'historique.
    pub fn reinitialiser(&mut self) {
        self.etat.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sinus(f: f32, fe: f32, n: usize, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (2.0 * PI * f * i as f32 / fe).sin())
            .collect()
    }

    #[test]
    fn rms_dun_sinus_plein_vaut_moins_3_dbfs() {
        let x = sinus(1000.0, 48_000.0, 48_000, 1.0);
        assert!((niveau_rms_db(&x) + 3.01).abs() < 0.01);
    }

    #[test]
    fn rms_du_silence_et_du_vide_vaut_le_plancher() {
        assert_eq!(niveau_rms_db(&[0.0; 100]), -120.0);
        assert_eq!(niveau_rms_db(&[]), -120.0);
    }

    #[test]
    fn rms_dun_signal_continu_plein_vaut_zero() {
        assert!(niveau_rms_db(&[1.0; 64]).abs() < 1e-6);
    }

    #[test]
    fn la_fft_trouve_le_pic_dun_sinus_de_1_khz_au_bon_niveau() {
        // 1 kHz = bin 128 pour 6144 points à 48 kHz → on prend une taille où le bin tombe juste.
        let (fe, n) = (48_000.0, 4800);
        let mut a = Analyseur::new(n, fe);
        let spectre = a.spectre(&sinus(1000.0, fe, n, 1.0));
        assert_eq!(spectre.len(), n / 2 + 1);
        let (bin, niveau) = spectre
            .iter()
            .copied()
            .enumerate()
            .fold((0, f32::MIN), |m, (i, v)| if v > m.1 { (i, v) } else { m });
        assert!((a.frequence_bin(bin) - 1000.0).abs() < 1.0);
        assert!(niveau.abs() < 0.1, "{niveau}");
        // Loin du pic, la fenêtre de Hann rend le spectre très bas.
        assert!(spectre[bin + 20] < -80.0);
    }

    #[test]
    fn le_niveau_suit_lamplitude() {
        let (fe, n) = (48_000.0, 4800);
        let mut a = Analyseur::new(n, fe);
        let s = a.spectre(&sinus(1000.0, fe, n, 0.5));
        let pic = s.iter().copied().fold(f32::MIN, f32::max);
        assert!((pic + 6.02).abs() < 0.1, "{pic}");
    }

    #[test]
    fn un_bloc_court_est_complete_par_du_silence_et_un_long_est_tronque() {
        let mut a = Analyseur::new(1024, 48_000.0);
        assert_eq!(a.spectre(&[0.1; 10]).len(), 513);
        assert_eq!(a.spectre(&[0.1; 5000]).len(), 513);
        assert!(a.spectre(&[]).iter().all(|&v| v == -120.0));
    }

    #[test]
    fn bandes_log_garde_la_crete_dans_la_bonne_bande() {
        let (fe, n) = (48_000.0, 4800);
        let mut a = Analyseur::new(n, fe);
        let s = a.spectre(&sinus(1000.0, fe, n, 1.0));
        let b = bandes_log(&s, fe, 30);
        assert_eq!(b.len(), 30);
        let (k, niveau) = b
            .iter()
            .copied()
            .enumerate()
            .fold((0, f32::MIN), |m, (i, v)| if v > m.1 { (i, v) } else { m });
        let rapport = (20_000f64 / 20.0).powf(1.0 / 30.0);
        let centre = 20.0 * rapport.powf(k as f64 + 0.5);
        assert!((centre / 1000.0).log2().abs() < 0.2, "{centre}");
        assert!(niveau > -1.0);
    }

    #[test]
    fn bandes_log_remplit_toutes_les_bandes_meme_dans_les_graves() {
        let s = vec![-50.0f32; 513];
        let b = bandes_log(&s, 48_000.0, 64);
        assert!(b.iter().all(|&v| (v + 50.0).abs() < 1e-3));
        assert_eq!(bandes_log(&[], 48_000.0, 3), vec![-120.0; 3]);
    }

    #[test]
    fn vers_spectre_donne_une_puissance_egale_pour_un_bruit_rose() {
        let (fe, n) = (48_000.0f32, 8192);
        let a = Analyseur::new(n, fe);
        // Puissance par bin ∝ 1/f : bruit rose.
        let mags: Vec<f32> = (0..=n / 2)
            .map(|i| {
                let f = a.frequence_bin(i).max(1.0);
                -10.0 * f.log10()
            })
            .collect();
        let s = a.vers_spectre(&mags, 100);
        assert_eq!(s.puissances.len(), 100);
        let db = |i: usize| 10.0 * s.puissances[i].log10();
        // On compare au-dessus de 1 kHz où chaque bande contient plusieurs bins.
        let i0 = s
            .frequences_hz
            .iter()
            .position(|&f| f > 1000.0)
            .unwrap_or(0);
        for i in i0..100 {
            assert!(
                (db(i) - db(i0)).abs() < 1.0,
                "point {i}: {} vs {}",
                db(i),
                db(i0)
            );
        }
    }

    /// Magnitudes (dB de puissance) d'un bruit rose idéal : la puissance d'un bin est
    /// l'intégrale de 1/f sur l'étendue du bin, soit ln((k+½)/(k−½)).
    fn magnitudes_bruit_rose(a: &Analyseur) -> Vec<f32> {
        (0..=a.taille() / 2)
            .map(|k| {
                let p = if k == 0 {
                    1e-12
                } else {
                    ((k as f64 + 0.5) / (k as f64 - 0.5)).ln()
                };
                (10.0 * p.log10()) as f32
            })
            .collect()
    }

    #[test]
    fn vers_spectre_est_plat_pour_un_bruit_rose_ideal_de_30_hz_a_16_khz() {
        let (fe, taille) = (48_000.0f32, 4096);
        let a = Analyseur::new(taille, fe);
        let mags = magnitudes_bruit_rose(&a);
        for n in [48, 96, 200] {
            let s = a.vers_spectre(&mags, n);
            let db: Vec<(f64, f64)> = s
                .frequences_hz
                .iter()
                .zip(&s.puissances)
                .filter(|(f, _)| (30.0..=16_000.0).contains(*f))
                .map(|(&f, &p)| (f, 10.0 * p.log10()))
                .collect();
            let moyenne = db.iter().map(|d| d.1).sum::<f64>() / db.len() as f64;
            for (f, d) in &db {
                assert!(
                    (d - moyenne).abs() < 0.3,
                    "n = {n}, {f:.1} Hz : {d:.2} dB pour une moyenne de {moyenne:.2} dB"
                );
            }
        }
    }

    #[test]
    fn vers_spectre_conserve_la_puissance_totale() {
        let (fe, taille) = (48_000.0f32, 4096);
        let a = Analyseur::new(taille, fe);
        for n in [48, 96, 200] {
            let freqs = frequences_log(n);
            let demi_pas = (FREQUENCE_MAX_HZ / FREQUENCE_MIN_HZ).powf(0.5 / (n - 1) as f64);
            let (bas, haut) = (freqs[0] / demi_pas, freqs[n - 1] * demi_pas);
            let largeur = f64::from(fe) / taille as f64;
            // Puissances quelconques (mais déterministes) dans les bins entièrement couverts.
            let mut total_bins = 0.0;
            let mags: Vec<f32> = (0..=taille / 2)
                .map(|k| {
                    let couvert =
                        (k as f64 - 0.5) * largeur >= bas && (k as f64 + 0.5) * largeur <= haut;
                    if couvert {
                        let p = 1.0 + ((k * 7919) % 13) as f64;
                        total_bins += p;
                        (10.0 * p.log10()) as f32
                    } else {
                        -120.0
                    }
                })
                .collect();
            let s = a.vers_spectre(&mags, n);
            let total: f64 = s.puissances.iter().sum();
            assert!(
                (total / total_bins - 1.0).abs() < 1e-4,
                "n = {n} : {total} au lieu de {total_bins}"
            );
        }
    }

    #[test]
    fn vers_spectre_place_un_sinus_dans_la_bonne_bande() {
        let (fe, taille) = (48_000.0f32, 4800);
        let mut a = Analyseur::new(taille, fe);
        for f_sinus in [100.0f32, 1000.0, 5000.0] {
            let mags = a.spectre(&sinus(f_sinus, fe, taille, 1.0));
            let s = a.vers_spectre(&mags, 200);
            let (k, _) = s
                .puissances
                .iter()
                .enumerate()
                .fold((0, f64::MIN), |m, (i, &p)| if p > m.1 { (i, p) } else { m });
            let lo = if k == 0 {
                0.0
            } else {
                (s.frequences_hz[k] * s.frequences_hz[k - 1]).sqrt()
            };
            let hi = (s.frequences_hz[k] * s.frequences_hz[k + 1]).sqrt();
            assert!(
                (lo..=hi).contains(&f64::from(f_sinus)),
                "{f_sinus} Hz : bande {lo:.1}..{hi:.1} Hz"
            );
        }
    }

    #[test]
    fn le_lissage_converge_vers_la_mesure() {
        let mut l = Lissage::new(0.5);
        assert_eq!(l.appliquer(&[0.0, 10.0]), &[0.0, 10.0]);
        assert_eq!(l.appliquer(&[10.0, 10.0]), &[5.0, 10.0]);
        let r = l.appliquer(&[10.0, 10.0]).to_vec();
        assert_eq!(r, vec![7.5, 10.0]);
        l.reinitialiser();
        assert_eq!(l.appliquer(&[1.0]), &[1.0]);
    }

    #[test]
    fn lissage_alpha_un_ne_lisse_pas_et_zero_fige() {
        let mut l = Lissage::new(1.0);
        l.appliquer(&[0.0]);
        assert_eq!(l.appliquer(&[7.0]), &[7.0]);
        let mut l = Lissage::new(0.0);
        l.appliquer(&[3.0]);
        assert_eq!(l.appliquer(&[7.0]), &[3.0]);
    }
}
