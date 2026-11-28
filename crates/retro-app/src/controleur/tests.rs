//! Le contrôleur de bout en bout, sans fenêtre : moteur simulé, dossier temporaire, heure
//! passée à la main.

#![allow(clippy::unwrap_used)]

use super::*;
use retro_core::{DuelPrevu, LectureJournal, Sujet};
use retro_dsp::TypeFiltre;
use retro_engine::{Appel, MoteurSimule};
use tempfile::TempDir;

/// 24 novembre 2025, 20:40 UTC.
const T0: u64 = 1_764_016_800_000;
/// L'intervalle le plus court permis : 10 s par passage.
const PASSAGE: u64 = 10_000;

/// Tout ce qu'il faut pour ouvrir un contrôleur, et le rouvrir sur le même dossier.
struct Banc {
    dossier: TempDir,
    moteur: MoteurSimule,
}

impl Banc {
    fn nouveau() -> Banc {
        Banc {
            dossier: TempDir::new().unwrap(),
            moteur: MoteurSimule::nouveau(),
        }
    }

    fn options(&self) -> Options {
        Options {
            dossier_export: self.dossier.path().join("exports"),
            moteur_absent: false,
            decalage_minutes: 60,
            graine: Some(7),
            langue_systeme: Langue::Fr,
            langue_imposee: None,
        }
    }

    /// Ouvre un contrôleur avec ces options plutôt que les options habituelles.
    fn ouvrir_avec(&self, options: Options, t: u64) -> Controleur {
        Controleur::ouvrir(
            self.dossier.path().join("donnees"),
            Box::new(self.moteur.clone()),
            options,
            t,
        )
        .unwrap()
    }

    fn ouvrir(&self, t: u64) -> Controleur {
        Controleur::ouvrir(
            self.dossier.path().join("donnees"),
            Box::new(self.moteur.clone()),
            self.options(),
            t,
        )
        .unwrap()
    }

    /// Un contrôleur dont l'utilisateur a déjà donné son accord.
    fn ouvrir_autorise(&self) -> Controleur {
        let mut c = self.ouvrir(T0);
        c.autoriser_moteur();
        c.prendre_annonce();
        c
    }

    fn depot(&self) -> Depot {
        Depot::ouvrir(self.dossier.path().join("donnees")).unwrap()
    }

    fn derniere_courbe(&self) -> Courbe {
        self.moteur.courbes_appliquees().last().cloned().unwrap()
    }

    fn journal(&self, test_id: &str) -> LectureJournal {
        self.depot().lire_journal(test_id).unwrap()
    }

    /// Les duels du test tels que le journal les connaît : le plan, puis le duel de
    /// vérification s'il a été ajouté. Les tests s'en servent pour « tricher » : savoir quel
    /// côté est la variante, ce que l'interface ne peut pas savoir.
    fn duels(&self, test_id: &str) -> Vec<DuelPrevu> {
        let mut duels = Vec::new();
        for entree in self.journal(test_id).entrees {
            match entree {
                Entree::TestDemarre { plan, .. } => duels = plan,
                Entree::DuelVerificationAjoute { duel, .. } => duels.push(duel),
                // Un duel rejoué après une pause a un nouveau tirage, qui remplace l'ancien.
                Entree::DuelRetire { duel, prevu, .. } => duels[duel as usize] = prevu,
                _ => {}
            }
        }
        duels
    }
}

fn identifiant(t: u64) -> String {
    format!("t{t}")
}

fn choix(mode: &str) -> ChoixTest {
    ChoixTest {
        mode: mode.to_string(),
        precis: false,
        intervalle_s: 10,
        zone: 1,
        autre_profil: String::new(),
    }
}

/// La réponse qui dit « je préfère la variante » (ou la référence) dans ce duel.
fn reponse_pour(duel: &DuelPrevu, variante: bool) -> Reponse {
    if duel.reference_est_a == variante {
        Reponse::B
    } else {
        Reponse::A
    }
}

/// Joue tout un test en écoutant les 6 passages de chaque duel. `choisir` reçoit le sujet
/// du duel et dit quoi répondre : `Some(true)` la variante, `Some(false)` la référence,
/// `None` « Pareil ». Rend l'heure de fin.
fn jouer(
    c: &mut Controleur,
    banc: &Banc,
    depart: u64,
    choisir: impl Fn(Sujet) -> Option<bool>,
) -> u64 {
    let id = identifiant(depart);
    let mut t = depart;
    let mut garde = 0;
    while c.test_en_cours() {
        garde += 1;
        assert!(garde < 100, "le test ne se termine pas");
        let numero = c.vue_duels(t).duel as usize;
        for _ in 0..6 {
            t += PASSAGE;
            c.tic(t);
        }
        assert_eq!(c.vue_duels(t).etat, EtatTestVue::AttenteReponse);
        let duel = banc.duels(&id)[numero - 1];
        let reponse = match choisir(duel.sujet) {
            Some(variante) => reponse_pour(&duel, variante),
            None => Reponse::Pareil,
        };
        t += 500;
        c.repondre(reponse, t);
    }
    t
}

/// Préfère plus de graves, et la nouvelle version à la vérification ; « Pareil » ailleurs.
fn plus_de_graves(sujet: Sujet) -> Option<bool> {
    match sujet {
        Sujet::Zone {
            zone: Zone::Graves,
            sens: retro_core::Sens::Plus,
        } => Some(true),
        Sujet::Zone {
            zone: Zone::Graves, ..
        } => Some(false),
        Sujet::Verification => Some(true),
        _ => None,
    }
}

// ----- Accord avant de toucher au son -----

#[test]
fn au_premier_lancement_rien_n_est_ecrit_dans_le_moteur() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir(T0);
    assert!(!c.vue_appli().moteur_autorise);
    assert_eq!(c.vue_appli().profil_actif, "Mon profil");

    // Même en réglant l'égaliseur, tant que l'accord n'est pas donné.
    c.regler_bande(5, 3.0);
    c.enregistrer_egaliseur(T0 + 1);
    assert!(banc.moteur.appels().is_empty());

    c.autoriser_moteur();
    assert_eq!(banc.moteur.appels().len(), 1);
    assert_eq!(banc.derniere_courbe(), c.vue_egaliseur().courbe);
    assert!(c.vue_appli().moteur_autorise);
}

#[test]
fn l_accord_survit_au_redemarrage_et_la_courbe_est_remise() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(2, -4.0);
    c.enregistrer_egaliseur(T0 + 1);
    let attendue = c.vue_egaliseur().courbe;
    drop(c);

    let avant = banc.moteur.appels().len();
    let c = banc.ouvrir(T0 + 10);
    assert!(c.vue_appli().moteur_autorise);
    assert_eq!(banc.moteur.appels().len(), avant + 1);
    assert_eq!(banc.derniere_courbe(), attendue);
}

#[test]
fn rendre_le_son_d_origine_retire_le_moteur_et_l_accord() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.rendre_son_origine(T0 + 1);
    assert_eq!(banc.moteur.appels().last(), Some(&Appel::Retirer));
    assert!(!c.vue_appli().moteur_autorise);

    let avant = banc.moteur.appels().len();
    c.regler_bande(0, 2.0);
    assert_eq!(banc.moteur.appels().len(), avant);
    drop(c);
    let c = banc.ouvrir(T0 + 10);
    assert!(!c.vue_appli().moteur_autorise);
    assert_eq!(banc.moteur.appels().len(), avant);
}

