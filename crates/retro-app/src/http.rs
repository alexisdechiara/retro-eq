//! Le client HTTP de l'appli : une seule fonction, « va chercher cette adresse ».
//!
//! C'est un trait (`ClientHttp`) pour la même raison que le moteur audio : le contrôleur et
//! ses tests ne doivent jamais toucher le vrai réseau. Le vrai client s'appuie sur `ureq`
//! (synchrone, sans moteur asynchrone à embarquer) ; les tests utilisent un faux qui
//! rejoue des réponses écrites à l'avance et note les adresses demandées.
//!
//! Ce qui part sur le réseau : une requête GET vers l'API publique de GitHub, sans compte ni
//! jeton, avec un `User-Agent` générique (`retro-eq/<version>`). GitHub voit l'adresse IP
//! de l'utilisateur, comme n'importe quel site.

use retro_core::mises_a_jour::ReponseApi;
use std::time::Duration;
use thiserror::Error;

/// Délai maximal de toute la requête (connexion, réponse et lecture comprises).
const DELAI_TOTAL: Duration = Duration::from_secs(10);
/// Délai maximal pour établir la connexion.
const DELAI_CONNEXION: Duration = Duration::from_secs(5);
/// Taille maximale de la réponse lue : la réponse de GitHub fait quelques ko, tout ce qui
/// est plus gros n'est pas ce qu'on attend.
const TAILLE_MAX_OCTETS: u64 = 1024 * 1024;

/// La requête n'a pas abouti : pas de réseau, délai dépassé, certificat refusé, réponse
/// trop grosse… Une réponse de GitHub, même en erreur (404, 403), n'est **pas** une
/// `ErreurHttp` : elle arrive en `Ok`, avec son statut.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct ErreurHttp(pub String);

/// Va chercher une adresse. `Send + Sync` : la requête part sur un autre fil que celui de
/// l'interface.
pub trait ClientHttp: Send + Sync {
    fn obtenir(&self, url: &str, entetes: &[(&str, &str)]) -> Result<ReponseApi, ErreurHttp>;
}

/// Le vrai client, basé sur `ureq`.
pub struct ClientUreq {
    agent: ureq::Agent,
}

impl ClientUreq {
    pub fn nouveau() -> ClientUreq {
        // Sous Windows, le TLS est celui du système (Schannel) et ses certificats : ureq ne
        // choisit pas cette variante tout seul. Ailleurs, rustls et ses certificats embarqués.
        #[cfg(windows)]
        let tls = ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        #[cfg(not(windows))]
        let tls = ureq::tls::TlsConfig::default();
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(DELAI_TOTAL))
            .timeout_connect(Some(DELAI_CONNEXION))
            // Un 404 ou un 403 est une réponse à lire, pas une panne : c'est le module
            // `mises_a_jour` du domaine qui les interprète.
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
    fn obtenir(&self, url: &str, entetes: &[(&str, &str)]) -> Result<ReponseApi, ErreurHttp> {
        let mut requete = self.agent.get(url);
        for (nom, valeur) in entetes {
            requete = requete.header(*nom, *valeur);
        }
        let mut reponse = requete.call().map_err(|e| ErreurHttp(e.to_string()))?;
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
            .limit(TAILLE_MAX_OCTETS)
            .read_to_string()
            .map_err(|e| ErreurHttp(e.to_string()))?;
        Ok(ReponseApi {
            statut,
            entetes,
            corps,
        })
    }
}

/// Essai réel contre api.github.com, ignoré par défaut (il faut du réseau) :
/// `cargo test -p retro-app reel -- --ignored --nocapture`.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod essai_reel {
    use super::*;
    use crate::controleur::DemandeVerification;
    use retro_core::mises_a_jour::{decider, Version};

    #[test]
    #[ignore = "contacte api.github.com"]
    fn le_vrai_point_d_acces_repond_et_se_lit() {
        // Derrière un proxy qui réécrit les certificats (cas des sessions cloud), on peut
        // donner son paquet de certificats : RETRO_EQ_CA_BUNDLE=/chemin/ca-bundle.crt.
        let client = match std::env::var_os("RETRO_EQ_CA_BUNDLE") {
            Some(chemin) => {
                let pem = std::fs::read(chemin).unwrap();
                let certs: Vec<_> = ureq::tls::parse_pem(&pem)
                    .filter_map(|p| match p.ok()? {
                        ureq::tls::PemItem::Certificate(c) => Some(c),
                        _ => None,
                    })
                    .collect();
                let config = ureq::Agent::config_builder()
                    .timeout_global(Some(DELAI_TOTAL))
                    .http_status_as_error(false)
                    .tls_config(
                        ureq::tls::TlsConfig::builder()
                            .root_certs(ureq::tls::RootCerts::new_with_certs(&certs))
                            .build(),
                    )
                    .build();
                ClientUreq {
                    agent: config.into(),
                }
            }
            None => ClientUreq::nouveau(),
        };
        let reponse = DemandeVerification::nouvelle().envoyer(&client);
        println!("réponse brute : {reponse:?}");
        let reponse = reponse.expect("GitHub aurait dû répondre");
        let etat = decider(&Version::analyser("0.1.0").unwrap(), &reponse);
        println!("statut {} → {etat:?}", reponse.statut);
    }
}

/// Un faux client pour les tests : des réponses préparées, et la liste des requêtes reçues.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod factice {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Les adresses demandées (avec leurs en-têtes), partagées avec le test.
    pub type Requetes = Arc<Mutex<Vec<(String, Vec<(String, String)>)>>>;

    pub struct ClientFactice {
        reponses: Mutex<VecDeque<Result<ReponseApi, ErreurHttp>>>,
        pub requetes: Requetes,
    }

    impl ClientFactice {
        pub fn nouveau() -> ClientFactice {
            ClientFactice {
                reponses: Mutex::new(VecDeque::new()),
                requetes: Requetes::default(),
            }
        }

        /// Prépare la prochaine réponse (une file : première préparée, première servie).
        pub fn repondre(&self, reponse: Result<ReponseApi, ErreurHttp>) -> &Self {
            self.reponses.lock().unwrap().push_back(reponse);
            self
        }

        pub fn nombre_de_requetes(&self) -> usize {
            self.requetes.lock().unwrap().len()
        }
    }

    impl ClientHttp for ClientFactice {
        fn obtenir(&self, url: &str, entetes: &[(&str, &str)]) -> Result<ReponseApi, ErreurHttp> {
            self.requetes.lock().unwrap().push((
                url.to_string(),
                entetes
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect(),
            ));
            self.reponses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(ErreurHttp("pas de réponse préparée".into())))
        }
    }
}
