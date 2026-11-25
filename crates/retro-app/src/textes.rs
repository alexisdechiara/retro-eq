//! Toutes les phrases que le code Rust fabrique pour l'interface, au même endroit.
//!
//! Les textes fixes des écrans sont dans les fichiers `.slint`, derrière `@tr(...)`. Ceux
//! d'ici dépendent de chiffres (« 2 zones changent »), et ne peuvent donc pas y être écrits
//! d'avance. Ce sont des fonctions pures : des nombres ou des énumérations en entrée, une
//! phrase en sortie. Le jour où la traduction de ces phrases est branchée, c'est ce seul
//! fichier qu'il faut reprendre.

use retro_core::formats::RaisonIgnoree;
use retro_core::{
    BilanControles, BilanDuels, Confiance, Cote, DuelFini, Origine, Preference, Reponse,
    ResultatZone, Sens, Sujet, Zone,
};

/// Un mode de test tel que l'écran « Tester » le présente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexteMode {
    pub id: &'static str,
    pub groupe: &'static str,
    pub nom: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub duels: &'static str,
    pub duree: &'static str,
    /// Faux pour les modes prévus après la v1 (ADR-0011) : affichés avec « Bientôt ».
    pub disponible: bool,
}

pub const SOURCE_PC: &str = "Ce qui joue sur le PC";

/// Les cassettes de l'écran « Tester ». Trois modes en v1, les autres sont annoncés.
pub fn modes() -> Vec<TexteMode> {
    let m = |id, groupe, nom, description, source, duels, duree, disponible| TexteMode {
        id,
        groupe,
        nom,
        description,
        source,
        duels,
        duree,
        disponible,
    };
    vec![
        m(
            "duels-en-jeu",
            "Trouver ma courbe",
            "Duels en jeu",
            "Le son bascule tout seul pendant que tu joues, regardes ou parles sur Discord.",
            SOURCE_PC,
            "12",
            "25 min",
            true,
        ),
        m(
            "parcours",
            "Trouver ma courbe",
            "Parcours complet",
            "Un son préparé par zone, des graves profonds jusqu’à l’air. Le plus complet.",
            "Échantillons préparés",
            "21",
            "35 min",
            false,
        ),
        m(
            "musique",
            "Trouver ma courbe",
            "Sur une musique",
            "Ton morceau préféré en boucle, sur le passage que tu choisis.",
            "Ton fichier audio",
            "12",
            "15 min",
            false,
        ),
        m(
            "echantillons",
            "Affiner",
            "Échantillons ciblés",
            "Un son choisi pour une seule zone : voix, pas, cymbales, basse.",
            "Un échantillon",
            "6",
            "8 min",
            false,
        ),
        m(
            "une-zone",
            "Affiner",
            "Une seule zone",
            "Tu sais ce qui cloche ? Les duels ne testent que les aigus, ou que les graves.",
            SOURCE_PC,
            "6",
            "12 min",
            true,
        ),
        m(
            "profil-contre-profil",
            "Affiner",
            "Profil contre profil",
            "Deux profils face à face, par exemple Harman contre ton profil du jour.",
            SOURCE_PC,
            "6",
            "12 min",
            true,
        ),
        m(
            "abx",
            "Vérifier",
            "Test à l’aveugle",
            "A, B, puis X au hasard : sais-tu vraiment reconnaître ton profil ?",
            "Au choix",
            "10",
            "10 min",
            false,
        ),
        m(
            "audition",
            "Vérifier",
            "Test d’audition",
            "Mesure ce que tes oreilles entendent, zone par zone, pour adapter la courbe.",
            "Sons purs",
            "—",
            "—",
            false,
        ),
    ]
}

/// « 1 duel », « 12 duels ».
fn pluriel(n: u32, singulier: &str, pluriel: &str) -> String {
    if n <= 1 {
        format!("{n} {singulier}")
    } else {
        format!("{n} {pluriel}")
    }
}

