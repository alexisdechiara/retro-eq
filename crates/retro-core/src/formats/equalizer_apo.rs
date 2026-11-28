//! Format texte d'Equalizer APO : `Preamp: -6 dB` et `Filter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41`.
//!
//! # Ce qu'on lit
//!
//! - `PK`/`PEQ` (cloche) avec `Q` ou `BW Oct` ;
//! - `LSC`/`HSC` (plateau bas/haut, fréquence centrale) **avec un `Q`**.
//!
//! Une ligne n'est importée que si **tous** ses mots sont compris et si son sens
//! correspond exactement à ce que l'appli sait représenter. Sinon elle va au rapport
//! ([`Import::ignores`]), jamais appliquée à moitié.
//!
//! # Ce qu'on refuse, et pourquoi
//!
//! La référence d'Equalizer APO distingue `LSC`/`HSC` (fréquence centrale, pente en dB par
//! octave ou Q) de `LS`/`HS` (fréquence de coin, pente fixe de 6 ou 12 dB par octave). Elle ne
//! donne ni la formule qui lie la pente au Q, ni la pente par défaut, ni la définition de
//! la fréquence de coin (les exemples du Q et de la pente ne se recoupent avec aucune
//! formule usuelle). Sans formule exacte, on ne convertit pas : ces lignes (`LS`, `HS`,
//! plateaux réglés par une pente, plateaux sans `Q`) sont signalées par
//! [`RaisonIgnoree::PenteNonGeree`]. La largeur `BW Oct` d'une cloche est convertie en Q
//! par `Q = sqrt(2^bw) / (2^bw - 1)`.
//!
//! Un filtre à 22 050 Hz ou plus (Nyquist de 44,1 kHz) n'est pas importé
//! ([`RaisonIgnoree::FrequenceTropHaute`]) : une sortie audio à 44,1 kHz ne peut pas le jouer.
//! Un gain au-delà de ±30 dB, un Q hors de 0,01 à 100, ou un préampli dont la somme avec les
//! précédents sort de ±60 dB ne l'est pas non plus ([`RaisonIgnoree::HorsLimites`]).
//!
//! # Canaux et périphériques
//!
//! L'appli n'a qu'une courbe pour tous les canaux et tous les périphériques.
//! - Sous `Channel: all` (ou sans `Channel:`), les lignes sont importées.
//! - Sous un `Channel:` qui nomme des canaux : si toutes les sections par canal sont
//!   identiques et couvrent L et R (cas stéréo habituel), une seule copie est importée et
//!   les autres sont notées ([`RaisonIgnoree::CanalIdentique`]) ; sinon ces lignes sont
//!   signalées ([`RaisonIgnoree::CanalParticulier`]) et [`Import::partiel`] est vrai.
//! - Sous un `Device:` précis, les lignes sont signalées
//!   ([`RaisonIgnoree::PeripheriqueParticulier`]), sauf si le fichier ne contient qu'un seul
//!   `Device:`, tout au début : tout le fichier est alors pour ce périphérique et est importé
//!   (la ligne est notée, [`RaisonIgnoree::PeripheriqueUnique`]).
//!
//! Tout le reste (commentaires, `OFF`, `Include`, `LP`, `HP`, `NO`…) est mis de côté sans
//! erreur. Un `#` en milieu de ligne commence un commentaire (comme dans les exemples
//! d'Equalizer APO). On n'accepte que la syntaxe d'Equalizer APO : pas de `1k` pour 1000 Hz.

use super::{
    ecrire_nombre, gain_dans_les_limites, lire_nombre, preampli_dans_les_limites,
    q_dans_les_limites, ErreurFormat, Import, LigneIgnoree, RaisonIgnoree, FREQUENCE_MAX_HZ,
};
use retro_dsp::{Courbe, Filtre, TypeFiltre};

/// Ce qu'une ligne `Filter` ou `Preamp` apporte à la courbe.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Element {
    Preampli(f64),
    Filtre(Filtre),
}

/// Une ligne lue, avec de quoi la signaler.
#[derive(Debug, Clone)]
struct LigneLue {
    numero: usize,
    texte: String,
    element: Element,
}

/// Les canaux visés par un `Channel:`, en majuscules, triés, sans doublon.
/// `None` : tous les canaux.
type Canaux = Option<Vec<String>>;

/// Les lignes lues sous un même groupe de canaux.
struct SectionCanal {
    canaux: Vec<String>,
    lignes: Vec<LigneLue>,
}

fn noter(ignores: &mut Vec<LigneIgnoree>, numero: usize, texte: &str, raison: RaisonIgnoree) {
    ignores.push(LigneIgnoree {
        numero,
        texte: texte.to_string(),
        raison,
    });
}

/// Le texte d'une ligne sans son commentaire de fin de ligne.
fn sans_commentaire(ligne: &str) -> &str {
    ligne
        .split_once('#')
        .map_or(ligne, |(avant, _)| avant)
        .trim()
}

