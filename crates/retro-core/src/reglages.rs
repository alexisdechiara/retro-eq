//! Les réglages : ceux d'un test (passés au déroulement) et ceux de l'appli (`reglages.json`).

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Plus court intervalle entre deux bascules (ms) : en dessous, on n'a pas le temps d'écouter.
pub const INTERVALLE_MIN_MS: u32 = 10_000;
/// Plus long intervalle (ms).
pub const INTERVALLE_MAX_MS: u32 = 120_000;
/// Intervalle par défaut (ms).
pub const INTERVALLE_DEFAUT_MS: u32 = 20_000;
/// Écoute cumulée sans pause après laquelle on conseille une pause : 40 minutes.
pub const PAUSE_CONSEILLEE_MS: u64 = 40 * 60 * 1000;

/// Délai le plus court laissé pour répondre (ms).
pub const DELAI_REPONSE_MIN_MS: u32 = 30_000;
/// Délai le plus long laissé pour répondre (ms) : 10 minutes.
pub const DELAI_REPONSE_MAX_MS: u32 = 600_000;
/// Délai par défaut pour répondre (ms) : 2 minutes (ADR-0012).
pub const DELAI_REPONSE_DEFAUT_MS: u32 = 120_000;

/// Ramène un délai de réponse dans `30 000..=600 000` ms.
pub fn borner_delai_reponse(ms: u32) -> u32 {
    ms.clamp(DELAI_REPONSE_MIN_MS, DELAI_REPONSE_MAX_MS)
}

fn lire_delai_reponse<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    u32::deserialize(d).map(borner_delai_reponse)
}

/// Ramène un intervalle dans `10 000..=120 000` ms.
pub fn borner_intervalle(ms: u32) -> u32 {
    ms.clamp(INTERVALLE_MIN_MS, INTERVALLE_MAX_MS)
}

/// Relit un intervalle depuis un fichier en le bornant : un fichier édité à la main ne doit
/// pas pouvoir imposer une valeur hors limites.
fn lire_intervalle<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    u32::deserialize(d).map(borner_intervalle)
}

fn intervalle_defaut() -> u32 {
    INTERVALLE_DEFAUT_MS
}

fn vrai() -> bool {
    true
}

/// Les réglages d'un test, copiés dans le journal au démarrage : un test repris relit
/// exactement ce avec quoi il avait commencé.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesTest {
    #[serde(deserialize_with = "lire_intervalle")]
    intervalle_ms: u32,
    bip: bool,
    pause_conseillee_apres_ms: u64,
    /// Temps laissé pour répondre une fois la question posée. Passé ce délai, le test se
    /// met en pause tout seul : un côté ne reste pas actif indéfiniment (ADR-0012).
    #[serde(deserialize_with = "lire_delai_reponse")]
    delai_reponse_ms: u32,
}

impl Default for ReglagesTest {
    fn default() -> Self {
        ReglagesTest {
            intervalle_ms: INTERVALLE_DEFAUT_MS,
            bip: true,
            pause_conseillee_apres_ms: PAUSE_CONSEILLEE_MS,
            delai_reponse_ms: DELAI_REPONSE_DEFAUT_MS,
        }
    }
}

impl ReglagesTest {
    /// Des réglages avec cet intervalle (borné à 10 s – 120 s) et ce réglage de bip.
    pub fn nouveau(intervalle_ms: u32, bip: bool) -> Self {
        ReglagesTest {
            intervalle_ms: borner_intervalle(intervalle_ms),
            bip,
            ..Default::default()
        }
    }

    /// Durée d'un passage, en ms (toujours dans 10 000..=120 000).
    pub fn intervalle_ms(&self) -> u32 {
        self.intervalle_ms
    }

    /// Un bip signale-t-il la fin des passages ? (Utile en plein écran exclusif, où la
    /// question n'est pas visible.)
    pub fn bip(&self) -> bool {
        self.bip
    }

