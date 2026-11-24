//! Le moteur Equalizer APO : on lui parle en écrivant ses fichiers texte,
//! qu'il recharge tout seul dès qu'ils changent.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use retro_dsp::Courbe;

use crate::texte::texte_config;
use crate::{ErreurMoteur, EtatMoteur, Moteur, SortieAudio};

const FICHIER_COURBE: &str = "retro-eq.txt";
const FICHIER_CONFIG: &str = "config.txt";
const FICHIER_SAUVEGARDE: &str = "config.txt.avant-retro-eq";
const LIGNE_INCLUDE: &str = "Include: retro-eq.txt";
/// Préfixe des lignes d'origine qu'on désactive ; il permet de les reconnaître.
const PREFIXE_COMMENTAIRE: &str = "# [retro-eq] ";
const BOM: &[u8] = b"\xEF\xBB\xBF";

fn io_err(action: impl Into<String>, source: io::Error) -> ErreurMoteur {
    ErreurMoteur::Io {
        action: action.into(),
        source,
    }
}

/// Écrit un fichier « d'un coup » : on écrit un fichier temporaire voisin puis
/// on le renomme par-dessus. Equalizer APO ne voit ainsi jamais un fichier à
/// moitié écrit (il recharge dès qu'il détecte un changement).
fn ecrire_atomique(chemin: &Path, octets: &[u8]) -> Result<(), ErreurMoteur> {
    let nom = chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporaire = chemin.with_file_name(format!("{nom}.tmp"));
    let resultat = (|| {
        let mut fichier = fs::File::create(&temporaire)?;
        io::Write::write_all(&mut fichier, octets)?;
        fichier.sync_all()?;
        drop(fichier);
        fs::rename(&temporaire, chemin)
    })();
    if let Err(e) = resultat {
        let _ = fs::remove_file(&temporaire);
        return Err(io_err(format!("écriture de {}", chemin.display()), e));
    }
    Ok(())
}

fn lire_si_present(chemin: &Path) -> Result<Option<Vec<u8>>, ErreurMoteur> {
    match fs::read(chemin) {
        Ok(octets) => Ok(Some(octets)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err(format!("lecture de {}", chemin.display()), e)),
    }
}

/// Ligne vide ou commentaire : Equalizer APO l'ignore.
fn est_inerte(ligne: &[u8]) -> bool {
    let t = ligne.trim_ascii();
    t.is_empty() || t.starts_with(b"#")
}

/// Est-ce notre ligne `Include: retro-eq.txt` ?
fn est_notre_include(ligne: &[u8]) -> bool {
    let t = ligne.trim_ascii();
    t.len() >= 8
        && t[..8].eq_ignore_ascii_case(b"include:")
        && t[8..]
            .trim_ascii()
            .eq_ignore_ascii_case(FICHIER_COURBE.as_bytes())
}

/// `config.txt` découpé en lignes, avec ce qu'il faut pour le réécrire à
/// l'identique côté mise en forme (BOM, CRLF ou LF, saut de ligne final).
/// On travaille sur des octets : le fichier n'est pas forcément de l'UTF-8.
struct ConfigTexte {
    bom: bool,
    fin_de_ligne: &'static [u8],
    lignes: Vec<Vec<u8>>,
    saut_final: bool,
}

