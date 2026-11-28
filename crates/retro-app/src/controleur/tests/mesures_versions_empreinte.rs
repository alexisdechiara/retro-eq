//! Tickets 26, 27 et 28 : l'écart de volume mesuré sur l'écran Résultat, la comparaison à
//! l'aveugle de deux versions d'un profil, l'appli au premier plan et l'Empreinte.

use super::*;
use retro_core::Sens;

/// Joue tout un test comme `jouer`, mais en signalant chaque seconde le volume mesuré :
/// `volume(sujet, variante_joue)` donne le volume (dB) du côté en train de jouer.
fn jouer_en_mesurant(
    c: &mut Controleur,
    banc: &Banc,
    depart: u64,
    choisir: impl Fn(Sujet) -> Option<bool>,
    volume: impl Fn(Sujet, bool) -> f64,
) -> u64 {
    let id = identifiant(depart);
    let mut t = depart;
    let mut garde = 0;
    while c.test_en_cours() {
        garde += 1;
        assert!(garde < 100, "le test ne se termine pas");
        let numero = c.vue_duels(t).duel as usize;
        let duel = banc.duels(&id)[numero - 1];
        for _ in 0..6 {
            for _ in 0..9 {
                t += 1_000;
                let cote = match c.vue_duels(t).cote_actif {
                    1 => Cote::A,
                    _ => Cote::B,
                };
                let variante_joue = cote != duel.cote_reference();
                c.signaler_volume_mesure(volume(duel.sujet, variante_joue), t);
                c.tic(t);
            }
            t += 1_000;
            c.tic(t);
        }
        assert_eq!(c.vue_duels(t).etat, EtatTestVue::AttenteReponse);
        let reponse = match choisir(duel.sujet) {
            Some(variante) => reponse_pour(&duel, variante),
            None => Reponse::Pareil,
        };
        t += 500;
        c.repondre(reponse, t);
    }
    t
}

/// La variante « plus de graves » sonne 3 dB plus fort que la référence ; ailleurs, même volume.
fn graves_plus_forts(sujet: Sujet, variante_joue: bool) -> f64 {
    match sujet {
        Sujet::Zone {
            zone: Zone::Graves,
            sens: Sens::Plus,
        } if variante_joue => -17.0,
        _ => -20.0,
    }
}

// ----- Ticket 26 : écart de volume mesuré -----

#[test]
fn l_ecart_de_volume_mesure_est_journalise_et_signale_sur_le_resultat() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    assert!(c.demarrer_test(T0));
    jouer_en_mesurant(&mut c, &banc, T0, plus_de_graves, graves_plus_forts);

    let ecarts: Vec<f64> = banc
        .journal(&identifiant(T0))
        .entrees
        .iter()
        .filter_map(|e| match e {
            Entree::EcartVolumeMesure { ecart_db, .. } => Some(*ecart_db),
            _ => None,
        })
        .collect();
    assert_eq!(ecarts.len(), 13, "un écart par duel répondu");

    let resultat = c.vue_resultat().unwrap();
    assert!(
        resultat.volume.contains("au plus 3 dB"),
        "{}",
        resultat.volume
    );
    assert!(!resultat.ecarts_volume.is_empty());
    for ligne in &resultat.ecarts_volume {
        assert!(ligne.titre.contains("graves en plus"), "{}", ligne.titre);
        assert_eq!(ligne.ecart, "+3 dB");
        assert!(ligne.detail.contains("plus fort"), "{}", ligne.detail);
    }

    // Relu depuis le journal après un redémarrage : le même écran.
    drop(c);
    let c = banc.ouvrir(T0 + 3_600_000);
    assert_eq!(c.vue_resultat(), Some(resultat));
}

#[test]
fn sans_son_capte_le_resultat_dit_que_le_volume_n_a_pas_ete_mesure() {
    // C'est aussi le cas des journaux écrits avant le ticket 26 : aucun écart dedans.
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer(&mut c, &banc, T0, plus_de_graves);
    let resultat = c.vue_resultat().unwrap();
    assert!(
        resultat.volume.contains("pas mesuré"),
        "{}",
        resultat.volume
    );
    assert!(resultat.ecarts_volume.is_empty());
}

#[test]
fn des_volumes_egaux_ne_signalent_aucun_duel() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer_en_mesurant(&mut c, &banc, T0, plus_de_graves, |_, _| -20.0);
    let resultat = c.vue_resultat().unwrap();
    assert!(
        resultat.volume.contains("Aucun ne dépasse"),
        "{}",
        resultat.volume
    );
    assert!(resultat.ecarts_volume.is_empty());
}

// ----- Ticket 27 : deux versions d'un même profil à l'aveugle -----

/// Un profil avec deux versions : la 1 à plat, la 2 (active) avec 6 dB à 62 Hz.
fn profil_a_deux_versions(c: &mut Controleur) {
    c.regler_bande(1, 6.0);
    c.enregistrer_egaliseur(T0 - 5);
    c.prendre_annonce();
    // L'intervalle vient des choix de l'écran « Nouveau test » : 10 s ici.
    c.choisir_test(choix("duels-en-jeu"));
}

