//! Tests de clic pour de vrai : on envoie les mêmes événements souris que Windows, sur la vraie
//! fenêtre, avec le moteur de test de Slint (sans écran, sans X). Les autres tests appellent
//! les callbacks directement et ne voient donc pas un bouton qui avale le premier clic.

use i_slint_backend_testing::ElementHandle;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, SharedString, VecModel};
use std::cell::Cell;
use std::rc::Rc;

use crate::{
    Appli, AutoEq, Ecran, FenetrePrincipale, LigneAutoEq, LigneMode, LigneProfil, Profils, Test,
};

slint::slint! {
    #[edition = "1.19"]
    export { BancClics } from "ui/tests/banc_clics.slint";
}

/// Prépare le moteur de test (une fois par fil de test) et affiche la fenêtre.
fn banc() -> BancClics {
    i_slint_backend_testing::init_no_event_loop();
    let banc = BancClics::new().expect("le banc se crée");
    banc.show().expect("le banc s'affiche");
    banc
}

fn trouve(fenetre: &impl ComponentHandle, nom: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(fenetre, nom)
        .next()
        .unwrap_or_else(|| panic!("élément « {nom} » introuvable"))
}

fn centre(element: &ElementHandle) -> LogicalPosition {
    let p = element.absolute_position();
    let t = element.size();
    LogicalPosition::new(p.x + t.width / 2., p.y + t.height / 2.)
}

fn appuie(fenetre: &impl ComponentHandle, position: LogicalPosition) {
    fenetre
        .window()
        .dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
}

fn relache(fenetre: &impl ComponentHandle, position: LogicalPosition) {
    fenetre
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
}

/// Un clic, un seul : appui puis relâchement au centre de l'élément.
fn clique(fenetre: &impl ComponentHandle, nom: &str) {
    let position = centre(&trouve(fenetre, nom));
    appuie(fenetre, position);
    relache(fenetre, position);
}

#[test]
fn un_clic_sur_chaque_genre_de_bouton_agit_tout_de_suite() {
    let b = banc();
    clique(&b, "Principal");
    clique(&b, "Secondaire");
    clique(&b, "Choisi");
    clique(&b, "Lien");
    assert_eq!(b.get_n_principal(), 1);
    assert_eq!(b.get_n_secondaire(), 1);
    assert_eq!(b.get_n_choisi(), 1);
    assert_eq!(b.get_n_lien(), 1);
}

#[test]
fn un_bouton_inactif_ne_fait_rien() {
    let b = banc();
    clique(&b, "Inactif");
    clique(&b, "Inactif");
    assert_eq!(b.get_n_inactif(), 0);
}

#[test]
fn un_clic_sur_un_interrupteur_le_bascule_tout_de_suite() {
    let b = banc();
    clique(&b, "Interrupteur");
    assert_eq!(b.get_n_bascule(), 1);
    assert!(b.get_coche());
    clique(&b, "Interrupteur");
    assert_eq!(b.get_n_bascule(), 2);
    assert!(!b.get_coche());
}

fn touche(fenetre: &impl ComponentHandle, texte: &str) {
    fenetre
        .window()
        .dispatch_event(WindowEvent::KeyPressed { text: texte.into() });
    fenetre
        .window()
        .dispatch_event(WindowEvent::KeyReleased { text: texte.into() });
}

#[test]
fn entree_et_espace_activent_le_bouton_qui_a_le_focus() {
    let b = banc();
    // Tab donne le focus au premier bouton, sans souris.
    touche(&b, "\t");
    touche(&b, "\n");
    assert_eq!(b.get_n_principal(), 1);
    touche(&b, " ");
    assert_eq!(b.get_n_principal(), 2);
    // Un clic donne aussi le focus : Entrée agit ensuite sur le bouton cliqué.
    clique(&b, "Secondaire");
    assert_eq!(b.get_n_secondaire(), 1);
    touche(&b, "\n");
    assert_eq!(b.get_n_secondaire(), 2);
}

#[test]
fn tab_saute_le_bouton_inactif() {
    let b = banc();
    // Principal, Secondaire, Choisi, Lien, (Inactif sauté), Interrupteur.
    for _ in 0..5 {
        touche(&b, "\t");
    }
    touche(&b, "\n");
    assert_eq!(b.get_n_inactif(), 0);
    assert_eq!(b.get_n_bascule(), 1);
}

#[test]
fn le_fader_suit_la_souris_des_le_premier_appui() {
    let b = banc();
    let fader = trouve(&b, "Fader");
    let p = fader.absolute_position();
    let t = fader.size();
    let x = p.x + t.width / 2.;
    // Appui en haut de la fente : la valeur change tout de suite, sans relâcher.
    appuie(&b, LogicalPosition::new(x, p.y + 40.));
    assert!(
        b.get_n_regle() >= 1,
        "le premier appui doit régler le fader"
    );
    let en_haut = b.get_gain();
    assert!(en_haut > 6., "gain après appui en haut : {en_haut}");
    // Glisser vers le bas descend la valeur.
    fenetre_glisse(&b, LogicalPosition::new(x, p.y + t.height - 40.));
    assert!(
        b.get_gain() < -6.,
        "gain après glissement : {}",
        b.get_gain()
    );
    relache(&b, LogicalPosition::new(x, p.y + t.height - 40.));
}