#[test]
fn sans_moteur_installe_rien_n_est_demande_et_on_le_dit() {
    let banc = Banc::nouveau();
    let mut c = Controleur::ouvrir(
        banc.dossier.path().join("donnees"),
        Box::new(banc.moteur.clone()),
        Options {
            moteur_absent: true,
            ..banc.options()
        },
        T0,
    )
    .unwrap();
    c.autoriser_moteur();
    c.regler_bande(1, 3.0);
    assert!(banc.moteur.appels().is_empty());
    assert_eq!(c.vue_appli().etat_moteur, EtatMoteurVue::Absent);

    c.choisir_test(choix("duels-en-jeu"));
    assert!(!c.demarrer_test(T0 + 5));
    assert!(c.prendre_annonce().unwrap().erreur);
}

// ----- Égaliseur -----

#[test]
fn un_fader_change_la_courbe_envoyee_au_moteur() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(5, 3.0);

    let courbe = banc.derniere_courbe();
    assert_eq!(courbe.filtres.len(), 1);
    assert_eq!(courbe.filtres[0].type_filtre, TypeFiltre::Cloche);
    assert_eq!(courbe.filtres[0].frequence_hz, 1000.0);
    assert_eq!(courbe.filtres[0].gain_db, 3.0);
    // Préampli automatique : le plus petit qui évite la saturation.
    assert!((courbe.preampli_db + 3.0).abs() < 0.05);

    let vue = c.vue_egaliseur();
    assert_eq!(vue.gains[5], 3.0);
    assert!(vue.modifie);
    assert!(vue.preampli_auto);
    // Le brouillon n'est pas ce qui sera remis au prochain démarrage.
    let sauve = banc.depot().charger_etat_moteur().unwrap().unwrap();
    assert!(sauve.courbe.filtres.is_empty());
}

#[test]
fn un_gain_hors_bornes_est_ramene_a_douze_db() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(0, 40.0);
    c.regler_bande(1, -40.0);
    c.regler_bande(12, 3.0);
    c.regler_bande(2, f64::NAN);
    let vue = c.vue_egaliseur();
    assert_eq!(vue.gains[0], 12.0);
    assert_eq!(vue.gains[1], -12.0);
    assert_eq!(vue.gains[2], 0.0);
}

#[test]
fn enregistrer_ajoute_une_version_sans_rien_effacer() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(5, 3.0);
    c.enregistrer_egaliseur(T0 + 60_000);

    assert!(!c.vue_egaliseur().modifie);
    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(profil.versions().len(), 2);
    assert_eq!(profil.numero_actif(), 2);
    assert_eq!(profil.versions()[1].origine, Origine::Manuel);
    assert_eq!(profil.versions()[1].creee_le, "2025-11-24T21:41:00+01:00");
    assert!(profil.versions()[0].courbe.filtres.is_empty());

    let sauve = banc.depot().charger_etat_moteur().unwrap().unwrap();
    assert_eq!(sauve.version, Some(2));
    assert_eq!(sauve.courbe, banc.derniere_courbe());

    let versions = c.vue_versions("mon-profil");
    assert_eq!(versions.len(), 2);
    assert!(versions[0].active && versions[0].numero == 2);
    assert_eq!(versions[0].date, "24/11 21:41");
    assert!(!c.prendre_annonce().unwrap().erreur);

    // Revenir à la version d'avant est toujours possible.
    c.activer_version("mon-profil", 1);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .versions()
            .len(),
        2
    );
}

#[test]
fn annuler_remet_la_version_active() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(5, 3.0);
    c.annuler_egaliseur();
    assert!(!c.vue_egaliseur().modifie);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(c.vue_egaliseur().gains, [0.0; 10]);
}

#[test]
fn le_preampli_manuel_part_de_la_valeur_automatique() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(5, 6.0);
    let auto = c.vue_egaliseur().preampli_db;

    // En automatique, le réglage manuel est ignoré.
    c.regler_preampli(-10.0);
    assert_eq!(c.vue_egaliseur().preampli_db, auto);

    c.basculer_preampli_auto();
    assert!(!c.vue_egaliseur().preampli_auto);
    assert_eq!(c.vue_egaliseur().preampli_db, auto);
    c.regler_preampli(-10.0);
    assert_eq!(banc.derniere_courbe().preampli_db, -10.0);
    c.regler_preampli(-99.0);
    assert_eq!(banc.derniere_courbe().preampli_db, -24.0);

    // Le choix du manuel est retrouvé après l'enregistrement.
    c.enregistrer_egaliseur(T0 + 1);
    assert!(!c.vue_egaliseur().preampli_auto);
    assert_eq!(c.vue_egaliseur().preampli_db, -24.0);
}

#[test]
fn les_filtres_supplementaires_d_un_import_survivent_aux_faders() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let fichier = banc.dossier.path().join("plateau.txt");
    std::fs::write(
        &fichier,
        "Preamp: -2 dB\nFilter 1: ON LSC Fc 105 Hz Gain 4 dB Q 0.7\nFilter 2: ON PK Fc 1000 Hz Gain -2 dB Q 1.41\n",
    )
    .unwrap();
    let id = c.importer(&fichier, T0 + 1).unwrap();
    c.activer_profil(&id);

    let vue = c.vue_egaliseur();
    assert_eq!(vue.filtres_supplementaires, 1);
    assert_eq!(vue.gains[5], -2.0);

    c.regler_bande(9, 2.0);
    let courbe = banc.derniere_courbe();
    assert!(courbe
        .filtres
        .iter()
        .any(|f| f.type_filtre == TypeFiltre::PlateauBas && f.gain_db == 4.0));
    assert_eq!(c.vue_egaliseur().filtres_supplementaires, 1);
}

#[test]
fn couper_l_egaliseur_est_retenu_au_redemarrage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(5, 3.0);
    c.enregistrer_egaliseur(T0 + 1);

    c.basculer_egaliseur();
    assert_eq!(banc.moteur.appels().last(), Some(&Appel::Couper));
    assert!(!c.vue_appli().egaliseur_actif);
    drop(c);

    let mut c = banc.ouvrir(T0 + 10);
    assert_eq!(banc.moteur.appels().last(), Some(&Appel::Couper));
    c.basculer_egaliseur();
    assert_eq!(banc.derniere_courbe().filtres.len(), 1);
    assert!(c.vue_appli().egaliseur_actif);
}

// ----- Profils -----

#[test]
fn creer_dupliquer_renommer_et_activer_un_profil() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(3, 5.0);
    c.enregistrer_egaliseur(T0 + 1);

    let copie = c.dupliquer_profil("mon-profil", T0 + 2).unwrap();
    let plat = c.creer_depuis_preset("À plat", T0 + 3).unwrap();
    c.renommer_profil(&copie, "  Soirée  ");
    assert_eq!(c.vue_profils().len(), 3);
    let vue = c.vue_profils().into_iter().find(|p| p.id == copie).unwrap();
    assert_eq!(vue.nom, "Soirée");
    assert_eq!(vue.courbe.filtres.len(), 1);
    assert!(!vue.actif);

    c.activer_profil(&plat);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(c.vue_appli().profil_actif, "À plat");
    assert_eq!(
        banc.depot()
            .charger_etat_moteur()
            .unwrap()
            .unwrap()
            .profil_id,
        Some(plat.clone())
    );

    // Le profil actif est retrouvé au redémarrage.
    drop(c);
    let mut c = banc.ouvrir(T0 + 10);
    assert_eq!(c.vue_appli().profil_actif, "À plat");

    // Ctrl+Alt+1 : le premier de la liste.
    c.activer_profil_numero(1);
    assert_eq!(c.vue_appli().profil_actif, "Mon profil");
    c.activer_profil_numero(9);
    assert_eq!(c.vue_appli().profil_actif, "Mon profil");

    c.activer_profil("inconnu");
    assert!(c.prendre_annonce().unwrap().erreur);
}

