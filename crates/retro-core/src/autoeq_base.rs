//! La base AutoEQ, côté logique pure : lire l'index des casques, le garder sous une forme
//! compacte, et chercher dedans. Aucun réseau ici (c'est l'appli qui télécharge, voir
//! ADR-0013) : ce module ne manipule que du texte, donc il se teste sans connexion.
//!
//! Un casque peut avoir plusieurs entrées, une par source de mesure (Rtings, oratory1990…).
//! On ne choisit jamais à la place de l'utilisateur : la recherche rend toutes les sources.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Le dossier des résultats dans le dépôt d'AutoEQ ; les chemins de l'index s'y ajoutent.
pub const URL_RESULTATS: &str =
    "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/";
/// L'index de tous les résultats (environ 0,85 Mo, 8 850 entrées le 25/11/2026).
pub const URL_INDEX: &str =
    "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/INDEX.md";
/// Combien de résultats `chercher` rend au plus : une liste plus longue ne se lit plus.
pub const LIMITE_RESULTATS: usize = 40;
/// Version du format de l'index gardé sur le disque.
const VERSION_FORMAT: u32 = 1;
/// Les types de casque qui terminent le nom du dossier « plateforme + type ».
const TYPES: [&str; 3] = ["over-ear", "in-ear", "earbud"];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErreurBase {
    /// L'index téléchargé ne contient aucune entrée lisible : mieux vaut garder l'ancien.
    #[error("l'index ne contient aucun casque lisible")]
    IndexVide,
    /// Le fichier d'index gardé sur le disque est abîmé.
    #[error("index gardé illisible : {0}")]
    Illisible(String),
    #[error("index gardé d'une version inconnue ({0})")]
    VersionInconnue(u32),
}

/// Un résultat d'AutoEQ : un casque mesuré par une source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntreeBase {
    /// Le nom du casque, tel que dans le dépôt (`Sony INZONE H9`).
    pub nom: String,
    /// Qui a mesuré (`Rtings`, `oratory1990`…).
    pub source: String,
    /// Le dossier « plateforme + type » (`HMS II.3 over-ear`, ou seulement `over-ear`).
    pub dossier: String,
}

impl EntreeBase {
    /// Le banc de mesure (`HMS II.3`), quand le dossier en donne un.
    pub fn plateforme(&self) -> Option<&str> {
        let (plateforme, _) = self.separer_dossier();
        (!plateforme.is_empty()).then_some(plateforme)
    }

    /// Le type de casque (`over-ear`, `in-ear`, `earbud`), quand il est reconnu.
    pub fn type_casque(&self) -> Option<&str> {
        self.separer_dossier().1
    }

    fn separer_dossier(&self) -> (&str, Option<&str>) {
        for t in TYPES {
            if self.dossier == t {
                return ("", Some(t));
            }
            if let Some(debut) = self.dossier.strip_suffix(t) {
                if let Some(plateforme) = debut.strip_suffix(' ') {
                    return (plateforme.trim(), Some(t));
                }
            }
        }
        (self.dossier.as_str(), None)
    }

    /// Le nom du fichier de réglages : `<Nom> ParametricEQ.txt`.
    pub fn nom_de_fichier(&self) -> String {
        format!("{} ParametricEQ.txt", self.nom)
    }

    /// L'adresse du fichier de réglages, chaque morceau de chemin encodé pour une URL.
    pub fn url(&self) -> String {
        format!(
            "{URL_RESULTATS}{}/{}/{}/{}",
            encoder_segment(&self.source),
            encoder_segment(&self.dossier),
            encoder_segment(&self.nom),
            encoder_segment(&self.nom_de_fichier())
        )
    }

    /// Le nom sous lequel le fichier est gardé sur le disque. Les caractères exotiques sont
    /// remplacés (jamais de `/` ni de `..` venus d'un index distant), et un hachage garde les
    /// noms distincts même si deux casques ne diffèrent que par ces caractères.
    pub fn nom_de_fichier_garde(&self) -> String {
        let brut = format!("{} - {} - {}", self.source, self.dossier, self.nom);
        let propre: String = brut
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(100)
            .collect();
        format!("{propre}-{:08x}.txt", hacher(&brut))
    }