/// Un gain en dB avec son signe : « +4 dB », « −2,5 dB », « 0 dB ».
pub fn db(valeur: f64) -> String {
    let arrondi = (valeur * 10.0).round() / 10.0;
    let nombre = if arrondi.fract() == 0.0 {
        format!("{}", arrondi.abs() as i64)
    } else {
        format!("{:.1}", arrondi.abs()).replace('.', ",")
    };
    if arrondi > 0.0 {
        format!("+{nombre} dB")
    } else if arrondi < 0.0 {
        format!("−{nombre} dB")
    } else {
        "0 dB".to_string()
    }
}

/// Une fréquence lisible : « 250 Hz », « 2 kHz ».
fn frequence(hz: f64) -> (String, &'static str) {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        let n = if k.fract() == 0.0 {
            format!("{}", k as i64)
        } else {
            format!("{k:.1}").replace('.', ",")
        };
        (n, "kHz")
    } else {
        (format!("{}", hz.round() as i64), "Hz")
    }
}

/// La plage d'une zone : « 60–250 Hz », « 500 Hz–2 kHz ».
pub fn plage_zone(zone: Zone) -> String {
    let (bas, haut) = zone.bornes_hz();
    let ((b, ub), (h, uh)) = (frequence(bas), frequence(haut));
    if ub == uh {
        format!("{b}–{h} {uh}")
    } else {
        format!("{b} {ub}–{h} {uh}")
    }
}

/// Les noms des 7 zones, pour le choix « Zone à tester ».
pub fn noms_zones() -> Vec<String> {
    Zone::TOUTES.iter().map(|z| z.nom().to_string()).collect()
}

/// Résumé de la durée d'un test : « 12 duels · environ 24 min · bascule toutes les 20 s ».
pub fn duree_test(duels: u32, intervalle_s: u32) -> String {
    let minutes = (duels * 6 * intervalle_s).div_ceil(60);
    let bascule = if intervalle_s >= 60 && intervalle_s % 60 == 0 {
        format!("{} min", intervalle_s / 60)
    } else {
        format!("{intervalle_s} s")
    };
    format!(
        "{} · environ {minutes} min · bascule toutes les {bascule}",
        pluriel(duels, "duel", "duels")
    )
}

/// Le nom du mode choisi, tel qu'il apparaît dans le résumé d'un nouveau test.
pub fn nom_mode(id: &str, zone: Option<Zone>, autre_profil: &str) -> String {
    match id {
        "une-zone" => match zone {
            Some(z) => format!("Une seule zone · {} ({})", z.nom(), plage_zone(z)),
            None => "Une seule zone".to_string(),
        },
        "profil-contre-profil" if !autre_profil.is_empty() => {
            format!("Profil contre profil · {autre_profil}")
        }
        "profil-contre-profil" => "Profil contre profil".to_string(),
        _ => "Duels en jeu · une zone change à chaque duel".to_string(),
    }
}

/// Temps restant avant la bascule : « 0:12 ». Arrondi à la seconde supérieure, pour ne pas
/// afficher « 0:00 » pendant la dernière seconde.
pub fn compte_a_rebours(ms: u64) -> String {
    let s = ms.div_ceil(1000);
    format!("{}:{:02}", s / 60, s % 60)
}

/// « A joue son 2e passage ». On ne dit rien de la suite : seul l'ordre déjà joué est connu
/// de l'interface, c'est voulu (test à l'aveugle).
pub fn suite(cote: Cote, numero: u8) -> String {
    let lettre = lettre(cote);
    let rang = if numero <= 1 {
        "1er".to_string()
    } else {
        format!("{numero}e")
    };
    format!("{lettre} joue son {rang} passage")
}

pub fn lettre(cote: Cote) -> &'static str {
    match cote {
        Cote::A => "A",
        Cote::B => "B",
    }
}