// ----- Import et export -----

fn proches(a: &Courbe, b: &Courbe) -> bool {
    retro_dsp::frequences_log(60)
        .into_iter()
        .all(|f| (a.reponse_totale_db(f) - b.reponse_totale_db(f)).abs() < 0.06)
}

#[test]
fn exporter_puis_importer_rend_la_meme_courbe() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(1, 4.0);
    c.regler_bande(6, -3.5);
    c.regler_bande(9, 2.0);
    c.enregistrer_egaliseur(T0 + 1);
    let origine = c.vue_egaliseur().courbe;
    c.prendre_annonce();

    let exports = banc.dossier.path().join("exports");
    for (format, fichier, origine_attendue) in [
        ("apo", "Mon profil v2.txt", "equalizer_apo"),
        ("peace", "Mon profil v2.peace", "peace"),
        ("autoeq", "Mon profil v2 ParametricEQ.txt", "autoeq"),
    ] {
        c.exporter_profil("mon-profil", format);
        let annonce = c.prendre_annonce().unwrap();
        assert!(!annonce.erreur, "{format} : {}", annonce.texte);
        assert!(annonce.texte.contains(fichier), "{}", annonce.texte);

        let id = c.importer(&exports.join(fichier), T0 + 100).unwrap();
        assert!(!c.prendre_annonce().unwrap().erreur);
        let profil = banc.depot().charger_profil(&id).unwrap();
        assert!(
            proches(&profil.version_active().courbe, &origine),
            "{format} : la courbe relue diffère"
        );
        assert_eq!(
            profil.version_active().origine,
            Origine::Import {
                format: origine_attendue.to_string()
            }
        );
        // Un import n'active rien tout seul.
        assert_eq!(c.vue_appli().profil_actif, "Mon profil");
    }
}

#[test]
fn l_export_wavelet_s_ecrit_mais_ne_se_reimporte_pas() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.exporter_profil_actif("wavelet");
    assert!(!c.prendre_annonce().unwrap().erreur);
    let fichier = banc
        .dossier
        .path()
        .join("exports/Mon profil v1 GraphicEQ.txt");
    assert!(std::fs::read_to_string(&fichier)
        .unwrap()
        .starts_with("GraphicEQ: 20 "));

    assert!(c.importer(&fichier, T0 + 1).is_none());
    let annonce = c.prendre_annonce().unwrap();
    assert!(annonce.erreur);
    assert!(annonce.texte.contains("Wavelet"));
}

#[test]
fn un_fichier_inconnu_ou_absent_donne_un_message_pas_un_plantage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let fichier = banc.dossier.path().join("liste.txt");
    std::fs::write(&fichier, "du pain\ndu lait\n").unwrap();
    assert!(c.importer(&fichier, T0).is_none());
    assert!(c.prendre_annonce().unwrap().erreur);
    assert!(c
        .importer(&banc.dossier.path().join("absent.peace"), T0)
        .is_none());
    assert!(c.prendre_annonce().unwrap().erreur);
    c.exporter_profil("mon-profil", "mp3");
    assert!(c.prendre_annonce().unwrap().erreur);
    assert_eq!(c.vue_profils().len(), 1);
}

// ----- Un test, du départ au profil activé -----

#[test]
fn un_test_complet_de_demarrer_a_activer() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    assert!(c.demarrer_test(T0));
    assert!(c.vue_appli().test_en_cours);
    assert!(c.touches_de_test_actives());

    let vue = c.vue_duels(T0 + 4_000);
    assert_eq!(vue.etat, EtatTestVue::Ecoute);
    assert_eq!((vue.duel, vue.total), (1, 12));
    assert_eq!(vue.compte_a_rebours, "0:06");
    assert!((vue.avancement - 0.4).abs() < 1e-9);
    assert!(vue.suite.ends_with("joue son 1er passage"));

    let fin = jouer(&mut c, &banc, T0, plus_de_graves);

    // Le test est fini : la référence est revenue dans le moteur, les touches sont rendues.
    assert!(!c.test_en_cours());
    assert!(!c.touches_de_test_actives());
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(c.vue_duels(fin).etat, EtatTestVue::Termine);
    // 12 duels plus le duel de vérification.
    assert_eq!(c.vue_duels(fin).journal.len(), 13);
    assert!(banc.journal(&identifiant(T0)).est_clos());
    // Des variantes ont bien été entendues en chemin.
    assert!(banc
        .moteur
        .courbes_appliquees()
        .iter()
        .any(|courbe| !courbe.filtres.is_empty()));

    // Le résultat : les graves montent de 4 dB, rien d'autre.
    let resultat = c.vue_resultat().unwrap();
    assert_eq!(resultat.titre, "Mon profil · version 2");
    let changees: Vec<_> = resultat.zones.iter().filter(|z| z.change).collect();
    assert_eq!(changees.len(), 1);
    assert_eq!(changees[0].nom, "Graves");
    assert_eq!(changees[0].gain_db, 4.0);
    assert!(resultat.resume.starts_with("1 zone change"));
    assert!(resultat.verification.contains("nouvelle version préférée"));
    assert!(resultat.controles.contains("Pareil"));
    assert!(!resultat.active);
    assert!((resultat.preampli_db + resultat.courbe_nouvelle.gain_max_db()).abs() < 1e-6);

    // La nouvelle version existe, mais c'est à l'utilisateur de l'activer.
    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(profil.versions().len(), 2);
    assert_eq!(profil.numero_actif(), 1);
    assert_eq!(
        profil.versions()[1].origine,
        Origine::Test {
            test_id: identifiant(T0)
        }
    );
    // Les zones sans changement retenu passent à un pas de 2 dB ; les graves gardent 4 dB.
    assert_eq!(profil.versions()[1].pas_par_zone[Zone::Graves.index()], 4.0);
    assert_eq!(
        profil.versions()[1].pas_par_zone[Zone::SubGraves.index()],
        2.0
    );
    // 12 duels ne couvrent que 5 couples (zone, sens) : une zone pas encore testée garde son pas.
    assert_eq!(profil.versions()[1].pas_par_zone[Zone::Air.index()], 4.0);

    c.activer_resultat();
    assert!(c.vue_resultat().unwrap().active);
    assert_eq!(banc.derniere_courbe(), resultat.courbe_nouvelle);
    let sauve = banc.depot().charger_etat_moteur().unwrap().unwrap();
    assert_eq!(sauve.version, Some(2));

    c.revenir_reference();
    assert!(!c.vue_resultat().unwrap().active);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .versions()
            .len(),
        2
    );
}

#[test]
fn chaque_reponse_est_sur_le_disque_des_qu_elle_est_donnee() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    // Une réponse peut arriver avant la fin des passages.
    c.repondre(Reponse::B, T0 + 3_000);

    let entrees = banc.journal(&identifiant(T0)).entrees;
    let position = entrees
        .iter()
        .position(|e| {
            matches!(
                e,
                Entree::ReponseDonnee {
                    duel: 0,
                    reponse: Reponse::B,
                    ..
                }
            )
        })
        .unwrap();
    // Elle précède tout ce qui concerne le duel suivant.
    assert!(matches!(
        entrees[position + 1],
        Entree::DuelDemarre { duel: 1, .. }
    ));
    assert_eq!(c.vue_duels(T0 + 3_000).duel, 2);
}

