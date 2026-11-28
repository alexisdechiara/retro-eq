//! Ce qui se passe au début d'un test : la réduction de bruit demandée (ticket 24), le duel
//! d'essai du premier lancement (ticket 25), le conseil de pause (ticket 29) et les touches
//! de réponse prises par une autre appli (ticket 30).
//!
//! Module enfant de `tests` : `super::*` donne accès au banc d'essai et aux outils de là-bas.

use super::*;
use retro_core::IssueEssai;

// ----- Réduction de bruit (ticket 24) -----

#[test]
fn sans_choix_l_ecran_nouveau_test_ne_suppose_rien() {
    let banc = Banc::nouveau();
    let c = banc.ouvrir_autorise();
    assert_eq!(c.vue_nouveau_test().reduction_bruit, None);
}

#[test]
fn la_reduction_de_bruit_choisie_est_retenue_par_le_profil_au_demarrage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_reduction_bruit(true);
    assert_eq!(c.vue_nouveau_test().reduction_bruit, Some(true));
    // Rien n'est écrit tant que le test n'a pas démarré.
    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(profil.reduction_bruit(), None);

    c.choisir_test(choix("duels-en-jeu"));
    assert!(c.demarrer_test(T0));

    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(profil.reduction_bruit(), Some(true));
    // Le profil ne gagne pas de version pour autant : ce n'est pas un réglage du son.
    assert_eq!(profil.versions().len(), 1);
}

#[test]
fn l_ecran_nouveau_test_reprend_la_valeur_retenue_par_le_profil() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_reduction_bruit(false);
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.arreter_test(T0 + 1_000);
    drop(c);

    let c = banc.ouvrir(T0 + 60_000);
    assert_eq!(c.vue_nouveau_test().reduction_bruit, Some(false));
}

#[test]
fn changer_de_profil_reprend_la_reduction_de_bruit_de_ce_profil() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let autre = c.creer_depuis_preset("À plat", T0 - 10).unwrap();
    c.choisir_reduction_bruit(true);
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.arreter_test(T0 + 1_000);

    // Un choix fait pour « Mon profil » ne doit pas être écrit dans l'autre profil.
    c.choisir_reduction_bruit(false);
    c.activer_profil(&autre);
    assert_eq!(c.vue_nouveau_test().reduction_bruit, None);
    c.demarrer_test(T0 + 2_000);
    c.arreter_test(T0 + 3_000);
    assert_eq!(
        banc.depot()
            .charger_profil(&autre)
            .unwrap()
            .reduction_bruit(),
        None
    );
    // De retour sur « Mon profil », le choix pas encore enregistré est toujours là.
    c.activer_profil("mon-profil");
    assert_eq!(c.vue_nouveau_test().reduction_bruit, Some(false));
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .reduction_bruit(),
        Some(true)
    );
}

// ----- Touches de réponse prises par une autre appli (ticket 30) -----

#[test]
fn hors_test_une_touche_prise_ne_fait_rien_dire() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.signaler_touches_mortes(vec!["F8".to_string()]);
    assert!(c.prendre_annonce().is_none());
    assert_eq!(c.vue_duels(T0).touches_mortes, "");
}

#[test]
fn une_touche_de_reponse_prise_est_annoncee_au_demarrage_du_test() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    // L'autotest l'avait déjà vue avant le test : le test qui commence le redit quand même.
    c.signaler_touches_mortes(vec!["F8".to_string()]);
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.prendre_annonce();
    c.signaler_touches_mortes(vec!["F8".to_string()]);

    let annonce = c.prendre_annonce().expect("le conflit est annoncé");
    assert!(annonce.erreur);
    assert!(annonce.texte.contains("F8"), "{}", annonce.texte);
    assert_eq!(c.vue_duels(T0 + 1_000).touches_mortes, "F8");

    // Une seule fois par test : le pont le redit à chaque image.
    c.signaler_touches_mortes(vec!["F8".to_string()]);
    assert!(c.prendre_annonce().is_none());
    // Une touche de plus : on le redit.
    c.signaler_touches_mortes(vec!["F8".to_string(), "F10".to_string()]);
    assert!(c.prendre_annonce().unwrap().texte.contains("F10"));
    assert_eq!(c.vue_duels(T0 + 1_000).touches_mortes, "F8, F10");
    // Les touches libérées, l'alerte disparaît.
    c.signaler_touches_mortes(Vec::new());
    assert!(c.prendre_annonce().is_none());
    assert_eq!(c.vue_duels(T0 + 1_000).touches_mortes, "");
}

