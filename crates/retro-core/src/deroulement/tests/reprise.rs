//! Reprise après un arrêt brutal : `Deroulement::reprendre_depuis`.

use super::proprietes::{banc_au_hasard, duree_au_hasard, evenement_au_hasard};
use super::*;
use crate::hasard::{Hasard, HasardGraine};
use crate::zone::{Sens, Zone};
use Evenement as E;

fn graves_plus() -> DuelPrevu {
    zone(Zone::Graves, Sens::Plus, true)
}

/// Reconstruit un banc depuis le journal d'un autre, comme au redémarrage de l'appli.
fn rebatir(journal: &[Entree], t: u64) -> Banc {
    let (d, effets) = Deroulement::reprendre_depuis(journal, contexte(), t).unwrap();
    let mut b = Banc::autour(d, reference(), t);
    b.journal = journal.to_vec();
    b.executer(&effets);
    b
}

fn erreur(journal: &[Entree]) -> ErreurReprise {
    Deroulement::reprendre_depuis(journal, contexte(), T0).unwrap_err()
}

#[test]
fn un_journal_vide_ou_sans_demarrage_est_une_erreur_claire() {
    assert_eq!(erreur(&[]), ErreurReprise::JournalVide);
    let sans = [Entree::Reprise { horodatage_ms: 1 }];
    assert_eq!(erreur(&sans), ErreurReprise::SansDemarrage);
    assert_eq!(
        ErreurReprise::SansDemarrage.to_string(),
        "le journal ne commence pas par le démarrage du test"
    );
}

#[test]
fn un_test_deja_clos_ne_se_reprend_pas() {
    let mut b = Banc::nouveau(vec![controle()]);
    b.demarrer();
    let mut abandonne = b.journal.clone();
    abandonne.push(Entree::TestAbandonne { horodatage_ms: 5 });
    assert_eq!(erreur(&abandonne), ErreurReprise::TestClos);
    b.repondre(Reponse::Pareil);
    assert_eq!(erreur(&b.journal), ErreurReprise::TestClos);
}

#[test]
fn la_version_fournie_doit_etre_celle_du_journal() {
    let mut b = Banc::nouveau(vec![controle()]);
    b.demarrer();
    let mut c = contexte();
    c.reference.numero = 9;
    let e = Deroulement::reprendre_depuis(&b.journal, c, T0).unwrap_err();
    assert_eq!(
        e,
        ErreurReprise::MauvaiseVersion {
            attendue: 3,
            fournie: 9
        }
    );
}

#[test]
fn un_journal_incoherent_est_une_erreur_pas_une_panique() {
    let mut b = Banc::nouveau(vec![controle(), controle()]);
    b.demarrer();
    let depart = b.journal[0].clone();
    let cas = [
        Entree::ReponseDonnee {
            duel: 1,
            reponse: Reponse::A,
            horodatage_ms: 2,
        },
        Entree::ReponseDonnee {
            duel: 99,
            reponse: Reponse::A,
            horodatage_ms: 2,
        },
        Entree::DuelPasse {
            duel: 7,
            horodatage_ms: 2,
        },
        Entree::ReponseAnnulee {
            duel: 0,
            horodatage_ms: 2,
        },
        Entree::DuelVerificationAjoute {
            duel: verification(),
            horodatage_ms: 2,
        },
        depart.clone(),
    ];
    for entree in cas {
        let e = erreur(&[depart.clone(), entree.clone()]);
        assert!(
            matches!(e, ErreurReprise::Incoherent { entree: 1, .. }),
            "{entree:?} → {e:?}"
        );
    }
    // Des passages d'un duel inconnu sont simplement ignorés.
    let j = [
        depart,
        Entree::PassageDemarre {
            duel: 42,
            passage: 77,
            cote: Cote::A,
            appli_premier_plan: None,
            horodatage_ms: 2,
        },
    ];
    assert!(Deroulement::reprendre_depuis(&j, contexte(), T0).is_ok());
}

