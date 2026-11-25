//! La vérification des mises à jour, côté contrôleur.
//!
//! Le contrôleur ne fait aucune requête lui-même : il ne tourne que sur le fil de
//! l'interface, et une requête réseau peut durer dix secondes. Il fonctionne en deux temps,
//! comme un aller-retour :
//!
//! 1. [`Controleur::demander_verification`] décide s'il faut vérifier (une fois par jour au
//!    plus pour la vérification automatique, jamais si le réglage est coupé) et rend une
//!    [`DemandeVerification`], un petit paquet qui peut voyager vers un autre fil ;
//! 2. quelqu'un (le pont, sur un fil à part) envoie la demande avec un `ClientHttp` et rend
//!    la réponse à [`Controleur::recevoir_verification`].
//!
//! Les tests font les deux temps à la suite, avec un faux client : aucun fil, aucun réseau.
//!
//! Hors ligne ou en panne, l'état connu avant reste affiché et une phrase calme explique
//! pourquoi on n'a pas pu vérifier.

use super::*;
use crate::http::{ClientHttp, ErreurHttp};
use retro_core::mises_a_jour::{
    self as maj, EtatMiseAJour, MiseAJourSauvee, RaisonInconnue, ReponseApi, Version,
};

/// Ce que le contrôleur sait de la vérification en cours et de la dernière.
pub(super) struct EtatVerification {
    /// Ce qui est gardé sur le disque : date de la dernière vérification et résultat.
    pub(super) sauve: MiseAJourSauvee,
    /// Une demande est partie et la réponse n'est pas revenue.
    pub(super) en_cours: bool,
    /// Pourquoi la dernière tentative a échoué (réseau, limite, panne). Pas gardé sur le
    /// disque : au lancement suivant, on repart sans reproche.
    pub(super) probleme: Option<RaisonInconnue>,
    /// La version qui tourne. Un champ plutôt qu'une constante pour que les tests puissent
    /// jouer « une version plus ancienne ».
    pub(super) version_courante: Version,
}

impl Default for EtatVerification {
    fn default() -> Self {
        EtatVerification {
            sauve: MiseAJourSauvee::default(),
            en_cours: false,
            probleme: None,
            // Illisible (impossible avec Cargo) : 0.0.0, qui propose tout ce qui est publié.
            version_courante: Version::analyser(env!("CARGO_PKG_VERSION")).unwrap_or_default(),
        }
    }
}

/// Une requête prête à partir : de quoi interroger GitHub sur un autre fil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandeVerification {
    url: String,
    entetes: Vec<(String, String)>,
}

impl DemandeVerification {
    pub(crate) fn nouvelle() -> DemandeVerification {
        DemandeVerification {
            url: maj::url_derniere_version(),
            entetes: vec![
                (
                    "Accept".to_string(),
                    "application/vnd.github+json".to_string(),
                ),
                // Seulement le nom et la version de l'appli : rien de personnel.
                (
                    "User-Agent".to_string(),
                    format!("retro-eq/{}", env!("CARGO_PKG_VERSION")),
                ),
                ("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string()),
            ],
        }
    }

    /// Envoie la requête. Bloquant : à appeler sur un fil à part, jamais sur celui de
    /// l'interface.
    pub fn envoyer(&self, client: &dyn ClientHttp) -> Result<ReponseApi, ErreurHttp> {
        let entetes: Vec<(&str, &str)> = self
            .entetes
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        client.obtenir(&self.url, &entetes)
    }
}

impl Controleur {
    /// Relit `mises-a-jour.json`. Un fichier illisible n'est pas une raison d'alarmer
    /// l'utilisateur : on repart comme au premier lancement.
    pub(super) fn charger_mises_a_jour(&mut self) {
        if let Ok(sauve) = self.depot.charger_mises_a_jour() {
            self.maj.sauve = sauve;
        }
        // Ce qui a été annoncé a peut-être été installé depuis.
        if let Some(etat) = self.maj.sauve.etat.take() {
            self.maj.sauve.etat = Some(etat.pour_version(&self.maj.version_courante));
        }
    }

    fn sauver_mises_a_jour(&mut self) {
        if let Err(e) = self.depot.sauver_mises_a_jour(&self.maj.sauve) {
            self.signaler(e.into());
        }
    }

    /// La vérification automatique du démarrage est-elle due à cet instant ?
    pub fn verification_automatique_due(&self, t: u64) -> bool {
        maj::verification_due(
            self.reglages.verifier_mises_a_jour,
            self.maj.sauve.dernier_contact_ms(),
            t,
        )
    }

