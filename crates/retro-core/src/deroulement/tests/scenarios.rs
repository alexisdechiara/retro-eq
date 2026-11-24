//! Un test par règle de la spec, écrit comme un scénario.

use super::*;
use crate::plan::Preference;
use crate::zone::{Sens, Zone};
use Evenement as E;

fn graves_plus(reference_est_a: bool) -> DuelPrevu {
    zone(Zone::Graves, Sens::Plus, reference_est_a)
}

fn en_ecoute(duel: u32, passage: u32) -> EtatTest {
    EtatTest::EnEcoute { duel, passage }
}

fn en_pause(raison: RaisonPause) -> EtatTest {
    EtatTest::EnPause { raison }
}

fn passage(duel: u32, passage: u32, cote: Cote, appli: Option<&str>, t: u64) -> Effet {
    Effet::Journaliser(Entree::PassageDemarre {
        duel,
        passage,
        cote,
        appli_premier_plan: appli.map(String::from),
        horodatage_ms: t,
    })
}

fn remise(b: &Banc) -> Effet {
    Effet::Appliquer {
        cote: None,
        courbe: b.reference.clone(),
    }
}

// ----- Démarrage et passages -----

#[test]
fn avant_de_demarrer_rien_ne_se_passe() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    assert_eq!(b.etat(), EtatTest::Pret);
    for evt in [
        E::Tic,
        E::Reponse(Reponse::A),
        E::PlusTard,
        E::Annuler,
        E::Pause,
        E::Reprendre,
        E::PasserDuel,
        E::RecommencerDuel,
        E::Reecouter {
            cote: Cote::A,
            appuye: true,
        },
    ] {
        assert!(b.envoyer(evt).is_empty());
        assert_eq!(b.etat(), EtatTest::Pret);
    }
}

#[test]
fn demarrer_journalise_le_test_puis_lance_le_premier_passage() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    let e = b.demarrer();
    let (cote_ref, _) = cotes_zone(Zone::Graves, Sens::Plus, &spectre());
    assert_eq!(e.len(), 4, "{e:?}");
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::TestDemarre {
            test_id: "t1".into(),
            profil_id: "casque".into(),
            version_reference: 3,
            courbe_reference: reference(),
            mode: Mode::DuelsEnJeu { precis: false },
            plan: vec![graves_plus(true), controle()],
            reglages: ReglagesTest::nouveau(10_000, true),
            horodatage_ms: T0,
        })
    );
    assert_eq!(
        e[1],
        Effet::Journaliser(Entree::DuelDemarre {
            duel: 0,
            horodatage_ms: T0
        })
    );
    assert_eq!(e[2], passage(0, 0, Cote::A, None, T0));
    assert_eq!(
        e[3],
        Effet::Appliquer {
            cote: Some(Cote::A),
            courbe: cote_ref
        }
    );
    assert_eq!(b.etat(), en_ecoute(0, 0));
    // Un second « démarrer » est ignoré.
    assert!(b.demarrer().is_empty());
}

#[test]
fn un_duel_joue_ses_6_passages_dans_l_ordre_puis_pose_la_question() {
    // La référence est B : A fait donc entendre la variante.
    let mut b = Banc::nouveau(vec![graves_plus(false)]);
    let (cote_ref, cote_var) = cotes_zone(Zone::Graves, Sens::Plus, &spectre());
    assert_ne!(cote_ref, cote_var);
    assert_ne!(
        cote_ref,
        reference(),
        "la copie égalisée a son propre préampli"
    );
    let courbe_de = |c: Cote| if c == Cote::B { &cote_ref } else { &cote_var };

    let e = b.demarrer();
    assert_eq!(
        appliquees(&e),
        vec![(Some(ORDRE[0]), courbe_de(ORDRE[0]).clone())]
    );
    for p in 1..6u32 {
        // Tant que l'intervalle n'est pas écoulé, un tic ne fait rien.
        assert!(b.tic_apres(INTERVALLE - 1).is_empty());
        let e = b.tic_apres(1);
        let cote = ORDRE[p as usize];
        assert_eq!(
            e,
            vec![
                passage(0, p, cote, None, b.t),
                Effet::Appliquer {
                    cote: Some(cote),
                    courbe: courbe_de(cote).clone()
                }
            ]
        );
        assert_eq!(b.etat(), en_ecoute(0, p));
    }
    let e = b.ecouter_un_passage();
    assert_eq!(e, vec![remise(&b), Effet::DemanderReponse]);
    assert_eq!(b.etat(), EtatTest::AttenteReponse);
    // En attente, le temps qui passe ne change rien.
    assert!(b.tic_apres(10 * INTERVALLE).is_empty());
}

#[test]
fn un_duel_de_controle_fait_entendre_deux_courbes_identiques_au_bit_pres() {
    let mut b = Banc::nouveau(vec![controle()]);
    b.demarrer();
    b.ecouter_un_duel();
    let cotes = appliquees(&b.tous);
    let a = &cotes.iter().find(|(c, _)| *c == Some(Cote::A)).unwrap().1;
    let bb = &cotes.iter().find(|(c, _)| *c == Some(Cote::B)).unwrap().1;
    assert_eq!(a.preampli_db.to_bits(), bb.preampli_db.to_bits());
    assert_eq!(a.filtres, bb.filtres);
    assert_eq!(a.filtres, reference().filtres);
    // Même le contrôle passe par le préampli de sécurité commun.
    let (attendue, _) = egaliser_volumes(&reference(), &reference(), &spectre());
    assert_eq!(a, &attendue);
}

