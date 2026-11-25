//! Le contrôleur : l'état de l'appli et les intentions de l'utilisateur, en Rust pur.
//!
//! **Il ne connaît pas Slint.** Ses méthodes publiques sont ce que l'utilisateur veut faire
//! (`regler_bande`, `demarrer_test`, `repondre`…) et ses « vues » décrivent ce que chaque
//! écran doit montrer. C'est ce qui permet de le tester en entier sans fenêtre, sans Windows
//! et sans casque : un moteur simulé, un dossier temporaire, et l'heure passée en argument.
//! Le pont (`pont.rs`) fait le lien avec l'interface.
//!
//! Aucune méthode ne panique ni ne rend d'erreur à l'appelant : un échec devient une
//! [`Annonce`] (le message en bas de la fenêtre), que le pont vient chercher.

mod dates;
#[cfg(test)]
mod tests;
mod vues;

pub use vues::*;

use crate::textes;
use retro_core::formats::{self, ErreurFormat, Format};
use retro_core::{
    duel_verification, plan_avec_historique, ContexteReprise, Cote, Depot, Deroulement, Effet,
    Entree, ErreurDepot, ErreurProfil, ErreurReprise, EtatMoteurSauve, EtatTest, Evenement,
    FadersEtSupplementaires, HasardGraine, HasardReel, HistoriquePlan, Journal, Langue, Mode,
    Navigation, Origine, ParametresTest, Profil, RaisonPause, ReglagesAppli, ReglagesTest,
    ReglagesVariantes, Reponse, ResultatTest, Sujet, Version, Zone,
};
use retro_dsp::{Courbe, Spectre};
use retro_engine::{ErreurMoteur, EtatMoteur, Moteur, SortieAudio};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Gain le plus fort d'un fader, en dB (dans les deux sens).
const GAIN_MAX_DB: f64 = 12.0;
/// Préampli manuel le plus bas, en dB.
const PREAMPLI_MIN_DB: f64 = -24.0;
/// Nombre de points du spectre par défaut (bruit rose).
const POINTS_SPECTRE: usize = 96;