#[test]
fn chaque_nouveau_test_redit_le_conflit() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.signaler_touches_mortes(vec!["F6".to_string()]);
    assert!(c.prendre_annonce().is_some());
    c.arreter_test(T0 + 1_000);
    c.prendre_annonce();
    assert_eq!(c.vue_duels(T0 + 1_000).touches_mortes, "");

    c.demarrer_test(T0 + 2_000);
    c.prendre_annonce();
    c.signaler_touches_mortes(vec!["F6".to_string()]);
    assert!(c.prendre_annonce().unwrap().texte.contains("F6"));
}

// ----- Conseil de pause (ticket 29) -----

#[test]
fn apres_40_minutes_d_ecoute_la_vue_du_test_conseille_une_pause() {
    // La même vue remplit l'écran Duels et la mini-fenêtre (`pousser_duels`, pont.rs).
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(ChoixTest {
        intervalle_s: 120,
        ..choix("duels-en-jeu")
    });
    c.demarrer_test(T0);
    let mut t = T0;
    while t < T0 + 38 * 60_000 {
        t += 120_000;
        c.tic(t);
        if c.vue_duels(t).etat == EtatTestVue::AttenteReponse {
            c.repondre(Reponse::Pareil, t);
        }
        assert!(
            !c.vue_duels(t).pause_conseillee,
            "trop tôt, à {} s",
            (t - T0) / 1000
        );
    }
    for _ in 0..2 {
        t += 120_000;
        c.tic(t);
    }
    assert!(c.vue_duels(t).pause_conseillee);
}

// ----- Duel d'essai (ticket 25) -----

/// L'essai a-t-il laissé une trace sur le disque comme un vrai test ? Il ne le doit pas :
/// un journal serait repris au démarrage suivant et lu par le plan des vrais tests.
fn journaux(banc: &Banc) -> usize {
    std::fs::read_dir(banc.dossier.path().join("donnees").join("tests"))
        .map(|d| d.count())
        .unwrap_or(0)
}

/// Lance l'essai et écoute ses 6 passages : la question est posée. Rend l'heure.
fn essai_jusqu_a_la_question(c: &mut Controleur, depart: u64) -> u64 {
    assert!(c.demarrer_essai(depart));
    let mut t = depart;
    for _ in 0..6 {
        t += PASSAGE;
        c.tic(t);
    }
    assert_eq!(c.vue_duels(t).etat, EtatTestVue::AttenteReponse);
    t
}

/// Le côté qui joue la version aux graves. Les tests trichent comme l'oreille le ferait :
/// on réécoute A (F6 maintenue) et on regarde si la courbe envoyée porte un filtre (la
/// référence du profil par défaut est à plat).
fn cote_des_graves(c: &mut Controleur, banc: &Banc, t: u64) -> Cote {
    c.reecouter(Cote::A, true, t);
    c.reecouter(Cote::A, false, t);
    if banc.derniere_courbe().filtres.is_empty() {
        Cote::B
    } else {
        Cote::A
    }
}

fn reponse_du_cote(cote: Cote) -> Reponse {
    match cote {
        Cote::A => Reponse::A,
        Cote::B => Reponse::B,
    }
}

#[test]
fn le_duel_d_essai_se_joue_avec_les_touches_sans_version_ni_resultat() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    assert!(c.demarrer_essai(T0));
    assert!(c.essai_en_cours());
    assert!(c.test_en_cours());
    assert!(c.touches_de_test_actives());
    let vue = c.vue_duels(T0);
    assert!(vue.essai);
    assert_eq!((vue.duel, vue.total), (1, 1));
    assert_eq!(c.vue_essai().issue, None);

    let mut t = T0;
    for _ in 0..6 {
        t += PASSAGE;
        c.tic(t);
    }
    // La variante aux graves a bien joué.
    assert!(banc
        .moteur
        .courbes_appliquees()
        .iter()
        .any(|courbe| !courbe.filtres.is_empty()));
    let graves = cote_des_graves(&mut c, &banc, t);
    c.repondre(reponse_du_cote(graves), t + 500);

    assert!(!c.test_en_cours());
    assert!(!c.essai_en_cours());
    assert!(!c.touches_de_test_actives());
    assert!(
        banc.derniere_courbe().filtres.is_empty(),
        "référence remise"
    );
    let vue = c.vue_essai();
    assert_eq!(vue.issue, Some(IssueEssai::Reconnue));
    assert!(
        vue.detail.contains(&format!("{graves:?}")),
        "{}",
        vue.detail
    );
    // Ni version, ni résultat, ni journal.
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .versions()
            .len(),
        1
    );
    assert!(c.vue_resultat().is_none());
    assert_eq!(journaux(&banc), 0);
    assert_eq!(c.vue_duels(t + 500), VueDuels::default());
}

#[test]
fn un_pareil_au_duel_d_essai_renvoie_vers_le_diagnostic() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let t = essai_jusqu_a_la_question(&mut c, T0);
    c.repondre(Reponse::Pareil, t + 500);
    let vue = c.vue_essai();
    assert_eq!(vue.issue, Some(IssueEssai::Pareil));
    assert!(vue.diagnostic);
}