#[test]
fn en_profil_contre_profil_la_variante_est_l_autre_profil() {
    let autre = Courbe {
        preampli_db: 0.0,
        filtres: vec![retro_dsp::Filtre {
            type_filtre: retro_dsp::TypeFiltre::PlateauBas,
            frequence_hz: 100.0,
            gain_db: -3.0,
            q: 0.71,
        }],
    };
    let mode = Mode::ProfilContreProfil {
        autre: autre.clone(),
        duels: 1,
    };
    let mut b = Banc::avec(parametres(mode, vec![duel(Sujet::AutreProfil, true)]));
    b.demarrer();
    b.ecouter_un_passage();
    let (cote_ref, cote_autre) = egaliser_volumes(&reference(), &autre, &spectre());
    let cotes = appliquees(&b.tous);
    assert_eq!(cotes[0], (Some(Cote::A), cote_ref));
    assert_eq!(cotes[1], (Some(Cote::B), cote_autre));
}

#[test]
fn le_pas_de_la_variante_est_celui_de_la_zone_dans_la_version() {
    // Graves : pas de 2 dB dans `version()`, Air : 1 dB.
    let mut b = Banc::nouveau(vec![zone(Zone::Air, Sens::Moins, true)]);
    b.demarrer();
    b.ecouter_un_passage();
    let cotes = appliquees(&b.tous);
    let variante_entendue = &cotes[1].1;
    let filtre = variante_entendue.filtres.last().unwrap();
    assert_eq!(filtre.gain_db, -1.0);
    assert_eq!(filtre.type_filtre, retro_dsp::TypeFiltre::PlateauHaut);
}

#[test]
fn un_nouveau_spectre_sert_a_partir_du_duel_suivant() {
    let mut b = Banc::nouveau(vec![graves_plus(true), graves_plus(true)]);
    let mut grave = spectre();
    for (f, p) in grave.frequences_hz.iter().zip(grave.puissances.iter_mut()) {
        *p = if *f < 250.0 { 100.0 } else { 0.01 };
    }
    let ancien = cotes_zone(Zone::Graves, Sens::Plus, &spectre());
    let nouveau = cotes_zone(Zone::Graves, Sens::Plus, &grave);
    assert_ne!(ancien, nouveau, "le spectre doit changer la compensation");

    b.demarrer();
    b.d.maj_spectre(grave);
    let e = b.ecouter_un_passage();
    assert_eq!(appliquees(&e), vec![(Some(Cote::B), ancien.1.clone())]);
    let e = b.repondre(Reponse::Pareil);
    assert_eq!(appliquees(&e), vec![(Some(Cote::A), nouveau.0.clone())]);
    let e = b.ecouter_un_passage();
    assert_eq!(appliquees(&e), vec![(Some(Cote::B), nouveau.1.clone())]);
}

// ----- Le temps -----

#[test]
fn un_tic_en_retard_de_moins_de_deux_intervalles_passe_au_passage_suivant() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.tic_apres(2 * INTERVALLE);
    assert_eq!(
        b.etat(),
        en_ecoute(0, 1),
        "un seul passage d'avance, jamais deux"
    );
    // Le passage suivant dure un intervalle entier à partir de ce tic.
    assert!(b.tic_apres(INTERVALLE - 1).is_empty());
    b.tic_apres(1);
    assert_eq!(b.etat(), en_ecoute(0, 2));
}

#[test]
fn un_tic_tres_en_retard_recommence_le_passage_au_lieu_d_en_sauter() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    assert_eq!(b.etat(), en_ecoute(0, 1));
    // Le PC a dormi une heure : 360 intervalles « écoulés ».
    let e = b.tic_apres(3_600_000);
    assert_eq!(e.len(), 2);
    assert_eq!(e[0], passage(0, 1, Cote::B, None, b.t));
    assert!(matches!(
        e[1],
        Effet::Appliquer {
            cote: Some(Cote::B),
            ..
        }
    ));
    assert_eq!(b.etat(), en_ecoute(0, 1));
    assert!(b.tic_apres(INTERVALLE - 1).is_empty());
    b.tic_apres(1);
    assert_eq!(b.etat(), en_ecoute(0, 2));
    // Juste au-dessus de deux intervalles : encore un recommencement.
    b.tic_apres(2 * INTERVALLE + 1);
    assert_eq!(b.etat(), en_ecoute(0, 2));
}

#[test]
fn une_horloge_qui_recule_ne_casse_rien() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.t -= 500_000;
    assert!(b.envoyer(E::Tic).is_empty());
    let v = b.vue();
    assert_eq!(v.progression, 0.0);
    assert_eq!(v.ms_avant_bascule, Some(INTERVALLE));
}

#[test]
fn le_temps_passe_en_pause_ne_compte_pas() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    b.avancer(4_000);
    b.envoyer(E::Pause);
    assert!(
        b.tic_apres(3_600_000).is_empty(),
        "un tic en pause ne fait rien"
    );
    let e = b.envoyer(E::Reprendre);
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::Reprise { horodatage_ms: b.t })
    );
    assert_eq!(e[1], passage(0, 1, Cote::B, None, b.t));
    assert_eq!(b.etat(), en_ecoute(0, 1));
    assert!(
        b.tic_apres(INTERVALLE - 1).is_empty(),
        "le passage repart de zéro"
    );
    b.tic_apres(1);
    assert_eq!(b.etat(), en_ecoute(0, 2));
}

