//! La base AutoEQ côté contrôleur (ADR-0013) : l'index gardé en local, la recherche, et la
//! création d'un profil depuis un résultat.
//!
//! Le réseau tourne sur un fil de travail : le contrôleur lance le téléchargement, note qu'une
//! tâche est en cours, et le pont vient relever la réponse à chaque image
//! ([`Controleur::autoeq_relever`]), comme il vide déjà le canal du son. Rien n'attend sur le
//! fil de l'interface. L'index est lu sur le disque (quelques ms) ; la recherche elle-même se
//! fait en mémoire et ne demande aucune attente.

use super::*;
use crate::reseau::{Telechargeur, TelechargeurUreq};
use retro_core::autoeq_base::{self, BaseAutoEq, EntreeBase, Resultat, URL_INDEX};
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// L'index fait environ 0,85 Mo : 8 Mo de marge avant de refuser une réponse.
const LIMITE_INDEX: u64 = 8 * 1024 * 1024;
/// Un réglage fait environ 0,5 Ko : 256 Ko de marge.
const LIMITE_REGLAGE: u64 = 256 * 1024;

/// Où en est la base, pour l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatBaseAutoEq {
    /// Jamais téléchargée (ou illisible) : la recherche est impossible.
    Absente,
    /// Gardée en local : la recherche marche, même sans réseau.
    Prete,
    /// L'index est en cours de téléchargement.
    MiseAJour,
}

/// Un casque trouvé, tel que l'écran l'affiche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueResultatAutoEq {
    pub nom: String,
    pub source: String,
    pub plateforme: Option<String>,
    pub type_casque: Option<String>,
    /// Ses réglages sont déjà sur ce PC : il s'importe sans réseau.
    pub hors_ligne: bool,
}

/// Le panneau « Chercher mon casque ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueAutoEq {
    pub etat: EtatBaseAutoEq,
    /// Un casque est en cours de téléchargement.
    pub import_en_cours: bool,
    /// « 8 850 casques · mise à jour le 25/11 » ; vide si la base est absente.
    pub resume: String,
    pub requete: String,
    pub resultats: Vec<VueResultatAutoEq>,
    /// Le dernier échec, dans la langue de l'utilisateur ; vide s'il n'y en a pas.
    pub erreur: String,
}

