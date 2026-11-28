//! Versions simulées de chaque capacité : l'appli tourne et se teste sans Windows ni carte son.

use crate::{
    Bip, Capture, DetecteurAppel, ErreurAudio, InfoSortie, PleinEcran, PremierPlan, SortiesAudio,
    Trame,
};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Fréquence d'échantillonnage du signal simulé.
pub const FREQUENCE_SIMULEE: u32 = 48_000;
/// Taille d'une trame simulée (≈ 21 ms à 48 kHz).
pub const TAILLE_TRAME_SIMULEE: usize = 1024;
/// Trames en attente au plus : au-delà, on jette plutôt que de remplir la mémoire.
pub const PROFONDEUR_CANAL: usize = 64;

/// Générateur du signal de test : bruit rose, plus une note qui glisse lentement entre
/// 200 Hz et 2 kHz, le tout dont le volume monte et descend.
pub struct Synthese {
    fe: f32,
    graine: u32,
    rose: [f32; 3],
    phase_note: f32,
    t: f32,
}

impl Synthese {
    pub fn new(frequence_echantillonnage: u32) -> Self {
        Self {
            fe: frequence_echantillonnage as f32,
            graine: 0x1234_5678,
            rose: [0.0; 3],
            phase_note: 0.0,
            t: 0.0,
        }
    }