/// Ce qui peut empêcher une action. Le texte est celui montré à l'utilisateur.
#[derive(Debug, Error)]
pub enum ErreurControleur {
    #[error("{0}")]
    Depot(#[from] ErreurDepot),
    #[error("Le moteur audio a refusé : {0}")]
    Moteur(#[from] ErreurMoteur),
    #[error("{0}")]
    Profil(#[from] ErreurProfil),
    #[error("Fichier illisible : {0}")]
    Format(#[from] ErreurFormat),
    #[error("Réponse non enregistrée sur le disque : {0}")]
    Journal(ErreurDepot),
    #[error("Ce profil n’existe plus.")]
    ProfilInconnu,
    #[error("Aucun profil actif : choisis ou crée un profil d’abord.")]
    AucunProfil,
    #[error("Un test est en cours : termine-le ou arrête-le d’abord.")]
    TestEnCours,
    #[error("Enregistre ou annule d’abord tes changements dans l’égaliseur.")]
    BrouillonEnCours,
    #[error("Le moteur audio est introuvable : installe Equalizer APO pour lancer un test.")]
    MoteurAbsent,
    #[error("Égaliseur pas encore activé : termine le premier lancement pour lancer un test.")]
    MoteurNonAutorise,
    #[error("Ce test n’est pas encore disponible.")]
    ModeIndisponible,
    #[error("Choisis l’autre profil à comparer.")]
    AutreProfilManquant,
    #[error("Format de fichier non reconnu.")]
    FormatInconnu,
    #[error("Un fichier Wavelet ne peut pas être importé : c’est un format d’export seulement.")]
    ImportWavelet,
    #[error("Pas de résultat de test pour l’instant.")]
    AucunResultat,
    #[error("Impossible de reprendre le test interrompu : {0}")]
    Reprise(#[from] ErreurReprise),
    #[error("{action} : {source}")]
    Fichier {
        action: String,
        #[source]
        source: std::io::Error,
    },
}

type Issue<T> = Result<T, ErreurControleur>;

/// Ce que l'appli fixe au lancement et que le contrôleur ne peut pas deviner.
#[derive(Debug, Clone)]
pub struct Options {
    /// Où écrire les exports (`Documents\Retro EQ`).
    pub dossier_export: PathBuf,
    /// Equalizer APO n'a pas été trouvé : le moteur donné est un simulacre, on ne lui
    /// demande rien et on le dit à l'utilisateur.
    pub moteur_absent: bool,
    /// Décalage de l'heure locale par rapport à UTC, en minutes, pour dater les versions.
    pub decalage_minutes: i32,
    /// Graine du hasard. `None` : vrai hasard. Les tests en fixent une pour être rejouables.
    pub graine: Option<u64>,
}

/// Les choix de l'écran « Nouveau test ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoixTest {
    /// Identifiant du mode : « duels-en-jeu », « une-zone », « profil-contre-profil ».
    pub mode: String,
    pub precis: bool,
    pub intervalle_s: u32,
    /// Indice de la zone dans `Zone::TOUTES` (mode « une seule zone »).
    pub zone: usize,
    /// Nom de l'autre profil (mode « profil contre profil »).
    pub autre_profil: String,
}

/// L'égaliseur en cours de réglage, pas encore enregistré.
#[derive(Debug, Clone, PartialEq)]
struct Brouillon {
    faders: FadersEtSupplementaires,
    preampli_auto: bool,
    /// Le préampli choisi à la main ; ignoré en automatique.
    preampli_db: f64,
}

impl Brouillon {
    fn depuis(courbe: &Courbe) -> Brouillon {
        // Une version ne dit pas si son préampli était automatique : on le reconnaît à sa valeur.
        let auto = (courbe.preampli_db - courbe.preampli_auto_db()).abs() < 0.05;
        Brouillon {
            faders: FadersEtSupplementaires::depuis_courbe(courbe),
            preampli_auto: auto,
            preampli_db: courbe.preampli_db,
        }
    }

    fn courbe(&self) -> Courbe {
        let courbe = self.faders.vers_courbe(self.preampli_db);
        if self.preampli_auto {
            courbe.avec_preampli_auto()
        } else {
            courbe
        }
    }
}

/// Un test commencé : sa machine à états, son journal ouvert, et ce qu'il faut pour le conclure.
struct TestEnCours {
    deroulement: Deroulement,
    journal: Journal,
    test_id: String,
    profil_id: String,
    reference: Version,
    mode: Mode,
    /// L'autre profil (identifiant, nom) en « profil contre profil ».
    autre: Option<(String, String)>,
}

/// Le résultat du dernier test fini, avec de quoi l'activer ou revenir en arrière.
struct DernierResultat {
    resultat: ResultatTest,
    profil_id: String,
    courbe_reference: Courbe,
    version_reference: u32,
    /// La version ajoutée au profil par ce test (aucune en « profil contre profil »).
    version_nouvelle: Option<u32>,
    autre: Option<(String, String)>,
}

/// Voir la documentation du module.
pub struct Controleur {
    depot: Depot,
    /// `Box<dyn Moteur>` : le contrôleur ne sait pas s'il parle à Equalizer APO ou à un
    /// moteur simulé, il ne connaît que le trait (ADR-0002).
    moteur: Box<dyn Moteur>,
    options: Options,
    reglages: ReglagesAppli,
    profils: Vec<Profil>,
    /// Identifiant du profil actif.
    actif: Option<String>,
    brouillon: Option<Brouillon>,
    test: Option<TestEnCours>,
    resultat: Option<DernierResultat>,
    spectre: Spectre,
    choix: ChoixTest,
    plein_ecran: bool,
    annonce: Option<Annonce>,
    bips: u32,
    tirages: u64,
}

impl Controleur {
    /// Ouvre le dossier de données et remet l'appli dans l'état où elle doit être.
    ///
    /// C'est ici que se joue « une variante ne reste jamais active » : si l'utilisateur a
    /// autorisé l'écriture, la courbe de référence est réécrite dans le moteur à chaque
    /// démarrage (ADR-0007), puis un test interrompu est retrouvé et mis en pause.
    pub fn ouvrir(
        dossier: impl AsRef<Path>,
        moteur: Box<dyn Moteur>,
        options: Options,
        maintenant_ms: u64,
    ) -> Result<Controleur, ErreurControleur> {
        let depot = Depot::ouvrir(dossier)?;
        let mut annonce = None;
        let reglages = depot.charger_reglages().unwrap_or_else(|e| {
            annonce = Some(Annonce {
                texte: format!("Réglages illisibles, valeurs par défaut reprises : {e}"),
                erreur: true,
            });
            ReglagesAppli::default()
        });
        let liste = depot.lister_profils()?;
        if let Some(ignore) = liste.ignores.first() {
            annonce = Some(Annonce {
                texte: format!(
                    "Profil illisible laissé de côté ({}) : {}",
                    ignore.chemin.display(),
                    ignore.raison
                ),
                erreur: true,
            });
        }
        let choix = ChoixTest {
            mode: "duels-en-jeu".to_string(),
            precis: false,
            intervalle_s: reglages.intervalle_ms / 1000,
            zone: 1,
            autre_profil: String::new(),
        };
        let mut c = Controleur {
            depot,
            moteur,
            options,
            reglages,
            profils: liste.profils,
            actif: None,
            brouillon: None,
            test: None,
            resultat: None,
            spectre: Spectre::rose(POINTS_SPECTRE),
            choix,
            plein_ecran: false,
            annonce,
            bips: 0,
            tirages: 0,
        };
        c.retrouver_profil_actif(maintenant_ms)?;
        // La référence d'abord, quoi qu'il se soit passé avant.
        if let Err(e) = c.appliquer_courbe_du_jour() {
            c.signaler(e);
        }
        c.retrouver_test_interrompu(maintenant_ms);
        Ok(c)
    }

    // ----- Démarrage -----

    /// Retrouve le profil actif d'après `etat-moteur.json`. Au tout premier lancement, crée
    /// un profil à plat : il faut une référence pour pouvoir tester.
    fn retrouver_profil_actif(&mut self, t: u64) -> Issue<()> {
        let sauve = self.depot.charger_etat_moteur().unwrap_or(None);
        let connu = sauve
            .and_then(|e| e.profil_id)
            .filter(|id| self.profils.iter().any(|p| p.id() == id));
        if let Some(id) = connu {
            self.actif = Some(id);
            return Ok(());
        }
        if self.profils.is_empty() {
            let profil = Profil::nouveau(
                "mon-profil",
                "Mon profil",
                Courbe::default(),
                Origine::Preset,
                self.date(t),
            );
            self.depot.sauver_profil(&profil)?;
            self.profils.push(profil);
        }
        self.actif = self.profils.first().map(|p| p.id().to_string());
        self.sauver_etat_moteur()
    }

    /// Cherche un test interrompu (plantage, PC éteint) et le remet en pause, prêt à
    /// reprendre. S'il y en a plusieurs, seul le plus récent est gardé ; les autres sont
    /// notés abandonnés dans leur journal (rien n'est effacé).
    fn retrouver_test_interrompu(&mut self, t: u64) {
        let mut ids = match self.depot.tests_interrompus() {
            Ok(ids) => ids,
            Err(e) => return self.signaler(e.into()),
        };
        let Some(dernier) = ids.pop() else { return };
        for ancien in ids {
            self.noter_abandon(&ancien, t);
        }
        match self.reprendre_test(&dernier, t) {
            Ok(effets) => {
                self.executer(effets, t);
                self.annoncer(textes::ANNONCE_TEST_REPRIS);
            }
            Err(e) => {
                self.noter_abandon(&dernier, t);
                self.signaler(e);
            }
        }
    }

    fn noter_abandon(&mut self, test_id: &str, t: u64) {
        let ecriture = self
            .depot
            .ouvrir_journal(test_id)
            .and_then(|mut journal| journal.ajouter(&Entree::TestAbandonne { horodatage_ms: t }));
        if let Err(e) = ecriture {
            self.signaler(e.into());
        }
    }

    fn reprendre_test(&mut self, test_id: &str, t: u64) -> Issue<Vec<Effet>> {
        let lecture = self.depot.lire_journal(test_id)?;
        let Some(Entree::TestDemarre {
            profil_id,
            version_reference,
            mode,
            ..
        }) = lecture.entrees.first()
        else {
            return Err(ErreurReprise::SansDemarrage.into());
        };
        let profil = self
            .profils
            .iter()
            .find(|p| p.id() == profil_id)
            .ok_or(ErreurControleur::ProfilInconnu)?;
        let reference = profil
            .version(*version_reference)
            .ok_or(ErreurProfil::VersionInconnue(*version_reference))?
            .clone();
        let verification = duel_verification(&mut self.hasard());
        let (deroulement, effets) = Deroulement::reprendre_depuis(
            &lecture.entrees,
            ContexteReprise {
                reference: reference.clone(),
                variantes: ReglagesVariantes::default(),
                spectre: self.spectre.clone(),
                verification,
            },
            t,
        )?;
        // Le journal ne garde que la courbe de l'autre profil : on retrouve lequel c'était.
        let autre = match mode {
            Mode::ProfilContreProfil { autre, .. } => self
                .profils
                .iter()
                .find(|p| p.id() != profil_id && &p.version_active().courbe == autre)
                .map(|p| (p.id().to_string(), p.nom().to_string())),
            _ => None,
        };
        self.test = Some(TestEnCours {
            deroulement,
            journal: self.depot.ouvrir_journal(test_id)?,
            test_id: test_id.to_string(),
            profil_id: profil_id.clone(),
            reference,
            mode: mode.clone(),
            autre,
        });
        Ok(effets)
    }

    // ----- Petits outils -----

    fn date(&self, t: u64) -> String {
        dates::rfc3339(t, self.options.decalage_minutes)
    }

    /// Une source de hasard neuve à chaque tirage. Avec une graine fixée (tests), chaque
    /// tirage en dérive une différente, pour que deux tests de suite n'aient pas le même plan.
    fn hasard(&mut self) -> HasardSource {
        self.tirages += 1;
        match self.options.graine {
            Some(graine) => {
                HasardSource::Graine(HasardGraine::new(graine.wrapping_add(self.tirages)))
            }
            None => HasardSource::Reel(HasardReel::new()),
        }
    }

    fn annoncer(&mut self, texte: impl Into<String>) {
        // Une erreur déjà en attente ne doit pas être remplacée par une confirmation.
        if self.annonce.as_ref().is_some_and(|a| a.erreur) {
            return;
        }
        self.annonce = Some(Annonce {
            texte: texte.into(),
            erreur: false,
        });
    }

    fn signaler(&mut self, erreur: ErreurControleur) {
        self.annonce = Some(Annonce {
            texte: erreur.to_string(),
            erreur: true,
        });
    }

    /// Exécute une action : son message de réussite devient une annonce, son erreur aussi.
    fn tenter(&mut self, action: impl FnOnce(&mut Self) -> Issue<Option<String>>) -> bool {
        match action(self) {
            Ok(Some(message)) => {
                self.annoncer(message);
                true
            }
            Ok(None) => true,
            Err(e) => {
                self.signaler(e);
                false
            }
        }
    }

    fn profil(&self, id: &str) -> Issue<&Profil> {
        self.profils
            .iter()
            .find(|p| p.id() == id)
            .ok_or(ErreurControleur::ProfilInconnu)
    }

    fn profil_mut(&mut self, id: &str) -> Issue<&mut Profil> {
        self.profils
            .iter_mut()
            .find(|p| p.id() == id)
            .ok_or(ErreurControleur::ProfilInconnu)
    }

    fn profil_actif(&self) -> Issue<&Profil> {
        let id = self.actif.as_deref().ok_or(ErreurControleur::AucunProfil)?;
        self.profil(id).map_err(|_| ErreurControleur::AucunProfil)
    }

    fn exiger_pas_de_test(&self) -> Issue<()> {
        if self.test_en_cours() {
            Err(ErreurControleur::TestEnCours)
        } else {
            Ok(())
        }
    }

    fn sauver_reglages(&mut self) {
        if let Err(e) = self.depot.sauver_reglages(&self.reglages) {
            self.signaler(e.into());
        }
    }

    /// Un identifiant de profil libre, utilisable comme nom de fichier.
    fn nouvel_id(&self, t: u64) -> String {
        let base = format!("p{t}");
        let mut id = base.clone();
        let mut n = 2;
        while self.profils.iter().any(|p| p.id() == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    fn ajouter_profil(&mut self, profil: Profil) -> Issue<()> {
        self.depot.sauver_profil(&profil)?;
        self.profils.push(profil);
        self.profils.sort_by(|a, b| a.id().cmp(b.id()));
        Ok(())
    }

    // ----- Moteur -----

    /// Le moteur peut-il recevoir une courbe ? Non s'il est absent, et non tant que
    /// l'utilisateur n'a pas donné son accord : avant, rien n'est écrit chez Equalizer APO.
    fn moteur_utilisable(&self) -> bool {
        !self.options.moteur_absent && self.reglages.moteur_autorise
    }

    fn appliquer_au_moteur(&mut self, courbe: &Courbe) -> Issue<()> {
        if self.moteur_utilisable() {
            self.moteur.appliquer(courbe)?;
        }
        Ok(())
    }

    /// La courbe de tous les jours : le brouillon de l'égaliseur s'il y en a un, sinon la
    /// version active du profil actif.
    fn courbe_du_jour(&self) -> Option<Courbe> {
        match &self.brouillon {
            Some(b) => Some(b.courbe()),
            None => self
                .profil_actif()
                .ok()
                .map(|p| p.version_active().courbe.clone()),
        }
    }

    /// Met dans le moteur ce qui doit s'entendre hors test.
    fn appliquer_courbe_du_jour(&mut self) -> Issue<()> {
        if !self.moteur_utilisable() {
            return Ok(());
        }
        if self.reglages.egaliseur_coupe {
            self.moteur.couper()?;
        } else if let Some(courbe) = self.courbe_du_jour() {
            self.moteur.appliquer(&courbe)?;
        }
        Ok(())
    }

    /// Note dans `etat-moteur.json` la courbe qui doit être active : c'est elle qui sera
    /// remise au prochain démarrage. Jamais une variante, jamais un brouillon.
    fn sauver_etat_moteur(&mut self) -> Issue<()> {
        let Ok(profil) = self.profil_actif() else {
            return Ok(());
        };
        let etat = EtatMoteurSauve {
            courbe: profil.version_active().courbe.clone(),
            profil_id: Some(profil.id().to_string()),
            version: Some(profil.numero_actif()),
        };
        self.depot.sauver_etat_moteur(&etat)?;
        Ok(())
    }

    /// Le bouton « Continuer » du premier lancement : l'accord pour écrire dans la
    /// configuration audio. C'est seulement à partir d'ici que le moteur reçoit une courbe.
    pub fn autoriser_moteur(&mut self) {
        if self.reglages.moteur_autorise {
            return;
        }
        self.reglages.moteur_autorise = true;
        self.sauver_reglages();
        self.tenter(|c| {
            c.appliquer_courbe_du_jour()?;
            Ok((!c.options.moteur_absent).then(|| textes::ANNONCE_MOTEUR_AUTORISE.to_string()))
        });
    }

    /// « Rendre le son d'origine » : la configuration d'Equalizer APO redevient celle
    /// d'avant Retro EQ, et l'accord est retiré.
    pub fn rendre_son_origine(&mut self, t: u64) {
        if self.test_en_cours() {
            self.arreter_test(t);
        }
        self.tenter(|c| {
            if !c.options.moteur_absent {
                c.moteur.retirer()?;
            }
            c.reglages.moteur_autorise = false;
            c.sauver_reglages();
            // Une ancienne copie de la configuration, différente de ce qui vient d'être
            // rendu, est restée sur le disque : on dit où.
            let copie = c
                .moteur
                .copie_de_securite()
                .filter(|_| !c.options.moteur_absent);
            Ok(Some(textes::annonce_son_origine(
                copie.as_deref().map(|p| p.display().to_string()).as_deref(),
            )))
        });
    }

    /// Coupe ou remet l'égaliseur (Ctrl+Alt+0). Le moteur reste en place, à plat.
    pub fn basculer_egaliseur(&mut self) {
        self.tenter(|c| {
            c.exiger_pas_de_test()?;
            c.reglages.egaliseur_coupe = !c.reglages.egaliseur_coupe;
            c.sauver_reglages();
            c.appliquer_courbe_du_jour()?;
            Ok(Some(
                textes::annonce_egaliseur(c.reglages.egaliseur_coupe).to_string(),
            ))
        });
    }

    /// Les sorties audio que le moteur connaît, pour l'autotest.
    pub fn sorties_moteur(&self) -> Vec<SortieAudio> {
        if self.options.moteur_absent {
            return Vec::new();
        }
        self.moteur.sorties().unwrap_or_default()
    }

    pub fn etat_moteur(&self) -> EtatMoteurVue {
        if self.options.moteur_absent {
            return EtatMoteurVue::Absent;
        }
        match self.moteur.etat() {
            EtatMoteur::Absent => EtatMoteurVue::Absent,
            EtatMoteur::Inactif => EtatMoteurVue::Inactif,
            EtatMoteur::Actif => EtatMoteurVue::Actif,
        }
    }

    // ----- Égaliseur -----

    fn brouillon_ou_actif(&self) -> Brouillon {
        match &self.brouillon {
            Some(b) => b.clone(),
            None => {
                let courbe = self
                    .profil_actif()
                    .map(|p| p.version_active().courbe.clone())
                    .unwrap_or_default();
                Brouillon::depuis(&courbe)
            }
        }
    }

    /// Change le brouillon et le fait entendre tout de suite.
    fn modifier_brouillon(&mut self, modifier: impl FnOnce(&mut Brouillon)) {
        self.tenter(|c| {
            c.exiger_pas_de_test()?;
            c.profil_actif()?;
            let mut brouillon = c.brouillon_ou_actif();
            modifier(&mut brouillon);
            c.brouillon = Some(brouillon);
            c.appliquer_courbe_du_jour()?;
            Ok(None)
        });
    }

    /// Bouge le fader `bande` (0 à 9). Le gain est borné à ±12 dB et arrondi au dixième.
    pub fn regler_bande(&mut self, bande: usize, gain_db: f64) {
        if bande >= 10 || !gain_db.is_finite() {
            return;
        }
        let gain = (gain_db.clamp(-GAIN_MAX_DB, GAIN_MAX_DB) * 10.0).round() / 10.0;
        self.modifier_brouillon(|b| b.faders.gains[bande] = gain);
    }

    /// Règle le préampli à la main (sans effet tant que l'automatique est actif).
    pub fn regler_preampli(&mut self, preampli_db: f64) {
        if !preampli_db.is_finite() {
            return;
        }
        self.modifier_brouillon(|b| {
            if !b.preampli_auto {
                b.preampli_db = preampli_db.clamp(PREAMPLI_MIN_DB, 0.0);
            }
        });
    }

    /// Passe du préampli automatique au manuel et retour. Le manuel part de la valeur
    /// automatique : basculer ne change rien à ce qu'on entend.
    pub fn basculer_preampli_auto(&mut self) {
        self.modifier_brouillon(|b| {
            if b.preampli_auto {
                b.preampli_db = b.courbe().preampli_db;
            }
            b.preampli_auto = !b.preampli_auto;
        });
    }

    /// Remet tout à plat dans le brouillon (la version enregistrée ne bouge pas).
    pub fn remettre_a_plat(&mut self) {
        self.modifier_brouillon(|b| {
            b.faders.gains = [0.0; 10];
            b.faders.supplementaires.clear();
            b.preampli_db = 0.0;
        });
    }

    /// Jette le brouillon : on réentend la version active.
    pub fn annuler_egaliseur(&mut self) {
        self.brouillon = None;
        self.tenter(|c| {
            c.appliquer_courbe_du_jour()?;
            Ok(None)
        });
    }

    /// Enregistre le brouillon comme nouvelle version du profil actif, et l'active.
    /// L'ancienne version reste là : on peut y revenir depuis « Mes profils ».
    pub fn enregistrer_egaliseur(&mut self, t: u64) {
        self.tenter(|c| {
            c.exiger_pas_de_test()?;
            let Some(brouillon) = c.brouillon.clone() else {
                return Ok(None);
            };
            let date = c.date(t);
            let id = c.profil_actif()?.id().to_string();
            let profil = c.profil_mut(&id)?;
            let pas = profil.version_active().pas_par_zone;
            let numero = profil.ajouter_version(brouillon.courbe(), Origine::Manuel, pas, date);
            profil.activer(numero)?;
            let (profil, nom) = (profil.clone(), profil.nom().to_string());
            c.depot.sauver_profil(&profil)?;
            c.brouillon = None;
            c.sauver_etat_moteur()?;
            c.appliquer_courbe_du_jour()?;
            Ok(Some(textes::annonce_version_enregistree(numero, &nom)))
        });
    }

    // ----- Profils -----

    /// Active un profil : sa version active devient ce qu'on entend.
    pub fn activer_profil(&mut self, id: &str) {
        self.tenter(|c| {
            c.exiger_pas_de_test()?;
            let nom = c.profil(id)?.nom().to_string();
            c.actif = Some(id.to_string());
            c.brouillon = None;
            c.sauver_etat_moteur()?;
            c.appliquer_courbe_du_jour()?;
            Ok(Some(textes::annonce_profil_active(&nom)))
        });
    }

    /// Active le n-ième profil de la liste (Ctrl+Alt+1 à 4), à partir de 1.
    pub fn activer_profil_numero(&mut self, rang: usize) {
        if let Some(id) = rang
            .checked_sub(1)
            .and_then(|i| self.profils.get(i))
            .map(|p| p.id().to_string())
        {
            self.activer_profil(&id);
        }
    }

    /// Rend active une version d'un profil. Toujours réversible : aucune version ne disparaît.
    pub fn activer_version(&mut self, id: &str, numero: u32) {
        self.tenter(|c| {
            c.exiger_pas_de_test()?;
            let profil = c.profil_mut(id)?;
            profil.activer(numero)?;
            let (profil, nom) = (profil.clone(), profil.nom().to_string());
            c.depot.sauver_profil(&profil)?;
            if c.actif.as_deref() == Some(id) {
                c.brouillon = None;
                c.sauver_etat_moteur()?;
                c.appliquer_courbe_du_jour()?;
            }
            Ok(Some(textes::annonce_version_activee(numero, &nom)))
        });
    }

    pub fn renommer_profil(&mut self, id: &str, nom: &str) {
        let nom = nom.trim().to_string();
        if nom.is_empty() {
            return;
        }
        self.tenter(|c| {
            let profil = c.profil_mut(id)?;
            profil.renommer(nom);
            let profil = profil.clone();
            c.depot.sauver_profil(&profil)?;
            Ok(None)
        });
    }

    /// Copie un profil : la copie part de sa version active. Rend l'identifiant de la copie.
    pub fn dupliquer_profil(&mut self, id: &str, t: u64) -> Option<String> {
        let mut nouveau = None;
        self.tenter(|c| {
            let source = c.profil(id)?;
            let version = source.version_active();
            let nom = format!("{} (copie)", source.nom());
            let copie = Profil::nouveau(
                c.nouvel_id(t),
                nom.clone(),
                version.courbe.clone(),
                version.origine.clone(),
                c.date(t),
            );
            nouveau = Some(copie.id().to_string());
            c.ajouter_profil(copie)?;
            Ok(Some(textes::annonce_profil_cree(&nom)))
        });
        nouveau
    }

    /// Crée un profil à partir d'un preset livré avec l'appli. Rend son identifiant.
    pub fn creer_depuis_preset(&mut self, nom_preset: &str, t: u64) -> Option<String> {
        let mut nouveau = None;
        self.tenter(|c| {
            let preset = retro_core::presets()
                .into_iter()
                .find(|p| p.nom == nom_preset)
                .ok_or(ErreurControleur::ProfilInconnu)?;
            let profil = Profil::nouveau(
                c.nouvel_id(t),
                preset.nom,
                preset.courbe.avec_preampli_auto(),
                Origine::Preset,
                c.date(t),
            );
            nouveau = Some(profil.id().to_string());
            c.ajouter_profil(profil)?;
            Ok(Some(textes::annonce_profil_cree(preset.nom)))
        });
        nouveau
    }

    // ----- Import et export -----

    /// Importe un fichier Peace, Equalizer APO ou AutoEQ comme nouveau profil (pas activé).
    /// Rend l'identifiant du profil créé.
    pub fn importer(&mut self, chemin: &Path, t: u64) -> Option<String> {
        let mut nouveau = None;
        self.tenter(|c| {
            let octets = std::fs::read(chemin).map_err(|source| ErreurControleur::Fichier {
                action: format!("Lecture de {}", chemin.display()),
                source,
            })?;
            // Peace écrit parfois en Windows-1252 : on lit ce qui est lisible plutôt que de refuser.
            let texte = String::from_utf8_lossy(&octets);
            let nom_fichier = chemin
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let format = formats::detecter_format(&texte, &nom_fichier)
                .ok_or(ErreurControleur::FormatInconnu)?;
            let (import, nom_format) = match format {
                Format::EqualizerApo => (formats::equalizer_apo::lire(&texte)?, "equalizer_apo"),
                Format::AutoEq => (formats::autoeq::lire(&texte, &nom_fichier)?, "autoeq"),
                Format::Peace => (formats::peace::lire(&texte)?, "peace"),
                Format::Wavelet => return Err(ErreurControleur::ImportWavelet),
            };
            let nom = import.nom.clone().unwrap_or_else(|| {
                chemin
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Profil importé".to_string())
            });
            // Préampli automatique : un fichier venu d'ailleurs ne doit pas pouvoir saturer.
            let profil = Profil::nouveau(
                c.nouvel_id(t),
                nom.clone(),
                import.courbe.avec_preampli_auto(),
                Origine::Import {
                    format: nom_format.to_string(),
                },
                c.date(t),
            );
            nouveau = Some(profil.id().to_string());
            c.ajouter_profil(profil)?;
            // Les commentaires et les lignes éteintes ne sont pas une perte : le résumé ne
            // compte que ce qui change le son, et à part ce qui a été repris de façon approchée.
            let mut laissees: Vec<(&'static str, usize)> = Vec::new();
            for ligne in import.ignores.iter().filter(|l| l.raison.change_le_son()) {
                let raison = textes::raison_ignoree(&ligne.raison);
                match laissees.iter_mut().find(|(r, _)| *r == raison) {
                    Some((_, n)) => *n += 1,
                    None => laissees.push((raison, 1)),
                }
            }
            let approchees = import
                .ignores
                .iter()
                .filter(|l| l.raison == formats::RaisonIgnoree::Approximation)
                .count();
            Ok(Some(textes::annonce_import(
                &nom, nom_format, &laissees, approchees,
            )))
        });
        nouveau
    }

    fn ecrire_export(&mut self, nom: &str, courbe: &Courbe, format: &str) -> Issue<Option<String>> {
        let (contenu, suffixe) = match format {
            "peace" => (formats::peace::ecrire(courbe), ".peace"),
            "apo" => (formats::equalizer_apo::ecrire(courbe), ".txt"),
            "autoeq" => (formats::autoeq::ecrire(courbe), " ParametricEQ.txt"),
            "wavelet" => (
                formats::wavelet::exporter(|f| courbe.reponse_db(f), courbe.preampli_db),
                " GraphicEQ.txt",
            ),
            _ => return Err(ErreurControleur::FormatInconnu),
        };
        // Le nom du profil devient un nom de fichier : on retire ce que Windows refuse.
        let propre: String = nom
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || " -_".contains(c) {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let dossier = self.options.dossier_export.clone();
        let chemin = dossier.join(format!("{}{suffixe}", propre.trim()));
        std::fs::create_dir_all(&dossier)
            .and_then(|_| std::fs::write(&chemin, contenu))
            .map_err(|source| ErreurControleur::Fichier {
                action: format!("Écriture de {}", chemin.display()),
                source,
            })?;
        Ok(Some(textes::annonce_export(&chemin.display().to_string())))
    }

    /// Exporte la version active d'un profil. `format` : « peace », « apo », « autoeq », « wavelet ».
    pub fn exporter_profil(&mut self, id: &str, format: &str) {
        self.tenter(|c| {
            let profil = c.profil(id)?;
            let nom = format!("{} v{}", profil.nom(), profil.numero_actif());
            let courbe = profil.version_active().courbe.clone();
            c.ecrire_export(&nom, &courbe, format)
        });
    }

    /// Exporte le profil actif (page Réglages).
    pub fn exporter_profil_actif(&mut self, format: &str) {
        match self.actif.clone() {
            Some(id) => self.exporter_profil(&id, format),
            None => self.signaler(ErreurControleur::AucunProfil),
        }
    }

    /// Exporte la courbe proposée par le dernier test.
    pub fn exporter_resultat(&mut self, format: &str) {
        self.tenter(|c| {
            let dernier = c.resultat.as_ref().ok_or(ErreurControleur::AucunResultat)?;
            let nom = format!("{} (résultat)", c.profil(&dernier.profil_id)?.nom());
            let courbe = dernier.resultat.nouvelle_courbe.clone();
            c.ecrire_export(&nom, &courbe, format)
        });
    }

    // ----- Réglages -----

    pub fn choisir_langue(&mut self, code: &str) {
        self.reglages.langue = if code == "en" { Langue::En } else { Langue::Fr };
        self.sauver_reglages();
    }

    pub fn regler_rail(&mut self, rail: bool) {
        self.reglages.navigation = if rail {
            Navigation::Rail
        } else {
            Navigation::Onglets
        };
        self.sauver_reglages();
    }

    pub fn regler_fermer_reduit(&mut self, valeur: bool) {
        self.reglages.fermer_reduit = valeur;
        self.sauver_reglages();
    }

    pub fn regler_mini_fenetre(&mut self, valeur: bool) {
        self.reglages.mini_fenetre = valeur;
        self.sauver_reglages();
    }

    pub fn regler_bip(&mut self, valeur: bool) {
        self.reglages.bip = valeur;
        self.sauver_reglages();
    }

    pub fn regler_pause_appels(&mut self, valeur: bool) {
        self.reglages.pause_appels = valeur;
        self.sauver_reglages();
    }

    /// L'intervalle par défaut des prochains tests, en secondes (borné à 10 s – 2 min).
    pub fn regler_intervalle(&mut self, secondes: u32) {
        let ms = retro_core::reglages::borner_intervalle(secondes.saturating_mul(1000));
        self.reglages.intervalle_ms = ms;
        self.choix.intervalle_s = ms / 1000;
        self.sauver_reglages();
    }

    pub fn fermer_reduit(&self) -> bool {
        self.reglages.fermer_reduit
    }

    // ----- Test -----

    /// Un test est commencé et pas fini.
    pub fn test_en_cours(&self) -> bool {
        self.test.as_ref().is_some_and(|t| {
            !matches!(
                t.deroulement.vue(0).etat,
                EtatTest::Termine | EtatTest::Abandonne
            )
        })
    }

    fn etat_test(&self) -> Option<EtatTest> {
        self.test.as_ref().map(|t| t.deroulement.vue(0).etat)
    }

    /// Les touches F6 à F10 doivent-elles être prises ? Seulement quand une réponse peut
    /// arriver : en écoute, en attente, ou pendant un appel (le test repart tout seul).
    /// En pause voulue ou une fois fini, elles sont rendues aux jeux (ADR-0008).
    pub fn touches_de_test_actives(&self) -> bool {
        matches!(
            self.etat_test(),
            Some(
                EtatTest::EnEcoute { .. }
                    | EtatTest::AttenteReponse
                    | EtatTest::EnPause {
                        raison: RaisonPause::Appel
                    }
            )
        )
    }

    /// Maintenir F6 ou F8 fait-il réécouter ? Seulement une fois la question posée.
    pub fn reecoute_possible(&self) -> bool {
        self.etat_test() == Some(EtatTest::AttenteReponse)
    }

    /// La mini-fenêtre doit-elle être visible ?
    pub fn mini_fenetre_voulue(&self) -> bool {
        self.reglages.mini_fenetre && self.touches_de_test_actives() && !self.plein_ecran
    }

    /// Mémorise les choix de l'écran « Nouveau test » (ils servent aussi à Ctrl+Alt+Entrée).
    pub fn choisir_test(&mut self, choix: ChoixTest) {
        self.choix = choix;
    }

    fn mode_choisi(&self) -> Issue<(Mode, Option<(String, String)>)> {
        let duels = if self.choix.precis { 12 } else { 6 };
        match self.choix.mode.as_str() {
            "duels-en-jeu" => Ok((
                Mode::DuelsEnJeu {
                    precis: self.choix.precis,
                },
                None,
            )),
            "une-zone" => {
                let zone = Zone::TOUTES
                    .get(self.choix.zone)
                    .copied()
                    .unwrap_or(Zone::Graves);
                Ok((Mode::UneSeuleZone { zone, duels }, None))
            }
            "profil-contre-profil" => {
                let autre = self
                    .profils
                    .iter()
                    .find(|p| {
                        p.nom() == self.choix.autre_profil && Some(p.id()) != self.actif.as_deref()
                    })
                    .ok_or(ErreurControleur::AutreProfilManquant)?;
                Ok((
                    Mode::ProfilContreProfil {
                        autre: autre.version_active().courbe.clone(),
                        duels,
                    },
                    Some((autre.id().to_string(), autre.nom().to_string())),
                ))
            }
            _ => Err(ErreurControleur::ModeIndisponible),
        }
    }

    /// Ce que les tests finis de ce profil apprennent au suivant, du plus ancien au plus
    /// récent : les couples (zone, sens) déjà joués, et les indices du dernier test, qui
    /// passeront en tête du plan (ADR-0005). Tout est relu dans les journaux.
    fn historique_plan(&self, profil_id: &str) -> HistoriquePlan {
        let mut historique = HistoriquePlan::depuis_zones(&[]);
        let Ok(dossier) = std::fs::read_dir(self.depot.dossier().join("tests")) else {
            return historique;
        };
        let mut ids: Vec<String> = dossier
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let chemin = e.path();
                (chemin.extension().and_then(|x| x.to_str()) == Some("jsonl"))
                    .then(|| chemin.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .flatten()
            })
            .collect();
        ids.sort();
        for id in ids {
            let Ok(lecture) = self.depot.lire_journal(&id) else {
                continue;
            };
            let Some(Entree::TestDemarre {
                profil_id: p, plan, ..
            }) = lecture.entrees.first()
            else {
                continue;
            };
            let resultat = lecture.entrees.iter().find_map(|e| match e {
                Entree::TestTermine { resultat, .. } => Some(resultat),
                _ => None,
            });
            if let (true, Some(resultat)) = (p == profil_id, resultat) {
                historique.ajouter_test(plan, &resultat.indices);
            }
        }
        historique
    }

    /// Lance un test avec les choix mémorisés. Rend vrai si un test tourne après l'appel
    /// (le pont affiche alors l'écran Duels).
    pub fn demarrer_test(&mut self, t: u64) -> bool {
        if self.test_en_cours() {
            // « Commencer » sur un test prêt ou repris : on le lance, on n'en crée pas un second.
            match self.etat_test() {
                Some(EtatTest::Pret) => self.envoyer(Evenement::Demarrer, t),
                Some(EtatTest::EnPause { .. }) => self.envoyer(Evenement::Reprendre, t),
                _ => {}
            }
            return true;
        }
        self.tenter(|c| {
            if c.options.moteur_absent {
                return Err(ErreurControleur::MoteurAbsent);
            }
            if !c.reglages.moteur_autorise {
                return Err(ErreurControleur::MoteurNonAutorise);
            }
            if c.brouillon.is_some() {
                return Err(ErreurControleur::BrouillonEnCours);
            }
            let (mode, autre) = c.mode_choisi()?;
            let profil = c.profil_actif()?;
            let (profil_id, reference) = (profil.id().to_string(), profil.version_active().clone());
            let historique = c.historique_plan(&profil_id);
            let mut hasard = c.hasard();
            let duels = plan_avec_historique(&mode, &reference, &historique, &mut hasard);
            let verification = duel_verification(&mut hasard);
            let test_id = format!("t{t}");
            let journal = c.depot.ouvrir_journal(&test_id)?;
            let deroulement = Deroulement::nouveau(ParametresTest {
                test_id: test_id.clone(),
                profil_id: profil_id.clone(),
                reference: reference.clone(),
                mode: mode.clone(),
                plan: duels,
                reglages: ReglagesTest::nouveau(
                    c.choix.intervalle_s.saturating_mul(1000),
                    c.reglages.bip,
                ),
                variantes: ReglagesVariantes::default(),
                spectre: c.spectre.clone(),
                verification,
            });
            // Un test se fait égaliseur en marche.
            if c.reglages.egaliseur_coupe {
                c.reglages.egaliseur_coupe = false;
                c.sauver_reglages();
            }
            c.resultat = None;
            c.test = Some(TestEnCours {
                deroulement,
                journal,
                test_id,
                profil_id,
                reference,
                mode,
                autre,
            });
            Ok(None)
        });
        if self.etat_test() == Some(EtatTest::Pret) {
            self.envoyer(Evenement::Demarrer, t);
        }
        self.test_en_cours()
    }

    /// Ctrl+Alt+Entrée : reprend le test en pause, ou en démarre un.
    pub fn demarrer_ou_reprendre(&mut self, t: u64) -> bool {
        self.demarrer_test(t)
    }

    /// Transmet un événement à la machine à états du test et exécute ce qu'elle demande.
    fn envoyer(&mut self, evenement: Evenement, t: u64) {
        let Some(test) = self.test.as_mut() else {
            return;
        };
        let effets = test.deroulement.traiter(evenement, t);
        self.executer(effets, t);
    }

    /// Exécute les effets dans l'ordre donné par la machine : c'est elle qui garantit que
    /// la réponse est journalisée avant toute autre conséquence.
    fn executer(&mut self, effets: Vec<Effet>, t: u64) {
        let mut echec_moteur = false;
        for effet in effets {
            echec_moteur |= self.executer_un(effet, t);
        }
        if echec_moteur {
            self.secourir(t);
        }
    }

    /// Exécute un effet. Rend vrai si le moteur a refusé une courbe.
    fn executer_un(&mut self, effet: Effet, t: u64) -> bool {
        match effet {
            Effet::Journaliser(entree) => {
                let ecriture = match self.test.as_mut() {
                    Some(test) => test.journal.ajouter(&entree),
                    None => Ok(()),
                };
                if let Err(e) = ecriture {
                    self.signaler(ErreurControleur::Journal(e));
                }
            }
            Effet::Appliquer { courbe, .. } => {
                if let Err(e) = self.appliquer_au_moteur(&courbe) {
                    self.signaler(e);
                    return true;
                }
            }
            Effet::DemanderReponse => {
                // En plein écran exclusif la mini-fenêtre ne peut pas s'afficher : un bip
                // dit que c'est le moment de répondre.
                if self.plein_ecran {
                    self.bips += 1;
                }
            }
            Effet::MasquerQuestion | Effet::ConseillerPause => {}
            Effet::Bip => self.bips += 1,
            Effet::Termine(resultat) => self.conclure(resultat, t),
        }
        false
    }

    /// Le moteur a refusé une courbe en plein test : on met le test en pause et on retente
    /// de remettre la référence. Une variante ne doit pas rester active.
    fn secourir(&mut self, t: u64) {
        let Some(test) = self.test.as_mut() else {
            return;
        };
        let reference = test.reference.courbe.clone();
        let effets = test.deroulement.traiter(Evenement::Pause, t);
        let mut reference_remise = false;
        for effet in effets {
            match effet {
                Effet::Appliquer { courbe, .. } => {
                    reference_remise |= self.appliquer_au_moteur(&courbe).is_ok();
                }
                autre => {
                    self.executer_un(autre, t);
                }
            }
        }
        if !reference_remise {
            let _ = self.appliquer_au_moteur(&reference);
        }
        // On garde le message précis du moteur s'il y en a un, complété par ce qu'on a fait.
        let cause = self.annonce.take().map(|a| a.texte).unwrap_or_default();
        self.annonce = Some(Annonce {
            texte: format!("{} {cause}", textes::ANNONCE_PAUSE_ERREUR)
                .trim()
                .to_string(),
            erreur: true,
        });
    }

    /// Fin d'un test : la nouvelle version est ajoutée au profil. Elle n'est activée que si
    /// elle sonne exactement comme la référence (aucun changement) ; sinon c'est à
    /// l'utilisateur de le décider sur l'écran Résultat.
    fn conclure(&mut self, resultat: ResultatTest, t: u64) {
        let Some(test) = self.test.as_ref() else {
            return;
        };
        let (profil_id, test_id) = (test.profil_id.clone(), test.test_id.clone());
        let reference = test.reference.clone();
        let autre = test.autre.clone();
        let sur_zones = !matches!(test.mode, Mode::ProfilContreProfil { .. });
        let date = self.date(t);
        let mut version_nouvelle = None;
        if sur_zones {
            let change = resultat.a_des_changements();
            self.tenter(|c| {
                let profil = c.profil_mut(&profil_id)?;
                // Sans changement, on garde la courbe de référence à l'identique (préampli
                // compris) : seule la taille des prochaines variantes évolue.
                let courbe = if change {
                    resultat.nouvelle_courbe.clone()
                } else {
                    reference.courbe.clone()
                };
                let numero = profil.ajouter_version(
                    courbe,
                    Origine::Test { test_id },
                    resultat.nouveaux_pas,
                    date,
                );
                if !change && profil.numero_actif() == reference.numero {
                    profil.activer(numero)?;
                }
                let profil = profil.clone();
                version_nouvelle = Some(numero);
                c.depot.sauver_profil(&profil)?;
                c.sauver_etat_moteur()?;
                Ok(None)
            });
        }
        self.resultat = Some(DernierResultat {
            resultat,
            profil_id,
            courbe_reference: reference.courbe,
            version_reference: reference.numero,
            version_nouvelle,
            autre,
        });
    }

    /// Le temps passe : à appeler plusieurs fois par seconde pendant un test.
    pub fn tic(&mut self, t: u64) {
        if !self.test_en_cours() {
            return;
        }
        let avant = self.etat_test();
        self.envoyer(Evenement::Tic, t);
        // Personne n'a répondu à temps : le test s'est mis en pause tout seul et la
        // référence est remise (ADR-0012). On le dit, sinon la pause surprend.
        let sans_reponse = Some(EtatTest::EnPause {
            raison: RaisonPause::SansReponse,
        });
        if avant != sans_reponse && self.etat_test() == sans_reponse {
            self.annoncer(textes::ANNONCE_SANS_REPONSE);
        }
    }

    pub fn repondre(&mut self, reponse: Reponse, t: u64) {
        self.envoyer(Evenement::Reponse(reponse), t);
    }

    pub fn mettre_en_pause(&mut self, t: u64) {
        self.envoyer(Evenement::Pause, t);
    }

    pub fn reprendre(&mut self, t: u64) {
        self.envoyer(Evenement::Reprendre, t);
    }

    pub fn passer_duel(&mut self, t: u64) {
        self.envoyer(Evenement::PasserDuel, t);
    }

    pub fn recommencer_duel(&mut self, t: u64) {
        self.envoyer(Evenement::RecommencerDuel, t);
    }

    pub fn plus_tard(&mut self, t: u64) {
        self.envoyer(Evenement::PlusTard, t);
    }

    pub fn annuler_reponse(&mut self, t: u64) {
        self.envoyer(Evenement::Annuler, t);
    }

    /// Touche de réécoute enfoncée ou relâchée (F6 ou F8 maintenue).
    pub fn reecouter(&mut self, cote: Cote, appuye: bool, t: u64) {
        self.envoyer(Evenement::Reecouter { cote, appuye }, t);
    }

    /// Arrête le test : la référence est remise, aucun résultat n'est calculé. Le journal
    /// garde les réponses déjà données.
    pub fn arreter_test(&mut self, t: u64) {
        if !self.test_en_cours() {
            return;
        }
        self.envoyer(Evenement::Abandonner, t);
        self.test = None;
        self.annoncer(textes::ANNONCE_TEST_ARRETE);
    }

    /// Un appel vocal commence ou se termine. Sans effet si le réglage est coupé.
    pub fn signaler_appel(&mut self, en_cours: bool, t: u64) {
        if self.reglages.pause_appels || !en_cours {
            self.envoyer(Evenement::AppelEnCours(en_cours), t);
        }
    }

    /// L'appli au premier plan a changé (elle est notée à chaque passage).
    pub fn signaler_premier_plan(&mut self, appli: Option<String>, t: u64) {
        self.envoyer(Evenement::PremierPlan(appli.unwrap_or_default()), t);
    }

    /// Un jeu occupe-t-il l'écran en plein écran exclusif ? Dans ce cas la mini-fenêtre ne
    /// peut pas s'afficher et c'est un bip qui prévient.
    pub fn signaler_plein_ecran(&mut self, exclusif: bool) {
        self.plein_ecran = exclusif;
    }

    /// Le spectre de ce qui joue, pour mettre A et B au même volume. Sert au prochain duel.
    pub fn maj_spectre(&mut self, spectre: Spectre) {
        if let Some(test) = self.test.as_mut() {
            test.deroulement.maj_spectre(spectre.clone());
        }
        self.spectre = spectre;
    }

    // ----- Résultat -----

    /// Active ce que le test propose : la nouvelle version, ou l'autre profil s'il a gagné.
    pub fn activer_resultat(&mut self) {
        let Some(dernier) = self.resultat.as_ref() else {
            return self.signaler(ErreurControleur::AucunResultat);
        };
        let profil_id = dernier.profil_id.clone();
        match (dernier.version_nouvelle, dernier.autre.clone()) {
            (Some(numero), _) => {
                self.activer_version(&profil_id, numero);
                if self.actif.as_deref() != Some(&profil_id) {
                    self.activer_profil(&profil_id);
                }
            }
            (None, Some((autre_id, _))) => self.activer_profil(&autre_id),
            (None, None) => {}
        }
    }

    /// « Revenir à la référence » : la version d'avant le test redevient active.
    pub fn revenir_reference(&mut self) {
        let Some(dernier) = self.resultat.as_ref() else {
            return self.signaler(ErreurControleur::AucunResultat);
        };
        let (profil_id, numero) = (dernier.profil_id.clone(), dernier.version_reference);
        if dernier.version_nouvelle.is_some() {
            self.activer_version(&profil_id, numero);
        }
        if self.actif.as_deref() != Some(&profil_id) {
            self.activer_profil(&profil_id);
        }
    }

    // ----- Ce que le pont vient chercher -----

    /// Le message à afficher, s'il y en a un nouveau. Le lire le retire.
    pub fn prendre_annonce(&mut self) -> Option<Annonce> {
        self.annonce.take()
    }

    /// Le nombre de bips à jouer depuis la dernière fois.
    pub fn prendre_bips(&mut self) -> u32 {
        std::mem::take(&mut self.bips)
    }

    pub fn dossier_donnees(&self) -> &Path {
        self.depot.dossier()
    }

    pub fn moteur_autorise(&self) -> bool {
        self.reglages.moteur_autorise
    }

    // ----- Vues -----

    pub fn vue_appli(&self) -> VueAppli {
        VueAppli {
            profil_actif: self
                .profil_actif()
                .map(|p| p.nom().to_string())
                .unwrap_or_default(),
            egaliseur_actif: !self.reglages.egaliseur_coupe,
            etat_moteur: self.etat_moteur(),
            moteur_autorise: self.reglages.moteur_autorise,
            test_en_cours: self.test_en_cours(),
        }
    }

    pub fn vue_egaliseur(&self) -> VueEgaliseur {
        let brouillon = self.brouillon_ou_actif();
        let courbe = brouillon.courbe();
        VueEgaliseur {
            gains: brouillon.faders.gains,
            preampli_db: courbe.preampli_db,
            preampli_auto: brouillon.preampli_auto,
            filtres_supplementaires: brouillon.faders.supplementaires.len(),
            courbe,
            modifie: self.brouillon.is_some(),
            profil: self
                .profil_actif()
                .map(|p| p.nom().to_string())
                .unwrap_or_default(),
        }
    }

    pub fn vue_profils(&self) -> Vec<VueProfil> {
        self.profils
            .iter()
            .map(|p| {
                let version = p.version_active();
                VueProfil {
                    id: p.id().to_string(),
                    nom: p.nom().to_string(),
                    actif: self.actif.as_deref() == Some(p.id()),
                    versions: p.versions().len(),
                    date: dates::jour_mois(&version.creee_le),
                    resume: textes::resume_profil(&version.origine),
                    courbe: version.courbe.clone(),
                }
            })
            .collect()
    }

    /// Les versions d'un profil, de la plus récente à la plus ancienne.
    pub fn vue_versions(&self, id: &str) -> Vec<VueVersion> {
        let Ok(profil) = self.profil(id) else {
            return Vec::new();
        };
        profil
            .versions()
            .iter()
            .rev()
            .map(|v| VueVersion {
                numero: v.numero,
                date: dates::jour_heure(&v.creee_le),
                origine: textes::origine(&v.origine),
                active: v.numero == profil.numero_actif(),
                courbe: v.courbe.clone(),
            })
            .collect()
    }

    pub fn presets(&self) -> Vec<String> {
        retro_core::presets()
            .into_iter()
            .map(|p| p.nom.to_string())
            .collect()
    }

    pub fn vue_nouveau_test(&self) -> VueNouveauTest {
        let zone = Zone::TOUTES.get(self.choix.zone).copied();
        let duels = match self.choix.mode.as_str() {
            "duels-en-jeu" if self.choix.precis => 30,
            "duels-en-jeu" => 12,
            _ if self.choix.precis => 12,
            _ => 6,
        };
        let intervalle_s =
            retro_core::reglages::borner_intervalle(self.choix.intervalle_s.saturating_mul(1000))
                / 1000;
        VueNouveauTest {
            reference: self
                .profil_actif()
                .map(|p| format!("{} · version {}", p.nom(), p.numero_actif()))
                .unwrap_or_default(),
            source: textes::SOURCE_PC.to_string(),
            mode: textes::nom_mode(&self.choix.mode, zone, &self.choix.autre_profil),
            duree: textes::duree_test(duels, intervalle_s),
            zones: textes::noms_zones(),
            profils: self
                .profils
                .iter()
                .filter(|p| Some(p.id()) != self.actif.as_deref())
                .map(|p| p.nom().to_string())
                .collect(),
        }
    }

    /// L'écran Duels à l'instant `t`. Ne contient que ce que le test à l'aveugle permet de
    /// montrer : ni le sujet du duel en cours, ni qui y joue la référence.
    pub fn vue_duels(&self, t: u64) -> VueDuels {
        let Some(test) = self.test.as_ref() else {
            return VueDuels::default();
        };
        let vue = test.deroulement.vue(t);
        let fini = vue.etat == EtatTest::Termine;
        let etat = match vue.etat {
            EtatTest::Pret | EtatTest::Abandonne => EtatTestVue::Pret,
            EtatTest::EnEcoute { .. } => EtatTestVue::Ecoute,
            EtatTest::AttenteReponse => EtatTestVue::AttenteReponse,
            EtatTest::EnPause {
                raison: RaisonPause::Appel,
            } => EtatTestVue::PauseAppel,
            EtatTest::EnPause { .. } => EtatTestVue::Pause,
            EtatTest::Termine => EtatTestVue::Termine,
        };
        let suite = match (vue.etat, vue.cote_actif) {
            (EtatTest::EnEcoute { .. }, Some(cote)) => {
                let faits = match cote {
                    Cote::A => vue.passages_a,
                    Cote::B => vue.passages_b,
                };
                textes::suite(cote, faits + 1)
            }
            _ => String::new(),
        };
        VueDuels {
            etat,
            duel: vue.duel,
            total: vue.total,
            cote_actif: match vue.cote_actif {
                None => 0,
                Some(Cote::A) => 1,
                Some(Cote::B) => 2,
            },
            passages_a: vue.passages_a,
            passages_b: vue.passages_b,
            avancement: vue.progression,
            compte_a_rebours: vue
                .ms_avant_bascule
                .map(textes::compte_a_rebours)
                .unwrap_or_default(),
            suite,
            journal: vue
                .historique
                .iter()
                .rev()
                .map(|duel| {
                    let (titre, detail) = textes::ligne_journal(duel, fini);
                    VueLigneJournal {
                        titre,
                        cote: textes::cote_reponse(duel.reponse),
                        detail,
                    }
                })
                .collect(),
            pause_conseillee: vue.pause_conseillee && !fini,
            pause_sans_reponse: vue.etat
                == EtatTest::EnPause {
                    raison: RaisonPause::SansReponse,
                },
        }
    }

    pub fn vue_resultat(&self) -> Option<VueResultat> {
        let dernier = self.resultat.as_ref()?;
        let profil = self.profil(&dernier.profil_id).ok()?;
        let r = &dernier.resultat;
        let est_actif = self.actif.as_deref() == Some(profil.id());
        let (titre, resume, courbe_nouvelle, active) = match (&r.autre_profil, &dernier.autre) {
            (Some(bilan), autre) => {
                let (autre_id, autre_nom) = autre
                    .clone()
                    .unwrap_or_else(|| (String::new(), "l’autre profil".to_string()));
                let courbe = self
                    .profil(&autre_id)
                    .map(|p| p.version_active().courbe.clone())
                    .unwrap_or_else(|_| dernier.courbe_reference.clone());
                (
                    format!("{} contre {autre_nom}", profil.nom()),
                    textes::resume_autre_profil(bilan, &autre_nom),
                    courbe,
                    self.actif.as_deref() == Some(autre_id.as_str()),
                )
            }
            (None, _) => {
                let changees = r.zones.iter().filter(|z| z.est_changee()).count() as u32;
                let numero = dernier
                    .version_nouvelle
                    .unwrap_or(dernier.version_reference);
                (
                    format!("{} · version {numero}", profil.nom()),
                    textes::resume_zones(
                        changees,
                        r.zones.len() as u32 - changees,
                        r.controles.rien_retenu,
                    ),
                    r.nouvelle_courbe.clone(),
                    est_actif && Some(profil.numero_actif()) == dernier.version_nouvelle,
                )
            }
        };
        Some(VueResultat {
            titre,
            confiance: textes::niveau_confiance(r.confiance),
            resume,
            controles: textes::bilan_controles(&r.controles),
            verification: textes::verification(r.verification_preference),
            zones: r
                .zones
                .iter()
                .map(|z| VueZone {
                    nom: z.zone.nom().to_string(),
                    plage: textes::plage_zone(z.zone),
                    gain_db: z.gain_db,
                    change: z.est_changee(),
                    confiance: textes::niveau_confiance(z.confiance),
                    detail: textes::detail_zone(z, r.controles.rien_retenu),
                })
                .collect(),
            applis: r
                .par_appli
                .iter()
                .map(|a| VueAppliResultat {
                    nom: a.appli.clone(),
                    duels: a.duels,
                    preference: textes::preference_appli(a.preference).to_string(),
                })
                .collect(),
            preampli_db: courbe_nouvelle.preampli_db,
            courbe_reference: dernier.courbe_reference.clone(),
            courbe_nouvelle,
            active,
            recommandee: r.recommandee
                || r.autre_profil
                    .as_ref()
                    .is_some_and(|b| b.preference == retro_core::Preference::Variante),
        })
    }

    pub fn vue_reglages(&self) -> VueReglages {
        VueReglages {
            langue: match self.reglages.langue {
                Langue::Fr => "fr",
                Langue::En => "en",
            },
            rail: self.reglages.navigation == Navigation::Rail,
            fermer_reduit: self.reglages.fermer_reduit,
            mini_fenetre: self.reglages.mini_fenetre,
            bip: self.reglages.bip,
            pause_appels: self.reglages.pause_appels,
            intervalle_s: self.reglages.intervalle_ms / 1000,
        }
    }
}

/// Vrai hasard ou hasard à graine, derrière un seul type : `plan` veut un type concret
/// (`impl Hasard`), pas un `Box<dyn Hasard>`.
enum HasardSource {
    Reel(HasardReel),
    Graine(HasardGraine),
}

impl retro_core::Hasard for HasardSource {
    fn entier(&mut self, borne: u32) -> u32 {
        match self {
            HasardSource::Reel(h) => h.entier(borne),
            HasardSource::Graine(h) => h.entier(borne),
        }
    }
}
