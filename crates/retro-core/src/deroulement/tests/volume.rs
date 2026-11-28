//! L'écart de volume mesuré pendant un duel (ticket 26) et la relecture des duels clos
//! (écran Résultat, Empreinte).

use super::*;
use crate::plan::Preference;
use Evenement as E;

const VARIANTE_DB: f64 = -18.0;
const REFERENCE_DB: f64 = -20.0;

impl Banc {
    /// Écoute le duel en cours jusqu'à la question en envoyant, pendant chaque passage,
    /// quatre mesures de volume (à 2, 4, 6 et 8 s) données par `volume(côté)`.
    fn ecouter_en_mesurant(&mut self, volume: impl Fn(Cote) -> f64) {
        while matches!(self.etat(), EtatTest::EnEcoute { .. }) {
            let cote = self.vue().cote_actif.unwrap();
            for _ in 0..4 {
                self.avancer(2_000);
                self.envoyer(E::VolumeMesure(volume(cote)));
            }
            self.tic_apres(INTERVALLE - 8_000);
        }
    }

    /// Le volume de chaque côté du duel courant : la variante plus fort que la référence.
    fn variante_plus_forte(&self) -> impl Fn(Cote) -> f64 {
        let cote_ref = self.prevu(self.d.issues.len()).cote_reference();
        move |cote| {
            if cote == cote_ref {
                REFERENCE_DB
            } else {
                VARIANTE_DB
            }
        }
    }
}

fn ecarts_journalises(journal: &[Entree]) -> Vec<(u32, f64)> {
    journal
        .iter()
        .filter_map(|e| match e {
            Entree::EcartVolumeMesure { duel, ecart_db, .. } => Some((*duel, *ecart_db)),
            _ => None,
        })
        .collect()
}

#[test]
fn l_ecart_mesure_est_journalise_juste_apres_la_reponse() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    let volume = b.variante_plus_forte();
    b.ecouter_en_mesurant(volume);
    let effets = b.repondre(Reponse::B);
    let journal = journalisees(&effets);
    assert!(matches!(journal[0], Entree::ReponseDonnee { .. }));
    assert!(
        matches!(journal[1], Entree::EcartVolumeMesure { duel: 0, ecart_db, .. } if (ecart_db - 2.0).abs() < 1e-9),
        "{journal:?}"
    );
    let clos = b.d.duels_clos();
    assert_eq!(clos.len(), 1);
    assert!((clos[0].ecart_volume_db.unwrap() - 2.0).abs() < 1e-9);
    assert!(
        !clos[0].ecart_signale(),
        "2 dB pile ne dépasse pas le seuil"
    );
}

#[test]
fn l_ecart_garde_son_sens_quelle_que_soit_l_orientation_du_duel() {
    // Le second duel du plan a la référence en B : l'écart reste « variante moins référence ».
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    for _ in 0..2 {
        let volume = b.variante_plus_forte();
        b.ecouter_en_mesurant(volume);
        b.repondre(Reponse::Pareil);
    }
    let ecarts = ecarts_journalises(&b.journal);
    assert_eq!(ecarts.len(), 2);
    for (_, e) in ecarts {
        assert!((e - 2.0).abs() < 1e-9);
    }
}

#[test]
fn un_ecart_au_dela_du_seuil_est_signale() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    let cote_ref = b.prevu(0).cote_reference();
    b.ecouter_en_mesurant(|c| if c == cote_ref { -24.0 } else { -20.0 });
    b.repondre(Reponse::B);
    assert!(b.d.duels_clos()[0].ecart_signale());
}