    /// La façon de le dire dans le format d'origine d'un profil : `autoeq|source|plateforme`.
    pub fn format_origine(&self) -> String {
        format!(
            "autoeq|{}|{}",
            self.source,
            self.plateforme().unwrap_or_default()
        )
    }
}

/// Hachage FNV-1a : simple, stable d'une version à l'autre (contrairement à celui de la
/// bibliothèque standard), suffisant pour distinguer des noms de fichiers.
fn hacher(texte: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for octet in texte.bytes() {
        h ^= u32::from(octet);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// La base : toutes les entrées, et de quoi chercher vite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseAutoEq {
    entrees: Vec<EntreeBase>,
    /// Quand l'index a été téléchargé (texte déjà prêt à afficher).
    recupere_le: String,
    /// Les mots normalisés de chaque entrée, calculés une fois pour toutes.
    cles: Vec<Cle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cle {
    nom: Vec<String>,
    source: Vec<String>,
    nom_colle: String,
}

/// Ce qu'on écrit sur le disque : des tableaux plutôt que des objets, pour rester compact
/// (8 850 entrées, environ 0,6 Mo).
#[derive(Serialize, Deserialize)]
struct Fichier {
    version: u32,
    recupere_le: String,
    entrees: Vec<(String, String, String)>,
}

impl BaseAutoEq {
    pub fn nouvelle(entrees: Vec<EntreeBase>, recupere_le: impl Into<String>) -> BaseAutoEq {
        let cles = entrees
            .iter()
            .map(|e| {
                let nom = mots(&e.nom);
                Cle {
                    nom_colle: nom.concat(),
                    nom,
                    source: mots(&e.source),
                }
            })
            .collect();
        BaseAutoEq {
            entrees,
            recupere_le: recupere_le.into(),
            cles,
        }
    }

    /// Lit le texte de `INDEX.md`. Une ligne qui n'a pas la forme attendue est sautée : le
    /// dépôt d'AutoEQ peut changer, mieux vaut prendre ce qui se lit que tout refuser.
    pub fn lire_index(
        texte: &str,
        recupere_le: impl Into<String>,
    ) -> Result<BaseAutoEq, ErreurBase> {
        let entrees: Vec<EntreeBase> = texte.lines().filter_map(lire_ligne).collect();
        if entrees.is_empty() {
            return Err(ErreurBase::IndexVide);
        }
        Ok(BaseAutoEq::nouvelle(entrees, recupere_le))
    }

    /// Relit l'index gardé sur le disque.
    pub fn depuis_json(texte: &str) -> Result<BaseAutoEq, ErreurBase> {
        let fichier: Fichier =
            serde_json::from_str(texte).map_err(|e| ErreurBase::Illisible(e.to_string()))?;
        if fichier.version != VERSION_FORMAT {
            return Err(ErreurBase::VersionInconnue(fichier.version));
        }
        let entrees: Vec<EntreeBase> = fichier
            .entrees
            .into_iter()
            .map(|(nom, source, dossier)| EntreeBase {
                nom,
                source,
                dossier,
            })
            .collect();
        if entrees.is_empty() {
            return Err(ErreurBase::IndexVide);
        }
        Ok(BaseAutoEq::nouvelle(entrees, fichier.recupere_le))
    }

    /// L'index sous la forme gardée sur le disque.
    pub fn vers_json(&self) -> String {
        let fichier = Fichier {
            version: VERSION_FORMAT,
            recupere_le: self.recupere_le.clone(),
            entrees: self
                .entrees
                .iter()
                .map(|e| (e.nom.clone(), e.source.clone(), e.dossier.clone()))
                .collect(),
        };
        // Des chaînes et des entiers : la sérialisation ne peut pas échouer.
        serde_json::to_string(&fichier).unwrap_or_default()
    }

    pub fn entrees(&self) -> &[EntreeBase] {
        &self.entrees
    }

    pub fn len(&self) -> usize {
        self.entrees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entrees.is_empty()
    }

    pub fn recupere_le(&self) -> &str {
        &self.recupere_le
    }
}

/// Un résultat de recherche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resultat {
    /// Le rang de l'entrée dans [`BaseAutoEq::entrees`].
    pub indice: usize,
    pub nom: String,
    pub source: String,
    pub plateforme: Option<String>,
    pub type_casque: Option<String>,
}

/// Cherche un casque. Tous les mots de la requête doivent se retrouver ; majuscules, accents
/// et ponctuation ne comptent pas. Classement : nom exact, puis nom qui commence par les mots
/// de la requête (le dernier entier, puis entamé), puis mots entiers ailleurs dans le nom, puis
/// mots entamés, puis requête collée (`wh1000xm4`).
/// À rang égal, le nom le plus court d'abord : « h9 » rend « H9 » avant « H9 II ».
pub fn chercher(requete: &str, base: &BaseAutoEq) -> Vec<Resultat> {
    let q = mots(requete);
    if q.is_empty() {
        return Vec::new();
    }
    let q_colle = q.concat();
    let mut trouves: Vec<(u8, usize, usize)> = Vec::new();
    for (i, cle) in base.cles.iter().enumerate() {
        if let Some(rang) = rang(&q, &q_colle, cle) {
            trouves.push((rang, cle.nom.len(), i));
        }
    }
    trouves.sort_by(|a, b| {
        (a.0, a.1)
            .cmp(&(b.0, b.1))
            .then_with(|| base.cles[a.2].nom.cmp(&base.cles[b.2].nom))
            .then_with(|| base.entrees[a.2].source.cmp(&base.entrees[b.2].source))
            .then(a.2.cmp(&b.2))
    });
    trouves
        .into_iter()
        .take(LIMITE_RESULTATS)
        .map(|(_, _, i)| {
            let e = &base.entrees[i];
            Resultat {
                indice: i,
                nom: e.nom.clone(),
                source: e.source.clone(),
                plateforme: e.plateforme().map(str::to_string),
                type_casque: e.type_casque().map(str::to_string),
            }
        })
        .collect()
}

fn rang(q: &[String], q_colle: &str, cle: &Cle) -> Option<u8> {
    if q == cle.nom.as_slice() {
        return Some(0);
    }
    let dernier = q.len() - 1;
    // Le nom commence par les mots de la requête : tous entiers, ou le dernier seulement entamé.
    if cle.nom.len() >= q.len() && cle.nom[..dernier] == q[..dernier] {
        if cle.nom[dernier] == q[dernier] {
            return Some(1);
        }
        if cle.nom[dernier].starts_with(q[dernier].as_str()) {
            return Some(2);
        }
    }
    if q.iter().all(|m| cle.nom.contains(m)) {
        return Some(3);
    }
    let entame = |m: &String| {
        cle.nom
            .iter()
            .any(|t| t.starts_with(m.as_str()) || (m.len() >= 3 && t.contains(m.as_str())))
            || cle.source.iter().any(|t| t.starts_with(m.as_str()))
    };
    if q.iter().all(entame) {
        return Some(4);
    }
    (q_colle.len() >= 3 && cle.nom_colle.contains(q_colle)).then_some(5)
}

/// Les mots d'un texte : minuscules, sans accents, séparés par tout ce qui n'est ni lettre
/// ni chiffre.
fn mots(texte: &str) -> Vec<String> {
    let mut mots = Vec::new();
    let mut courant = String::new();
    for c in texte.chars() {
        match sans_accent(c) {
            Some(remplace) => courant.push_str(remplace),
            None if c.is_alphanumeric() => courant.extend(c.to_lowercase()),
            None => {
                if !courant.is_empty() {
                    mots.push(std::mem::take(&mut courant));
                }
            }
        }
    }
    if !courant.is_empty() {
        mots.push(courant);
    }
    mots
}

/// Les lettres accentuées courantes, ramenées à leur lettre de base (sans bibliothèque
/// d'Unicode : on ne cherche que des noms de casques).
fn sans_accent(c: char) -> Option<&'static str> {
    Some(match c.to_lowercase().next().unwrap_or(c) {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'ç' => "c",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'ñ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => "o",
        'œ' => "oe",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'ý' | 'ÿ' => "y",
        'ß' => "ss",
        _ => return None,
    })
}

