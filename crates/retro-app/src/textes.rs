//! Toutes les phrases que le code Rust fabrique pour l'interface, au même endroit, dans les
//! deux langues.
//!
//! Les textes fixes des écrans sont dans les fichiers `.slint`, derrière `@tr(...)`, et leur
//! traduction est dans `lang/` (voir `lang/README.md`). Ceux d'ici dépendent de chiffres
//! (« 2 zones changent »), de noms ou d'erreurs : ils ne peuvent pas y être écrits d'avance.
//! Ce sont des fonctions pures : des nombres ou des énumérations en entrée, une phrase en
//! sortie.
//!
//! Chaque fonction prend la [`Langue`] en dernier argument et fait un `match` dessus : le
//! français est la langue de référence, l'anglais en est la traduction. Ajouter une langue,
//! c'est ajouter une variante à `Langue` : le compilateur montre alors, fonction par
//! fonction, chaque `match` à compléter. Aucune bibliothèque de traduction.

use crate::controleur::{ActionFichier, ErreurControleur};
use crate::http::ErreurReseau;
use retro_core::autoeq_base::ErreurBase;
use retro_core::formats::{ErreurFormat, RaisonIgnoree};
use retro_core::{
    BilanControles, BilanDuels, ComptesSens, Confiance, Cote, DuelFini, ErreurDepot, ErreurProfil,
    ErreurReprise, Langue, Origine, Preference, Reponse, ResultatZone, Sens, Sujet, Zone,
};
use retro_engine::ErreurMoteur;

#[cfg(test)]
mod tests;

/// Choisit entre deux phrases fixes : le français d'abord, puis l'anglais.
fn choisir(l: Langue, fr: &'static str, en: &'static str) -> &'static str {
    match l {
        Langue::Fr => fr,
        Langue::En => en,
    }
}

/// La langue du système, d'après son code de locale (« fr-FR », « fr_FR.UTF-8 », « en-GB »).
/// Français seulement si le système est en français, anglais dans tous les autres cas, y
/// compris quand le système ne dit rien.
pub fn langue_depuis_locale(locale: Option<&str>) -> Langue {
    match locale {
        Some(code) if code.trim().to_lowercase().starts_with("fr") => Langue::Fr,
        _ => Langue::En,
    }
}

/// Un nombre suivi de son nom, accordé : « 1 duel », « 12 duels ». En français 0 et 1 sont
/// au singulier (« 0 fois »), en anglais seul 1 l'est (« 0 times »).
fn nombre(l: Langue, n: u32, fr: [&str; 2], en: [&str; 2]) -> String {
    let (mot, singulier) = match l {
        Langue::Fr => (fr, n <= 1),
        Langue::En => (en, n == 1),
    };
    format!("{n} {}", if singulier { mot[0] } else { mot[1] })
}

fn duels(n: u32, l: Langue) -> String {
    nombre(l, n, ["duel", "duels"], ["duel", "duels"])
}

fn lignes(n: usize, l: Langue) -> String {
    nombre(l, n as u32, ["ligne", "lignes"], ["line", "lines"])
}

fn fois(n: u32, l: Langue) -> String {
    nombre(l, n, ["fois", "fois"], ["time", "times"])
}

/// Un texte entre guillemets : « texte » en français, “texte” en anglais.
fn entre_guillemets(texte: &str, l: Langue) -> String {
    match l {
        Langue::Fr => format!("« {texte} »"),
        Langue::En => format!("“{texte}”"),
    }
}

/// « 1er », « 2e » ; « 1st », « 2nd », « 3rd », « 4th ».
fn rang(n: u8, l: Langue) -> String {
    match l {
        Langue::Fr if n <= 1 => "1er".to_string(),
        Langue::Fr => format!("{n}e"),
        Langue::En => match n {
            1 => "1st".to_string(),
            2 => "2nd".to_string(),
            3 => "3rd".to_string(),
            _ => format!("{n}th"),
        },
    }
}

/// Met la première lettre en majuscule (un message d'erreur commence une phrase).
fn majuscule(texte: &str) -> String {
    let mut lettres = texte.chars();
    match lettres.next() {
        Some(premiere) => premiere.to_uppercase().chain(lettres).collect(),
        None => String::new(),
    }
}

/// Une valeur en dB comme l'interface l'écrit : « −6 dB », « −4,2 dB » (« −4.2 dB » en
/// anglais), « +3 dB ».
pub fn db(valeur: f64, l: Langue) -> String {
    let dixiemes = (valeur.abs() * 10.0).round() as i64;
    let signe = match valeur {
        _ if dixiemes == 0 => "",
        v if v < 0.0 => "−",
        _ => "+",
    };
    let (entier, decimale) = (dixiemes / 10, dixiemes % 10);
    let separateur = choisir(l, ",", ".");
    if decimale == 0 {
        format!("{signe}{entier} dB")
    } else {
        format!("{signe}{entier}{separateur}{decimale} dB")
    }
}

// ----- Zones, presets, profils par défaut -----

/// Le nom d'une zone, pour l'affichage. (Le nom français de `Zone::nom` sert d'identifiant.)
pub fn nom_zone(zone: Zone, l: Langue) -> &'static str {
    match l {
        Langue::Fr => zone.nom(),
        Langue::En => match zone {
            Zone::SubGraves => "Sub-bass",
            Zone::Graves => "Bass",
            Zone::BasMediums => "Low mids",
            Zone::Mediums => "Mids",
            Zone::Presence => "Presence",
            Zone::Aigus => "Treble",
            Zone::Air => "Air",
        },
    }
}

/// Les noms des 7 zones, pour le choix « Zone à tester ».
pub fn noms_zones(l: Langue) -> Vec<String> {
    Zone::TOUTES
        .iter()
        .map(|&z| nom_zone(z, l).to_string())
        .collect()
}

/// Le nom d'un preset livré avec l'appli, tel qu'on l'affiche (`nom` est son nom français).
pub fn nom_preset(nom: &str, l: Langue) -> String {
    match (l, nom) {
        (Langue::En, "À plat") => "Flat".to_string(),
        _ => nom.to_string(),
    }
}

/// Le nom du profil créé au tout premier lancement.
pub fn profil_par_defaut(l: Langue) -> &'static str {
    choisir(l, "Mon profil", "My profile")
}

/// Le nom d'un profil importé dont le fichier n'en donne aucun.
pub fn profil_importe(l: Langue) -> &'static str {
    choisir(l, "Profil importé", "Imported profile")
}

/// Le nom d'une copie : « Mon profil (copie) ».
pub fn copie_de(nom: &str, l: Langue) -> String {
    match l {
        Langue::Fr => format!("{nom} (copie)"),
        Langue::En => format!("{nom} (copy)"),
    }
}

/// Le nom du fichier exporté depuis un résultat, avant son suffixe de format.
pub fn nom_export_resultat(profil: &str, l: Langue) -> String {
    match l {
        Langue::Fr => format!("{profil} (résultat)"),
        Langue::En => format!("{profil} (result)"),
    }
}

/// « Mon profil · version 2 ».
pub fn profil_et_version(profil: &str, numero: u32, l: Langue) -> String {
    match l {
        Langue::Fr | Langue::En => format!("{profil} · version {numero}"),
    }
}

/// Le titre d'un résultat « profil contre profil » : « Mon profil contre Harman ».
pub fn titre_profil_contre_profil(profil: &str, autre: &str, l: Langue) -> String {
    match l {
        Langue::Fr => format!("{profil} contre {autre}"),
        Langue::En => format!("{profil} vs {autre}"),
    }
}

/// Le nom de l'autre profil quand il a disparu entre-temps.
pub fn l_autre_profil(l: Langue) -> &'static str {
    choisir(l, "l’autre profil", "the other profile")
}

// ----- Les modes de test -----

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

pub fn source_pc(l: Langue) -> &'static str {
    choisir(l, "Ce qui joue sur le PC", "What plays on the PC")
}