/// Vrai pour `all` seul ou dans une liste (`Device: a; all`).
fn vise_tout(valeur: &str, separateur: impl Fn(char) -> bool) -> bool {
    valeur
        .split(separateur)
        .any(|m| m.trim().eq_ignore_ascii_case("all"))
}

/// Lit un fichier Equalizer APO.
///
/// Rend une erreur seulement s'il n'y a ni préampli ni filtre utilisable.
pub fn lire(texte: &str) -> Result<Import, ErreurFormat> {
    let texte = texte.trim_start_matches('\u{feff}');
    let lignes: Vec<(usize, &str)> = texte
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .collect();
    let unique_device = device_unique_en_tete(&lignes);

    let mut courbe = Courbe::default();
    let mut ignores = Vec::new();
    let mut preampli_vu = false;
    let mut canal: Canaux = None;
    let mut device_precis = false;
    let mut sections: Vec<SectionCanal> = Vec::new();
    let mut appliquees: Vec<LigneLue> = Vec::new();

    for &(numero, ligne) in &lignes {
        if ligne.is_empty() {
            continue;
        }
        if ligne.starts_with('#') {
            noter(&mut ignores, numero, ligne, RaisonIgnoree::Commentaire);
            continue;
        }
        let utile = sans_commentaire(ligne);
        let Some((mot, reste)) = utile.split_once(':') else {
            noter(&mut ignores, numero, ligne, RaisonIgnoree::Illisible);
            continue;
        };
        let mot = mot.trim().to_lowercase();
        if mot == "channel" {
            let noms: Vec<&str> = reste.split_whitespace().collect();
            if noms.is_empty() {
                noter(&mut ignores, numero, ligne, RaisonIgnoree::Illisible);
            } else if noms.iter().any(|n| n.eq_ignore_ascii_case("all")) {
                canal = None;
            } else {
                let mut liste: Vec<String> = noms.iter().map(|n| nom_de_canal(n)).collect();
                liste.sort();
                liste.dedup();
                canal = Some(liste);
            }
            continue;
        }
        if mot == "device" {
            if unique_device == Some(numero) {
                noter(
                    &mut ignores,
                    numero,
                    ligne,
                    RaisonIgnoree::PeripheriqueUnique,
                );
            } else {
                device_precis = !vise_tout(reste, |c| c == ';');
            }
            continue;
        }
        let element = if mot == "preamp" {
            match valeur_en_db(reste) {
                Some(db) => Element::Preampli(db),
                None => {
                    noter(&mut ignores, numero, ligne, RaisonIgnoree::Illisible);
                    continue;
                }
            }
        } else if est_un_mot_filtre(&mot) {
            match lire_filtre(reste) {
                Ok(Some(filtre)) => Element::Filtre(filtre),
                Ok(None) => {
                    noter(&mut ignores, numero, ligne, RaisonIgnoree::Desactive);
                    continue;
                }
                Err(raison) => {
                    noter(&mut ignores, numero, ligne, raison);
                    continue;
                }
            }
        } else {
            noter(&mut ignores, numero, ligne, RaisonIgnoree::CommandeNonGeree);
            continue;
        };
        let lue = LigneLue {
            numero,
            texte: ligne.to_string(),
            element,
        };
        if device_precis {
            noter(
                &mut ignores,
                numero,
                ligne,
                RaisonIgnoree::PeripheriqueParticulier,
            );
        } else if let Some(canaux) = &canal {
            match sections.iter_mut().find(|s| &s.canaux == canaux) {
                Some(section) => section.lignes.push(lue),
                None => sections.push(SectionCanal {
                    canaux: canaux.clone(),
                    lignes: vec![lue],
                }),
            }
        } else {
            appliquees.push(lue);
        }
    }

    appliquees.extend(retenir_sections(sections, &mut ignores));
    for ligne in &appliquees {
        match ligne.element {
            Element::Preampli(db) => {
                // Equalizer APO additionne les préamplis. Chaque valeur est finie, mais pas
                // forcément leur somme : c'est elle qu'on vérifie.
                let somme = courbe.preampli_db + db;
                if preampli_dans_les_limites(somme) {
                    courbe.preampli_db = somme;
                    preampli_vu = true;
                } else {
                    noter(
                        &mut ignores,
                        ligne.numero,
                        &ligne.texte,
                        RaisonIgnoree::HorsLimites,
                    );
                }
            }
            Element::Filtre(filtre) => courbe.filtres.push(filtre),
        }
    }
    ignores.sort_by_key(|l| l.numero);

    if !preampli_vu && courbe.filtres.is_empty() {
        return Err(ErreurFormat::AucuneLigneUtilisable);
    }
    Ok(Import::nouveau(courbe, ignores))
}

/// `1` et `2` sont `L` et `R` en stéréo ; le reste est comparé en majuscules.
fn nom_de_canal(nom: &str) -> String {
    match nom {
        "1" => "L".to_string(),
        "2" => "R".to_string(),
        autre => autre.to_uppercase(),
    }
}