// ----- Réponse -----

#[test]
fn la_reponse_est_journalisee_en_premier_puis_le_duel_suivant_commence() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    b.demarrer();
    b.ecouter_un_duel();
    b.avancer(1234);
    let e = b.repondre(Reponse::Pareil);
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::ReponseDonnee {
            duel: 0,
            reponse: Reponse::Pareil,
            horodatage_ms: b.t
        })
    );
    assert_eq!(e[1], Effet::Bip);
    assert_eq!(e[2], Effet::MasquerQuestion);
    assert_eq!(
        e[3],
        Effet::Journaliser(Entree::DuelDemarre {
            duel: 1,
            horodatage_ms: b.t
        })
    );
    assert_eq!(e[4], passage(1, 0, Cote::A, None, b.t));
    assert!(matches!(
        e[5],
        Effet::Appliquer {
            cote: Some(Cote::A),
            ..
        }
    ));
    assert_eq!(e.len(), 6);
    assert_eq!(b.etat(), en_ecoute(1, 0));
}

#[test]
fn une_reponse_peut_arriver_avant_la_fin_des_passages() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    b.demarrer();
    b.ecouter_un_passage();
    let e = b.repondre(Reponse::B);
    assert!(matches!(
        e[0],
        Effet::Journaliser(Entree::ReponseDonnee { duel: 0, .. })
    ));
    assert!(
        !e.contains(&Effet::MasquerQuestion),
        "aucune question n'était affichée"
    );
    assert_eq!(b.etat(), en_ecoute(1, 0));
}

#[test]
fn sans_le_reglage_bip_aucun_bip() {
    let mut p = parametres(
        Mode::DuelsEnJeu { precis: false },
        vec![controle(), controle()],
    );
    p.reglages = ReglagesTest::nouveau(INTERVALLE as u32, false);
    let mut b = Banc::avec(p);
    b.demarrer();
    b.repondre(Reponse::A);
    b.repondre(Reponse::A);
    assert_eq!(b.compte(&Effet::Bip), 0);
    assert_eq!(b.etat(), EtatTest::Termine);
}

#[test]
fn une_reponse_en_pause_est_ignoree() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.envoyer(E::Pause);
    assert!(b.repondre(Reponse::A).is_empty());
    assert_eq!(b.etat(), en_pause(RaisonPause::Manuelle));
    assert!(b.vue().historique.is_empty());
}

// ----- Réécoute -----

#[test]
fn maintenir_une_touche_fait_reecouter_ce_cote_et_la_relacher_remet_la_reference() {
    let mut b = Banc::nouveau(vec![graves_plus(false), controle()]);
    let (cote_ref, cote_var) = cotes_zone(Zone::Graves, Sens::Plus, &spectre());
    b.demarrer();
    b.ecouter_un_duel();

    let appui = |cote| E::Reecouter { cote, appuye: true };
    let relache = |cote| E::Reecouter {
        cote,
        appuye: false,
    };

    let e = b.envoyer(appui(Cote::A));
    assert_eq!(
        e,
        vec![Effet::Appliquer {
            cote: Some(Cote::A),
            courbe: cote_var.clone()
        }]
    );
    assert_eq!(b.vue().cote_actif, Some(Cote::A));
    assert!(b.envoyer(appui(Cote::A)).is_empty(), "répétition de touche");
    assert_eq!(b.envoyer(relache(Cote::A)), vec![remise(&b)]);
    assert_eq!(b.vue().cote_actif, None);

    let e = b.envoyer(appui(Cote::B));
    assert_eq!(
        e,
        vec![Effet::Appliquer {
            cote: Some(Cote::B),
            courbe: cote_ref
        }]
    );
    // Relâcher l'autre touche ne coupe pas la réécoute en cours.
    assert!(b.envoyer(relache(Cote::A)).is_empty());
    assert_eq!(b.vue().cote_actif, Some(Cote::B));
    // Passer d'une touche à l'autre sans relâcher.
    b.envoyer(appui(Cote::A));
    assert!(b.envoyer(relache(Cote::B)).is_empty());
    assert_eq!(b.envoyer(relache(Cote::A)), vec![remise(&b)]);

    // Rien de tout cela n'est une réponse.
    assert_eq!(b.etat(), EtatTest::AttenteReponse);
    assert!(b.vue().historique.is_empty());
    assert!(!journalisees(&b.tous)
        .iter()
        .any(|e| matches!(e, Entree::ReponseDonnee { .. })));
}

#[test]
fn la_reecoute_est_ignoree_pendant_l_ecoute() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    for appuye in [true, false] {
        let e = b.envoyer(E::Reecouter {
            cote: Cote::B,
            appuye,
        });
        assert!(e.is_empty());
    }
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

#[test]
fn une_pause_pendant_une_reecoute_remet_la_reference() {
    let mut b = Banc::nouveau(vec![graves_plus(false)]);
    b.demarrer();
    b.ecouter_un_duel();
    b.envoyer(E::Reecouter {
        cote: Cote::A,
        appuye: true,
    });
    b.envoyer(E::AppelEnCours(true));
    assert!(b.reference_remise());
    // La touche relâchée pendant la pause n'a plus d'effet.
    let e = b.envoyer(E::Reecouter {
        cote: Cote::A,
        appuye: false,
    });
    assert!(e.is_empty());
}

// ----- Annuler -----