/// Les cassettes de l'écran « Tester ». Trois modes en v1, les autres sont annoncés.
pub fn modes(l: Langue) -> Vec<TexteMode> {
    let trouver = choisir(l, "Trouver ma courbe", "Find my curve");
    let affiner = choisir(l, "Affiner", "Fine-tune");
    let verifier = choisir(l, "Vérifier", "Verify");
    let pc = source_pc(l);
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
            trouver,
            choisir(l, "Duels en jeu", "In-game duels"),
            choisir(
                l,
                "Le son bascule tout seul pendant que tu joues, regardes ou parles sur Discord.",
                "The sound switches by itself while you play, watch or chat on Discord.",
            ),
            pc,
            "12",
            "25 min",
            true,
        ),
        m(
            "parcours",
            trouver,
            choisir(l, "Parcours complet", "Full run"),
            choisir(
                l,
                "Un son préparé par zone, des graves profonds jusqu’à l’air. Le plus complet.",
                "A prepared sound for each zone, from deep bass up to air. The most thorough.",
            ),
            choisir(l, "Échantillons préparés", "Prepared samples"),
            "21",
            "35 min",
            false,
        ),
        m(
            "musique",
            trouver,
            choisir(l, "Sur une musique", "On a song"),
            choisir(
                l,
                "Ton morceau préféré en boucle, sur le passage que tu choisis.",
                "Your favourite track on loop, on the section you pick.",
            ),
            choisir(l, "Ton fichier audio", "Your audio file"),
            "12",
            "15 min",
            false,
        ),
        m(
            "echantillons",
            affiner,
            choisir(l, "Échantillons ciblés", "Targeted samples"),
            choisir(
                l,
                "Un son choisi pour une seule zone : voix, pas, cymbales, basse.",
                "A sound chosen for a single zone: voice, footsteps, cymbals, bass.",
            ),
            choisir(l, "Un échantillon", "One sample"),
            "6",
            "8 min",
            false,
        ),
        m(
            "une-zone",
            affiner,
            choisir(l, "Une seule zone", "Single zone"),
            choisir(
                l,
                "Tu sais ce qui cloche ? Les duels ne testent que les aigus, ou que les graves.",
                "Know what’s off? The duels only test the treble, or only the bass.",
            ),
            pc,
            "6",
            "12 min",
            true,
        ),
        m(
            "profil-contre-profil",
            affiner,
            choisir(l, "Profil contre profil", "Profile vs profile"),
            choisir(
                l,
                "Deux profils face à face, par exemple Harman contre ton profil du jour.",
                "Two profiles head to head, for example Harman against your everyday profile.",
            ),
            pc,
            "6",
            "12 min",
            true,
        ),
        m(
            "abx",
            verifier,
            choisir(l, "Test à l’aveugle", "Blind test"),
            choisir(
                l,
                "A, B, puis X au hasard : sais-tu vraiment reconnaître ton profil ?",
                "A, B, then a random X: can you really tell your profile apart?",
            ),
            choisir(l, "Au choix", "Your choice"),
            "10",
            "10 min",
            false,
        ),
        m(
            "audition",
            verifier,
            choisir(l, "Test d’audition", "Hearing test"),
            choisir(
                l,
                "Mesure ce que tes oreilles entendent, zone par zone, pour adapter la courbe.",
                "Measures what your ears pick up, zone by zone, to adapt the curve.",
            ),
            choisir(l, "Sons purs", "Pure tones"),
            "—",
            "—",
            false,
        ),
    ]
}

/// Une fréquence lisible : « 250 Hz », « 2 kHz ». Le séparateur décimal suit la langue.
fn frequence(hz: f64, l: Langue) -> (String, &'static str) {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        let n = if k.fract() == 0.0 {
            format!("{}", k as i64)
        } else {
            format!("{k:.1}").replace('.', choisir(l, ",", "."))
        };
        (n, "kHz")
    } else {
        (format!("{}", hz.round() as i64), "Hz")
    }
}

/// La plage d'une zone : « 60–250 Hz », « 500 Hz–2 kHz ».
pub fn plage_zone(zone: Zone, l: Langue) -> String {
    let (bas, haut) = zone.bornes_hz();
    let ((b, ub), (h, uh)) = (frequence(bas, l), frequence(haut, l));
    if ub == uh {
        format!("{b}–{h} {uh}")
    } else {
        format!("{b} {ub}–{h} {uh}")
    }
}

/// Résumé de la durée d'un test : « 12 duels · environ 24 min · bascule toutes les 20 s ».
pub fn duree_test(nombre_duels: u32, intervalle_s: u32, l: Langue) -> String {
    let minutes = (nombre_duels * 6 * intervalle_s).div_ceil(60);
    let bascule = if intervalle_s >= 60 && intervalle_s.is_multiple_of(60) {
        format!("{} min", intervalle_s / 60)
    } else {
        format!("{intervalle_s} s")
    };
    let duels = duels(nombre_duels, l);
    match l {
        Langue::Fr => format!("{duels} · environ {minutes} min · bascule toutes les {bascule}"),
        Langue::En => format!("{duels} · about {minutes} min · switches every {bascule}"),
    }
}

