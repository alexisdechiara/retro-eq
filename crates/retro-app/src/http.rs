//! Le client HTTP de l'appli : une seule fonction, « va chercher cette adresse ».
//!
//! C'est un trait (`ClientHttp`) pour la même raison que le moteur audio : le contrôleur et
//! ses tests ne doivent jamais toucher le vrai réseau. Le vrai client s'appuie sur `ureq`
//! (synchrone, sans moteur asynchrone à embarquer) ; les tests utilisent un faux qui
//! rejoue des réponses écrites à l'avance et note les adresses demandées.
//!
//! Deux usages, un seul client : la base AutoEQ (ADR-0013, `telecharger`) et la vérification
//! des mises à jour sur GitHub (`controleur/mises_a_jour.rs`).
//!
//! Ce qui part sur le réseau : des requêtes GET en HTTPS, sans compte ni jeton, avec un
//! `User-Agent` générique (`retro-eq/<version>`). Le serveur voit l'adresse IP de
//! l'utilisateur, comme n'importe quel site. Le TLS et les certificats sont ceux du système
//! (Schannel sous Windows).

use std::time::Duration;
use thiserror::Error;

/// Délai maximal de toute la requête (connexion, réponse et lecture comprises) : assez pour
/// l'index AutoEQ (environ 0,9 Mo), et la vérification de version n'attend jamais devant l'écran.
const DELAI_TOTAL: Duration = Duration::from_secs(30);
/// Délai maximal pour établir la connexion.
const DELAI_CONNEXION: Duration = Duration::from_secs(6);

/// La requête n'a pas abouti : pas de réseau, délai dépassé, certificat refusé, réponse
/// trop grosse… Une réponse du serveur, même en erreur (404, 403), n'est **pas** une
/// `ErreurHttp` : elle arrive en `Ok`, avec son statut.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErreurHttp {
    /// Pas de réseau, serveur injoignable, délai dépassé, TLS refusé, adresse en `http://`.
    #[error("pas de connexion : {0}")]
    Connexion(String),
    /// La réponse dépasse la taille permise : on n'en lit pas plus.
    #[error("réponse trop volumineuse")]
    TropVolumineuse,
}

/// Ce que le serveur a répondu, brut : statut, en-têtes et corps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReponseHttp {
    pub statut: u16,
    pub entetes: Vec<(String, String)>,
    pub corps: Vec<u8>,
}

/// Va chercher une adresse. `Send + Sync` : la requête part sur un autre fil que celui de
/// l'interface, pour qu'elle ne se fige jamais pendant un téléchargement.
pub trait ClientHttp: Send + Sync {
    /// GET `url` avec ces en-têtes en plus, en refusant un corps de plus de `limite_octets`.
    fn obtenir(
        &self,
        url: &str,
        entetes: &[(&str, &str)],
        limite_octets: u64,
    ) -> Result<ReponseHttp, ErreurHttp>;
}

/// Pourquoi un téléchargement de la base AutoEQ a échoué. Le texte (`Display`) est en
/// français et sert aux journaux ; l'utilisateur lit `textes::erreur`, dans sa langue.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErreurReseau {
    /// Pas de réseau, serveur injoignable, délai dépassé, TLS refusé.
    #[error("pas de connexion : {0}")]
    Connexion(String),
    /// Le serveur a répondu, mais pas par un succès (404 : le fichier n'existe plus).
    #[error("le serveur a répondu {0}")]
    Http(u16),
    /// La réponse dépasse la taille permise : on n'en lit pas plus.
    #[error("réponse trop volumineuse")]
    TropVolumineuse,
}

/// Télécharge les octets d'une adresse : pour ce cas, tout statut hors 2xx est une erreur
/// (contrairement à la vérification des mises à jour, qui lit même un 404).
pub fn telecharger(
    client: &dyn ClientHttp,
    url: &str,
    limite_octets: u64,
) -> Result<Vec<u8>, ErreurReseau> {
    let reponse = client
        .obtenir(url, &[], limite_octets)
        .map_err(|e| match e {
            ErreurHttp::Connexion(m) => ErreurReseau::Connexion(m),
            ErreurHttp::TropVolumineuse => ErreurReseau::TropVolumineuse,
        })?;
    if (200..300).contains(&reponse.statut) {
        Ok(reponse.corps)
    } else {
        Err(ErreurReseau::Http(reponse.statut))
    }
}

