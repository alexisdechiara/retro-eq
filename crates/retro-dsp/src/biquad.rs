//! Filtre biquad (RBJ Audio EQ Cookbook) : coefficients, réponse en fréquence et traitement
//! échantillon par échantillon.

use crate::courbe::{Filtre, TypeFiltre};
use std::f64::consts::PI;

/// Fréquence d'échantillonnage utilisée pour tous les calculs de courbe.
pub const FREQUENCE_ECHANTILLONNAGE: f64 = 48_000.0;

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

    /// Coefficients du « Audio EQ Cookbook » pour ce filtre (mêmes formules qu'Equalizer APO
    /// pour `PK`, `LSC` et `HSC`).
    pub fn depuis(filtre: &Filtre, fe: f64) -> Self {
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

    #[test]
    fn filtre_a_gain_nul_est_transparent() {
        let f = filtre(TypeFiltre::Cloche, 1000.0, 0.0, 1.0);
        let mut b = Biquad::depuis(&f, FREQUENCE_ECHANTILLONNAGE);
        for x in [0.5f32, -0.25, 1.0, 0.0] {
            assert!((b.traiter(x) - x).abs() < 1e-6);
        }
    }
}