/// Le nom du mode choisi, tel qu'il apparaît dans le résumé d'un nouveau test.
pub fn nom_mode(id: &str, zone: Option<Zone>, autre_profil: &str, l: Langue) -> String {
    match id {
        "une-zone" => {
            let titre = choisir(l, "Une seule zone", "Single zone");
            match zone {
                Some(z) => format!("{titre} · {} ({})", nom_zone(z, l), plage_zone(z, l)),
                None => titre.to_string(),
            }
        }
        "profil-contre-profil" => {
            let titre = choisir(l, "Profil contre profil", "Profile vs profile");
            if autre_profil.is_empty() {
                titre.to_string()
            } else {
                format!("{titre} · {autre_profil}")
            }
        }
        _ => choisir(
            l,
            "Duels en jeu · une zone change à chaque duel",
            "In-game duels · one zone changes in each duel",
        )
        .to_string(),
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
pub fn suite(cote: Cote, numero: u8, l: Langue) -> String {
    let lettre = lettre(cote);
    let rang = rang(numero, l);
    match l {
        Langue::Fr => format!("{lettre} joue son {rang} passage"),
        Langue::En => format!("{lettre} plays its {rang} pass"),
    }
}

pub fn lettre(cote: Cote) -> &'static str {
    match cote {
        Cote::A => "A",
        Cote::B => "B",
    }
}

// ----- Le journal d'un test -----

/// « Plus de graves », « Moins d’air » : un sens d'une zone, avec l'élision devant une voyelle
/// en français (« More bass », « Less air » en anglais).
fn sens_de_zone(zone: Zone, sens: Sens, l: Langue) -> String {
    let nom = nom_zone(zone, l).to_lowercase();
    match l {
        Langue::Fr => {
            let debut = match sens {
                Sens::Plus => "Plus",
                Sens::Moins => "Moins",
            };
            if nom.starts_with(['a', 'e', 'i', 'o', 'u', 'é']) {
                format!("{debut} d’{nom}")
            } else {
                format!("{debut} de {nom}")
            }
        }
        Langue::En => match sens {
            Sens::Plus => format!("More {nom}"),
            Sens::Moins => format!("Less {nom}"),
        },
    }
}

/// Une ligne du journal d'un test : titre et détail.
///
/// `revele` ne devient vrai qu'une fois le test fini. Avant, **toutes** les lignes se
/// ressemblent : « Duel n · réponse enregistrée », avec la réponse donnée (la pastille A ou
/// B, ou « Pareil ») et rien d'autre. Montrer la zone, ou qui était la référence, d'une
/// ligne sur deux apprendrait à repérer les duels de contrôle par élimination, et à
/// deviner la variante.
pub fn ligne_journal(duel: &DuelFini, revele: bool, l: Langue) -> (String, String) {
    let n = duel.numero;
    let Some(preference) = duel.preference else {
        return (
            format!("Duel {n}"),
            choisir(l, "passé, sans réponse", "skipped, no answer").to_string(),
        );
    };
    if !revele {
        let detail = if duel.reponse == Some(Reponse::Pareil) {
            choisir(l, "Pareil", "Same")
        } else {
            ""
        };
        let titre = match l {
            Langue::Fr => format!("Duel {n} · réponse enregistrée"),
            Langue::En => format!("Duel {n} · answer recorded"),
        };
        return (titre, detail.to_string());
    }
    let titre = match (duel.sujet, l) {
        (Sujet::Controle, Langue::Fr) => format!("Duel {n} · contrôle (deux versions identiques)"),
        (Sujet::Controle, Langue::En) => format!("Duel {n} · control (two identical versions)"),
        (Sujet::Zone { zone, sens }, Langue::Fr) => format!(
            "Duel {n} · {} {}",
            nom_zone(zone, l).to_lowercase(),
            match sens {
                Sens::Plus => "en plus",
                Sens::Moins => "en moins",
            }
        ),
        (Sujet::Zone { zone, sens }, Langue::En) => {
            format!("Duel {n} · {}", sens_de_zone(zone, sens, l).to_lowercase())
        }
        (Sujet::AutreProfil, Langue::Fr) => format!("Duel {n} · autre profil"),
        (Sujet::AutreProfil, Langue::En) => format!("Duel {n} · other profile"),
        (Sujet::Verification, Langue::Fr) => format!("Duel {n} · vérification"),
        (Sujet::Verification, Langue::En) => format!("Duel {n} · check"),
    };
    let detail = match (duel.sujet, preference) {
        (Sujet::Controle, Preference::Aucune) => {
            choisir(l, "Pareil · bonne réponse", "Same · correct answer")
        }
        (Sujet::Controle, _) => choisir(
            l,
            "une différence entendue, alors qu’il n’y en avait pas",
            "a difference heard, though there was none",
        ),
        (_, Preference::Aucune) => choisir(l, "Pareil · référence gardée", "Same · reference kept"),
        (_, Preference::Reference) => choisir(
            l,
            "était la référence · préférée",
            "was the reference · preferred",
        ),
        (Sujet::AutreProfil, Preference::Variante) => choisir(
            l,
            "était l’autre profil · préféré",
            "was the other profile · preferred",
        ),
        (Sujet::Verification, Preference::Variante) => choisir(
            l,
            "était la nouvelle version · préférée",
            "was the new version · preferred",
        ),
        (_, Preference::Variante) => choisir(
            l,
            "était la variante · préférée",
            "was the variant · preferred",
        ),
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

// ----- Le résultat -----

/// 0 = Indice, 1 = Probable, 2 = Sûr : l'échelle de l'interface.
pub fn niveau_confiance(confiance: Confiance) -> i32 {
    match confiance {
        Confiance::Indice => 0,
        Confiance::Probable => 1,
        Confiance::Sur => 2,
    }
}

/// Le résumé d'un résultat par zones : « 2 zones changent, 3 restent comme avant. »
pub fn resume_zones(changees: u32, inchangees: u32, rien_retenu: bool, l: Langue) -> String {
    if rien_retenu {
        return choisir(
            l,
            "Rien n’est retenu de ce test : tu as entendu des différences entre des versions identiques. Ton réglage est gardé tel quel.",
            "Nothing is kept from this test: you heard differences between identical versions. Your setting is kept as it is.",
        )
        .to_string();
    }
    match l {
        Langue::Fr => {
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
        Langue::En => {
            let reste = match inchangees {
                0 => String::new(),
                1 => ", 1 stays as before".to_string(),
                n => format!(", {n} stay as before"),
            };
            match changees {
                0 => "No zone changes: your setting is kept as it is.".to_string(),
                1 => format!("1 zone changes{reste}."),
                n => format!("{n} zones change{reste}."),
            }
        }
    }
}

/// Le résumé d'un test « profil contre profil ».
pub fn resume_autre_profil(bilan: &BilanDuels, autre: &str, l: Langue) -> String {
    let departages = bilan.pour_variante + bilan.pour_reference;
    let autre = entre_guillemets(autre, l);
    match (bilan.preference, l) {
        (Preference::Variante, Langue::Fr) => format!(
            "{autre} préféré {} sur {departages}.",
            fois(bilan.pour_variante, l)
        ),
        (Preference::Variante, Langue::En) => format!(
            "{autre} preferred {} out of {departages}.",
            fois(bilan.pour_variante, l)
        ),
        (Preference::Reference, Langue::Fr) => format!(
            "Ta référence préférée {} sur {departages} face à {autre}.",
            fois(bilan.pour_reference, l)
        ),
        (Preference::Reference, Langue::En) => format!(
            "Your reference preferred {} out of {departages} against {autre}.",
            fois(bilan.pour_reference, l)
        ),
        (Preference::Aucune, Langue::Fr) => {
            format!("Pas de préférence nette entre ta référence et {autre}.")
        }
        (Preference::Aucune, Langue::En) => {
            format!("No clear preference between your reference and {autre}.")
        }
    }
}

/// Ce que les duels de contrôle disent des réponses. Vide s'il n'y en a pas eu.
pub fn bilan_controles(c: &BilanControles, l: Langue) -> String {
    if c.repondus == 0 {
        return String::new();
    }
    let sur = nombre(
        l,
        c.repondus,
        ["duel de contrôle", "duels de contrôle"],
        ["control duel", "control duels"],
    );
    match (c.rien_retenu || c.penalise, l) {
        (true, Langue::Fr) => format!(
            "Sur {sur} (deux versions identiques), tu as entendu une différence {}. Tes réponses ne distinguent donc pas deux versions identiques : aucun changement n’est retenu pour ce test.",
            fois(c.a_ou_b, l)
        ),
        (true, Langue::En) => format!(
            "Out of {sur} (two identical versions), you heard a difference {}. Your answers can’t tell two identical versions apart, so no change is kept for this test.",
            fois(c.a_ou_b, l)
        ),
        (false, Langue::Fr) => format!(
            "Sur {sur} (deux versions identiques), {} jugés « Pareil » : tes réponses se tiennent.",
            c.pareil
        ),
        (false, Langue::En) => format!(
            "Out of {sur} (two identical versions), {} judged “Same”: your answers hold up.",
            c.pareil
        ),
    }
}

/// Ce que le duel de vérification a donné. Vide s'il n'a pas eu lieu.
pub fn verification(preference: Option<Preference>, l: Langue) -> String {
    match preference {
        None => String::new(),
        Some(Preference::Variante) => choisir(
            l,
            "Duel de vérification : nouvelle version préférée à la référence.",
            "Verification duel: new version preferred over the reference.",
        )
        .to_string(),
        Some(Preference::Reference) => choisir(
            l,
            "Duel de vérification : tu as préféré ta référence à la nouvelle version. La nouvelle version est gardée dans le profil, mais rien n’indique qu’elle te plaît davantage.",
            "Verification duel: you preferred your reference to the new version. The new version is kept in the profile, but nothing suggests you like it better.",
        )
        .to_string(),
        Some(Preference::Aucune) => choisir(
            l,
            "Duel de vérification : pas de différence entendue entre la référence et la nouvelle version.",
            "Verification duel: no difference heard between the reference and the new version.",
        )
        .to_string(),
    }
}

/// Ce que les réponses disent d'un seul sens d'une zone : « préféré 2 fois sur 2 ». Chaque
/// sens est dit à part, jamais mélangé à l'autre. « Pareil » ne compte pour personne dans le
/// « sur » (ADR-0005) ; il est rappelé à côté quand il y en a eu.
fn comptes_du_sens(c: &ComptesSens, l: Langue) -> String {
    let departages = c.variante + c.reference;
    let pareil = |n: u32| match l {
        Langue::Fr => format!("Pareil {}", fois(n, l)),
        Langue::En => format!("Same {}", fois(n, l)),
    };
    let mut texte = match (c.variante, c.reference, l) {
        (0, 0, _) => return pareil(c.pareil),
        (v, 0, Langue::Fr) => format!("préféré {} sur {v}", fois(v, l)),
        (v, 0, Langue::En) => format!("preferred {} out of {v}", fois(v, l)),
        (0, r, Langue::Fr) => format!("la référence préférée {} sur {r}", fois(r, l)),
        (0, r, Langue::En) => format!("reference preferred {} out of {r}", fois(r, l)),
        (v, _, Langue::Fr) => format!("avis partagés, préféré {} sur {departages}", fois(v, l)),
        (v, _, Langue::En) => format!("split, preferred {} out of {departages}", fois(v, l)),
    };
    if c.pareil > 0 {
        texte.push_str(&format!(" ({})", pareil(c.pareil)));
    }
    texte
}

/// Le détail d'une zone du résultat : chaque sens testé est dit à part et simplement, puis
/// ce qu'on en conclut. Un sens qui n'a pas été testé n'est pas mentionné. Les comptes sont
/// ceux des comparaisons, sans précision inventée (ADR-0005).
pub fn detail_zone(z: &ResultatZone, rien_retenu: bool, l: Langue) -> String {
    let mut morceaux: Vec<String> = [(Sens::Plus, &z.plus), (Sens::Moins, &z.moins)]
        .into_iter()
        .filter(|(_, c)| c.total() > 0)
        .map(|(sens, c)| {
            format!(
                "{}{}{}",
                sens_de_zone(z.zone, sens, l),
                choisir(l, " : ", ": "),
                comptes_du_sens(c, l)
            )
        })
        .collect();
    if morceaux.is_empty() {
        // Un test enregistré avant que les sens soient comptés à part : on n'a que les totaux.
        morceaux.push(match l {
            Langue::Fr => format!(
                "deux sens confondus : variante {}, référence {}, Pareil {}",
                fois(z.pour_variante, l),
                fois(z.pour_reference, l),
                fois(z.pareil, l),
            ),
            Langue::En => format!(
                "both directions combined: variant {}, reference {}, Same {}",
                fois(z.pour_variante, l),
                fois(z.pour_reference, l),
                fois(z.pareil, l),
            ),
        });
    }
    let conclusion = if rien_retenu {
        choisir(
            l,
            "non retenu pour ce test (voir les duels de contrôle)",
            "not kept for this test (see the control duels)",
        )
        .to_string()
    } else if z.contradictoire {
        choisir(
            l,
            "préféré en plus comme en moins : rien ne change",
            "preferred both ways: nothing changes",
        )
        .to_string()
    } else if z.est_changee() {
        match l {
            Langue::Fr => format!(
                "changement retenu : {}",
                explication_confiance(z.confiance, l)
            ),
            Langue::En => format!("change kept: {}", explication_confiance(z.confiance, l)),
        }
    } else if z.indice.is_some() {
        choisir(
            l,
            "un indice, à confirmer au prochain test",
            "a hint, to confirm at the next test",
        )
        .to_string()
    } else if z.pour_variante == 0 && z.pour_reference == 0 {
        choisir(l, "ton réglage est gardé", "your setting is kept").to_string()
    } else {
        choisir(l, "rien ne change", "nothing changes").to_string()
    };
    morceaux.push(conclusion);
    morceaux.join(" · ")
}

/// Ce qu'un niveau de confiance veut dire, dans les termes de l'ADR-0005 : il décrit une
/// comparaison prise seule. Il ne dit pas « ce changement est probablement réel », parce
/// qu'un test fait plusieurs comparaisons à la fois.
pub fn explication_confiance(confiance: Confiance, l: Langue) -> &'static str {
    match confiance {
        Confiance::Sur => choisir(
            l,
            "prise seule, cette comparaison avait moins de 6 % de chances de tomber ainsi par hasard",
            "taken alone, this comparison had under a 6% chance of coming out this way by luck",
        ),
        Confiance::Probable => choisir(
            l,
            "prise seule, cette comparaison avait au plus 1 chance sur 4 de tomber ainsi par hasard",
            "taken alone, this comparison had at most a 1 in 4 chance of coming out this way by luck",
        ),
        Confiance::Indice => choisir(
            l,
            "prise seule, cette comparaison peut très bien être due au hasard",
            "taken alone, this comparison may well be down to luck",
        ),
    }
}

/// La préférence relevée dans une appli.
pub fn preference_appli(preference: Preference, l: Langue) -> &'static str {
    match preference {
        Preference::Variante => choisir(
            l,
            "les changements proposés ont plu",
            "the proposed changes were liked",
        ),
        Preference::Reference => choisir(
            l,
            "la référence a été préférée",
            "the reference was preferred",
        ),
        Preference::Aucune => choisir(l, "pas de préférence nette", "no clear preference"),
    }
}

// ----- Profils et versions -----

/// D'où vient une version, pour la liste des versions d'un profil.
pub fn origine(origine: &Origine, l: Langue) -> String {
    match origine {
        Origine::Preset => choisir(l, "Preset de départ", "Starting preset").to_string(),
        Origine::Import { format } => format!(
            "Import {}{}",
            nom_format(format, l),
            mesures_autoeq(format, l)
        ),
        Origine::Manuel => choisir(
            l,
            "Réglée à la main dans l’égaliseur",
            "Set by hand in the equalizer",
        )
        .to_string(),
        Origine::Test { .. } => choisir(l, "Issue d’un test", "From a test").to_string(),
    }
}

/// Le résumé d'un profil sur sa cassette.
pub fn resume_profil(origine_active: &Origine, l: Langue) -> String {
    match origine_active {
        Origine::Preset => choisir(l, "Parti d’un preset", "Started from a preset").to_string(),
        Origine::Import { format } => match l {
            Langue::Fr => format!(
                "Importé de {}{}",
                nom_format(format, l),
                mesures_autoeq(format, l)
            ),
            Langue::En => format!(
                "Imported from {}{}",
                nom_format(format, l),
                mesures_autoeq(format, l)
            ),
        },
        Origine::Manuel => choisir(l, "Réglé à la main", "Set by hand").to_string(),
        Origine::Test { .. } => choisir(l, "Issu d’un test", "From a test").to_string(),
    }
}

/// Le nom lisible d'un format de fichier (identifiant interne ou identifiant de l'interface).
pub fn nom_format(format: &str, l: Langue) -> &'static str {
    match format {
        "peace" => "Peace",
        "apo" | "equalizer_apo" => "Equalizer APO",
        "autoeq" => "AutoEQ",
        f if f.starts_with("autoeq|") => "AutoEQ",
        "wavelet" => "Wavelet",
        _ => choisir(l, "fichier", "file"),
    }
}

// ----- Annonces (le message court en bas de la fenêtre) -----

pub fn annonce_version_enregistree(numero: u32, profil: &str, l: Langue) -> String {
    let profil = entre_guillemets(profil, l);
    match l {
        Langue::Fr => format!("Version {numero} de {profil} enregistrée et activée."),
        Langue::En => format!("Version {numero} of {profil} saved and activated."),
    }
}

pub fn annonce_profil_active(profil: &str, l: Langue) -> String {
    let profil = entre_guillemets(profil, l);
    match l {
        Langue::Fr => format!("{profil} est maintenant ton profil actif."),
        Langue::En => format!("{profil} is now your active profile."),
    }
}

pub fn annonce_version_activee(numero: u32, profil: &str, l: Langue) -> String {
    let profil = entre_guillemets(profil, l);
    match l {
        Langue::Fr => format!("Version {numero} de {profil} activée."),
        Langue::En => format!("Version {numero} of {profil} activated."),
    }
}

pub fn annonce_profil_cree(profil: &str, l: Langue) -> String {
    let profil = entre_guillemets(profil, l);
    match l {
        Langue::Fr => format!("Profil {profil} créé."),
        Langue::En => format!("Profile {profil} created."),
    }
}

/// Pourquoi une ligne d'un fichier importé a été laissée de côté, en quelques mots.
pub fn raison_ignoree(raison: &RaisonIgnoree, l: Langue) -> &'static str {
    match raison {
        RaisonIgnoree::TypeFiltreNonGere(_) => {
            choisir(l, "type de filtre non géré", "unsupported filter type")
        }
        RaisonIgnoree::PenteNonGeree => {
            choisir(l, "plateau réglé par une pente", "shelf set by a slope")
        }
        RaisonIgnoree::CanalParticulier => {
            choisir(l, "réglage propre à un canal", "channel-specific setting")
        }
        RaisonIgnoree::PeripheriqueParticulier => choisir(
            l,
            "réglage propre à un autre périphérique",
            "setting for another device",
        ),
        RaisonIgnoree::HautParleurParticulier => choisir(
            l,
            "réglage propre à un haut-parleur",
            "speaker-specific setting",
        ),
        RaisonIgnoree::CommandeNonGeree => choisir(l, "commande non gérée", "unsupported command"),
        _ => choisir(l, "ligne illisible", "unreadable line"),
    }
}