#[test]
fn deux_versions_d_un_profil_se_comparent_a_l_aveugle_par_le_test_profil_contre_profil() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    profil_a_deux_versions(&mut c);
    assert!(c.comparer_version(1, T0));

    // Le même mécanisme que « profil contre profil » : l'autre côté est la version 1,
    // et qui est A est tiré au hasard à chaque duel.
    let journal = banc.journal(&identifiant(T0));
    let Some(Entree::TestDemarre {
        mode: Mode::ProfilContreProfil { autre, .. },
        version_reference,
        plan,
        ..
    }) = journal.entrees.first()
    else {
        panic!("pas un test profil contre profil");
    };
    assert_eq!(*version_reference, 2);
    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(autre, &profil.version(1).unwrap().courbe);
    let face_a_l_autre: Vec<_> = plan
        .iter()
        .filter(|d| d.sujet == Sujet::AutreProfil)
        .collect();
    assert!(!face_a_l_autre.is_empty());
    assert!(face_a_l_autre.iter().any(|d| d.reference_est_a));
    assert!(face_a_l_autre.iter().any(|d| !d.reference_est_a));

    jouer(&mut c, &banc, T0, |sujet| match sujet {
        Sujet::AutreProfil => Some(true),
        _ => None,
    });
    let resultat = c.vue_resultat().unwrap();
    assert_eq!(resultat.titre, "Mon profil : version 2 contre version 1");
    assert!(resultat.resume.contains("version 1"), "{}", resultat.resume);
    assert!(resultat.recommandee);
    assert!(!resultat.active);
    // Aucune version n'est ajoutée par une comparaison.
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .versions()
            .len(),
        2
    );

    c.activer_resultat();
    assert!(c.vue_resultat().unwrap().active);
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .numero_actif(),
        1
    );
    assert!(banc.derniere_courbe().filtres.is_empty());
    c.revenir_reference();
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .numero_actif(),
        2
    );

    // Relu au redémarrage : on retrouve la comparaison de versions.
    let avant = c.vue_resultat().unwrap();
    drop(c);
    let c = banc.ouvrir(T0 + 3_600_000);
    assert_eq!(c.vue_resultat(), Some(avant));
}

#[test]
fn on_ne_compare_pas_la_version_active_a_elle_meme_ni_une_version_inconnue() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    profil_a_deux_versions(&mut c);
    for numero in [2, 9] {
        assert!(!c.comparer_version(numero, T0));
        assert!(c.prendre_annonce().unwrap().erreur);
    }
    // Le choix de l'écran « Nouveau test » n'en est pas changé.
    c.choisir_test(choix("duels-en-jeu"));
    assert!(c.demarrer_test(T0));
    assert!(banc
        .duels(&identifiant(T0))
        .iter()
        .all(|d| d.sujet != Sujet::AutreProfil));
}

// ----- Ticket 28 : appli au premier plan et Empreinte -----

fn premiere_appli_notee(banc: &Banc, test_id: &str) -> Option<String> {
    banc.journal(test_id).entrees.iter().find_map(|e| match e {
        Entree::PassageDemarre {
            appli_premier_plan, ..
        } => Some(appli_premier_plan.clone()),
        _ => None,
    })?
}

#[test]
fn l_appli_au_premier_plan_est_notee_des_le_premier_passage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    // Relevée avant le test (le pont la regarde chaque seconde, test ou pas).
    c.signaler_premier_plan(Some("Valorant".into()), T0 - 1_000);
    c.choisir_test(choix("duels-en-jeu"));
    assert!(c.demarrer_test(T0));
    assert_eq!(
        premiere_appli_notee(&banc, &identifiant(T0)).as_deref(),
        Some("Valorant")
    );
}

#[test]
fn l_empreinte_regroupe_les_reponses_de_tous_les_tests_du_profil_par_appli() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    assert!(c.vue_empreinte("mon-profil").is_empty());

    c.signaler_premier_plan(Some("Valorant".into()), T0 - 1_000);
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let fin = jouer(&mut c, &banc, T0, plus_de_graves);

    let t1 = fin + 60_000;
    c.signaler_premier_plan(Some("Discord".into()), t1 - 1_000);
    c.demarrer_test(t1);
    jouer(&mut c, &banc, t1, |_| None);

    let empreinte = c.vue_empreinte("mon-profil");
    assert_eq!(empreinte.len(), 2, "{empreinte:?}");
    let valorant = empreinte.iter().find(|a| a.nom == "Valorant").unwrap();
    assert!(valorant.duels >= 2);
    assert!(
        valorant.tendances.contains("Plus de graves"),
        "{}",
        valorant.tendances
    );
    let discord = empreinte.iter().find(|a| a.nom == "Discord").unwrap();
    assert_eq!(discord.confiance, 0);
    assert!(discord.tendances.contains("Rien ne se dégage"));

    // Un autre profil n'a pas d'Empreinte : ses tests n'ont pas eu lieu.
    let autre = c.creer_depuis_preset("À plat", t1 + 10).unwrap();
    assert!(c.vue_empreinte(&autre).is_empty());
    // Et l'Empreinte survit au redémarrage : elle est relue dans les journaux.
    drop(c);
    let c = banc.ouvrir(t1 + 3_600_000);
    assert_eq!(c.vue_empreinte("mon-profil"), empreinte);
}
