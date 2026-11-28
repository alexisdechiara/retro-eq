//! Le journal d'un test : un fichier `.jsonl`, une [`Entree`] par ligne, écrite sur le disque
//! aussitôt (ADR-0007). L'état d'un test se reconstruit en relisant son journal : c'est ce qui
//! garde chaque réponse dès l'appui sur la touche et permet de reprendre après un plantage.

use crate::depot::ErreurDepot;
use crate::plan::{Cote, DuelPrevu, Mode, Reponse};
use crate::reglages::ReglagesTest;
use crate::resultat::ResultatTest;
use crate::zone::ReglagesVariantes;
use retro_dsp::Courbe;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Pourquoi un test est en pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RaisonPause {
    /// L'utilisateur a mis en pause.
    Manuelle,
    /// Un appel vocal est en cours.
    Appel,
    /// « Plus tard » : le duel en cours sera rejoué.
    PlusTard,
    /// Personne n'a répondu à temps : le test s'est mis en pause tout seul (ADR-0012).
    SansReponse,
}

/// Un événement du journal. Une ligne JSON par entrée, avec un champ `"type"` qui nomme la
/// variante. Toutes portent l'heure de l'événement (`horodatage_ms`, ms depuis l'epoch Unix,
/// donnée par l'appelant : ce crate ne lit jamais l'horloge).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Entree {
    /// Première ligne : tout ce qu'il faut pour rejouer le test.
    TestDemarre {
        test_id: String,
        profil_id: String,
        /// Numéro de la version de référence dans le profil.
        version_reference: u32,
        courbe_reference: Courbe,
        mode: Mode,
        plan: Vec<DuelPrevu>,
        reglages: ReglagesTest,
        /// Les réglages des variantes (pas initial, plancher) en vigueur pour ce test.
        /// Absents des journaux plus anciens : valeurs par défaut.
        #[serde(default)]
        variantes: ReglagesVariantes,
        horodatage_ms: u64,
    },
    /// Le duel `duel` (indice dans le plan, depuis 0) commence.
    DuelDemarre { duel: u32, horodatage_ms: u64 },
    /// Un passage commence : quel côté joue, et quelle appli est au premier plan.
    PassageDemarre {
        duel: u32,
        /// Indice du passage dans le duel, de 0 à 5.
        passage: u32,
        cote: Cote,
        appli_premier_plan: Option<String>,
        horodatage_ms: u64,
    },
    /// La réponse à un duel. Écrite avant toute autre conséquence.
    ReponseDonnee {
        duel: u32,
        reponse: Reponse,
        horodatage_ms: u64,
    },
    /// La dernière réponse est annulée : le duel sera rejoué.
    ReponseAnnulee { duel: u32, horodatage_ms: u64 },
    /// Le duel est passé, sans réponse.
    DuelPasse { duel: u32, horodatage_ms: u64 },
    /// Le duel recommence depuis son premier passage.
    DuelRecommence { duel: u32, horodatage_ms: u64 },
    /// Le duel `duel` va être rejoué après une interruption, avec un nouveau tirage
    /// (qui est A, ordre des passages) : `prevu` remplace le duel du plan (ADR-0012).
    /// Toujours écrit avant le `DuelDemarre` du duel rejoué.
    DuelRetire {
        duel: u32,
        prevu: DuelPrevu,
        horodatage_ms: u64,
    },
    /// Le test est en pause.
    Pause {
        raison: RaisonPause,
        horodatage_ms: u64,
    },
    /// Le test reprend.
    Reprise { horodatage_ms: u64 },
    /// Un duel de vérification (référence contre nouvelle version) est ajouté en fin de plan.
    DuelVerificationAjoute { duel: DuelPrevu, horodatage_ms: u64 },
    /// Le test est terminé, avec son résultat.
    TestTermine {
        resultat: ResultatTest,
        horodatage_ms: u64,
    },
    /// Le test est abandonné.
    TestAbandonne { horodatage_ms: u64 },
}

impl Entree {
    /// L'heure de l'événement, en ms depuis l'epoch Unix.
    pub fn horodatage_ms(&self) -> u64 {
        match self {
            Entree::TestDemarre { horodatage_ms, .. }
            | Entree::DuelDemarre { horodatage_ms, .. }
            | Entree::PassageDemarre { horodatage_ms, .. }
            | Entree::ReponseDonnee { horodatage_ms, .. }
            | Entree::ReponseAnnulee { horodatage_ms, .. }
            | Entree::DuelPasse { horodatage_ms, .. }
            | Entree::DuelRecommence { horodatage_ms, .. }
            | Entree::DuelRetire { horodatage_ms, .. }
            | Entree::Pause { horodatage_ms, .. }
            | Entree::Reprise { horodatage_ms }
            | Entree::DuelVerificationAjoute { horodatage_ms, .. }
            | Entree::TestTermine { horodatage_ms, .. }
            | Entree::TestAbandonne { horodatage_ms } => *horodatage_ms,
        }
    }