/// Le compte rendu d'un import : ce qui a été laissé de côté et pourquoi, ce qui a été
/// repris de façon approchée. Jamais un simple « réussi » quand le profil n'est pas tout le fichier.
///
/// `preampli` : (celui du fichier, l'automatique) quand ils diffèrent de plus de 0,5 dB. Le
/// préampli automatique est gardé, parce qu'il évite la saturation : on le dit au lieu de
/// remplacer celui du fichier en silence.
pub fn annonce_import(
    profil: &str,
    format: &str,
    laissees: &[(&'static str, usize)],
    approchees: usize,
    preampli: Option<(f64, f64)>,
    l: Langue,
) -> String {
    let profil = entre_guillemets(profil, l);
    let mut texte = match l {
        Langue::Fr => format!("{profil} importé depuis {}", nom_format(format, l)),
        Langue::En => format!("{profil} imported from {}", nom_format(format, l)),
    };
    let total: usize = laissees.iter().map(|(_, n)| n).sum();
    if total == 0 {
        texte.push('.');
    } else {
        let details: Vec<String> = laissees
            .iter()
            .map(|(raison, n)| format!("{n} × {raison}"))
            .collect();
        let accord = if total > 1 { "s" } else { "" };
        texte.push_str(&match l {
            Langue::Fr => format!(
                ", en partie seulement : {} laissée{accord} de côté ({}).",
                lignes(total, l),
                details.join(", ")
            ),
            Langue::En => format!(
                ", partially only: {} left out ({}).",
                lignes(total, l),
                details.join(", ")
            ),
        });
    }
    if approchees > 0 {
        let accord = if approchees > 1 { "s" } else { "" };
        texte.push_str(&match l {
            Langue::Fr => format!(
                " {} reprise{accord} de façon approchée.",
                lignes(approchees, l)
            ),
            Langue::En => format!(" {} taken over approximately.", lignes(approchees, l)),
        });
    }
    if let Some((fichier, auto)) = preampli {
        let (fichier, auto) = (db(fichier, l), db(auto, l));
        texte.push_str(&match l {
            Langue::Fr => format!(
                " Note : préampli du fichier ({fichier}) remplacé par le préampli automatique ({auto}), qui évite la saturation."
            ),
            Langue::En => format!(
                " Note: the file’s preamp ({fichier}) was replaced by the automatic preamp ({auto}), which avoids clipping."
            ),
        });
    }
    texte
}

/// Après « Rendre le son d'origine ». `copie` : une ancienne copie de la configuration
/// restée sur le disque parce qu'elle différait de ce qui a été rendu.
pub fn annonce_son_origine(copie: Option<&str>, l: Langue) -> String {
    let base = choisir(
        l,
        "Son d’origine rendu : ce que Retro EQ avait écrit dans ta configuration audio est retiré.",
        "Original sound restored: what Retro EQ had written to your audio configuration has been removed.",
    );
    match (copie, l) {
        (None, _) => base.to_string(),
        (Some(chemin), Langue::Fr) => format!(
            "{base} Une ancienne copie de ta configuration, différente, est gardée ici : {chemin}"
        ),
        (Some(chemin), Langue::En) => {
            format!("{base} An older, different copy of your configuration is kept here: {chemin}")
        }
    }
}

pub fn annonce_export(chemin: &str, l: Langue) -> String {
    match l {
        Langue::Fr => format!("Exporté dans {chemin}"),
        Langue::En => format!("Exported to {chemin}"),
    }
}

pub fn annonce_egaliseur(coupe: bool, l: Langue) -> &'static str {
    if coupe {
        choisir(
            l,
            "Égaliseur coupé : le son est à plat.",
            "Equalizer off: the sound is flat.",
        )
    } else {
        choisir(l, "Égaliseur actif.", "Equalizer on.")
    }
}

pub fn annonce_moteur_autorise(l: Langue) -> &'static str {
    choisir(
        l,
        "Égaliseur activé sur ta sortie audio.",
        "Equalizer enabled on your audio output.",
    )
}

