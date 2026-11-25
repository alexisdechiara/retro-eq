//! Données d'exemple reprises de la maquette. `--demo` remplit l'interface avec,
//! pour travailler un écran ou le capturer sans moteur ni test en cours. Les phrases viennent
//! de `textes`, comme dans l'appli : la démo existe dans les deux langues.

use crate::pont::modeles_de_modes;
use crate::textes;
use crate::*;
use retro_core::{
    BilanControles, ComptesSens, Confiance, Cote, DuelFini, Langue, Origine, Preference, Reponse,
    ResultatZone, Sens, Sujet, Zone,
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

fn modele<T: Clone + 'static>(lignes: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(lignes))
}

fn textes(lignes: &[&str]) -> ModelRc<SharedString> {
    modele(lignes.iter().map(|t| SharedString::from(*t)).collect())
}

const COURBE_A: &str = "M 0 150 C 120 150 180 96 300 104 S 480 150 600 132 S 820 70 1000 96";
const COURBE_B: &str = "M 0 120 L 1000 120";

/// Un duel du journal, tel que l'interface le montre pendant le test : toutes les lignes se
/// ressemblent, seule la réponse change.
fn ligne_journal(numero: u32, reponse: Reponse, langue: Langue) -> LigneJournal {
    let duel = DuelFini {
        numero,
        sujet: Sujet::Controle,
        reponse: Some(reponse),
        reference: Cote::A,
        preference: Some(Preference::Aucune),
    };
    let (titre, detail) = textes::ligne_journal(&duel, false, langue);
    LigneJournal {
        titre: titre.into(),
        cote: textes::cote_reponse(duel.reponse),
        detail: detail.into(),
    }
}

/// Une zone du résultat, avec ses réponses sens par sens.
fn zone_resultat(
    zone: Zone,
    gain_db: f64,
    plus: (u32, u32, u32),
    moins: (u32, u32, u32),
    indice: Option<Sens>,
    langue: Langue,
) -> LigneZone {
    let comptes = |(variante, reference, pareil)| ComptesSens {
        variante,
        reference,
        pareil,
    };
    let (plus, moins) = (comptes(plus), comptes(moins));
    let confiance = if gain_db == 0.0 {
        Confiance::Indice
    } else {
        Confiance::Probable
    };
    let resultat = ResultatZone {
        zone,
        gain_db,
        duels: plus.total() + moins.total(),
        pour_variante: plus.variante + moins.variante,
        pour_reference: plus.reference + moins.reference,
        pareil: plus.pareil + moins.pareil,
        confiance,
        contradictoire: false,
        indice,
        plus,
        moins,
    };
    LigneZone {
        nom: textes::nom_zone(zone, langue).into(),
        plage: textes::plage_zone(zone, langue).into(),
        gain_db: gain_db as f32,
        change: resultat.est_changee(),
        confiance: textes::niveau_confiance(confiance),
        detail: textes::detail_zone(&resultat, false, langue).into(),
    }
}