#[test]
fn annuler_sans_aucune_reponse_ne_fait_rien() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    b.demarrer();
    assert!(b.envoyer(E::Annuler).is_empty());
    b.envoyer(E::PasserDuel);
    assert!(
        b.envoyer(E::Annuler).is_empty(),
        "un duel passé n'est pas une réponse"
    );
    assert_eq!(b.etat(), en_ecoute(1, 0));
}

#[test]
fn annuler_defait_la_derniere_reponse_et_rejoue_ce_duel_depuis_le_debut() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle(), controle()]);
    b.demarrer();
    b.repondre(Reponse::A);
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    assert_eq!(b.etat(), en_ecoute(1, 2));
    let e = b.envoyer(E::Annuler);
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::ReponseAnnulee {
            duel: 0,
            horodatage_ms: b.t
        })
    );
    assert_eq!(
        e[1],
        Effet::Journaliser(Entree::DuelDemarre {
            duel: 0,
            horodatage_ms: b.t
        })
    );
    assert_eq!(e[2], passage(0, 0, Cote::A, None, b.t));
    assert!(matches!(
        e[3],
        Effet::Appliquer {
            cote: Some(Cote::A),
            ..
        }
    ));
    assert_eq!(b.etat(), en_ecoute(0, 0));
    assert!(b.vue().historique.is_empty());
    // Plus rien à annuler.
    assert!(b.envoyer(E::Annuler).is_empty());
}

#[test]
fn annuler_pendant_la_question_la_masque() {
    let mut b = Banc::nouveau(vec![controle(), controle()]);
    b.demarrer();
    b.repondre(Reponse::A);
    b.ecouter_un_duel();
    let e = b.envoyer(E::Annuler);
    assert!(e.contains(&Effet::MasquerQuestion));
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

#[test]
fn annuler_remonte_a_la_derniere_reponse_meme_par_dessus_un_duel_passe() {
    let mut b = Banc::nouveau(vec![controle(), controle(), controle(), controle()]);
    b.demarrer();
    b.repondre(Reponse::A); // duel 0
    b.repondre(Reponse::B); // duel 1
    b.envoyer(E::PasserDuel); // duel 2
    assert_eq!(b.etat(), en_ecoute(3, 0));
    b.envoyer(E::Annuler);
    // La dernière réponse est celle du duel 1 : on y revient, le duel 2 sera rejoué aussi.
    assert_eq!(b.etat(), en_ecoute(1, 0));
    assert_eq!(b.vue().historique.len(), 1);
    b.envoyer(E::Annuler);
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

#[test]
fn annuler_pendant_la_verification_retire_le_duel_de_verification() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.repondre(Reponse::B);
    assert!(b.vue().est_verification);
    assert_eq!(b.vue().total, 2);
    b.envoyer(E::Annuler);
    assert_eq!(b.etat(), en_ecoute(0, 0));
    assert_eq!(b.vue().total, 1);
    assert!(!b.vue().est_verification);
    // Cette fois « Pareil » : plus de changement, donc plus de vérification.
    b.repondre(Reponse::Pareil);
    assert_eq!(b.etat(), EtatTest::Termine);
    assert_eq!(b.resultat().unwrap().verification, None);
}

// ----- Plus tard, pause, appel -----

#[test]
fn plus_tard_met_en_pause_et_le_duel_est_rejoue_depuis_son_premier_passage() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    let e = b.envoyer(E::PlusTard);
    assert_eq!(
        e,
        vec![
            remise(&b),
            Effet::Journaliser(Entree::Pause {
                raison: RaisonPause::PlusTard,
                horodatage_ms: b.t
            })
        ]
    );
    assert_eq!(b.etat(), en_pause(RaisonPause::PlusTard));
    assert_eq!((b.vue().passages_a, b.vue().passages_b), (0, 0));
    b.avancer(60_000);
    let e = b.envoyer(E::Reprendre);
    assert_eq!(
        journalisees(&e),
        vec![
            Entree::Reprise { horodatage_ms: b.t },
            Entree::DuelDemarre {
                duel: 0,
                horodatage_ms: b.t
            },
            Entree::PassageDemarre {
                duel: 0,
                passage: 0,
                cote: Cote::A,
                appli_premier_plan: None,
                horodatage_ms: b.t
            }
        ]
    );
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

#[test]
fn plus_tard_pendant_la_question_masque_la_question_et_rejoue_le_duel() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_duel();
    let e = b.envoyer(E::PlusTard);
    assert!(e.contains(&Effet::MasquerQuestion));
    b.envoyer(E::Reprendre);
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

#[test]
fn la_pause_manuelle_remet_la_reference_et_reprend_au_debut_du_passage_interrompu() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    b.avancer(7_000);
    let e = b.envoyer(E::Pause);
    assert_eq!(e[0], remise(&b), "la référence d'abord");
    assert_eq!(
        e[1],
        Effet::Journaliser(Entree::Pause {
            raison: RaisonPause::Manuelle,
            horodatage_ms: b.t
        })
    );
    assert_eq!(b.etat(), en_pause(RaisonPause::Manuelle));
    assert!(b.envoyer(E::Pause).is_empty(), "déjà en pause");
    assert!(b.envoyer(E::PlusTard).is_empty());
    let e = b.envoyer(E::Reprendre);
    assert_eq!(e[1], passage(0, 2, Cote::B, None, b.t));
    assert_eq!(b.etat(), en_ecoute(0, 2));
    assert!(
        b.envoyer(E::Reprendre).is_empty(),
        "reprendre en écoute est ignoré"
    );
}

