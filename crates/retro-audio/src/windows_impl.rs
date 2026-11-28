//! Implémentations Windows (WASAPI et API Win32). Compilées uniquement sous Windows.
//!
//! // A VERIFIER SUR LE PC : tout ce module n'a été que vérifié par le compilateur depuis
//! Linux, jamais exécuté. Voir la liste dans le rapport de livraison.

use crate::{
    Bip, Capture, DetecteurAppel, ErreurAudio, InfoSortie, PleinEcran, PremierPlan, SortiesAudio,
    Trame,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use wasapi::{
    AudioClient, DeviceEnumerator, Direction, SampleType, SessionState, StreamMode, WasapiError,
};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, MB_OK,
};

impl From<WasapiError> for ErreurAudio {
    fn from(e: WasapiError) -> Self {
        ErreurAudio::Systeme(e.to_string())
    }
}

/// Initialise COM (la couche d'objets sur laquelle repose tout l'audio Windows) pour le fil
/// courant. COM se prépare fil par fil : un fil qui ne l'a pas fait ne peut pas créer
/// d'objets audio. Le `Drop` défait ce qui a été fait.
struct ComFil {
    initialise: bool,
}

impl ComFil {
    fn new() -> Self {
        // Échec si le fil a déjà été initialisé d'une autre façon (par exemple par
        // l'interface) : COM reste alors utilisable, mais on ne doit pas le « désinitialiser ».
        Self {
            initialise: wasapi::initialize_mta().is_ok(),
        }
    }
}

impl Drop for ComFil {
    fn drop(&mut self) {
        if self.initialise {
            wasapi::deinitialize();
        }
    }
}

/// Nom du fichier exécutable d'un processus, sans dossier ni `.exe`.
fn nom_processus(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    // SAFETY: appel Win32 sans pointeur ; rend un handle que l'on referme plus bas.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut tampon = [0u16; 1024];
    let mut taille = tampon.len() as u32;
    // SAFETY: `tampon` et `taille` vivent pendant l'appel ; `taille` donne bien sa capacité.
    let resultat = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(tampon.as_mut_ptr()),
            &mut taille,
        )
    };
    // SAFETY: `handle` vient d'`OpenProcess` et n'est refermé qu'ici.
    let _ = unsafe { CloseHandle(handle) };
    resultat.ok()?;
    let chemin = String::from_utf16_lossy(&tampon[..taille as usize]);
    Path::new(&chemin)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------- Capture loopback

/// Durée d'un bloc livré, en millisecondes.
const BLOC_MS: u32 = 20;
/// Attente maximale d'un événement WASAPI ; en loopback il n'y en a pas quand rien ne joue.
const ATTENTE_EVENEMENT_MS: u32 = 100;
const PROFONDEUR_CANAL: usize = 64;

/// Écoute (« loopback ») de ce que joue la sortie par défaut, en mode partagé : on reçoit le
/// mélange que le système envoie aux enceintes, sans rien changer à ce qu'on entend.
#[derive(Default)]
pub struct CaptureLoopback {
    arret: Option<Arc<AtomicBool>>,
    fil: Option<JoinHandle<()>>,
}