#[test]
fn le_journal_affiche_ne_revele_rien_tant_que_le_test_court() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let mut t = T0;
    for reponse in [Reponse::A, Reponse::B, Reponse::Pareil] {
        for _ in 0..3 {
            t += 1_000;
            c.repondre(reponse, t);
        }
    }
    let vue = c.vue_duels(t);
    assert_eq!(vue.journal.len(), 9);
    // Le journal va du plus récent au plus ancien : toutes les lignes ont la même forme,
    // contrôles compris, et aucune ne nomme une zone ni la référence.
    for (ligne, numero) in vue.journal.iter().zip((1..=9).rev()) {
        assert_eq!(ligne.titre, format!("Duel {numero} · réponse enregistrée"));
        let texte = format!("{} {}", ligne.titre, ligne.detail);
        for interdit in [
            "contrôle",
            "référence",
            "variante",
            "zone",
            "graves",
            "aigus",
        ] {
            assert!(!texte.to_lowercase().contains(interdit), "{texte}");
        }
        // Ce que l'utilisateur a répondu, lui, est montré.
        match numero {
            1..=3 => assert_eq!(ligne.cote, 1),
            4..=6 => assert_eq!(ligne.cote, 2),
            _ => assert_eq!((ligne.cote, ligne.detail.as_str()), (0, "Pareil")),
        }
    }
    // Rien sur le duel en cours.
    assert_eq!(vue.duel, 10);
}

#[test]
fn une_fois_le_test_fini_le_journal_revele_zones_et_controles() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let fin = jouer(&mut c, &banc, T0, plus_de_graves);
    let vue = c.vue_duels(fin);
    assert_eq!(vue.etat, EtatTestVue::Termine);
    let textes: Vec<String> = vue
        .journal
        .iter()
        .map(|l| format!("{} {}", l.titre, l.detail))
        .collect();
    assert!(textes.iter().any(|t| t.contains("contrôle")), "{textes:?}");
    assert!(
        textes.iter().any(|t| t.contains("graves en plus")),
        "{textes:?}"
    );
    assert!(textes.iter().all(|t| !t.contains("réponse enregistrée")));
}

#[test]
fn le_dernier_resultat_survit_au_redemarrage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer(&mut c, &banc, T0, plus_de_graves);
    let avant = c.vue_resultat().unwrap();
    drop(c);

    // Un nouveau contrôleur sur le même dossier retrouve le résultat dans le journal.
    let c = banc.ouvrir(T0 + 3_600_000);
    assert_eq!(c.vue_resultat(), Some(avant.clone()));
    assert!(avant.zones.iter().any(|z| z.change));

    // Il est encore activable, comme avant le redémarrage.
    let mut c = c;
    c.activer_resultat();
    assert!(c.vue_resultat().unwrap().active);
    c.revenir_reference();
    assert!(!c.vue_resultat().unwrap().active);
}

#[test]
fn le_resultat_retrouve_est_le_plus_recent_du_profil_actif() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer(&mut c, &banc, T0, plus_de_graves);
    let t1 = T0 + 7_200_000;
    c.demarrer_test(t1);
    jouer(&mut c, &banc, t1, |_| None);
    let second = c.vue_resultat().unwrap();
    // Le second test n'a rien changé : le résultat montré n'est pas celui du premier.
    assert!(second.zones.iter().all(|z| !z.change));
    drop(c);
    let c = banc.ouvrir(t1 + 3_600_000);
    assert_eq!(c.vue_resultat(), Some(second));
}

#[test]
fn sans_test_fini_il_n_y_a_pas_de_resultat_au_demarrage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    // Un test abandonné n'a pas de résultat à retrouver.
    c.arreter_test(T0 + 5_000);
    drop(c);
    assert!(banc.ouvrir(T0 + 60_000).vue_resultat().is_none());
}

#[test]
fn le_resultat_profil_contre_profil_survit_au_redemarrage() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let autre = c.creer_depuis_preset("À plat", T0 - 10).unwrap();
    c.renommer_profil(&autre, "Harman");
    c.activer_profil(&autre);
    c.regler_bande(1, 6.0);
    c.enregistrer_egaliseur(T0 - 5);
    c.activer_profil("mon-profil");
    c.choisir_test(ChoixTest {
        autre_profil: "Harman".to_string(),
        ..choix("profil-contre-profil")
    });
    assert!(c.demarrer_test(T0));
    jouer(&mut c, &banc, T0, |sujet| match sujet {
        Sujet::AutreProfil => Some(true),
        _ => None,
    });
    let avant = c.vue_resultat().unwrap();
    assert!(avant.titre.contains("Harman"));
    drop(c);
    let c = banc.ouvrir(T0 + 3_600_000);
    assert_eq!(c.vue_resultat(), Some(avant));
}

#[test]
fn sans_changement_la_nouvelle_version_est_activee_avec_des_pas_reduits() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer(&mut c, &banc, T0, |_| None);

    let resultat = c.vue_resultat().unwrap();
    assert!(resultat.resume.starts_with("Aucune zone"));
    assert!(resultat.active);
    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(profil.numero_actif(), 2);
    assert_eq!(profil.version_active().courbe, profil.versions()[0].courbe);
    assert!(profil.version_active().pas_par_zone.contains(&2.0));
    assert_eq!(
        banc.depot().charger_etat_moteur().unwrap().unwrap().version,
        Some(2)
    );
}

#[test]
fn une_seule_zone_ne_teste_que_cette_zone() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(ChoixTest {
        zone: 5,
        ..choix("une-zone")
    });
    let vue = c.vue_nouveau_test();
    assert!(vue.mode.contains("Aigus"));
    assert_eq!(
        vue.duree,
        "6 duels · environ 6 min · bascule toutes les 10 s"
    );
    assert_eq!(vue.reference, "Mon profil · version 1");

    c.demarrer_test(T0);
    let duels = banc.duels(&identifiant(T0));
    assert_eq!(duels.len(), 6);
    assert!(duels.iter().all(|d| matches!(
        d.sujet,
        Sujet::Controle
            | Sujet::Zone {
                zone: Zone::Aigus,
                ..
            }
    )));
}

#[test]
fn profil_contre_profil_propose_d_activer_le_gagnant() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let autre = c.creer_depuis_preset("À plat", T0 - 10).unwrap();
    c.renommer_profil(&autre, "Harman");
    c.activer_profil(&autre);
    c.regler_bande(1, 6.0);
    c.enregistrer_egaliseur(T0 - 5);
    c.activer_profil("mon-profil");

    c.choisir_test(choix("profil-contre-profil"));
    assert!(!c.demarrer_test(T0));
    assert!(c.prendre_annonce().unwrap().erreur);

    c.choisir_test(ChoixTest {
        autre_profil: "Harman".to_string(),
        ..choix("profil-contre-profil")
    });
    assert_eq!(c.vue_nouveau_test().profils, vec!["Harman".to_string()]);
    assert!(c.demarrer_test(T0));
    jouer(&mut c, &banc, T0, |sujet| match sujet {
        Sujet::AutreProfil => Some(true),
        _ => None,
    });

    let resultat = c.vue_resultat().unwrap();
    assert_eq!(resultat.titre, "Mon profil contre Harman");
    assert!(resultat.resume.contains("« Harman » préféré"));
    assert!(!resultat.active);
    // Aucune version n'est ajoutée dans ce mode.
    assert_eq!(
        banc.depot()
            .charger_profil("mon-profil")
            .unwrap()
            .versions()
            .len(),
        1
    );

    c.activer_resultat();
    assert_eq!(c.vue_appli().profil_actif, "Harman");
    assert!(c.vue_resultat().unwrap().active);
    c.revenir_reference();
    assert_eq!(c.vue_appli().profil_actif, "Mon profil");
    assert!(banc.derniere_courbe().filtres.is_empty());
}

