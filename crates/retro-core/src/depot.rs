//! Le dépôt : tout ce que l'appli garde sur le disque (ADR-0007).
//!
//! ```text
//! <dossier>/profils/<id>.json    un profil et toutes ses versions
//! <dossier>/reglages.json        les réglages de l'appli
//! <dossier>/etat-moteur.json     la courbe qui devrait être active
//! <dossier>/tests/<id>.jsonl     le journal d'un test
//! ```
//!
//! Les fichiers JSON sont remplacés de façon atomique : on écrit à côté, puis on renomme.
//! Un plantage laisse l'ancien fichier intact ou le nouveau complet, jamais un demi-fichier.

use crate::journal::{Journal, LectureJournal};
use crate::profil::Profil;
use crate::reglages::ReglagesAppli;
use retro_dsp::Courbe;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Ce qui peut mal tourner dans le dépôt.
#[derive(Debug, Error)]
pub enum ErreurDepot {
    #[error("erreur de fichier sur {} : {source}", chemin.display())]
    Io {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON invalide dans {} : {source}", chemin.display())]
    Json {
        chemin: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// Un identifiant sert de nom de fichier : lettres, chiffres, `-` et `_` seulement.
    #[error("identifiant invalide : {0:?}")]
    IdentifiantInvalide(String),
}

impl ErreurDepot {
    /// Vrai si l'erreur vient d'un fichier qui n'existe pas.
    pub fn est_introuvable(&self) -> bool {
        matches!(self, ErreurDepot::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound)
    }
}

fn io(chemin: &Path, source: std::io::Error) -> ErreurDepot {
    ErreurDepot::Io {
        chemin: chemin.to_path_buf(),
        source,
    }
}

/// Écrit `octets` dans `chemin` de façon atomique : fichier temporaire dans le même dossier
/// (un renommage ne traverse pas de disque), écrit et synchronisé, puis renommé par-dessus.
pub(crate) fn ecrire_atomique(chemin: &Path, octets: &[u8]) -> Result<(), ErreurDepot> {
    let dossier = chemin.parent().unwrap_or_else(|| Path::new("."));
    let nom = chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporaire = dossier.join(format!(".{nom}.{}.tmp", std::process::id()));
    let ecriture = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&temporaire)?;
        f.write_all(octets)?;
        f.sync_all()?;
        fs::rename(&temporaire, chemin)
    })();
    if let Err(e) = ecriture {
        let _ = fs::remove_file(&temporaire);
        return Err(io(chemin, e));
    }
    // Sur Unix, on synchronise aussi le dossier pour que le renommage survive à une coupure.
    // Sous Windows ouvrir un dossier échoue : sans importance, on ignore.
    if let Ok(d) = fs::File::open(dossier) {
        let _ = d.sync_all();
    }
    Ok(())
}

fn ecrire_json<T: Serialize>(chemin: &Path, valeur: &T) -> Result<(), ErreurDepot> {
    let mut octets = serde_json::to_vec_pretty(valeur).map_err(|source| ErreurDepot::Json {
        chemin: chemin.to_path_buf(),
        source,
    })?;
    octets.push(b'\n');
    ecrire_atomique(chemin, &octets)
}

fn lire_json<T: DeserializeOwned>(chemin: &Path) -> Result<T, ErreurDepot> {
    let octets = fs::read(chemin).map_err(|e| io(chemin, e))?;
    serde_json::from_slice(&octets).map_err(|source| ErreurDepot::Json {
        chemin: chemin.to_path_buf(),
        source,
    })
}

/// Vérifie qu'un identifiant peut servir de nom de fichier (pas de `/`, de `..`, etc.).
fn verifier_id(id: &str) -> Result<(), ErreurDepot> {
    let valide = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valide {
        Ok(())
    } else {
        Err(ErreurDepot::IdentifiantInvalide(id.to_string()))
    }
}

/// La courbe que le moteur devrait avoir, sauvée pour que l'appli la remette au démarrage :
/// une variante restée active après un plantage est ainsi remplacée par la référence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EtatMoteurSauve {
    pub courbe: Courbe,
    /// Le profil dont elle vient, s'il y en a un.
    #[serde(default)]
    pub profil_id: Option<String>,
    /// La version du profil dont elle vient.
    #[serde(default)]
    pub version: Option<u32>,
}

/// Un fichier de profil qu'on n'a pas pu lire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierIgnore {
    pub chemin: PathBuf,
    pub raison: String,
}

/// Les profils d'un dépôt. Un fichier abîmé n'empêche pas de charger les autres : il est
/// signalé dans `ignores`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfilsListes {
    /// Triés par identifiant.
    pub profils: Vec<Profil>,
    pub ignores: Vec<FichierIgnore>,
}