pub fn annonce_sans_reponse(l: Langue) -> &'static str {
    choisir(
        l,
        "Pas de réponse depuis un moment : test en pause, ta référence est remise. Ce duel sera rejoué.",
        "No answer for a while: test paused, your reference is back. This duel will be replayed.",
    )
}

pub fn annonce_test_arrete(l: Langue) -> &'static str {
    choisir(
        l,
        "Test arrêté. Ta référence est remise.",
        "Test stopped. Your reference is back.",
    )
}

pub fn annonce_test_repris(l: Langue) -> &'static str {
    choisir(
        l,
        "Un test interrompu a été retrouvé : tes réponses sont gardées, tu peux le reprendre.",
        "An interrupted test was found: your answers are kept, you can resume it.",
    )
}

pub fn annonce_pause_erreur(l: Langue) -> &'static str {
    choisir(
        l,
        "Le moteur audio n’a pas pu changer de version : test en pause, référence remise.",
        "The audio engine couldn’t switch versions: test paused, reference restored.",
    )
}

pub fn annonce_pas_encore_active(l: Langue) -> &'static str {
    choisir(
        l,
        "Égaliseur pas encore activé : termine le premier lancement pour l’entendre.",
        "Equalizer not enabled yet: finish the first launch to hear it.",
    )
}

pub fn annonce_import_indisponible(l: Langue) -> &'static str {
    choisir(
        l,
        "Le choix d’un fichier n’est disponible que sous Windows.",
        "Choosing a file is only available on Windows.",
    )
}

pub fn annonce_nouvelle_sortie(nom: &str, l: Langue) -> String {
    let nom = entre_guillemets(nom, l);
    match l {
        Langue::Fr => format!(
            "Nouvelle sortie audio {nom} : Retro EQ n’y est pas encore actif. Ouvre le Configurator d’Equalizer APO pour l’y installer."
        ),
        Langue::En => format!(
            "New audio output {nom}: Retro EQ isn’t active on it yet. Open Equalizer APO’s Configurator to install it there."
        ),
    }
}

/// Au démarrage : les réglages n'ont pas pu être relus.
pub fn annonce_reglages_illisibles(erreur: &ErreurDepot, l: Langue) -> String {
    match l {
        Langue::Fr => format!(
            "Réglages illisibles, valeurs par défaut reprises : {}",
            erreur_depot(erreur, l)
        ),
        Langue::En => format!(
            "Settings unreadable, default values restored: {}",
            erreur_depot(erreur, l)
        ),
    }
}

/// Au démarrage : un fichier de profil n'a pas pu être lu. `raison` est le détail technique
/// donné par le dépôt, tel quel.
pub fn annonce_profil_ignore(chemin: &str, raison: &str, l: Langue) -> String {
    match l {
        Langue::Fr => format!("Profil illisible laissé de côté ({chemin}) : {raison}"),
        Langue::En => format!("Unreadable profile left aside ({chemin}): {raison}"),
    }
}

// ----- Erreurs -----

