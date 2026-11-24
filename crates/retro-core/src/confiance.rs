//! La confiance : à quel point un résultat peut-il être dû au hasard ?

use serde::{Deserialize, Serialize};

/// Probabilité d'obtenir au moins `victoires` succès en `total` essais si chaque essai est
/// un pile ou face (loi binomiale de paramètre 1/2) : `P(X >= victoires)`.
///
/// Plus elle est petite, moins le résultat peut s'expliquer par la chance. Sans aucun essai
/// (`total == 0`) on ne sait rien : le risque est 1. Plus de victoires que d'essais est
/// impossible : le risque est 0.
pub fn risque_hasard(victoires: u32, total: u32) -> f64 {
    if total == 0 || victoires == 0 {
        return 1.0;
    }
    if victoires > total {
        return 0.0;
    }
    if total <= 100 {
        // Calcul exact en entiers : le diviseur est une puissance de 2, donc le résultat
        // est représenté sans erreur (0,25 vaut exactement 0,25, ce qui compte pour le seuil).
        let mut c: u128 = 1; // C(total, 0)
        let mut somme: u128 = 0;
        for k in 0..=total {
            if k >= victoires {
                somme += c;
            }
            // C(n, k+1) = C(n, k) * (n - k) / (k + 1), toujours entier.
            c = c * u128::from(total - k) / u128::from(k + 1);
        }
        return somme as f64 / 2f64.powi(total as i32);
    }
    // Grand nombre d'essais (jamais atteint en pratique) : on somme en logarithmes.
    let n = f64::from(total);
    let mut ln_terme = -n * std::f64::consts::LN_2;
    let mut somme = 0.0;
    for k in 0..=total {
        if k >= victoires {
            somme += ln_terme.exp();
        }
        ln_terme += (f64::from(total - k)).ln() - (f64::from(k) + 1.0).ln();
    }
    somme.min(1.0)
}

/// Niveau de certitude d'un résultat, du plus faible au plus fort.
///
/// L'ordre des variantes est celui de la certitude : `Indice < Probable < Sur`, donc `min`
/// donne la plus basse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Confiance {
    Indice,
    Probable,
    Sur,
}

impl Confiance {
    /// Risque sous lequel un résultat est « Sûr ».
    pub const SEUIL_SUR: f64 = 0.06;
    /// Risque jusqu'auquel (compris) un résultat est « Probable ».
    pub const SEUIL_PROBABLE: f64 = 0.25;

    /// La confiance qui correspond à un risque de hasard : `Sur` sous 0,06, `Probable`
    /// jusqu'à 0,25 compris, sinon `Indice`.
    pub fn depuis_risque(risque: f64) -> Confiance {
        if risque < Self::SEUIL_SUR {
            Confiance::Sur
        } else if risque <= Self::SEUIL_PROBABLE {
            Confiance::Probable
        } else {
            Confiance::Indice
        }
    }

    /// Raccourci : la confiance de `victoires` sur `total`.
    pub fn depuis_victoires(victoires: u32, total: u32) -> Confiance {
        Self::depuis_risque(risque_hasard(victoires, total))
    }

    /// Un cran plus bas (utilisé quand les duels de contrôle montrent trop de hasard).
    /// `Indice` ne descend pas plus bas.
    pub fn baisser_d_un_cran(self) -> Confiance {
        match self {
            Confiance::Sur => Confiance::Probable,
            Confiance::Probable | Confiance::Indice => Confiance::Indice,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risques_de_reference() {
        assert_eq!(risque_hasard(1, 1), 0.5);
        assert_eq!(risque_hasard(2, 2), 0.25);
        assert_eq!(risque_hasard(4, 4), 0.0625);
        assert_eq!(risque_hasard(5, 5), 0.03125);
        assert_eq!(risque_hasard(0, 0), 1.0);
        assert_eq!(risque_hasard(3, 0), 1.0);
    }

    #[test]
    fn risque_binomial_calcule_a_la_main() {
        // P(X >= 3) pour 4 essais : (4 + 1) / 16.
        assert_eq!(risque_hasard(3, 4), 5.0 / 16.0);
        // P(X >= 2) pour 3 essais : (3 + 1) / 8.
        assert_eq!(risque_hasard(2, 3), 0.5);
        // P(X >= 7) pour 10 essais : (120 + 45 + 10 + 1) / 1024.
        assert_eq!(risque_hasard(7, 10), 176.0 / 1024.0);
    }

    #[test]
    fn cas_limites_du_risque() {
        assert_eq!(risque_hasard(0, 5), 1.0);
        assert_eq!(risque_hasard(6, 5), 0.0);
    }

    #[test]
    fn le_risque_decroit_avec_les_victoires() {
        for total in 1..=30u32 {
            let mut precedent = f64::INFINITY;
            for v in 0..=total {
                let r = risque_hasard(v, total);
                assert!(r <= precedent && (0.0..=1.0).contains(&r));
                precedent = r;
            }
        }
    }

    #[test]
    fn le_calcul_logarithmique_colle_au_calcul_exact() {
        // Une moitié symétrique : P(X >= n/2) vaut un peu plus de 0,5.
        let r = risque_hasard(51, 102);
        assert!(r > 0.5 && r < 0.6, "{r}");
        let r = risque_hasard(102, 102);
        assert!(r > 0.0 && r < 1e-29);
    }

    #[test]
    fn seuils_de_confiance_de_la_spec() {
        assert_eq!(Confiance::depuis_victoires(1, 1), Confiance::Indice);
        assert_eq!(Confiance::depuis_victoires(2, 2), Confiance::Probable);
        assert_eq!(Confiance::depuis_victoires(4, 4), Confiance::Probable);
        assert_eq!(Confiance::depuis_victoires(5, 5), Confiance::Sur);
        assert_eq!(Confiance::depuis_victoires(0, 0), Confiance::Indice);
        assert_eq!(Confiance::depuis_risque(0.0599), Confiance::Sur);
        assert_eq!(Confiance::depuis_risque(0.06), Confiance::Probable);
        assert_eq!(Confiance::depuis_risque(0.25), Confiance::Probable);
        assert_eq!(Confiance::depuis_risque(0.2501), Confiance::Indice);
    }

    #[test]
    fn baisser_d_un_cran() {
        assert_eq!(Confiance::Sur.baisser_d_un_cran(), Confiance::Probable);
        assert_eq!(Confiance::Probable.baisser_d_un_cran(), Confiance::Indice);
        assert_eq!(Confiance::Indice.baisser_d_un_cran(), Confiance::Indice);
    }

    #[test]
    fn l_ordre_va_de_l_indice_au_sur() {
        assert!(Confiance::Indice < Confiance::Probable);
        assert!(Confiance::Probable < Confiance::Sur);
        assert_eq!(Confiance::Sur.min(Confiance::Indice), Confiance::Indice);
    }
}
