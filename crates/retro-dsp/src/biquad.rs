//! Filtre biquad (RBJ Audio EQ Cookbook) : coefficients, réponse en fréquence et traitement
//! échantillon par échantillon.

use crate::courbe::{Filtre, TypeFiltre};
use std::f64::consts::PI;

/// Fréquence d'échantillonnage de la courbe affichée et des calculs de volume.
pub const FREQUENCE_ECHANTILLONNAGE: f64 = 48_000.0;

/// Les fréquences d'échantillonnage courantes d'une sortie audio. Equalizer APO calcule ses
/// filtres à la vraie fréquence de la sortie, que l'appli ne connaît pas : près de Nyquist,
/// la même courbe monte plus haut à 44,1 kHz qu'à 48 kHz. Le préampli automatique prend la
/// crête la plus haute de toutes.
pub const FREQUENCES_ECHANTILLONNAGE_COURANTES: [f64; 3] = [44_100.0, 48_000.0, 96_000.0];

/// Les cinq coefficients d'un biquad, déjà divisés par `a0`.
///
/// Sert aux deux usages : évaluer la réponse en fréquence (calcul de courbe) et filtrer
/// un vrai signal (`Biquad`). Les deux partagent ainsi exactement les mêmes nombres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefficients {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Coefficients {
    /// Normalise des coefficients bruts en divisant tout par `a0`.
    pub fn normalises(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
        }
    }

    /// Un biquad qui laisse passer le signal tel quel.
    pub const TRANSPARENTS: Coefficients = Coefficients {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Coefficients du « Audio EQ Cookbook » pour ce filtre (mêmes formules qu'Equalizer APO
    /// pour `PK`, `LSC` et `HSC`).
    ///
    /// Un filtre que cette fréquence d'échantillonnage ne peut pas représenter est **ignoré**
    /// (coefficients transparents) : fréquence nulle, négative, à Nyquist (`fe / 2`) ou au-delà,
    /// Q nul ou négatif, valeur non finie, ou coefficients qui ne seraient pas finis et stables.
    /// Au-delà de Nyquist, les formules donnent un filtre instable ou le reflet du filtre sous
    /// Nyquist (une cloche à 30 kHz devient une cloche à 18 kHz à 48 kHz) : un son et une courbe
    /// affichée faux. On n'en ramène pas la fréquence juste sous Nyquist : ce serait inventer un
    /// renfort tout en haut du spectre, différent à chaque fréquence d'échantillonnage, que le
    /// fichier ne demandait pas à cet endroit. Ignorer donne une courbe vraie (rien entre 20 Hz
    /// et Nyquist) ; l'import signale ces filtres à part.
    pub fn depuis(filtre: &Filtre, fe: f64) -> Self {
        let representable = filtre.frequence_hz > 0.0
            && filtre.frequence_hz < fe / 2.0
            && filtre.q > 0.0
            && filtre.q.is_finite()
            && filtre.gain_db.is_finite();
        if !representable {
            return Self::TRANSPARENTS;
        }
        let c = Self::du_cookbook(filtre, fe);
        let finis = [c.b0, c.b1, c.b2, c.a1, c.a2].iter().all(|x| x.is_finite());
        let stable = c.a2.abs() < 1.0 && c.a1.abs() < 1.0 + c.a2;
        if finis && stable {
            c
        } else {
            Self::TRANSPARENTS
        }
    }

    fn du_cookbook(filtre: &Filtre, fe: f64) -> Self {
        let a = 10f64.powf(filtre.gain_db / 40.0);
        let w0 = 2.0 * PI * filtre.frequence_hz / fe;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * filtre.q);
        match filtre.type_filtre {
            TypeFiltre::Cloche => Self::normalises(
                [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
                [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
            ),
            TypeFiltre::PlateauBas => {
                let t = 2.0 * a.sqrt() * alpha;
                Self::normalises(
                    [
                        a * ((a + 1.0) - (a - 1.0) * cos + t),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                        a * ((a + 1.0) - (a - 1.0) * cos - t),
                    ],
                    [
                        (a + 1.0) + (a - 1.0) * cos + t,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                        (a + 1.0) + (a - 1.0) * cos - t,
                    ],
                )
            }
            TypeFiltre::PlateauHaut => {
                let t = 2.0 * a.sqrt() * alpha;
                Self::normalises(
                    [
                        a * ((a + 1.0) + (a - 1.0) * cos + t),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                        a * ((a + 1.0) + (a - 1.0) * cos - t),
                    ],
                    [
                        (a + 1.0) - (a - 1.0) * cos + t,
                        2.0 * ((a - 1.0) - (a + 1.0) * cos),
                        (a + 1.0) - (a - 1.0) * cos - t,
                    ],
                )
            }
        }
    }

    /// Carré du module de la réponse (rapport de puissance, sans unité) à `f_hz`.
    pub fn puissance_a(&self, f_hz: f64, fe: f64) -> f64 {
        let w = 2.0 * PI * f_hz / fe;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        // Numérateur et dénominateur évalués en z = e^{jw}, avec z^-1 = cos w - j sin w.
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = -(self.b1 * s1 + self.b2 * s2);
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = -(self.a1 * s1 + self.a2 * s2);
        (nr * nr + ni * ni) / (dr * dr + di * di)
    }

    /// Gain en dB à `f_hz`.
    pub fn gain_db_a(&self, f_hz: f64, fe: f64) -> f64 {
        10.0 * self.puissance_a(f_hz, fe).log10()
    }
}

/// Un filtre biquad qui traite un vrai signal, échantillon par échantillon.
///
/// Sert surtout aux tests : il prouve que la courbe calculée est bien celle du filtre réel.
#[derive(Debug, Clone)]
pub struct Biquad {
    c: Coefficients,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    /// Construit le biquad d'un filtre pour la fréquence d'échantillonnage `fe`.
    pub fn depuis(filtre: &Filtre, fe: f64) -> Self {
        Self::avec(Coefficients::depuis(filtre, fe))
    }

    /// Construit un biquad à partir de coefficients déjà calculés.
    pub fn avec(c: Coefficients) -> Self {
        Self {
            c,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Filtre un échantillon (forme directe I, état en f64 pour éviter le bruit de calcul).
    pub fn traiter(&mut self, x: f32) -> f32 {
        let x = f64::from(x);
        let y = self.c.b0 * x + self.c.b1 * self.x1 + self.c.b2 * self.x2
            - self.c.a1 * self.y1
            - self.c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filtre(type_filtre: TypeFiltre, f: f64, g: f64, q: f64) -> Filtre {
        Filtre {
            type_filtre,
            frequence_hz: f,
            gain_db: g,
            q,
        }
    }

    /// Gain mesuré (dB) en faisant passer un sinus dans le biquad, régime établi seulement.
    fn gain_mesure(filtre: &Filtre, f: f64) -> f64 {
        let fe = FREQUENCE_ECHANTILLONNAGE;
        let mut b = Biquad::depuis(filtre, fe);
        let n = 48_000;
        let (mut e_in, mut e_out) = (0.0f64, 0.0f64);
        for i in 0..n {
            let x = (2.0 * PI * f * i as f64 / fe).sin() as f32;
            let y = b.traiter(x);
            if i >= n / 2 {
                e_in += f64::from(x) * f64::from(x);
                e_out += f64::from(y) * f64::from(y);
            }
        }
        10.0 * (e_out / e_in).log10()
    }

    #[test]
    fn le_domaine_temporel_reproduit_la_reponse_calculee() {
        let cas = [
            filtre(TypeFiltre::Cloche, 1000.0, 6.0, 1.41),
            filtre(TypeFiltre::Cloche, 3000.0, -8.0, 2.0),
            filtre(TypeFiltre::PlateauBas, 200.0, 6.0, 0.71),
            filtre(TypeFiltre::PlateauBas, 200.0, -5.0, 1.0),
            filtre(TypeFiltre::PlateauHaut, 4000.0, 6.0, 0.71),
            filtre(TypeFiltre::PlateauHaut, 4000.0, -7.0, 1.2),
        ];
        let c_fe = FREQUENCE_ECHANTILLONNAGE;
        for f in cas {
            let c = Coefficients::depuis(&f, c_fe);
            for freq in [100.0, 500.0, 1000.0, 3000.0, 5000.0, 12000.0] {
                let attendu = c.gain_db_a(freq, c_fe);
                let mesure = gain_mesure(&f, freq);
                assert!(
                    (attendu - mesure).abs() < 0.1,
                    "{f:?} à {freq} Hz : calculé {attendu}, mesuré {mesure}"
                );
            }
        }
    }

    /// Les deux pôles de `1 + a1 z^-1 + a2 z^-2` sont dans le cercle unité (triangle de stabilité).
    fn est_stable(c: &Coefficients) -> bool {
        c.a2.abs() < 1.0 && c.a1.abs() < 1.0 + c.a2
    }

    #[test]
    fn un_filtre_a_nyquist_ou_au_dela_est_ignore_et_reste_stable() {
        let fe = FREQUENCE_ECHANTILLONNAGE;
        for type_filtre in [
            TypeFiltre::Cloche,
            TypeFiltre::PlateauBas,
            TypeFiltre::PlateauHaut,
        ] {
            for hz in [24_000.0, 30_000.0, 47_000.0, 100_000.0] {
                let f = filtre(type_filtre, hz, 6.0, 1.0);
                let c = Coefficients::depuis(&f, fe);
                assert!(est_stable(&c), "{f:?} : {c:?}");
                // Pas de reflet sous Nyquist : la réponse reste plate partout.
                for mesure in [100.0, 1000.0, 18_000.0, 23_000.0] {
                    let g = c.gain_db_a(mesure, fe);
                    assert!(g.abs() < 1e-9, "{f:?} à {mesure} Hz : {g} dB");
                }
                let mut b = Biquad::depuis(&f, fe);
                for i in 0..10_000 {
                    let y = b.traiter(if i % 2 == 0 { 1.0 } else { -1.0 });
                    assert!(y.abs() <= 1.0 + 1e-6);
                }
            }
        }
    }

    #[test]
    fn un_filtre_aux_valeurs_absurdes_est_ignore() {
        let fe = FREQUENCE_ECHANTILLONNAGE;
        for f in [
            filtre(TypeFiltre::Cloche, 0.0, 6.0, 1.0),
            filtre(TypeFiltre::Cloche, -100.0, 6.0, 1.0),
            filtre(TypeFiltre::Cloche, f64::NAN, 6.0, 1.0),
            filtre(TypeFiltre::Cloche, 1000.0, f64::INFINITY, 1.0),
            filtre(TypeFiltre::Cloche, 1000.0, 1e300, 1.0),
            filtre(TypeFiltre::Cloche, 1000.0, 6.0, 0.0),
            filtre(TypeFiltre::PlateauBas, 1000.0, 6.0, f64::NAN),
        ] {
            let c = Coefficients::depuis(&f, fe);
            assert!(est_stable(&c), "{f:?} : {c:?}");
            assert_eq!(c.gain_db_a(1000.0, fe), 0.0, "{f:?}");
        }
    }

    #[test]
    fn un_filtre_juste_sous_nyquist_reste_stable() {
        let fe = FREQUENCE_ECHANTILLONNAGE;
        for type_filtre in [
            TypeFiltre::Cloche,
            TypeFiltre::PlateauBas,
            TypeFiltre::PlateauHaut,
        ] {
            let c = Coefficients::depuis(&filtre(type_filtre, 23_900.0, 6.0, 1.0), fe);
            assert!(est_stable(&c), "{c:?}");
        }
    }

    #[test]
    fn filtre_a_gain_nul_est_transparent() {
        let f = filtre(TypeFiltre::Cloche, 1000.0, 0.0, 1.0);
        let mut b = Biquad::depuis(&f, FREQUENCE_ECHANTILLONNAGE);
        for x in [0.5f32, -0.25, 1.0, 0.0] {
            assert!((b.traiter(x) - x).abs() < 1e-6);
        }
    }
}