#[test]
fn se_tromper_de_cote_dit_quand_meme_que_le_son_change() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let t = essai_jusqu_a_la_question(&mut c, T0);
    let graves = cote_des_graves(&mut c, &banc, t);
    c.repondre(reponse_du_cote(graves.autre()), t + 500);
    let vue = c.vue_essai();
    assert_eq!(vue.issue, Some(IssueEssai::Confondue));
    assert!(!vue.diagnostic);
}

#[test]
fn refaire_l_essai_efface_le_bilan_precedent() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let t = essai_jusqu_a_la_question(&mut c, T0);
    c.repondre(Reponse::Pareil, t + 500);
    assert!(c.demarrer_essai(t + 1_000));
    assert_eq!(c.vue_essai().issue, None);
    assert!(c.vue_essai().en_cours);
}

#[test]
fn une_pause_pendant_l_essai_remet_la_reference() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.demarrer_essai(T0);
    let mut t = T0;
    while banc.derniere_courbe().filtres.is_empty() {
        t += PASSAGE;
        c.tic(t);
    }
    c.mettre_en_pause(t + 500);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(c.vue_duels(t + 500).etat, EtatTestVue::Pause);
    assert!(!c.touches_de_test_actives());
    c.reprendre(t + 1_000);
    assert_eq!(c.vue_duels(t + 1_000).etat, EtatTestVue::Ecoute);
}

#[test]
fn arreter_l_essai_remet_la_reference() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let t = essai_jusqu_a_la_question(&mut c, T0);
    c.reecouter(Cote::A, true, t);
    c.reecouter(Cote::B, true, t);
    c.arreter_test(t + 500);
    assert!(!c.essai_en_cours());
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(c.vue_essai().issue, None);
    assert!(c.prendre_annonce().unwrap().texte.contains("Essai arrêté"));
}

#[test]
fn quitter_pendant_l_essai_remet_la_reference_et_rien_n_est_repris() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let t = essai_jusqu_a_la_question(&mut c, T0);
    let graves = cote_des_graves(&mut c, &banc, t);
    c.reecouter(graves, true, t + 100);
    assert!(!banc.derniere_courbe().filtres.is_empty());

    c.avant_de_quitter(t + 500);
    assert!(banc.derniere_courbe().filtres.is_empty());
    drop(c);
    let mut c = banc.ouvrir(t + 60_000);
    assert!(!c.test_en_cours());
    assert!(c.prendre_annonce().is_none(), "aucun test à reprendre");
    assert!(banc.derniere_courbe().filtres.is_empty());
}

#[test]
fn apres_un_arret_brutal_pendant_l_essai_la_reference_revient() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let t = essai_jusqu_a_la_question(&mut c, T0);
    let graves = cote_des_graves(&mut c, &banc, t);
    c.reecouter(graves, true, t + 100);
    assert!(!banc.derniere_courbe().filtres.is_empty());
    // Plantage : rien n'est remis avant de fermer.
    drop(c);

    let c = banc.ouvrir(t + 60_000);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert!(!c.test_en_cours());
    assert_eq!(journaux(&banc), 0);
}

#[test]
fn l_essai_ne_touche_ni_au_dernier_resultat_ni_au_plan_des_vrais_tests() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let fin = jouer(&mut c, &banc, T0, plus_de_graves);
    let avant = c.vue_resultat().unwrap();
    let versions = c.vue_versions("mon-profil").len();

    let t = essai_jusqu_a_la_question(&mut c, fin + 1_000);
    c.repondre(Reponse::A, t + 500);

    assert_eq!(c.vue_resultat().unwrap(), avant);
    assert_eq!(c.vue_versions("mon-profil").len(), versions);
    assert_eq!(journaux(&banc), 1);
}

#[test]
fn ctrl_alt_entree_pendant_l_essai_ne_lance_pas_un_vrai_test() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.demarrer_essai(T0);
    c.mettre_en_pause(T0 + 1_000);
    assert!(c.demarrer_ou_reprendre(T0 + 2_000));
    assert!(c.essai_en_cours());
    assert_eq!(c.vue_duels(T0 + 2_000).etat, EtatTestVue::Ecoute);
    assert_eq!(journaux(&banc), 0);
}

#[test]
fn pas_d_essai_sans_accord_ni_pendant_un_vrai_test() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir(T0);
    assert!(!c.demarrer_essai(T0));
    assert!(c.prendre_annonce().unwrap().erreur);

    c.autoriser_moteur();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.prendre_annonce();
    assert!(!c.demarrer_essai(T0 + 1_000));
    assert!(c.prendre_annonce().unwrap().erreur);
    assert!(!c.essai_en_cours());
}