/// Le numéro de la ligne `Device:` si le fichier n'en a qu'une, précise, avant toute
/// ligne `Filter`/`Preamp`/`Channel` : c'est alors le périphérique de tout le fichier.
fn device_unique_en_tete(lignes: &[(usize, &str)]) -> Option<usize> {
    let mut devices = lignes.iter().filter_map(|&(n, l)| {
        let (mot, reste) = sans_commentaire(l).split_once(':')?;
        mot.trim()
            .eq_ignore_ascii_case("device")
            .then(|| (n, reste.to_string()))
    });
    let (numero, valeur) = devices.next()?;
    if devices.next().is_some() || vise_tout(&valeur, |c| c == ';') {
        return None;
    }
    let avant_contenu = lignes
        .iter()
        .take_while(|&&(n, _)| n < numero)
        .all(|&(_, l)| {
            let mot = sans_commentaire(l)
                .split_once(':')
                .map(|(m, _)| m.trim().to_lowercase())
                .unwrap_or_default();
            !(mot == "preamp" || mot == "channel" || est_un_mot_filtre(&mot))
        });
    avant_contenu.then_some(numero)
}

/// Décide quoi faire des sections par canal : rend les lignes à appliquer et
/// remplit le rapport pour les autres.
fn retenir_sections(sections: Vec<SectionCanal>, ignores: &mut Vec<LigneIgnoree>) -> Vec<LigneLue> {
    let Some(premiere) = sections.first() else {
        return Vec::new();
    };
    let elements =
        |s: &SectionCanal| -> Vec<Element> { s.lignes.iter().map(|l| l.element).collect() };
    let identiques = sections.iter().all(|s| elements(s) == elements(premiere));
    let couvre = |nom: &str| sections.iter().any(|s| s.canaux.iter().any(|c| c == nom));
    if identiques && couvre("L") && couvre("R") {
        let mut sections = sections.into_iter();
        let retenue = sections.next().map(|s| s.lignes).unwrap_or_default();
        for section in sections {
            for l in section.lignes {
                noter(ignores, l.numero, &l.texte, RaisonIgnoree::CanalIdentique);
            }
        }
        retenue
    } else {
        for section in sections {
            for l in section.lignes {
                noter(ignores, l.numero, &l.texte, RaisonIgnoree::CanalParticulier);
            }
        }
        Vec::new()
    }
}