/// Le dossier de données de l'appli.
#[derive(Debug, Clone)]
pub struct Depot {
    dossier: PathBuf,
}

impl Depot {
    /// Ouvre le dépôt dans `dossier` (`%APPDATA%\Retro EQ`), en le créant avec ses
    /// sous-dossiers s'il n'existe pas.
    pub fn ouvrir(dossier: impl AsRef<Path>) -> Result<Depot, ErreurDepot> {
        let dossier = dossier.as_ref().to_path_buf();
        for d in [
            dossier.clone(),
            dossier.join("profils"),
            dossier.join("tests"),
        ] {
            fs::create_dir_all(&d).map_err(|e| io(&d, e))?;
        }
        Ok(Depot { dossier })
    }

    pub fn dossier(&self) -> &Path {
        &self.dossier
    }

    fn chemin_profil(&self, id: &str) -> Result<PathBuf, ErreurDepot> {
        verifier_id(id)?;
        Ok(self.dossier.join("profils").join(format!("{id}.json")))
    }

    fn chemin_journal(&self, test_id: &str) -> Result<PathBuf, ErreurDepot> {
        verifier_id(test_id)?;
        Ok(self.dossier.join("tests").join(format!("{test_id}.jsonl")))
    }

    // --- Profils ---

    /// Écrit le profil (toutes ses versions) dans `profils/<id>.json`.
    pub fn sauver_profil(&self, profil: &Profil) -> Result<(), ErreurDepot> {
        ecrire_json(&self.chemin_profil(profil.id())?, profil)
    }

    pub fn charger_profil(&self, id: &str) -> Result<Profil, ErreurDepot> {
        lire_json(&self.chemin_profil(id)?)
    }

    /// Tous les profils du dépôt, triés par identifiant.
    pub fn lister_profils(&self) -> Result<ProfilsListes, ErreurDepot> {
        let dossier = self.dossier.join("profils");
        let mut liste = ProfilsListes {
            profils: Vec::new(),
            ignores: Vec::new(),
        };
        for entree in fs::read_dir(&dossier).map_err(|e| io(&dossier, e))? {
            let chemin = entree.map_err(|e| io(&dossier, e))?.path();
            if chemin.extension().and_then(|e| e.to_str()) != Some("json") {
                continue; // fichiers temporaires et autres
            }
            match lire_json::<Profil>(&chemin) {
                Ok(p) => liste.profils.push(p),
                Err(e) => liste.ignores.push(FichierIgnore {
                    chemin,
                    raison: e.to_string(),
                }),
            }
        }
        liste.profils.sort_by(|a, b| a.id().cmp(b.id()));
        liste.ignores.sort_by(|a, b| a.chemin.cmp(&b.chemin));
        Ok(liste)
    }

    // --- Réglages ---

    /// Les réglages de l'appli ; les valeurs par défaut si le fichier n'existe pas encore.
    pub fn charger_reglages(&self) -> Result<ReglagesAppli, ErreurDepot> {
        match lire_json(&self.dossier.join("reglages.json")) {
            Err(e) if e.est_introuvable() => Ok(ReglagesAppli::default()),
            autre => autre,
        }
    }

    pub fn sauver_reglages(&self, reglages: &ReglagesAppli) -> Result<(), ErreurDepot> {
        ecrire_json(&self.dossier.join("reglages.json"), reglages)
    }

    // --- État du moteur ---

    /// La courbe qui devrait être active, ou `None` si rien n'a encore été sauvé.
    pub fn charger_etat_moteur(&self) -> Result<Option<EtatMoteurSauve>, ErreurDepot> {
        match lire_json(&self.dossier.join("etat-moteur.json")) {
            Err(e) if e.est_introuvable() => Ok(None),
            autre => autre.map(Some),
        }
    }

    pub fn sauver_etat_moteur(&self, etat: &EtatMoteurSauve) -> Result<(), ErreurDepot> {
        ecrire_json(&self.dossier.join("etat-moteur.json"), etat)
    }

    // --- Journaux de test ---

    /// Ouvre (ou crée) le journal du test `test_id`, pour y ajouter des entrées.
    pub fn ouvrir_journal(&self, test_id: &str) -> Result<Journal, ErreurDepot> {
        Journal::ouvrir(self.chemin_journal(test_id)?)
    }

    /// Relit le journal du test `test_id`.
    pub fn lire_journal(&self, test_id: &str) -> Result<LectureJournal, ErreurDepot> {
        LectureJournal::lire(self.chemin_journal(test_id)?)
    }