/// Le texte d'une erreur du contrôleur, tel que l'utilisateur le lit. Le `Display` de
/// l'erreur, lui, est en français et ne sert qu'aux journaux : l'interface passe toujours par ici.
pub fn erreur(erreur: &ErreurControleur, l: Langue) -> String {
    match erreur {
        ErreurControleur::Depot(e) => erreur_depot(e, l),
        ErreurControleur::Moteur(e) => match l {
            Langue::Fr => format!("Le moteur audio a refusé : {}", erreur_moteur(e, l)),
            Langue::En => format!("The audio engine refused: {}", erreur_moteur(e, l)),
        },
        ErreurControleur::Profil(e) => erreur_profil(e, l),
        ErreurControleur::Format(e) => match l {
            Langue::Fr => format!("Fichier illisible : {}", erreur_format(e, l)),
            Langue::En => format!("Unreadable file: {}", erreur_format(e, l)),
        },
        ErreurControleur::Journal(e) => match l {
            Langue::Fr => format!(
                "Réponse non enregistrée sur le disque : {}",
                erreur_depot(e, l)
            ),
            Langue::En => format!("Answer not saved to disk: {}", erreur_depot(e, l)),
        },
        ErreurControleur::ProfilInconnu => choisir(
            l,
            "Ce profil n’existe plus.",
            "This profile no longer exists.",
        )
        .to_string(),
        ErreurControleur::AucunProfil => choisir(
            l,
            "Aucun profil actif : choisis ou crée un profil d’abord.",
            "No active profile: pick or create a profile first.",
        )
        .to_string(),
        ErreurControleur::TestEnCours => choisir(
            l,
            "Un test est en cours : termine-le ou arrête-le d’abord.",
            "A test is running: finish it or stop it first.",
        )
        .to_string(),
        ErreurControleur::BrouillonEnCours => choisir(
            l,
            "Enregistre ou annule d’abord tes changements dans l’égaliseur.",
            "Save or undo your changes in the equalizer first.",
        )
        .to_string(),
        ErreurControleur::MoteurAbsent => choisir(
            l,
            "Le moteur audio est introuvable : installe Equalizer APO pour lancer un test.",
            "The audio engine can’t be found: install Equalizer APO to start a test.",
        )
        .to_string(),
        ErreurControleur::MoteurNonAutorise => choisir(
            l,
            "Égaliseur pas encore activé : termine le premier lancement pour lancer un test.",
            "Equalizer not enabled yet: finish the first launch to start a test.",
        )
        .to_string(),
        ErreurControleur::ModeIndisponible => choisir(
            l,
            "Ce test n’est pas encore disponible.",
            "This test isn’t available yet.",
        )
        .to_string(),
        ErreurControleur::AutreProfilManquant => choisir(
            l,
            "Choisis l’autre profil à comparer.",
            "Pick the other profile to compare.",
        )
        .to_string(),
        ErreurControleur::FormatInconnu => choisir(
            l,
            "Format de fichier non reconnu.",
            "File format not recognised.",
        )
        .to_string(),
        ErreurControleur::ImportWavelet => choisir(
            l,
            "Un fichier Wavelet ne peut pas être importé : c’est un format d’export seulement.",
            "A Wavelet file can’t be imported: it’s an export-only format.",
        )
        .to_string(),
        ErreurControleur::AucunResultat => choisir(
            l,
            "Pas de résultat de test pour l’instant.",
            "No test result yet.",
        )
        .to_string(),
        ErreurControleur::Reprise(e) => match l {
            Langue::Fr => format!(
                "Impossible de reprendre le test interrompu : {}",
                erreur_reprise(e, l)
            ),
            Langue::En => format!(
                "Can’t resume the interrupted test: {}",
                erreur_reprise(e, l)
            ),
        },
        ErreurControleur::Fichier {
            action,
            chemin,
            source,
        } => {
            let verbe = match (action, l) {
                (ActionFichier::Lecture, Langue::Fr) => "Lecture de",
                (ActionFichier::Lecture, Langue::En) => "Reading",
                (ActionFichier::Ecriture, Langue::Fr) => "Écriture de",
                (ActionFichier::Ecriture, Langue::En) => "Writing",
            };
            format!("{verbe} {} : {source}", chemin.display())
        }
        ErreurControleur::BaseAutoEqReseau(e) => match l {
            Langue::Fr => format!(
                "Impossible de télécharger la base AutoEQ : {}. Ta base actuelle reste en place.",
                erreur_reseau(e, l)
            ),
            Langue::En => format!(
                "Couldn’t download the AutoEQ database: {}. Your current database is kept.",
                erreur_reseau(e, l)
            ),
        },
        ErreurControleur::CasqueAutoEqReseau(e) => {
            let hors_ligne = matches!(e, ErreurReseau::Connexion(_));
            let suite = match (hors_ligne, l) {
                (true, Langue::Fr) => {
                    " Sans connexion, seuls les casques déjà téléchargés peuvent être importés."
                }
                (true, Langue::En) => {
                    " Without a connection, only headphones already downloaded can be imported."
                }
                (false, _) => "",
            };
            match l {
                Langue::Fr => format!(
                    "Impossible de télécharger les réglages de ce casque : {}.{suite}",
                    erreur_reseau(e, l)
                ),
                Langue::En => format!(
                    "Couldn’t download this headphone’s settings: {}.{suite}",
                    erreur_reseau(e, l)
                ),
            }
        }
        ErreurControleur::BaseAutoEq(e) => match (e, l) {
            (ErreurBase::IndexVide, Langue::Fr) => "La base téléchargée ne contient aucun casque lisible : AutoEQ a peut-être changé de forme. Ta base actuelle reste en place.".to_string(),
            (ErreurBase::IndexVide, Langue::En) => "The downloaded database has no readable headphone: AutoEQ may have changed its layout. Your current database is kept.".to_string(),
            (_, Langue::Fr) => "La base AutoEQ gardée sur ce PC est illisible : télécharge-la de nouveau.".to_string(),
            (_, Langue::En) => "The AutoEQ database stored on this PC is unreadable: download it again.".to_string(),
        },
        ErreurControleur::ResultatAutoEqInconnu => choisir(
            l,
            "Ce résultat n’est plus dans la base : refais la recherche.",
            "This result is no longer in the database: search again.",
        )
        .to_string(),
    }
}

fn erreur_reseau(e: &ErreurReseau, l: Langue) -> String {
    match (e, l) {
        (ErreurReseau::Connexion(_), Langue::Fr) => {
            "pas de connexion ou serveur injoignable".to_string()
        }
        (ErreurReseau::Connexion(_), Langue::En) => {
            "no connection, or the server can’t be reached".to_string()
        }
        (ErreurReseau::Http(404), Langue::Fr) => {
            "ce fichier n’existe plus chez AutoEQ (erreur 404)".to_string()
        }
        (ErreurReseau::Http(404), Langue::En) => {
            "this file no longer exists at AutoEQ (error 404)".to_string()
        }
        (ErreurReseau::Http(code), Langue::Fr) => {
            format!("le serveur a répondu par l’erreur {code}")
        }
        (ErreurReseau::Http(code), Langue::En) => format!("the server answered with error {code}"),
        (ErreurReseau::TropVolumineuse, Langue::Fr) => {
            "réponse anormalement volumineuse, abandonnée".to_string()
        }
        (ErreurReseau::TropVolumineuse, Langue::En) => {
            "unusually large response, dropped".to_string()
        }
    }
}

fn erreur_depot(e: &ErreurDepot, l: Langue) -> String {
    match (e, l) {
        (ErreurDepot::Io { chemin, source }, Langue::Fr) => {
            format!("Erreur de fichier sur {} : {source}", chemin.display())
        }
        (ErreurDepot::Io { chemin, source }, Langue::En) => {
            format!("File error on {}: {source}", chemin.display())
        }
        (ErreurDepot::Json { chemin, source }, Langue::Fr) => {
            format!("JSON invalide dans {} : {source}", chemin.display())
        }
        (ErreurDepot::Json { chemin, source }, Langue::En) => {
            format!("Invalid JSON in {}: {source}", chemin.display())
        }
        (ErreurDepot::IdentifiantInvalide(id), Langue::Fr) => {
            format!("Identifiant invalide : {id:?}")
        }
        (ErreurDepot::IdentifiantInvalide(id), Langue::En) => {
            format!("Invalid identifier: {id:?}")
        }
    }
}

fn erreur_moteur(e: &ErreurMoteur, l: Langue) -> String {
    match (e, l) {
        (ErreurMoteur::DossierIntrouvable(dossier), Langue::Fr) => format!(
            "le dossier de configuration d’Equalizer APO est introuvable : {}",
            dossier.display()
        ),
        (ErreurMoteur::DossierIntrouvable(dossier), Langue::En) => format!(
            "Equalizer APO’s configuration folder can’t be found: {}",
            dossier.display()
        ),
        (ErreurMoteur::Io { action, source }, _) => {
            format!("{} : {source}", action_moteur(action, l))
        }
        (ErreurMoteur::CourbeInvalide(detail), Langue::Fr) => {
            format!("courbe invalide : {}", detail_courbe(detail, l))
        }
        (ErreurMoteur::CourbeInvalide(detail), Langue::En) => {
            format!("invalid curve: {}", detail_courbe(detail, l))
        }
        (ErreurMoteur::Registre(detail), Langue::Fr) => {
            format!("lecture du registre impossible : {detail}")
        }
        (ErreurMoteur::Registre(detail), Langue::En) => {
            format!("couldn’t read the registry: {detail}")
        }
        (ErreurMoteur::Simule(detail), Langue::Fr) => format!("échec simulé : {detail}"),
        (ErreurMoteur::Simule(detail), Langue::En) => format!("simulated failure: {detail}"),
    }
}