/// Vrai pour `filter`, `filter 3`, `filter3`.
fn est_un_mot_filtre(mot: &str) -> bool {
    match mot.strip_prefix("filter") {
        Some(suite) => suite.trim().chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

/// Lit `-6 dB` (l'unité est facultative).
fn valeur_en_db(texte: &str) -> Option<f64> {
    let mut mots = texte.split_whitespace();
    let valeur = lire_nombre(mots.next()?)?;
    match mots.next() {
        None => Some(valeur),
        Some(unite) if unite.eq_ignore_ascii_case("db") && mots.next().is_none() => Some(valeur),
        Some(_) => None,
    }
}

/// Ce que contient une ligne `Filter` une fois tous ses mots lus.
#[derive(Default)]
struct MotsFiltre {
    frequence: Option<f64>,
    gain: Option<f64>,
    q: Option<f64>,
    /// Largeur `BW Oct`, en octaves.
    largeur: Option<f64>,
    /// Pente donnée juste après le type (`LSC 10.8 dB`, `LS 6dB`), en dB par octave.
    pente: Option<f64>,
}

/// Lit une valeur et son unité facultative : `Fc 100 Hz`, `Gain 3 dB`.
/// Rend la valeur et le nombre de mots consommés.
fn valeur_avec_unite(mots: &[&str], i: usize, unite: &str) -> Option<(f64, usize)> {
    let valeur = lire_nombre(mots.get(i + 1)?)?;
    let avec_unite = mots
        .get(i + 2)
        .is_some_and(|m| m.eq_ignore_ascii_case(unite));
    Some((valeur, if avec_unite { 3 } else { 2 }))
}

/// Lit les mots après le type. `None` au moindre mot incompris ou répété :
/// on ne devine rien.
fn lire_mots(mots: &[&str]) -> Option<MotsFiltre> {
    let mut lu = MotsFiltre::default();
    let mut i = 0;
    while i < mots.len() {
        let mot = mots[i].to_lowercase();
        let deja = |v: &Option<f64>| v.is_some();
        match mot.as_str() {
            "fc" if !deja(&lu.frequence) => {
                let (v, pas) = valeur_avec_unite(mots, i, "hz")?;
                lu.frequence = Some(v);
                i += pas;
            }
            "gain" if !deja(&lu.gain) => {
                let (v, pas) = valeur_avec_unite(mots, i, "db")?;
                lu.gain = Some(v);
                i += pas;
            }
            "q" if !deja(&lu.q) => {
                lu.q = Some(lire_nombre(mots.get(i + 1)?)?);
                i += 2;
            }
            "bw" if !deja(&lu.largeur) => {
                // `BW Oct 0.5` : largeur en octaves.
                if !mots.get(i + 1)?.eq_ignore_ascii_case("oct") {
                    return None;
                }
                lu.largeur = Some(lire_nombre(mots.get(i + 2)?)?);
                i += 3;
            }
            _ if i == 0 && lu.pente.is_none() => {
                // Pente : `6dB` d'un seul mot, ou `10.8 dB` en deux.
                if let Some(n) = mot.strip_suffix("db").and_then(lire_nombre) {
                    lu.pente = Some(n);
                    i += 1;
                } else if mots.get(1).is_some_and(|m| m.eq_ignore_ascii_case("db")) {
                    lu.pente = Some(lire_nombre(mots[0])?);
                    i += 2;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(lu)
}

/// Lit ce qui suit `Filter n:`.
///
/// `Ok(None)` : le filtre est `OFF`.
fn lire_filtre(reste: &str) -> Result<Option<Filtre>, RaisonIgnoree> {
    let mots: Vec<&str> = reste.split_whitespace().collect();
    let mut position = 0;
    match mots.first().map(|m| m.to_uppercase()).as_deref() {
        Some("OFF") => return Ok(None),
        Some("ON") => position = 1,
        _ => {}
    }
    let code = mots
        .get(position)
        .ok_or(RaisonIgnoree::Illisible)?
        .to_uppercase();
    let (type_filtre, est_plateau, coin) = match code.as_str() {
        "PK" | "PEQ" => (TypeFiltre::Cloche, false, false),
        "LSC" => (TypeFiltre::PlateauBas, true, false),
        "HSC" => (TypeFiltre::PlateauHaut, true, false),
        "LS" => (TypeFiltre::PlateauBas, true, true),
        "HS" => (TypeFiltre::PlateauHaut, true, true),
        _ => return Err(RaisonIgnoree::TypeFiltreNonGere(code)),
    };

    let lu = lire_mots(&mots[position + 1..]).ok_or(RaisonIgnoree::Illisible)?;
    let frequence_hz = lu
        .frequence
        .filter(|f| *f > 0.0)
        .ok_or(RaisonIgnoree::Illisible)?;
    if frequence_hz >= FREQUENCE_MAX_HZ {
        return Err(RaisonIgnoree::FrequenceTropHaute);
    }
    let gain_db = lu.gain.ok_or(RaisonIgnoree::Illisible)?;

    let q = if est_plateau {
        // `LS`/`HS` : fréquence de coin ; `LSC`/`HSC` réglé par une pente, une largeur ou
        // sans Q : le sens exact n'est pas documenté. On ne convertit pas.
        match (coin, lu.q, lu.pente, lu.largeur) {
            (false, Some(q), None, None) if q > 0.0 => q,
            (false, Some(_), None, None) => return Err(RaisonIgnoree::Illisible),
            (false, Some(_), _, _) => return Err(RaisonIgnoree::Illisible),
            _ => return Err(RaisonIgnoree::PenteNonGeree),
        }
    } else {
        // Une cloche n'a ni pente, ni les deux largeurs à la fois.
        match (lu.q, lu.largeur, lu.pente) {
            (Some(q), None, None) if q > 0.0 => q,
            (None, Some(bw), None) => q_depuis_octaves(bw).ok_or(RaisonIgnoree::Illisible)?,
            _ => return Err(RaisonIgnoree::Illisible),
        }
    };
    if !gain_dans_les_limites(gain_db) || !q_dans_les_limites(q) {
        return Err(RaisonIgnoree::HorsLimites);
    }
    Ok(Some(Filtre {
        type_filtre,
        frequence_hz,
        gain_db,
        q,
    }))
}

/// Passe d'une largeur en octaves à un Q : `Q = sqrt(2^bw) / (2^bw - 1)`.
fn q_depuis_octaves(octaves: f64) -> Option<f64> {
    if octaves <= 0.0 {
        return None;
    }
    let p = 2f64.powf(octaves);
    Some(p.sqrt() / (p - 1.0))
}

/// Écrit une courbe au format Equalizer APO, une ligne par filtre.
pub fn ecrire(courbe: &Courbe) -> String {
    let mut sortie = format!("Preamp: {} dB\n", ecrire_nombre(courbe.preampli_db));
    for (i, f) in courbe.filtres.iter().enumerate() {
        let code = match f.type_filtre {
            TypeFiltre::Cloche => "PK",
            TypeFiltre::PlateauBas => "LSC",
            TypeFiltre::PlateauHaut => "HSC",
        };
        sortie.push_str(&format!(
            "Filter {}: ON {} Fc {} Hz Gain {} dB Q {}\n",
            i + 1,
            code,
            ecrire_nombre(f.frequence_hz),
            ecrire_nombre(f.gain_db),
            ecrire_nombre(f.q),
        ));
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::outils_de_test::{courbe_melangee, proches};
    use crate::formats::RaisonIgnoree;
    use retro_dsp::TypeFiltre;

    fn lire_ok(texte: &str) -> Import {
        lire(texte).expect("le texte doit se lire")
    }

    #[test]
    fn lit_le_cas_simple() {
        let i = lire_ok("Preamp: -6 dB\nFilter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41\n");
        assert_eq!(i.courbe.preampli_db, -6.0);
        assert_eq!(i.courbe.filtres.len(), 1);
        let f = i.courbe.filtres[0];
        assert_eq!(f.type_filtre, TypeFiltre::Cloche);
        assert_eq!(f.frequence_hz, 1000.0);
        assert_eq!(f.gain_db, -3.0);
        assert_eq!(f.q, 1.41);
        assert!(i.ignores.is_empty());
    }

    #[test]
    fn lit_les_types_de_plateaux() {
        let i = lire_ok(
            "Filter 1: ON LSC Fc 100 Hz Gain 2 dB Q 0.5\n\
             Filter 2: ON HSC Fc 9000 Hz Gain -2 dB Q 0.6\n\
             Filter 3: ON PEQ Fc 500 Hz Gain 1 dB Q 2\n",
        );
        let types: Vec<_> = i.courbe.filtres.iter().map(|f| f.type_filtre).collect();
        assert_eq!(
            types,
            [
                TypeFiltre::PlateauBas,
                TypeFiltre::PlateauHaut,
                TypeFiltre::Cloche
            ]
        );
    }

    /// Le rapport d'un texte qui contient aussi une cloche importable.
    fn raisons(texte: &str) -> (Import, Vec<RaisonIgnoree>) {
        let i = lire_ok(&format!(
            "{texte}\nFilter 99: ON PK Fc 1000 Hz Gain 1 dB Q 1\n"
        ));
        let r = i.ignores.iter().map(|l| l.raison.clone()).collect();
        (i, r)
    }

    #[test]
    fn ls_et_hs_ne_sont_plus_devines_ils_vont_au_rapport() {
        // Fréquence de coin et pente par défaut non documentées : aucune approximation silencieuse.
        let (i, r) = raisons(
            "Filter 1: ON LS Fc 100 Hz Gain 2 dB\nFilter 2: ON HS Fc 9000 Hz Gain -2 dB\n\
             Filter 3: ON LS Fc 100 Hz Gain 2 dB Q 0.9\nFilter 4: ON HS 12dB Fc 8000 Hz Gain -2 dB",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(r, vec![RaisonIgnoree::PenteNonGeree; 4]);
        assert!(i.partiel);
    }

    #[test]
    fn les_plateaux_avec_pente_ou_sans_q_ne_sont_pas_approximes() {
        let (i, r) = raisons(
            "Filter 1: ON LSC 10.8 dB Fc 300 Hz Gain 5 dB\n\
             Filter 2: ON LS 6dB Fc 50 Hz Gain 3 dB\n\
             Filter 3: ON HS 12dB Fc 8000 Hz Gain -2 dB\n\
             Filter 4: ON HSC 6 dB Fc 100 Hz Gain -6 dB\n\
             Filter 5: ON LSC Fc 100 Hz Gain 2 dB\n\
             Filter 6: ON HSC Fc 100 Hz Gain 2 dB BW Oct 1",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(r, vec![RaisonIgnoree::PenteNonGeree; 6]);
    }

    #[test]
    fn un_mot_inconnu_dans_une_ligne_la_rend_illisible_au_lieu_d_etre_saute() {
        let (i, r) = raisons(
            "Filter 1: ON PK Fc 1000 Hz Gain 3 dB Q 1 Foo 3\n\
             Filter 2: ON LSC Fc 100 Hz Gain 3 dB Q 0.7 Slope 6\n\
             Filter 3: ON PK Fc 1000 Hz Gain 3 dB Q 1 Q 2\n\
             Filter 4: ON PK 6 dB Fc 1000 Hz Gain 3 dB Q 1\n\
             Filter 5: ON PK Fc 1000 Hz Gain 3 dB Q 1 BW Oct 1\n\
             Filter 6: ON PK Fc 1000 kHz Gain 3 dB Q 1",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(r, vec![RaisonIgnoree::Illisible; 6]);
    }

    #[test]
    fn lsc_et_hsc_avec_q_et_unites_facultatives_sont_importes() {
        let i = lire_ok(
            "Filter 1: ON LSC Fc 100 Hz Gain 2 dB Q 0.9\nFilter 2: ON HSC Fc 9000 Gain -2 Q 0.5\n",
        );
        assert_eq!(i.courbe.filtres.len(), 2);
        assert_eq!(i.courbe.filtres[0].q, 0.9);
        assert_eq!(i.courbe.filtres[1].q, 0.5);
        assert!(i.ignores.is_empty());
        assert!(!i.partiel);
    }

    #[test]
    fn un_commentaire_en_fin_de_ligne_est_ignore() {
        let i = lire_ok("Filter 1: ON PK Fc 1000 Hz Gain 3 dB Q 1 # graves\n");
        assert_eq!(i.courbe.filtres.len(), 1);
        assert!(i.ignores.is_empty());
    }

    // --- canaux et périphériques ---

    #[test]
    fn channel_all_et_absence_de_channel_s_appliquent() {
        let i = lire_ok(
            "Channel: all\nFilter 1: ON PK Fc 100 Hz Gain 1 dB Q 1\nChannel: L R\n\
             Filter 2: ON PK Fc 200 Hz Gain 1 dB Q 1\nChannel: ALL\nPreamp: -1 dB\n",
        );
        // La section « L R » couvre L et R : une seule section, importée.
        assert_eq!(i.courbe.filtres.len(), 2);
        assert_eq!(i.courbe.preampli_db, -1.0);
        assert!(i.ignores.is_empty());
        assert!(!i.partiel);
    }

    #[test]
    fn sections_gauche_et_droite_identiques_donnent_une_seule_copie() {
        let i = lire_ok(
            "Channel: L\nFilter 1: ON PK Fc 100 Hz Gain 6 dB Q 1\n\
             Channel: R\nFilter 2: ON PK Fc 100 Hz Gain 6 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1, "+6 dB, pas +12");
        assert_eq!(i.courbe.filtres[0].gain_db, 6.0);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].numero, 4);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::CanalIdentique);
        assert!(!i.partiel);
    }

    #[test]
    fn sections_differentes_ne_sont_pas_importees_et_le_signalent() {
        let i = lire_ok(
            "Filter 1: ON PK Fc 50 Hz Gain 2 dB Q 1\n\
             Channel: L\nFilter 2: ON PK Fc 100 Hz Gain 6 dB Q 1\n\
             Channel: R\nFilter 3: ON PK Fc 100 Hz Gain -6 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.courbe.filtres[0].frequence_hz, 50.0);
        assert_eq!(i.ignores.len(), 2);
        assert!(i
            .ignores
            .iter()
            .all(|l| l.raison == RaisonIgnoree::CanalParticulier));
        assert!(i.partiel);
    }

    #[test]
    fn un_seul_canal_precis_n_est_pas_applique_a_tous() {
        let i = lire_ok("Preamp: -1 dB\nChannel: L\nFilter 1: ON PK Fc 100 Hz Gain 6 dB Q 1\n");
        assert!(i.courbe.filtres.is_empty());
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::CanalParticulier);
        assert!(i.partiel);
    }

    #[test]
    fn des_sections_par_canal_en_plus_du_commun_gardent_le_commun_et_une_copie() {
        let i = lire_ok(
            "Preamp: -3 dB\nFilter 1: ON PK Fc 60 Hz Gain 2 dB Q 1\n\
             Channel: 1\nPreamp: -2 dB\nChannel: 2\nPreamp: -2 dB\n",
        );
        // Équivalent à « -5 dB sur chaque oreille » : -3 commun + -2 une seule fois.
        assert_eq!(i.courbe.preampli_db, -5.0);
        assert!(!i.partiel);
    }

    #[test]
    fn un_device_unique_en_tete_n_empeche_pas_l_import_mais_est_note() {
        let i =
            lire_ok("Device: Casque USB\nPreamp: -3 dB\nFilter 1: ON PK Fc 60 Hz Gain 2 dB Q 1\n");
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.courbe.preampli_db, -3.0);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::PeripheriqueUnique);
        assert!(!i.partiel);
    }

    #[test]
    fn les_lignes_apres_un_device_precis_ne_sont_pas_appliquees() {
        let i = lire_ok(
            "Filter 1: ON PK Fc 60 Hz Gain 2 dB Q 1\nDevice: Casque USB\n\
             Filter 2: ON PK Fc 80 Hz Gain 2 dB Q 1\nDevice: all\nFilter 3: ON PK Fc 90 Hz Gain 1 dB Q 1\n",
        );
        let freqs: Vec<_> = i.courbe.filtres.iter().map(|f| f.frequence_hz).collect();
        assert_eq!(freqs, [60.0, 90.0]);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::PeripheriqueParticulier);
        assert!(i.partiel);
    }

    #[test]
    fn deux_devices_precis_ne_comptent_pas_comme_un_device_unique() {
        let i = lire_ok(
            "Device: A\nFilter 1: ON PK Fc 60 Hz Gain 2 dB Q 1\nDevice: B\n\
             Filter 2: ON PK Fc 80 Hz Gain 2 dB Q 1\nDevice: all\nPreamp: -1 dB\n",
        );
        assert_eq!(i.courbe.filtres.len(), 0);
        assert_eq!(i.courbe.preampli_db, -1.0);
        assert!(i.partiel);
    }

    #[test]
    fn partiel_est_faux_pour_commentaires_lignes_off_et_vrai_pour_un_include() {
        let i = lire_ok("# x\nFilter 1: OFF PK Fc 1 Hz Gain 1 dB Q 1\nPreamp: -1 dB\n");
        assert!(!i.partiel);
        let i = lire_ok("Include: autre.txt\nPreamp: -1 dB\n");
        assert!(i.partiel);
    }

    #[test]
    fn tolere_filter_sans_numero_espaces_casse_et_crlf() {
        let i = lire_ok(
            "preamp:   -4.5   db\r\n\
             FILTER:  on   pk   fc   200   hz   gain   3   db   q   1.0\r\n\
             filter 12 : ON Pk Fc 2000 HZ Gain -1 DB Q 2\r\n",
        );
        assert_eq!(i.courbe.preampli_db, -4.5);
        assert_eq!(i.courbe.filtres.len(), 2);
        assert_eq!(i.courbe.filtres[0].frequence_hz, 200.0);
        assert_eq!(i.courbe.filtres[1].gain_db, -1.0);
    }

    #[test]
    fn tolere_le_bom_utf8() {
        let i = lire_ok("\u{feff}Preamp: -3 dB\nFilter 1: ON PK Fc 100 Hz Gain 1 dB Q 1\n");
        assert_eq!(i.courbe.preampli_db, -3.0);
        assert_eq!(i.courbe.filtres.len(), 1);
    }

    #[test]
    fn tolere_la_virgule_decimale() {
        let i = lire_ok("Preamp: -3,5 dB\nFilter 1: ON PK Fc 1000,5 Hz Gain -2,25 dB Q 1,41\n");
        assert_eq!(i.courbe.preampli_db, -3.5);
        let f = i.courbe.filtres[0];
        assert_eq!(f.frequence_hz, 1000.5);
        assert_eq!(f.gain_db, -2.25);
        assert_eq!(f.q, 1.41);
    }

    #[test]
    fn lit_la_largeur_en_octaves() {
        // Q = sqrt(2^bw) / (2^bw - 1) ; bw = 1 octave donne Q = sqrt(2).
        let i = lire_ok("Filter 1: ON PK Fc 1000 Hz Gain 3 dB BW Oct 1\n");
        assert!((i.courbe.filtres[0].q - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn les_lignes_off_sont_ignorees_et_listees() {
        let i = lire_ok(
            "Filter 1: OFF PK Fc 1000 Hz Gain -3 dB Q 1.41\nFilter 2: ON PK Fc 2000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].numero, 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Desactive);
        assert!(i.ignores[0].texte.starts_with("Filter 1: OFF"));
    }

    #[test]
    fn les_commentaires_sont_ignores_et_listes() {
        let i = lire_ok("# mon profil\nFilter 1: ON PK Fc 1000 Hz Gain 1 dB Q 1\n");
        assert_eq!(i.ignores.len(), 1);
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Commentaire);
        assert_eq!(i.ignores[0].numero, 1);
    }

    #[test]
    fn les_commandes_inconnues_sont_ignorees_sans_erreur() {
        let i = lire_ok(
            "Include: autre.txt\nStage: pre-mix\nCopy: L=R\nGraphicEQ: 20 0; 100 1\n\
             Filter 1: ON PK Fc 1000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 4);
        assert!(i
            .ignores
            .iter()
            .all(|l| l.raison == RaisonIgnoree::CommandeNonGeree));
    }

    #[test]
    fn les_autres_types_de_filtre_sont_ignores_avec_leur_code() {
        let i = lire_ok(
            "Filter 1: ON LP Fc 15000 Hz\nFilter 2: ON HP Fc 20 Hz\nFilter 3: ON NO Fc 50 Hz\n\
             Filter 4: ON AP Fc 50 Hz Q 1\nFilter 5: ON PK Fc 1000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        let codes: Vec<_> = i.ignores.iter().map(|l| l.raison.clone()).collect();
        assert_eq!(
            codes,
            [
                RaisonIgnoree::TypeFiltreNonGere("LP".into()),
                RaisonIgnoree::TypeFiltreNonGere("HP".into()),
                RaisonIgnoree::TypeFiltreNonGere("NO".into()),
                RaisonIgnoree::TypeFiltreNonGere("AP".into()),
            ]
        );
    }

    #[test]
    fn une_ligne_filtre_cassee_est_ignoree_comme_illisible() {
        let i = lire_ok(
            "Filter 1: ON PK Fc abc Hz Gain 1 dB Q 1\nFilter 2: ON PK Fc 1000 Hz Q 1\n\
             Filter 3: ON PK Fc 1000 Hz Gain 1 dB Q 0\nFilter 4: ON PK Fc 1000 Hz Gain 1 dB Q 1\n",
        );
        assert_eq!(i.courbe.filtres.len(), 1);
        assert_eq!(i.ignores.len(), 3);
        assert!(i
            .ignores
            .iter()
            .all(|l| l.raison == RaisonIgnoree::Illisible));
    }

    #[test]
    fn n_invente_pas_la_syntaxe_1k() {
        let i = lire_ok("Preamp: 0 dB\nFilter 1: ON PK Fc 1k Hz Gain 1 dB Q 1\n");
        assert!(i.courbe.filtres.is_empty());
        assert_eq!(i.ignores[0].raison, RaisonIgnoree::Illisible);
    }

    #[test]
    fn les_preamps_s_additionnent_comme_dans_equalizer_apo() {
        let i = lire_ok("Preamp: -2 dB\nPreamp: -1.5 dB\n");
        assert_eq!(i.courbe.preampli_db, -3.5);
    }

    #[test]
    fn un_fichier_sans_ligne_utilisable_donne_une_erreur() {
        for texte in [
            "",
            "\n\n",
            "# rien\n",
            "Include: x.txt\n",
            "Filter 1: OFF PK Fc 1 Hz Gain 1 dB Q 1\n",
            "n'importe quoi",
        ] {
            assert_eq!(
                lire(texte),
                Err(ErreurFormat::AucuneLigneUtilisable),
                "{texte:?}"
            );
        }
    }

    #[test]
    fn des_preamplis_dont_la_somme_deborde_ne_donnent_jamais_un_preampli_infini() {
        // Chaque valeur est finie, mais leur somme ne l'est pas.
        assert_eq!(
            lire("Preamp: 1e308 dB\nPreamp: 1e308 dB\n"),
            Err(ErreurFormat::AucuneLigneUtilisable)
        );
        let (i, r) = raisons("Preamp: 1e308 dB\nPreamp: 1e308 dB");
        assert_eq!(r, [RaisonIgnoree::HorsLimites, RaisonIgnoree::HorsLimites]);
        assert_eq!(i.courbe.preampli_db, 0.0);
        assert!(i.partiel);
        // La somme est vérifiée, pas seulement chaque ligne.
        let (i, r) = raisons("Preamp: -40 dB\nPreamp: -40 dB");
        assert_eq!(r, [RaisonIgnoree::HorsLimites]);
        assert_eq!(i.courbe.preampli_db, -40.0);
        assert_eq!(i.ignores[0].numero, 2);
    }

    #[test]
    fn un_gain_ou_un_q_hors_limites_est_signale() {
        for ligne in [
            "Filter 1: ON PK Fc 1000 Hz Gain 1e300 dB Q 1",
            "Filter 1: ON PK Fc 1000 Hz Gain 31 dB Q 1",
            "Filter 1: ON LSC Fc 100 Hz Gain -31 dB Q 0.7",
            "Filter 1: ON PK Fc 1000 Hz Gain 3 dB Q 1000",
            "Filter 1: ON PK Fc 1000 Hz Gain 3 dB Q 0.001",
            "Filter 1: ON PK Fc 1000 Hz Gain 3 dB BW Oct 2000",
            "Filter 1: ON PK Fc 1000 Hz Gain 3 dB BW Oct 1e-12",
        ] {
            let (i, r) = raisons(ligne);
            assert_eq!(r, [RaisonIgnoree::HorsLimites], "{ligne}");
            assert_eq!(i.courbe.filtres.len(), 1, "{ligne}");
            assert!(i.partiel);
        }
        // Les bornes elles-mêmes sont acceptées.
        let i = lire_ok(
            "Preamp: -60 dB\nFilter 1: ON PK Fc 1000 Hz Gain 30 dB Q 100\n\
             Filter 2: ON PK Fc 1000 Hz Gain -30 dB Q 0.01\n",
        );
        assert_eq!(i.courbe.filtres.len(), 2);
        assert!(i.ignores.is_empty());
    }

    #[test]
    fn un_filtre_a_ou_au_dela_de_nyquist_est_signale_au_lieu_d_etre_importe() {
        for hz in ["30000", "22050", "96000"] {
            let (i, r) = raisons(&format!("Filter 1: ON PK Fc {hz} Hz Gain 6 dB Q 1"));
            assert_eq!(r, [RaisonIgnoree::FrequenceTropHaute], "{hz}");
            assert_eq!(
                i.courbe.filtres.len(),
                1,
                "seule la cloche témoin est importée"
            );
            assert!(i.partiel);
        }
        // Juste sous Nyquist à 44,1 kHz : jouable partout, importé.
        let i = lire_ok("Filter 1: ON HSC Fc 22000 Hz Gain 2 dB Q 0.7\n");
        assert_eq!(i.courbe.filtres.len(), 1);
        assert!(!i.partiel);
    }

    #[test]
    fn un_preampli_seul_est_une_courbe_plate_valide() {
        let i = lire_ok("Preamp: -2 dB\n");
        assert!(i.courbe.filtres.is_empty());
        assert_eq!(i.courbe.preampli_db, -2.0);
    }

    #[test]
    fn ecrit_la_grammaire_attendue() {
        let texte = ecrire(&courbe_melangee());
        let lignes: Vec<_> = texte.lines().collect();
        assert_eq!(lignes[0], "Preamp: -6.5 dB");
        assert_eq!(lignes[1], "Filter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.7");
        assert_eq!(lignes[2], "Filter 2: ON PK Fc 1000 Hz Gain -3 dB Q 1.41");
        assert_eq!(lignes[3], "Filter 3: ON PK Fc 3150.5 Hz Gain 0 dB Q 4");
        assert_eq!(
            lignes[4],
            "Filter 4: ON HSC Fc 8000 Hz Gain -2.25 dB Q 0.71"
        );
        assert!(texte.ends_with('\n'));
    }

    #[test]
    fn aller_retour_garde_la_courbe() {
        let c = courbe_melangee();
        let relue = lire(&ecrire(&c)).expect("relecture").courbe;
        assert!(proches(&c, &relue), "{c:?} != {relue:?}");
    }

    #[test]
    fn aller_retour_d_une_courbe_plate() {
        let c = retro_dsp::Courbe::default();
        assert_eq!(lire(&ecrire(&c)).expect("relecture").courbe, c);
    }
}