    /// Les tests commencés mais ni terminés ni abandonnés : ceux qu'on peut proposer de
    /// reprendre. Un journal sans aucune entrée lisible n'a rien à reprendre et n'est pas
    /// compté. Triés par identifiant.
    pub fn tests_interrompus(&self) -> Result<Vec<String>, ErreurDepot> {
        let dossier = self.dossier.join("tests");
        let mut ids = Vec::new();
        for entree in fs::read_dir(&dossier).map_err(|e| io(&dossier, e))? {
            let chemin = entree.map_err(|e| io(&dossier, e))?.path();
            if chemin.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(id) = chemin.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let lecture = LectureJournal::lire(&chemin)?;
            if !lecture.entrees.is_empty() && !lecture.est_clos() {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::Entree;
    use crate::profil::Origine;
    use crate::reglages::{Langue, Navigation};
    use tempfile::tempdir;

    fn profil(id: &str) -> Profil {
        let mut p = Profil::nouveau(id, "Mon casque", Courbe::default(), Origine::Manuel, "d");
        p.ajouter_version(Courbe::default(), Origine::Manuel, [2.0; 7], "d2");
        p
    }

    fn fichiers(dossier: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dossier)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn ouvrir_cree_le_dossier_et_ses_sous_dossiers() {
        let t = tempdir().unwrap();
        let racine = t.path().join("Retro EQ");
        let d = Depot::ouvrir(&racine).unwrap();
        assert!(racine.join("profils").is_dir());
        assert!(racine.join("tests").is_dir());
        assert_eq!(d.dossier(), racine);
        Depot::ouvrir(&racine).unwrap(); // deuxième ouverture : sans effet
    }

    #[test]
    fn un_profil_fait_l_aller_retour_et_sort_en_json_lisible() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        let p = profil("casque-1");
        d.sauver_profil(&p).unwrap();
        assert_eq!(d.charger_profil("casque-1").unwrap(), p);
        let texte = fs::read_to_string(t.path().join("profils/casque-1.json")).unwrap();
        assert!(texte.contains("\n  \"nom\": \"Mon casque\""), "{texte}");
    }

    #[test]
    fn sauver_remplace_sans_laisser_de_fichier_temporaire() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        let mut p = profil("a");
        d.sauver_profil(&p).unwrap();
        p.renommer("Nouveau nom");
        d.sauver_profil(&p).unwrap();
        assert_eq!(d.charger_profil("a").unwrap().nom(), "Nouveau nom");
        assert_eq!(fichiers(&t.path().join("profils")), vec!["a.json"]);
    }

    #[test]
    fn l_ecriture_atomique_remplace_un_fichier_existant_et_nettoie_en_cas_d_echec() {
        let t = tempdir().unwrap();
        let chemin = t.path().join("x.json");
        ecrire_atomique(&chemin, b"un").unwrap();
        ecrire_atomique(&chemin, b"deux").unwrap();
        assert_eq!(fs::read(&chemin).unwrap(), b"deux");
        assert_eq!(fichiers(t.path()), vec!["x.json"]);
        // Le dossier cible n'existe pas : échec, et rien ne traîne.
        let e = ecrire_atomique(&t.path().join("absent/x.json"), b"x").unwrap_err();
        assert!(matches!(e, ErreurDepot::Io { .. }));
        assert_eq!(fichiers(t.path()), vec!["x.json"]);
    }

    #[test]
    fn charger_un_profil_inconnu_est_introuvable() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        assert!(d.charger_profil("nope").unwrap_err().est_introuvable());
    }