fn fenetre_glisse(fenetre: &impl ComponentHandle, position: LogicalPosition) {
    fenetre
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
}

// ---- La vraie fenêtre principale ----

fn fenetre_principale() -> FenetrePrincipale {
    i_slint_backend_testing::init_no_event_loop();
    let f = FenetrePrincipale::new().expect("la fenêtre se crée");
    // Les libellés cherchés sont en français. Slint suit sinon la langue du système (une machine
    // anglaise, comme le runner Windows de la CI) : on impose la langue source, comme l'appli.
    // À faire après `new()`, qui enregistre les traductions embarquées. `banc()` n'en a pas
    // besoin : ses textes sont compilés sans traductions.
    slint::select_bundled_translation("").expect("le français (langue source) se sélectionne");
    // Certains tests agrandissent la fenêtre pour atteindre le bas d'une page.
    Appli::get(&f).set_taille_libre(true);
    f.window().set_size(slint::LogicalSize::new(1280., 800.));
    f.show().expect("la fenêtre s'affiche");
    f
}

#[test]
fn un_clic_sur_un_onglet_change_d_ecran() {
    let f = fenetre_principale();
    clique(&f, "Réglages");
    assert_eq!(Appli::get(&f).get_ecran(), Ecran::Reglages);
    clique(&f, "Mes profils");
    assert_eq!(Appli::get(&f).get_ecran(), Ecran::Profils);
    clique(&f, "Accueil");
    assert_eq!(Appli::get(&f).get_ecran(), Ecran::Accueil);
}

#[test]
fn les_boutons_de_la_fenetre_agissent_au_premier_clic() {
    let f = fenetre_principale();
    let reduit = Rc::new(Cell::new(0));
    let agrandi = Rc::new(Cell::new(0));
    let ferme = Rc::new(Cell::new(0));
    let (r, a, c) = (reduit.clone(), agrandi.clone(), ferme.clone());
    Appli::get(&f).on_reduire(move || r.set(r.get() + 1));
    Appli::get(&f).on_agrandir(move || a.set(a.get() + 1));
    Appli::get(&f).on_fermer(move || c.set(c.get() + 1));
    clique(&f, "Réduire");
    clique(&f, "Agrandir");
    clique(&f, "Fermer");
    assert_eq!((reduit.get(), agrandi.get(), ferme.get()), (1, 1, 1));
}

#[test]
fn la_barre_de_titre_deplace_des_l_appui_et_agrandit_au_double_clic() {
    let f = fenetre_principale();
    let deplace = Rc::new(Cell::new(0));
    let agrandi = Rc::new(Cell::new(0));
    let (d, a) = (deplace.clone(), agrandi.clone());
    Appli::get(&f).on_deplacer_fenetre(move || d.set(d.get() + 1));
    Appli::get(&f).on_agrandir(move || a.set(a.get() + 1));
    // Un coin libre de la barre de titre, entre les onglets et les boutons de fenêtre.
    let position = LogicalPosition::new(900., 30.);
    appuie(&f, position);
    relache(&f, position);
    assert_eq!(deplace.get(), 1, "un seul appui lance le déplacement");
    assert_eq!(agrandi.get(), 0);
    appuie(&f, position);
    relache(&f, position);
    assert_eq!(agrandi.get(), 1, "le double-clic agrandit");
}

#[test]
fn un_clic_sur_un_bouton_d_ecran_agit_au_premier_clic() {
    let f = fenetre_principale();
    Appli::get(&f).set_ecran(Ecran::Accueil);
    clique(&f, "Mes profils");
    // L'onglet et le bouton de l'accueil portent le même nom : l'un des deux a répondu.
    assert_eq!(Appli::get(&f).get_ecran(), Ecran::Profils);
}

#[test]
fn un_clic_sur_une_cassette_du_tester_la_choisit() {
    let f = fenetre_principale();
    let mode = |id: &str, nom: &str| LigneMode {
        id: id.into(),
        nom: nom.into(),
        groupe: "G".into(),
        disponible: true,
        ..Default::default()
    };
    // Fenêtre haute : les étagères sont sous le pli, et un clic hors fenêtre ne prouverait rien.
    f.window().set_size(slint::LogicalSize::new(1280., 2400.));
    let t = Test::get(&f);
    t.set_modes(ModelRc::new(VecModel::from(vec![
        mode("a", "Mode un"),
        mode("b", "Mode deux"),
    ])));
    t.set_mode_choisi("a".into());
    Appli::get(&f).set_ecran(Ecran::Tester);
    clique(&f, "Mode deux");
    assert_eq!(Test::get(&f).get_mode_choisi(), SharedString::from("b"));
}

