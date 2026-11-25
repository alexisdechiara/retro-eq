//! Le moteur Equalizer APO : on lui parle en écrivant ses fichiers texte,
//! qu'il recharge tout seul dès qu'ils changent.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use retro_dsp::Courbe;

use crate::texte::texte_config;
use crate::{ErreurMoteur, EtatMoteur, Moteur, SortieAudio};

const FICHIER_COURBE: &str = "retro-eq.txt";
const FICHIER_CONFIG: &str = "config.txt";
const FICHIER_SAUVEGARDE: &str = "config.txt.avant-retro-eq";
/// Notre ligne `Include`, telle qu'on l'écrit (et seule forme qu'on retire).
const LIGNE_INCLUDE: &[u8] = b"Include: retro-eq.txt";
/// Préfixe des lignes d'origine qu'on désactive ; il permet de les reconnaître.
const PREFIXE_COMMENTAIRE: &[u8] = b"# [retro-eq] ";
const BOM: &[u8] = b"\xEF\xBB\xBF";

fn io_err(action: impl Into<String>, source: io::Error) -> ErreurMoteur {
    ErreurMoteur::Io {
        action: action.into(),
        source,
    }
}

/// Un nom de fichier temporaire unique, dans le même dossier que `chemin`
/// (le renommage final doit rester sur le même disque pour être atomique).
///
/// Numéro de processus + compteur : deux processus, ou deux écritures du même
/// processus, ne se marchent jamais dessus.
fn nom_temporaire(chemin: &Path) -> PathBuf {
    static COMPTEUR: AtomicU64 = AtomicU64::new(0);
    let nom = chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let numero = COMPTEUR.fetch_add(1, Ordering::Relaxed);
    chemin.with_file_name(format!("{nom}.{}.{numero}.tmp", std::process::id()))
}

/// Écrit un fichier « d'un coup » : on écrit un fichier temporaire voisin puis
/// on le renomme par-dessus. Equalizer APO ne voit ainsi jamais un fichier à
/// moitié écrit (il recharge dès qu'il détecte un changement).
fn ecrire_atomique(chemin: &Path, octets: &[u8]) -> Result<(), ErreurMoteur> {
    let temporaire = nom_temporaire(chemin);
    let resultat = (|| {
        // `create_new` : on refuse d'ouvrir un fichier qui existe déjà.
        let mut fichier = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporaire)?;
        io::Write::write_all(&mut fichier, octets)?;
        fichier.sync_all()?;
        drop(fichier);
        fs::rename(&temporaire, chemin)
    })();
    if let Err(e) = resultat {
        // Si le fichier n'a pas pu être créé, il n'est pas à nous : on ne le
        // supprime que s'il existe parce que *nous* l'avons créé (le nom est unique).
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

/// Est-ce un `Include: retro-eq.txt` actif, écrit comme l'utilisateur veut
/// (casse, espaces) ? Sert à dire si le moteur est branché, pas à défaire.
fn ressemble_a_notre_include(ligne: &[u8]) -> bool {
    let t = ligne.trim_ascii();
    t.len() >= 8
        && t[..8].eq_ignore_ascii_case(b"include:")
        && t[8..]
            .trim_ascii()
            .eq_ignore_ascii_case(FICHIER_COURBE.as_bytes())
}

/// Une ligne du fichier : son contenu et sa fin (`""`, `"\n"` ou `"\r\n"`).
///
/// On garde la fin de chaque ligne telle quelle, au lieu de deviner « le » style
/// du fichier : c'est ce qui rend l'annulation exacte même avec des fins de
/// ligne mélangées.
struct Ligne<'a> {
    contenu: &'a [u8],
    fin: &'static [u8],
}