/// Une ligne de `INDEX.md` : `- [Nom](./source/plateforme type/Nom) by source on banc`.
/// Le nom et les dossiers sont pris dans le chemin, qui est celui du dépôt.
fn lire_ligne(ligne: &str) -> Option<EntreeBase> {
    let reste = ligne.trim().strip_prefix("- [")?;
    let (_, apres) = reste.split_once("](./")?;
    let chemin = match apres.rfind(") by ") {
        Some(i) => &apres[..i],
        None => apres.trim_end().strip_suffix(')')?,
    };
    let morceaux: Vec<String> = chemin.split('/').map(decoder_segment).collect();
    let [source, dossier, nom] = <[String; 3]>::try_from(morceaux).ok()?;
    let valide = |s: &str| !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\']);
    (valide(&source) && valide(&dossier) && valide(&nom)).then_some(EntreeBase {
        nom,
        source,
        dossier,
    })
}

/// `%20` devient une espace, etc. Une séquence `%` invalide est laissée telle quelle.
fn decoder_segment(segment: &str) -> String {
    let octets = segment.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'%' && i + 2 < octets.len() {
            let hex = std::str::from_utf8(&octets[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                sortie.push(v);
                i += 3;
                continue;
            }
        }
        sortie.push(octets[i]);
        i += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

/// Encode un morceau de chemin pour une URL : tout sauf lettres, chiffres et `-._~`.
fn encoder_segment(segment: &str) -> String {
    let mut sortie = String::with_capacity(segment.len() * 2);
    for octet in segment.bytes() {
        if octet.is_ascii_alphanumeric() || matches!(octet, b'-' | b'.' | b'_' | b'~') {
            sortie.push(octet as char);
        } else {
            sortie.push_str(&format!("%{octet:02X}"));
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un index écrit à la main, de la même forme que le vrai `INDEX.md` mais avec des
    /// casques inventés : on teste la structure, pas les données d'AutoEQ.
    const INDEX: &str = "\
# Index
Texte d'introduction sur plusieurs
lignes, sans rapport avec les entrées.

- [Acme Studio H9](./Mesures%20Alpha/Banc%20X%20over-ear/Acme%20Studio%20H9) by Mesures Alpha on Banc X
- [Acme Studio H9 II](./Mesures%20Alpha/Banc%20X%20over-ear/Acme%20Studio%20H9%20II) by Mesures Alpha on Banc X
- [Acme Studio H9](./Labo%20Beta/over-ear/Acme%20Studio%20H9) by Labo Beta
- [Acme Studio H95](./Labo%20Beta/over-ear/Acme%20Studio%20H95) by Labo Beta
- [Acme Studio H9i](./Labo%20Beta/over-ear/Acme%20Studio%20H9i) by Labo Beta
- [Zeta Café Pro (cuir)](./Labo%20Beta/over-ear/Zeta%20Caf\u{e9}%20Pro%20(cuir)) by Labo Beta
- [Orbe & Co Delta](./Mesures%20Alpha/Banc%20Y%20in-ear/Orbe%20&%20Co%20Delta) by Mesures Alpha on Banc Y
- [Orbe & Co Delta S](./Mesures%20Alpha/Banc%20Y%20in-ear/Orbe%20&%20Co%20Delta%20S) by Mesures Alpha on Banc Y
- [Orbe & Co Delta S](./Labo%20Beta/in-ear/Orbe%20&%20Co%20Delta%20S) by Labo Beta
- [Quark WH-1000X4](./Labo%20Beta/over-ear/Quark%20WH-1000X4) by Labo Beta
- [Pico Bud](./Labo%20Beta/earbud/Pico%20Bud) by Labo Beta
- [Sans Plateforme](./Labo%20Gamma/Sans%20Plateforme)
- [Trop de dossiers](./a/b/c/d) by x
- ligne sans forme reconnue
";

    fn base() -> BaseAutoEq {
        BaseAutoEq::lire_index(INDEX, "25/11").unwrap()
    }

    fn noms(requete: &str) -> Vec<(String, String)> {
        chercher(requete, &base())
            .into_iter()
            .map(|r| (r.nom, r.source))
            .collect()
    }

    fn paire(nom: &str, source: &str) -> (String, String) {
        (nom.to_string(), source.to_string())
    }

    #[test]
    fn lit_les_entrees_et_saute_les_lignes_inconnues() {
        let b = base();
        // 11 casques ; « Sans Plateforme » n'a que deux dossiers, « Trop de dossiers » quatre.
        assert_eq!(b.len(), 11);
        assert_eq!(b.recupere_le(), "25/11");
    }

    #[test]
    fn decode_les_chemins_et_garde_source_banc_et_type() {
        let b = base();
        let e = &b.entrees()[6];
        assert_eq!(e.nom, "Orbe & Co Delta");
        assert_eq!(e.source, "Mesures Alpha");
        assert_eq!(e.plateforme(), Some("Banc Y"));
        assert_eq!(e.type_casque(), Some("in-ear"));
        let sans_banc = &b.entrees()[2];
        assert_eq!(sans_banc.plateforme(), None);
        assert_eq!(sans_banc.type_casque(), Some("over-ear"));
        assert_eq!(b.entrees()[5].nom, "Zeta Café Pro (cuir)");
    }

    #[test]
    fn index_sans_entree_refuse() {
        assert_eq!(
            BaseAutoEq::lire_index("# Index\nrien\n", "x"),
            Err(ErreurBase::IndexVide)
        );
        assert_eq!(BaseAutoEq::lire_index("", "x"), Err(ErreurBase::IndexVide));
    }

    #[test]
    fn url_encode_chaque_morceau() {
        let b = base();
        assert_eq!(
            b.entrees()[6].url(),
            format!(
                "{URL_RESULTATS}Mesures%20Alpha/Banc%20Y%20in-ear/Orbe%20%26%20Co%20Delta/Orbe%20%26%20Co%20Delta%20ParametricEQ.txt"
            )
        );
        assert!(b.entrees()[5]
            .url()
            .contains("Zeta%20Caf%C3%A9%20Pro%20%28cuir%29"));
    }

    #[test]
    fn nom_de_fichier_garde_est_sur_et_distinct() {
        let b = base();
        let a = b.entrees()[0].nom_de_fichier_garde();
        let c = b.entrees()[2].nom_de_fichier_garde();
        assert_ne!(a, c, "deux sources, deux fichiers");
        assert!(a.ends_with(".txt"));
        assert!(!a.contains(['/', '\\', ' ']));
        let piege = EntreeBase {
            nom: "../../x".into(),
            source: "..".into(),
            dossier: "a/b".into(),
        };
        assert!(!piege.nom_de_fichier_garde().contains('/'));
    }

    #[test]
    fn format_d_origine_nomme_la_source_et_le_banc() {
        let b = base();
        assert_eq!(
            b.entrees()[0].format_origine(),
            "autoeq|Mesures Alpha|Banc X"
        );
        assert_eq!(b.entrees()[2].format_origine(), "autoeq|Labo Beta|");
    }

    #[test]
    fn json_aller_retour() {
        let b = base();
        let relu = BaseAutoEq::depuis_json(&b.vers_json()).unwrap();
        assert_eq!(relu, b);
        assert_eq!(chercher("delta", &relu), chercher("delta", &b));
    }

    #[test]
    fn json_abime_ou_vide_ou_d_une_autre_version_refuse() {
        assert!(matches!(
            BaseAutoEq::depuis_json("{pas du json"),
            Err(ErreurBase::Illisible(_))
        ));
        assert!(matches!(
            BaseAutoEq::depuis_json("{\"version\":1}"),
            Err(ErreurBase::Illisible(_))
        ));
        assert_eq!(
            BaseAutoEq::depuis_json("{\"version\":1,\"recupere_le\":\"x\",\"entrees\":[]}"),
            Err(ErreurBase::IndexVide)
        );
        assert_eq!(
            BaseAutoEq::depuis_json("{\"version\":9,\"recupere_le\":\"x\",\"entrees\":[]}"),
            Err(ErreurBase::VersionInconnue(9))
        );
    }

    #[test]
    fn recherche_sans_tenir_compte_des_majuscules_accents_et_ponctuation() {
        let attendu = vec![paire("Zeta Café Pro (cuir)", "Labo Beta")];
        assert_eq!(noms("zeta cafe pro"), attendu);
        assert_eq!(noms("ZETA CAFÉ"), attendu);
        assert_eq!(noms("  zeta,   cafe -- cuir "), attendu);
        assert_eq!(noms("orbe & co"), noms("orbe co"));
    }

    #[test]
    fn tous_les_mots_doivent_se_retrouver() {
        assert!(noms("acme zeta").is_empty());
        assert!(noms("introuvable").is_empty());
        assert!(noms("").is_empty());
        assert!(noms("  ,, ").is_empty());
    }

    #[test]
    fn h9_ii_n_est_pas_noye_par_h9() {
        assert_eq!(
            noms("h9 ii"),
            vec![paire("Acme Studio H9 II", "Mesures Alpha")]
        );
        assert_eq!(
            noms("acme h9 ii"),
            vec![paire("Acme Studio H9 II", "Mesures Alpha")]
        );
    }

    #[test]
    fn nom_exact_puis_debut_puis_mots_et_le_plus_court_d_abord() {
        // Nom exact d'abord (les deux sources), puis le nom qui commence par la requête.
        assert_eq!(
            noms("acme studio h9"),
            vec![
                paire("Acme Studio H9", "Labo Beta"),
                paire("Acme Studio H9", "Mesures Alpha"),
                paire("Acme Studio H9 II", "Mesures Alpha"),
                paire("Acme Studio H95", "Labo Beta"),
                paire("Acme Studio H9i", "Labo Beta"),
            ]
        );
        // « h9 » : mots entiers (H9, H9 II) avant les mots entamés (H95, H9i).
        let r = noms("h9");
        assert_eq!(r[0], paire("Acme Studio H9", "Labo Beta"));
        assert_eq!(r[2], paire("Acme Studio H9 II", "Mesures Alpha"));
        assert_eq!(r.len(), 5);
    }

    #[test]
    fn plusieurs_sources_sont_toutes_listees() {
        let r = chercher("delta s", &base());
        let sources: Vec<&str> = r.iter().map(|r| r.source.as_str()).collect();
        assert_eq!(sources, vec!["Labo Beta", "Mesures Alpha"]);
        assert_eq!(r[1].plateforme.as_deref(), Some("Banc Y"));
        assert_eq!(r[1].type_casque.as_deref(), Some("in-ear"));
    }

    #[test]
    fn la_source_aide_a_chercher_et_la_requete_collee_est_comprise() {
        assert_eq!(noms("bud labo").len(), 1);
        assert_eq!(
            noms("wh1000x4"),
            vec![paire("Quark WH-1000X4", "Labo Beta")]
        );
        assert_eq!(noms("wh 1000"), vec![paire("Quark WH-1000X4", "Labo Beta")]);
    }

    #[test]
    fn la_liste_est_limitee() {
        let entrees = (0..200)
            .map(|i| EntreeBase {
                nom: format!("Modèle {i}"),
                source: "S".into(),
                dossier: "over-ear".into(),
            })
            .collect();
        let grosse = BaseAutoEq::nouvelle(entrees, "x");
        assert_eq!(chercher("modele", &grosse).len(), LIMITE_RESULTATS);
    }

    #[test]
    fn decode_une_sequence_invalide_sans_planter() {
        assert_eq!(decoder_segment("100%"), "100%");
        assert_eq!(decoder_segment("a%2"), "a%2");
        assert_eq!(decoder_segment("a%zzb"), "a%zzb");
        assert_eq!(decoder_segment("a%20b%26"), "a b&");
    }
}