#[test]
fn le_test_repris_est_en_pause_reference_remise_et_rejoue_le_duel_interrompu() {
    let mut b = Banc::nouveau(vec![controle(), graves_plus(), controle()]);
    b.envoyer(E::PremierPlan("Jeu.exe".into()));
    b.demarrer();
    b.ecouter_un_duel();
    b.repondre(Reponse::A);
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    // Arrêt brutal au milieu du duel 1, la variante peut-être active.

    let t = b.t + 86_400_000;
    let (d, effets) = Deroulement::reprendre_depuis(&b.journal, contexte(), t).unwrap();
    assert_eq!(
        effets,
        vec![
            Effet::Appliquer {
                cote: None,
                courbe: reference()
            },
            Effet::Journaliser(Entree::Pause {
                raison: RaisonPause::Manuelle,
                horodatage_ms: t
            })
        ]
    );
    let mut r = Banc::autour(d, reference(), t);
    r.executer(&effets);
    let v = r.vue();
    assert_eq!(
        v.etat,
        EtatTest::EnPause {
            raison: RaisonPause::Manuelle
        }
    );
    assert_eq!((v.duel, v.total), (2, 3));
    assert_eq!(
        (v.passages_a, v.passages_b),
        (0, 0),
        "le duel sera rejoué en entier"
    );
    assert_eq!(v.historique, b.vue().historique);
    assert!(r.envoyer(E::Tic).is_empty());
    assert!(r.repondre(Reponse::B).is_empty());

    r.envoyer(E::PremierPlan("Jeu.exe".into()));
    let e = r.envoyer(E::Reprendre);
    let (cote_ref, _) = cotes_zone(Zone::Graves, Sens::Plus, &spectre());
    assert_eq!(
        e,
        vec![
            Effet::Journaliser(Entree::Reprise { horodatage_ms: t }),
            Effet::Journaliser(Entree::DuelDemarre {
                duel: 1,
                horodatage_ms: t
            }),
            Effet::Journaliser(Entree::PassageDemarre {
                duel: 1,
                passage: 0,
                cote: Cote::A,
                appli_premier_plan: Some("Jeu.exe".into()),
                horodatage_ms: t
            }),
            Effet::Appliquer {
                cote: Some(Cote::A),
                courbe: cote_ref
            },
        ]
    );
}

#[test]
fn la_reprise_respecte_annulations_duels_passes_et_recommences() {
    let mut b = Banc::nouveau(vec![controle(); 5]);
    b.demarrer();
    b.repondre(Reponse::A); // 0
    b.envoyer(E::PasserDuel); // 1
    b.repondre(Reponse::B); // 2
    b.repondre(Reponse::Pareil); // 3
    b.envoyer(E::Annuler); // revient au duel 3
    b.envoyer(E::RecommencerDuel);
    b.envoyer(E::PlusTard);
    let r = rebatir(&b.journal, b.t);
    assert_eq!(r.vue().historique, b.vue().historique);
    let reponses: Vec<_> = r.vue().historique.iter().map(|d| d.reponse).collect();
    assert_eq!(reponses, vec![Some(Reponse::A), None, Some(Reponse::B)]);
    assert_eq!(r.vue().duel, 4);
}

#[test]
fn la_reprise_retrouve_le_duel_de_verification_du_journal() {
    let mut b = Banc::nouveau(vec![graves_plus()]);
    b.demarrer();
    b.repondre(Reponse::B);
    b.ecouter_un_passage();
    assert!(b.vue().est_verification);

    // Le contexte propose un autre tirage : c'est celui du journal qui doit être gardé.
    let mut c = contexte();
    c.verification = duel(Sujet::Verification, true);
    let (d, effets) = Deroulement::reprendre_depuis(&b.journal, c, b.t).unwrap();
    let mut r = Banc::autour(d, reference(), b.t);
    r.executer(&effets);
    assert_eq!(r.vue().total, 2);
    assert!(r.vue().est_verification);
    let e = r.envoyer(E::Reprendre);
    assert_eq!(appliquees(&e), vec![b.appliquee_au_passage(1, 0)]);
    r.repondre(Reponse::A); // dans le tirage du journal, A est la nouvelle version
    let resultat = r.resultat().unwrap();
    assert_eq!(resultat.verification, Some(Reponse::A));
    assert_eq!(
        resultat.verification_preference,
        Some(crate::plan::Preference::Variante)
    );
}

#[test]
fn un_journal_coupe_juste_apres_la_derniere_reponse_se_termine_a_la_reprise() {
    let mut b = Banc::nouveau(vec![controle(), controle()]);
    b.demarrer();
    b.repondre(Reponse::A);
    b.repondre(Reponse::Pareil);
    let attendu = b.resultat().unwrap();
    // On retire la ligne « test terminé » : la coupure est tombée juste avant.
    let journal: Vec<Entree> = b
        .journal
        .iter()
        .filter(|e| !e.est_finale())
        .cloned()
        .collect();
    let mut r = rebatir(&journal, b.t);
    assert_eq!(r.vue().historique.len(), 2);
    let e = r.envoyer(E::Reprendre);
    assert_eq!(resultat_de(&e), Some(attendu));
    assert_eq!(r.etat(), EtatTest::Termine);
}

#[test]
fn la_courbe_de_reference_est_celle_du_journal() {
    let mut b = Banc::nouveau(vec![controle()]);
    b.demarrer();
    // Le profil fourni à la reprise a (par erreur) une autre courbe pour cette version.
    let mut c = contexte();
    c.reference.courbe = Courbe::default();
    let (_, effets) = Deroulement::reprendre_depuis(&b.journal, c, b.t).unwrap();
    assert_eq!(appliquees(&effets), vec![(None, reference())]);
}