impl ConfigTexte {
    fn analyser(octets: &[u8]) -> Self {
        let (bom, corps) = match octets.strip_prefix(BOM) {
            Some(reste) => (true, reste),
            None => (false, octets),
        };
        let crlf = corps.windows(2).any(|w| w == b"\r\n");
        let mut lignes: Vec<Vec<u8>> = if corps.is_empty() {
            Vec::new()
        } else {
            corps
                .split(|&b| b == b'\n')
                .map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec())
                .collect()
        };
        // « a\nb\n » donne un dernier morceau vide : c'est le saut final.
        let saut_final = lignes.last().is_some_and(|l| l.is_empty()) && corps.ends_with(b"\n");
        if saut_final {
            lignes.pop();
        }
        ConfigTexte {
            bom,
            fin_de_ligne: if crlf { b"\r\n" } else { b"\n" },
            lignes,
            saut_final,
        }
    }

    fn rendre(&self) -> Vec<u8> {
        let mut sortie = Vec::new();
        if self.bom {
            sortie.extend_from_slice(BOM);
        }
        for (i, ligne) in self.lignes.iter().enumerate() {
            if i > 0 {
                sortie.extend_from_slice(self.fin_de_ligne);
            }
            sortie.extend_from_slice(ligne);
        }
        if self.saut_final {
            sortie.extend_from_slice(self.fin_de_ligne);
        }
        sortie
    }

    /// Commente les lignes actives sauf notre Include, qu'on ajoute s'il manque.
    fn brancher(&mut self) {
        let mut trouve = false;
        for ligne in &mut self.lignes {
            if est_inerte(ligne) {
                continue;
            }
            if est_notre_include(ligne) && !trouve {
                trouve = true;
                continue;
            }
            let mut commentee = PREFIXE_COMMENTAIRE.as_bytes().to_vec();
            commentee.extend_from_slice(ligne);
            *ligne = commentee;
        }
        if !trouve {
            self.lignes.push(LIGNE_INCLUDE.as_bytes().to_vec());
            self.saut_final = true;
        }
    }

    /// Chemin inverse (sans sauvegarde sous la main) : retire notre Include et
    /// réactive les lignes que nous avions commentées.
    fn debrancher(&mut self) {
        self.lignes.retain(|l| !est_notre_include(l));
        for ligne in &mut self.lignes {
            if let Some(origine) = ligne.strip_prefix(PREFIXE_COMMENTAIRE.as_bytes()) {
                *ligne = origine.to_vec();
            }
        }
    }

    fn est_vide(&self) -> bool {
        self.lignes.iter().all(|l| l.trim_ascii().is_empty())
    }
}

/// Courbes intermédiaires entre `avant` et `apres`, `n` au total, sans les
/// extrémités. Vide si les deux courbes n'ont pas la même disposition de
/// filtres (même nombre, mêmes types, fréquences et Q) : on ne sait alors pas
/// quoi interpoler, on écrira directement la nouvelle.
fn paliers(avant: &Courbe, apres: &Courbe, n: u32) -> Vec<Courbe> {
    let meme_disposition = avant.filtres.len() == apres.filtres.len()
        && avant.filtres.iter().zip(&apres.filtres).all(|(a, b)| {
            a.type_filtre == b.type_filtre && a.frequence_hz == b.frequence_hz && a.q == b.q
        });
    if !meme_disposition {
        return Vec::new();
    }
    (1..=n)
        .map(|k| {
            let t = f64::from(k) / f64::from(n + 1);
            let melange = |a: f64, b: f64| a + (b - a) * t;
            Courbe {
                preampli_db: melange(avant.preampli_db, apres.preampli_db),
                filtres: avant
                    .filtres
                    .iter()
                    .zip(&apres.filtres)
                    .map(|(a, b)| retro_dsp::Filtre {
                        gain_db: melange(a.gain_db, b.gain_db),
                        ..*a
                    })
                    .collect(),
            }
        })
        .collect()
}

fn verifier(courbe: &Courbe) -> Result<(), ErreurMoteur> {
    if !courbe.preampli_db.is_finite() {
        return Err(ErreurMoteur::CourbeInvalide("préampli non fini".into()));
    }
    for (i, f) in courbe.filtres.iter().enumerate() {
        let ok = f.gain_db.is_finite()
            && f.frequence_hz.is_finite()
            && f.frequence_hz > 0.0
            && f.q.is_finite()
            && f.q > 0.0;
        if !ok {
            return Err(ErreurMoteur::CourbeInvalide(format!(
                "filtre {} : fréquence, gain ou Q inutilisable",
                i + 1
            )));
        }
    }
    Ok(())
}

/// Pilote Equalizer APO en écrivant dans son dossier `config`.
pub struct EqualizerApo {
    dossier: PathBuf,
    paliers_fondu: u32,
    pause_fondu: Duration,
    dormir: Box<dyn FnMut(Duration) + Send>,
    derniere: Option<Courbe>,
}

impl EqualizerApo {
    /// `dossier_config` : le dossier `config` d'Equalizer APO (voir [`Self::detecter`]).
    /// Rien n'est lu ni écrit tant qu'on n'appelle pas `appliquer`.
    pub fn nouveau(dossier_config: PathBuf) -> Self {
        EqualizerApo {
            dossier: dossier_config,
            paliers_fondu: 0,
            pause_fondu: Duration::from_millis(20),
            dormir: Box::new(thread::sleep),
            derniere: None,
        }
    }