    /// Bruit blanc dans [-1, 1] (xorshift : rapide, sans dépendance, reproductible).
    fn blanc(&mut self) -> f32 {
        self.graine ^= self.graine << 13;
        self.graine ^= self.graine >> 17;
        self.graine ^= self.graine << 5;
        (self.graine as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Produit `n` échantillons ; la phase et le temps se poursuivent d'un appel à l'autre,
    /// sans coupure entre deux blocs.
    pub fn bloc(&mut self, n: usize) -> Vec<f32> {
        let mut sortie = Vec::with_capacity(n);
        for _ in 0..n {
            // Rose approché : somme de trois bruits blancs filtrés à des vitesses différentes
            // (plus le filtre est lent, plus il favorise les graves).
            let b = self.blanc();
            self.rose[0] = 0.997 * self.rose[0] + 0.029 * b;
            self.rose[1] = 0.95 * self.rose[1] + 0.15 * b;
            self.rose[2] = 0.6 * self.rose[2] + 0.4 * b;
            let bruit = (self.rose[0] + self.rose[1] + self.rose[2]) * 0.8;

            // La note glisse de façon logarithmique (une octave perçue = pas constant).
            let position = 0.5 + 0.5 * (TAU * self.t / 17.0).sin();
            let f_note = 200.0 * 10f32.powf(position);
            self.phase_note = (self.phase_note + TAU * f_note / self.fe) % TAU;
            let note = 0.3 * self.phase_note.sin();

            // Volume : de -34 dB à -6 dB environ, en 9 secondes.
            let enveloppe = 0.02 + 0.48 * (0.5 + 0.5 * (TAU * self.t / 9.0).sin());
            sortie.push((bruit * 0.5 + note) * enveloppe * 2.0);
            self.t += 1.0 / self.fe;
        }
        sortie
    }
}

/// Capture factice : un signal de synthèse à 48 kHz, en blocs de 1024 échantillons cadencés
/// en temps réel (comme le ferait la vraie carte son).
#[derive(Default)]
pub struct CaptureSimulee {
    arret: Option<Arc<AtomicBool>>,
    fil: Option<JoinHandle<()>>,
}

impl CaptureSimulee {
    pub fn new() -> Self {
        Self::default()
    }

    /// Le fil d'arrière-plan tourne-t-il encore ? (Sert aux tests : après `drop` du récepteur,
    /// il doit finir de lui-même.)
    pub fn actif(&self) -> bool {
        self.fil.as_ref().is_some_and(|f| !f.is_finished())
    }
}

impl Capture for CaptureSimulee {
    fn demarrer(&mut self) -> Result<Receiver<Trame>, ErreurAudio> {
        self.arreter();
        let (tx, rx) = mpsc::sync_channel(PROFONDEUR_CANAL);
        let arret = Arc::new(AtomicBool::new(false));
        let arret_fil = Arc::clone(&arret);
        let fil = thread::Builder::new()
            .name("capture-simulee".into())
            .spawn(move || boucle_simulee(tx, arret_fil))
            .map_err(|e| ErreurAudio::Systeme(e.to_string()))?;
        self.arret = Some(arret);
        self.fil = Some(fil);
        Ok(rx)
    }

    fn arreter(&mut self) {
        if let Some(arret) = self.arret.take() {
            arret.store(true, Ordering::Relaxed);
        }
        if let Some(fil) = self.fil.take() {
            // Un fil qui a paniqué n'a plus rien à nous apprendre : on ignore son résultat.
            let _ = fil.join();
        }
    }
}

impl Drop for CaptureSimulee {
    fn drop(&mut self) {
        self.arreter();
    }
}

fn boucle_simulee(tx: SyncSender<Trame>, arret: Arc<AtomicBool>) {
    let mut synthese = Synthese::new(FREQUENCE_SIMULEE);
    let duree = Duration::from_secs_f64(TAILLE_TRAME_SIMULEE as f64 / f64::from(FREQUENCE_SIMULEE));
    // On vise des échéances fixes (et non « dormir une durée ») pour ne pas accumuler de dérive.
    let mut echeance = Instant::now();
    while !arret.load(Ordering::Relaxed) {
        let trame = Trame {
            echantillons: synthese.bloc(TAILLE_TRAME_SIMULEE),
            frequence_echantillonnage: FREQUENCE_SIMULEE,
        };
        match tx.try_send(trame) {
            Ok(()) => {}
            // Consommateur à la traîne : on saute ce bloc au lieu de bloquer.
            Err(TrySendError::Full(_)) => {}
            // Récepteur lâché : plus personne n'écoute, le fil se termine.
            Err(TrySendError::Disconnected(_)) => return,
        }
        echeance += duree;
        let maintenant = Instant::now();
        if echeance > maintenant {
            thread::sleep(echeance - maintenant);
        } else {
            // En retard de plus d'un bloc (machine saturée) : on repart de maintenant.
            echeance = maintenant;
        }
    }
}

/// Appli au premier plan fixée d'avance.
pub struct PremierPlanFixe(pub Option<String>);

impl PremierPlan for PremierPlanFixe {
    fn appli(&self) -> Option<String> {
        self.0.clone()
    }
}

/// Détecteur d'appel piloté depuis l'extérieur : le drapeau partagé se règle d'un autre fil.
#[derive(Default, Clone)]
pub struct DetecteurAppelSimule {
    pub drapeau: Arc<AtomicBool>,
}

impl DetecteurAppelSimule {
    pub fn new(drapeau: Arc<AtomicBool>) -> Self {
        Self { drapeau }
    }
}

impl DetecteurAppel for DetecteurAppelSimule {
    fn appel_en_cours(&self) -> bool {
        self.drapeau.load(Ordering::Relaxed)
    }
}

/// Plein écran exclusif fixé d'avance.
pub struct PleinEcranFixe(pub bool);

impl PleinEcran for PleinEcranFixe {
    fn exclusif(&self) -> bool {
        self.0
    }
}

/// Liste de sorties fixée d'avance ; la sortie par défaut est celle marquée `par_defaut`.
pub struct SortiesFixes(pub Vec<InfoSortie>);

impl SortiesAudio for SortiesFixes {
    fn sortie_par_defaut(&self) -> Option<InfoSortie> {
        self.0.iter().find(|s| s.par_defaut).cloned()
    }

    fn sorties(&self) -> Vec<InfoSortie> {
        self.0.clone()
    }
}

/// Bip silencieux qui compte ses appels (le compteur est partagé entre les copies).
#[derive(Default, Clone)]
pub struct BipMuet {
    pub appels: Arc<AtomicUsize>,
}

impl BipMuet {
    pub fn nombre_appels(&self) -> usize {
        self.appels.load(Ordering::Relaxed)
    }
}

impl Bip for BipMuet {
    fn bip(&self) {
        self.appels.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_dsp::analyse::niveau_rms_db;

    #[test]
    fn la_capture_livre_au_rythme_du_temps_reel() {
        let mut capture = CaptureSimulee::new();
        let rx = capture.demarrer().expect("démarrage");
        let debut = Instant::now();
        let mut total = 0usize;
        for _ in 0..10 {
            let trame = rx.recv_timeout(Duration::from_secs(2)).expect("trame");
            assert_eq!(trame.frequence_echantillonnage, 48_000);
            assert_eq!(trame.echantillons.len(), 1024);
            total += trame.echantillons.len();
        }
        let ecoule = debut.elapsed().as_secs_f64();
        // 10 trames de 21,3 ms : la première part tout de suite, donc ≈ 192 ms (pas 0, pas 2 s).
        let attendu = 9.0 * 1024.0 / 48_000.0;
        assert!(
            ecoule > attendu * 0.8 && ecoule < attendu * 3.0,
            "écoulé {ecoule} s, attendu ≈ {attendu} s"
        );
        assert_eq!(total, 10_240);
        capture.arreter();
    }

    #[test]
    fn le_fil_s_arrete_quand_le_recepteur_est_lache() {
        let mut capture = CaptureSimulee::new();
        let rx = capture.demarrer().expect("démarrage");
        rx.recv_timeout(Duration::from_secs(2)).expect("trame");
        drop(rx);
        let limite = Instant::now() + Duration::from_secs(2);
        while capture.actif() && Instant::now() < limite {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!capture.actif(), "le fil tourne encore sans récepteur");
    }

    #[test]
    fn arreter_ferme_le_canal_et_redemarrer_marche() {
        let mut capture = CaptureSimulee::new();
        let rx = capture.demarrer().expect("démarrage");
        capture.arreter();
        assert!(!capture.actif());
        // Les trames restantes sont lues, puis le canal est fermé.
        while rx.recv_timeout(Duration::from_secs(1)).is_ok() {}
        assert!(rx.try_recv().is_err());
        let rx2 = capture.demarrer().expect("redémarrage");
        assert!(rx2.recv_timeout(Duration::from_secs(2)).is_ok());
    }

    #[test]
    fn le_signal_est_vivant_et_borne() {
        let mut synthese = Synthese::new(48_000);
        let bloc = synthese.bloc(48_000);
        assert!(bloc.iter().all(|x| x.abs() <= 1.0));
        let niveau = niveau_rms_db(&bloc);
        assert!(niveau > -50.0 && niveau < -3.0, "niveau {niveau}");
    }

    #[test]
    fn les_detecteurs_simules_se_comportent() {
        let drapeau = Arc::new(AtomicBool::new(false));
        let appel = DetecteurAppelSimule::new(Arc::clone(&drapeau));
        assert!(!appel.appel_en_cours());
        drapeau.store(true, Ordering::Relaxed);
        assert!(appel.appel_en_cours());

        assert_eq!(PremierPlanFixe(None).appli(), None);
        assert_eq!(
            PremierPlanFixe(Some("VALORANT-Win64-Shipping".into())).appli(),
            Some("VALORANT-Win64-Shipping".into())
        );
        assert!(PleinEcranFixe(true).exclusif());
        assert!(!PleinEcranFixe(false).exclusif());

        let casque = InfoSortie {
            id: "{0.0.0}.casque".into(),
            nom: "Casque USB".into(),
            par_defaut: true,
        };
        let enceintes = InfoSortie {
            id: "{0.0.0}.hp".into(),
            nom: "Enceintes".into(),
            par_defaut: false,
        };
        let sorties = SortiesFixes(vec![enceintes.clone(), casque.clone()]);
        assert_eq!(sorties.sortie_par_defaut(), Some(casque));
        assert_eq!(sorties.sorties().len(), 2);
        assert_eq!(SortiesFixes(vec![enceintes]).sortie_par_defaut(), None);

        let bip = BipMuet::default();
        let copie = bip.clone();
        bip.bip();
        copie.bip();
        assert_eq!(bip.nombre_appels(), 2);
    }

    #[cfg(not(windows))]
    #[test]
    fn les_constructeurs_systeme_rendent_des_objets_utilisables() {
        use crate::systeme;
        let _ = systeme::premier_plan().appli();
        let _ = systeme::detecteur_appel().appel_en_cours();
        let _ = systeme::plein_ecran().exclusif();
        let _ = systeme::sorties().sorties();
        systeme::bip().bip();
        let _ = systeme::capture();
    }
}