#[test]
fn une_pause_pendant_la_question_revient_a_la_question() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_duel();
    let e = b.envoyer(E::Pause);
    assert!(e.contains(&Effet::MasquerQuestion));
    assert_eq!((b.vue().passages_a, b.vue().passages_b), (3, 3));
    let e = b.envoyer(E::Reprendre);
    assert_eq!(
        e,
        vec![
            Effet::Journaliser(Entree::Reprise { horodatage_ms: b.t }),
            Effet::DemanderReponse
        ]
    );
    assert_eq!(b.etat(), EtatTest::AttenteReponse);
    assert!(b.reference_remise());
}

#[test]
fn un_appel_met_en_pause_et_sa_fin_reprend_au_debut_du_passage() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    b.avancer(3_000);
    let e = b.envoyer(E::AppelEnCours(true));
    assert_eq!(
        e,
        vec![
            remise(&b),
            Effet::Journaliser(Entree::Pause {
                raison: RaisonPause::Appel,
                horodatage_ms: b.t
            })
        ]
    );
    assert_eq!(b.etat(), en_pause(RaisonPause::Appel));
    assert!(
        b.envoyer(E::AppelEnCours(true)).is_empty(),
        "valeur répétée"
    );
    b.avancer(600_000);
    let e = b.envoyer(E::AppelEnCours(false));
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::Reprise { horodatage_ms: b.t })
    );
    assert_eq!(e[1], passage(0, 1, Cote::B, None, b.t));
    assert_eq!(b.etat(), en_ecoute(0, 1));
    assert!(b.envoyer(E::AppelEnCours(false)).is_empty());
}

#[test]
fn un_appel_pendant_une_pause_manuelle_ne_relance_pas_le_test_a_sa_fin() {
    for premiere in [E::Pause, E::PlusTard] {
        let mut b = Banc::nouveau(vec![graves_plus(true)]);
        b.demarrer();
        b.envoyer(premiere);
        let etat = b.etat();
        assert!(b.envoyer(E::AppelEnCours(true)).is_empty());
        assert!(b.envoyer(E::AppelEnCours(false)).is_empty());
        assert_eq!(b.etat(), etat);
    }
}

#[test]
fn demander_la_pause_pendant_un_appel_la_rend_manuelle() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    b.envoyer(E::AppelEnCours(true));
    let e = b.envoyer(E::Pause);
    assert_eq!(
        e,
        vec![Effet::Journaliser(Entree::Pause {
            raison: RaisonPause::Manuelle,
            horodatage_ms: b.t
        })]
    );
    assert!(b.envoyer(E::AppelEnCours(false)).is_empty());
    assert_eq!(b.etat(), en_pause(RaisonPause::Manuelle));
    b.envoyer(E::Reprendre);
    assert_eq!(b.etat(), en_ecoute(0, 1));
}

#[test]
fn reprendre_a_la_main_pendant_un_appel_est_respecte() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.envoyer(E::AppelEnCours(true));
    b.envoyer(E::Reprendre);
    assert_eq!(b.etat(), en_ecoute(0, 0));
    // L'appli qui répète « appel en cours » ne remet pas en pause.
    assert!(b.envoyer(E::AppelEnCours(true)).is_empty());
    assert!(b.envoyer(E::AppelEnCours(false)).is_empty());
    assert_eq!(b.etat(), en_ecoute(0, 0));
    // Un nouvel appel, lui, remet en pause.
    b.envoyer(E::AppelEnCours(true));
    assert_eq!(b.etat(), en_pause(RaisonPause::Appel));
}

#[test]
fn demarrer_pendant_un_appel_attend_la_fin_de_l_appel() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    assert!(b.envoyer(E::AppelEnCours(true)).is_empty());
    let e = b.demarrer();
    assert!(matches!(
        e[0],
        Effet::Journaliser(Entree::TestDemarre { .. })
    ));
    assert_eq!(b.etat(), en_pause(RaisonPause::Appel));
    assert!(b.reference_remise());
    let e = b.envoyer(E::AppelEnCours(false));
    assert!(journalisees(&e).contains(&Entree::DuelDemarre {
        duel: 0,
        horodatage_ms: b.t
    }));
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

// ----- Passer, recommencer -----

#[test]
fn passer_un_duel_le_note_passe_sans_reponse() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    b.demarrer();
    b.ecouter_un_passage();
    let e = b.envoyer(E::PasserDuel);
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::DuelPasse {
            duel: 0,
            horodatage_ms: b.t
        })
    );
    assert_eq!(b.etat(), en_ecoute(1, 0));
    assert_eq!(b.vue().historique[0].reponse, None);
    b.envoyer(E::PasserDuel);
    assert_eq!(b.etat(), EtatTest::Termine);
    let r = b.resultat().unwrap();
    assert!(r.zones.is_empty());
    assert_eq!(r.controles.total, 1);
    assert_eq!(r.controles.repondus, 0);
}

#[test]
fn recommencer_un_duel_repart_de_son_premier_passage() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    let e = b.envoyer(E::RecommencerDuel);
    assert_eq!(
        e[0],
        Effet::Journaliser(Entree::DuelRecommence {
            duel: 0,
            horodatage_ms: b.t
        })
    );
    assert_eq!(e[1], passage(0, 0, Cote::A, None, b.t));
    assert_eq!(b.etat(), en_ecoute(0, 0));
    b.ecouter_un_duel();
    let e = b.envoyer(E::RecommencerDuel);
    assert!(e.contains(&Effet::MasquerQuestion));
    assert_eq!(b.etat(), en_ecoute(0, 0));
}