impl CaptureLoopback {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Capture for CaptureLoopback {
    fn demarrer(&mut self) -> Result<Receiver<Trame>, ErreurAudio> {
        self.arreter();
        let (tx, rx) = mpsc::sync_channel(PROFONDEUR_CANAL);
        let (pret_tx, pret_rx) = mpsc::sync_channel::<Result<(), ErreurAudio>>(1);
        let arret = Arc::new(AtomicBool::new(false));
        let arret_fil = Arc::clone(&arret);
        let fil = thread::Builder::new()
            .name("capture-loopback".into())
            .spawn(move || {
                // COM doit être préparé sur le fil qui s'en sert, donc ici et pas dans `demarrer`.
                let _com = ComFil::new();
                match ouvrir() {
                    Ok(flux) => {
                        let _ = pret_tx.send(Ok(()));
                        // Toute erreur en cours de route (casque débranché, sortie par défaut
                        // changée) termine le fil ; le canal se ferme et l'appli relance.
                        let _ = boucle_capture(flux, &tx, &arret_fil);
                    }
                    Err(e) => {
                        let _ = pret_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| ErreurAudio::Systeme(e.to_string()))?;
        // On attend que le fil dise si l'ouverture a marché, pour rendre l'erreur ici.
        match pret_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {
                self.arret = Some(arret);
                self.fil = Some(fil);
                Ok(rx)
            }
            Ok(Err(e)) => {
                let _ = fil.join();
                Err(e)
            }
            Err(_) => {
                arret.store(true, Ordering::Relaxed);
                Err(ErreurAudio::Indisponible(
                    "l'ouverture de la capture est trop longue".into(),
                ))
            }
        }
    }

    fn arreter(&mut self) {
        if let Some(arret) = self.arret.take() {
            arret.store(true, Ordering::Relaxed);
        }
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
    }
}

impl Drop for CaptureLoopback {
    fn drop(&mut self) {
        self.arreter();
    }
}

/// Format des échantillons bruts lus dans le flux.
#[derive(Clone, Copy)]
enum Codage {
    Flottant32,
    Entier16,
    Entier24,
    Entier32,
}

impl Codage {
    fn octets(self) -> usize {
        match self {
            Codage::Flottant32 | Codage::Entier32 => 4,
            Codage::Entier16 => 2,
            Codage::Entier24 => 3,
        }
    }

    /// Lit un échantillon (petit-boutiste, comme tout Windows) et le ramène à [-1, 1].
    fn lire(self, o: &[u8]) -> f32 {
        match self {
            Codage::Flottant32 => f32::from_le_bytes([o[0], o[1], o[2], o[3]]),
            Codage::Entier16 => f32::from(i16::from_le_bytes([o[0], o[1]])) / 32_768.0,
            // 24 bits : on place les 3 octets en haut d'un entier 32 bits signé.
            Codage::Entier24 => i32::from_le_bytes([0, o[0], o[1], o[2]]) as f32 / 2_147_483_648.0,
            Codage::Entier32 => {
                i32::from_le_bytes([o[0], o[1], o[2], o[3]]) as f32 / 2_147_483_648.0
            }
        }
    }
}

/// Tout ce qu'il faut pour lire le flux ouvert.
struct FluxOuvert {
    client: AudioClient,
    // Gardé pour que l'événement reste ouvert tant que le flux vit.
    evenement: wasapi::Handle,
    lecteur: wasapi::AudioCaptureClient,
    canaux: usize,
    codage: Codage,
    frequence: u32,
}

fn ouvrir() -> Result<FluxOuvert, ErreurAudio> {
    let enumerateur = DeviceEnumerator::new()?;
    let appareil = enumerateur
        .get_default_device(Direction::Render)
        .map_err(|e| ErreurAudio::Indisponible(e.to_string()))?;
    let mut client = appareil.get_iaudioclient()?;
    // Le format de mixage est celui que le système utilise déjà : aucune conversion à payer.
    let format = client.get_mixformat()?;
    let bits = format.get_bitspersample();
    let codage = match (format.get_subformat()?, bits) {
        (SampleType::Float, 32) => Codage::Flottant32,
        (SampleType::Int, 16) => Codage::Entier16,
        (SampleType::Int, 24) => Codage::Entier24,
        (SampleType::Int, 32) => Codage::Entier32,
        (type_, bits) => return Err(ErreurAudio::FormatInconnu(format!("{type_} {bits} bits"))),
    };
    let canaux = usize::from(format.get_nchannels());
    if canaux == 0 || format.get_blockalign() as usize != canaux * codage.octets() {
        return Err(ErreurAudio::FormatInconnu(format!(
            "{canaux} canaux, {bits} bits, alignement {}",
            format.get_blockalign()
        )));
    }
    let (periode_defaut, _) = client.get_device_period()?;
    // Demander la direction « capture » sur un client de sortie active le mode loopback.
    client.initialize_client(
        &format,
        Direction::Capture,
        StreamMode::EventsShared {
            autoconvert: false,
            buffer_duration_hns: periode_defaut,
        },
        None,
    )?;
    let evenement = client.set_get_eventhandle()?;
    let lecteur = client.get_audiocaptureclient()?;
    client.start_stream()?;
    Ok(FluxOuvert {
        client,
        evenement,
        lecteur,
        canaux,
        codage,
        frequence: format.get_samplespersec(),
    })
}

/// Lit le flux jusqu'à l'arrêt demandé, la fermeture du canal ou une erreur.
fn boucle_capture(
    flux: FluxOuvert,
    tx: &SyncSender<Trame>,
    arret: &AtomicBool,
) -> Result<(), ErreurAudio> {
    let taille_bloc = (flux.frequence * BLOC_MS / 1000).max(1) as usize;
    let largeur_frame = flux.canaux * flux.codage.octets();
    let mut octets: Vec<u8> = Vec::new();
    let mut accumule: Vec<f32> = Vec::with_capacity(taille_bloc * 2);
    let mut dernier = Instant::now();

    let resultat = (|| -> Result<(), ErreurAudio> {
        while !arret.load(Ordering::Relaxed) {
            let attente = flux.evenement.wait_for_event(ATTENTE_EVENEMENT_MS);
            let maintenant = Instant::now();
            if attente.is_err() {
                // Rien ne joue : en loopback le système n'envoie alors aucun paquet. On comble
                // par du silence pour que l'affichage retombe, et on interroge le flux, ce qui
                // échoue si la sortie a disparu ou changé.
                flux.lecteur.get_next_packet_size()?;
                let manque = (maintenant - dernier).as_secs_f64() * f64::from(flux.frequence);
                accumule.resize(accumule.len() + manque as usize, 0.0);
            }
            dernier = maintenant;

            while let Some(frames) = flux.lecteur.get_next_packet_size()? {
                if frames == 0 {
                    break;
                }
                octets.resize(frames as usize * largeur_frame, 0);
                let (lues, _) = flux.lecteur.read_from_device(&mut octets)?;
                for frame in octets[..lues as usize * largeur_frame].chunks_exact(largeur_frame) {
                    // Mono = moyenne des canaux.
                    let somme: f32 = frame
                        .chunks_exact(flux.codage.octets())
                        .map(|o| flux.codage.lire(o))
                        .sum();
                    accumule.push(somme / flux.canaux as f32);
                }
            }

            while accumule.len() >= taille_bloc {
                let echantillons: Vec<f32> = accumule.drain(..taille_bloc).collect();
                let trame = Trame {
                    echantillons,
                    frequence_echantillonnage: flux.frequence,
                };
                match tx.try_send(trame) {
                    Ok(()) => {}
                    // Consommateur à la traîne : on saute ce bloc plutôt que de bloquer.
                    Err(TrySendError::Full(_)) => {}
                    // Récepteur lâché : plus personne n'écoute.
                    Err(TrySendError::Disconnected(_)) => return Ok(()),
                }
            }
        }
        Ok(())
    })();
    let _ = flux.client.stop_stream();
    resultat
}

// ---------------------------------------------------------------- Observation du système

/// Appli au premier plan, d'après la fenêtre active.
pub struct PremierPlanWindows;

impl PremierPlan for PremierPlanWindows {
    fn appli(&self) -> Option<String> {
        // SAFETY: appel Win32 sans argument ; une fenêtre nulle est testée juste après.
        let fenetre = unsafe { GetForegroundWindow() };
        if fenetre.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        // SAFETY: `pid` vit pendant l'appel.
        unsafe { GetWindowThreadProcessId(fenetre, Some(&mut pid)) };
        // Retro EQ lui-même n'est pas une activité : on ne le note pas (ticket 28).
        crate::appli_a_noter(pid, std::process::id(), nom_processus)
    }
}

/// Détecte un appel Discord : une session de capture micro active appartenant à Discord.
///
/// // A VERIFIER SUR LE PC : c'est une heuristique. Discord garde, d'après nos observations
/// // publiques, sa session micro active seulement pendant un appel ou dans un salon vocal.
/// // Si Discord garde le micro ouvert en permanence, ou le ferme en push-to-talk, le résultat
/// // sera faux.
pub struct DetecteurAppelDiscord;

fn est_discord(nom: &str) -> bool {
    matches!(
        nom.to_ascii_lowercase().as_str(),
        "discord" | "discordptb" | "discordcanary"
    )
}

fn discord_utilise_un_micro() -> Result<bool, WasapiError> {
    let enumerateur = DeviceEnumerator::new()?;
    let micros = enumerateur.get_device_collection(Direction::Capture)?;
    for i in 0..micros.get_nbr_devices()? {
        // Un micro qui refuse l'accès ne doit pas empêcher d'examiner les autres.
        let Ok(appareil) = micros.get_device_at_index(i) else {
            continue;
        };
        let Ok(gestionnaire) = appareil.get_iaudiosessionmanager() else {
            continue;
        };
        let Ok(sessions) = gestionnaire.get_audiosessionenumerator() else {
            continue;
        };
        for j in 0..sessions.get_count()? {
            let Ok(session) = sessions.get_session(j) else {
                continue;
            };
            if !matches!(session.get_state(), Ok(SessionState::Active)) {
                continue;
            }
            let Ok(pid) = session.get_process_id() else {
                continue;
            };
            if nom_processus(pid).is_some_and(|n| est_discord(&n)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

impl DetecteurAppel for DetecteurAppelDiscord {
    fn appel_en_cours(&self) -> bool {
        // COM est propre à chaque fil, et cette méthode peut être appelée de n'importe lequel.
        let _com = ComFil::new();
        discord_utilise_un_micro().unwrap_or(false)
    }
}

/// Plein écran exclusif d'un jeu (Direct3D), seul cas où aucune fenêtre ne peut s'afficher.
pub struct PleinEcranWindows;

impl PleinEcran for PleinEcranWindows {
    fn exclusif(&self) -> bool {
        // SAFETY: appel Win32 sans argument.
        matches!(
            unsafe { SHQueryUserNotificationState() },
            Ok(etat) if etat == QUNS_RUNNING_D3D_FULL_SCREEN
        )
    }
}

/// Sorties audio actives (casque, enceintes...), avec leur nom lisible.
pub struct SortiesWindows;

fn lister_sorties() -> Result<Vec<InfoSortie>, WasapiError> {
    let enumerateur = DeviceEnumerator::new()?;
    let id_defaut = enumerateur
        .get_default_device(Direction::Render)
        .and_then(|d| d.get_id())
        .ok();
    let collection = enumerateur.get_device_collection(Direction::Render)?;
    let mut sorties = Vec::new();
    for i in 0..collection.get_nbr_devices()? {
        let Ok(appareil) = collection.get_device_at_index(i) else {
            continue;
        };
        let (Ok(id), Ok(nom)) = (appareil.get_id(), appareil.get_friendlyname()) else {
            continue;
        };
        let par_defaut = id_defaut.as_deref() == Some(id.as_str());
        sorties.push(InfoSortie {
            id,
            nom,
            par_defaut,
        });
    }
    Ok(sorties)
}

impl SortiesAudio for SortiesWindows {
    fn sortie_par_defaut(&self) -> Option<InfoSortie> {
        self.sorties().into_iter().find(|s| s.par_defaut)
    }

    fn sorties(&self) -> Vec<InfoSortie> {
        let _com = ComFil::new();
        lister_sorties().unwrap_or_default()
    }
}

/// Bip système court.
pub struct BipWindows;

impl Bip for BipWindows {
    fn bip(&self) {
        // SAFETY: appel Win32 sans pointeur ; il met le son en file et rend la main aussitôt,
        // donc il ne bloque pas l'appelant.
        // A VERIFIER SUR LE PC : MB_OK joue le son « Bip par défaut » du thème Windows ; il est
        // muet si l'utilisateur l'a désactivé.
        let _ = unsafe { MessageBeep(MB_OK) };
    }
}