#[test]
fn pause_plus_tard_annuler_passer_et_arreter() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);

    c.mettre_en_pause(T0 + 1_000);
    assert_eq!(c.vue_duels(T0 + 1_000).etat, EtatTestVue::Pause);
    assert!(banc.derniere_courbe().filtres.is_empty());
    // En pause voulue, F6 à F10 sont rendues aux jeux.
    assert!(!c.touches_de_test_actives());
    assert!(c.test_en_cours());
    c.reprendre(T0 + 2_000);
    assert_eq!(c.vue_duels(T0 + 2_000).etat, EtatTestVue::Ecoute);
    assert!(c.touches_de_test_actives());

    c.repondre(Reponse::A, T0 + 3_000);
    c.annuler_reponse(T0 + 4_000);
    assert_eq!(c.vue_duels(T0 + 4_000).duel, 1);
    c.passer_duel(T0 + 5_000);
    assert_eq!(c.vue_duels(T0 + 5_000).duel, 2);
    assert_eq!(
        c.vue_duels(T0 + 5_000).journal[0].detail,
        "passé, sans réponse"
    );
    c.plus_tard(T0 + 6_000);
    assert_eq!(c.vue_duels(T0 + 6_000).etat, EtatTestVue::Pause);

    // Ctrl+Alt+Entrée reprend le test en pause au lieu d'en lancer un second.
    assert!(c.demarrer_ou_reprendre(T0 + 7_000));
    assert_eq!(c.vue_duels(T0 + 7_000).etat, EtatTestVue::Ecoute);
    assert_eq!(c.vue_duels(T0 + 7_000).duel, 2);

    c.arreter_test(T0 + 8_000);
    assert!(!c.test_en_cours());
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert!(banc.journal(&identifiant(T0)).est_clos());
    assert!(c.vue_resultat().is_none());
    assert_eq!(c.vue_duels(T0 + 8_000), VueDuels::default());
}

#[test]
fn pendant_un_test_l_egaliseur_et_les_profils_sont_proteges() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let autre = c.creer_depuis_preset("À plat", T0 - 1).unwrap();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.prendre_annonce();
    let avant = banc.moteur.appels().len();

    c.regler_bande(4, 6.0);
    assert!(c.prendre_annonce().unwrap().erreur);
    c.activer_profil(&autre);
    assert!(c.prendre_annonce().unwrap().erreur);
    c.basculer_egaliseur();
    assert!(c.prendre_annonce().unwrap().erreur);

    assert_eq!(banc.moteur.appels().len(), avant);
    assert!(!c.vue_egaliseur().modifie);
    assert_eq!(c.vue_appli().profil_actif, "Mon profil");
}

#[test]
fn un_test_ne_demarre_pas_sans_accord_ni_avec_un_brouillon() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir(T0);
    c.choisir_test(choix("duels-en-jeu"));
    assert!(!c.demarrer_test(T0));
    assert!(c
        .prendre_annonce()
        .unwrap()
        .texte
        .contains("pas encore activé"));

    c.autoriser_moteur();
    c.regler_bande(0, 1.0);
    assert!(!c.demarrer_test(T0));
    assert!(c.prendre_annonce().unwrap().erreur);

    c.annuler_egaliseur();
    c.choisir_test(choix("abx"));
    assert!(!c.demarrer_test(T0));
    assert!(c.prendre_annonce().unwrap().erreur);
    assert!(banc.depot().tests_interrompus().unwrap().is_empty());
}

#[test]
fn un_appel_met_en_pause_seulement_si_le_reglage_le_veut() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.signaler_appel(true, T0 + 1_000);
    assert_eq!(c.vue_duels(T0 + 1_000).etat, EtatTestVue::PauseAppel);
    assert!(banc.derniere_courbe().filtres.is_empty());
    // Le test repart tout seul : les touches restent prises.
    assert!(c.touches_de_test_actives());
    c.signaler_appel(false, T0 + 5_000);
    assert_eq!(c.vue_duels(T0 + 5_000).etat, EtatTestVue::Ecoute);

    c.regler_pause_appels(false);
    c.signaler_appel(true, T0 + 6_000);
    assert_eq!(c.vue_duels(T0 + 6_000).etat, EtatTestVue::Ecoute);
}

#[test]
fn les_bips_confirment_les_reponses_et_previennent_en_plein_ecran() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.repondre(Reponse::Pareil, T0 + 1_000);
    assert_eq!(c.prendre_bips(), 1);
    assert_eq!(c.prendre_bips(), 0);

    // Plein écran exclusif : la mini-fenêtre ne peut pas s'afficher, un bip pose la question.
    assert!(c.mini_fenetre_voulue());
    c.signaler_plein_ecran(true);
    assert!(!c.mini_fenetre_voulue());
    let mut t = T0 + 1_000;
    for _ in 0..6 {
        t += PASSAGE;
        c.tic(t);
    }
    assert_eq!(c.vue_duels(t).etat, EtatTestVue::AttenteReponse);
    assert!(c.reecoute_possible());
    assert_eq!(c.prendre_bips(), 1);

    // Réglage « bip » coupé : plus de confirmation au test suivant.
    c.arreter_test(t);
    c.regler_bip(false);
    c.signaler_plein_ecran(false);
    c.demarrer_test(t + 1_000);
    c.repondre(Reponse::Pareil, t + 2_000);
    assert_eq!(c.prendre_bips(), 0);
}

#[test]
fn maintenir_une_touche_fait_reecouter_et_ce_cote_reste_en_place() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let mut t = T0;
    for _ in 0..6 {
        t += PASSAGE;
        c.tic(t);
    }
    let avant = banc.moteur.appels().len();
    c.reecouter(Cote::A, true, t + 100);
    assert_eq!(c.vue_duels(t + 100).cote_actif, 1);
    assert_eq!(banc.moteur.appels().len(), avant + 1);
    // Au relâchement, on ne repasse pas par la référence : l'entendre revenir dirait quel
    // côté elle était (ADR-0012). Le moteur ne reçoit rien de plus.
    let apres = banc.moteur.appels().len();
    c.reecouter(Cote::A, false, t + 900);
    assert_eq!(banc.moteur.appels().len(), apres);
    assert_eq!(c.vue_duels(t + 900).cote_actif, 1);
    assert_eq!(c.vue_duels(t + 900).etat, EtatTestVue::AttenteReponse);
}

// ----- Reprise après un arrêt brutal -----