    /// Active le fondu : `paliers` courbes intermédiaires entre deux courbes,
    /// séparées de `pause`. 0 (défaut) = écriture directe. À régler après
    /// mesure sur un vrai PC.
    pub fn avec_fondu(mut self, paliers: u32, pause: Duration) -> Self {
        self.paliers_fondu = paliers;
        self.pause_fondu = pause;
        self
    }

    /// Remplace la fonction d'attente (les tests y mettent un faux sommeil).
    pub fn avec_sommeil(mut self, dormir: impl FnMut(Duration) + Send + 'static) -> Self {
        self.dormir = Box::new(dormir);
        self
    }

    /// Cherche le dossier `config` d'Equalizer APO.
    ///
    /// Sous Windows : valeur `InstallPath` de `HKLM\SOFTWARE\EqualizerAPO`,
    /// sinon `C:\Program Files\EqualizerAPO`. Ailleurs : `None`.
    pub fn detecter() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            let installation = crate::registre::dossier_installation()
                .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\EqualizerAPO"));
            let config = installation.join("config");
            config.is_dir().then_some(config)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    fn chemin(&self, nom: &str) -> PathBuf {
        self.dossier.join(nom)
    }

    fn exiger_dossier(&self) -> Result<(), ErreurMoteur> {
        if self.dossier.is_dir() {
            Ok(())
        } else {
            Err(ErreurMoteur::DossierIntrouvable(self.dossier.clone()))
        }
    }

    fn ecrire_courbe(&self, courbe: &Courbe) -> Result<(), ErreurMoteur> {
        ecrire_atomique(
            &self.chemin(FICHIER_COURBE),
            texte_config(courbe).as_bytes(),
        )
    }

    /// Fait en sorte que `config.txt` inclue notre fichier. Sauvegarde
    /// l'original avant la toute première modification.
    fn brancher_config(&self) -> Result<(), ErreurMoteur> {
        let chemin = self.chemin(FICHIER_CONFIG);
        let original = lire_si_present(&chemin)?;
        let mut config = ConfigTexte::analyser(original.as_deref().unwrap_or(&[]));
        config.brancher();
        let nouveau = config.rendre();
        if original.as_deref() == Some(nouveau.as_slice()) {
            return Ok(()); // déjà branché : on ne touche à rien
        }
        if let Some(octets) = &original {
            let sauvegarde = self.chemin(FICHIER_SAUVEGARDE);
            if !sauvegarde.exists() {
                ecrire_atomique(&sauvegarde, octets)?;
            }
        }
        ecrire_atomique(&chemin, &nouveau)
    }
}

impl Moteur for EqualizerApo {
    fn appliquer(&mut self, courbe: &Courbe) -> Result<(), ErreurMoteur> {
        self.exiger_dossier()?;
        verifier(courbe)?;
        let intermediaires = match &self.derniere {
            Some(avant) if self.paliers_fondu > 0 => paliers(avant, courbe, self.paliers_fondu),
            _ => Vec::new(),
        };
        let mut premiere_ecriture = true;
        for etape in intermediaires.iter().chain(std::iter::once(courbe)) {
            if !premiere_ecriture {
                (self.dormir)(self.pause_fondu);
            }
            self.ecrire_courbe(etape)?;
            if premiere_ecriture {
                // Après la 1re écriture seulement : l'Include ne pointe jamais
                // vers un fichier absent.
                self.brancher_config()?;
                premiere_ecriture = false;
            }
            self.derniere = Some(etape.clone());
        }
        Ok(())
    }

    fn couper(&mut self) -> Result<(), ErreurMoteur> {
        self.appliquer(&Courbe::default())
    }

    fn etat(&self) -> EtatMoteur {
        if !self.dossier.is_dir() {
            return EtatMoteur::Absent;
        }
        match fs::read(self.chemin(FICHIER_CONFIG)) {
            Ok(octets) => {
                let config = ConfigTexte::analyser(&octets);
                if config
                    .lignes
                    .iter()
                    .any(|l| !est_inerte(l) && est_notre_include(l))
                {
                    EtatMoteur::Actif
                } else {
                    EtatMoteur::Inactif
                }
            }
            Err(_) => EtatMoteur::Inactif,
        }
    }

    fn sorties(&self) -> Result<Vec<SortieAudio>, ErreurMoteur> {
        #[cfg(windows)]
        {
            crate::registre::sorties_installees()
        }
        #[cfg(not(windows))]
        {
            Ok(Vec::new())
        }
    }