/// Découpe en lignes sur des octets (le fichier n'est pas forcément de l'UTF-8).
/// Seule la dernière ligne peut ne pas avoir de fin ; un texte vide n'a aucune ligne.
fn decouper(mut reste: &[u8]) -> Vec<Ligne<'_>> {
    let mut lignes = Vec::new();
    while !reste.is_empty() {
        match reste.iter().position(|&b| b == b'\n') {
            Some(i) => {
                let (avant, suite) = (&reste[..i], &reste[i + 1..]);
                lignes.push(match avant.strip_suffix(b"\r") {
                    Some(contenu) => Ligne {
                        contenu,
                        fin: b"\r\n",
                    },
                    None => Ligne {
                        contenu: avant,
                        fin: b"\n",
                    },
                });
                reste = suite;
            }
            None => {
                lignes.push(Ligne {
                    contenu: reste,
                    fin: b"",
                });
                break;
            }
        }
    }
    lignes
}

fn sans_bom(octets: &[u8]) -> (bool, &[u8]) {
    match octets.strip_prefix(BOM) {
        Some(reste) => (true, reste),
        None => (false, octets),
    }
}

/// Faut-il commenter cette ligne ? Les lignes actives, bien sûr ; et aussi celles
/// qui portent déjà notre préfixe : sans cela, l'annulation leur retirerait un
/// préfixe qui n'est pas de nous et le fichier serait abîmé.
fn a_commenter(contenu: &[u8]) -> bool {
    !est_inerte(contenu) || contenu.starts_with(PREFIXE_COMMENTAIRE)
}

/// Ce que devient `config.txt` pour qu'Equalizer APO n'utilise que notre courbe :
/// les lignes actives sont mises en commentaire et notre `Include` est ajouté
/// à la fin. `None` = le fichier n'existe pas.
///
/// Cette transformation est **exactement réversible** (voir [`debrancher`]),
/// pour n'importe quels octets :
/// - chaque ligne garde sa propre fin, le BOM reste au début ;
/// - sans saut de ligne final, on en ajoute un avant notre `Include` et on
///   laisse ce dernier sans fin : c'est ce qui dit à l'annulation qu'il faut
///   aussi retirer ce saut de ligne ;
/// - un fichier absent donne `Include…\n`, un fichier vide `Include…` sans fin.
fn brancher(original: Option<&[u8]>) -> Vec<u8> {
    let Some(original) = original else {
        let mut sortie = LIGNE_INCLUDE.to_vec();
        sortie.push(b'\n');
        return sortie;
    };
    let (bom, corps) = sans_bom(original);
    let lignes = decouper(corps);
    let mut sortie = Vec::with_capacity(original.len() + 64);
    if bom {
        sortie.extend_from_slice(BOM);
    }
    for ligne in &lignes {
        if a_commenter(ligne.contenu) {
            sortie.extend_from_slice(PREFIXE_COMMENTAIRE);
        }
        sortie.extend_from_slice(ligne.contenu);
        sortie.extend_from_slice(ligne.fin);
    }
    // Fin de ligne à ajouter : celle de la dernière ligne terminée, sinon LF.
    let fin = lignes
        .iter()
        .rev()
        .map(|l| l.fin)
        .find(|f| !f.is_empty())
        .unwrap_or(b"\n");
    match lignes.last() {
        None => sortie.extend_from_slice(LIGNE_INCLUDE),
        Some(derniere) if derniere.fin.is_empty() => {
            // Un `\r` en fin de contenu serait relu comme la moitié d'un `\r\n` :
            // dans ce cas on écrit `\r\n` pour que le contenu garde son `\r`.
            let a_un_cr_final = derniere.contenu.ends_with(b"\r");
            sortie.extend_from_slice(if a_un_cr_final { b"\r\n" } else { fin });
            sortie.extend_from_slice(LIGNE_INCLUDE);
        }
        Some(_) => {
            sortie.extend_from_slice(LIGNE_INCLUDE);
            sortie.extend_from_slice(fin);
        }
    }
    sortie
}

