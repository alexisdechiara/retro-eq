//! Le réseau, derrière un trait : le contrôleur demande « les octets de cette adresse » et ne
//! sait pas si c'est `ureq` ou un faux de test qui répond. Même idée que `Moteur`
//! (ADR-0002) : ce qui touche au monde extérieur est remplaçable.
//!
//! Un seul usage pour l'instant : la base AutoEQ (ADR-0013). Les délais sont courts et
//! l'appli marche entièrement hors ligne : une panne de réseau est une erreur ordinaire,
//! jamais un plantage.

use std::time::Duration;
use thiserror::Error;

/// Pourquoi un téléchargement a échoué. Le texte (`Display`) est en français et sert aux
/// journaux ; l'utilisateur lit `textes::erreur`, dans sa langue.
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

/// Va chercher les octets d'une adresse. `Send + Sync` : le contrôleur le donne à un fil de
/// travail, pour que l'interface ne se fige jamais pendant un téléchargement.
pub trait Telechargeur: Send + Sync {
    /// Télécharge `url`, en refusant plus de `limite_octets` octets.
    fn obtenir(&self, url: &str, limite_octets: u64) -> Result<Vec<u8>, ErreurReseau>;
}

/// Le vrai téléchargeur, par `ureq` (HTTPS seulement, rustls).
pub struct TelechargeurUreq {
    agent: ureq::Agent,
}

impl TelechargeurUreq {
    pub fn nouveau() -> TelechargeurUreq {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(6)))
            .timeout_global(Some(Duration::from_secs(30)))
            .https_only(true)
            // Un User-Agent générique : le nom et la version de l'appli, rien de personnel.
            .user_agent(format!("retro-eq/{}", env!("CARGO_PKG_VERSION")))
            .build();
        TelechargeurUreq {
            agent: config.into(),
        }
    }
}

impl Telechargeur for TelechargeurUreq {
    fn obtenir(&self, url: &str, limite_octets: u64) -> Result<Vec<u8>, ErreurReseau> {
        let mut reponse = self.agent.get(url).call().map_err(convertir)?;
        reponse
            .body_mut()
            .with_config()
            .limit(limite_octets)
            .read_to_vec()
            .map_err(convertir)
    }
}

fn convertir(erreur: ureq::Error) -> ErreurReseau {
    match erreur {
        ureq::Error::StatusCode(code) => ErreurReseau::Http(code),
        ureq::Error::BodyExceedsLimit(_) => ErreurReseau::TropVolumineuse,
        autre => ErreurReseau::Connexion(autre.to_string()),
    }
}

#[cfg(test)]
pub use faux::FauxTelechargeur;

#[cfg(test)]
mod faux {
    use super::{ErreurReseau, Telechargeur};
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Un téléchargeur de test : des réponses préparées par adresse, et la liste des
    /// adresses demandées. Une adresse inconnue répond 404.
    #[derive(Default)]
    pub struct FauxTelechargeur {
        reponses: Mutex<HashMap<String, Result<Vec<u8>, ErreurReseau>>>,
        demandes: Mutex<Vec<String>>,
    }

    impl FauxTelechargeur {
        pub fn repondre(&self, url: &str, reponse: Result<Vec<u8>, ErreurReseau>) {
            if let Ok(mut r) = self.reponses.lock() {
                r.insert(url.to_string(), reponse);
            }
        }

        pub fn texte(&self, url: &str, texte: &str) {
            self.repondre(url, Ok(texte.as_bytes().to_vec()));
        }

        /// Plus aucune réponse : tout est « hors ligne » à partir de maintenant.
        pub fn couper_le_reseau(&self) {
            if let Ok(mut r) = self.reponses.lock() {
                for reponse in r.values_mut() {
                    *reponse = Err(ErreurReseau::Connexion("réseau coupé".into()));
                }
            }
        }

        pub fn demandes(&self) -> Vec<String> {
            self.demandes.lock().map(|d| d.clone()).unwrap_or_default()
        }
    }

    impl Telechargeur for FauxTelechargeur {
        fn obtenir(&self, url: &str, limite_octets: u64) -> Result<Vec<u8>, ErreurReseau> {
            if let Ok(mut d) = self.demandes.lock() {
                d.push(url.to_string());
            }
            let reponse = self
                .reponses
                .lock()
                .ok()
                .and_then(|r| r.get(url).cloned())
                .unwrap_or(Err(ErreurReseau::Http(404)));
            match reponse {
                Ok(octets) if octets.len() as u64 > limite_octets => {
                    Err(ErreurReseau::TropVolumineuse)
                }
                autre => autre,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_faux_repond_ou_404_et_note_les_demandes() {
        let faux = FauxTelechargeur::default();
        faux.texte("https://x/a", "abc");
        assert_eq!(faux.obtenir("https://x/a", 10), Ok(b"abc".to_vec()));
        assert_eq!(
            faux.obtenir("https://x/b", 10),
            Err(ErreurReseau::Http(404))
        );
        assert_eq!(
            faux.obtenir("https://x/a", 2),
            Err(ErreurReseau::TropVolumineuse)
        );
        assert_eq!(faux.demandes().len(), 3);
    }

    #[test]
    fn le_vrai_telechargeur_refuse_le_http_clair() {
        // Pas de réseau nécessaire : l'adresse est refusée avant toute connexion.
        let reel = TelechargeurUreq::nouveau();
        assert!(matches!(
            reel.obtenir("http://exemple.invalid/", 10),
            Err(ErreurReseau::Connexion(_))
        ));
    }
}