/// Ce que le moteur faisait quand un fichier lui a résisté. Le moteur l'écrit en français
/// (« écriture de … ») : on reconnaît le début et on le dit dans la langue demandée.
fn action_moteur(action: &str, l: Langue) -> String {
    const VERBES: [(&str, &str); 4] = [
        ("écriture de ", "writing "),
        ("lecture de ", "reading "),
        ("suppression de ", "deleting "),
        ("conservation de l'ancienne copie ", "keeping the old copy "),
    ];
    if l == Langue::En {
        for (fr, en) in VERBES {
            if let Some(reste) = action.strip_prefix(fr) {
                return majuscule(&format!("{en}{reste}"));
            }
        }
    }
    majuscule(action)
}

/// Le détail d'une courbe refusée par le moteur (écrit en français par le moteur).
fn detail_courbe(detail: &str, l: Langue) -> String {
    if l == Langue::Fr {
        return detail.to_string();
    }
    if detail == "préampli non fini" {
        return "the preamp is not a finite number".to_string();
    }
    match detail
        .strip_prefix("filtre ")
        .and_then(|reste| reste.split_once(" : "))
    {
        Some((numero, _)) => format!("filter {numero}: unusable frequency, gain or Q"),
        None => detail.to_string(),
    }
}

fn erreur_profil(e: &ErreurProfil, l: Langue) -> String {
    match (e, l) {
        (ErreurProfil::VersionInconnue(n), Langue::Fr) => {
            format!("Le profil n’a pas de version {n}")
        }
        (ErreurProfil::VersionInconnue(n), Langue::En) => {
            format!("The profile has no version {n}")
        }
        (ErreurProfil::Invalide(detail), Langue::Fr) => format!("Profil invalide : {detail}"),
        (ErreurProfil::Invalide(detail), Langue::En) => {
            let detail = match detail.as_str() {
                "aucune version" => "no version",
                "les numéros de version ne sont pas strictement croissants" => {
                    "version numbers are not strictly increasing"
                }
                autre => autre,
            };
            format!("Invalid profile: {detail}")
        }
    }
}

fn erreur_format(e: &ErreurFormat, l: Langue) -> String {
    match (e, l) {
        (ErreurFormat::AucuneLigneUtilisable, _) => choisir(
            l,
            "Aucune ligne utilisable : ni filtre ni préampli reconnu dans ce fichier",
            "No usable line: no filter or preamp recognised in this file",
        )
        .to_string(),
        (ErreurFormat::ContenuInvalide(detail), Langue::Fr) => {
            format!("Contenu invalide : {detail}")
        }
        (ErreurFormat::ContenuInvalide(detail), Langue::En) => {
            let detail = match detail.as_str() {
                "pas de section INI `[...]`, ce n'est pas un fichier .peace" => {
                    "no INI section `[...]`, this is not a .peace file"
                }
                autre => autre,
            };
            format!("Invalid content: {detail}")
        }
    }
}

fn erreur_reprise(e: &ErreurReprise, l: Langue) -> String {
    match (e, l) {
        (ErreurReprise::JournalVide, _) => {
            choisir(l, "le journal est vide", "the log is empty").to_string()
        }
        (ErreurReprise::SansDemarrage, _) => choisir(
            l,
            "le journal ne commence pas par le démarrage du test",
            "the log doesn’t start with the start of the test",
        )
        .to_string(),
        (ErreurReprise::TestClos, _) => choisir(
            l,
            "le test est déjà clos (terminé ou abandonné) : rien à reprendre",
            "the test is already closed (finished or abandoned): nothing to resume",
        )
        .to_string(),
        (ErreurReprise::MauvaiseVersion { attendue, fournie }, Langue::Fr) => format!(
            "la version fournie (n° {fournie}) n’est pas la référence du test (n° {attendue})"
        ),
        (ErreurReprise::MauvaiseVersion { attendue, fournie }, Langue::En) => format!(
            "the version supplied (no. {fournie}) is not the test’s reference (no. {attendue})"
        ),
        (ErreurReprise::Incoherent { entree, raison }, Langue::Fr) => {
            format!("journal incohérent à l’entrée {entree} : {raison}")
        }
        (ErreurReprise::Incoherent { entree, raison }, Langue::En) => {
            let raison = match *raison {
                "second démarrage" => "second start",
                "duel passé qui n'est pas le duel courant" => {
                    "skipped duel that isn’t the current one"
                }
                "annulation d'un duel sans réponse" => "undo of a duel with no answer",
                "réponse à un duel qui n'est pas le duel courant" => {
                    "answer to a duel that isn’t the current one"
                }
                "nouveau tirage d'un autre duel que le duel courant" => {
                    "new draw for a duel other than the current one"
                }
                "duel de vérification ajouté avant la fin du plan" => {
                    "verification duel added before the end of the plan"
                }
                autre => autre,
            };
            format!("inconsistent log at entry {entree}: {raison}")
        }
    }
}

// ----- Raccourcis et menus -----

/// Les lignes du tableau des raccourcis : (touches, action).
pub fn raccourcis(l: Langue) -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "F6",
            choisir(
                l,
                "Je préfère A (maintenir : réécouter A)",
                "I prefer A (hold: listen to A again)",
            ),
        ),
        ("F7", choisir(l, "Pareil", "Same")),
        (
            "F8",
            choisir(
                l,
                "Je préfère B (maintenir : réécouter B)",
                "I prefer B (hold: listen to B again)",
            ),
        ),
        ("F9", choisir(l, "Plus tard", "Later")),
        (
            "F10",
            choisir(l, "Annuler ma dernière réponse", "Undo my last answer"),
        ),
        (
            choisir(l, "Ctrl+Alt+Entrée", "Ctrl+Alt+Enter"),
            choisir(l, "Démarrer ou reprendre un test", "Start or resume a test"),
        ),
        (
            choisir(l, "Ctrl+Alt+1 à 4", "Ctrl+Alt+1 to 4"),
            choisir(l, "Choisir un profil", "Pick a profile"),
        ),
        (
            "Ctrl+Alt+0",
            choisir(
                l,
                "Couper ou remettre l’égaliseur",
                "Turn the equalizer off or on",
            ),
        ),
    ]
}

/// Le menu de l'icône dans la barre des tâches : (ouvrir, égaliseur actif, quitter).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn menu_barre(l: Langue) -> (&'static str, &'static str, &'static str) {
    (
        choisir(l, "Ouvrir Retro EQ", "Open Retro EQ"),
        choisir(l, "Égaliseur actif", "Equalizer on"),
        choisir(l, "Quitter", "Quit"),
    )
}

/// Le filtre de la boîte « Ouvrir un fichier » (Windows).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn filtre_fichiers(l: Langue) -> &'static str {
    choisir(
        l,
        "Égaliseur (Peace, Equalizer APO, AutoEQ)",
        "Equalizer (Peace, Equalizer APO, AutoEQ)",
    )
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

pub fn controle_sortie(nom: Option<&str>, l: Langue) -> TexteControle {
    match nom {
        Some(nom) => controle(
            choisir(l, "Sortie audio détectée", "Audio output detected"),
            match l {
                Langue::Fr => format!("Sortie par défaut : {}", entre_guillemets(nom, l)),
                Langue::En => format!("Default output: {}", entre_guillemets(nom, l)),
            },
            "",
        ),
        None => controle(
            choisir(
                l,
                "Aucune sortie audio détectée",
                "No audio output detected",
            ),
            choisir(
                l,
                "Branche ton casque, puis relance l’autotest.",
                "Plug in your headset, then run the self-test again.",
            ),
            "",
        ),
    }
}

