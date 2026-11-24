//! Ce que l'interface affiche du son, et ce dont le calcul de compensation de volume a besoin.

use crate::Trame;
use retro_dsp::analyse::{bandes_log, niveau_rms_db, Analyseur, Lissage, PLANCHER_DB};
use retro_dsp::Spectre;

/// Taille de la FFT : ≈ 85 ms à 48 kHz, soit 11,7 Hz par bin (assez fin pour les graves).
pub const TAILLE_FFT: usize = 4096;
/// Nombre de points du `Spectre` rendu pour la compensation.
pub const POINTS_SPECTRE: usize = 96;
/// En dessous de ce niveau (dBFS), une bande d'affichage vaut 0 ; à 0 dBFS elle vaut 1.
pub const PLANCHER_AFFICHAGE_DB: f32 = -90.0;
/// Vitesse de retombée de la crête, en dB par seconde.
const CHUTE_CRETE_DB_PAR_S: f32 = 20.0;

/// Mesures lissées d'un flux de trames. Pur calcul : ni fil ni horloge, on lui donne des
/// trames, il met à jour ses chiffres, et l'interface les lit quand elle veut.
pub struct Mesure {
    n_bandes: usize,
    frequence: u32,
    analyseur: Analyseur,
    /// Les `TAILLE_FFT` derniers échantillons : une trame est plus courte qu'une FFT.
    historique: Vec<f32>,
    niveau_db: f32,
    crete_db: f32,
    initialise: bool,
    lissage_affichage: Lissage,
    lissage_spectre: Lissage,
    bandes: Vec<f32>,
    spectre: Spectre,
}

impl Mesure {
    /// `n_bandes` : nombre de barres du visualiseur.
    pub fn new(n_bandes: usize) -> Self {
        let frequence = 48_000;
        let analyseur = Analyseur::new(TAILLE_FFT, frequence as f32);
        let silence = vec![PLANCHER_DB; TAILLE_FFT / 2 + 1];
        let spectre = analyseur.vers_spectre(&silence, POINTS_SPECTRE);
        Self {
            n_bandes,
            frequence,
            analyseur,
            historique: Vec::with_capacity(TAILLE_FFT),
            niveau_db: PLANCHER_DB,
            crete_db: PLANCHER_DB,
            initialise: false,
            // L'affichage doit réagir vite ; le spectre de compensation, lui, doit être stable.
            lissage_affichage: Lissage::new(0.5),
            lissage_spectre: Lissage::new(0.1),
            bandes: vec![0.0; n_bandes],
            spectre,
        }
    }

    /// Intègre une trame. Si la fréquence d'échantillonnage change (autre sortie audio), la
    /// FFT est recréée et l'historique repart de zéro.
    pub fn ajouter(&mut self, trame: &Trame) {
        if trame.echantillons.is_empty() || trame.frequence_echantillonnage == 0 {
            return;
        }
        if trame.frequence_echantillonnage != self.frequence {
            self.frequence = trame.frequence_echantillonnage;
            self.analyseur = Analyseur::new(TAILLE_FFT, self.frequence as f32);
            self.historique.clear();
            self.lissage_affichage.reinitialiser();
            self.lissage_spectre.reinitialiser();
        }
        let duree_s = trame.echantillons.len() as f32 / self.frequence as f32;

        // Niveau : lissage dans le domaine des dB, montée rapide et descente plus lente
        // (comme un vumètre), pour que l'aiguille ne tremble pas.
        let niveau = niveau_rms_db(&trame.echantillons);
        if self.initialise {
            let alpha = if niveau > self.niveau_db { 0.6 } else { 0.15 };
            self.niveau_db += alpha * (niveau - self.niveau_db);
        } else {
            self.niveau_db = niveau;
            self.initialise = true;
        }

        // Crête : saute tout de suite au pic, retombe lentement.
        let pic = trame
            .echantillons
            .iter()
            .fold(0.0f32, |m, &x| m.max(x.abs()));
        let pic_db = if pic > 0.0 {
            (20.0 * pic.log10()).max(PLANCHER_DB)
        } else {
            PLANCHER_DB
        };
        let retombee = self.crete_db - CHUTE_CRETE_DB_PAR_S * duree_s;
        self.crete_db = pic_db.max(retombee).max(PLANCHER_DB);

        // Historique glissant des derniers échantillons.
        self.historique.extend_from_slice(&trame.echantillons);
        if self.historique.len() > TAILLE_FFT {
            let en_trop = self.historique.len() - TAILLE_FFT;
            self.historique.drain(..en_trop);
        }
        // Avant d'avoir une fenêtre pleine, la FFT verrait du silence artificiel : on attend.
        if self.historique.len() < TAILLE_FFT {
            return;
        }
        let magnitudes = self.analyseur.spectre(&self.historique);

        let rapide = self.lissage_affichage.appliquer(&magnitudes).to_vec();
        self.bandes = bandes_log(&rapide, self.frequence as f32, self.n_bandes)
            .into_iter()
            .map(|db| ((db - PLANCHER_AFFICHAGE_DB) / -PLANCHER_AFFICHAGE_DB).clamp(0.0, 1.0))
            .collect();

        let stable = self.lissage_spectre.appliquer(&magnitudes);
        self.spectre = self.analyseur.vers_spectre(stable, POINTS_SPECTRE);
    }