/// Ce que le fil de travail renvoie.
enum Reponse {
    Index(Result<Vec<u8>, ErreurReseau>),
    Casque(EntreeBase, Result<Vec<u8>, ErreurReseau>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tache {
    Index,
    Casque,
}

/// L'état de la base dans le contrôleur.
pub(super) struct Etat {
    base: Option<BaseAutoEq>,
    /// Le disque a déjà été regardé (inutile de relire l'index à chaque ouverture du panneau).
    disque_lu: bool,
    requete: String,
    resultats: Vec<Resultat>,
    /// Les fichiers de réglages déjà gardés sur ce PC.
    telecharges: HashSet<String>,
    tache: Option<Tache>,
    erreur: String,
    telechargeur: Arc<dyn Telechargeur>,
    envoi: Sender<Reponse>,
    reception: Receiver<Reponse>,
}

impl Default for Etat {
    fn default() -> Etat {
        let (envoi, reception) = channel();
        Etat {
            base: None,
            disque_lu: false,
            requete: String::new(),
            resultats: Vec::new(),
            telecharges: HashSet::new(),
            tache: None,
            erreur: String::new(),
            telechargeur: Arc::new(TelechargeurUreq::nouveau()),
            envoi,
            reception,
        }
    }
}

impl Controleur {
    /// Remplace le téléchargeur (les tests donnent un faux : jamais de vrai réseau).
    #[cfg(test)]
    pub fn regler_telechargeur(&mut self, telechargeur: Arc<dyn Telechargeur>) {
        self.autoeq.telechargeur = telechargeur;
    }

    fn dossier_autoeq(&self) -> PathBuf {
        self.depot.dossier().join("autoeq")
    }

    fn fichier_index(&self) -> PathBuf {
        self.dossier_autoeq().join("index.json")
    }

    fn fichier_reglage(&self, entree: &EntreeBase) -> PathBuf {
        self.dossier_autoeq()
            .join("fichiers")
            .join(entree.nom_de_fichier_garde())
    }

    /// Ouverture du panneau : relit l'index gardé et la liste des casques déjà téléchargés.
    /// Pas de réseau ici : la base n'est téléchargée que sur demande.
    pub fn autoeq_ouvrir(&mut self) {
        self.autoeq.erreur.clear();
        if self.autoeq.disque_lu {
            return;
        }
        self.autoeq.disque_lu = true;
        self.autoeq.telecharges = std::fs::read_dir(self.dossier_autoeq().join("fichiers"))
            .map(|dossier| {
                dossier
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        match std::fs::read_to_string(self.fichier_index()) {
            Ok(texte) => match BaseAutoEq::depuis_json(&texte) {
                Ok(base) => self.autoeq.base = Some(base),
                // Un index abîmé n'est pas grave : on le dit, et on propose de le retélécharger.
                Err(e) => self.autoeq_echec(ErreurControleur::BaseAutoEq(e)),
            },
            // Pas de fichier : la base n'a jamais été téléchargée.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => self.autoeq_echec(ErreurControleur::Fichier {
                action: ActionFichier::Lecture,
                chemin: self.fichier_index(),
                source,
            }),
        }
        let requete = self.autoeq.requete.clone();
        self.autoeq_chercher(&requete);
    }

    /// Cherche un casque. Instantané : tout est en mémoire.
    pub fn autoeq_chercher(&mut self, requete: &str) {
        self.autoeq.requete = requete.to_string();
        self.autoeq.resultats = match &self.autoeq.base {
            Some(base) => autoeq_base::chercher(requete, base),
            None => Vec::new(),
        };
    }

    /// Télécharge l'index (première fois, ou « Mettre à jour la base »), sur un fil de travail.
    pub fn autoeq_mettre_a_jour(&mut self) {
        if self.autoeq.tache.is_some() {
            return;
        }
        self.autoeq.erreur.clear();
        self.lancer(Tache::Index, move |telechargeur, envoi| {
            let _ = envoi.send(Reponse::Index(
                telechargeur.obtenir(URL_INDEX, LIMITE_INDEX),
            ));
        });
    }

    /// Crée un profil depuis le résultat n° `rang` de la dernière recherche. Si ses réglages
    /// sont déjà sur ce PC, le profil est créé tout de suite et son identifiant rendu ;
    /// sinon ils sont téléchargés, et le profil naît à l'appel de [`Self::autoeq_relever`].
    pub fn autoeq_choisir(&mut self, rang: usize, t: u64) -> Option<String> {
        if self.autoeq.tache.is_some() {
            return None;
        }
        self.autoeq.erreur.clear();
        let entree = self
            .autoeq
            .resultats
            .get(rang)
            .zip(self.autoeq.base.as_ref())
            .and_then(|(r, base)| base.entrees().get(r.indice))
            .cloned();
        let Some(entree) = entree else {
            self.autoeq_echec(ErreurControleur::ResultatAutoEqInconnu);
            return None;
        };
        if let Ok(octets) = std::fs::read(self.fichier_reglage(&entree)) {
            return self.autoeq_creer_profil(&entree, &octets, t, false);
        }
        let url = entree.url();
        self.lancer(Tache::Casque, move |telechargeur, envoi| {
            let _ = envoi.send(Reponse::Casque(
                entree,
                telechargeur.obtenir(&url, LIMITE_REGLAGE),
            ));
        });
        None
    }

    /// Un téléchargement est en cours (le pont ne relève la réponse que dans ce cas).
    pub fn autoeq_occupe(&self) -> bool {
        self.autoeq.tache.is_some()
    }

    /// À appeler à chaque image : prend la réponse d'un téléchargement terminé, s'il y en a
    /// une. Rend l'identifiant du profil créé quand c'était un casque.
    pub fn autoeq_relever(&mut self, t: u64) -> Option<String> {
        let reponse = self.autoeq.reception.try_recv().ok()?;
        self.traiter(reponse, t)
    }

    /// Comme [`Self::autoeq_relever`], mais attend la réponse (pour les tests).
    #[cfg(test)]
    pub(super) fn autoeq_attendre(&mut self, t: u64) -> Option<String> {
        let reponse = self
            .autoeq
            .reception
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?;
        self.traiter(reponse, t)
    }

    fn lancer(
        &mut self,
        tache: Tache,
        travail: impl FnOnce(Arc<dyn Telechargeur>, Sender<Reponse>) + Send + 'static,
    ) {
        let telechargeur = self.autoeq.telechargeur.clone();
        let envoi = self.autoeq.envoi.clone();
        self.autoeq.tache = Some(tache);
        let lance = std::thread::Builder::new()
            .name("autoeq".into())
            .spawn(move || travail(telechargeur, envoi));
        if let Err(e) = lance {
            self.autoeq.tache = None;
            let erreur = ErreurReseau::Connexion(e.to_string());
            self.autoeq_echec(match tache {
                Tache::Index => ErreurControleur::BaseAutoEqReseau(erreur),
                Tache::Casque => ErreurControleur::CasqueAutoEqReseau(erreur),
            });
        }
    }

    fn traiter(&mut self, reponse: Reponse, t: u64) -> Option<String> {
        self.autoeq.tache = None;
        match reponse {
            Reponse::Index(Ok(octets)) => {
                if let Err(e) = self.autoeq_garder_index(&octets, t) {
                    self.autoeq_echec(e);
                }
                None
            }
            Reponse::Index(Err(e)) => {
                self.autoeq_echec(ErreurControleur::BaseAutoEqReseau(e));
                None
            }
            Reponse::Casque(entree, Ok(octets)) => {
                self.autoeq_creer_profil(&entree, &octets, t, true)
            }
            Reponse::Casque(_, Err(e)) => {
                self.autoeq_echec(ErreurControleur::CasqueAutoEqReseau(e));
                None
            }
        }
    }

    /// Lit l'index reçu, le garde sur le disque et le met en service. Un index illisible ou
    /// vide ne remplace pas celui qu'on avait.
    fn autoeq_garder_index(&mut self, octets: &[u8], t: u64) -> Issue<()> {
        let texte = String::from_utf8_lossy(octets);
        let base = BaseAutoEq::lire_index(&texte, self.date(t))?;
        let dossier = self.dossier_autoeq();
        let fichier = self.fichier_index();
        let ecrire = |chemin: &Path, source: std::io::Error| ErreurControleur::Fichier {
            action: ActionFichier::Ecriture,
            chemin: chemin.to_path_buf(),
            source,
        };
        std::fs::create_dir_all(&dossier).map_err(|e| ecrire(&dossier, e))?;
        // Écrit à côté puis renomme : un arrêt en plein milieu ne laisse jamais un index coupé.
        let provisoire = dossier.join("index.json.tmp");
        std::fs::write(&provisoire, base.vers_json()).map_err(|e| ecrire(&provisoire, e))?;
        std::fs::rename(&provisoire, &fichier).map_err(|e| ecrire(&fichier, e))?;
        let nombre = base.len();
        self.autoeq.base = Some(base);
        self.autoeq.disque_lu = true;
        let requete = self.autoeq.requete.clone();
        self.autoeq_chercher(&requete);
        let langue = self.reglages.langue;
        self.annoncer(textes::resume_base_autoeq(
            nombre,
            &dates::jour_mois(&self.date(t)),
            langue,
        ));
        Ok(())
    }

    /// Lit les réglages d'un casque, les garde sur le disque et crée le profil : le même
    /// chemin et le même résumé honnête qu'un import de fichier.
    fn autoeq_creer_profil(
        &mut self,
        entree: &EntreeBase,
        octets: &[u8],
        t: u64,
        a_garder: bool,
    ) -> Option<String> {
        let mut id = None;
        let ok = self.tenter(|c| {
            let l = c.reglages.langue;
            let texte = String::from_utf8_lossy(octets);
            let import = formats::autoeq::lire(&texte, &entree.nom_de_fichier())?;
            if a_garder {
                c.autoeq_garder_reglage(entree, octets);
            }
            let nom = textes::nom_profil_autoeq(&entree.nom, &entree.source);
            let (nouveau, compte_rendu) =
                c.creer_profil_importe(import, nom, &entree.format_origine(), t)?;
            id = Some(nouveau);
            Ok(Some(textes::annonce_autoeq(
                &compte_rendu,
                &entree.source,
                entree.plateforme(),
                l,
            )))
        });
        if !ok {
            // L'annonce porte déjà l'erreur ; le panneau la répète à côté du résultat.
            self.autoeq.erreur = self
                .annonce
                .as_ref()
                .map(|a| a.texte.clone())
                .unwrap_or_default();
        }
        id
    }

    /// Garde les réglages sur le disque pour pouvoir les importer hors ligne. Un échec ici
    /// n'empêche pas l'import : au pire, il faudra retélécharger.
    fn autoeq_garder_reglage(&mut self, entree: &EntreeBase, octets: &[u8]) {
        let chemin = self.fichier_reglage(entree);
        let ecrit = chemin
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::write(&chemin, octets));
        if ecrit.is_ok() {
            self.autoeq
                .telecharges
                .insert(entree.nom_de_fichier_garde());
        }
    }

    /// Un échec : annonce en bas de la fenêtre, et même texte dans le panneau.
    fn autoeq_echec(&mut self, erreur: ErreurControleur) {
        self.autoeq.erreur = textes::erreur(&erreur, self.reglages.langue);
        self.signaler(erreur);
    }

    /// Ce que le panneau doit montrer.
    pub fn vue_autoeq(&self) -> VueAutoEq {
        let etat = match (&self.autoeq.base, self.autoeq.tache) {
            (_, Some(Tache::Index)) => EtatBaseAutoEq::MiseAJour,
            (Some(_), _) => EtatBaseAutoEq::Prete,
            (None, _) => EtatBaseAutoEq::Absente,
        };
        let resume = self.autoeq.base.as_ref().map_or_else(String::new, |b| {
            textes::resume_base_autoeq(
                b.len(),
                &dates::jour_mois(b.recupere_le()),
                self.reglages.langue,
            )
        });
        let resultats = match &self.autoeq.base {
            Some(base) => self
                .autoeq
                .resultats
                .iter()
                .map(|r| VueResultatAutoEq {
                    nom: r.nom.clone(),
                    source: r.source.clone(),
                    plateforme: r.plateforme.clone(),
                    type_casque: r.type_casque.clone(),
                    hors_ligne: base.entrees().get(r.indice).is_some_and(|e| {
                        self.autoeq.telecharges.contains(&e.nom_de_fichier_garde())
                    }),
                })
                .collect(),
            None => Vec::new(),
        };
        VueAutoEq {
            etat,
            import_en_cours: self.autoeq.tache == Some(Tache::Casque),
            resume,
            requete: self.autoeq.requete.clone(),
            resultats,
            erreur: self.autoeq.erreur.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::reseau::FauxTelechargeur;
    use retro_engine::MoteurSimule;
    use tempfile::TempDir;

    const T0: u64 = 1_764_016_800_000;

    /// Le même genre d'index que le vrai, avec des casques inventés.
    const INDEX: &str = "\
# Index
- [Acme Studio H9](./Mesures%20Alpha/Banc%20X%20over-ear/Acme%20Studio%20H9) by Mesures Alpha on Banc X
- [Acme Studio H9 II](./Mesures%20Alpha/Banc%20X%20over-ear/Acme%20Studio%20H9%20II) by Mesures Alpha on Banc X
- [Acme Studio H9](./Labo%20Beta/over-ear/Acme%20Studio%20H9) by Labo Beta
- [Orbe & Co Delta](./Labo%20Beta/in-ear/Orbe%20&%20Co%20Delta) by Labo Beta
";

    const REGLAGE: &str = "\
Preamp: -6.4 dB
Filter 1: ON LSC Fc 105 Hz Gain 8.0 dB Q 0.70
Filter 2: ON PK Fc 830 Hz Gain 5.7 dB Q 1.32
Filter 3: ON HSC Fc 10000 Hz Gain -6.9 dB Q 0.70
";

    struct Banc {
        dossier: TempDir,
        faux: Arc<FauxTelechargeur>,
    }

    impl Banc {
        fn nouveau() -> Banc {
            let faux = Arc::new(FauxTelechargeur::default());
            faux.texte(URL_INDEX, INDEX);
            Banc {
                dossier: TempDir::new().unwrap(),
                faux,
            }
        }

        fn ouvrir(&self) -> Controleur {
            let options = Options {
                dossier_export: self.dossier.path().join("exports"),
                moteur_absent: false,
                decalage_minutes: 60,
                graine: Some(7),
                langue_systeme: Langue::Fr,
                langue_imposee: None,
            };
            let mut c = Controleur::ouvrir(
                self.dossier.path().join("donnees"),
                Box::new(MoteurSimule::nouveau()),
                options,
                T0,
            )
            .unwrap();
            c.regler_telechargeur(self.faux.clone());
            c.prendre_annonce();
            c
        }

        fn url_reglage(&self, c: &Controleur, rang: usize) -> String {
            let indice = c.autoeq.resultats[rang].indice;
            c.autoeq.base.as_ref().unwrap().entrees()[indice].url()
        }
    }

    /// Une base téléchargée et prête, dans un contrôleur neuf.
    fn avec_base(banc: &Banc) -> Controleur {
        let mut c = banc.ouvrir();
        c.autoeq_ouvrir();
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        c.prendre_annonce();
        c
    }

    #[test]
    fn la_base_est_absente_tant_qu_elle_n_a_pas_ete_telechargee() {
        let banc = Banc::nouveau();
        let mut c = banc.ouvrir();
        c.autoeq_ouvrir();
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Absente);
        assert!(vue.resume.is_empty() && vue.erreur.is_empty());
        c.autoeq_chercher("h9");
        assert!(c.vue_autoeq().resultats.is_empty());
        assert!(
            banc.faux.demandes().is_empty(),
            "rien n'est téléchargé tout seul"
        );
    }

    #[test]
    fn mettre_a_jour_telecharge_et_garde_l_index() {
        let banc = Banc::nouveau();
        let mut c = banc.ouvrir();
        c.autoeq_ouvrir();
        c.autoeq_mettre_a_jour();
        assert_eq!(c.vue_autoeq().etat, EtatBaseAutoEq::MiseAJour);
        assert_eq!(c.autoeq_attendre(T0), None);
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete);
        assert_eq!(vue.resume, "4 casques · mise à jour le 24/11");
        assert_eq!(banc.faux.demandes(), vec![URL_INDEX.to_string()]);
        assert!(banc
            .dossier
            .path()
            .join("donnees/autoeq/index.json")
            .is_file());
        assert!(!c.annonce.as_ref().unwrap().erreur);
    }

    #[test]
    fn l_index_garde_sert_apres_un_redemarrage_sans_reseau() {
        let banc = Banc::nouveau();
        drop(avec_base(&banc));
        banc.faux.couper_le_reseau();
        let mut c = banc.ouvrir();
        c.autoeq_ouvrir();
        assert_eq!(c.vue_autoeq().etat, EtatBaseAutoEq::Prete);
        c.autoeq_chercher("delta");
        let vue = c.vue_autoeq();
        assert_eq!(vue.resultats.len(), 1);
        assert_eq!(vue.resultats[0].source, "Labo Beta");
        assert_eq!(vue.resultats[0].plateforme, None);
        assert_eq!(
            banc.faux.demandes().len(),
            1,
            "un seul téléchargement : le premier"
        );
    }

    #[test]
    fn la_recherche_liste_toutes_les_sources() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.autoeq_chercher("acme studio h9");
        let sources: Vec<_> = c
            .vue_autoeq()
            .resultats
            .iter()
            .map(|r| (r.nom.clone(), r.source.clone(), r.plateforme.clone()))
            .collect();
        assert_eq!(sources[0].1, "Labo Beta");
        assert_eq!(
            sources[1],
            (
                "Acme Studio H9".into(),
                "Mesures Alpha".into(),
                Some("Banc X".into())
            )
        );
        assert_eq!(sources[2].0, "Acme Studio H9 II");
    }