    /// Écoute cumulée (ms) après laquelle on conseille une pause.
    pub fn pause_conseillee_apres_ms(&self) -> u64 {
        self.pause_conseillee_apres_ms
    }

    /// Temps laissé pour répondre après la question, en ms (toujours dans 30 000..=600 000).
    pub fn delai_reponse_ms(&self) -> u32 {
        self.delai_reponse_ms
    }

    /// Mêmes réglages avec un autre délai de réponse (borné à 30 s – 10 min).
    pub fn avec_delai_reponse_ms(mut self, ms: u32) -> Self {
        self.delai_reponse_ms = borner_delai_reponse(ms);
        self
    }

    /// Même réglages avec un autre délai avant le conseil de pause (utile aux tests).
    pub fn avec_pause_conseillee_apres_ms(mut self, ms: u64) -> Self {
        self.pause_conseillee_apres_ms = ms;
        self
    }
}

/// Langue de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Langue {
    #[default]
    #[serde(rename = "fr")]
    Fr,
    #[serde(rename = "en")]
    En,
}

/// Où se trouve la navigation principale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Navigation {
    /// Onglets dans la barre de titre.
    #[default]
    #[serde(rename = "onglets")]
    Onglets,
    /// Rail latéral.
    #[serde(rename = "rail")]
    Rail,
}

/// Les réglages de l'appli, dans `reglages.json`.
///
/// `#[serde(default)]` : un fichier écrit par une version plus ancienne, auquel il manque un
/// champ, se charge quand même, le champ prenant sa valeur par défaut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesAppli {
    pub langue: Langue,
    pub navigation: Navigation,
    /// Durée d'un passage (ms), bornée à 10 000..=120 000 au chargement.
    #[serde(deserialize_with = "lire_intervalle")]
    pub intervalle_ms: u32,
    pub bip: bool,
    /// Affiche la mini-fenêtre pendant un test.
    pub mini_fenetre: bool,
    /// Fermer la fenêtre la réduit dans la barre des tâches au lieu de quitter.
    pub fermer_reduit: bool,
    /// Raccourci clavier global de chaque profil : identifiant du profil → touches (« Ctrl+Alt+1 »).
    pub raccourcis_profils: BTreeMap<String, String>,
    /// L'utilisateur a accepté que Retro EQ écrive dans la configuration du moteur audio
    /// (bouton « Continuer » du premier lancement). Tant que c'est faux, rien n'est écrit.
    pub moteur_autorise: bool,
    /// L'égaliseur est coupé (Ctrl+Alt+0) : le moteur reste en place, à plat.
    pub egaliseur_coupe: bool,
    /// Met le test en pause tout seul pendant un appel vocal.
    pub pause_appels: bool,
}

impl Default for ReglagesAppli {
    fn default() -> Self {
        ReglagesAppli {
            langue: Langue::Fr,
            navigation: Navigation::Onglets,
            intervalle_ms: intervalle_defaut(),
            bip: vrai(),
            mini_fenetre: vrai(),
            fermer_reduit: vrai(),
            raccourcis_profils: BTreeMap::new(),
            moteur_autorise: false,
            egaliseur_coupe: false,
            pause_appels: vrai(),
        }
    }
}

