#![allow(clippy::unwrap_used)]

use super::*;
use retro_core::formats::ErreurFormat;
use std::path::PathBuf;

const FR: Langue = Langue::Fr;
const EN: Langue = Langue::En;

#[test]
fn les_plages_de_zone_melangent_hz_et_khz_quand_il_faut() {
    assert_eq!(plage_zone(Zone::Graves, FR), "60–250 Hz");
    assert_eq!(plage_zone(Zone::Mediums, FR), "500 Hz–2 kHz");
    assert_eq!(plage_zone(Zone::Presence, FR), "2–5 kHz");
}

#[test]
fn le_compte_a_rebours_arrondit_vers_le_haut() {
    assert_eq!(compte_a_rebours(20_000), "0:20");
    assert_eq!(compte_a_rebours(11_001), "0:12");
    assert_eq!(compte_a_rebours(120_000), "2:00");
    assert_eq!(compte_a_rebours(0), "0:00");
}

#[test]
fn la_duree_d_un_test_compte_six_passages_par_duel() {
    assert_eq!(
        duree_test(12, 20, FR),
        "12 duels · environ 24 min · bascule toutes les 20 s"
    );
    assert_eq!(
        duree_test(6, 120, FR),
        "6 duels · environ 72 min · bascule toutes les 2 min"
    );
}

fn duel_fini(sujet: Sujet, reponse: Reponse, reference: Cote) -> DuelFini {
    let variante_a_gauche = reference == Cote::B;
    let preference = match reponse {
        Reponse::Pareil => Preference::Aucune,
        Reponse::A if variante_a_gauche => Preference::Variante,
        Reponse::B if !variante_a_gauche => Preference::Variante,
        _ => Preference::Reference,
    };
    DuelFini {
        numero: 3,
        sujet,
        reponse: Some(reponse),
        reference,
        preference: Some(preference),
    }
}

#[test]
fn pendant_le_test_toutes_les_lignes_du_journal_sont_identiques() {
    let sujets = [
        Sujet::Controle,
        Sujet::Zone {
            zone: Zone::Graves,
            sens: Sens::Plus,
        },
        Sujet::AutreProfil,
        Sujet::Verification,
    ];
    for sujet in sujets {
        for reponse in [Reponse::A, Reponse::B, Reponse::Pareil] {
            for reference in [Cote::A, Cote::B] {
                let (titre, detail) =
                    ligne_journal(&duel_fini(sujet, reponse, reference), false, FR);
                assert_eq!(titre, "Duel 3 · réponse enregistrée");
                // Le détail ne dit que la réponse : « Pareil », ou rien (la pastille A ou B).
                let attendu = if reponse == Reponse::Pareil {
                    "Pareil"
                } else {
                    ""
                };
                assert_eq!(detail, attendu);
            }
        }
    }
}

#[test]
fn une_fois_le_test_fini_le_journal_donne_tout_le_detail() {
    let zone = Sujet::Zone {
        zone: Zone::Graves,
        sens: Sens::Plus,
    };
    let (titre, detail) = ligne_journal(&duel_fini(zone, Reponse::B, Cote::A), true, FR);
    assert_eq!(titre, "Duel 3 · graves en plus");
    assert_eq!(detail, "était la variante · préférée");
    let (titre, _) = ligne_journal(&duel_fini(Sujet::Controle, Reponse::A, Cote::A), true, FR);
    assert!(titre.contains("contrôle"));
}

#[test]
fn un_duel_passe_ne_revele_rien_non_plus() {
    let duel = DuelFini {
        numero: 2,
        sujet: Sujet::Controle,
        reponse: None,
        reference: Cote::A,
        preference: None,
    };
    assert_eq!(
        ligne_journal(&duel, false, FR),
        ("Duel 2".to_string(), "passé, sans réponse".to_string())
    );
}

fn zone_resultat(zone: Zone, plus: (u32, u32, u32), moins: (u32, u32, u32)) -> ResultatZone {
    let c = |(variante, reference, pareil)| ComptesSens {
        variante,
        reference,
        pareil,
    };
    let (plus, moins) = (c(plus), c(moins));
    ResultatZone {
        zone,
        gain_db: 0.0,
        duels: plus.total() + moins.total(),
        pour_variante: plus.variante + moins.variante,
        pour_reference: plus.reference + moins.reference,
        pareil: plus.pareil + moins.pareil,
        confiance: Confiance::Indice,
        contradictoire: false,
        indice: None,
        plus,
        moins,
    }
}