// ----- Appli au premier plan -----

#[test]
fn chaque_passage_note_l_appli_au_premier_plan_et_le_duel_garde_la_plus_vue() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    assert!(b.envoyer(E::PremierPlan("Jeu.exe".into())).is_empty());
    let e = b.demarrer();
    assert_eq!(e[2], passage(0, 0, Cote::A, Some("Jeu.exe"), b.t));
    b.envoyer(E::PremierPlan("Discord.exe".into()));
    let e = b.ecouter_un_passage();
    assert_eq!(e[0], passage(0, 1, Cote::B, Some("Discord.exe"), b.t));
    b.envoyer(E::PremierPlan(String::new()));
    let e = b.ecouter_un_passage();
    assert_eq!(
        e[0],
        passage(0, 2, Cote::B, None, b.t),
        "chaîne vide : inconnue"
    );
    b.envoyer(E::PremierPlan("Discord.exe".into()));
    b.ecouter_un_passage();
    b.envoyer(E::PremierPlan("Jeu.exe".into()));
    b.ecouter_un_passage();
    // Passages : Jeu, Discord, (rien), Discord, Jeu → égalité, la première vue gagne.
    b.envoyer(E::PremierPlan("Discord.exe".into()));
    b.ecouter_un_passage();
    // Jeu 2, Discord 3.
    b.ecouter_un_passage();
    b.repondre(Reponse::A);
    b.envoyer(E::PremierPlan("Navigateur.exe".into()));
    b.repondre(Reponse::Pareil); // le contrôle n'entre pas dans les préférences par appli
    let r = b.resultat().unwrap();
    assert_eq!(r.par_appli.len(), 1);
    assert_eq!(r.par_appli[0].appli, "Discord.exe");
    assert_eq!(r.par_appli[0].duels, 1);
    assert_eq!(r.par_appli[0].preference, Preference::Reference);
}

#[test]
fn a_egalite_l_appli_vue_en_premier_est_retenue_et_un_duel_rejoue_repart_de_zero() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.envoyer(E::PremierPlan("Ancien.exe".into()));
    b.demarrer();
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    // Tout ce qui a été vu avant de recommencer est oublié.
    b.envoyer(E::PremierPlan("Jeu.exe".into()));
    b.envoyer(E::RecommencerDuel);
    b.envoyer(E::PremierPlan("Discord.exe".into()));
    b.ecouter_un_passage();
    b.repondre(Reponse::Pareil);
    let r = b.resultat().unwrap();
    assert_eq!(r.par_appli.len(), 1);
    assert_eq!(r.par_appli[0].appli, "Jeu.exe");
}

// ----- Fin du test -----

#[test]
fn sans_changement_le_test_se_termine_apres_le_dernier_duel() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    b.demarrer();
    b.repondre(Reponse::A); // la référence est A : pas de changement
    b.ecouter_un_duel();
    let e = b.repondre(Reponse::Pareil);
    let r = resultat_de(&e).unwrap();
    assert!(!r.a_des_changements());
    assert_eq!(r.verification, None);
    assert!(matches!(
        e[0],
        Effet::Journaliser(Entree::ReponseDonnee { duel: 1, .. })
    ));
    let n = e.len();
    assert_eq!(e[n - 3], remise(&b));
    assert_eq!(
        e[n - 2],
        Effet::Journaliser(Entree::TestTermine {
            resultat: r.clone(),
            horodatage_ms: b.t
        })
    );
    assert_eq!(e[n - 1], Effet::Termine(r));
    assert_eq!(b.etat(), EtatTest::Termine);
    assert!(b.reference_remise());
    // Un test terminé ignore tout (vérifié par le banc à chaque événement).
    for evt in [E::Tic, E::Annuler, E::Reprendre, E::Abandonner, E::Demarrer] {
        assert!(b.envoyer(evt).is_empty());
    }
}

#[test]
fn avec_un_changement_un_duel_de_verification_est_ajoute_et_joue() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    assert_eq!(b.vue().total, 1);
    let e = b.repondre(Reponse::B); // la variante gagne : Graves change
    assert_eq!(
        journalisees(&e)[1..],
        [
            Entree::DuelVerificationAjoute {
                duel: verification(),
                horodatage_ms: b.t
            },
            Entree::DuelDemarre {
                duel: 1,
                horodatage_ms: b.t
            },
            Entree::PassageDemarre {
                duel: 1,
                passage: 0,
                cote: Cote::A,
                appli_premier_plan: None,
                horodatage_ms: b.t
            }
        ]
    );
    assert!(resultat_de(&e).is_none());
    let v = b.vue();
    assert_eq!((v.duel, v.total), (2, 2), "le total grandit d'un duel");
    assert!(v.est_verification);

    // Dans la vérification, la référence est B : A fait entendre la nouvelle courbe.
    let nouvelle = calculer_resultat(
        &reference(),
        &version(),
        &ReglagesVariantes::default(),
        &[graves_plus(true)],
        &[Some(Reponse::B)],
        None,
        &[None],
    )
    .nouvelle_courbe;
    let (cote_ref, cote_nouvelle) = egaliser_volumes(&reference(), &nouvelle, &spectre());
    assert_eq!(appliquees(&e), vec![(Some(Cote::A), cote_nouvelle)]);
    let e = b.ecouter_un_passage();
    assert_eq!(appliquees(&e), vec![(Some(Cote::B), cote_ref)]);

    b.ecouter_un_duel();
    let e = b.repondre(Reponse::A);
    let r = resultat_de(&e).unwrap();
    assert_eq!(r.verification, Some(Reponse::A));
    assert_eq!(r.verification_preference, Some(Preference::Variante));
    assert_eq!(r.nouvelle_courbe, nouvelle);
    assert_eq!(b.etat(), EtatTest::Termine);
    assert!(b.reference_remise());
    assert_eq!(b.vue().historique.len(), 2);
    assert_eq!(b.vue().historique[1].sujet, Sujet::Verification);
}