#[test]
fn les_mesures_qui_suivent_une_bascule_ne_comptent_pas() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    let cote_ref = b.prevu(0).cote_reference();
    while matches!(b.etat(), EtatTest::EnEcoute { .. }) {
        let cote = b.vue().cote_actif.unwrap();
        // Juste après la bascule, le son mêle encore les deux côtés : une mesure absurde,
        // qui fausserait tout si elle comptait.
        b.avancer(500);
        b.envoyer(E::VolumeMesure(if cote == cote_ref { -60.0 } else { 0.0 }));
        for _ in 0..4 {
            b.avancer(1_500);
            b.envoyer(E::VolumeMesure(-20.0));
        }
        b.tic_apres(INTERVALLE - 6_500);
    }
    b.repondre(Reponse::A);
    let ecart = b.d.duels_clos()[0].ecart_volume_db.unwrap();
    assert!(ecart.abs() < 1e-9, "{ecart}");
}

#[test]
fn sans_assez_de_mesures_aucun_ecart_n_est_donne() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    // Une seule mesure par passage : 3 par côté au mieux… mais seulement pour A.
    while matches!(b.etat(), EtatTest::EnEcoute { .. }) {
        let cote = b.vue().cote_actif.unwrap();
        b.avancer(3_000);
        if cote == Cote::A {
            b.envoyer(E::VolumeMesure(-20.0));
        }
        b.tic_apres(INTERVALLE - 3_000);
    }
    b.repondre(Reponse::A);
    assert!(ecarts_journalises(&b.journal).is_empty());
    assert_eq!(b.d.duels_clos()[0].ecart_volume_db, None);
}

#[test]
fn le_silence_ne_compte_pas_comme_une_mesure() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    b.ecouter_en_mesurant(|_| -95.0);
    b.repondre(Reponse::A);
    assert_eq!(b.d.duels_clos()[0].ecart_volume_db, None);
}

#[test]
fn hors_de_l_ecoute_les_mesures_sont_ignorees() {
    let mut b = Banc::nouveau(plan_gagnant());
    assert!(b.envoyer(E::VolumeMesure(-20.0)).is_empty());
    b.demarrer();
    let volume = b.variante_plus_forte();
    b.ecouter_en_mesurant(volume);
    // Pendant la question : rien n'est cumulé, quel que soit le côté réécouté.
    for _ in 0..10 {
        b.avancer(1_000);
        assert!(b.envoyer(E::VolumeMesure(0.0)).is_empty());
    }
    b.repondre(Reponse::B);
    assert!((b.d.duels_clos()[0].ecart_volume_db.unwrap() - 2.0).abs() < 1e-9);
}

#[test]
fn un_duel_passe_n_a_pas_d_ecart() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    let volume = b.variante_plus_forte();
    b.ecouter_en_mesurant(volume);
    b.envoyer(E::PasserDuel);
    assert!(ecarts_journalises(&b.journal).is_empty());
    assert_eq!(b.d.duels_clos()[0].ecart_volume_db, None);
}

#[test]
fn un_duel_recommence_remesure_depuis_zero() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    // Des mesures très fortes, puis on recommence : elles ne doivent plus compter.
    b.ecouter_en_mesurant(|_| 0.0);
    b.envoyer(E::RecommencerDuel);
    let volume = b.variante_plus_forte();
    b.ecouter_en_mesurant(volume);
    b.repondre(Reponse::B);
    assert!((b.d.duels_clos()[0].ecart_volume_db.unwrap() - 2.0).abs() < 1e-9);
}

#[test]
fn une_reponse_annulee_retire_son_ecart() {
    let mut b = Banc::nouveau(vec![
        zone(Zone::Graves, Sens::Plus, true),
        zone(Zone::Air, Sens::Plus, true),
        controle(),
    ]);
    b.demarrer();
    let volume = b.variante_plus_forte();
    b.ecouter_en_mesurant(volume);
    b.repondre(Reponse::B);
    b.ecouter_un_passage();
    b.envoyer(E::Annuler);
    assert!(b.d.duels_clos().is_empty());
    b.ecouter_un_duel();
    b.repondre(Reponse::A);
    assert_eq!(b.d.duels_clos()[0].ecart_volume_db, None);
}