    /// Vrai pour les entrées qui closent un test : terminé ou abandonné.
    pub fn est_finale(&self) -> bool {
        matches!(
            self,
            Entree::TestTermine { .. } | Entree::TestAbandonne { .. }
        )
    }
}

/// Un journal ouvert en ajout.
#[derive(Debug)]
pub struct Journal {
    chemin: PathBuf,
    fichier: File,
}

fn io(chemin: &Path, source: std::io::Error) -> ErreurDepot {
    ErreurDepot::Io {
        chemin: chemin.to_path_buf(),
        source,
    }
}

impl Journal {
    /// Ouvre (ou crée) le journal à `chemin`, prêt à ajouter des lignes.
    ///
    /// Si le fichier existe et que sa dernière ligne n'est pas terminée (arrêt brutal en
    /// pleine écriture), on la termine d'abord : la ligne cassée reste seule sur sa ligne
    /// au lieu d'avaler la prochaine entrée.
    pub fn ouvrir(chemin: impl AsRef<Path>) -> Result<Journal, ErreurDepot> {
        let chemin = chemin.as_ref().to_path_buf();
        let mut fichier = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&chemin)
            .map_err(|e| io(&chemin, e))?;
        let longueur = fichier.metadata().map_err(|e| io(&chemin, e))?.len();
        if longueur > 0 {
            let mut dernier = [0u8; 1];
            fichier
                .seek(SeekFrom::End(-1))
                .and_then(|_| fichier.read_exact(&mut dernier))
                .map_err(|e| io(&chemin, e))?;
            if dernier[0] != b'\n' {
                fichier
                    .write_all(b"\n")
                    .and_then(|_| fichier.sync_data())
                    .map_err(|e| io(&chemin, e))?;
            }
        }
        Ok(Journal { chemin, fichier })
    }

    pub fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// Ajoute une ligne et force son écriture sur le disque (`sync_data`) avant de rendre
    /// la main : une réponse donnée n'est jamais perdue par un plantage.
    pub fn ajouter(&mut self, entree: &Entree) -> Result<(), ErreurDepot> {
        let mut ligne = serde_json::to_vec(entree).map_err(|source| ErreurDepot::Json {
            chemin: self.chemin.clone(),
            source,
        })?;
        ligne.push(b'\n');
        // Un seul `write_all` : la ligne et son retour à la ligne partent ensemble.
        self.fichier
            .write_all(&ligne)
            .and_then(|_| self.fichier.sync_data())
            .map_err(|e| io(&self.chemin, e))
    }
}

/// Ce que la relecture d'un journal a trouvé.
#[derive(Debug, Clone, PartialEq)]
pub struct LectureJournal {
    /// Les entrées lisibles, dans l'ordre.
    pub entrees: Vec<Entree>,
    /// La dernière ligne était coupée (arrêt brutal pendant l'écriture) : elle est ignorée.
    pub derniere_ligne_tronquee: bool,
    /// Numéros (depuis 1) des lignes illisibles *avant* la dernière : un journal abîmé
    /// ailleurs qu'à la fin, ce qui ne devrait pas arriver.
    pub lignes_illisibles: Vec<usize>,
}

impl LectureJournal {
    /// Le test a-t-il une entrée finale (terminé ou abandonné) ?
    pub fn est_clos(&self) -> bool {
        self.entrees.iter().any(Entree::est_finale)
    }

    /// Lit un journal déjà chargé en mémoire.
    pub fn depuis_octets(contenu: &[u8]) -> LectureJournal {
        let mut lecture = LectureJournal {
            entrees: Vec::new(),
            derniere_ligne_tronquee: false,
            lignes_illisibles: Vec::new(),
        };
        let lignes: Vec<(usize, &[u8])> = contenu
            .split(|&o| o == b'\n')
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .filter(|(_, l)| !l.iter().all(u8::is_ascii_whitespace))
            .collect();
        let derniere = lignes.last().map(|(n, _)| *n);
        for (numero, ligne) in lignes {
            match serde_json::from_slice::<Entree>(ligne) {
                Ok(e) => lecture.entrees.push(e),
                Err(_) if Some(numero) == derniere => lecture.derniere_ligne_tronquee = true,
                Err(_) => lecture.lignes_illisibles.push(numero),
            }
        }
        lecture
    }