/// Une ligne du journal d'un test en cours : titre et détail.
///
/// `revele_controles` ne devient vrai qu'une fois le test fini. Avant, un duel de contrôle
/// est montré comme une simple réponse enregistrée : dire lequel était un contrôle
/// apprendrait à l'utilisateur à quel rythme ils reviennent.
pub fn ligne_journal(duel: &DuelFini, revele_controles: bool) -> (String, String) {
    let n = duel.numero;
    let Some(preference) = duel.preference else {
        return (format!("Duel {n}"), "passé, sans réponse".to_string());
    };
    let titre = match duel.sujet {
        Sujet::Controle if !revele_controles => {
            return (format!("Duel {n}"), "réponse enregistrée".to_string());
        }
        Sujet::Controle => format!("Duel {n} · contrôle (deux versions identiques)"),
        Sujet::Zone { zone, sens } => format!(
            "Duel {n} · {} {}",
            zone.nom().to_lowercase(),
            match sens {
                Sens::Plus => "en plus",
                Sens::Moins => "en moins",
            }
        ),
        Sujet::AutreProfil => format!("Duel {n} · autre profil"),
        Sujet::Verification => format!("Duel {n} · vérification"),
    };
    let detail = match (duel.sujet, preference) {
        (Sujet::Controle, Preference::Aucune) => "Pareil · bonne réponse",
        (Sujet::Controle, _) => "une différence entendue, alors qu’il n’y en avait pas",
        (_, Preference::Aucune) => "Pareil · référence gardée",
        (_, Preference::Reference) => "était la référence · préférée",
        (Sujet::AutreProfil, Preference::Variante) => "était l’autre profil · préféré",
        (Sujet::Verification, Preference::Variante) => "était la nouvelle version · préférée",
        (_, Preference::Variante) => "était la variante · préférée",
    };
    (titre, detail.to_string())
}

/// Le côté à montrer dans la pastille d'une ligne du journal : 1 = A, 2 = B, 0 = aucun.
pub fn cote_reponse(reponse: Option<Reponse>) -> i32 {
    match reponse {
        Some(Reponse::A) => 1,
        Some(Reponse::B) => 2,
        _ => 0,
    }
}

/// 0 = Indice, 1 = Probable, 2 = Sûr : l'échelle de l'interface.
pub fn niveau_confiance(confiance: Confiance) -> i32 {
    match confiance {
        Confiance::Indice => 0,
        Confiance::Probable => 1,
        Confiance::Sur => 2,
    }
}

pub fn nom_confiance(confiance: Confiance) -> &'static str {
    match confiance {
        Confiance::Indice => "Indice",
        Confiance::Probable => "Probable",
        Confiance::Sur => "Sûr",
    }
}

/// Le résumé d'un résultat par zones : « 2 zones changent, 3 restent comme avant. »
pub fn resume_zones(changees: u32, inchangees: u32, rien_retenu: bool) -> String {
    if rien_retenu {
        return "Rien n’est retenu de ce test : tu as entendu des différences entre des versions identiques. Ton réglage est gardé tel quel.".to_string();
    }
    let reste = match inchangees {
        0 => String::new(),
        1 => ", 1 reste comme avant".to_string(),
        n => format!(", {n} restent comme avant"),
    };
    match changees {
        0 => "Aucune zone ne change : ton réglage est gardé tel quel.".to_string(),
        1 => format!("1 zone change{reste}."),
        n => format!("{n} zones changent{reste}."),
    }
}

/// Le résumé d'un test « profil contre profil ».
pub fn resume_autre_profil(bilan: &BilanDuels, autre: &str) -> String {
    let departages = bilan.pour_variante + bilan.pour_reference;
    match bilan.preference {
        Preference::Variante => format!(
            "« {autre} » préféré {} fois sur {departages}.",
            bilan.pour_variante
        ),
        Preference::Reference => format!(
            "Ta référence préférée {} fois sur {departages} face à « {autre} ».",
            bilan.pour_reference
        ),
        Preference::Aucune => format!("Pas de préférence nette entre ta référence et « {autre} »."),
    }
}

/// Ce que les duels de contrôle disent des réponses. Vide s'il n'y en a pas eu.
pub fn bilan_controles(c: &BilanControles) -> String {
    if c.repondus == 0 {
        return String::new();
    }
    let sur = pluriel(c.repondus, "duel de contrôle", "duels de contrôle");
    if c.rien_retenu || c.penalise {
        format!(
            "Sur {sur} (deux versions identiques), tu as entendu une différence {} fois. Tes réponses ne distinguent donc pas deux versions identiques : aucun changement n’est retenu pour ce test.",
            c.a_ou_b
        )
    } else {
        format!(
            "Sur {sur} (deux versions identiques), {} jugés « Pareil » : tes réponses se tiennent.",
            c.pareil
        )
    }
}

