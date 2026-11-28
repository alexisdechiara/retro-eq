//! Ce qui se passe au début d'un test : la réduction de bruit demandée (ticket 24), le duel
//! d'essai du premier lancement (ticket 25), le conseil de pause (ticket 29) et les touches
//! de réponse prises par une autre appli (ticket 30).
//!
//! Module enfant de `tests` : `super::*` donne accès au banc d'essai et aux outils de là-bas.

use super::*;

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