    /// Lit le journal du fichier `chemin`.
    pub fn lire(chemin: impl AsRef<Path>) -> Result<LectureJournal, ErreurDepot> {
        let chemin = chemin.as_ref();
        let contenu = std::fs::read(chemin).map_err(|e| io(chemin, e))?;
        Ok(Self::depuis_octets(&contenu))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Sujet;
    use tempfile::tempdir;

    fn duel() -> DuelPrevu {
        DuelPrevu {
            sujet: Sujet::Controle,
            reference_est_a: true,
            ordre_passages: [Cote::A, Cote::B, Cote::B, Cote::A, Cote::A, Cote::B],
        }
    }

    fn demarrage() -> Entree {
        Entree::TestDemarre {
            test_id: "t1".into(),
            profil_id: "p1".into(),
            version_reference: 2,
            courbe_reference: Courbe::default(),
            mode: Mode::DuelsEnJeu { precis: false },
            plan: vec![duel()],
            reglages: ReglagesTest::default(),
            variantes: ReglagesVariantes::default(),
            horodatage_ms: 1000,
        }
    }

    fn toutes_les_entrees() -> Vec<Entree> {
        vec![
            demarrage(),
            Entree::DuelDemarre {
                duel: 0,
                horodatage_ms: 1001,
            },
            Entree::PassageDemarre {
                duel: 0,
                passage: 0,
                cote: Cote::A,
                appli_premier_plan: Some("Jeu.exe".into()),
                horodatage_ms: 1002,
            },
            Entree::PassageDemarre {
                duel: 0,
                passage: 1,
                cote: Cote::B,
                appli_premier_plan: None,
                horodatage_ms: 1003,
            },
            Entree::ReponseDonnee {
                duel: 0,
                reponse: Reponse::Pareil,
                horodatage_ms: 1004,
            },
            Entree::ReponseAnnulee {
                duel: 0,
                horodatage_ms: 1005,
            },
            Entree::DuelPasse {
                duel: 1,
                horodatage_ms: 1006,
            },
            Entree::DuelRecommence {
                duel: 1,
                horodatage_ms: 1007,
            },
            Entree::Pause {
                raison: RaisonPause::Appel,
                horodatage_ms: 1008,
            },
            Entree::Reprise {
                horodatage_ms: 1009,
            },
            Entree::DuelVerificationAjoute {
                duel: duel(),
                horodatage_ms: 1010,
            },
            Entree::DuelRetire {
                duel: 1,
                prevu: duel(),
                horodatage_ms: 1011,
            },
            Entree::Pause {
                raison: RaisonPause::SansReponse,
                horodatage_ms: 1012,
            },
            Entree::TestAbandonne {
                horodatage_ms: 1013,
            },
        ]
    }

    #[test]
    fn chaque_entree_passe_par_json_en_une_ligne() {
        for e in toutes_les_entrees() {
            let j = serde_json::to_string(&e).unwrap();
            assert!(!j.contains('\n'));
            assert_eq!(serde_json::from_str::<Entree>(&j).unwrap(), e);
        }
    }

    #[test]
    fn l_entree_termine_porte_son_resultat() {
        let r = crate::resultat::calculer_resultat(
            &Courbe::default(),
            &crate::profil::Profil::nouveau(
                "p",
                "n",
                Courbe::default(),
                crate::profil::Origine::Manuel,
                "d",
            )
            .version_active()
            .clone(),
            &crate::zone::ReglagesVariantes::default(),
            &[],
            &[],
            None,
            &[],
        );
        let e = Entree::TestTermine {
            resultat: r,
            horodatage_ms: 5,
        };
        let j = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Entree>(&j).unwrap(), e);
        assert!(e.est_finale());
    }

    #[test]
    fn le_champ_type_nomme_la_variante() {
        let v = serde_json::to_value(Entree::Reprise { horodatage_ms: 3 }).unwrap();
        assert_eq!(v["type"], "reprise");
        let v = serde_json::to_value(demarrage()).unwrap();
        assert_eq!(v["type"], "test_demarre");
        assert_eq!(v["version_reference"], 2);
    }

    #[test]
    fn un_journal_sans_reglages_de_variantes_prend_les_valeurs_par_defaut() {
        let mut v = serde_json::to_value(demarrage()).unwrap();
        v.as_object_mut().unwrap().remove("variantes");
        let Entree::TestDemarre { variantes, .. } = serde_json::from_value(v).unwrap() else {
            panic!("mauvaise variante");
        };
        assert_eq!(variantes, ReglagesVariantes::default());
        let v = serde_json::to_value(Entree::DuelRetire {
            duel: 2,
            prevu: duel(),
            horodatage_ms: 4,
        })
        .unwrap();
        assert_eq!(v["type"], "duel_retire");
    }