#[test]
fn apres_un_arret_brutal_la_reference_revient_et_le_test_se_reprend() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.regler_bande(7, -3.0);
    c.enregistrer_egaliseur(T0 - 1_000);
    let reference = c.vue_egaliseur().courbe;
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let duels = banc.duels(&identifiant(T0));

    // Trois réponses, puis on s'arrange pour qu'une variante soit en train de jouer.
    let mut t = T0;
    for duel in duels.iter().take(3) {
        t += 2_000;
        c.repondre(reponse_pour(duel, true), t);
    }
    let mut garde = 0;
    while banc.derniere_courbe() == reference {
        t += PASSAGE;
        c.tic(t);
        garde += 1;
        assert!(garde < 12);
    }
    assert_ne!(banc.derniere_courbe(), reference);

    // Plantage : le contrôleur disparaît sans rien ranger.
    drop(c);

    // Nouveau lancement, moteur neuf : la première chose écrite est la référence.
    let banc2 = Banc {
        dossier: banc.dossier,
        moteur: MoteurSimule::nouveau(),
    };
    let mut c = banc2.ouvrir(t + 60_000);
    assert_eq!(banc2.moteur.courbes_appliquees()[0], reference);
    assert_eq!(banc2.derniere_courbe(), reference);

    // Le test interrompu est proposé, en pause, avec ses réponses.
    assert!(c.vue_appli().test_en_cours);
    assert!(!c.prendre_annonce().unwrap().erreur);
    let vue = c.vue_duels(t + 60_000);
    assert_eq!(vue.etat, EtatTestVue::Pause);
    assert_eq!(vue.duel, 4);
    assert_eq!(vue.journal.len(), 3);
    assert!(!c.touches_de_test_actives());

    // Il reprend au début du duel interrompu, et va jusqu'au bout.
    let reprise = t + 61_000;
    c.reprendre(reprise);
    let vue = c.vue_duels(reprise);
    assert_eq!(vue.etat, EtatTestVue::Ecoute);
    assert_eq!((vue.passages_a, vue.passages_b), (0, 0));
    let mut t = reprise;
    while c.test_en_cours() {
        t += 1_000;
        c.repondre(Reponse::Pareil, t);
    }
    assert_eq!(banc2.derniere_courbe(), reference);
    assert!(c.vue_resultat().is_some());
    assert!(banc2.depot().tests_interrompus().unwrap().is_empty());
    assert_eq!(
        banc2
            .depot()
            .charger_profil("mon-profil")
            .unwrap()
            .versions()
            .len(),
        3
    );
}

#[test]
fn seul_le_test_interrompu_le_plus_recent_est_propose() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.repondre(Reponse::A, T0 + 1_000);
    drop(c);
    // Deuxième lancement : le test est repris en pause, puis nouveau plantage.
    let c = banc.ouvrir(T0 + 10_000);
    assert!(c.test_en_cours());
    drop(c);
    // Un journal illisible d'un autre test traîne dans le dossier.
    std::fs::write(
        banc.dossier.path().join("donnees/tests/t1.jsonl"),
        "{\"type\":\"pause\",\"raison\":\"Manuelle\",\"horodatage_ms\":1}\n",
    )
    .unwrap();

    let c = banc.ouvrir(T0 + 20_000);
    assert!(c.test_en_cours());
    assert_eq!(c.vue_duels(T0 + 20_000).journal.len(), 1);
    // L'autre est noté abandonné : il ne sera plus proposé, mais son fichier est gardé.
    assert_eq!(
        banc.depot().tests_interrompus().unwrap(),
        vec![identifiant(T0)]
    );
    assert!(banc.dossier.path().join("donnees/tests/t1.jsonl").exists());
}

#[test]
fn un_journal_inutilisable_n_empeche_pas_de_demarrer() {
    let banc = Banc::nouveau();
    drop(banc.ouvrir_autorise());
    std::fs::write(
        banc.dossier.path().join("donnees/tests/t5.jsonl"),
        "{\"type\":\"pause\",\"raison\":\"Manuelle\",\"horodatage_ms\":1}\n",
    )
    .unwrap();
    let mut c = banc.ouvrir(T0 + 10);
    assert!(!c.test_en_cours());
    assert!(c.prendre_annonce().unwrap().erreur);
    assert!(banc.depot().tests_interrompus().unwrap().is_empty());
}

// ----- Pannes du moteur et du disque -----

#[test]
fn une_panne_du_moteur_hors_test_devient_un_message() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    banc.moteur.echouer_au_prochain_appel("disque plein");
    c.regler_bande(3, 2.0);
    let annonce = c.prendre_annonce().unwrap();
    assert!(annonce.erreur);
    assert!(annonce.texte.contains("disque plein"));
    // L'appli continue : le réglage suivant passe.
    c.regler_bande(3, 3.0);
    assert!(c.prendre_annonce().is_none());
    assert_eq!(banc.derniere_courbe().filtres[0].gain_db, 3.0);
}

#[test]
fn une_panne_du_moteur_au_demarrage_n_empeche_pas_d_ouvrir() {
    let banc = Banc::nouveau();
    drop(banc.ouvrir_autorise());
    banc.moteur.echouer_au_prochain_appel("dossier protégé");
    let mut c = banc.ouvrir(T0 + 10);
    assert!(c
        .prendre_annonce()
        .unwrap()
        .texte
        .contains("dossier protégé"));
    assert_eq!(c.vue_appli().profil_actif, "Mon profil");
}

#[test]
fn une_panne_du_moteur_pendant_un_test_met_en_pause_sur_la_reference() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    // Le prochain changement de passage échoue.
    banc.moteur.echouer_au_prochain_appel("fichier verrouillé");
    c.tic(T0 + PASSAGE);

    let annonce = c.prendre_annonce().unwrap();
    assert!(annonce.erreur);
    assert!(annonce.texte.contains("fichier verrouillé"));
    assert_eq!(c.vue_duels(T0 + PASSAGE).etat, EtatTestVue::Pause);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert!(c.test_en_cours());

    // Et le test peut repartir ensuite.
    c.reprendre(T0 + 2 * PASSAGE);
    assert_eq!(c.vue_duels(T0 + 2 * PASSAGE).etat, EtatTestVue::Ecoute);
}

#[test]
fn un_journal_impossible_a_ouvrir_empeche_le_test_et_le_dit() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    // Le dossier des journaux est remplacé par un fichier : aucune écriture n'y est possible.
    let tests = banc.dossier.path().join("donnees/tests");
    std::fs::remove_dir_all(&tests).unwrap();
    std::fs::write(&tests, "").unwrap();

    c.choisir_test(choix("duels-en-jeu"));
    let avant = banc.moteur.appels().len();
    assert!(!c.demarrer_test(T0));
    assert!(c.prendre_annonce().unwrap().erreur);
    assert_eq!(banc.moteur.appels().len(), avant);
}

#[test]
fn des_reglages_illisibles_sont_remplaces_par_les_valeurs_par_defaut() {
    let banc = Banc::nouveau();
    drop(banc.ouvrir(T0));
    std::fs::write(
        banc.dossier.path().join("donnees/reglages.json"),
        "{ pas du json",
    )
    .unwrap();
    let mut c = banc.ouvrir(T0 + 1);
    assert!(c.prendre_annonce().unwrap().erreur);
    assert_eq!(c.vue_reglages().intervalle_s, 20);
    assert!(!c.moteur_autorise());
}

#[test]
fn les_reglages_sont_gardes_d_un_lancement_a_l_autre() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir(T0);
    c.choisir_langue("en");
    c.regler_rail(true);
    c.regler_fermer_reduit(false);
    c.regler_mini_fenetre(false);
    c.regler_intervalle(45);
    c.regler_intervalle(3);
    drop(c);
    let c = banc.ouvrir(T0 + 1);
    let vue = c.vue_reglages();
    assert_eq!(vue.langue, "en");
    assert!(vue.rail && !vue.fermer_reduit && !vue.mini_fenetre);
    assert_eq!(vue.intervalle_s, 10);
    assert!(!c.fermer_reduit());
}

// ----- Rester à l'aveugle (ADR-0012) et décider prudemment (ADR-0005) -----