/// Ce que le duel de vérification a donné. Vide s'il n'a pas eu lieu.
pub fn verification(preference: Option<Preference>) -> String {
    match preference {
        None => String::new(),
        Some(Preference::Variante) => {
            "Duel de vérification : nouvelle version préférée à la référence.".to_string()
        }
        Some(Preference::Reference) => {
            "Duel de vérification : tu as préféré ta référence à la nouvelle version. La nouvelle version est gardée dans le profil, mais rien n’indique qu’elle te plaît davantage."
                .to_string()
        }
        Some(Preference::Aucune) => {
            "Duel de vérification : pas de différence entendue entre la référence et la nouvelle version."
                .to_string()
        }
    }
}

/// Le détail d'une zone du résultat : les comptes tels quels, sans précision inventée.
pub fn detail_zone(z: &ResultatZone, rien_retenu: bool) -> String {
    let comptes = format!(
        "variante {} fois, référence {} fois, Pareil {} fois",
        z.pour_variante, z.pour_reference, z.pareil
    );
    if rien_retenu {
        format!("Non retenu pour ce test (voir les duels de contrôle) · {comptes}")
    } else if z.contradictoire {
        format!(
            "Réponses contradictoires (préférée en plus et en moins) : rien ne change · {comptes}"
        )
    } else if z.est_changee() {
        format!(
            "Changement retenu · {comptes} · {}",
            explication_confiance(z.confiance)
        )
    } else if let Some(sens) = z.indice {
        format!(
            "Un indice seulement ({} de {}, préféré une seule fois) : rien ne change, ce sera rejoué en premier au prochain test · {comptes}",
            match sens {
                Sens::Plus => "plus",
                Sens::Moins => "moins",
            },
            z.zone.nom().to_lowercase()
        )
    } else if z.pour_variante == 0 && z.pour_reference == 0 {
        format!("Pareil {} fois · ton réglage est gardé", z.pareil)
    } else if z.pour_reference > z.pour_variante {
        format!("Référence préférée · {comptes}")
    } else {
        format!("Avis partagés : rien ne change · {comptes}")
    }
}

/// Ce qu'un niveau de confiance veut dire, dans les termes de l'ADR-0005 : il décrit une
/// comparaison prise seule. Il ne dit pas « ce changement est probablement réel », parce
/// qu'un test fait plusieurs comparaisons à la fois.
pub fn explication_confiance(confiance: Confiance) -> &'static str {
    match confiance {
        Confiance::Sur => {
            "prise seule, cette comparaison avait moins de 6 % de chances de tomber ainsi par hasard"
        }
        Confiance::Probable => {
            "prise seule, cette comparaison avait au plus 1 chance sur 4 de tomber ainsi par hasard"
        }
        Confiance::Indice => "prise seule, cette comparaison peut très bien être due au hasard",
    }
}

/// La préférence relevée dans une appli.
pub fn preference_appli(preference: Preference) -> &'static str {
    match preference {
        Preference::Variante => "les changements proposés ont plu",
        Preference::Reference => "la référence a été préférée",
        Preference::Aucune => "pas de préférence nette",
    }
}

/// D'où vient une version, pour la liste des versions d'un profil.
pub fn origine(origine: &Origine) -> String {
    match origine {
        Origine::Preset => "Preset de départ".to_string(),
        Origine::Import { format } => format!("Import {}", nom_format(format)),
        Origine::Manuel => "Réglée à la main dans l’égaliseur".to_string(),
        Origine::Test { .. } => "Issue d’un test".to_string(),
    }
}

/// Le résumé d'un profil sur sa cassette.
pub fn resume_profil(origine_active: &Origine) -> String {
    match origine_active {
        Origine::Preset => "Parti d’un preset".to_string(),
        Origine::Import { format } => format!("Importé de {}", nom_format(format)),
        Origine::Manuel => "Réglé à la main".to_string(),
        Origine::Test { .. } => "Issu d’un test".to_string(),
    }
}