    /// Prépare une vérification, ou `None` s'il n'y a rien à faire : une demande est déjà
    /// partie ; ou, pour une vérification `automatique`, le réglage est coupé ou la
    /// dernière réponse date de moins d'un jour. Le bouton « Vérifier maintenant » passe
    /// `automatique = false` : l'utilisateur l'a demandé, on ne discute pas.
    ///
    /// Quand le réglage est coupé, seule la demande manuelle peut contacter le réseau.
    pub fn demander_verification(
        &mut self,
        automatique: bool,
        t: u64,
    ) -> Option<DemandeVerification> {
        if self.maj.en_cours || (automatique && !self.verification_automatique_due(t)) {
            return None;
        }
        self.maj.en_cours = true;
        Some(DemandeVerification::nouvelle())
    }

    /// Reçoit la réponse d'une demande. Une réponse de GitHub qui dit quelque chose
    /// (à jour, disponible, aucune version publiée) devient le nouvel état connu ; un échec
    /// (réseau, limite, panne, réponse illisible) laisse l'état connu en place.
    pub fn recevoir_verification(&mut self, resultat: Result<ReponseApi, ErreurHttp>, t: u64) {
        self.maj.en_cours = false;
        let etat = match resultat {
            Ok(reponse) => maj::decider(&self.maj.version_courante, &reponse),
            Err(_) => EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::Reseau,
            },
        };
        match etat {
            EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::Reseau,
            } => {
                // Pas de réponse du tout : rien n'est noté, la prochaine ouverture réessaiera.
                self.maj.probleme = Some(RaisonInconnue::Reseau);
            }
            EtatMiseAJour::Inconnu {
                raison:
                    raison @ (RaisonInconnue::LimiteAtteinte
                    | RaisonInconnue::ReponseIllisible
                    | RaisonInconnue::VersionIllisible
                    | RaisonInconnue::Http(_)),
            } => {
                // GitHub a répondu, mal : on le note pour ne pas insister à chaque lancement.
                self.maj.sauve.derniere_reponse_ms = Some(t);
                self.maj.probleme = Some(raison);
                self.sauver_mises_a_jour();
            }
            connu => {
                self.maj.sauve.derniere_verification_ms = Some(t);
                self.maj.sauve.derniere_reponse_ms = Some(t);
                self.maj.sauve.etat = Some(connu);
                self.maj.probleme = None;
                self.sauver_mises_a_jour();
            }
        }
    }

    /// Règle « Vérifier les mises à jour au démarrage ».
    pub fn regler_verifier_mises_a_jour(&mut self, valeur: bool) {
        self.reglages.verifier_mises_a_jour = valeur;
        self.sauver_reglages();
    }

    /// Ce que la section « Mises à jour » doit montrer.
    pub fn vue_mise_a_jour(&self, t: u64) -> VueMiseAJour {
        let l = self.reglages.langue;
        let mut vue = VueMiseAJour {
            en_cours: self.maj.en_cours,
            probleme: self
                .maj
                .probleme
                .as_ref()
                .map(|r| textes::probleme_de_verification(r, l))
                .unwrap_or_default(),
            ..VueMiseAJour::default()
        };
        if let Some(ms) = self.maj.sauve.derniere_verification_ms {
            let decalage = self.options.decalage_minutes;
            let rfc = dates::rfc3339(ms, decalage);
            let aujourd_hui = dates::rfc3339(t, decalage).get(..10) == rfc.get(..10);
            vue.verification = textes::verification_faite(
                aujourd_hui,
                &dates::jour_mois(&rfc),
                rfc.get(11..16).unwrap_or(""),
                l,
            );
        }
        match &self.maj.sauve.etat {
            Some(EtatMiseAJour::AJour) => vue.etat = 1,
            Some(EtatMiseAJour::Disponible {
                version,
                date,
                notes,
                page,
                installateur,
            }) => {
                vue.etat = 2;
                vue.version = version.clone();
                vue.date = date
                    .as_deref()
                    .map(|d| textes::date_de_version(d, l))
                    .unwrap_or_default();
                vue.notes = notes.clone();
                vue.page = page.clone();
                vue.installateur = installateur
                    .as_ref()
                    .map(|i| textes::installateur(&i.nom, i.taille, l))
                    .unwrap_or_default();
            }
            Some(EtatMiseAJour::Inconnu {
                raison: RaisonInconnue::AucuneVersionPubliee,
            }) => vue.etat = 3,
            // Un autre « inconnu » n'est jamais gardé ; un vieux fichier en aurait un : on l'ignore.
            Some(EtatMiseAJour::Inconnu { .. }) | None => {}
        }
        vue
    }
}