impl ReglagesAppli {
    /// Les réglages de test qui en découlent.
    pub fn reglages_test(&self) -> ReglagesTest {
        ReglagesTest::nouveau(self.intervalle_ms, self.bip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valeurs_par_defaut_d_un_test() {
        let r = ReglagesTest::default();
        assert_eq!(r.intervalle_ms(), 20_000);
        assert!(r.bip());
        assert_eq!(r.pause_conseillee_apres_ms(), 40 * 60 * 1000);
    }

    #[test]
    fn le_delai_de_reponse_vaut_2_minutes_et_reste_entre_30_secondes_et_10_minutes() {
        let r = ReglagesTest::default();
        assert_eq!(r.delai_reponse_ms(), 120_000);
        assert_eq!(r.clone().avec_delai_reponse_ms(1).delai_reponse_ms(), 30_000);
        assert_eq!(
            r.clone().avec_delai_reponse_ms(9_999_999).delai_reponse_ms(),
            600_000
        );
        assert_eq!(r.avec_delai_reponse_ms(45_000).delai_reponse_ms(), 45_000);
        // Un journal écrit avant ce réglage se relit avec la valeur par défaut, et une
        // valeur hors limites dans un fichier est bornée.
        let ancien: ReglagesTest = serde_json::from_str(r#"{"intervalle_ms": 20000}"#).unwrap();
        assert_eq!(ancien.delai_reponse_ms(), 120_000);
        let hors: ReglagesTest = serde_json::from_str(r#"{"delai_reponse_ms": 5}"#).unwrap();
        assert_eq!(hors.delai_reponse_ms(), 30_000);
    }

    #[test]
    fn l_intervalle_est_borne() {
        assert_eq!(ReglagesTest::nouveau(1, true).intervalle_ms(), 10_000);
        assert_eq!(ReglagesTest::nouveau(10_000, true).intervalle_ms(), 10_000);
        assert_eq!(ReglagesTest::nouveau(45_000, true).intervalle_ms(), 45_000);
        assert_eq!(
            ReglagesTest::nouveau(120_000, true).intervalle_ms(),
            120_000
        );
        assert_eq!(
            ReglagesTest::nouveau(999_999, true).intervalle_ms(),
            120_000
        );
    }

    #[test]
    fn un_intervalle_hors_limites_est_borne_a_la_lecture() {
        let r: ReglagesTest = serde_json::from_str(r#"{"intervalle_ms": 5}"#).unwrap();
        assert_eq!(r.intervalle_ms(), 10_000);
        let a: ReglagesAppli = serde_json::from_str(r#"{"intervalle_ms": 500000}"#).unwrap();
        assert_eq!(a.intervalle_ms, 120_000);
    }

    #[test]
    fn reglages_test_passe_par_json() {
        let r = ReglagesTest::nouveau(30_000, false).avec_pause_conseillee_apres_ms(1234);
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<ReglagesTest>(&j).unwrap(), r);
    }

    #[test]
    fn un_vieux_fichier_de_reglages_se_charge_avec_les_defauts() {
        let a: ReglagesAppli = serde_json::from_str("{}").unwrap();
        assert_eq!(a, ReglagesAppli::default());
        let a: ReglagesAppli = serde_json::from_str(r#"{"langue":"en"}"#).unwrap();
        assert_eq!(a.langue, Langue::En);
        assert_eq!(a.navigation, Navigation::Onglets);
        assert_eq!(a.intervalle_ms, 20_000);
        assert!(a.fermer_reduit);
    }

    #[test]
    fn les_champs_inconnus_d_un_fichier_plus_recent_sont_ignores() {
        let a: ReglagesAppli = serde_json::from_str(r#"{"bip":false,"truc_futur":1}"#).unwrap();
        assert!(!a.bip);
    }

    #[test]
    fn les_reglages_ecrits_utilisent_les_noms_attendus() {
        let mut a = ReglagesAppli {
            navigation: Navigation::Rail,
            ..Default::default()
        };
        a.raccourcis_profils
            .insert("casque".into(), "Ctrl+Alt+1".into());
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["langue"], "fr");
        assert_eq!(v["navigation"], "rail");
        assert_eq!(v["raccourcis_profils"]["casque"], "Ctrl+Alt+1");
        assert_eq!(serde_json::from_value::<ReglagesAppli>(v).unwrap(), a);
    }

    #[test]
    fn les_reglages_de_test_decoulent_de_ceux_de_l_appli() {
        let a = ReglagesAppli {
            intervalle_ms: 30_000,
            bip: false,
            ..Default::default()
        };
        let t = a.reglages_test();
        assert_eq!(t.intervalle_ms(), 30_000);
        assert!(!t.bip());
    }
}