/// Prépare l'original pour la comparaison : il rejoue lui aussi son duel en cours depuis
/// le premier passage, comme le fera le test repris.
fn remettre_au_debut_du_duel(b: &mut Banc) {
    b.envoyer(E::AppelEnCours(false));
    b.envoyer(E::Reprendre);
    b.envoyer(E::RecommencerDuel);
}

#[test]
fn aller_retour_un_test_repris_finit_comme_l_original() {
    let mut repris = 0;
    let mut clos = 0;
    for graine in 0..150 {
        let mut h = HasardGraine::new(1_000 + graine);
        let mut original = banc_au_hasard(&mut h);
        // Un début de test au hasard, de longueur variable.
        for _ in 0..h.entier(400) {
            original.avancer(duree_au_hasard(&mut h));
            original.envoyer(evenement_au_hasard(&mut h, 6, graine % 5 == 0));
        }
        let journal = original.journal.clone();
        let t = original.t + 1_000;
        let mut c = contexte();
        c.verification = original.d.verification_prevue;
        let reprise = Deroulement::reprendre_depuis(&journal, c, t);

        match original.etat() {
            EtatTest::Pret => {
                assert_eq!(reprise.unwrap_err(), ErreurReprise::JournalVide);
                continue;
            }
            EtatTest::Termine | EtatTest::Abandonne if !journal.is_empty() => {
                assert_eq!(reprise.unwrap_err(), ErreurReprise::TestClos);
                clos += 1;
                continue;
            }
            EtatTest::Abandonne => {
                assert_eq!(reprise.unwrap_err(), ErreurReprise::JournalVide);
                continue;
            }
            _ => {}
        }
        repris += 1;
        let (d, effets) = reprise.unwrap();
        let mut copie = Banc::autour(d, reference(), t);
        copie.journal = journal;
        copie.executer(&effets);

        // Ce qui est acquis est identique.
        assert_eq!(copie.vue().historique, original.vue().historique);
        assert_eq!(copie.vue().total, original.vue().total);
        assert_eq!(copie.vue().duel, original.vue().duel);
        assert_eq!(copie.d.applis, original.d.applis);
        assert_eq!(copie.d.plan, original.d.plan);

        // La même suite d'événements, sur les deux.
        original.t = t;
        for b in [&mut original, &mut copie] {
            b.envoyer(E::PremierPlan("Suite.exe".into()));
        }
        remettre_au_debut_du_duel(&mut original);
        copie.envoyer(E::Reprendre);
        assert_eq!(copie.etat(), original.etat());

        let mut suite = h.clone();
        let mut pas = 0;
        while original.etat() != EtatTest::Termine {
            pas += 1;
            assert!(pas < 5_000, "le test ne se termine pas");
            let duree = duree_au_hasard(&mut suite).min(2 * INTERVALLE);
            let evt = match suite.entier(10) {
                0..=5 => E::Tic,
                6 => E::PasserDuel,
                7 => E::Annuler,
                _ => {
                    E::Reponse([Reponse::A, Reponse::Pareil, Reponse::B][suite.entier(3) as usize])
                }
            };
            for b in [&mut original, &mut copie] {
                b.avancer(duree);
            }
            // Seule différence admise : le conseil de pause, car le temps d'écoute
            // cumulé repart de zéro après une reprise.
            let sans_conseil = |mut effets: Vec<Effet>| {
                effets.retain(|e| *e != Effet::ConseillerPause);
                effets
            };
            let attendus = sans_conseil(original.envoyer(evt.clone()));
            assert_eq!(sans_conseil(copie.envoyer(evt)), attendus);
        }
        assert_eq!(copie.etat(), EtatTest::Termine);
        assert_eq!(copie.vue().historique, original.vue().historique);
        assert_eq!(copie.resultat(), original.resultat());
        assert!(copie.resultat().is_some());
        // Le journal complet du test repris se relit sans erreur jusqu'à sa clôture.
        assert_eq!(erreur(&copie.journal), ErreurReprise::TestClos);
    }
    assert!(repris > 60, "{repris}");
    assert!(clos > 5, "{clos}");
}

#[test]
fn a_chaque_instant_le_journal_suffit_a_retrouver_les_reponses() {
    for graine in 0..12 {
        let mut h = HasardGraine::new(5_000 + graine);
        let mut b = banc_au_hasard(&mut h);
        for _ in 0..600 {
            b.avancer(duree_au_hasard(&mut h));
            b.envoyer(evenement_au_hasard(&mut h, 4, false));
            if matches!(b.etat(), EtatTest::Pret | EtatTest::Termine) {
                continue;
            }
            let mut c = contexte();
            c.verification = b.d.verification_prevue;
            let (d, _) = Deroulement::reprendre_depuis(&b.journal, c, b.t).unwrap();
            assert_eq!(d.vue(b.t).historique, b.vue().historique);
            assert_eq!(d.plan, b.d.plan);
            assert_eq!(d.applis, b.d.applis);
        }
    }
}