#[test]
fn chaque_sens_est_dit_a_part_puis_la_confiance() {
    let mut z = zone_resultat(Zone::Graves, (2, 0, 0), (0, 2, 0));
    z.gain_db = 4.0;
    z.confiance = Confiance::Probable;
    assert_eq!(
        detail_zone(&z, false, FR),
        "Plus de graves : préféré 2 fois sur 2 · Moins de graves : la référence préférée 2 fois sur 2 · changement retenu : prise seule, cette comparaison avait au plus 1 chance sur 4 de tomber ainsi par hasard"
    );
    // Jamais de comptes mélangés des deux sens.
    assert!(!detail_zone(&z, false, FR).contains("confondus"));
}

#[test]
fn un_indice_se_dit_simplement_et_un_sens_non_teste_est_tu() {
    let mut z = zone_resultat(Zone::Air, (0, 0, 0), (1, 0, 1));
    z.indice = Some(Sens::Moins);
    assert_eq!(
        detail_zone(&z, false, FR),
        "Moins d’air : préféré 1 fois sur 1 (Pareil 1 fois) · un indice, à confirmer au prochain test"
    );
}

#[test]
fn les_autres_conclusions_restent_honnetes() {
    let z = zone_resultat(Zone::Mediums, (1, 1, 0), (0, 0, 0));
    assert_eq!(
        detail_zone(&z, false, FR),
        "Plus de médiums : avis partagés, préféré 1 fois sur 2 · rien ne change"
    );
    let z = zone_resultat(Zone::Aigus, (0, 0, 2), (0, 0, 0));
    assert_eq!(
        detail_zone(&z, false, FR),
        "Plus d’aigus : Pareil 2 fois · ton réglage est gardé"
    );
    let mut z = zone_resultat(Zone::Presence, (2, 0, 0), (2, 0, 0));
    z.contradictoire = true;
    assert_eq!(
        detail_zone(&z, false, FR),
        "Plus de présence : préféré 2 fois sur 2 · Moins de présence : préféré 2 fois sur 2 · préféré en plus comme en moins : rien ne change"
    );
    let z = zone_resultat(Zone::Graves, (2, 0, 0), (0, 0, 0));
    assert!(
        detail_zone(&z, true, FR).ends_with("non retenu pour ce test (voir les duels de contrôle)")
    );
}

#[test]
fn un_ancien_resultat_sans_sens_garde_un_detail_lisible() {
    let mut z = zone_resultat(Zone::Graves, (0, 0, 0), (0, 0, 0));
    z.duels = 3;
    z.pour_variante = 2;
    z.pour_reference = 1;
    assert_eq!(
        detail_zone(&z, false, FR),
        "deux sens confondus : variante 2 fois, référence 1 fois, Pareil 0 fois · rien ne change"
    );
}

#[test]
fn le_resume_accorde_le_singulier_et_le_pluriel() {
    assert_eq!(
        resume_zones(2, 3, false, FR),
        "2 zones changent, 3 restent comme avant."
    );
    assert_eq!(
        resume_zones(1, 1, false, FR),
        "1 zone change, 1 reste comme avant."
    );
    assert!(resume_zones(0, 4, false, FR).starts_with("Aucune zone"));
}

#[test]
fn le_resume_et_le_detail_existent_aussi_en_anglais() {
    assert_eq!(
        resume_zones(2, 3, false, EN),
        "2 zones change, 3 stay as before."
    );
    assert_eq!(
        resume_zones(1, 1, false, EN),
        "1 zone changes, 1 stays as before."
    );
    let mut z = zone_resultat(Zone::Graves, (2, 0, 0), (0, 2, 0));
    z.gain_db = 4.0;
    z.confiance = Confiance::Probable;
    assert_eq!(
        detail_zone(&z, false, EN),
        "More bass: preferred 2 times out of 2 · Less bass: reference preferred 2 times out of 2 · change kept: taken alone, this comparison had at most a 1 in 4 chance of coming out this way by luck"
    );
    let mut z = zone_resultat(Zone::Air, (1, 0, 0), (0, 0, 0));
    z.indice = Some(Sens::Plus);
    assert_eq!(
        detail_zone(&z, false, EN),
        "More air: preferred 1 time out of 1 · a hint, to confirm at the next test"
    );
}

