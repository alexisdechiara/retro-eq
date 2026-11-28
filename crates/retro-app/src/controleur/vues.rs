//! Ce que le contrôleur montre : une structure par écran, en lecture seule.
//!
//! Ce sont des « photos » sans aucun type Slint. Le pont (`pont.rs`) les recopie dans les
//! propriétés de l'interface ; les tests, eux, les lisent directement.

use retro_dsp::Courbe;

/// Où en est le moteur audio, vu de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatMoteurVue {
    Absent,
    Inactif,
    Actif,
}

impl EtatMoteurVue {
    /// Le code attendu par `Appli.etat-moteur` : 0 absent, 1 inactif, 2 actif.
    pub fn code(self) -> i32 {
        match self {
            EtatMoteurVue::Absent => 0,
            EtatMoteurVue::Inactif => 1,
            EtatMoteurVue::Actif => 2,
        }
    }
}

/// L'état général : barre de titre et accueil.
#[derive(Debug, Clone, PartialEq)]
pub struct VueAppli {
    pub profil_actif: String,
    /// Faux quand l'égaliseur est coupé (Ctrl+Alt+0).
    pub egaliseur_actif: bool,
    pub etat_moteur: EtatMoteurVue,
    /// L'utilisateur a accepté que l'appli écrive dans la configuration audio.
    pub moteur_autorise: bool,
    /// Un test est commencé et pas fini (en écoute, en attente ou en pause).
    pub test_en_cours: bool,
}

/// L'écran Égaliseur.
#[derive(Debug, Clone, PartialEq)]
pub struct VueEgaliseur {
    pub gains: [f64; 10],
    pub preampli_db: f64,
    pub preampli_auto: bool,
    /// La courbe affichée (le brouillon s'il y en a un).
    pub courbe: Courbe,
    pub filtres_supplementaires: usize,
    pub modifie: bool,
    pub profil: String,
}

/// Une cassette de l'écran « Mes profils ».
#[derive(Debug, Clone, PartialEq)]
pub struct VueProfil {
    pub id: String,
    pub nom: String,
    pub actif: bool,
    pub versions: usize,
    pub date: String,
    pub resume: String,
    pub courbe: Courbe,
}

/// Une version dans le détail d'un profil.
#[derive(Debug, Clone, PartialEq)]
pub struct VueVersion {
    pub numero: u32,
    pub date: String,
    pub origine: String,
    pub active: bool,
    pub courbe: Courbe,
}

/// L'état d'un test tel que l'interface le distingue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatTestVue {
    Pret,
    Ecoute,
    AttenteReponse,
    Pause,
    PauseAppel,
    Termine,
}

/// Une ligne du journal affiché pendant un test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueLigneJournal {
    pub titre: String,
    /// 1 = A, 2 = B, 0 = aucun côté à montrer.
    pub cote: i32,
    pub detail: String,
}

/// L'écran Duels et la mini-fenêtre.
#[derive(Debug, Clone, PartialEq)]
pub struct VueDuels {
    pub etat: EtatTestVue,
    pub duel: u32,
    pub total: u32,
    /// 0 aucun, 1 A, 2 B.
    pub cote_actif: i32,
    pub passages_a: u8,
    pub passages_b: u8,
    pub avancement: f64,
    pub compte_a_rebours: String,
    pub suite: String,
    /// Du duel le plus récent au plus ancien.
    pub journal: Vec<VueLigneJournal>,
    pub pause_conseillee: bool,
    /// La pause vient d'un délai de réponse dépassé (ADR-0012).
    pub pause_sans_reponse: bool,
    /// Les touches de réponse prises par une autre appli pendant ce test (« F8, F10 ») :
    /// elles ne répondront pas. Vide s'il n'y en a pas (ticket 30).
    pub touches_mortes: String,
}

impl Default for VueDuels {
    /// Aucun test : l'écran Duels propose d'en commencer un.
    fn default() -> Self {
        VueDuels {
            etat: EtatTestVue::Pret,
            duel: 1,
            total: 12,
            cote_actif: 0,
            passages_a: 0,
            passages_b: 0,
            avancement: 0.0,
            compte_a_rebours: String::new(),
            suite: String::new(),
            journal: Vec::new(),
            pause_conseillee: false,
            pause_sans_reponse: false,
            touches_mortes: String::new(),
        }
    }
}

/// Le résumé de l'écran « Nouveau test ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueNouveauTest {
    pub reference: String,
    pub source: String,
    pub mode: String,
    pub duree: String,
    pub zones: Vec<String>,
    /// Les autres profils, pour « profil contre profil ».
    pub profils: Vec<String>,
    /// La réduction de bruit du casque : le choix en cours, sinon ce que le profil actif a
    /// retenu. `None` : jamais répondu, aucune option n'est cochée (ticket 24).
    pub reduction_bruit: Option<bool>,
}

/// Une zone du résultat.
#[derive(Debug, Clone, PartialEq)]
pub struct VueZone {
    pub nom: String,
    pub plage: String,
    pub gain_db: f64,
    pub change: bool,
    pub confiance: i32,
    pub detail: String,
}

/// Une appli du résultat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueAppliResultat {
    pub nom: String,
    pub duels: u32,
    pub preference: String,
}

/// L'écran Résultat.
#[derive(Debug, Clone, PartialEq)]
pub struct VueResultat {
    pub titre: String,
    pub confiance: i32,
    pub resume: String,
    pub controles: String,
    pub verification: String,
    pub zones: Vec<VueZone>,
    pub applis: Vec<VueAppliResultat>,
    pub courbe_reference: Courbe,
    pub courbe_nouvelle: Courbe,
    /// Vrai quand ce que le test propose est ce qui est actif en ce moment.
    pub active: bool,
    /// Faux quand rien ne pousse à activer : aucun changement retenu, ou la référence
    /// préférée au duel de vérification. L'activation reste possible, sans être mise en avant.
    pub recommandee: bool,
    pub preampli_db: f64,
}

/// La page Réglages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueReglages {
    pub langue: &'static str,
    pub rail: bool,
    pub fermer_reduit: bool,
    pub mini_fenetre: bool,
    pub bip: bool,
    pub pause_appels: bool,
    pub intervalle_s: u32,
    pub verifier_mises_a_jour: bool,
}

/// La section « Mises à jour » de l'écran À propos, et la pastille de la barre de titre.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VueMiseAJour {
    /// Une vérification est partie et n'a pas encore répondu.
    pub en_cours: bool,
    /// 0 pas encore vérifié, 1 à jour, 2 mise à jour disponible, 3 aucune version publiée.
    pub etat: i32,
    /// « vérifié aujourd’hui à 19:40 » ; vide si aucune vérification n'a réussi.
    pub verification: String,
    /// La version proposée (état 2), sinon vide.
    pub version: String,
    /// Sa date, en toutes lettres, vide si GitHub ne l'a pas donnée.
    pub date: String,
    /// Ses notes, en texte brut tronqué.
    pub notes: String,
    /// La page de la version, à ouvrir dans le navigateur.
    pub page: String,
    /// « Retro-EQ-Installateur-0.2.0.exe · 11,8 Mo », vide s'il n'y en a pas.
    pub installateur: String,
    /// La dernière vérification a échoué : pourquoi, en une phrase. Vide sinon. L'état
    /// précédemment connu reste affiché à côté.
    pub probleme: String,
}

impl VueMiseAJour {
    /// Une mise à jour est à proposer : la pastille s'allume.
    pub fn disponible(&self) -> bool {
        self.etat == 2
    }
}

/// Le message court affiché en bas de la fenêtre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annonce {
    pub texte: String,
    pub erreur: bool,
}