#[test]
fn la_reprise_retrouve_les_ecarts_du_journal() {
    let mut b = Banc::nouveau(vec![
        zone(Zone::Graves, Sens::Plus, true),
        zone(Zone::Air, Sens::Plus, false),
        controle(),
    ]);
    b.demarrer();
    let volume = b.variante_plus_forte();
    b.ecouter_en_mesurant(volume);
    b.repondre(Reponse::B);
    b.ecouter_un_duel();
    b.repondre(Reponse::A);
    b.ecouter_un_passage();
    let (repris, _) = Deroulement::reprendre_depuis(&b.journal, contexte(), b.t).unwrap();
    assert_eq!(repris.duels_clos(), b.d.duels_clos());
    assert_eq!(duels_clos_du_journal(&b.journal).unwrap(), b.d.duels_clos());
}

#[test]
fn un_ecart_qui_ne_suit_pas_sa_reponse_rend_le_journal_incoherent() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    let mut journal = b.journal.clone();
    journal.push(Entree::EcartVolumeMesure {
        duel: 0,
        ecart_db: 1.0,
        horodatage_ms: b.t,
    });
    assert!(matches!(
        Deroulement::reprendre_depuis(&journal, contexte(), b.t),
        Err(ErreurReprise::Incoherent { .. })
    ));
}

#[test]
fn les_duels_d_un_test_fini_se_relisent_avec_leur_appli() {
    let mut b = Banc::nouveau(plan_gagnant());
    b.envoyer(E::PremierPlan("Valorant".into()));
    b.demarrer();
    b.ecouter_un_duel();
    b.repondre_selon(Preference::Variante);
    b.envoyer(E::PremierPlan("chrome".into()));
    b.ecouter_un_duel();
    b.repondre_selon(Preference::Variante);
    // Le duel de vérification suit (un changement est retenu).
    b.ecouter_un_duel();
    b.repondre_selon(Preference::Variante);
    assert_eq!(b.etat(), EtatTest::Termine);
    let clos = duels_clos_du_journal(&b.journal).unwrap();
    assert_eq!(clos.len(), 3);
    assert_eq!(clos[0].appli.as_deref(), Some("Valorant"));
    assert_eq!(clos[1].appli.as_deref(), Some("chrome"));
    assert_eq!(clos[2].sujet, Sujet::Verification);
    assert_eq!(clos[0].preference, Some(Preference::Variante));
    assert_eq!(duels_pour_empreinte(&b.journal), clos);
}

#[test]
fn un_test_non_termine_ou_bloque_par_les_controles_ne_compte_pas_pour_l_empreinte() {
    // Non terminé.
    let mut b = Banc::nouveau(plan_gagnant());
    b.demarrer();
    b.ecouter_un_duel();
    b.repondre(Reponse::A);
    assert!(duels_pour_empreinte(&b.journal).is_empty());
    assert_eq!(duels_clos_du_journal(&b.journal).unwrap().len(), 1);

    // Terminé, mais le seul contrôle a reçu A : rien n'est retenu.
    let mut b = Banc::nouveau(vec![controle(), zone(Zone::Graves, Sens::Plus, true)]);
    b.demarrer();
    b.ecouter_un_duel();
    b.repondre(Reponse::A);
    b.ecouter_un_duel();
    b.repondre(Reponse::B);
    assert_eq!(b.etat(), EtatTest::Termine);
    assert!(duels_pour_empreinte(&b.journal).is_empty());
}

#[test]
fn un_journal_vide_ou_sans_demarrage_ne_se_relit_pas() {
    assert_eq!(duels_clos_du_journal(&[]), Err(ErreurReprise::JournalVide));
    assert_eq!(
        duels_clos_du_journal(&[Entree::Reprise { horodatage_ms: 1 }]),
        Err(ErreurReprise::SansDemarrage)
    );
}