/// Le vrai client, basé sur `ureq` (HTTPS seulement, TLS du système).
pub struct ClientUreq {
    agent: ureq::Agent,
}

impl ClientUreq {
    pub fn nouveau() -> ClientUreq {
        // ureq ne choisit pas le TLS du système tout seul : on le demande, avec les
        // certificats du système (Windows : le magasin de Windows, Linux : OpenSSL, qui lit
        // aussi `SSL_CERT_FILE`). Aucune liste de certificats n'est embarquée.
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(DELAI_TOTAL))
            .timeout_connect(Some(DELAI_CONNEXION))
            .https_only(true)
            // Un User-Agent générique : le nom et la version de l'appli, rien de personnel.
            .user_agent(format!("retro-eq/{}", env!("CARGO_PKG_VERSION")))
            // Un 404 ou un 403 est une réponse à lire, pas une panne : chaque usage
            // interprète le statut à sa façon.
            .http_status_as_error(false)
            .tls_config(tls)
            .build();
        ClientUreq {
            agent: config.into(),
        }
    }
}

impl Default for ClientUreq {
    fn default() -> Self {
        ClientUreq::nouveau()
    }
}

impl ClientHttp for ClientUreq {
    fn obtenir(
        &self,
        url: &str,
        entetes: &[(&str, &str)],
        limite_octets: u64,
    ) -> Result<ReponseHttp, ErreurHttp> {
        let mut requete = self.agent.get(url);
        for (nom, valeur) in entetes {
            requete = requete.header(*nom, *valeur);
        }
        let mut reponse = requete.call().map_err(convertir)?;
        let statut = reponse.status().as_u16();
        let entetes = reponse
            .headers()
            .iter()
            .filter_map(|(nom, valeur)| {
                Some((nom.as_str().to_string(), valeur.to_str().ok()?.to_string()))
            })
            .collect();
        let corps = reponse
            .body_mut()
            .with_config()
            .limit(limite_octets)
            .read_to_vec()
            .map_err(convertir)?;
        Ok(ReponseHttp {
            statut,
            entetes,
            corps,
        })
    }
}

fn convertir(erreur: ureq::Error) -> ErreurHttp {
    match erreur {
        ureq::Error::BodyExceedsLimit(_) => ErreurHttp::TropVolumineuse,
        autre => ErreurHttp::Connexion(autre.to_string()),
    }
}