/// Défait exactement ce que fait [`brancher`] : retire notre `Include` (et le
/// saut de ligne qu'on avait ajouté avant lui) et réactive les lignes qui
/// portent notre préfixe. Tout le reste, octet pour octet, est rendu tel quel.
///
/// Rend `None` quand le fichier n'est que notre `Include` : il n'existait pas
/// avant nous, c'est à l'appelant de le supprimer.
///
/// Sur un fichier qui a bougé depuis (lignes ajoutées par l'utilisateur), elle
/// défait seulement nos marques et laisse le reste : on rend ce que
/// l'utilisateur aurait sans Retro EQ.
fn debrancher(courant: &[u8]) -> Option<Vec<u8>> {
    let (bom, corps) = sans_bom(courant);
    let mut lignes = decouper(corps);
    let est_notre_include = |l: &Ligne| l.contenu == LIGNE_INCLUDE;
    if !bom
        && !lignes.is_empty()
        && lignes
            .iter()
            .all(|l| est_notre_include(l) && !l.fin.is_empty())
    {
        return None;
    }
    let include_final_sans_fin = lignes
        .last()
        .is_some_and(|l| est_notre_include(l) && l.fin.is_empty());
    lignes.retain(|l| !est_notre_include(l));
    if include_final_sans_fin {
        if let Some(derniere) = lignes.last_mut() {
            derniere.fin = b"";
        }
    }
    let mut sortie = Vec::with_capacity(courant.len());
    if bom {
        sortie.extend_from_slice(BOM);
    }
    for ligne in &lignes {
        let contenu = ligne
            .contenu
            .strip_prefix(PREFIXE_COMMENTAIRE)
            .unwrap_or(ligne.contenu);
        sortie.extend_from_slice(contenu);
        sortie.extend_from_slice(ligne.fin);
    }
    Some(sortie)
}

/// Notre `Include` exact est-il dans ce fichier ? Si oui, le fichier a déjà été
/// branché par nous (une copie à l'identique de notre ligne n'a aucune autre raison d'être).
fn contient_notre_include(octets: &[u8]) -> bool {
    let (_, corps) = sans_bom(octets);
    decouper(corps).iter().any(|l| l.contenu == LIGNE_INCLUDE)
}