    /// Niveau efficace lissé, en dBFS (−120 = silence).
    pub fn niveau_db(&self) -> f32 {
        self.niveau_db
    }

    /// Crête à retombée lente, en dBFS.
    pub fn crete_db(&self) -> f32 {
        self.crete_db
    }

    /// Bandes d'affichage de 20 Hz à 20 kHz (log), chacune entre 0 et 1.
    pub fn bandes(&self) -> &[f32] {
        &self.bandes
    }

    /// Spectre lissé, au format attendu par `retro_dsp::compensation_db`.
    pub fn spectre(&self) -> &Spectre {
        &self.spectre
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn sinus(freq: f32, amplitude: f32, fe: u32, depart: usize, n: usize) -> Trame {
        Trame {
            echantillons: (depart..depart + n)
                .map(|i| amplitude * (TAU * freq * i as f32 / fe as f32).sin())
                .collect(),
            frequence_echantillonnage: fe,
        }
    }

    fn nourrir(mesure: &mut Mesure, freq: f32, amplitude: f32, trames: usize) {
        for k in 0..trames {
            mesure.ajouter(&sinus(freq, amplitude, 48_000, k * 1024, 1024));
        }
    }

    #[test]
    fn un_sinus_donne_le_bon_niveau_et_la_bonne_bande() {
        let n_bandes = 32;
        let mut mesure = Mesure::new(n_bandes);
        nourrir(&mut mesure, 1000.0, 0.5, 40);

        // RMS d'un sinus d'amplitude 0,5 : 0,5/√2 → −9,03 dBFS.
        assert!(
            (mesure.niveau_db() - -9.03).abs() < 0.1,
            "niveau {}",
            mesure.niveau_db()
        );
        // Crête : 0,5 → −6,02 dBFS.
        assert!(
            (mesure.crete_db() - -6.02).abs() < 0.1,
            "crête {}",
            mesure.crete_db()
        );

        let attendue = ((1000f32 / 20.0).ln() / 1000f32.ln() * n_bandes as f32) as usize;
        let (meilleure, valeur) = mesure
            .bandes()
            .iter()
            .enumerate()
            .fold((0, 0.0f32), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
        assert_eq!(meilleure, attendue, "bandes {:?}", mesure.bandes());
        assert!(valeur > 0.8 && valeur <= 1.0, "valeur {valeur}");
        assert!(mesure.bandes().iter().all(|&b| (0.0..=1.0).contains(&b)));
        // Loin de la raie, l'affichage est bas.
        assert!(mesure.bandes()[2] < 0.3);
    }

    #[test]
    fn le_spectre_de_compensation_pointe_vers_la_frequence() {
        let mut mesure = Mesure::new(16);
        nourrir(&mut mesure, 1000.0, 0.5, 80);
        let spectre = mesure.spectre();
        let (imax, _) = spectre
            .puissances
            .iter()
            .enumerate()
            .fold((0, 0.0f64), |m, (i, &p)| if p > m.1 { (i, p) } else { m });
        let f = spectre.frequences_hz[imax];
        assert!((800.0..1250.0).contains(&f), "pic à {f} Hz");
    }

    #[test]
    fn le_silence_donne_le_plancher_et_des_bandes_nulles() {
        let mut mesure = Mesure::new(24);
        for _ in 0..20 {
            mesure.ajouter(&Trame {
                echantillons: vec![0.0; 1024],
                frequence_echantillonnage: 48_000,
            });
        }
        assert_eq!(mesure.niveau_db(), PLANCHER_DB);
        assert_eq!(mesure.crete_db(), PLANCHER_DB);
        assert_eq!(mesure.bandes().len(), 24);
        assert!(mesure.bandes().iter().all(|&b| b == 0.0));
    }

    #[test]
    fn la_crete_retombe_lentement_apres_un_pic() {
        let mut mesure = Mesure::new(8);
        mesure.ajouter(&sinus(1000.0, 1.0, 48_000, 0, 1024));
        let haut = mesure.crete_db();
        assert!(haut > -0.5);
        for _ in 0..47 {
            // ≈ 1 s de silence
            mesure.ajouter(&Trame {
                echantillons: vec![0.0; 1024],
                frequence_echantillonnage: 48_000,
            });
        }
        let apres = mesure.crete_db();
        assert!(apres < haut - 15.0 && apres > haut - 25.0, "après {apres}");
    }

    #[test]
    fn un_changement_de_frequence_ne_plante_pas() {
        let mut mesure = Mesure::new(16);
        nourrir(&mut mesure, 440.0, 0.3, 10);
        for k in 0..20 {
            mesure.ajouter(&sinus(440.0, 0.3, 44_100, k * 882, 882));
        }
        assert!(mesure.niveau_db() > -20.0);
        assert_eq!(mesure.bandes().len(), 16);
    }
}