/// Essai réel contre api.github.com, ignoré par défaut (il faut du réseau) :
/// `cargo test -p retro-app reel -- --ignored --nocapture`. Derrière un proxy qui réécrit les
/// certificats (sessions cloud), donner son paquet : `SSL_CERT_FILE=/chemin/ca-bundle.crt`.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod essai_reel {
    use super::*;
    use crate::controleur::DemandeVerification;
    use retro_core::mises_a_jour::{decider, Version};

    #[test]
    #[ignore = "contacte api.github.com"]
    fn le_vrai_point_d_acces_repond_et_se_lit() {
        let client = ClientUreq::nouveau();
        let reponse = DemandeVerification::nouvelle().envoyer(&client);
        println!("réponse brute : {reponse:?}");
        let reponse = reponse.expect("GitHub aurait dû répondre");
        let etat = decider(&Version::analyser("0.1.0").unwrap(), &reponse);
        println!("statut {} → {etat:?}", reponse.statut);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests_reseau {
    use super::*;

    #[test]
    fn le_vrai_client_refuse_le_http_clair() {
        // Pas de réseau nécessaire : l'adresse est refusée avant toute connexion.
        let reel = ClientUreq::nouveau();
        assert!(matches!(
            reel.obtenir("http://exemple.invalid/", &[], 10),
            Err(ErreurHttp::Connexion(_))
        ));
    }

    #[test]
    fn telecharger_traduit_les_statuts_et_les_erreurs() {
        let faux = factice::ClientFactice::nouveau();
        faux.texte("https://x/a", "abc");
        assert_eq!(telecharger(&faux, "https://x/a", 10), Ok(b"abc".to_vec()));
        assert_eq!(
            telecharger(&faux, "https://x/b", 10),
            Err(ErreurReseau::Http(404))
        );
        assert_eq!(
            telecharger(&faux, "https://x/a", 2),
            Err(ErreurReseau::TropVolumineuse)
        );
        faux.couper_le_reseau();
        assert!(matches!(
            telecharger(&faux, "https://x/a", 10),
            Err(ErreurReseau::Connexion(_))
        ));
        assert_eq!(faux.nombre_de_requetes(), 4);
    }

    #[test]
    fn le_faux_sert_les_reponses_en_file_et_note_les_en_tetes() {
        let faux = factice::ClientFactice::nouveau();
        faux.repondre(Ok(ReponseHttp {
            statut: 200,
            entetes: vec![],
            corps: b"un".to_vec(),
        }));
        let r = faux
            .obtenir("https://x/", &[("Accept", "a/b")], 10)
            .unwrap();
        assert_eq!(r.corps, b"un");
        assert_eq!(
            faux.requetes.lock().unwrap()[0].1,
            vec![("Accept".to_string(), "a/b".to_string())]
        );
    }
}

/// Un faux client pour les tests : des réponses par adresse ou en file, et la liste des
/// requêtes reçues. Sans réponse préparée, une adresse répond 404.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod factice {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    /// Les adresses demandées (avec leurs en-têtes), partagées avec le test.
    pub type Requetes = Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>;

    pub struct ClientFactice {
        par_adresse: Mutex<HashMap<String, Result<ReponseHttp, ErreurHttp>>>,
        file: Mutex<VecDeque<Result<ReponseHttp, ErreurHttp>>>,
        pub requetes: Requetes,
    }

    impl ClientFactice {
        pub fn nouveau() -> ClientFactice {
            ClientFactice {
                par_adresse: Mutex::new(HashMap::new()),
                file: Mutex::new(VecDeque::new()),
                requetes: Requetes::default(),
            }
        }

        /// Prépare la prochaine réponse (une file : première préparée, première servie),
        /// pour les adresses sans réponse propre.
        pub fn repondre(&self, reponse: Result<ReponseHttp, ErreurHttp>) -> &Self {
            self.file.lock().unwrap().push_back(reponse);
            self
        }

        /// Prépare la réponse d'une adresse (elle reste servie à chaque demande).
        pub fn repondre_a(&self, url: &str, reponse: Result<ReponseHttp, ErreurHttp>) {
            self.par_adresse
                .lock()
                .unwrap()
                .insert(url.to_string(), reponse);
        }

        /// Une adresse qui répond 200 avec ce texte.
        pub fn texte(&self, url: &str, texte: &str) {
            self.repondre_a(
                url,
                Ok(ReponseHttp {
                    statut: 200,
                    entetes: vec![],
                    corps: texte.as_bytes().to_vec(),
                }),
            );
        }

        /// Les adresses déjà préparées ne répondent plus que par une panne de connexion.
        pub fn couper_le_reseau(&self) {
            for reponse in self.par_adresse.lock().unwrap().values_mut() {
                *reponse = Err(ErreurHttp::Connexion("réseau coupé".into()));
            }
        }

        pub fn nombre_de_requetes(&self) -> usize {
            self.requetes.lock().unwrap().len()
        }

        /// Les adresses demandées, dans l'ordre.
        pub fn demandes(&self) -> Vec<String> {
            self.requetes
                .lock()
                .unwrap()
                .iter()
                .map(|(url, _)| url.clone())
                .collect()
        }
    }

    impl ClientHttp for ClientFactice {
        fn obtenir(
            &self,
            url: &str,
            entetes: &[(&str, &str)],
            limite_octets: u64,
        ) -> Result<ReponseHttp, ErreurHttp> {
            self.requetes.lock().unwrap().push((
                url.to_string(),
                entetes
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect(),
            ));
            let propre = self.par_adresse.lock().unwrap().get(url).cloned();
            let reponse = propre
                .or_else(|| self.file.lock().unwrap().pop_front())
                .unwrap_or_else(|| {
                    Ok(ReponseHttp {
                        statut: 404,
                        entetes: vec![],
                        corps: Vec::new(),
                    })
                })?;
            if reponse.corps.len() as u64 > limite_octets {
                return Err(ErreurHttp::TropVolumineuse);
            }
            Ok(reponse)
        }
    }
}