#[test]
fn passer_le_duel_de_verification_termine_le_test_sans_verification() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.repondre(Reponse::B);
    b.envoyer(E::PasserDuel);
    assert_eq!(b.etat(), EtatTest::Termine);
    let r = b.resultat().unwrap();
    assert!(r.a_des_changements());
    assert_eq!(r.verification, None);
}

#[test]
fn le_mode_une_seule_zone_a_aussi_sa_verification_mais_pas_profil_contre_profil() {
    let mode = Mode::UneSeuleZone {
        zone: Zone::Graves,
        duels: 1,
    };
    let mut b = Banc::avec(parametres(mode, vec![graves_plus(true)]));
    b.demarrer();
    b.repondre(Reponse::B);
    assert!(b.vue().est_verification);

    let mode = Mode::ProfilContreProfil {
        autre: Courbe::default(),
        duels: 1,
    };
    let mut b = Banc::avec(parametres(mode, vec![duel(Sujet::AutreProfil, true)]));
    b.demarrer();
    b.repondre(Reponse::B);
    assert_eq!(b.etat(), EtatTest::Termine);
    let r = b.resultat().unwrap();
    assert_eq!(r.autre_profil.unwrap().preference, Preference::Variante);
}

#[test]
fn un_plan_vide_se_termine_tout_de_suite() {
    let mut b = Banc::nouveau(vec![]);
    assert_eq!((b.vue().duel, b.vue().total), (0, 0));
    let e = b.demarrer();
    assert!(matches!(
        e[0],
        Effet::Journaliser(Entree::TestDemarre { .. })
    ));
    assert!(resultat_de(&e).is_some());
    assert_eq!(b.etat(), EtatTest::Termine);
}

#[test]
fn abandonner_remet_la_reference_et_ne_rend_aucun_resultat() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    b.demarrer();
    b.repondre(Reponse::B);
    b.ecouter_un_passage();
    let e = b.envoyer(E::Abandonner);
    assert_eq!(
        e,
        vec![
            remise(&b),
            Effet::Journaliser(Entree::TestAbandonne { horodatage_ms: b.t })
        ]
    );
    assert_eq!(b.etat(), EtatTest::Abandonne);
    assert!(b.resultat().is_none());
    assert!(b.envoyer(E::Reprendre).is_empty());
    assert!(b.envoyer(E::Abandonner).is_empty());
}

#[test]
fn abandonner_depuis_la_question_ou_une_pause_ou_avant_le_depart() {
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.ecouter_un_duel();
    let e = b.envoyer(E::Abandonner);
    assert!(e.contains(&Effet::MasquerQuestion));
    assert_eq!(b.etat(), EtatTest::Abandonne);

    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    b.demarrer();
    b.envoyer(E::PlusTard);
    let e = b.envoyer(E::Abandonner);
    assert!(journalisees(&e).contains(&Entree::TestAbandonne { horodatage_ms: b.t }));

    // Avant le départ : rien n'a été journalisé, donc rien à clore dans le journal.
    let mut b = Banc::nouveau(vec![graves_plus(true)]);
    let e = b.envoyer(E::Abandonner);
    assert_eq!(e, vec![remise(&b)]);
    assert_eq!(b.etat(), EtatTest::Abandonne);
    assert!(b.demarrer().is_empty());
}

// ----- Pause conseillée -----

#[test]
fn la_pause_est_conseillee_une_seule_fois_apres_le_temps_d_ecoute_cumule() {
    let mut p = parametres(Mode::DuelsEnJeu { precis: false }, vec![controle(); 4]);
    p.reglages = p.reglages.avec_pause_conseillee_apres_ms(25_000);
    let mut b = Banc::avec(p);
    b.demarrer();
    b.ecouter_un_passage(); // 10 s
    b.ecouter_un_passage(); // 20 s
    b.avancer(3_000);
    b.envoyer(E::Pause); // 23 s d'écoute
    b.tic_apres(3_600_000); // une heure de pause ne compte pas
    b.envoyer(E::Reprendre);
    assert!(b.tic_apres(1_999).is_empty());
    assert!(!b.vue().pause_conseillee);
    assert_eq!(b.tic_apres(1), vec![Effet::ConseillerPause]);
    assert!(b.vue().pause_conseillee);
    for _ in 0..4 {
        if b.etat() != EtatTest::Termine {
            b.ecouter_un_duel();
            b.repondre(Reponse::Pareil);
        }
    }
    assert_eq!(b.etat(), EtatTest::Termine);
    assert_eq!(b.compte(&Effet::ConseillerPause), 1);
}