    #[test]
    fn creer_un_profil_telecharge_importe_et_nomme_la_source() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.autoeq_chercher("h9 ii");
        banc.faux.texte(&banc.url_reglage(&c, 0), REGLAGE);
        assert_eq!(
            c.autoeq_choisir(0, T0),
            None,
            "le téléchargement est en cours"
        );
        assert!(c.vue_autoeq().import_en_cours);
        let id = c.autoeq_attendre(T0 + 5).unwrap();
        assert!(!c.vue_autoeq().import_en_cours);

        let profil = c.profil(&id).unwrap();
        assert_eq!(profil.nom(), "Acme Studio H9 II (Mesures Alpha)");
        let version = profil.version_active();
        assert_eq!(
            version.origine,
            Origine::Import {
                format: "autoeq|Mesures Alpha|Banc X".into()
            }
        );
        // Le préampli est l'automatique, pas celui du fichier.
        assert!((version.courbe.preampli_db - version.courbe.preampli_auto_db()).abs() < 1e-9);
        assert_eq!(version.courbe.filtres.len(), 3);
        let annonce = c.prendre_annonce().unwrap();
        assert!(!annonce.erreur);
        assert!(annonce.texte.contains("AutoEQ"), "{}", annonce.texte);
        assert!(
            annonce.texte.contains("Mesures Alpha (Banc X)"),
            "{}",
            annonce.texte
        );
        assert!(
            annonce.texte.contains("point de départ"),
            "{}",
            annonce.texte
        );
        // Le profil est enregistré sur le disque.
        assert!(c.depot.charger_profil(&id).is_ok());
    }

    #[test]
    fn un_casque_deja_telecharge_s_importe_hors_ligne() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.autoeq_chercher("delta");
        banc.faux.texte(&banc.url_reglage(&c, 0), REGLAGE);
        c.autoeq_choisir(0, T0);
        c.autoeq_attendre(T0).unwrap();
        assert!(c.vue_autoeq().resultats[0].hors_ligne);

        banc.faux.couper_le_reseau();
        let avant = banc.faux.demandes().len();
        let id = c.autoeq_choisir(0, T0 + 10).unwrap();
        assert_eq!(banc.faux.demandes().len(), avant, "aucune demande réseau");
        assert_ne!(c.profil(&id).unwrap().nom(), "");
        assert_eq!(
            c.profils.len(),
            3,
            "le profil de départ et les deux importés"
        );
    }

    #[test]
    fn un_casque_jamais_telecharge_ne_s_importe_pas_hors_ligne_et_le_dit() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        banc.faux.couper_le_reseau();
        banc.faux.repondre(
            &banc.url_reglage_apres_recherche(&mut c),
            Err(ErreurReseau::Connexion("coupé".into())),
        );
        let profils = c.profils.len();
        c.autoeq_choisir(0, T0);
        assert_eq!(c.autoeq_attendre(T0), None);
        assert_eq!(c.profils.len(), profils);
        let vue = c.vue_autoeq();
        assert!(vue.erreur.contains("Sans connexion"), "{}", vue.erreur);
        assert!(vue.erreur.contains("déjà téléchargés"), "{}", vue.erreur);
        assert!(c.annonce.as_ref().unwrap().erreur);
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete, "la base reste utilisable");
        assert!(!vue.import_en_cours);
    }

    impl Banc {
        fn url_reglage_apres_recherche(&self, c: &mut Controleur) -> String {
            c.autoeq_chercher("delta");
            self.url_reglage(c, 0)
        }
    }

    #[test]
    fn une_erreur_http_est_dite_sans_perdre_la_base() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.autoeq_chercher("delta");
        // Pas de réponse préparée pour ce fichier : 404.
        c.autoeq_choisir(0, T0);
        assert_eq!(c.autoeq_attendre(T0), None);
        let vue = c.vue_autoeq();
        assert!(vue.erreur.contains("404"), "{}", vue.erreur);
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete);
        assert_eq!(vue.resultats.len(), 1);
        // On peut réessayer.
        banc.faux.texte(&banc.url_reglage(&c, 0), REGLAGE);
        c.autoeq_choisir(0, T0 + 1);
        assert!(c.autoeq_attendre(T0 + 1).is_some());
        assert!(c.vue_autoeq().erreur.is_empty());
    }

    #[test]
    fn un_fichier_de_reglages_illisible_ne_cree_pas_de_profil_et_n_est_pas_garde() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.autoeq_chercher("delta");
        banc.faux
            .texte(&banc.url_reglage(&c, 0), "<html>Not found</html>");
        let profils = c.profils.len();
        c.autoeq_choisir(0, T0);
        assert_eq!(c.autoeq_attendre(T0), None);
        assert_eq!(c.profils.len(), profils);
        assert!(
            c.vue_autoeq().erreur.contains("illisible"),
            "{}",
            c.vue_autoeq().erreur
        );
        assert!(!c.vue_autoeq().resultats[0].hors_ligne);
    }

    #[test]
    fn echec_du_telechargement_de_l_index_garde_l_ancien_et_le_dit() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        banc.faux.repondre(URL_INDEX, Err(ErreurReseau::Http(503)));
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete);
        assert!(vue.erreur.contains("503"), "{}", vue.erreur);
        assert!(vue.erreur.contains("reste en place"), "{}", vue.erreur);
        assert!(c.annonce.as_ref().unwrap().erreur);
    }

    #[test]
    fn premiere_base_impossible_a_telecharger_reste_absente() {
        let banc = Banc::nouveau();
        banc.faux
            .repondre(URL_INDEX, Err(ErreurReseau::Connexion("hors ligne".into())));
        let mut c = banc.ouvrir();
        c.autoeq_ouvrir();
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Absente);
        assert!(vue.erreur.contains("pas de connexion"), "{}", vue.erreur);
    }

    #[test]
    fn un_index_recu_sans_casque_ne_remplace_pas_l_ancien() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        banc.faux.texte(URL_INDEX, "<html>Page d'erreur</html>");
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete);
        assert!(vue.resume.starts_with("4 casques"), "{}", vue.resume);
        assert!(
            vue.erreur.contains("aucun casque lisible"),
            "{}",
            vue.erreur
        );
        // Et le fichier gardé est toujours bon.
        let mut autre = banc.ouvrir();
        autre.autoeq_ouvrir();
        assert_eq!(autre.vue_autoeq().etat, EtatBaseAutoEq::Prete);
    }

    #[test]
    fn un_index_garde_abime_est_signale_et_peut_etre_retelecharge() {
        let banc = Banc::nouveau();
        drop(avec_base(&banc));
        std::fs::write(
            banc.dossier.path().join("donnees/autoeq/index.json"),
            "{pas du json",
        )
        .unwrap();
        let mut c = banc.ouvrir();
        c.autoeq_ouvrir();
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Absente);
        assert!(vue.erreur.contains("illisible"), "{}", vue.erreur);
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        let vue = c.vue_autoeq();
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete);
        assert!(vue.erreur.is_empty());
    }

    #[test]
    fn une_seule_tache_a_la_fois() {
        let banc = Banc::nouveau();
        let mut c = banc.ouvrir();
        c.autoeq_mettre_a_jour();
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        assert_eq!(banc.faux.demandes().len(), 1);
    }

    #[test]
    fn choisir_un_rang_inconnu_est_une_erreur_calme() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        assert_eq!(c.autoeq_choisir(3, T0), None);
        assert!(c.vue_autoeq().erreur.contains("refais la recherche"));
    }

    #[test]
    fn en_anglais_les_textes_suivent_la_langue() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.reglages.langue = Langue::En;
        c.autoeq_chercher("delta");
        c.autoeq_choisir(0, T0);
        c.autoeq_attendre(T0);
        let erreur = c.vue_autoeq().erreur;
        assert!(erreur.contains("Couldn’t download"), "{erreur}");
        assert!(erreur.contains("no longer exists"), "{erreur}");
    }

    #[test]
    fn le_profil_importe_s_affiche_avec_sa_source() {
        let banc = Banc::nouveau();
        let mut c = avec_base(&banc);
        c.autoeq_chercher("delta");
        banc.faux.texte(&banc.url_reglage(&c, 0), REGLAGE);
        c.autoeq_choisir(0, T0);
        let id = c.autoeq_attendre(T0).unwrap();
        let vue = c.vue_profils().into_iter().find(|p| p.id == id).unwrap();
        assert_eq!(vue.resume, "Importé de AutoEQ · mesures Labo Beta");
        let versions = c.vue_versions(&id);
        assert_eq!(versions[0].origine, "Import AutoEQ · mesures Labo Beta");
    }

    /// Vérification contre le vrai dépôt d'AutoEQ (réseau nécessaire), ignorée par défaut :
    /// `cargo test -p retro-app reel_ -- --ignored --nocapture`.
    #[test]
    #[ignore = "télécharge la vraie base AutoEQ"]
    fn reel_index_recherche_et_import_de_l_inzone_h9() {
        let banc = Banc::nouveau();
        let mut c = banc.ouvrir();
        c.regler_telechargeur(Arc::new(TelechargeurUreq::nouveau()));
        c.autoeq_ouvrir();
        c.autoeq_mettre_a_jour();
        c.autoeq_attendre(T0);
        let vue = c.vue_autoeq();
        println!("état : {:?}, erreur : {:?}", vue.etat, vue.erreur);
        assert_eq!(vue.etat, EtatBaseAutoEq::Prete, "{}", vue.erreur);
        let taille = std::fs::metadata(banc.dossier.path().join("donnees/autoeq/index.json"))
            .unwrap()
            .len();
        println!("index : {} ({} octets gardés)", vue.resume, taille);

        for requete in ["INZONE H9", "inzone h9 ii", "rog delta", "oratory hd 800"] {
            c.autoeq_chercher(requete);
            let r = c.vue_autoeq().resultats;
            println!("« {requete} » : {} résultat(s)", r.len());
            for ligne in r.iter().take(6) {
                println!(
                    "    {} | {} | {:?} | {:?}",
                    ligne.nom, ligne.source, ligne.plateforme, ligne.type_casque
                );
            }
        }

        c.autoeq_chercher("INZONE H9");
        assert!(c
            .vue_autoeq()
            .resultats
            .iter()
            .any(|r| r.nom == "Sony INZONE H9"));
        let id = {
            c.autoeq_choisir(0, T0 + 1);
            c.autoeq_attendre(T0 + 1)
        };
        let id = id.unwrap_or_else(|| panic!("import raté : {}", c.vue_autoeq().erreur));
        let annonce = c.prendre_annonce().unwrap().texte;
        let profil = c.profil(&id).unwrap();
        let courbe = &profil.version_active().courbe;
        println!(
            "profil « {} » : {} filtres, préampli {:+.1} dB",
            profil.nom(),
            courbe.filtres.len(),
            courbe.preampli_db
        );
        for f in &courbe.filtres {
            println!("    {:?}", f);
        }
        println!("annonce : {annonce}");
        assert_eq!(courbe.filtres.len(), 10);
    }
}