/// Remet à neuf un fichier déjà branché mais qui a bougé : on défait nos marques
/// puis on les refait, pour que les lignes ajoutées depuis soient commentées
/// elles aussi, sans empiler de préfixes. Sur un fichier propre, rend les mêmes octets.
fn rebrancher(courant: &[u8]) -> Vec<u8> {
    match debrancher(courant) {
        Some(propre) => brancher(Some(&propre)),
        None => brancher(None),
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
    copie_conservee: Option<PathBuf>,
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
            copie_conservee: None,
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

    /// Fait en sorte que `config.txt` inclue notre fichier. Copie l'original
    /// avant la toute première modification (jamais écrasée ensuite).
    fn brancher_config(&self) -> Result<(), ErreurMoteur> {
        let chemin = self.chemin(FICHIER_CONFIG);
        let courant = lire_si_present(&chemin)?;
        let deja_branche = courant.as_deref().is_some_and(contient_notre_include);
        let nouveau = match &courant {
            Some(octets) if deja_branche => rebrancher(octets),
            autre => brancher(autre.as_deref()),
        };
        if courant.as_deref() == Some(nouveau.as_slice()) {
            return Ok(()); // déjà branché : on ne touche à rien
        }
        if let (Some(octets), false) = (&courant, deja_branche) {
            let sauvegarde = self.chemin(FICHIER_SAUVEGARDE);
            if !sauvegarde.exists() {
                ecrire_atomique(&sauvegarde, octets)?;
            }
        }
        ecrire_atomique(&chemin, &nouveau)
    }

    /// Un nom libre pour garder une ancienne copie : `config.txt.avant-retro-eq.1`, `.2`…
    fn nom_copie_conservee(&self) -> PathBuf {
        let mut numero = 1u32;
        loop {
            let candidat = self.chemin(&format!("{FICHIER_SAUVEGARDE}.{numero}"));
            if !candidat.exists() {
                return candidat;
            }
            numero += 1;
        }
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
                let (_, corps) = sans_bom(&octets);
                if decouper(corps)
                    .iter()
                    .any(|l| !est_inerte(l.contenu) && ressemble_a_notre_include(l.contenu))
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

    /// Rend à l'utilisateur ce qu'il aurait sans Retro EQ : on défait exactement
    /// nos modifications dans le `config.txt` **actuel** (notre `Include`
    /// disparaît, les lignes préfixées `# [retro-eq] ` sont réactivées, octet
    /// pour octet), puis on supprime `retro-eq.txt`.
    ///
    /// On ne restaure pas l'ancienne sauvegarde par-dessus : si le fichier a été
    /// remplacé entre-temps (autre outil, réinstallation), elle écraserait la
    /// config actuelle. La sauvegarde `config.txt.avant-retro-eq` ne sert que de
    /// filet : identique au résultat, elle est supprimée ; différente, elle est
    /// gardée sous `config.txt.avant-retro-eq.N` et signalée par
    /// [`Moteur::copie_de_securite`].
    ///
    /// Si `config.txt` n'était que notre `Include` (il n'existait pas avant
    /// nous), il est supprimé. Si le dossier n'existe plus (Equalizer APO
    /// désinstallé), il n'y a rien à rendre : succès.
    fn retirer(&mut self) -> Result<(), ErreurMoteur> {
        self.derniere = None;
        self.copie_conservee = None;
        if !self.dossier.is_dir() {
            return Ok(());
        }
        let config = self.chemin(FICHIER_CONFIG);
        let sauvegarde = self.chemin(FICHIER_SAUVEGARDE);
        let courant = lire_si_present(&config)?;
        let rendu: Option<Vec<u8>> = match courant {
            None => None,
            Some(octets) => match debrancher(&octets) {
                None => {
                    supprimer_si_present(&config)?;
                    None
                }
                Some(rendu) => {
                    if rendu != octets {
                        ecrire_atomique(&config, &rendu)?;
                    }
                    Some(rendu)
                }
            },
        };
        if let Some(copie) = lire_si_present(&sauvegarde)? {
            if rendu.as_deref() == Some(copie.as_slice()) {
                supprimer_si_present(&sauvegarde)?;
            } else {
                let destination = self.nom_copie_conservee();
                fs::rename(&sauvegarde, &destination).map_err(|e| {
                    io_err(
                        format!("conservation de l'ancienne copie {}", sauvegarde.display()),
                        e,
                    )
                })?;
                self.copie_conservee = Some(destination);
            }
        }
        supprimer_si_present(&self.chemin(FICHIER_COURBE))
    }

    fn copie_de_securite(&self) -> Option<PathBuf> {
        self.copie_conservee.clone()
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
        assert!(texte.contains("Preamp: 0.00 dB"));
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

    fn noms_du_dossier(dir: &TempDir) -> Vec<String> {
        let mut noms: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        noms.sort();
        noms
    }

    #[test]
    fn config_remplacee_entre_deux_appliquer_n_est_pas_perdue_par_retirer() {
        // O1 -> appliquer -> l'utilisateur (ou une réinstallation) met O2 -> appliquer -> retirer.
        let dir = TempDir::new().unwrap();
        let o1 = b"Preamp: -1 dB\n";
        let o2 = b"Preamp: -9 dB\r\nFilter: ON PK Fc 60 Hz Gain 4 dB Q 1";
        fs::write(dir.path().join("config.txt"), o1).unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        fs::write(dir.path().join("config.txt"), o2).unwrap();
        m.appliquer(&courbe_test()).unwrap();
        assert_eq!(m.etat(), EtatMoteur::Actif);
        m.retirer().unwrap();

        // La config actuelle de l'utilisateur est rendue, pas l'ancienne.
        assert_eq!(lire(&dir, "config.txt"), o2);
        assert!(!dir.path().join("retro-eq.txt").exists());
        // L'ancienne copie reste, sous un nom qui ne gêne pas une future installation.
        assert!(!dir.path().join("config.txt.avant-retro-eq").exists());
        let noms = noms_du_dossier(&dir);
        assert_eq!(noms.len(), 2, "{noms:?}");
        let gardee = noms
            .iter()
            .find(|n| n.starts_with("config.txt.avant-retro-eq."))
            .expect("copie de sécurité conservée");
        assert_eq!(lire(&dir, gardee), o1);
        assert_eq!(m.copie_de_securite(), Some(dir.path().join(gardee)));
    }

    #[test]
    fn deux_copies_conservees_ne_se_marchent_pas_dessus() {
        let dir = TempDir::new().unwrap();
        for (i, (avant, apres)) in [(b"a\n", b"b\n"), (b"c\n", b"d\n")].into_iter().enumerate() {
            fs::write(dir.path().join("config.txt"), avant).unwrap();
            let mut m = moteur(&dir);
            m.appliquer(&courbe_test()).unwrap();
            fs::write(dir.path().join("config.txt"), apres).unwrap();
            m.retirer().unwrap();
            assert_eq!(lire(&dir, "config.txt"), apres, "tour {i}");
        }
        let copies: Vec<_> = noms_du_dossier(&dir)
            .into_iter()
            .filter(|n| n.starts_with("config.txt.avant-retro-eq."))
            .collect();
        assert_eq!(copies.len(), 2, "{copies:?}");
    }

    #[test]
    fn retirer_sans_ecart_ne_garde_pas_de_copie_et_ne_signale_rien() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "x\n").unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        m.retirer().unwrap();
        assert_eq!(m.copie_de_securite(), None);
        assert_eq!(noms_du_dossier(&dir), vec!["config.txt".to_string()]);
    }

    #[test]
    fn retirer_apres_ajout_d_une_ligne_par_l_utilisateur_la_garde() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("config.txt"), "Preamp: -1 dB\n").unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        let mut courante = lire(&dir, "config.txt");
        courante.extend_from_slice(b"Filter: ON PK Fc 1 Hz Gain 1 dB Q 1\n");
        fs::write(dir.path().join("config.txt"), courante).unwrap();
        // Un nouvel appliquer remet tout au propre, sans rien empiler.
        m.appliquer(&courbe_test()).unwrap();
        m.retirer().unwrap();
        assert_eq!(
            lire(&dir, "config.txt"),
            b"Preamp: -1 dB\nFilter: ON PK Fc 1 Hz Gain 1 dB Q 1\n"
        );
    }

    #[test]
    fn config_absente_puis_creee_par_l_utilisateur_reste_apres_retirer() {
        let dir = TempDir::new().unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        fs::write(dir.path().join("config.txt"), "Preamp: -4 dB\n").unwrap();
        m.appliquer(&courbe_test()).unwrap();
        m.retirer().unwrap();
        assert_eq!(lire(&dir, "config.txt"), b"Preamp: -4 dB\n");
    }

    // --- annulation exacte des modifications ---

    /// Petit générateur pseudo-aléatoire (xorshift) : pas de dépendance de test en plus.
    struct Alea(u64);
    impl Alea {
        fn suivant(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn jusqu_a(&mut self, n: usize) -> usize {
            (self.suivant() % n as u64) as usize
        }
    }

    /// Morceaux qui piègent la découpe en lignes, la détection de nos marques et le BOM.
    const MORCEAUX: &[&[u8]] = &[
        b"\n",
        b"\r\n",
        b"\r",
        b"a",
        b"Preamp: -3 dB",
        b"Filter: ON PK Fc 100 Hz Gain 5 dB Q 1",
        b"#",
        b"# [retro-eq] ",
        b"# [retro-eq] Preamp: 1 dB",
        b"Include: retro-eq.txt",
        b"include:  RETRO-EQ.txt ",
        b" ",
        b"\t",
        b"\xEF\xBB\xBF",
        b"\xFF\xFE",
        b"\x00",
    ];

    fn octets_generes(alea: &mut Alea) -> Vec<u8> {
        let mut x = Vec::new();
        for _ in 0..alea.jusqu_a(12) {
            if alea.jusqu_a(5) == 0 {
                x.push(alea.suivant() as u8);
            } else {
                x.extend_from_slice(MORCEAUX[alea.jusqu_a(MORCEAUX.len())]);
            }
        }
        x
    }

    #[test]
    fn debrancher_defait_exactement_brancher_pour_n_importe_quels_octets() {
        let mut alea = Alea(0x9E37_79B9_7F4A_7C15);
        for _ in 0..20_000 {
            let x = octets_generes(&mut alea);
            let y = brancher(Some(&x));
            assert_eq!(debrancher(&y), Some(x.clone()), "x = {x:?}, y = {y:?}");
            // Notre Include est présent et se retrouve.
            assert!(contient_notre_include(&y), "x = {x:?}");
        }
    }

    #[test]
    fn brancher_est_injectif_sur_les_cas_limites() {
        // Deux entrées différentes ne donnent jamais la même sortie.
        let cas: Vec<Option<&[u8]>> = vec![
            None,
            Some(b""),
            Some(b"\n"),
            Some(b"a"),
            Some(b"a\n"),
            Some(b"a\r\n"),
            Some(b"\xEF\xBB\xBF"),
        ];
        let sorties: Vec<Vec<u8>> = cas.iter().map(|x| brancher(*x)).collect();
        for i in 0..sorties.len() {
            for j in i + 1..sorties.len() {
                assert_ne!(sorties[i], sorties[j], "{:?} / {:?}", cas[i], cas[j]);
            }
        }
        assert_eq!(debrancher(&brancher(None)), None);
        assert_eq!(debrancher(&brancher(Some(b""))), Some(Vec::new()));
    }

    #[test]
    fn une_ligne_deja_prefixee_ou_notre_include_dans_l_original_ne_corrompt_rien() {
        for x in [
            &b"# [retro-eq] Preamp: 1 dB\n"[..],
            b"Include: retro-eq.txt\n",
            b"Include: retro-eq.txt",
            b"# [retro-eq] ",
            b"\xEF\xBB\xBF# [retro-eq] x\r\nInclude: retro-eq.txt\r\n",
        ] {
            assert_eq!(debrancher(&brancher(Some(x))), Some(x.to_vec()), "{x:?}");
        }
    }

    #[test]
    fn brancher_ne_se_cumule_pas_avec_normaliser() {
        let mut alea = Alea(42);
        for _ in 0..2_000 {
            let x = octets_generes(&mut alea);
            let y = brancher(Some(&x));
            // Ce que fait `appliquer` sur un fichier déjà branché : même octets.
            assert_eq!(rebrancher(&y), y, "x = {x:?}");
        }
    }

    #[test]
    fn un_fichier_temporaire_d_un_autre_processus_n_est_pas_ecrase() {
        let dir = TempDir::new().unwrap();
        // Les anciens noms fixes : un autre processus pourrait les utiliser.
        fs::write(dir.path().join("config.txt.tmp"), "autre processus").unwrap();
        fs::write(dir.path().join("retro-eq.txt.tmp"), "autre processus").unwrap();
        fs::write(dir.path().join("config.txt"), "x\n").unwrap();
        let mut m = moteur(&dir);
        m.appliquer(&courbe_test()).unwrap();
        assert_eq!(lire(&dir, "config.txt.tmp"), b"autre processus");
        assert_eq!(lire(&dir, "retro-eq.txt.tmp"), b"autre processus");
    }

    #[test]
    fn les_noms_temporaires_sont_uniques_et_voisins() {
        let cible = Path::new("/un/dossier/config.txt");
        let a = nom_temporaire(cible);
        let b = nom_temporaire(cible);
        assert_ne!(a, b);
        assert_eq!(a.parent(), cible.parent());
        assert!(a
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("config.txt."));
        assert!(a
            .to_string_lossy()
            .contains(&std::process::id().to_string()));
    }

    #[test]
    fn un_echec_d_ecriture_nettoie_le_fichier_temporaire() {
        let dir = TempDir::new().unwrap();
        // Un dossier à la place du fichier : le renommage final échoue.
        fs::create_dir(dir.path().join("cible")).unwrap();
        fs::write(dir.path().join("cible").join("occupe"), "x").unwrap();
        assert!(ecrire_atomique(&dir.path().join("cible"), b"abc").is_err());
        assert_eq!(noms_du_dossier(&dir), vec!["cible".to_string()]);
    }

    #[cfg(not(windows))]
    #[test]
    fn hors_windows_pas_de_detection_ni_de_sorties() {
        assert_eq!(EqualizerApo::detecter(), None);
        let dir = TempDir::new().unwrap();
        assert!(moteur(&dir).sorties().unwrap().is_empty());
    }
}
