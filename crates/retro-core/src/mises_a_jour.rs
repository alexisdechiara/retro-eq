//! Les mises à jour : la logique pure, sans réseau ni horloge.
//!
//! L'appli demande à GitHub quelle est la dernière version publiée
//! (`GET /repos/{dépôt}/releases/latest`), puis ce module décide si elle est plus récente
//! que celle qui tourne. Il ne fait **aucune** requête : on lui donne la réponse (statut,
//! en-têtes, corps), il rend un [`EtatMiseAJour`]. C'est ce qui permet de tout tester sur
//! Linux avec des JSON écrits à la main.
//!
//! Tout ce qui vient du réseau est du texte non fiable : les notes sont réduites à du texte
//! brut tronqué (jamais interprétées comme du Markdown ou du HTML), et les liens ne sont
//! gardés que s'ils pointent vers github.com en https.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use thiserror::Error;

/// Le dépôt qui publie les versions.
pub const DEPOT_GITHUB: &str = "alexisdechiara/retro-eq";
/// L'adresse de la page des versions du dépôt : repli si GitHub ne donne pas de lien sûr.
pub const PAGE_VERSIONS: &str = "https://github.com/alexisdechiara/retro-eq/releases/latest";
/// L'adresse où signaler un problème.
pub const PAGE_NOUVEAU_PROBLEME: &str = "https://github.com/alexisdechiara/retro-eq/issues/new";
/// Délai minimal entre deux vérifications automatiques : une journée.
pub const DELAI_ENTRE_VERIFICATIONS_MS: u64 = 24 * 60 * 60 * 1000;
/// Longueur maximale (en caractères) des notes gardées et affichées.
pub const NOTES_MAX_CARACTERES: usize = 600;
/// Nombre maximal de lignes des notes gardées et affichées.
pub const NOTES_MAX_LIGNES: usize = 10;

/// L'adresse de l'API pour la dernière version publiée.
pub fn url_derniere_version() -> String {
    format!("https://api.github.com/repos/{DEPOT_GITHUB}/releases/latest")
}

// ----- Versions -----

/// Un numéro de version illisible.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("numéro de version illisible : « {0} »")]
pub struct ErreurVersion(pub String);

/// Un identifiant de pré-version (`beta`, `1` dans `0.2.0-beta.1`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum IdPre {
    Numerique(u64),
    Texte(String),
}