/// `etat` : 0 absent, 1 inactif, 2 actif. `autorise` : l'utilisateur a accepté l'écriture.
pub fn controle_moteur(
    etat: i32,
    autorise: bool,
    inscriptible: Option<bool>,
    l: Langue,
) -> TexteControle {
    match (etat, autorise, inscriptible) {
        (0, _, _) => controle(
            choisir(l, "Moteur audio introuvable", "Audio engine not found"),
            choisir(
                l,
                "Equalizer APO n’est pas installé (ou pas là où Retro EQ le cherche). Sans lui, rien n’est égalisé.",
                "Equalizer APO isn’t installed (or not where Retro EQ looks for it). Without it, nothing is equalised.",
            ),
            choisir(l, "Télécharger Equalizer APO", "Download Equalizer APO"),
        ),
        (_, _, Some(false)) => controle(
            choisir(
                l,
                "Moteur audio : dossier protégé",
                "Audio engine: protected folder",
            ),
            choisir(
                l,
                "Retro EQ n’a pas le droit d’écrire dans le dossier config d’Equalizer APO.",
                "Retro EQ isn’t allowed to write to Equalizer APO’s config folder.",
            ),
            choisir(l, "Ouvrir le dossier", "Open the folder"),
        ),
        (2, true, _) => controle(
            choisir(l, "Moteur audio actif", "Audio engine active"),
            choisir(
                l,
                "Equalizer APO est installé et lit la courbe de Retro EQ.",
                "Equalizer APO is installed and reads Retro EQ’s curve.",
            ),
            "",
        ),
        (_, false, _) => controle(
            choisir(l, "Égaliseur pas encore activé", "Equalizer not enabled yet"),
            choisir(
                l,
                "Equalizer APO est installé. Retro EQ n’écrira dans sa configuration qu’après ton accord (bouton « Continuer »).",
                "Equalizer APO is installed. Retro EQ will only write to its configuration once you agree (“Continue” button).",
            ),
            "",
        ),
        _ => controle(
            choisir(l, "Moteur audio inactif", "Audio engine inactive"),
            choisir(
                l,
                "Equalizer APO est installé, mais ne lit pas encore la courbe de Retro EQ.",
                "Equalizer APO is installed, but doesn’t read Retro EQ’s curve yet.",
            ),
            "",
        ),
    }
}

pub fn controle_raccourcis(conflits: &[String], disponibles: bool, l: Langue) -> TexteControle {
    if !disponibles {
        controle(
            choisir(
                l,
                "Raccourcis globaux indisponibles",
                "Global shortcuts unavailable",
            ),
            choisir(
                l,
                "Le système n’a pas laissé Retro EQ enregistrer de raccourcis clavier.",
                "The system wouldn’t let Retro EQ register keyboard shortcuts.",
            ),
            "",
        )
    } else if conflits.is_empty() {
        controle(
            choisir(l, "Raccourcis libres", "Shortcuts available"),
            choisir(
                l,
                "Ctrl+Alt+Entrée, Ctrl+Alt+0 à 4. F6 à F10 ne sont pris que pendant un test.",
                "Ctrl+Alt+Enter, Ctrl+Alt+0 to 4. F6 to F10 are only taken during a test.",
            ),
            "",
        )
    } else {
        let liste = conflits.join(", ");
        controle(
            choisir(l, "Raccourcis déjà pris", "Shortcuts already taken"),
            match l {
                Langue::Fr => {
                    format!(
                        "Une autre appli utilise déjà : {liste}. Ferme-la ou libère ces touches."
                    )
                }
                Langue::En => format!(
                    "Another app already uses: {}. Close it or free those keys.",
                    liste.replace("Entrée", "Enter")
                ),
            },
            "",
        )
    }
}

/// Les contrôles que l'appli ne sait pas faire elle-même : elle dit où regarder, sans
/// afficher une coche verte qu'elle ne peut pas garantir.
pub fn controles_a_verifier_soi_meme(l: Langue) -> Vec<TexteControle> {
    let son = choisir(l, "Ouvrir les réglages du son", "Open the sound settings");
    vec![
        controle(
            choisir(
                l,
                "Égaliseur et son 360 d’INZONE Hub",
                "INZONE Hub equalizer and 360 sound",
            ),
            choisir(
                l,
                "Retro EQ ne peut pas les lire. Dans INZONE Hub : égaliseur sur « Plat », son spatial 360 désactivé.",
                "Retro EQ can’t read them. In INZONE Hub: equalizer set to “Flat”, 360 spatial sound off.",
            ),
            "",
        ),
        controle(
            choisir(l, "Son spatial de Windows", "Windows spatial sound"),
            choisir(
                l,
                "À vérifier toi-même : Paramètres › Système › Son › ta sortie › Son spatial sur « Désactivé ».",
                "Check this yourself: Settings › System › Sound › your output › Spatial sound set to “Off”.",
            ),
            son,
        ),
        controle(
            choisir(
                l,
                "Améliorations audio de Windows",
                "Windows audio enhancements",
            ),
            choisir(
                l,
                "À vérifier toi-même : Paramètres › Système › Son › ta sortie › Améliorations audio sur « Désactivé ».",
                "Check this yourself: Settings › System › Sound › your output › Audio enhancements set to “Off”.",
            ),
            son,
        ),
    ]
}

pub fn bilan_autotest(bons: usize, a_verifier: usize, erreurs: usize, l: Langue) -> String {
    let mut morceaux = vec![match l {
        Langue::Fr => format!("{bons} bons"),
        Langue::En => format!("{bons} good"),
    }];
    if a_verifier > 0 {
        morceaux.push(match l {
            Langue::Fr => format!("{a_verifier} à vérifier toi-même"),
            Langue::En => format!("{a_verifier} to check yourself"),
        });
    }
    if erreurs > 0 {
        morceaux.push(match l {
            Langue::Fr => format!("{erreurs} à régler"),
            Langue::En => format!("{erreurs} to fix"),
        });
    }
    morceaux.join(" · ")
}

// ----- Base AutoEQ -----

/// La source et le banc d'un format d'origine `autoeq|source|banc` (ADR-0013).
pub fn source_autoeq(format: &str) -> Option<(&str, Option<&str>)> {
    let mut morceaux = format.strip_prefix("autoeq|")?.splitn(2, '|');
    let source = morceaux.next().filter(|s| !s.is_empty())?;
    let banc = morceaux.next().filter(|b| !b.is_empty());
    Some((source, banc))
}

/// « source (banc) », ou seulement la source quand le banc n'est pas connu.
fn source_et_banc(source: &str, banc: Option<&str>) -> String {
    match banc {
        Some(banc) => format!("{source} ({banc})"),
        None => source.to_string(),
    }
}

/// Ce qu'on ajoute à « Import AutoEQ » : qui a mesuré le casque. Vide pour les autres formats.
fn mesures_autoeq(format: &str, l: Langue) -> String {
    match source_autoeq(format) {
        Some((source, banc)) => match l {
            Langue::Fr => format!(" · mesures {}", source_et_banc(source, banc)),
            Langue::En => format!(" · measurements by {}", source_et_banc(source, banc)),
        },
        None => String::new(),
    }
}

/// Le nom du profil créé depuis la base : « Sony INZONE H9 (Rtings) ». La source distingue
/// deux mesures du même casque.
pub fn nom_profil_autoeq(casque: &str, source: &str) -> String {
    format!("{casque} ({source})")
}

/// Le compte rendu d'un import depuis la base : celui d'un import de fichier, puis d'où
/// viennent les mesures et le rappel qu'un réglage AutoEQ est un point de départ.
pub fn annonce_autoeq(compte_rendu: &str, source: &str, banc: Option<&str>, l: Langue) -> String {
    let origine = source_et_banc(source, banc);
    match l {
        Langue::Fr => format!(
            "{compte_rendu} Mesures de {origine} : un point de départ, à juger à l’oreille."
        ),
        Langue::En => format!(
            "{compte_rendu} Measurements by {origine}: a starting point, to be judged by ear."
        ),
    }
}

/// « 8 850 casques · mise à jour le 25/11 » : ce que contient la base gardée.
pub fn resume_base_autoeq(nombre: usize, date: &str, l: Langue) -> String {
    let n = milliers(nombre, l);
    match l {
        Langue::Fr => format!("{n} casques · mise à jour le {date}"),
        Langue::En => format!("{n} headphones · updated {date}"),
    }
}

fn milliers(n: usize, l: Langue) -> String {
    let chiffres = n.to_string();
    let separateur = match l {
        Langue::Fr => '\u{202f}',
        Langue::En => ',',
    };
    let mut sortie = String::new();
    for (i, c) in chiffres.chars().enumerate() {
        if i > 0 && (chiffres.len() - i).is_multiple_of(3) {
            sortie.push(separateur);
        }
        sortie.push(c);
    }
    sortie
}

/// La ligne sous un résultat : « HMS II.3 · over-ear ».
pub fn detail_resultat_autoeq(banc: Option<&str>, type_casque: Option<&str>) -> String {
    [banc, type_casque]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

mod mises_a_jour;
pub use mises_a_jour::*;
