//! Données d'exemple reprises de la maquette. `--demo` remplit l'interface avec,
//! pour travailler un écran ou le capturer sans moteur ni test en cours.

use crate::*;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

fn modele<T: Clone + 'static>(lignes: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(lignes))
}

fn textes(lignes: &[&str]) -> ModelRc<SharedString> {
    modele(lignes.iter().map(|t| SharedString::from(*t)).collect())
}

const COURBE_A: &str = "M 0 150 C 120 150 180 96 300 104 S 480 150 600 132 S 820 70 1000 96";
const COURBE_B: &str = "M 0 120 L 1000 120";

pub fn remplir(fenetre: &FenetrePrincipale) {
    let appli = fenetre.global::<Appli>();
    appli.set_profil_actif("Mon profil 24/11".into());
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
    test.set_suite("A joue son 3e passage, puis B".into());
    test.set_source("Ce qui joue sur le PC".into());
    test.set_appli("Valorant".into());
    test.set_mode("Duels en jeu".into());
    test.set_reference("ROG Delta précis".into());
    test.set_duree("12 duels · environ 25 min · bascule toutes les 20 s".into());
    test.set_zones(textes(&["Sub-graves", "Graves", "Bas-médiums", "Médiums", "Présence", "Aigus", "Air"]));
    test.set_profils(textes(&["Quotidien", "Voix et Discord", "Harman (mesuré)", "ROG Delta précis"]));
    test.set_journal(modele(vec![
        LigneJournal { titre: "Duel 6 · graves 125 Hz".into(), cote: 2, detail: "était la variante · préférée".into() },
        LigneJournal { titre: "Duel 5 · air 16 kHz".into(), cote: 1, detail: "était la référence · préférée".into() },
        LigneJournal { titre: "Duel 4 · aigus 8 kHz".into(), cote: 0, detail: "Pareil · référence gardée".into() },
    ]));
    let mode = |id: &str, groupe: &str, nom: &str, description: &str, source: &str, duels: &str, duree: &str, disponible: bool| LigneMode {
        id: id.into(), groupe: groupe.into(), nom: nom.into(), description: description.into(), source: source.into(),
        duels: duels.into(), duree: duree.into(), disponible,
    };
    test.set_modes(modele(vec![
        mode("duels-en-jeu", "Trouver ma courbe", "Duels en jeu", "Le son bascule tout seul pendant que tu joues, regardes ou parles sur Discord.", "Ce qui joue sur le PC", "12", "25 min", true),
        mode("parcours", "Trouver ma courbe", "Parcours complet", "Un son préparé par zone, des graves profonds jusqu’à l’air. Le plus complet.", "Échantillons préparés", "21", "35 min", false),
        mode("musique", "Trouver ma courbe", "Sur une musique", "Ton morceau préféré en boucle, sur le passage que tu choisis.", "Ton fichier audio", "12", "15 min", false),
        mode("echantillons", "Affiner", "Échantillons ciblés", "Un son choisi pour une seule zone : voix, pas, cymbales, basse.", "Un échantillon", "6", "8 min", false),
        mode("une-zone", "Affiner", "Une seule zone", "Tu sais ce qui cloche ? Les duels ne testent que les aigus, ou que les graves.", "Ce qui joue sur le PC", "6", "12 min", true),
        mode("profil-contre-profil", "Affiner", "Profil contre profil", "Deux profils face à face, par exemple Harman contre ton profil du jour.", "Ce qui joue sur le PC", "6", "12 min", true),
        mode("abx", "Vérifier", "Test à l’aveugle", "A, B, puis X au hasard : sais-tu vraiment reconnaître ton profil ?", "Au choix", "10", "10 min", false),
        mode("audition", "Vérifier", "Test d’audition", "Mesure ce que tes oreilles entendent, zone par zone, pour adapter la courbe.", "Sons purs", "—", "—", false),
    ]));

    let resultat = fenetre.global::<Resultat>();
    resultat.set_disponible(true);
    resultat.set_titre("Mon profil 24/11 · version 3".into());
    resultat.set_confiance(1);
    resultat.set_resume("2 zones changent, 3 restent comme avant.".into());
    resultat.set_controles("2 duels de contrôle sur 2 jugés « Pareil » : tes réponses sont fiables.".into());
    resultat.set_verification("Nouvelle version préférée à la référence.".into());
    resultat.set_courbe_reference(COURBE_B.into());
    resultat.set_courbe_nouvelle(COURBE_A.into());
    resultat.set_preampli_db(-4.0);
    let zone = |nom: &str, plage: &str, gain: f32, change: bool, confiance: i32, detail: &str| LigneZone {
        nom: nom.into(), plage: plage.into(), gain_db: gain, change, confiance, detail: detail.into(),
    };
    resultat.set_zones(modele(vec![
        zone("Graves", "60–250 Hz", -4.0, true, 1, "Variante préférée 2 fois sur 2"),
        zone("Bas-médiums", "250–500 Hz", 0.0, false, 0, "Pareil 2 fois · ton réglage est gardé"),
        zone("Médiums", "500 Hz–2 kHz", 0.0, false, 0, "Avis partagés"),
        zone("Présence", "2–5 kHz", 4.0, true, 1, "Variante préférée 2 fois sur 2"),
        zone("Aigus", "5–10 kHz", 0.0, false, 0, "Référence préférée"),
    ]));
    resultat.set_applis(modele(vec![
        LigneAppli { nom: "Valorant".into(), duels: 8, preference: "plus de présence".into() },
        LigneAppli { nom: "Discord".into(), duels: 2, preference: "pas de préférence nette".into() },
    ]));

    let profils = fenetre.global::<Profils>();
    let profil = |id: &str, nom: &str, actif: bool, versions: i32, date: &str, resume: &str| LigneProfil {
        id: id.into(), nom: nom.into(), actif, versions, date: date.into(), resume: resume.into(), courbe: COURBE_A.into(),
    };
    profils.set_liste(modele(vec![
        profil("mon-profil", "Mon profil 24/11", true, 3, "24/11", "Issu d’un test · confiance Probable"),
        profil("quotidien", "Quotidien", false, 1, "12/10", "Importé de Peace"),
        profil("voix", "Voix et Discord", false, 1, "12/10", "Importé de Peace"),
        profil("harman", "Harman (mesuré)", false, 1, "02/10", "Importé d’AutoEQ"),
    ]));
    profils.set_choisi("mon-profil".into());
    profils.set_presets(textes(&["À plat"]));
    let version = |numero: i32, date: &str, origine: &str, active: bool| LigneVersion {
        numero, date: date.into(), origine: origine.into(), active, courbe: COURBE_A.into(),
    };
    profils.set_versions(modele(vec![
        version(3, "24/11 21:40", "Test · Duels en jeu · 12 duels", true),
        version(2, "20/11 18:05", "Test · Une seule zone", false),
        version(1, "12/10 10:12", "Import Peace", false),
    ]));

    let egaliseur = fenetre.global::<Egaliseur>();
    egaliseur.set_gains(modele(vec![3.0, 2.0, -1.5, -2.0, 0.0, 1.0, 3.5, 2.0, -1.0, 1.5]));
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
    visualiseur.set_bandes(modele((0..32).map(|i| 0.25 + 0.6 * ((i as f32 * 0.37).sin().abs()) * (1.0 - i as f32 / 48.0)).collect()));

    let diagnostic = fenetre.global::<Diagnostic>();
    diagnostic.set_bilan("5 contrôles sur 6 sont bons.".into());
    diagnostic.set_dossier_moteur("C:\\Program Files\\EqualizerAPO\\config".into());
    diagnostic.set_dossier_donnees("C:\\Users\\Alexis\\AppData\\Roaming\\Retro EQ".into());
    let controle = |titre: &str, detail: &str, etat: EtatControle, action: &str| LigneControle {
        titre: titre.into(), detail: detail.into(), etat, action: action.into(),
    };
    diagnostic.set_controles(modele(vec![
        controle("INZONE H9 II branché", "Sortie « INZONE H9 II – Game », 48 kHz", EtatControle::Ok, ""),
        controle("Moteur audio actif", "Equalizer APO détecté et installé sur cette sortie", EtatControle::Ok, ""),
        controle("Égaliseur d’INZONE Hub à plat", "L’appli ne peut pas le lire : vérifie-le dans INZONE Hub.", EtatControle::Attention, "Comment faire"),
        controle("Son spatial Windows coupé", "Ni Windows Sonic ni Dolby Atmos", EtatControle::Ok, ""),
        controle("Améliorations audio Windows coupées", "L’égalisation du volume est active sur cette sortie.", EtatControle::Erreur, "Ouvrir les réglages"),
        controle("Raccourcis libres", "F6 à F10 et Ctrl+Alt+Entrée", EtatControle::Ok, ""),
    ]));

    let reglages = fenetre.global::<Reglages>();
    reglages.set_sorties(textes(&["INZONE H9 II – Game", "INZONE H9 II – Chat"]));
    reglages.set_sortie("INZONE H9 II – Game".into());
    let raccourci = |touches: &str, action: &str, conflit: bool| LigneRaccourci { touches: touches.into(), action: action.into(), conflit };
    reglages.set_raccourcis(modele(vec![
        raccourci("F6", "Je préfère A (maintenir : réécouter A)", false),
        raccourci("F7", "Pareil", false),
        raccourci("F8", "Je préfère B (maintenir : réécouter B)", false),
        raccourci("F9", "Plus tard", false),
        raccourci("F10", "Annuler ma dernière réponse", true),
        raccourci("Ctrl+Alt+Entrée", "Démarrer ou reprendre un test", false),
        raccourci("Ctrl+Alt+1 à 4", "Choisir un profil", false),
        raccourci("Ctrl+Alt+0", "Couper l’égaliseur", false),
    ]));
}
