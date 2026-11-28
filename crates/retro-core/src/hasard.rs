//! Le hasard du plan de test, derrière un trait pour pouvoir le fixer dans les tests.
//!
//! Deux implémentations écrites à la main (pas de dépendance `rand`) : [`HasardGraine`],
//! déterministe, pour les tests et pour rejouer un plan ; [`HasardReel`], initialisé depuis l'horloge.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Une source d'entiers tirés au hasard.
pub trait Hasard {
    /// Un entier uniforme dans `0..borne`. Rend 0 si `borne` vaut 0 ou 1.
    fn entier(&mut self, borne: u32) -> u32;

    /// Pile ou face.
    fn pile_ou_face(&mut self) -> bool
    where
        Self: Sized,
    {
        self.entier(2) == 1
    }

    /// Mélange `elements` sur place (Fisher-Yates : chaque ordre est équiprobable).
    fn melanger<T>(&mut self, elements: &mut [T])
    where
        Self: Sized,
    {
        for i in (1..elements.len()).rev() {
            let j = self.entier(i as u32 + 1) as usize;
            elements.swap(i, j);
        }
    }
}

/// Permet de passer `&mut dyn Hasard` (ou `&mut &mut H`) là où on attend `impl Hasard`.
impl<H: Hasard + ?Sized> Hasard for &mut H {
    fn entier(&mut self, borne: u32) -> u32 {
        (**self).entier(borne)
    }
}

/// Mélangeur de graine (SplitMix64) : transforme une graine quelconque, même 0 ou 1,
/// en un état de départ bien brassé pour le générateur.
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Générateur xorshift64* : quelques lignes, rapide, largement assez bon pour un plan de test.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Xorshift {
    etat: u64,
}

impl Xorshift {
    fn new(graine: u64) -> Self {
        let mut etat = splitmix64(graine);
        if etat == 0 {
            // xorshift reste coincé sur 0 : on l'évite.
            etat = 0x9E37_79B9_7F4A_7C15;
        }
        Xorshift { etat }
    }

    fn suivant_u32(&mut self) -> u32 {
        let mut x = self.etat;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.etat = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Entier uniforme dans `0..borne`, sans biais de modulo (on rejette la queue).
    fn entier(&mut self, borne: u32) -> u32 {
        if borne <= 1 {
            return 0;
        }
        let borne64 = u64::from(borne);
        let limite = (1u64 << 32) / borne64 * borne64;
        loop {
            let x = u64::from(self.suivant_u32());
            if x < limite {
                return (x % borne64) as u32;
            }
        }
    }
}

/// Hasard déterministe : la même graine donne toujours la même suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HasardGraine {
    generateur: Xorshift,
}

impl HasardGraine {
    pub fn new(graine: u64) -> Self {
        HasardGraine {
            generateur: Xorshift::new(graine),
        }
    }
}

impl Hasard for HasardGraine {
    fn entier(&mut self, borne: u32) -> u32 {
        self.generateur.entier(borne)
    }
}

/// Compteur global : deux `HasardReel` créés dans la même nanoseconde diffèrent quand même.
static COMPTEUR: AtomicU64 = AtomicU64::new(0);

/// Le vrai hasard de l'appli. Graine : horloge système, adresses mémoire (que l'ASLR
/// change à chaque lancement), entropie de `RandomState` (fournie par le système) et un compteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HasardReel {
    generateur: Xorshift,
}

impl HasardReel {
    pub fn new() -> Self {
        let horloge = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let sur_la_pile = 0u8;
        let adresse_pile = &sur_la_pile as *const u8 as usize as u64;
        let sur_le_tas = Box::new(0u8);
        let adresse_tas = &*sur_le_tas as *const u8 as usize as u64;
        let systeme = RandomState::new().build_hasher().finish();
        let compteur = COMPTEUR.fetch_add(1, Ordering::Relaxed);
        let graine = splitmix64(horloge)
            ^ splitmix64(adresse_pile.rotate_left(17))
            ^ splitmix64(adresse_tas.rotate_left(33))
            ^ splitmix64(systeme)
            ^ splitmix64(compteur);
        HasardReel {
            generateur: Xorshift::new(graine),
        }
    }
}