impl Ord for IdPre {
    /// Règle de semver : un identifiant numérique est plus petit qu'un textuel ; les
    /// numériques se comparent comme des nombres, les textuels dans l'ordre ASCII.
    fn cmp(&self, autre: &Self) -> Ordering {
        match (self, autre) {
            (IdPre::Numerique(a), IdPre::Numerique(b)) => a.cmp(b),
            (IdPre::Numerique(_), IdPre::Texte(_)) => Ordering::Less,
            (IdPre::Texte(_), IdPre::Numerique(_)) => Ordering::Greater,
            (IdPre::Texte(a), IdPre::Texte(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for IdPre {
    fn partial_cmp(&self, autre: &Self) -> Option<Ordering> {
        Some(self.cmp(autre))
    }
}

/// Un numéro de version comparable, avec la précédence de semver.
///
/// `1.0.0-alpha < 1.0.0-alpha.1 < 1.0.0-beta < 1.0.0-beta.2 < 1.0.0-beta.11 < 1.0.0-rc.1 <
/// 1.0.0`. Les métadonnées de build (`+abc`) sont lues puis ignorées, comme le veut semver.
/// Écrit à la main : une dépendance entière pour ces quelques lignes serait disproportionnée.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Version {
    majeure: u64,
    mineure: u64,
    correctif: u64,
    pre: Vec<IdPre>,
}

impl Version {
    /// Lit `0.1.0`, `v0.1.0`, `0.2.0-beta.1`, `1.0.0+build5`.
    pub fn analyser(texte: &str) -> Result<Version, ErreurVersion> {
        let erreur = || ErreurVersion(texte.chars().take(40).collect());
        let t = texte.trim();
        let t = t.strip_prefix(['v', 'V']).unwrap_or(t);
        // Les métadonnées de build ne comptent pas dans la comparaison.
        let t = t.split('+').next().unwrap_or("");
        let (noyau, pre) = match t.split_once('-') {
            Some((noyau, pre)) => (noyau, Some(pre)),
            None => (t, None),
        };
        let mut nombres = noyau.split('.');
        let mut suivant = || -> Result<u64, ErreurVersion> {
            let morceau = nombres.next().ok_or_else(erreur)?;
            if morceau.is_empty() || !morceau.bytes().all(|b| b.is_ascii_digit()) {
                return Err(erreur());
            }
            morceau.parse().map_err(|_| erreur())
        };
        let (majeure, mineure, correctif) = (suivant()?, suivant()?, suivant()?);
        if nombres.next().is_some() {
            return Err(erreur());
        }
        let pre = match pre {
            None => Vec::new(),
            Some(pre) => pre
                .split('.')
                .map(|id| {
                    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        Err(erreur())
                    } else if id.bytes().all(|b| b.is_ascii_digit()) {
                        id.parse().map(IdPre::Numerique).map_err(|_| erreur())
                    } else {
                        Ok(IdPre::Texte(id.to_string()))
                    }
                })
                .collect::<Result<_, _>>()?,
        };
        Ok(Version {
            majeure,
            mineure,
            correctif,
            pre,
        })
    }

    /// Est-ce une pré-version (`-beta.1`, `-rc.1`) ?
    pub fn est_pre_version(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for Version {
    fn cmp(&self, autre: &Self) -> Ordering {
        (self.majeure, self.mineure, self.correctif)
            .cmp(&(autre.majeure, autre.mineure, autre.correctif))
            .then_with(|| match (self.pre.is_empty(), autre.pre.is_empty()) {
                // Une version finale passe devant ses pré-versions.
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                // Sinon on compare identifiant par identifiant ; le plus court perd s'il
                // est le début de l'autre (c'est le comportement de `Vec::cmp`).
                (false, false) => self.pre.cmp(&autre.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, autre: &Self) -> Option<Ordering> {
        Some(self.cmp(autre))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.majeure, self.mineure, self.correctif)?;
        for (i, id) in self.pre.iter().enumerate() {
            f.write_str(if i == 0 { "-" } else { "." })?;
            match id {
                IdPre::Numerique(n) => write!(f, "{n}")?,
                IdPre::Texte(t) => f.write_str(t)?,
            }
        }
        Ok(())
    }
}

// ----- Réponse de l'API -----

/// Ce que l'API a répondu, brut : statut HTTP, en-têtes et corps. Le client HTTP de l'appli
/// le fabrique ; ce module le lit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReponseApi {
    pub statut: u16,
    pub entetes: Vec<(String, String)>,
    pub corps: String,
}

impl ReponseApi {
    /// La valeur d'un en-tête (le nom se compare sans tenir compte des majuscules).
    pub fn entete(&self, nom: &str) -> Option<&str> {
        self.entetes
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nom))
            .map(|(_, v)| v.as_str())
    }

    /// GitHub refuse parce que la limite de requêtes est atteinte : 429, ou 403 avec
    /// `x-ratelimit-remaining: 0` (ou `retry-after`, pour la limite « secondaire »).
    pub fn limite_atteinte(&self) -> bool {
        match self.statut {
            429 => true,
            403 => {
                self.entete("x-ratelimit-remaining")
                    .is_some_and(|v| v.trim() == "0")
                    || self.entete("retry-after").is_some()
            }
            _ => false,
        }
    }
}

/// L'installateur trouvé parmi les fichiers de la version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installateur {
    pub nom: String,
    /// Taille en octets, 0 si GitHub ne la donne pas.
    pub taille: u64,
    /// Adresse de téléchargement, en https vers github.com. L'appli ne télécharge rien :
    /// elle ne montre que le nom et la taille, et ouvre la page de la version.
    pub lien: String,
}

/// Les fichiers dont le nom ressemble à un installateur Windows : `…installateur….exe` ou
/// `…setup….exe`, sans tenir compte des majuscules.
pub fn est_un_installateur(nom: &str) -> bool {
    let n = nom.to_lowercase();
    n.ends_with(".exe") && (n.contains("installateur") || n.contains("setup"))
}

/// Une version publiée, réduite à ce dont l'appli a besoin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPubliee {
    /// Le numéro lu dans `tag_name`.
    pub version: Version,
    /// `AAAA-MM-JJ`, tirée de `published_at`.
    pub date: Option<String>,
    /// Les notes de version en texte brut tronqué.
    pub notes: String,
    /// La page de la version sur GitHub (adresse sûre, sinon la page des versions).
    pub page: String,
    pub installateur: Option<Installateur>,
    pub brouillon: bool,
    pub pre_version: bool,
}

/// Pourquoi on ne sait pas s'il existe une mise à jour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "code")]
pub enum RaisonInconnue {
    /// Le dépôt n'a encore publié aucune version (404). Ce n'est pas une panne.
    AucuneVersionPubliee,
    /// GitHub a refusé parce qu'il y a eu trop de requêtes depuis cette adresse.
    LimiteAtteinte,
    /// La réponse n'est pas du JSON attendu.
    ReponseIllisible,
    /// La version publiée n'a pas un numéro lisible.
    VersionIllisible,
    /// GitHub a répondu avec un autre statut (503, 500…).
    Http(u16),
    /// Pas de réseau, ou GitHub ne répond pas.
    Reseau,
}

/// Ce que l'appli sait de la dernière version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "etat")]
pub enum EtatMiseAJour {
    /// La version qui tourne est la plus récente publiée (ou plus récente : build local).
    AJour,
    Disponible {
        version: String,
        date: Option<String>,
        notes: String,
        page: String,
        installateur: Option<Installateur>,
    },
    Inconnu {
        raison: RaisonInconnue,
    },
}

impl EtatMiseAJour {
    pub fn est_disponible(&self) -> bool {
        matches!(self, EtatMiseAJour::Disponible { .. })
    }

    /// Un état gardé sur le disque peut être périmé : si l'utilisateur a installé depuis
    /// la version annoncée, ou une plus récente, il n'y a plus rien à proposer.
    pub fn pour_version(self, courante: &Version) -> EtatMiseAJour {
        match &self {
            EtatMiseAJour::Disponible { version, .. }
                if Version::analyser(version).is_ok_and(|v| v <= *courante) =>
            {
                EtatMiseAJour::AJour
            }
            _ => self,
        }
    }
}

// ----- Lecture du JSON -----

#[derive(Deserialize, Default)]
#[serde(default)]
struct AssetJson {
    name: Option<String>,
    size: Option<u64>,
    browser_download_url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReleaseJson {
    tag_name: Option<String>,
    html_url: Option<String>,
    published_at: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Option<Vec<AssetJson>>,
}

/// Un lien qu'on accepte d'afficher et d'ouvrir : https, vers github.com, sans espace ni
/// caractère de contrôle.
fn lien_github_sur(lien: &str) -> bool {
    lien.starts_with("https://github.com/")
        && lien.len() <= 300
        && !lien.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// `2026-01-02T10:00:00Z` devient `2026-01-02` ; tout ce qui n'a pas cette forme est écarté.
fn date_du_jour(horodatage: &str) -> Option<String> {
    let jour = horodatage.get(..10)?;
    let forme = jour.bytes().enumerate().all(|(i, c)| match i {
        4 | 7 => c == b'-',
        _ => c.is_ascii_digit(),
    });
    forme.then(|| jour.to_string())
}

/// Réduit des notes de version à du texte brut sûr à afficher : sans caractères de contrôle
/// ni de réécriture du sens de lecture, sans retours Windows, sans lignes vides en série,
/// tronqué en lignes et en caractères. Le Markdown n'est pas interprété : ce texte vient du
/// réseau.
pub fn notes_en_texte_brut(brut: &str) -> String {
    let nettoye: String = brut
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| {
            *c == '\n'
                || (!c.is_control()
                    && !matches!(*c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}'..='\u{200F}'))
        })
        .collect();
    let mut lignes: Vec<&str> = Vec::new();
    let mut vide_avant = true;
    for ligne in nettoye.lines().map(str::trim_end) {
        if ligne.trim().is_empty() {
            if !vide_avant {
                lignes.push("");
            }
            vide_avant = true;
        } else {
            lignes.push(ligne);
            vide_avant = false;
        }
    }
    while lignes.last() == Some(&"") {
        lignes.pop();
    }
    let trop_de_lignes = lignes.len() > NOTES_MAX_LIGNES;
    lignes.truncate(NOTES_MAX_LIGNES);
    let texte = lignes.join("\n");
    if texte.chars().count() > NOTES_MAX_CARACTERES {
        let coupe: String = texte.chars().take(NOTES_MAX_CARACTERES).collect();
        format!("{}…", coupe.trim_end())
    } else if trop_de_lignes {
        format!("{texte}\n…")
    } else {
        texte
    }
}

/// Lit le corps de `releases/latest`.
pub fn analyser_version_publiee(corps: &str) -> Result<VersionPubliee, RaisonInconnue> {
    let json: ReleaseJson =
        serde_json::from_str(corps).map_err(|_| RaisonInconnue::ReponseIllisible)?;
    let tag = json.tag_name.ok_or(RaisonInconnue::ReponseIllisible)?;
    let version = Version::analyser(&tag).map_err(|_| RaisonInconnue::VersionIllisible)?;
    let installateur = json.assets.unwrap_or_default().into_iter().find_map(|a| {
        let nom = a.name?;
        let lien = a.browser_download_url?;
        (est_un_installateur(&nom) && lien_github_sur(&lien)).then(|| Installateur {
            nom: nom.chars().filter(|c| !c.is_control()).take(120).collect(),
            taille: a.size.unwrap_or(0),
            lien,
        })
    });
    Ok(VersionPubliee {
        pre_version: json.prerelease || version.est_pre_version(),
        brouillon: json.draft,
        date: json.published_at.as_deref().and_then(date_du_jour),
        notes: notes_en_texte_brut(json.body.as_deref().unwrap_or("")),
        page: json
            .html_url
            .filter(|l| lien_github_sur(l))
            .unwrap_or_else(|| PAGE_VERSIONS.to_string()),
        installateur,
        version,
    })
}

/// Décide de l'état à partir de la réponse de l'API et de la version qui tourne.
///
/// - 404 : le dépôt n'a rien publié, ce n'est pas une panne ;
/// - limite de requêtes atteinte (429, ou 403 avec `x-ratelimit-remaining: 0`) ;
/// - une version brouillon est ignorée, une pré-version aussi, sauf si la version qui tourne
///   en est une elle-même ;
/// - une version égale ou plus ancienne que la nôtre (build local en avance) : à jour.
pub fn decider(courante: &Version, reponse: &ReponseApi) -> EtatMiseAJour {
    let inconnu = |raison| EtatMiseAJour::Inconnu { raison };
    if reponse.limite_atteinte() {
        return inconnu(RaisonInconnue::LimiteAtteinte);
    }
    match reponse.statut {
        200..=299 => {}
        404 => return inconnu(RaisonInconnue::AucuneVersionPubliee),
        autre => return inconnu(RaisonInconnue::Http(autre)),
    }
    let publiee = match analyser_version_publiee(&reponse.corps) {
        Ok(p) => p,
        Err(raison) => return inconnu(raison),
    };
    if publiee.brouillon || (publiee.pre_version && !courante.est_pre_version()) {
        return inconnu(RaisonInconnue::AucuneVersionPubliee);
    }
    if publiee.version > *courante {
        EtatMiseAJour::Disponible {
            version: publiee.version.to_string(),
            date: publiee.date,
            notes: publiee.notes,
            page: publiee.page,
            installateur: publiee.installateur,
        }
    } else {
        EtatMiseAJour::AJour
    }
}

// ----- Quand vérifier -----

/// Ce qui est gardé entre deux lancements (`mises-a-jour.json`) : quand on a vérifié pour la
/// dernière fois, et ce que GitHub a répondu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MiseAJourSauvee {
    /// Instant de la dernière vérification réussie, en ms depuis 1970.
    pub derniere_verification_ms: Option<u64>,
    /// Instant de la dernière fois où GitHub a répondu, bien ou mal (limite atteinte,
    /// panne). Une vérification ratée ne se réessaie donc pas à chaque lancement.
    pub derniere_reponse_ms: Option<u64>,
    pub etat: Option<EtatMiseAJour>,
}

impl MiseAJourSauvee {
    /// Le dernier instant où l'on a obtenu une réponse de GitHub : celui qui compte pour la
    /// règle « une fois par jour ».
    pub fn dernier_contact_ms(&self) -> Option<u64> {
        self.derniere_verification_ms
            .into_iter()
            .chain(self.derniere_reponse_ms)
            .max()
    }
}

/// La vérification automatique est-elle due ? Au plus une fois par jour, et jamais si
/// l'utilisateur l'a coupée. Si l'horloge est revenue en arrière, on revérifie (mieux vaut
/// une requête de trop qu'une vérification bloquée jusqu'en 2030).
pub fn verification_due(reglage_actif: bool, derniere_ms: Option<u64>, maintenant_ms: u64) -> bool {
    if !reglage_actif {
        return false;
    }
    match derniere_ms {
        None => true,
        Some(d) if maintenant_ms < d => true,
        Some(d) => maintenant_ms - d >= DELAI_ENTRE_VERIFICATIONS_MS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(t: &str) -> Version {
        Version::analyser(t).unwrap()
    }

    // --- Versions ---

    #[test]
    fn les_formes_courantes_se_lisent() {
        assert_eq!(v("0.1.0"), v("v0.1.0"));
        assert_eq!(v("V1.2.3"), v(" 1.2.3 "));
        assert_eq!(v("0.2.0-beta.1").to_string(), "0.2.0-beta.1");
        assert_eq!(v("v10.20.30").to_string(), "10.20.30");
        assert!(v("0.2.0-beta.1").est_pre_version());
        assert!(!v("0.2.0").est_pre_version());
    }

    #[test]
    fn les_numeros_illisibles_sont_refuses() {
        for texte in [
            "",
            "v",
            "1",
            "1.2",
            "1.2.3.4",
            "a.b.c",
            "1.2.x",
            "1..3",
            "1.2.3-",
            "1.2.3-a..b",
            "1.2.3-é",
            "-1.2.3",
            "99999999999999999999.0.0",
        ] {
            assert!(
                Version::analyser(texte).is_err(),
                "{texte:?} devrait échouer"
            );
        }
    }

    #[test]
    fn la_precedence_suit_semver() {
        // L'exemple de la spécification, du plus petit au plus grand.
        let ordre = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "2.0.0",
        ];
        for paire in ordre.windows(2) {
            assert!(v(paire[0]) < v(paire[1]), "{} < {}", paire[0], paire[1]);
            assert!(v(paire[1]) > v(paire[0]));
        }
        // 9 < 10 : on compare des nombres, pas du texte.
        assert!(v("0.9.0") < v("0.10.0"));
        assert!(v("1.0.0-beta.2") < v("1.0.0-beta.10"));
    }

    #[test]
    fn les_metadonnees_de_build_sont_ignorees() {
        assert_eq!(v("1.0.0+abc"), v("1.0.0+def"));
        assert_eq!(v("1.0.0-rc.1+x"), v("1.0.0-rc.1"));
    }

    // --- Quand vérifier ---

    const JOUR: u64 = DELAI_ENTRE_VERIFICATIONS_MS;

    #[test]
    fn une_verification_par_jour_au_plus() {
        let t = 1_000 * JOUR;
        assert!(verification_due(true, None, t));
        assert!(!verification_due(true, Some(t - 1), t));
        assert!(!verification_due(true, Some(t - JOUR + 1), t));
        assert!(verification_due(true, Some(t - JOUR), t));
        assert!(verification_due(true, Some(t - 5 * JOUR), t));
        // Horloge revenue en arrière : on revérifie.
        assert!(verification_due(true, Some(t + 10), t));
    }

    #[test]
    fn reglage_coupe_jamais_due() {
        assert!(!verification_due(false, None, 5 * JOUR));
        assert!(!verification_due(false, Some(0), 50 * JOUR));
    }

    // --- Réponses de l'API (JSON écrits à la main, forme de la doc GitHub) ---

    fn release(tag: &str, draft: bool, pre: bool) -> String {
        format!(
            r###"{{
  "url": "https://api.github.com/repos/alexisdechiara/retro-eq/releases/1",
  "html_url": "https://github.com/alexisdechiara/retro-eq/releases/tag/{tag}",
  "id": 1,
  "tag_name": "{tag}",
  "target_commitish": "main",
  "name": "Retro EQ {tag}",
  "body": "## Nouveautés\r\n\r\n- Mises à jour\r\n- Corrections",
  "draft": {draft},
  "prerelease": {pre},
  "created_at": "2026-01-01T09:00:00Z",
  "published_at": "2026-01-02T10:30:00Z",
  "author": {{ "login": "alexisdechiara", "id": 7 }},
  "assets": [
    {{
      "url": "https://api.github.com/repos/alexisdechiara/retro-eq/releases/assets/10",
      "id": 10,
      "name": "retro-eq-{tag}-windows.zip",
      "content_type": "application/zip",
      "size": 5000000,
      "browser_download_url": "https://github.com/alexisdechiara/retro-eq/releases/download/{tag}/retro-eq-{tag}-windows.zip"
    }},
    {{
      "url": "https://api.github.com/repos/alexisdechiara/retro-eq/releases/assets/11",
      "id": 11,
      "name": "Retro-EQ-Installateur-{tag}.exe",
      "content_type": "application/octet-stream",
      "size": 12345678,
      "browser_download_url": "https://github.com/alexisdechiara/retro-eq/releases/download/{tag}/Retro-EQ-Installateur-{tag}.exe"
    }}
  ]
}}"###
        )
    }

    fn ok(corps: String) -> ReponseApi {
        ReponseApi {
            statut: 200,
            entetes: vec![("X-RateLimit-Remaining".into(), "59".into())],
            corps,
        }
    }

    fn statut(code: u16, entetes: &[(&str, &str)]) -> ReponseApi {
        ReponseApi {
            statut: code,
            entetes: entetes
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect(),
            corps: r#"{"message":"Not Found"}"#.into(),
        }
    }

    #[test]
    fn une_version_plus_recente_est_proposee_avec_ses_details() {
        let etat = decider(&v("0.1.0"), &ok(release("v0.2.0", false, false)));
        let EtatMiseAJour::Disponible {
            version,
            date,
            notes,
            page,
            installateur,
        } = etat
        else {
            panic!("une mise à jour était attendue : {etat:?}");
        };
        assert_eq!(version, "0.2.0");
        assert_eq!(date.as_deref(), Some("2026-01-02"));
        assert_eq!(notes, "## Nouveautés\n\n- Mises à jour\n- Corrections");
        assert_eq!(
            page,
            "https://github.com/alexisdechiara/retro-eq/releases/tag/v0.2.0"
        );
        let installateur = installateur.unwrap();
        assert_eq!(installateur.nom, "Retro-EQ-Installateur-v0.2.0.exe");
        assert_eq!(installateur.taille, 12_345_678);
    }

    #[test]
    fn la_meme_version_est_a_jour() {
        assert_eq!(
            decider(&v("0.2.0"), &ok(release("v0.2.0", false, false))),
            EtatMiseAJour::AJour
        );
    }

    #[test]
    fn un_build_local_en_avance_est_a_jour() {
        assert_eq!(
            decider(&v("0.3.0"), &ok(release("v0.2.0", false, false))),
            EtatMiseAJour::AJour
        );
    }

    #[test]
    fn une_pre_version_est_ignoree_pour_une_version_stable() {
        for (drapeau, tag) in [(true, "v0.2.0"), (false, "v0.2.0-beta.1")] {
            assert_eq!(
                decider(&v("0.1.0"), &ok(release(tag, false, drapeau))),
                EtatMiseAJour::Inconnu {
                    raison: RaisonInconnue::AucuneVersionPubliee
                },
                "{tag} / prerelease={drapeau}"
            );
        }
    }

    #[test]
    fn une_pre_version_compte_pour_qui_utilise_deja_une_pre_version() {
        let etat = decider(
            &v("0.2.0-beta.1"),
            &ok(release("v0.2.0-beta.2", false, true)),
        );
        assert!(etat.est_disponible());
        // Et une pré-version plus ancienne ne propose rien.
        assert_eq!(
            decider(
                &v("0.2.0-beta.2"),
                &ok(release("v0.2.0-beta.1", false, true))
            ),
            EtatMiseAJour::AJour
        );
        // La finale de la même version remplace la bêta.
        assert!(decider(&v("0.2.0-rc.1"), &ok(release("v0.2.0", false, false))).est_disponible());
    }

    #[test]
    fn un_brouillon_est_ignore() {
        for courante in ["0.1.0", "0.1.0-beta.1"] {
            assert_eq!(
                decider(&v(courante), &ok(release("v9.0.0", true, false))),
                EtatMiseAJour::Inconnu {
                    raison: RaisonInconnue::AucuneVersionPubliee
                }
            );
        }
    }

    #[test]
    fn aucune_version_publiee_n_est_pas_une_erreur() {
        assert_eq!(
            decider(&v("0.1.0"), &statut(404, &[])),
            EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::AucuneVersionPubliee
            }
        );
    }