    /// Rend `config.txt` tel qu'il était, à l'octet près, depuis la sauvegarde,
    /// puis supprime la sauvegarde et `retro-eq.txt`.
    ///
    /// Sans sauvegarde (le fichier n'existait pas avant nous, ou il a été
    /// supprimé), on défait nos modifications ligne à ligne : notre Include
    /// disparaît, les lignes préfixées `# [retro-eq] ` sont réactivées, et si
    /// plus rien d'utile ne reste, `config.txt` est supprimé car c'est nous
    /// qui l'avions créé. Si le dossier n'existe plus (Equalizer APO
    /// désinstallé), il n'y a rien à rendre : succès.
    fn retirer(&mut self) -> Result<(), ErreurMoteur> {
        self.derniere = None;
        if !self.dossier.is_dir() {
            return Ok(());
        }
        let config = self.chemin(FICHIER_CONFIG);
        let sauvegarde = self.chemin(FICHIER_SAUVEGARDE);
        match lire_si_present(&sauvegarde)? {
            Some(octets) => {
                ecrire_atomique(&config, &octets)?;
                supprimer_si_present(&sauvegarde)?;
            }
            None => {
                if let Some(octets) = lire_si_present(&config)? {
                    let mut texte = ConfigTexte::analyser(&octets);
                    texte.debrancher();
                    if texte.est_vide() {
                        supprimer_si_present(&config)?;
                    } else {
                        ecrire_atomique(&config, &texte.rendre())?;
                    }
                }
            }
        }
        supprimer_si_present(&self.chemin(FICHIER_COURBE))
    }
}