    #[test]
    fn un_identifiant_dangereux_est_refuse() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        for id in ["", "../evil", "a/b", "a b", "a.json", "é"] {
            assert!(
                matches!(
                    d.charger_profil(id),
                    Err(ErreurDepot::IdentifiantInvalide(_))
                ),
                "{id:?}"
            );
            assert!(matches!(
                d.ouvrir_journal(id),
                Err(ErreurDepot::IdentifiantInvalide(_))
            ));
        }
        assert!(matches!(
            d.sauver_profil(&profil("../x")),
            Err(ErreurDepot::IdentifiantInvalide(_))
        ));
    }

    #[test]
    fn lister_les_profils_trie_et_signale_les_fichiers_abimes() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        d.sauver_profil(&profil("b")).unwrap();
        d.sauver_profil(&profil("a")).unwrap();
        fs::write(t.path().join("profils/casse.json"), "{pas du json").unwrap();
        fs::write(t.path().join("profils/notes.txt"), "ignoré").unwrap();
        let l = d.lister_profils().unwrap();
        let ids: Vec<_> = l.profils.iter().map(|p| p.id().to_string()).collect();
        assert_eq!(ids, vec!["a", "b"]);
        assert_eq!(l.ignores.len(), 1);
        assert!(l.ignores[0].chemin.ends_with("casse.json"));
    }

    #[test]
    fn reglages_par_defaut_puis_aller_retour() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        assert_eq!(d.charger_reglages().unwrap(), ReglagesAppli::default());
        let r = ReglagesAppli {
            langue: Langue::En,
            navigation: Navigation::Rail,
            intervalle_ms: 45_000,
            bip: false,
            ..Default::default()
        };
        d.sauver_reglages(&r).unwrap();
        assert_eq!(d.charger_reglages().unwrap(), r);
    }

    #[test]
    fn un_vieux_fichier_de_reglages_sur_disque_se_charge() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        fs::write(
            t.path().join("reglages.json"),
            r#"{"langue":"en","bip":false}"#,
        )
        .unwrap();
        let r = d.charger_reglages().unwrap();
        assert_eq!(r.langue, Langue::En);
        assert!(!r.bip);
        assert_eq!(r.intervalle_ms, 20_000);
    }

    #[test]
    fn des_reglages_corrompus_sont_une_erreur_json() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        fs::write(t.path().join("reglages.json"), "{").unwrap();
        assert!(matches!(
            d.charger_reglages(),
            Err(ErreurDepot::Json { .. })
        ));
    }

    #[test]
    fn etat_du_moteur_absent_puis_aller_retour() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        assert_eq!(d.charger_etat_moteur().unwrap(), None);
        let e = EtatMoteurSauve {
            courbe: Courbe {
                preampli_db: -3.0,
                filtres: vec![],
            },
            profil_id: Some("a".into()),
            version: Some(2),
        };
        d.sauver_etat_moteur(&e).unwrap();
        assert_eq!(d.charger_etat_moteur().unwrap(), Some(e));
        assert!(t.path().join("etat-moteur.json").is_file());
    }

    fn debut(id: &str) -> Entree {
        Entree::TestDemarre {
            test_id: id.into(),
            profil_id: "p".into(),
            version_reference: 1,
            courbe_reference: Courbe::default(),
            mode: crate::plan::Mode::DuelsEnJeu { precis: false },
            plan: vec![],
            reglages: Default::default(),
            variantes: Default::default(),
            horodatage_ms: 1,
        }
    }

    #[test]
    fn tests_interrompus_ne_retient_que_les_journaux_sans_fin() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        // En cours (interrompu).
        d.ouvrir_journal("t-en-cours")
            .unwrap()
            .ajouter(&debut("t-en-cours"))
            .unwrap();
        // Abandonné.
        let mut j = d.ouvrir_journal("t-abandonne").unwrap();
        j.ajouter(&debut("t-abandonne")).unwrap();
        j.ajouter(&Entree::TestAbandonne { horodatage_ms: 2 })
            .unwrap();
        // Terminé.
        let mut j = d.ouvrir_journal("t-termine").unwrap();
        j.ajouter(&debut("t-termine")).unwrap();
        let resultat = crate::resultat::calculer_resultat(
            &Courbe::default(),
            &profil("p").version_active().clone(),
            &Default::default(),
            &[],
            &[],
            None,
            &[],
        );
        j.ajouter(&Entree::TestTermine {
            resultat,
            horodatage_ms: 3,
        })
        .unwrap();
        // Interrompu en pleine écriture de sa deuxième ligne : reste interrompu.
        let mut contenu = serde_json::to_vec(&debut("t-coupe")).unwrap();
        contenu.push(b'\n');
        contenu.extend(b"{\"type\":\"duel_dem");
        fs::write(t.path().join("tests/t-coupe.jsonl"), contenu).unwrap();
        // Vide : rien à reprendre.
        d.ouvrir_journal("t-vide").unwrap();
        // Autre fichier : ignoré.
        fs::write(t.path().join("tests/notes.txt"), "x").unwrap();

        assert_eq!(
            d.tests_interrompus().unwrap(),
            vec!["t-coupe".to_string(), "t-en-cours".to_string()]
        );
    }

    #[test]
    fn lire_journal_signale_la_ligne_tronquee() {
        let t = tempdir().unwrap();
        let d = Depot::ouvrir(t.path()).unwrap();
        d.ouvrir_journal("x").unwrap().ajouter(&debut("x")).unwrap();
        let chemin = t.path().join("tests/x.jsonl");
        let mut contenu = fs::read(&chemin).unwrap();
        contenu.extend(b"{\"type\":\"reprise\",\"horod");
        fs::write(&chemin, contenu).unwrap();
        let l = d.lire_journal("x").unwrap();
        assert_eq!(l.entrees.len(), 1);
        assert!(l.derniere_ligne_tronquee);
    }
}