/// Le nom lisible d'un format de fichier (identifiant interne ou identifiant de l'interface).
pub fn nom_format(format: &str) -> &'static str {
    match format {
        "peace" => "Peace",
        "apo" | "equalizer_apo" => "Equalizer APO",
        "autoeq" => "AutoEQ",
        "wavelet" => "Wavelet",
        _ => "fichier",
    }
}

// ----- Annonces (le message court en bas de la fenêtre) -----

pub fn annonce_version_enregistree(numero: u32, profil: &str) -> String {
    format!("Version {numero} de « {profil} » enregistrée et activée.")
}

pub fn annonce_profil_active(profil: &str) -> String {
    format!("« {profil} » est maintenant ton profil actif.")
}

pub fn annonce_version_activee(numero: u32, profil: &str) -> String {
    format!("Version {numero} de « {profil} » activée.")
}

pub fn annonce_profil_cree(profil: &str) -> String {
    format!("Profil « {profil} » créé.")
}

/// Pourquoi une ligne d'un fichier importé a été laissée de côté, en quelques mots.
pub fn raison_ignoree(raison: &RaisonIgnoree) -> &'static str {
    match raison {
        RaisonIgnoree::TypeFiltreNonGere(_) => "type de filtre non géré",
        RaisonIgnoree::PenteNonGeree => "plateau réglé par une pente",
        RaisonIgnoree::CanalParticulier => "réglage propre à un canal",
        RaisonIgnoree::PeripheriqueParticulier => "réglage propre à un autre périphérique",
        RaisonIgnoree::HautParleurParticulier => "réglage propre à un haut-parleur",
        RaisonIgnoree::CommandeNonGeree => "commande non gérée",
        _ => "ligne illisible",
    }
}