    #[test]
    fn horodatage_et_entree_finale() {
        let entrees = toutes_les_entrees();
        for (i, e) in entrees.iter().enumerate() {
            assert_eq!(e.horodatage_ms(), 1000 + i as u64);
        }
        assert!(entrees.last().unwrap().est_finale());
        assert!(!entrees[0].est_finale());
    }

    #[test]
    fn ajouter_ecrit_une_ligne_par_entree_et_la_relecture_est_fidele() {
        let dossier = tempdir().unwrap();
        let chemin = dossier.path().join("t1.jsonl");
        let entrees = toutes_les_entrees();
        {
            let mut j = Journal::ouvrir(&chemin).unwrap();
            for e in &entrees {
                j.ajouter(e).unwrap();
            }
        }
        let texte = std::fs::read_to_string(&chemin).unwrap();
        assert_eq!(texte.lines().count(), entrees.len());
        assert!(texte.ends_with('\n'));
        let lecture = LectureJournal::lire(&chemin).unwrap();
        assert_eq!(lecture.entrees, entrees);
        assert!(!lecture.derniere_ligne_tronquee);
        assert!(lecture.lignes_illisibles.is_empty());
        assert!(lecture.est_clos());
    }

    #[test]
    fn rouvrir_un_journal_ajoute_a_la_suite() {
        let dossier = tempdir().unwrap();
        let chemin = dossier.path().join("t.jsonl");
        Journal::ouvrir(&chemin)
            .unwrap()
            .ajouter(&demarrage())
            .unwrap();
        let mut j = Journal::ouvrir(&chemin).unwrap();
        j.ajouter(&Entree::Reprise { horodatage_ms: 9 }).unwrap();
        assert_eq!(j.chemin(), chemin);
        assert_eq!(LectureJournal::lire(&chemin).unwrap().entrees.len(), 2);
    }

    #[test]
    fn une_derniere_ligne_coupee_est_ignoree_et_signalee() {
        let mut octets = Vec::new();
        for e in toutes_les_entrees().iter().take(3) {
            octets.extend(serde_json::to_vec(e).unwrap());
            octets.push(b'\n');
        }
        let coupee = serde_json::to_vec(&Entree::ReponseDonnee {
            duel: 0,
            reponse: Reponse::A,
            horodatage_ms: 7,
        })
        .unwrap();
        octets.extend(&coupee[..coupee.len() / 2]);
        let l = LectureJournal::depuis_octets(&octets);
        assert_eq!(l.entrees.len(), 3);
        assert!(l.derniere_ligne_tronquee);
        assert!(l.lignes_illisibles.is_empty());
    }

    #[test]
    fn une_coupure_au_milieu_d_un_caractere_utf8_est_toleree() {
        let e = Entree::PassageDemarre {
            duel: 0,
            passage: 0,
            cote: Cote::A,
            appli_premier_plan: Some("Éditeur.exe".into()),
            horodatage_ms: 1,
        };
        let json = serde_json::to_vec(&e).unwrap();
        let position = json.iter().position(|&o| o == 0xC3).unwrap();
        let l = LectureJournal::depuis_octets(&json[..=position]);
        assert!(l.entrees.is_empty());
        assert!(l.derniere_ligne_tronquee);
    }

    #[test]
    fn apres_un_arret_brutal_la_reouverture_isole_la_ligne_cassee() {
        let dossier = tempdir().unwrap();
        let chemin = dossier.path().join("t.jsonl");
        let ligne = serde_json::to_vec(&demarrage()).unwrap();
        let mut contenu = ligne.clone();
        contenu.push(b'\n');
        contenu.extend(&ligne[..10]); // coupée, sans retour à la ligne
        std::fs::write(&chemin, &contenu).unwrap();
        let mut j = Journal::ouvrir(&chemin).unwrap();
        j.ajouter(&Entree::Reprise { horodatage_ms: 2 }).unwrap();
        let l = LectureJournal::lire(&chemin).unwrap();
        assert_eq!(l.entrees.len(), 2);
        assert_eq!(l.lignes_illisibles, vec![2], "la ligne cassée est signalée");
        assert!(!l.derniere_ligne_tronquee);
    }

    #[test]
    fn les_lignes_vides_sont_ignorees_et_un_journal_vide_est_lisible() {
        let l = LectureJournal::depuis_octets(b"\n\n  \n");
        assert!(l.entrees.is_empty());
        assert!(!l.derniere_ligne_tronquee);
        assert!(!l.est_clos());
        let l = LectureJournal::depuis_octets(b"");
        assert!(l.entrees.is_empty());
    }

    #[test]
    fn lire_un_journal_absent_est_une_erreur_d_entree_sortie() {
        let dossier = tempdir().unwrap();
        let e = LectureJournal::lire(dossier.path().join("absent.jsonl")).unwrap_err();
        assert!(matches!(e, ErreurDepot::Io { .. }));
    }
}