#[test]
fn les_durees_et_les_plages_suivent_la_langue() {
    assert_eq!(
        duree_test(12, 20, EN),
        "12 duels · about 24 min · switches every 20 s"
    );
    assert_eq!(plage_zone(Zone::Graves, EN), "60–250 Hz");
    assert_eq!(suite(Cote::A, 1, EN), "A plays its 1st pass");
    assert_eq!(suite(Cote::B, 3, EN), "B plays its 3rd pass");
    assert_eq!(suite(Cote::A, 2, FR), "A joue son 2e passage");
}

#[test]
fn les_decibels_s_ecrivent_comme_l_interface() {
    assert_eq!(db(-6.0, FR), "−6 dB");
    assert_eq!(db(-4.2, FR), "−4,2 dB");
    assert_eq!(db(-4.2, EN), "−4.2 dB");
    assert_eq!(db(3.0, EN), "+3 dB");
    assert_eq!(db(0.04, FR), "0 dB");
}

#[test]
fn la_langue_du_systeme_est_francaise_seulement_si_le_systeme_l_est() {
    for fr in ["fr", "fr-FR", "fr_CA.UTF-8", "FR_fr", " fr-BE "] {
        assert_eq!(langue_depuis_locale(Some(fr)), Langue::Fr, "{fr}");
    }
    for autre in ["en-US", "en_GB.UTF-8", "de-DE", "es", "C", ""] {
        assert_eq!(langue_depuis_locale(Some(autre)), Langue::En, "{autre}");
    }
    assert_eq!(langue_depuis_locale(None), Langue::En);
}

#[test]
fn le_preampli_remplace_se_dit_dans_les_deux_langues() {
    let fr = annonce_import("X", "peace", &[], 0, Some((-6.0, -4.2)), FR);
    assert!(
        fr.contains("préampli du fichier (−6 dB) remplacé par le préampli automatique (−4,2 dB)")
    );
    let en = annonce_import("X", "peace", &[], 0, Some((-6.0, -4.2)), EN);
    assert!(
        en.contains("the file’s preamp (−6 dB) was replaced by the automatic preamp (−4.2 dB)"),
        "{en}"
    );
    assert_eq!(
        annonce_import("X", "peace", &[], 0, None, EN),
        "“X” imported from Peace."
    );
}

#[test]
fn les_erreurs_n_affichent_pas_de_francais_en_anglais() {
    let erreurs = [
        ErreurControleur::ProfilInconnu,
        ErreurControleur::AucunProfil,
        ErreurControleur::TestEnCours,
        ErreurControleur::BrouillonEnCours,
        ErreurControleur::MoteurAbsent,
        ErreurControleur::MoteurNonAutorise,
        ErreurControleur::ModeIndisponible,
        ErreurControleur::AutreProfilManquant,
        ErreurControleur::FormatInconnu,
        ErreurControleur::ImportWavelet,
        ErreurControleur::AucunResultat,
        ErreurControleur::Format(ErreurFormat::AucuneLigneUtilisable),
        ErreurControleur::Reprise(ErreurReprise::JournalVide),
        ErreurControleur::Reprise(ErreurReprise::Incoherent {
            entree: 4,
            raison: "second démarrage",
        }),
        ErreurControleur::Profil(ErreurProfil::VersionInconnue(3)),
        ErreurControleur::Profil(ErreurProfil::Invalide("aucune version".into())),
        ErreurControleur::Depot(ErreurDepot::IdentifiantInvalide("a b".into())),
        ErreurControleur::Moteur(ErreurMoteur::CourbeInvalide("préampli non fini".into())),
        ErreurControleur::Moteur(ErreurMoteur::DossierIntrouvable(PathBuf::from("/x"))),
        ErreurControleur::Moteur(ErreurMoteur::Io {
            action: "écriture de /x".into(),
            source: std::io::Error::other("disk full"),
        }),
        ErreurControleur::Fichier {
            action: ActionFichier::Lecture,
            chemin: PathBuf::from("/x.txt"),
            source: std::io::Error::other("boom"),
        },
    ];
    // Des mots que seul du français contient dans ces phrases.
    let francais = [
        " le ", " la ", " les ", " du ", " des ", " est ", "é", "è", "à ",
    ];
    for e in &erreurs {
        let fr = erreur(e, FR);
        let en = erreur(e, EN);
        assert_ne!(fr, en, "{e:?}");
        let trouve: Vec<_> = francais
            .iter()
            .filter(|m| en.to_lowercase().contains(**m))
            .collect();
        assert!(trouve.is_empty(), "français dans « {en} » : {trouve:?}");
    }
}