/// Le compte rendu d'un import : ce qui a été laissé de côté et pourquoi, ce qui a été
/// repris de façon approchée. Jamais un simple « réussi » quand le profil n'est pas tout le fichier.
pub fn annonce_import(
    profil: &str,
    format: &str,
    laissees: &[(&'static str, usize)],
    approchees: usize,
) -> String {
    let mut texte = format!("« {profil} » importé depuis {}", nom_format(format));
    let total: usize = laissees.iter().map(|(_, n)| n).sum();
    if total == 0 {
        texte.push('.');
    } else {
        let details: Vec<String> = laissees
            .iter()
            .map(|(raison, n)| format!("{n} × {raison}"))
            .collect();
        texte.push_str(&format!(
            ", en partie seulement : {} laissée{} de côté ({}).",
            pluriel(total as u32, "ligne", "lignes"),
            if total > 1 { "s" } else { "" },
            details.join(", ")
        ));
    }
    if approchees > 0 {
        texte.push_str(&format!(
            " {} reprise{} de façon approchée.",
            pluriel(approchees as u32, "ligne", "lignes"),
            if approchees > 1 { "s" } else { "" }
        ));
    }
    texte
}

/// Après « Rendre le son d'origine ». `copie` : une ancienne copie de la configuration
/// restée sur le disque parce qu'elle différait de ce qui a été rendu.
pub fn annonce_son_origine(copie: Option<&str>) -> String {
    match copie {
        None => ANNONCE_SON_ORIGINE.to_string(),
        Some(chemin) => format!(
            "{ANNONCE_SON_ORIGINE} Une ancienne copie de ta configuration, différente, est gardée ici : {chemin}"
        ),
    }
}

pub fn annonce_export(chemin: &str) -> String {
    format!("Exporté dans {chemin}")
}

pub fn annonce_egaliseur(coupe: bool) -> &'static str {
    if coupe {
        "Égaliseur coupé : le son est à plat."
    } else {
        "Égaliseur actif."
    }
}

pub const ANNONCE_MOTEUR_AUTORISE: &str = "Égaliseur activé sur ta sortie audio.";
pub const ANNONCE_SON_ORIGINE: &str =
    "Son d’origine rendu : ce que Retro EQ avait écrit dans ta configuration audio est retiré.";
pub const ANNONCE_SANS_REPONSE: &str =
    "Pas de réponse depuis un moment : test en pause, ta référence est remise. Ce duel sera rejoué.";
pub const ANNONCE_TEST_ARRETE: &str = "Test arrêté. Ta référence est remise.";
pub const ANNONCE_TEST_REPRIS: &str =
    "Un test interrompu a été retrouvé : tes réponses sont gardées, tu peux le reprendre.";
pub const ANNONCE_PAUSE_ERREUR: &str =
    "Le moteur audio n’a pas pu changer de version : test en pause, référence remise.";
pub const ANNONCE_PAS_ENCORE_ACTIVE: &str =
    "Égaliseur pas encore activé : termine le premier lancement pour l’entendre.";
pub const ANNONCE_IMPORT_INDISPONIBLE: &str =
    "Le choix d’un fichier n’est disponible que sous Windows.";

pub fn annonce_nouvelle_sortie(nom: &str) -> String {
    format!(
        "Nouvelle sortie audio « {nom} » : Retro EQ n’y est pas encore actif. Ouvre le Configurator d’Equalizer APO pour l’y installer."
    )
}

// ----- Raccourcis -----

/// Les lignes du tableau des raccourcis : (touches, action).
pub fn raccourcis() -> Vec<(&'static str, &'static str)> {
    vec![
        ("F6", "Je préfère A (maintenir : réécouter A)"),
        ("F7", "Pareil"),
        ("F8", "Je préfère B (maintenir : réécouter B)"),
        ("F9", "Plus tard"),
        ("F10", "Annuler ma dernière réponse"),
        ("Ctrl+Alt+Entrée", "Démarrer ou reprendre un test"),
        ("Ctrl+Alt+1 à 4", "Choisir un profil"),
        ("Ctrl+Alt+0", "Couper ou remettre l’égaliseur"),
    ]
}

// ----- Autotest -----

/// Un contrôle de l'autotest : titre, détail, et le libellé du bouton d'action (vide : aucun).
pub struct TexteControle {
    pub titre: String,
    pub detail: String,
    pub action: String,
}

fn controle(titre: &str, detail: impl Into<String>, action: &str) -> TexteControle {
    TexteControle {
        titre: titre.to_string(),
        detail: detail.into(),
        action: action.to_string(),
    }
}

pub fn controle_sortie(nom: Option<&str>) -> TexteControle {
    match nom {
        Some(nom) => controle(
            "Sortie audio détectée",
            format!("Sortie par défaut : « {nom} »"),
            "",
        ),
        None => controle(
            "Aucune sortie audio détectée",
            "Branche ton casque, puis relance l’autotest.",
            "",
        ),
    }
}

/// `etat` : 0 absent, 1 inactif, 2 actif. `autorise` : l'utilisateur a accepté l'écriture.
pub fn controle_moteur(etat: i32, autorise: bool, inscriptible: Option<bool>) -> TexteControle {
    match (etat, autorise, inscriptible) {
        (0, _, _) => controle(
            "Moteur audio introuvable",
            "Equalizer APO n’est pas installé (ou pas là où Retro EQ le cherche). Sans lui, rien n’est égalisé.",
            "Télécharger Equalizer APO",
        ),
        (_, _, Some(false)) => controle(
            "Moteur audio : dossier protégé",
            "Retro EQ n’a pas le droit d’écrire dans le dossier config d’Equalizer APO.",
            "Ouvrir le dossier",
        ),
        (2, true, _) => controle(
            "Moteur audio actif",
            "Equalizer APO est installé et lit la courbe de Retro EQ.",
            "",
        ),
        (_, false, _) => controle(
            "Égaliseur pas encore activé",
            "Equalizer APO est installé. Retro EQ n’écrira dans sa configuration qu’après ton accord (bouton « Continuer »).",
            "",
        ),
        _ => controle(
            "Moteur audio inactif",
            "Equalizer APO est installé, mais ne lit pas encore la courbe de Retro EQ.",
            "",
        ),
    }
}

pub fn controle_raccourcis(conflits: &[String], disponibles: bool) -> TexteControle {
    if !disponibles {
        controle(
            "Raccourcis globaux indisponibles",
            "Le système n’a pas laissé Retro EQ enregistrer de raccourcis clavier.",
            "",
        )
    } else if conflits.is_empty() {
        controle(
            "Raccourcis libres",
            "Ctrl+Alt+Entrée, Ctrl+Alt+0 à 4. F6 à F10 ne sont pris que pendant un test.",
            "",
        )
    } else {
        controle(
            "Raccourcis déjà pris",
            format!(
                "Une autre appli utilise déjà : {}. Ferme-la ou libère ces touches.",
                conflits.join(", ")
            ),
            "",
        )
    }
}

/// Les contrôles que l'appli ne sait pas faire elle-même : elle dit où regarder, sans
/// afficher une coche verte qu'elle ne peut pas garantir.
pub fn controles_a_verifier_soi_meme() -> Vec<TexteControle> {
    vec![
        controle(
            "Égaliseur et son 360 d’INZONE Hub",
            "Retro EQ ne peut pas les lire. Dans INZONE Hub : égaliseur sur « Plat », son spatial 360 désactivé.",
            "",
        ),
        controle(
            "Son spatial de Windows",
            "À vérifier toi-même : Paramètres › Système › Son › ta sortie › Son spatial sur « Désactivé ».",
            "Ouvrir les réglages du son",
        ),
        controle(
            "Améliorations audio de Windows",
            "À vérifier toi-même : Paramètres › Système › Son › ta sortie › Améliorations audio sur « Désactivé ».",
            "Ouvrir les réglages du son",
        ),
    ]
}

pub fn bilan_autotest(bons: usize, a_verifier: usize, erreurs: usize) -> String {
    let mut morceaux = vec![format!("{bons} bons")];
    if a_verifier > 0 {
        morceaux.push(format!("{a_verifier} à vérifier toi-même"));
    }
    if erreurs > 0 {
        morceaux.push(format!("{erreurs} à régler"));
    }
    morceaux.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_gains_sont_ecrits_a_la_francaise() {
        assert_eq!(db(4.0), "+4 dB");
        assert_eq!(db(-2.5), "−2,5 dB");
        assert_eq!(db(0.0), "0 dB");
        assert_eq!(db(-0.01), "0 dB");
    }

    #[test]
    fn les_plages_de_zone_melangent_hz_et_khz_quand_il_faut() {
        assert_eq!(plage_zone(Zone::Graves), "60–250 Hz");
        assert_eq!(plage_zone(Zone::Mediums), "500 Hz–2 kHz");
        assert_eq!(plage_zone(Zone::Presence), "2–5 kHz");
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
            duree_test(12, 20),
            "12 duels · environ 24 min · bascule toutes les 20 s"
        );
        assert_eq!(
            duree_test(6, 120),
            "6 duels · environ 72 min · bascule toutes les 2 min"
        );
    }

    #[test]
    fn un_controle_n_est_pas_revele_avant_la_fin_du_test() {
        let duel = DuelFini {
            numero: 3,
            sujet: Sujet::Controle,
            reponse: Some(Reponse::A),
            reference: Cote::A,
            preference: Some(Preference::Reference),
        };
        let (titre, detail) = ligne_journal(&duel, false);
        assert_eq!(titre, "Duel 3");
        assert_eq!(detail, "réponse enregistrée");
        assert!(!format!("{titre}{detail}").contains("contrôle"));
        let (titre, _) = ligne_journal(&duel, true);
        assert!(titre.contains("contrôle"));
    }

    #[test]
    fn le_resume_accorde_le_singulier_et_le_pluriel() {
        assert_eq!(
            resume_zones(2, 3, false),
            "2 zones changent, 3 restent comme avant."
        );
        assert_eq!(
            resume_zones(1, 1, false),
            "1 zone change, 1 reste comme avant."
        );
        assert!(resume_zones(0, 4, false).starts_with("Aucune zone"));
    }
}