#[test]
fn le_temps_passe_devant_la_question_ou_pc_endormi_ne_compte_pas_comme_ecoute() {
    let mut p = parametres(Mode::DuelsEnJeu { precis: false }, vec![controle(); 2]);
    p.reglages = p.reglages.avec_pause_conseillee_apres_ms(65_000);
    let mut b = Banc::avec(p);
    b.demarrer();
    b.ecouter_un_duel(); // 60 s
    b.tic_apres(3_600_000); // devant la question
    b.repondre(Reponse::Pareil);
    b.tic_apres(3_600_000); // PC endormi : le passage recommence, rien n'est compté
    assert_eq!(b.compte(&Effet::ConseillerPause), 0);
    assert_eq!(b.tic_apres(5_000), vec![Effet::ConseillerPause]);
}

#[test]
fn par_defaut_la_pause_est_conseillee_apres_40_minutes() {
    let mut b = Banc::nouveau(vec![controle(); 60]);
    b.demarrer();
    // 40 minutes = 240 passages de 10 s = 40 duels.
    for _ in 0..39 {
        b.ecouter_un_duel();
        b.repondre(Reponse::Pareil);
    }
    for _ in 0..5 {
        b.ecouter_un_passage();
    }
    assert_eq!(b.compte(&Effet::ConseillerPause), 0);
    let e = b.ecouter_un_passage();
    assert!(e.contains(&Effet::ConseillerPause), "{e:?}");
}

// ----- Vue -----

#[test]
fn la_vue_suit_le_duel_passage_par_passage() {
    let mut b = Banc::nouveau(vec![graves_plus(true), controle()]);
    let v = b.vue();
    assert_eq!((v.etat, v.duel, v.total), (EtatTest::Pret, 1, 2));
    assert_eq!(v.cote_actif, None);
    assert_eq!(v.ms_avant_bascule, None);

    b.demarrer();
    b.avancer(2_500);
    let v = b.vue();
    assert_eq!(v.cote_actif, Some(Cote::A));
    assert_eq!((v.passages_a, v.passages_b), (0, 0));
    assert_eq!(v.progression, 0.25);
    assert_eq!(v.ms_avant_bascule, Some(7_500));
    assert!(!v.est_verification);
    assert!(!v.pause_conseillee);

    b.tic_apres(7_500);
    b.ecouter_un_passage();
    b.ecouter_un_passage();
    // ORDRE = A B B A B A : trois passages finis, le quatrième (A) joue.
    let v = b.vue();
    assert_eq!(
        v.etat,
        EtatTest::EnEcoute {
            duel: 0,
            passage: 3
        }
    );
    assert_eq!(v.cote_actif, Some(Cote::A));
    assert_eq!((v.passages_a, v.passages_b), (1, 2));
    // Très en retard : la vue reste dans ses bornes.
    b.avancer(50_000);
    assert_eq!(b.vue().progression, 1.0);
    assert_eq!(b.vue().ms_avant_bascule, Some(0));

    b.envoyer(E::Pause);
    let v = b.vue();
    assert_eq!((v.passages_a, v.passages_b), (1, 2));
    assert_eq!(
        (v.cote_actif, v.progression, v.ms_avant_bascule),
        (None, 0.0, None)
    );
    b.envoyer(E::Reprendre);

    b.ecouter_un_duel();
    let v = b.vue();
    assert_eq!((v.passages_a, v.passages_b), (3, 3));
    assert_eq!(v.cote_actif, None);

    b.repondre(Reponse::B);
    let v = b.vue();
    assert_eq!((v.duel, v.total), (2, 2));
    assert_eq!((v.passages_a, v.passages_b), (0, 0));
    assert_eq!(
        v.historique,
        vec![DuelFini {
            numero: 1,
            sujet: Sujet::Zone {
                zone: Zone::Graves,
                sens: Sens::Plus
            },
            reponse: Some(Reponse::B),
            reference: Cote::A,
            preference: Some(Preference::Variante),
        }]
    );
}

#[test]
fn la_vue_ne_trahit_ni_le_sujet_ni_la_reference_du_duel_en_cours() {
    // Deux tests identiques, sauf le sujet et l'orientation du duel en cours.
    let mut x = Banc::nouveau(vec![controle(), graves_plus(true), controle()]);
    let mut y = Banc::nouveau(vec![controle(), duel(Sujet::Controle, false), controle()]);
    for b in [&mut x, &mut y] {
        b.demarrer();
        b.repondre(Reponse::A);
    }
    let scenario = [
        E::Tic,
        E::Pause,
        E::Reprendre,
        E::Tic,
        E::Tic,
        E::Tic,
        E::Tic,
        E::Tic,
        E::Tic,
        E::Reecouter {
            cote: Cote::B,
            appuye: true,
        },
        E::Reecouter {
            cote: Cote::B,
            appuye: false,
        },
        E::PlusTard,
    ];
    for evt in scenario {
        x.avancer(INTERVALLE);
        y.avancer(INTERVALLE);
        x.envoyer(evt.clone());
        y.envoyer(evt.clone());
        assert_eq!(x.vue(), y.vue(), "après {evt:?}");
        assert_eq!(x.vue().historique.len(), 1);
    }
    // Une fois le duel clos, l'historique dit ce qu'il était.
    for b in [&mut x, &mut y] {
        b.envoyer(E::Reprendre);
        b.repondre(Reponse::A);
    }
    assert_ne!(x.vue().historique[1], y.vue().historique[1]);
    assert_eq!(y.vue().historique[1].reference, Cote::B);
}
