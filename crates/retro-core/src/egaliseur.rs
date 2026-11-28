//! L'égaliseur à 10 bandes : les faders que l'utilisateur voit, et les « filtres
//! supplémentaires » qu'il ne voit pas mais qu'on ne doit jamais perdre.
//!
//! Une `Courbe` est une liste libre de filtres ; l'écran Égaliseur n'en montre que 10 :
//! des cloches à fréquences et Q fixes. Tout autre filtre (un plateau d'un import Peace, une
//! zone retenue par un test…) reste dans la courbe, à l'abri, quand on bouge un fader.

use retro_dsp::{Courbe, Filtre, TypeFiltre};

/// Fréquences centrales des 10 bandes, en Hz.
pub const BANDES_HZ: [f64; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

/// Q commun des 10 bandes.
pub const Q_BANDES: f64 = 1.41;

/// Écart toléré pour reconnaître une bande : un fichier importé arrondit parfois (31,0 Hz,
/// Q 1,41), jamais plus que ça.
const TOLERANCE_FREQUENCE: f64 = 1e-3;
const TOLERANCE_Q: f64 = 2e-2;

fn filtre_de_bande(indice: usize, gain_db: f64) -> Filtre {
    Filtre {
        type_filtre: TypeFiltre::Cloche,
        frequence_hz: BANDES_HZ[indice],
        gain_db,
        q: Q_BANDES,
    }
}

/// Indice de la bande que ce filtre représente, s'il en représente une.
fn bande_du_filtre(f: &Filtre) -> Option<usize> {
    if f.type_filtre != TypeFiltre::Cloche || (f.q - Q_BANDES).abs() > TOLERANCE_Q {
        return None;
    }
    BANDES_HZ
        .iter()
        .position(|&hz| (f.frequence_hz / hz - 1.0).abs() <= TOLERANCE_FREQUENCE)
}

/// La courbe d'un égaliseur à 10 bandes : une cloche par bande dont le gain n'est pas nul
/// (une bande à 0 dB ne fait rien, on ne l'écrit pas). Préampli à 0 : c'est à l'appelant
/// d'y mettre le préampli de son choix (`avec_preampli_auto` par exemple).
pub fn courbe_10_bandes(gains: [f64; 10]) -> Courbe {
    FadersEtSupplementaires {
        gains,
        supplementaires: Vec::new(),
    }
    .vers_courbe(0.0)
}

/// Une courbe vue comme l'égaliseur la montre : 10 gains de faders, plus le reste.
#[derive(Debug, Clone, PartialEq)]
pub struct FadersEtSupplementaires {
    /// Gain (dB) de chaque fader, dans l'ordre de [`BANDES_HZ`].
    pub gains: [f64; 10],
    /// Les filtres qui ne sont pas un fader : conservés tels quels, dans leur ordre.
    pub supplementaires: Vec<Filtre>,
}

impl FadersEtSupplementaires {
    /// Sépare une courbe. Deux cloches qui tombent sur la même bande s'additionnent dans le
    /// même fader (elles font exactement le même effet réunies).
    pub fn depuis_courbe(courbe: &Courbe) -> Self {
        let mut gains = [0.0; 10];
        let mut supplementaires = Vec::new();
        for f in &courbe.filtres {
            match bande_du_filtre(f) {
                Some(i) => gains[i] += f.gain_db,
                None => supplementaires.push(*f),
            }
        }
        FadersEtSupplementaires {
            gains,
            supplementaires,
        }
    }

    /// Reconstitue une courbe : d'abord les faders non nuls, puis les filtres supplémentaires.
    /// `preampli_db` est le préampli de la courbe (il n'appartient à aucun fader).
    pub fn vers_courbe(&self, preampli_db: f64) -> Courbe {
        let mut filtres: Vec<Filtre> = self
            .gains
            .iter()
            .enumerate()
            .filter(|(_, &g)| g != 0.0)
            .map(|(i, &g)| filtre_de_bande(i, g))
            .collect();
        filtres.extend(self.supplementaires.iter().copied());
        Courbe {
            preampli_db,
            filtres,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_dsp::frequences_log;

    fn supplementaire() -> Filtre {
        Filtre {
            type_filtre: TypeFiltre::PlateauBas,
            frequence_hz: 60.0,
            gain_db: 4.0,
            q: 0.71,
        }
    }

    #[test]
    fn dix_bandes_aux_frequences_de_la_spec() {
        assert_eq!(
            BANDES_HZ,
            [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0]
        );
        assert_eq!(Q_BANDES, 1.41);
    }

    #[test]
    fn courbe_10_bandes_pose_une_cloche_par_gain_non_nul() {
        let mut g = [0.0; 10];
        g[2] = 3.0;
        g[7] = -2.5;
        let c = courbe_10_bandes(g);
        assert_eq!(c.preampli_db, 0.0);
        assert_eq!(c.filtres.len(), 2);
        assert_eq!(c.filtres[0].frequence_hz, 125.0);
        assert_eq!(c.filtres[0].gain_db, 3.0);
        assert_eq!(c.filtres[0].q, 1.41);
        assert_eq!(c.filtres[0].type_filtre, TypeFiltre::Cloche);
        assert_eq!(c.filtres[1].frequence_hz, 4000.0);
        assert_eq!(c.filtres[1].gain_db, -2.5);
    }

    #[test]
    fn l_egaliseur_a_plat_est_la_courbe_vide() {
        assert_eq!(courbe_10_bandes([0.0; 10]), Courbe::default());
    }

    #[test]
    fn faders_vers_courbe_vers_faders_est_l_identite() {
        let g = [1.0, -2.0, 0.0, 3.5, 0.0, 0.0, -4.0, 2.0, 0.5, -1.0];
        let f = FadersEtSupplementaires::depuis_courbe(&courbe_10_bandes(g));
        assert_eq!(f.gains, g);
        assert!(f.supplementaires.is_empty());
    }

    #[test]
    fn les_filtres_supplementaires_survivent_a_un_aller_retour() {
        let mut g = [0.0; 10];
        g[5] = 2.0;
        let mut c = courbe_10_bandes(g);
        c.preampli_db = -3.0;
        c.filtres.insert(0, supplementaire());
        let extra2 = Filtre {
            type_filtre: TypeFiltre::Cloche,
            frequence_hz: 1000.0,
            gain_db: 1.0,
            q: 0.667, // même fréquence qu'une bande, mais pas son Q : ce n'est pas un fader
        };
        c.filtres.push(extra2);

        let f = FadersEtSupplementaires::depuis_courbe(&c);
        assert_eq!(f.gains[5], 2.0);
        assert_eq!(f.supplementaires, vec![supplementaire(), extra2]);

        let retour = f.vers_courbe(c.preampli_db);
        assert_eq!(retour.preampli_db, -3.0);
        for hz in frequences_log(200) {
            assert!((retour.reponse_db(hz) - c.reponse_db(hz)).abs() < 1e-9);
        }
        assert!(retour.filtres.contains(&supplementaire()));
        assert!(retour.filtres.contains(&extra2));
    }

    #[test]
    fn bouger_un_fader_garde_les_supplementaires() {
        let mut c = courbe_10_bandes([0.0; 10]);
        c.filtres.push(supplementaire());
        let mut f = FadersEtSupplementaires::depuis_courbe(&c);
        f.gains[0] = 6.0;
        let nouvelle = f.vers_courbe(0.0);
        assert_eq!(nouvelle.filtres.len(), 2);
        assert!(nouvelle.filtres.contains(&supplementaire()));
    }

    #[test]
    fn deux_cloches_sur_la_meme_bande_s_additionnent() {
        let c = Courbe {
            preampli_db: 0.0,
            filtres: vec![filtre_de_bande(3, 2.0), filtre_de_bande(3, 1.0)],
        };
        let f = FadersEtSupplementaires::depuis_courbe(&c);
        assert_eq!(f.gains[3], 3.0);
        assert!(f.supplementaires.is_empty());
    }

    #[test]
    fn un_import_arrondi_est_reconnu_comme_fader() {
        let c = Courbe {
            preampli_db: 0.0,
            filtres: vec![Filtre {
                type_filtre: TypeFiltre::Cloche,
                frequence_hz: 1000.0,
                gain_db: 2.0,
                q: 1.4,
            }],
        };
        assert_eq!(FadersEtSupplementaires::depuis_courbe(&c).gains[5], 2.0);
    }
}