    #[test]
    fn la_limite_de_requetes_est_reconnue() {
        let attendu = EtatMiseAJour::Inconnu {
            raison: RaisonInconnue::LimiteAtteinte,
        };
        let c = v("0.1.0");
        assert_eq!(
            decider(&c, &statut(403, &[("x-ratelimit-remaining", "0")])),
            attendu
        );
        assert_eq!(
            decider(&c, &statut(403, &[("X-RateLimit-Remaining", "0")])),
            attendu
        );
        assert_eq!(decider(&c, &statut(429, &[])), attendu);
        assert_eq!(decider(&c, &statut(403, &[("Retry-After", "60")])), attendu);
        // Un 403 sans rapport avec la limite reste une erreur HTTP ordinaire.
        assert_eq!(
            decider(&c, &statut(403, &[("x-ratelimit-remaining", "12")])),
            EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::Http(403)
            }
        );
    }

    #[test]
    fn les_autres_statuts_sont_des_erreurs_http() {
        assert_eq!(
            decider(&v("0.1.0"), &statut(503, &[])),
            EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::Http(503)
            }
        );
    }

    #[test]
    fn un_json_abime_ou_incomplet_est_illisible() {
        let illisible = EtatMiseAJour::Inconnu {
            raison: RaisonInconnue::ReponseIllisible,
        };
        for corps in [
            "",
            "<html>Bad gateway</html>",
            "{\"tag_name\": ",
            "[]",
            "null",
            "{}",
            r#"{"name":"sans tag"}"#,
            r#"{"tag_name": 12}"#,
            r#"{"tag_name":"v1.0.0","assets":"oups"}"#,
        ] {
            assert_eq!(
                decider(&v("0.1.0"), &ok(corps.to_string())),
                illisible,
                "{corps:?}"
            );
        }
        assert_eq!(
            decider(&v("0.1.0"), &ok(r#"{"tag_name":"nightly"}"#.into())),
            EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::VersionIllisible
            }
        );
    }

    #[test]
    fn les_champs_facultatifs_peuvent_manquer_ou_etre_nuls() {
        let etat = decider(
            &v("0.1.0"),
            &ok(r#"{"tag_name":"v0.2.0","name":null,"body":null,"published_at":null,"html_url":null,"assets":null}"#.into()),
        );
        assert_eq!(
            etat,
            EtatMiseAJour::Disponible {
                version: "0.2.0".into(),
                date: None,
                notes: String::new(),
                page: PAGE_VERSIONS.into(),
                installateur: None,
            }
        );
        // Un champ en trop (ou de nouveaux champs côté GitHub) ne gêne pas.
        assert!(decider(
            &v("0.1.0"),
            &ok(r#"{"tag_name":"0.2.0","futur":{"a":1}}"#.into())
        )
        .est_disponible());
    }

    #[test]
    fn les_liens_douteux_sont_remplaces() {
        let corps = r#"{"tag_name":"v0.2.0","html_url":"javascript:alert(1)","assets":[
            {"name":"setup.exe","size":1,"browser_download_url":"http://evil.example/setup.exe"}]}"#;
        let EtatMiseAJour::Disponible {
            page, installateur, ..
        } = decider(&v("0.1.0"), &ok(corps.into()))
        else {
            panic!()
        };
        assert_eq!(page, PAGE_VERSIONS);
        assert_eq!(installateur, None);
    }

    #[test]
    fn l_installateur_se_reconnait_a_son_nom() {
        for oui in [
            "Retro-EQ-Installateur-0.2.0.exe",
            "retro-eq-setup-0.2.0.EXE",
            "RetroEQ_Setup.exe",
        ] {
            assert!(est_un_installateur(oui), "{oui}");
        }
        for non in [
            "retro-eq-0.2.0.zip",
            "retro-eq.exe",
            "installateur.zip",
            "setup.msi",
        ] {
            assert!(!est_un_installateur(non), "{non}");
        }
    }

    /// Le nom du fichier que la publication joint à la version doit être reconnu par
    /// `est_un_installateur`, sinon l'appli n'afficherait jamais « Installateur : … ». On lit
    /// le nom dans les trois fichiers qui le fixent : le script Inno Setup, le script de
    /// construction et le workflow de publication.
    #[test]
    fn le_nom_de_l_installateur_publie_est_reconnu() {
        const ISS: &str = include_str!("../../../installateur/retro-eq.iss");
        const CONSTRUIRE: &str = include_str!("../../../installateur/construire.ps1");
        const RELEASE: &str = include_str!("../../../.github/workflows/release.yml");

        // Le texte entre `debut` et `.exe`, `.exe` compris, sur la première ligne qui le contient.
        let nom_dans = |texte: &str, debut: &str| -> String {
            let ligne = texte
                .lines()
                .find(|l| l.contains(debut))
                .unwrap_or_else(|| panic!("« {debut} » absent"));
            let depart = ligne.find(debut).unwrap_or(0) + debut.len();
            let reste = &ligne[depart..];
            let fin = reste.find(".exe").map_or(reste.len(), |i| i + 4);
            reste[..fin].to_string()
        };
        let version = "0.2.0";

        // Inno Setup : `OutputBaseFilename=retro-eq-{#AppVersion}-installateur-x64` (sans `.exe`).
        let iss = ISS
            .lines()
            .find_map(|l| l.strip_prefix("OutputBaseFilename="))
            .unwrap_or_else(|| panic!("OutputBaseFilename absent"))
            .trim()
            .replace("{#AppVersion}", version)
            + ".exe";
        // construire.ps1 : `"retro-eq-$version-installateur-x64.exe"`.
        let ps1 = nom_dans(CONSTRUIRE, "\"retro-eq-").replace("$version", version);
        let ps1 = format!("retro-eq-{ps1}");
        // release.yml : `installateur/sortie/retro-eq-$env:VERSION-installateur-x64.exe`.
        let yml = nom_dans(RELEASE, "installateur/sortie/").replace("$env:VERSION", version);

        assert_eq!(iss, "retro-eq-0.2.0-installateur-x64.exe");
        assert_eq!(ps1, iss, "construire.ps1 attend un autre nom que Inno Setup");
        assert_eq!(yml, iss, "release.yml joint un autre nom que Inno Setup");
        assert!(est_un_installateur(&iss), "{iss} n'est pas reconnu");

        // L'archive jointe en plus ne doit pas être prise pour l'installateur.
        let zip = nom_dans(RELEASE, "dist/retro-eq-").replace("$env:VERSION", version);
        assert!(zip.contains("windows-x64"), "{zip}");
        assert!(!est_un_installateur(&format!("retro-eq-{zip}")));
    }

    #[test]
    fn les_notes_sont_du_texte_brut_tronque() {
        assert_eq!(notes_en_texte_brut("a\r\n\r\n\r\n\r\nb  \n"), "a\n\nb");
        // Caractères de contrôle et de réécriture du sens de lecture.
        assert_eq!(
            notes_en_texte_brut("a\u{0007}b\u{202E}c\u{1b}[31m"),
            "abc[31m"
        );
        // Pas d'interprétation : le HTML reste du texte.
        assert_eq!(notes_en_texte_brut("<b>x</b>"), "<b>x</b>");
        let long = "é".repeat(5_000);
        let coupe = notes_en_texte_brut(&long);
        assert_eq!(coupe.chars().count(), NOTES_MAX_CARACTERES + 1);
        assert!(coupe.ends_with('…'));
        let lignes: Vec<String> = (0..30).map(|i| format!("ligne {i}")).collect();
        let coupe = notes_en_texte_brut(&lignes.join("\n"));
        assert_eq!(coupe.lines().count(), NOTES_MAX_LIGNES + 1);
        assert!(coupe.ends_with("\n…"));
        assert_eq!(notes_en_texte_brut(""), "");
    }

    #[test]
    fn un_etat_garde_peut_etre_perime() {
        let dispo = decider(&v("0.1.0"), &ok(release("v0.2.0", false, false)));
        assert!(dispo.clone().pour_version(&v("0.1.5")).est_disponible());
        assert_eq!(
            dispo.clone().pour_version(&v("0.2.0")),
            EtatMiseAJour::AJour
        );
        assert_eq!(dispo.pour_version(&v("1.0.0")), EtatMiseAJour::AJour);
        let inconnu = EtatMiseAJour::Inconnu {
            raison: RaisonInconnue::Reseau,
        };
        assert_eq!(inconnu.clone().pour_version(&v("1.0.0")), inconnu);
    }

    #[test]
    fn la_sauvegarde_passe_par_json_et_les_vieux_fichiers_se_chargent() {
        let s = MiseAJourSauvee {
            derniere_verification_ms: Some(42),
            derniere_reponse_ms: Some(43),
            etat: Some(decider(&v("0.1.0"), &ok(release("v0.2.0", false, false)))),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<MiseAJourSauvee>(&json).unwrap(), s);
        assert_eq!(
            serde_json::from_str::<MiseAJourSauvee>("{}").unwrap(),
            MiseAJourSauvee::default()
        );
        assert_eq!(s.dernier_contact_ms(), Some(43));
        assert_eq!(MiseAJourSauvee::default().dernier_contact_ms(), None);
        let inconnu = MiseAJourSauvee {
            derniere_verification_ms: None,
            derniere_reponse_ms: None,
            etat: Some(EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::Http(500),
            }),
        };
        let json = serde_json::to_string(&inconnu).unwrap();
        assert_eq!(
            serde_json::from_str::<MiseAJourSauvee>(&json).unwrap(),
            inconnu
        );
    }
}