fn supprimer_si_present(chemin: &Path) -> Result<(), ErreurMoteur> {
    match fs::remove_file(chemin) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(format!("suppression de {}", chemin.display()), e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_dsp::{Filtre, TypeFiltre};
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    fn courbe_test() -> Courbe {
        Courbe {
            preampli_db: -3.0,
            filtres: vec![Filtre {
                type_filtre: TypeFiltre::Cloche,
                frequence_hz: 1000.0,
                gain_db: 3.0,
                q: 1.41,
            }],
        }
    }

    fn moteur(dir: &TempDir) -> EqualizerApo {
        EqualizerApo::nouveau(dir.path().to_path_buf())
    }

    fn lire(dir: &TempDir, nom: &str) -> Vec<u8> {
        fs::read(dir.path().join(nom)).unwrap()
    }

    #[test]
    fn dossier_absent_donne_erreur_claire_et_etat_absent() {
        let dir = TempDir::new().unwrap();
        let mut m = EqualizerApo::nouveau(dir.path().join("nexiste-pas"));
        assert_eq!(m.etat(), EtatMoteur::Absent);
        let err = m.appliquer(&courbe_test()).unwrap_err();
        assert!(matches!(err, ErreurMoteur::DossierIntrouvable(_)));
        assert!(err.to_string().contains("introuvable"));
        assert!(m.couper().is_err());
    }

    #[test]
    fn premier_appliquer_sauvegarde_commente_et_ajoute_include() {
        let dir = TempDir::new().unwrap();
        let original = b"# mon fichier\nPreamp: -2 dB\n\nFilter: ON PK Fc 100 Hz Gain 5 dB Q 1\n";
        fs::write(dir.path().join("config.txt"), original).unwrap();
        let mut m = moteur(&dir);
        assert_eq!(m.etat(), EtatMoteur::Inactif);
        m.appliquer(&courbe_test()).unwrap();

        assert_eq!(lire(&dir, "config.txt.avant-retro-eq"), original);
        let config = String::from_utf8(lire(&dir, "config.txt")).unwrap();
        assert_eq!(
            config,
            "# mon fichier\n# [retro-eq] Preamp: -2 dB\n\n# [retro-eq] Filter: ON PK Fc 100 Hz Gain 5 dB Q 1\nInclude: retro-eq.txt\n"
        );
        assert_eq!(
            String::from_utf8(lire(&dir, "retro-eq.txt")).unwrap(),
            texte_config(&courbe_test())
        );
        assert_eq!(m.etat(), EtatMoteur::Actif);
    }

    #[test]
    fn appliquer_plusieurs_fois_est_idempotent_sur_config() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "Preamp: -2 dB\n").unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        let apres_un = lire(&dir, "config.txt");
        let sauvegarde = lire(&dir, "config.txt.avant-retro-eq");
        m.appliquer(&Courbe::default()).unwrap();
        m.couper().unwrap();
        m.appliquer(&courbe_test()).unwrap();
        assert_eq!(lire(&dir, "config.txt"), apres_un);
        assert_eq!(lire(&dir, "config.txt.avant-retro-eq"), sauvegarde);
        assert_eq!(sauvegarde, b"Preamp: -2 dB\n");
    }

    #[test]
    fn la_sauvegarde_n_est_jamais_ecrasee() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "ancien\n").unwrap();
        fs::write(dir.path().join("config.txt.avant-retro-eq"), "deja la\n").unwrap();
        moteur(&dir).appliquer(&courbe_test()).unwrap();
        assert_eq!(lire(&dir, "config.txt.avant-retro-eq"), b"deja la\n");
    }

    #[test]
    fn couper_ecrit_une_courbe_plate() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "").unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        m.couper().unwrap();
        let texte = String::from_utf8(lire(&dir, "retro-eq.txt")).unwrap();
        assert!(texte.contains("Preamp: 0.0 dB"));
        assert!(!texte.contains("Filter"));
        assert_eq!(m.etat(), EtatMoteur::Actif);
    }

    fn aller_retour(original: &[u8]) {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), original).unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        assert_ne!(lire(&dir, "config.txt"), original);
        m.retirer().unwrap();
        assert_eq!(lire(&dir, "config.txt"), original);
        assert!(!dir.path().join("config.txt.avant-retro-eq").exists());
        assert!(!dir.path().join("retro-eq.txt").exists());
        assert_eq!(m.etat(), EtatMoteur::Inactif);
    }

    #[test]
    fn retirer_restaure_les_octets_lf() {
        aller_retour(b"Preamp: -1 dB\nFilter: ON PK Fc 100 Hz Gain 5 dB Q 1\n");
    }

    #[test]
    fn retirer_restaure_les_octets_crlf_sans_saut_final() {
        aller_retour(b"# c\r\nPreamp: -1 dB\r\nDevice: all");
    }

    #[test]
    fn retirer_restaure_les_octets_avec_bom() {
        aller_retour(b"\xEF\xBB\xBFPreamp: -1 dB\r\n");
    }

    #[test]
    fn retirer_restaure_un_fichier_vide() {
        aller_retour(b"");
    }

    #[test]
    fn crlf_et_bom_sont_conserves_a_l_edition() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("config.txt"),
            b"\xEF\xBB\xBFPreamp: -1 dB\r\n",
        )
        .unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        assert_eq!(
            lire(&dir, "config.txt"),
            b"\xEF\xBB\xBF# [retro-eq] Preamp: -1 dB\r\nInclude: retro-eq.txt\r\n"
        );
        assert_eq!(m.etat(), EtatMoteur::Actif);
    }

    #[test]
    fn sans_config_d_origine_on_en_cree_un_puis_on_le_supprime() {
        let dir = TempDir::new().unwrap();
        let mut m = moteur(&dir);
        assert_eq!(m.etat(), EtatMoteur::Inactif);
        m.appliquer(&courbe_test()).unwrap();
        assert_eq!(lire(&dir, "config.txt"), b"Include: retro-eq.txt\n");
        assert!(!dir.path().join("config.txt.avant-retro-eq").exists());
        m.retirer().unwrap();
        assert!(!dir.path().join("config.txt").exists());
        assert!(!dir.path().join("retro-eq.txt").exists());
    }

    #[test]
    fn retirer_sans_sauvegarde_reactive_les_lignes_commentees() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("config.txt"),
            "# [retro-eq] Preamp: -1 dB\nInclude: retro-eq.txt\n",
        )
        .unwrap();
        let mut m = moteur(&dir);
        m.retirer().unwrap();
        assert_eq!(lire(&dir, "config.txt"), b"Preamp: -1 dB\n");
    }

    #[test]
    fn retirer_sur_dossier_absent_reussit() {
        let dir = TempDir::new().unwrap();
        let mut m = EqualizerApo::nouveau(dir.path().join("rien"));
        assert!(m.retirer().is_ok());
    }

    #[test]
    fn etat_inactif_si_l_include_est_commente() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "# Include: retro-eq.txt\n").unwrap();
        assert_eq!(moteur(&dir).etat(), EtatMoteur::Inactif);
    }

    #[test]
    fn l_ecriture_atomique_ne_laisse_aucun_fichier_temporaire() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "Preamp: 0 dB\n").unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        m.appliquer(&Courbe::default()).unwrap();
        m.retirer().unwrap();
        let restes: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(restes, vec!["config.txt".to_string()]);
    }

    #[test]
    fn courbe_invalide_est_refusee_sans_rien_ecrire() {
        let dir = TempDir::new().unwrap();
        let mut c = courbe_test();
        c.filtres[0].gain_db = f64::NAN;
        let mut m = moteur(&dir);
        assert!(matches!(
            m.appliquer(&c),
            Err(ErreurMoteur::CourbeInvalide(_))
        ));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    // --- fondu ---

    fn courbe_gain(preampli: f64, gain: f64) -> Courbe {
        let mut c = courbe_test();
        c.preampli_db = preampli;
        c.filtres[0].gain_db = gain;
        c
    }

    #[test]
    fn paliers_interpolent_lineairement() {
        let p = paliers(&courbe_gain(0.0, 0.0), &courbe_gain(-6.0, 6.0), 2);
        assert_eq!(p, vec![courbe_gain(-2.0, 2.0), courbe_gain(-4.0, 4.0)]);
    }

    #[test]
    fn paliers_vides_si_disposition_differente() {
        assert!(paliers(&Courbe::default(), &courbe_test(), 3).is_empty());
    }

    #[test]
    fn fondu_ecrit_la_suite_attendue_avec_pauses_injectees() {
        let dir = TempDir::new().unwrap();
        let pauses = Arc::new(Mutex::new(Vec::new()));
        let pauses2 = pauses.clone();
        let mut m = moteur(&dir)
            .avec_fondu(2, Duration::from_millis(7))
            .avec_sommeil(move |d| pauses2.lock().unwrap().push(d));
        // Première courbe : rien avant, écriture directe, aucune pause.
        m.appliquer(&courbe_gain(0.0, 0.0)).unwrap();
        assert!(pauses.lock().unwrap().is_empty());
        // Suivante : 2 paliers puis la cible, donc 2 pauses.
        m.appliquer(&courbe_gain(-6.0, 6.0)).unwrap();
        assert_eq!(*pauses.lock().unwrap(), vec![Duration::from_millis(7); 2]);
        assert_eq!(
            String::from_utf8(lire(&dir, "retro-eq.txt")).unwrap(),
            texte_config(&courbe_gain(-6.0, 6.0))
        );
    }

    #[test]
    fn fondu_passe_bien_par_les_courbes_intermediaires() {
        // On observe chaque écriture en lisant le fichier dans le faux sommeil.
        let dir = TempDir::new().unwrap();
        let chemin = dir.path().join("retro-eq.txt");
        let vues = Arc::new(Mutex::new(Vec::new()));
        let vues2 = vues.clone();
        let mut m = moteur(&dir)
            .avec_fondu(2, Duration::ZERO)
            .avec_sommeil(move |_| {
                vues2
                    .lock()
                    .unwrap()
                    .push(fs::read_to_string(&chemin).unwrap());
            });
        m.appliquer(&courbe_gain(0.0, 0.0)).unwrap();
        m.appliquer(&courbe_gain(-6.0, 6.0)).unwrap();
        assert_eq!(
            *vues.lock().unwrap(),
            vec![
                texte_config(&courbe_gain(-2.0, 2.0)),
                texte_config(&courbe_gain(-4.0, 4.0)),
            ]
        );
    }

    #[test]
    fn sans_fondu_une_seule_ecriture_et_aucune_pause() {
        let dir = TempDir::new().unwrap();
        let pauses = Arc::new(Mutex::new(0));
        let p2 = pauses.clone();
        let mut m = moteur(&dir).avec_sommeil(move |_| *p2.lock().unwrap() += 1);
        m.appliquer(&courbe_gain(0.0, 0.0)).unwrap();
        m.appliquer(&courbe_gain(-6.0, 6.0)).unwrap();
        assert_eq!(*pauses.lock().unwrap(), 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn hors_windows_pas_de_detection_ni_de_sorties() {
        assert_eq!(EqualizerApo::detecter(), None);
        let dir = TempDir::new().unwrap();
        assert!(moteur(&dir).sorties().unwrap().is_empty());
    }
}