#[test]
fn un_clic_sur_la_reduction_de_bruit_la_choisit_au_premier_clic() {
    let f = fenetre_principale();
    // Fenêtre haute : la carte « Réglages du test » est sous le pli.
    f.window().set_size(slint::LogicalSize::new(1280., 2400.));
    let choisis = Rc::new(std::cell::RefCell::new(Vec::<bool>::new()));
    let c = choisis.clone();
    Test::get(&f).on_choisir_reduction_bruit(move |v| c.borrow_mut().push(v));
    Appli::get(&f).set_ecran(Ecran::NouveauTest);
    clique(&f, "Coupée");
    clique(&f, "Activée");
    assert_eq!(*choisis.borrow(), vec![false, true]);
}

// ---- La mini-fenêtre (tickets 29 et 30) ----

fn mini_fenetre() -> crate::MiniFenetre {
    i_slint_backend_testing::init_no_event_loop();
    let m = crate::MiniFenetre::new().expect("la mini-fenêtre se crée");
    slint::select_bundled_translation("").expect("le français (langue source) se sélectionne");
    m.show().expect("la mini-fenêtre s'affiche");
    m
}

fn montre(fenetre: &impl ComponentHandle, texte: &str) -> bool {
    ElementHandle::find_by_accessible_label(fenetre, texte)
        .next()
        .is_some()
}

const CONSEIL_PAUSE: &str = "Plus de 40 min d’écoute : F9 pour faire une pause";
const TOUCHE_MORTE: &str = "Sans effet : F8 (une autre appli les garde)";

#[test]
fn la_mini_fenetre_conseille_la_pause_apres_40_minutes() {
    let m = mini_fenetre();
    let test = Test::get(&m);
    test.set_etat(crate::EtatTest::Ecoute);
    assert!(!montre(&m, CONSEIL_PAUSE));
    test.set_pause_conseillee(true);
    assert!(montre(&m, CONSEIL_PAUSE));
}

#[test]
fn dans_la_mini_fenetre_une_touche_morte_passe_avant_le_conseil_de_pause() {
    let m = mini_fenetre();
    let test = Test::get(&m);
    test.set_pause_conseillee(true);
    test.set_touches_mortes("F8".into());
    assert!(montre(&m, TOUCHE_MORTE));
    assert!(!montre(&m, CONSEIL_PAUSE));
}

#[test]
fn un_clic_sur_une_cassette_de_profil_la_choisit() {
    let f = fenetre_principale();
    let profil = |id: &str, nom: &str| LigneProfil {
        id: id.into(),
        nom: nom.into(),
        versions: 1,
        ..Default::default()
    };
    let choisis = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let c = choisis.clone();
    Profils::get(&f).on_choisir(move |id| c.borrow_mut().push(id.to_string()));
    Profils::get(&f).set_liste(ModelRc::new(VecModel::from(vec![
        profil("p1", "Un"),
        profil("p2", "Deux"),
    ])));
    Appli::get(&f).set_ecran(Ecran::Profils);
    clique(&f, "Deux");
    assert_eq!(*choisis.borrow(), vec!["p2".to_string()]);
}

#[test]
fn un_clic_sur_une_reponse_de_duel_repond_au_premier_clic() {
    let f = fenetre_principale();
    let reponses = Rc::new(std::cell::RefCell::new(Vec::<i32>::new()));
    let r = reponses.clone();
    Test::get(&f).on_repondre(move |v| r.borrow_mut().push(v));
    Test::get(&f).set_etat(crate::EtatTest::AttenteReponse);
    Appli::get(&f).set_ecran(Ecran::Duels);
    clique(&f, "Version B");
    assert_eq!(*reponses.borrow(), vec![2]);
}

#[test]
fn le_bouton_chercher_mon_casque_ouvre_le_panneau_au_premier_clic() {
    let f = fenetre_principale();
    Appli::get(&f).set_ecran(Ecran::Profils);
    assert!(!AutoEq::get(&f).get_ouvert());
    clique(&f, "Chercher mon casque (AutoEQ)");
    assert!(
        AutoEq::get(&f).get_ouvert(),
        "un seul clic ouvre le panneau"
    );
}

#[test]
fn un_clic_sur_un_resultat_autoeq_le_choisit_au_premier_clic() {
    let f = fenetre_principale();
    let ligne = |nom: &str| LigneAutoEq {
        nom: nom.into(),
        source: "Rtings".into(),
        ..Default::default()
    };
    let choisis = Rc::new(std::cell::RefCell::new(Vec::<i32>::new()));
    let c = choisis.clone();
    let autoeq = AutoEq::get(&f);
    autoeq.on_choisir(move |i| c.borrow_mut().push(i));
    autoeq.set_etat(1);
    autoeq.set_ouvert(true);
    autoeq.set_resultats(ModelRc::new(VecModel::from(vec![
        ligne("Casque un"),
        ligne("Casque deux"),
    ])));
    Appli::get(&f).set_ecran(Ecran::Profils);
    // Chaque ligne a son bouton « Créer un profil » : on clique celui de la dernière (la seconde).
    let bouton = ElementHandle::find_by_accessible_label(&f, "Créer un profil")
        .last()
        .expect("le bouton de la dernière ligne existe");
    let position = centre(&bouton);
    appuie(&f, position);
    relache(&f, position);
    assert_eq!(*choisis.borrow(), vec![1], "un seul clic choisit la ligne");
}