#[test]
fn pendant_la_question_la_reference_n_est_pas_remise() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    let mut t = T0;
    for _ in 0..5 {
        t += PASSAGE;
        c.tic(t);
    }
    let avant = banc.moteur.appels().len();
    t += PASSAGE;
    c.tic(t);
    assert_eq!(c.vue_duels(t).etat, EtatTestVue::AttenteReponse);
    // Le dernier côté entendu reste en place : rien n'est envoyé au moteur.
    assert_eq!(banc.moteur.appels().len(), avant);
    assert_ne!(c.vue_duels(t).cote_actif, 0);
}

#[test]
fn sans_reponse_le_test_se_met_en_pause_sur_la_reference_et_le_dit() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    c.prendre_annonce();
    let mut t = T0;
    for _ in 0..6 {
        t += PASSAGE;
        c.tic(t);
    }
    // Deux minutes passent sans réponse.
    for _ in 0..13 {
        t += PASSAGE;
        c.tic(t);
    }
    assert_eq!(c.vue_duels(t).etat, EtatTestVue::Pause);
    assert!(c.vue_duels(t).pause_sans_reponse);
    assert!(banc.derniere_courbe().filtres.is_empty());
    assert!(!c.touches_de_test_actives());
    let annonce = c.prendre_annonce().unwrap();
    assert!(!annonce.erreur);
    assert!(annonce.texte.contains("Pas de réponse"));
    // Dit une seule fois.
    c.tic(t + 1_000);
    assert!(c.prendre_annonce().is_none());

    // À la reprise, le duel est rejoué avec un nouveau tirage, noté dans le journal.
    c.reprendre(t + 2_000);
    assert_eq!(c.vue_duels(t + 2_000).etat, EtatTestVue::Ecoute);
    assert_eq!(c.vue_duels(t + 2_000).duel, 1);
    assert!(banc
        .journal(&identifiant(T0))
        .entrees
        .iter()
        .any(|e| matches!(e, Entree::DuelRetire { duel: 0, .. })));
}

#[test]
fn si_la_verification_prefere_la_reference_l_activation_n_est_pas_mise_en_avant() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer(&mut c, &banc, T0, |sujet| match sujet {
        Sujet::Verification => Some(false),
        autre => plus_de_graves(autre),
    });
    let resultat = c.vue_resultat().unwrap();
    assert!(resultat.zones.iter().any(|z| z.change));
    assert!(!resultat.recommandee);
    assert!(!resultat.active);
    assert!(resultat.verification.contains("préféré ta référence"));
    // La version existe quand même, et l'utilisateur garde la main.
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
}

#[test]
fn des_differences_entendues_aux_controles_ne_laissent_rien_retenir() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    // Toujours « la variante », y compris quand les deux côtés sont identiques.
    jouer(&mut c, &banc, T0, |_| Some(true));
    let resultat = c.vue_resultat().unwrap();
    assert!(resultat.zones.iter().all(|z| !z.change));
    assert!(resultat.resume.starts_with("Rien n’est retenu"));
    assert!(resultat.controles.contains("aucun changement n’est retenu"));
    assert!(!resultat.recommandee);
    let profil = banc.depot().charger_profil("mon-profil").unwrap();
    assert_eq!(profil.version_active().courbe, profil.versions()[0].courbe);
}

#[test]
fn une_seule_victoire_est_un_indice_rejoue_en_tete_du_test_suivant() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    // Plus de graves préféré une seule fois sur les deux duels de ce couple.
    let vus = std::cell::Cell::new(0);
    let fin = jouer(&mut c, &banc, T0, |sujet| match sujet {
        Sujet::Zone {
            zone: Zone::Graves,
            sens: retro_core::Sens::Plus,
        } => {
            vus.set(vus.get() + 1);
            (vus.get() == 1).then_some(true)
        }
        _ => None,
    });
    let resultat = c.vue_resultat().unwrap();
    let graves = resultat.zones.iter().find(|z| z.nom == "Graves").unwrap();
    assert!(!graves.change);
    assert!(
        graves
            .detail
            .ends_with("un indice, à confirmer au prochain test"),
        "{}",
        graves.detail
    );

    let suivant = fin + 60_000;
    assert!(c.demarrer_test(suivant));
    let duels = banc.duels(&identifiant(suivant));
    let graves_plus = duels
        .iter()
        .filter(|d| {
            d.sujet
                == Sujet::Zone {
                    zone: Zone::Graves,
                    sens: retro_core::Sens::Plus,
                }
        })
        .count();
    assert_eq!(
        graves_plus, 2,
        "l'indice est rejoué, deux fois, au test suivant"
    );
}

#[test]
fn un_import_partiel_dit_ce_qui_a_ete_laisse_de_cote() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let fichier = banc.dossier.path().join("mixte.txt");
    std::fs::write(
        &fichier,
        "# mon réglage\nPreamp: -3 dB\nFilter 1: ON PK Fc 1000 Hz Gain 3 dB Q 1.41\nFilter 2: ON LP Fc 18000 Hz\nFilter 3: OFF PK Fc 50 Hz Gain 2 dB Q 1\n",
    )
    .unwrap();
    assert!(c.importer(&fichier, T0).is_some());
    let annonce = c.prendre_annonce().unwrap();
    assert!(!annonce.erreur);
    assert!(
        annonce.texte.contains("en partie seulement"),
        "{}",
        annonce.texte
    );
    assert!(
        annonce.texte.contains("1 ligne laissée de côté"),
        "{}",
        annonce.texte
    );
    assert!(
        annonce.texte.contains("type de filtre non géré"),
        "{}",
        annonce.texte
    );
}

#[test]
fn un_import_dit_quand_son_preampli_est_remplace_par_l_automatique() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let ecrire = |nom: &str, preampli: &str| {
        let fichier = banc.dossier.path().join(nom);
        std::fs::write(
            &fichier,
            format!("Preamp: {preampli} dB\nFilter 1: ON PK Fc 1000 Hz Gain 4.2 dB Q 1.41\n"),
        )
        .unwrap();
        fichier
    };

    // Préampli du fichier à -6 dB, automatique à -4,2 dB : on le dit, avec les deux valeurs.
    assert!(c.importer(&ecrire("a.txt", "-6"), T0).is_some());
    let annonce = c.prendre_annonce().unwrap();
    assert!(!annonce.erreur);
    assert!(
        annonce
            .texte
            .contains("préampli du fichier (−6 dB) remplacé par le préampli automatique (−4,2 dB)"),
        "{}",
        annonce.texte
    );
    // Le profil créé porte bien le préampli automatique.
    let profil = c.vue_profils().into_iter().last().unwrap();
    assert!((profil.courbe.preampli_db - profil.courbe.preampli_auto_db()).abs() < 1e-9);
    assert!((profil.courbe.preampli_db + 4.2).abs() < 0.05);

    // À 0,3 dB près, rien à signaler.
    assert!(c.importer(&ecrire("b.txt", "-4.5"), T0 + 1).is_some());
    let annonce = c.prendre_annonce().unwrap();
    assert!(!annonce.texte.contains("préampli"), "{}", annonce.texte);
}

// ----- Langue -----

#[test]
fn au_premier_lancement_l_appli_prend_la_langue_du_systeme() {
    let banc = Banc::nouveau();
    let options = Options {
        langue_systeme: Langue::En,
        ..banc.options()
    };
    let c = banc.ouvrir_avec(options.clone(), T0);
    assert_eq!(c.langue(), Langue::En);
    assert_eq!(c.vue_reglages().langue, "en");
    // Le profil de départ porte son nom dans la langue de l'appli.
    assert_eq!(c.vue_profils()[0].nom, "My profile");
    assert_eq!(c.vue_nouveau_test().source, "What plays on the PC");
}