impl Default for HasardReel {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasard for HasardReel {
    fn entier(&mut self, borne: u32) -> u32 {
        self.generateur.entier(borne)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_meme_graine_donne_la_meme_suite() {
        let mut a = HasardGraine::new(42);
        let mut b = HasardGraine::new(42);
        for _ in 0..100 {
            assert_eq!(a.entier(1000), b.entier(1000));
        }
    }

    #[test]
    fn des_graines_differentes_donnent_des_suites_differentes() {
        let mut a = HasardGraine::new(1);
        let mut b = HasardGraine::new(2);
        let sa: Vec<u32> = (0..20).map(|_| a.entier(1_000_000)).collect();
        let sb: Vec<u32> = (0..20).map(|_| b.entier(1_000_000)).collect();
        assert_ne!(sa, sb);
    }

    #[test]
    fn la_graine_zero_fonctionne() {
        let mut h = HasardGraine::new(0);
        let v: Vec<u32> = (0..20).map(|_| h.entier(1_000_000)).collect();
        assert!(v.iter().any(|&x| x != v[0]));
    }

    #[test]
    fn entier_reste_dans_la_borne_et_les_bornes_degenerees_rendent_zero() {
        let mut h = HasardGraine::new(7);
        for borne in [1u32, 2, 3, 6, 7, 20, 1000] {
            for _ in 0..200 {
                assert!(h.entier(borne) < borne);
            }
        }
        assert_eq!(h.entier(0), 0);
        assert_eq!(h.entier(1), 0);
    }

    #[test]
    fn entier_est_a_peu_pres_uniforme() {
        let mut h = HasardGraine::new(99);
        let mut comptes = [0u32; 6];
        for _ in 0..60_000 {
            comptes[h.entier(6) as usize] += 1;
        }
        for c in comptes {
            assert!((9_000..11_000).contains(&c), "{comptes:?}");
        }
    }

    #[test]
    fn pile_ou_face_donne_les_deux_resultats() {
        let mut h = HasardGraine::new(5);
        let piles = (0..1000).filter(|_| h.pile_ou_face()).count();
        assert!((400..600).contains(&piles), "{piles}");
    }

    #[test]
    fn melanger_garde_les_elements_et_change_l_ordre() {
        let mut h = HasardGraine::new(3);
        let mut v: Vec<u32> = (0..20).collect();
        h.melanger(&mut v);
        assert_ne!(v, (0..20).collect::<Vec<u32>>());
        v.sort();
        assert_eq!(v, (0..20).collect::<Vec<u32>>());
    }

    #[test]
    fn melanger_tolere_les_listes_vides_ou_a_un_element() {
        let mut h = HasardGraine::new(3);
        let mut vide: Vec<u8> = vec![];
        h.melanger(&mut vide);
        let mut un = vec![1];
        h.melanger(&mut un);
        assert_eq!(un, vec![1]);
    }

    #[test]
    fn un_dyn_hasard_se_passe_comme_un_impl_hasard() {
        fn tire(h: &mut impl Hasard) -> u32 {
            h.entier(10)
        }
        let mut h = HasardGraine::new(1);
        let mut d: &mut dyn Hasard = &mut h;
        assert!(tire(&mut d) < 10);
    }

    #[test]
    fn le_vrai_hasard_varie_d_une_instance_a_l_autre() {
        let suites: Vec<Vec<u32>> = (0..5)
            .map(|_| {
                let mut h = HasardReel::new();
                (0..8).map(|_| h.entier(u32::MAX)).collect()
            })
            .collect();
        for i in 0..suites.len() {
            for j in i + 1..suites.len() {
                assert_ne!(suites[i], suites[j]);
            }
        }
    }
}