#[test]
fn chaque_langue_donne_des_phrases_non_vides_et_differentes() {
    use std::collections::HashSet;
    let controles = BilanControles {
        total: 2,
        repondus: 2,
        a_ou_b: 0,
        pareil: 2,
        penalise: false,
        rien_retenu: false,
    };
    let bilan = BilanDuels {
        duels: 4,
        pour_variante: 3,
        pour_reference: 1,
        pareil: 0,
        preference: Preference::Variante,
        confiance: Confiance::Probable,
    };
    let duel = duel_fini(Sujet::Controle, Reponse::A, Cote::A);
    let mut z = zone_resultat(Zone::Graves, (2, 0, 0), (0, 2, 0));
    z.gain_db = 4.0;
    z.confiance = Confiance::Probable;
    // Chaque phrase, dans chaque langue, pour toutes les langues connues.
    let phrases = |l: Langue| -> Vec<(&'static str, String)> {
        vec![
            ("durée", duree_test(12, 20, l)),
            ("mode", nom_mode("une-zone", Some(Zone::Graves), "", l)),
            ("suite", suite(Cote::A, 2, l)),
            ("journal caché", ligne_journal(&duel, false, l).0),
            ("journal révélé", ligne_journal(&duel, true, l).0),
            ("journal révélé détail", ligne_journal(&duel, true, l).1),
            ("résumé", resume_zones(2, 3, false, l)),
            ("résumé rien", resume_zones(0, 3, true, l)),
            ("autre profil", resume_autre_profil(&bilan, "Harman", l)),
            ("contrôles", bilan_controles(&controles, l)),
            ("vérification", verification(Some(Preference::Variante), l)),
            ("détail", detail_zone(&z, false, l)),
            (
                "confiance",
                explication_confiance(Confiance::Sur, l).to_string(),
            ),
            (
                "préférence",
                preference_appli(Preference::Variante, l).to_string(),
            ),
            ("origine", origine(&Origine::Manuel, l)),
            ("résumé profil", resume_profil(&Origine::Preset, l)),
            ("annonce version", annonce_version_enregistree(2, "P", l)),
            ("annonce profil", annonce_profil_active("P", l)),
            (
                "annonce import",
                annonce_import("P", "peace", &[("x", 1)], 1, None, l),
            ),
            ("annonce son", annonce_son_origine(None, l)),
            ("annonce export", annonce_export("/x", l)),
            ("annonce égaliseur", annonce_egaliseur(true, l).to_string()),
            ("annonce sans réponse", annonce_sans_reponse(l).to_string()),
            ("annonce pause", annonce_pause_erreur(l).to_string()),
            ("annonce sortie", annonce_nouvelle_sortie("S", l)),
            ("erreur", erreur(&ErreurControleur::TestEnCours, l)),
            ("zone", nom_zone(Zone::Graves, l).to_string()),
            ("profil par défaut", profil_par_defaut(l).to_string()),
            ("sortie", controle_sortie(Some("S"), l).titre),
            ("moteur", controle_moteur(0, false, None, l).detail),
            ("raccourcis", controle_raccourcis(&[], true, l).detail),
            ("bilan autotest", bilan_autotest(3, 2, 1, l)),
            ("raccourci F10", raccourcis(l)[4].1.to_string()),
            ("mode", modes(l)[0].description.to_string()),
            ("menu", menu_barre(l).0.to_string()),
        ]
    };
    let reference = phrases(Langue::Fr);
    for langue in Langue::TOUTES {
        let autres = phrases(langue);
        assert_eq!(reference.len(), autres.len());
        let mut vues = HashSet::new();
        for ((nom, fr), (_, texte)) in reference.iter().zip(&autres) {
            assert!(!texte.trim().is_empty(), "{langue:?} : « {nom} » est vide");
            if langue != Langue::Fr {
                assert_ne!(fr, texte, "{langue:?} : « {nom} » n'est pas traduit");
            }
            vues.insert(texte.clone());
        }
        assert!(vues.len() > reference.len() * 3 / 4);
    }
}

#[test]
fn les_modes_ont_les_memes_ids_dans_les_deux_langues() {
    let ids = |l| {
        modes(l)
            .into_iter()
            .map(|m| (m.id, m.disponible))
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(FR), ids(EN));
    assert_eq!(nom_preset("À plat", EN), "Flat");
    assert_eq!(nom_preset("À plat", FR), "À plat");
}