#[test]
fn le_choix_de_l_utilisateur_l_emporte_ensuite_sur_la_langue_du_systeme() {
    let banc = Banc::nouveau();
    let systeme_anglais = Options {
        langue_systeme: Langue::En,
        ..banc.options()
    };
    let mut c = banc.ouvrir_avec(systeme_anglais.clone(), T0);
    c.choisir_langue("fr");
    assert_eq!(c.langue(), Langue::Fr);
    drop(c);
    // Au lancement suivant, le français choisi est resté, même si le système est en anglais.
    let c = banc.ouvrir_avec(systeme_anglais, T0 + 1_000);
    assert_eq!(c.langue(), Langue::Fr);
    // Un code inconnu ne change rien.
    let mut c = c;
    c.choisir_langue("de");
    assert_eq!(c.langue(), Langue::Fr);
}

#[test]
fn une_langue_imposee_ne_touche_pas_aux_reglages_enregistres() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_langue("fr");
    drop(c);
    let imposee = Options {
        langue_imposee: Some(Langue::En),
        ..banc.options()
    };
    let c = banc.ouvrir_avec(imposee, T0 + 1_000);
    assert_eq!(c.langue(), Langue::En);
    drop(c);
    assert_eq!(banc.depot().charger_reglages().unwrap().langue, Langue::Fr);
}

#[test]
fn changer_de_langue_change_toutes_les_phrases_des_vues_et_des_annonces() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_test(choix("duels-en-jeu"));
    c.demarrer_test(T0);
    jouer(&mut c, &banc, T0, plus_de_graves);
    let fr = c.vue_resultat().unwrap();
    assert!(fr.resume.contains("zone change"));

    c.choisir_langue("en");
    let en = c.vue_resultat().unwrap();
    assert_eq!(en.resume, "1 zone changes, 2 stay as before.");
    assert_ne!(fr.zones[0].detail, en.zones[0].detail);
    assert!(en.zones.iter().any(|z| z.nom == "Bass"));
    assert!(en.controles.starts_with("Out of "));
    assert!(en.verification.starts_with("Verification duel"));
    // Le journal du test fini, révélé, parle anglais aussi.
    let journal = c.vue_duels(T0 + 10_000_000).journal;
    assert!(journal.iter().any(|l| l.titre.contains("control")));

    // Les annonces et les erreurs aussi.
    c.activer_resultat();
    assert!(c.prendre_annonce().unwrap().texte.contains("activated"));
    c.arreter_test(T0);
    c.regler_bande(0, 1.0);
    c.choisir_test(choix("duels-en-jeu"));
    assert!(!c.demarrer_test(T0 + 20_000_000));
    let erreur = c.prendre_annonce().unwrap();
    assert!(erreur.erreur);
    assert_eq!(
        erreur.texte,
        "Save or undo your changes in the equalizer first."
    );
}

#[test]
fn un_fichier_illisible_donne_une_erreur_dans_la_langue_choisie() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_langue("en");
    let fichier = banc.dossier.path().join("vide.txt");
    std::fs::write(&fichier, "du texte sans rien d'utile").unwrap();
    assert!(c.importer(&fichier, T0).is_none());
    let annonce = c.prendre_annonce().unwrap();
    assert!(annonce.erreur);
    assert_eq!(annonce.texte, "File format not recognised.");
    assert!(c
        .importer(&banc.dossier.path().join("absent.txt"), T0)
        .is_none());
    let annonce = c.prendre_annonce().unwrap();
    assert!(annonce.texte.starts_with("Reading "), "{}", annonce.texte);
}

#[test]
fn un_preset_se_cree_sous_son_nom_traduit() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.choisir_langue("en");
    assert_eq!(c.presets(), vec!["Flat".to_string()]);
    let id = c.creer_depuis_preset("Flat", T0).unwrap();
    let nom = c
        .vue_profils()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap()
        .nom;
    assert_eq!(nom, "Flat");
    // Le nom d'origine marche aussi.
    assert!(c.creer_depuis_preset("À plat", T0 + 1).is_some());
}

// ----- Spectre capté (ADR-0006, ticket 16) -----

/// Un plateau bas marqué et un préampli : de quoi voir si la courbe est bien retirée.
fn courbe_marquee() -> Courbe {
    Courbe {
        preampli_db: -4.0,
        filtres: vec![retro_dsp::Filtre {
            type_filtre: TypeFiltre::PlateauBas,
            frequence_hz: 100.0,
            gain_db: 8.0,
            q: 0.71,
        }],
    }
}

/// Ce que la capture entend quand `courbe` joue sur un son au spectre `2 × rose`.
fn capte_a_travers(courbe: &Courbe) -> Spectre {
    let rose = Spectre::rose(POINTS_SPECTRE);
    let puissances = rose
        .frequences_hz
        .iter()
        .map(|&f| 2.0 * 10f64.powf(courbe.reponse_totale_db(f) / 10.0))
        .collect();
    Spectre {
        frequences_hz: rose.frequences_hz,
        puissances,
    }
}

fn est_le_son_d_origine(spectre: &Spectre) -> bool {
    spectre.puissances.iter().all(|&p| (p - 2.0).abs() < 1e-9)
}

#[test]
fn le_spectre_capte_est_debarrasse_de_la_courbe_qui_joue() {
    // La capture loopback se fait après Equalizer APO (mesuré sur le PC) : sans division,
    // la compensation de volume serait calculée sur un son déjà égalisé.
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    let courbe = courbe_marquee();
    c.appliquer_au_moteur(&courbe).unwrap();
    let capte = capte_a_travers(&courbe);
    // Juste après un changement de courbe, la mesure peut mêler l'ancienne et la nouvelle.
    c.maj_spectre(capte.clone());
    assert_eq!(
        c.spectre,
        Spectre::rose(POINTS_SPECTRE),
        "première mesure ignorée"
    );
    // La courbe n'a pas bougé depuis : le spectre est retenu, sans l'effet de la courbe.
    c.maj_spectre(capte);
    assert!(
        est_le_son_d_origine(&c.spectre),
        "{:?}",
        &c.spectre.puissances[..5]
    );
}

#[test]
fn une_mesure_a_cheval_sur_un_changement_de_courbe_est_ignoree() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.maj_spectre(capte_a_travers(&Courbe::default()));
    c.maj_spectre(capte_a_travers(&Courbe::default()));
    assert!(est_le_son_d_origine(&c.spectre));
    // Une nouvelle courbe arrive entre deux mesures : la suivante est mise de côté…
    c.appliquer_au_moteur(&courbe_marquee()).unwrap();
    let melange = capte_a_travers(&Courbe::default());
    c.maj_spectre(Spectre {
        puissances: melange.puissances.iter().map(|p| p * 10.0).collect(),
        ..melange
    });
    assert!(est_le_son_d_origine(&c.spectre), "mesure à cheval ignorée");
    // …et la suivante, stable, est retenue.
    c.maj_spectre(capte_a_travers(&courbe_marquee()));
    assert!(est_le_son_d_origine(&c.spectre));
}

#[test]
fn egaliseur_coupe_ou_son_rendu_le_spectre_capte_est_pris_tel_quel() {
    let banc = Banc::nouveau();
    let mut c = banc.ouvrir_autorise();
    c.appliquer_au_moteur(&courbe_marquee()).unwrap();
    c.rendre_son_origine(T0);
    c.maj_spectre(capte_a_travers(&Courbe::default()));
    c.maj_spectre(capte_a_travers(&Courbe::default()));
    assert!(est_le_son_d_origine(&c.spectre));
}