pub fn remplir(fenetre: &FenetrePrincipale, langue: Langue) {
    let l = langue;
    let en = l == Langue::En;
    let appli = fenetre.global::<Appli>();
    appli.set_langue(l.code().into());
    fenetre.global::<Reglages>().set_langue(l.code().into());
    let nom_profil = "Mon profil 24/11";
    appli.set_profil_actif(nom_profil.into());
    appli.set_niveau_db(-18.0);
    appli.set_etat_moteur(2);
    appli.set_sortie_audio("INZONE H9 II – Game".into());

    let test = fenetre.global::<Test>();
    test.set_etat(EtatTest::Ecoute);
    test.set_duel(7);
    test.set_total(12);
    test.set_cote_actif(1);
    test.set_passages_a(2);
    test.set_passages_b(2);
    test.set_avancement(0.55);
    test.set_compte_a_rebours("0:12".into());
    test.set_suite(textes::suite(Cote::A, 3, l).into());
    test.set_source(textes::source_pc(l).into());
    test.set_appli("Valorant".into());
    test.set_mode(textes::nom_mode("duels-en-jeu", None, "", l).into());
    test.set_reference(
        if en {
            "ROG Delta precise"
        } else {
            "ROG Delta précis"
        }
        .into(),
    );
    test.set_duree(textes::duree_test(12, 20, l).into());
    test.set_zones(textes(
        &textes::noms_zones(l)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    ));
    test.set_profils(textes(&if en {
        [
            "Everyday",
            "Voice and Discord",
            "Harman (measured)",
            "ROG Delta precise",
        ]
    } else {
        [
            "Quotidien",
            "Voix et Discord",
            "Harman (mesuré)",
            "ROG Delta précis",
        ]
    }));
    // Du plus récent au plus ancien, comme pendant un vrai test.
    test.set_journal(modele(vec![
        ligne_journal(6, Reponse::B, l),
        ligne_journal(5, Reponse::A, l),
        ligne_journal(4, Reponse::Pareil, l),
        ligne_journal(3, Reponse::B, l),
        ligne_journal(2, Reponse::A, l),
        ligne_journal(1, Reponse::A, l),
    ]));
    test.set_modes(modeles_de_modes(l));

    let resultat = fenetre.global::<Resultat>();
    resultat.set_disponible(true);
    resultat.set_titre(textes::profil_et_version(nom_profil, 3, l).into());
    resultat.set_confiance(1);
    resultat.set_resume(textes::resume_zones(2, 4, false, l).into());
    resultat.set_controles(
        textes::bilan_controles(
            &BilanControles {
                total: 2,
                repondus: 2,
                a_ou_b: 0,
                pareil: 2,
                penalise: false,
                rien_retenu: false,
            },
            l,
        )
        .into(),
    );
    resultat.set_verification(textes::verification(Some(Preference::Variante), l).into());
    resultat.set_courbe_reference(COURBE_B.into());
    resultat.set_courbe_nouvelle(COURBE_A.into());
    resultat.set_preampli_db(-4.0);
    resultat.set_zones(modele(vec![
        zone_resultat(Zone::Graves, -4.0, (0, 2, 0), (2, 0, 0), None, l),
        zone_resultat(Zone::BasMediums, 0.0, (0, 0, 1), (0, 0, 1), None, l),
        zone_resultat(Zone::Mediums, 0.0, (1, 1, 0), (0, 0, 0), None, l),
        zone_resultat(Zone::Presence, 4.0, (2, 0, 0), (0, 2, 0), None, l),
        zone_resultat(Zone::Aigus, 0.0, (0, 2, 0), (0, 0, 0), None, l),
        zone_resultat(Zone::Air, 0.0, (1, 0, 0), (0, 0, 0), Some(Sens::Plus), l),
    ]));
    resultat.set_applis(modele(vec![
        LigneAppli {
            nom: "Valorant".into(),
            duels: 8,
            preference: textes::preference_appli(Preference::Variante, l).into(),
        },
        LigneAppli {
            nom: "Discord".into(),
            duels: 1,
            preference: textes::preference_appli(Preference::Aucune, l).into(),
        },
    ]));

    let profils = fenetre.global::<Profils>();
    let profil = |id: &str, nom: &str, actif: bool, versions: i32, date: &str, origine: Origine| {
        LigneProfil {
            id: id.into(),
            nom: nom.into(),
            actif,
            versions,
            date: date.into(),
            resume: textes::resume_profil(&origine, l).into(),
            courbe: COURBE_A.into(),
        }
    };
    let importe = |format: &str| Origine::Import {
        format: format.into(),
    };
    profils.set_liste(modele(vec![
        profil(
            "mon-profil",
            nom_profil,
            true,
            3,
            "24/11",
            Origine::Test {
                test_id: "t1".into(),
            },
        ),
        profil(
            "quotidien",
            if en { "Everyday" } else { "Quotidien" },
            false,
            1,
            "12/10",
            importe("peace"),
        ),
        profil(
            "voix",
            if en {
                "Voice and Discord"
            } else {
                "Voix et Discord"
            },
            false,
            1,
            "12/10",
            importe("peace"),
        ),
        profil(
            "harman",
            if en {
                "Harman (measured)"
            } else {
                "Harman (mesuré)"
            },
            false,
            1,
            "02/10",
            importe("autoeq"),
        ),
    ]));
    profils.set_choisi("mon-profil".into());
    profils.set_presets(textes(&[&textes::nom_preset("À plat", l)]));
    let version = |numero: i32, date: &str, origine: Origine, active: bool| LigneVersion {
        numero,
        date: date.into(),
        origine: textes::origine(&origine, l).into(),
        active,
        courbe: COURBE_A.into(),
    };
    profils.set_versions(modele(vec![
        version(
            3,
            "24/11 21:40",
            Origine::Test {
                test_id: "t1".into(),
            },
            true,
        ),
        version(2, "20/11 18:05", Origine::Manuel, false),
        version(1, "12/10 10:12", importe("peace"), false),
    ]));

    let egaliseur = fenetre.global::<Egaliseur>();
    egaliseur.set_gains(modele(vec![
        3.0, 2.0, -1.5, -2.0, 0.0, 1.0, 3.5, 2.0, -1.0, 1.5,
    ]));
    egaliseur.set_preampli_db(-3.5);
    egaliseur.set_courbe(COURBE_A.into());
    egaliseur.set_profil("Mon profil 24/11".into());
    egaliseur.set_filtres_supplementaires(2);

    let visualiseur = fenetre.global::<Visualiseur>();
    visualiseur.set_niveau_db(-18.0);
    visualiseur.set_crete_db(-6.0);
    visualiseur.set_volume_lufs(-21.0);
    visualiseur.set_capture_active(true);
    visualiseur.set_source("INZONE H9 II – Game".into());
    visualiseur.set_courbe(COURBE_A.into());
    visualiseur.set_bandes(modele(
        (0..32)
            .map(|i| 0.25 + 0.6 * ((i as f32 * 0.37).sin().abs()) * (1.0 - i as f32 / 48.0))
            .collect(),
    ));

    let diagnostic = fenetre.global::<Diagnostic>();
    diagnostic.set_bilan(textes::bilan_autotest(2, 3, 1, l).into());
    diagnostic.set_dossier_moteur("C:\\Program Files\\EqualizerAPO\\config".into());
    diagnostic.set_dossier_donnees("C:\\Users\\Alexis\\AppData\\Roaming\\Retro EQ".into());
    let controle = |texte: textes::TexteControle, etat: EtatControle, action: bool| LigneControle {
        titre: texte.titre.into(),
        detail: texte.detail.into(),
        etat,
        action: if action { texte.action } else { String::new() }.into(),
    };
    let a_verifier = textes::controles_a_verifier_soi_meme(l);
    let mut a_verifier = a_verifier.into_iter();
    let (inzone, spatial, ameliorations) =
        match (a_verifier.next(), a_verifier.next(), a_verifier.next()) {
            (Some(a), Some(b), Some(c)) => (a, b, c),
            _ => return,
        };
    diagnostic.set_controles(modele(vec![
        controle(
            textes::controle_sortie(Some("INZONE H9 II – Game"), l),
            EtatControle::Ok,
            false,
        ),
        controle(
            textes::controle_moteur(2, true, Some(true), l),
            EtatControle::Ok,
            false,
        ),
        controle(inzone, EtatControle::Attention, false),
        controle(spatial, EtatControle::Attention, true),
        controle(ameliorations, EtatControle::Attention, true),
        controle(
            textes::controle_raccourcis(&["F10".to_string()], true, l),
            EtatControle::Erreur,
            false,
        ),
    ]));

    let reglages = fenetre.global::<Reglages>();
    reglages.set_sorties(textes(&["INZONE H9 II – Game", "INZONE H9 II – Chat"]));
    reglages.set_sortie("INZONE H9 II – Game".into());
    reglages.set_raccourcis(modele(
        textes::raccourcis(l)
            .into_iter()
            .enumerate()
            .map(|(i, (touches, action))| LigneRaccourci {
                touches: touches.into(),
                action: action.into(),
                // F10 est déjà pris par une autre appli : de quoi voir l'état « conflit ».
                conflit: i == 4,
            })
            .collect(),
    ));
}
