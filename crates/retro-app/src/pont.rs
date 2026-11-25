//! Le pont : le seul module qui connaît à la fois Slint et le contrôleur.
//!
//! Dans un sens, il branche chaque fonction de l'interface (`callback` des fichiers
//! `.slint`) sur une intention du contrôleur. Dans l'autre, il recopie les « vues » du
//! contrôleur dans les propriétés de l'interface ([`synchroniser`]).
//!
//! Tout se passe sur le fil de l'interface. L'état est dans un `Rc<RefCell<…>>` : `Rc`
//! parce que chaque fonction branchée en garde une référence (comme une variable capturée
//! par plusieurs écouteurs d'événements en JavaScript), `RefCell` parce que Rust veut la
//! preuve qu'une seule d'entre elles le modifie à la fois, et la vérifie ici à l'exécution.
//! Le son capté, lui, arrive d'un autre fil par un canal, vidé 30 fois par seconde.

use crate::autotest::{self, Cible};
use crate::barre::{Barre, EvenementBarre};
use crate::controleur::{ChoixTest, Controleur, EtatBaseAutoEq, EtatTestVue, VueDuels};
use crate::courbes::chemin_svg;
use crate::mini::Mini;
use crate::raccourcis::{Action, Raccourcis};
use crate::textes;
use crate::*;
use retro_audio::{
    Bip, Capture, DetecteurAppel, Mesure, PleinEcran, PremierPlan, SortiesAudio, Trame,
};
use retro_core::{Langue, Reponse};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Nombre de barres du visualiseur.
const BANDES: usize = 32;
/// Le minuteur de l'interface : 30 images par seconde.
const PERIODE: Duration = Duration::from_millis(33);
/// Durée d'affichage d'une annonce, en ms (une erreur reste plus longtemps).
const DUREE_ANNONCE_MS: u64 = 6_000;
const DUREE_ERREUR_MS: u64 = 12_000;

/// Ce que l'appli observe du système. Des traits, pour tourner pareil sous Windows (vraies
/// API) et ailleurs (versions simulées).
pub struct Systeme {
    pub capture: Box<dyn Capture>,
    pub premier_plan: Box<dyn PremierPlan>,
    pub appel: Box<dyn DetecteurAppel>,
    pub plein_ecran: Box<dyn PleinEcran>,
    pub sorties: Box<dyn SortiesAudio>,
    pub bip: Box<dyn Bip>,
}

impl Systeme {
    pub fn reel() -> Systeme {
        use retro_audio::systeme;
        Systeme {
            capture: systeme::capture(),
            premier_plan: systeme::premier_plan(),
            appel: systeme::detecteur_appel(),
            plein_ecran: systeme::plein_ecran(),
            sorties: systeme::sorties(),
            bip: systeme::bip(),
        }
    }
}

/// Les listes affichées. On garde les mêmes modèles d'un bout à l'autre et on ne change
/// que les lignes qui diffèrent : remplacer tout un modèle recréerait ses éléments à
/// l'écran, et un fader en train d'être glissé serait lâché.
struct Modeles {
    gains: Rc<VecModel<f32>>,
    bandes: Rc<VecModel<f32>>,
    journal: Rc<VecModel<LigneJournal>>,
    profils: Rc<VecModel<LigneProfil>>,
    versions: Rc<VecModel<LigneVersion>>,
    zones: Rc<VecModel<LigneZone>>,
    applis: Rc<VecModel<LigneAppli>>,
    controles: Rc<VecModel<LigneControle>>,
    raccourcis: Rc<VecModel<LigneRaccourci>>,
    autoeq: Rc<VecModel<LigneAutoEq>>,
}

fn maj<T: Clone + PartialEq + 'static>(modele: &Rc<VecModel<T>>, lignes: Vec<T>) {
    if modele.row_count() != lignes.len() {
        modele.set_vec(lignes);
        return;
    }
    for (i, ligne) in lignes.into_iter().enumerate() {
        if modele.row_data(i).as_ref() != Some(&ligne) {
            modele.set_row_data(i, ligne);
        }
    }
}

/// Les cassettes de l'écran « Tester », dans la langue demandée.
pub fn modeles_de_modes(langue: Langue) -> ModelRc<LigneMode> {
    ModelRc::new(VecModel::from(
        textes::modes(langue)
            .into_iter()
            .map(|mode| LigneMode {
                id: mode.id.into(),
                groupe: mode.groupe.into(),
                nom: mode.nom.into(),
                description: mode.description.into(),
                source: mode.source.into(),
                duels: mode.duels.into(),
                duree: mode.duree.into(),
                disponible: mode.disponible,
            })
            .collect::<Vec<_>>(),
    ))
}

fn textes_modele(lignes: Vec<String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        lignes
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
}

/// L'heure du système, en millisecondes depuis 1970.
fn horloge_systeme() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Tout l'état vivant de l'appli, côté interface.
pub struct Pont {
    pub controleur: Controleur,
    systeme: Systeme,
    raccourcis: Raccourcis,
    barre: Option<Barre>,
    mini: Option<Mini>,
    modeles: Modeles,
    mesure: Mesure,
    trames: Option<Receiver<Trame>>,
    /// Avance ajoutée à l'heure du système : seul un scénario d'essai s'en sert, pour
    /// faire défiler un test sans attendre.
    avance_ms: u64,
    images: u64,
    annonce_jusqua_ms: u64,
    choix: Option<ChoixTest>,
    controles: Vec<autotest::Controle>,
    dossier_moteur: Option<PathBuf>,
    sortie_signalee: Option<String>,
    dernier_premier_plan: Option<String>,
    dernier_appel: bool,
    /// La langue dont les textes sont en ce moment dans l'interface : tant qu'elle diffère de
    /// celle du contrôleur, `synchroniser` rechange les textes (voir `appliquer_langue`).
    langue_appliquee: Option<Langue>,
}

impl Pont {
    pub fn nouveau(
        controleur: Controleur,
        systeme: Systeme,
        raccourcis: Raccourcis,
        mini: Option<Mini>,
        dossier_moteur: Option<PathBuf>,
    ) -> Pont {
        let modele = || Rc::new(VecModel::default());
        let barre = Barre::nouvelle(controleur.vue_appli().egaliseur_actif, controleur.langue());
        Pont {
            controleur,
            systeme,
            raccourcis,
            barre,
            mini,
            modeles: Modeles {
                gains: Rc::new(VecModel::from(vec![0.0; 10])),
                bandes: Rc::new(VecModel::from(vec![0.0; BANDES])),
                journal: Rc::new(VecModel::default()),
                profils: modele(),
                versions: Rc::new(VecModel::default()),
                zones: Rc::new(VecModel::default()),
                applis: Rc::new(VecModel::default()),
                controles: Rc::new(VecModel::default()),
                raccourcis: Rc::new(VecModel::default()),
                autoeq: Rc::new(VecModel::default()),
            },
            mesure: Mesure::new(BANDES),
            trames: None,
            avance_ms: 0,
            images: 0,
            annonce_jusqua_ms: 0,
            choix: None,
            controles: Vec::new(),
            dossier_moteur,
            sortie_signalee: None,
            dernier_premier_plan: None,
            dernier_appel: false,
            langue_appliquee: None,
        }
    }

    pub fn maintenant(&self) -> u64 {
        horloge_systeme() + self.avance_ms
    }

    /// Y a-t-il une icône dans la barre des tâches pour retrouver l'appli une fois cachée ?
    fn peut_se_reduire(&self) -> bool {
        self.barre.is_some() && self.controleur.fermer_reduit()
    }

    // ----- Du contrôleur vers l'interface -----

    /// Recopie tout ce que le contrôleur montre. Appelé après chaque action de l'utilisateur.
    pub fn synchroniser(&mut self, fenetre: &FenetrePrincipale) {
        let t = self.maintenant();
        let langue = self.controleur.langue();
        if self.langue_appliquee != Some(langue) {
            self.appliquer_langue(fenetre, langue);
        }
        let c = &self.controleur;

        let vue = c.vue_appli();
        let appli = fenetre.global::<Appli>();
        appli.set_profil_actif(vue.profil_actif.into());
        appli.set_egaliseur_actif(vue.egaliseur_actif);
        appli.set_etat_moteur(vue.etat_moteur.code());
        appli.set_moteur_autorise(vue.moteur_autorise);
        appli.set_test_en_cours(vue.test_en_cours);
        if let Some(barre) = &self.barre {
            barre.regler_egaliseur(vue.egaliseur_actif);
        }

        let vue = c.vue_egaliseur();
        let egaliseur = fenetre.global::<Egaliseur>();
        maj(
            &self.modeles.gains,
            vue.gains.iter().map(|&g| g as f32).collect(),
        );
        egaliseur.set_preampli_db(vue.preampli_db as f32);
        egaliseur.set_preampli_auto(vue.preampli_auto);
        let chemin: SharedString = chemin_svg(&vue.courbe).into();
        egaliseur.set_courbe(chemin.clone());
        fenetre.global::<Visualiseur>().set_courbe(chemin);
        egaliseur.set_filtres_supplementaires(vue.filtres_supplementaires as i32);
        egaliseur.set_modifie(vue.modifie);
        egaliseur.set_profil(vue.profil.into());

        let profils = fenetre.global::<Profils>();
        maj(
            &self.modeles.profils,
            c.vue_profils()
                .into_iter()
                .map(|p| LigneProfil {
                    id: p.id.into(),
                    nom: p.nom.into(),
                    actif: p.actif,
                    versions: p.versions as i32,
                    date: p.date.into(),
                    resume: p.resume.into(),
                    courbe: chemin_svg(&p.courbe).into(),
                })
                .collect(),
        );
        maj(
            &self.modeles.versions,
            c.vue_versions(&profils.get_choisi())
                .into_iter()
                .map(|v| LigneVersion {
                    numero: v.numero as i32,
                    date: v.date.into(),
                    origine: v.origine.into(),
                    active: v.active,
                    courbe: chemin_svg(&v.courbe).into(),
                })
                .collect(),
        );

        self.synchroniser_autoeq(fenetre);

        let resultat = fenetre.global::<Resultat>();
        match c.vue_resultat() {
            None => resultat.set_disponible(false),
            Some(vue) => {
                resultat.set_disponible(true);
                resultat.set_titre(vue.titre.into());
                resultat.set_confiance(vue.confiance);
                resultat.set_resume(vue.resume.into());
                resultat.set_controles(vue.controles.into());
                resultat.set_verification(vue.verification.into());
                resultat.set_courbe_reference(chemin_svg(&vue.courbe_reference).into());
                resultat.set_courbe_nouvelle(chemin_svg(&vue.courbe_nouvelle).into());
                resultat.set_active(vue.active);
                resultat.set_recommandee(vue.recommandee);
                resultat.set_preampli_db(vue.preampli_db as f32);
                maj(
                    &self.modeles.zones,
                    vue.zones
                        .into_iter()
                        .map(|z| LigneZone {
                            nom: z.nom.into(),
                            plage: z.plage.into(),
                            gain_db: z.gain_db as f32,
                            change: z.change,
                            confiance: z.confiance,
                            detail: z.detail.into(),
                        })
                        .collect(),
                );
                maj(
                    &self.modeles.applis,
                    vue.applis
                        .into_iter()
                        .map(|a| LigneAppli {
                            nom: a.nom.into(),
                            duels: a.duels as i32,
                            preference: a.preference.into(),
                        })
                        .collect(),
                );
            }
        }

        let vue = c.vue_reglages();
        let reglages = fenetre.global::<Reglages>();
        reglages.set_langue(vue.langue.into());
        appli.set_langue(vue.langue.into());
        reglages.set_rail(vue.rail);
        reglages.set_fermer_reduit(vue.fermer_reduit);
        reglages.set_mini_fenetre(vue.mini_fenetre);
        reglages.set_bip(vue.bip);
        reglages.set_pause_appels(vue.pause_appels);
        reglages.set_intervalle_s(vue.intervalle_s as i32);
        let conflits = self.raccourcis.conflits_par_ligne();
        maj(
            &self.modeles.raccourcis,
            textes::raccourcis(langue)
                .into_iter()
                .zip(conflits)
                .map(|((touches, action), conflit)| LigneRaccourci {
                    touches: touches.into(),
                    action: action.into(),
                    conflit,
                })
                .collect(),
        );

        self.synchroniser_nouveau_test(fenetre);
        self.synchroniser_test(fenetre, t);
    }

    /// Met l'interface dans la langue du contrôleur : les textes des fichiers `.slint` (la
    /// traduction groupée avec l'appli), puis tout ce que Rust fabrique et ne donne qu'une
    /// fois (cassettes de l'écran « Tester », presets, menu de la barre des tâches,
    /// autotest). Le reste est recopié par `synchroniser`, juste après.
    fn appliquer_langue(&mut self, fenetre: &FenetrePrincipale, langue: Langue) {
        let premiere_fois = self.langue_appliquee.is_none();
        self.langue_appliquee = Some(langue);
        // Le français est écrit dans les fichiers .slint : c'est la langue « d'origine », que
        // Slint désigne par une chaîne vide. L'anglais est le dossier `lang/en`.
        let code = match langue {
            Langue::Fr => "",
            Langue::En => "en",
        };
        if let Err(erreur) = slint::select_bundled_translation(code) {
            eprintln!("langue : {erreur}");
        }
        fenetre.global::<Test>().set_modes(modeles_de_modes(langue));
        fenetre
            .global::<Profils>()
            .set_presets(textes_modele(self.controleur.presets()));
        if let Some(barre) = &self.barre {
            barre.regler_langue(langue);
        }
        // Au premier affichage, `brancher` refait déjà l'autotest juste après.
        if premiere_fois {
            return;
        }
        self.autotester(fenetre);
    }

    /// Le panneau « Chercher mon casque » : l'état de la base et les résultats.
    fn synchroniser_autoeq(&self, fenetre: &FenetrePrincipale) {
        let vue = self.controleur.vue_autoeq();
        let autoeq = fenetre.global::<AutoEq>();
        autoeq.set_etat(match vue.etat {
            EtatBaseAutoEq::Absente => 0,
            EtatBaseAutoEq::Prete => 1,
            EtatBaseAutoEq::MiseAJour => 2,
        });
        autoeq.set_import_en_cours(vue.import_en_cours);
        autoeq.set_resume(vue.resume.into());
        autoeq.set_erreur(vue.erreur.into());
        maj(
            &self.modeles.autoeq,
            vue.resultats
                .into_iter()
                .map(|r| LigneAutoEq {
                    detail: textes::detail_resultat_autoeq(
                        r.plateforme.as_deref(),
                        r.type_casque.as_deref(),
                    )
                    .into(),
                    nom: r.nom.into(),
                    source: r.source.into(),
                    hors_ligne: r.hors_ligne,
                })
                .collect(),
        );
    }

    /// Prend la réponse d'un téléchargement AutoEQ terminé, s'il y en a une. Rend vrai quand
    /// l'écran doit être remis à jour.
    fn relever_autoeq(&mut self, fenetre: &FenetrePrincipale, t: u64) -> bool {
        if !self.controleur.autoeq_occupe() {
            return false;
        }
        let id = self.controleur.autoeq_relever(t);
        if self.controleur.autoeq_occupe() {
            return false;
        }
        if let Some(id) = id {
            fenetre.global::<Profils>().set_choisi(id.into());
        }
        true
    }

    fn synchroniser_nouveau_test(&self, fenetre: &FenetrePrincipale) {
        let vue = self.controleur.vue_nouveau_test();
        let test = fenetre.global::<Test>();
        test.set_reference(vue.reference.into());
        test.set_source(vue.source.into());
        test.set_mode(vue.mode.into());
        test.set_duree(vue.duree.into());
        // Ces deux listes ne changent presque jamais : on ne remplace le modèle que s'il le faut.
        let memes = |modele: ModelRc<SharedString>, lignes: &[String]| {
            modele.row_count() == lignes.len()
                && modele.iter().zip(lignes).all(|(a, b)| a.as_str() == b)
        };
        if !memes(test.get_zones(), &vue.zones) {
            test.set_zones(textes_modele(vue.zones));
        }
        if !memes(test.get_profils(), &vue.profils) {
            test.set_profils(textes_modele(vue.profils));
        }
    }

    /// L'écran Duels et la mini-fenêtre : recopiés à chaque image pendant un test.
    fn synchroniser_test(&mut self, fenetre: &FenetrePrincipale, t: u64) {
        let vue = self.controleur.vue_duels(t);
        pousser_duels(&fenetre.global::<Test>(), &vue);
        fenetre
            .global::<Appli>()
            .set_test_en_cours(self.controleur.test_en_cours());
        maj(
            &self.modeles.journal,
            vue.journal
                .iter()
                .map(|l| LigneJournal {
                    titre: l.titre.as_str().into(),
                    cote: l.cote,
                    detail: l.detail.as_str().into(),
                })
                .collect(),
        );
        if let Some(mini) = &mut self.mini {
            pousser_duels(&mini.fenetre().global::<Test>(), &vue);
            mini.montrer(self.controleur.mini_fenetre_voulue());
        }
    }

    /// Refait l'autotest et l'affiche.
    pub fn autotester(&mut self, fenetre: &FenetrePrincipale) {
        let sortie = self.systeme.sorties.sortie_par_defaut();
        self.raccourcis.sonder_touches_de_test();
        let conflits = self.raccourcis.conflits();
        let (controles, bilan) = autotest::controles(&autotest::Contexte {
            sortie: sortie.as_ref().map(|s| s.nom.as_str()),
            etat_moteur: self.controleur.etat_moteur().code(),
            moteur_autorise: self.controleur.moteur_autorise(),
            dossier_moteur: self.dossier_moteur.as_deref(),
            conflits: &conflits,
            raccourcis_disponibles: self.raccourcis.disponibles(),
            langue: self.controleur.langue(),
        });
        let diagnostic = fenetre.global::<Diagnostic>();
        diagnostic.set_bilan(bilan.into());
        diagnostic.set_en_cours(false);
        diagnostic.set_dossier_moteur(
            self.dossier_moteur
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_default()
                .into(),
        );
        diagnostic.set_dossier_donnees(
            self.controleur
                .dossier_donnees()
                .display()
                .to_string()
                .into(),
        );
        maj(
            &self.modeles.controles,
            controles
                .iter()
                .map(|c| LigneControle {
                    titre: c.titre.as_str().into(),
                    detail: c.detail.as_str().into(),
                    etat: match c.etat {
                        autotest::Etat::Ok => EtatControle::Ok,
                        autotest::Etat::Attention => EtatControle::Attention,
                        autotest::Etat::Erreur => EtatControle::Erreur,
                    },
                    action: c.action.as_str().into(),
                })
                .collect(),
        );
        self.controles = controles;
        self.synchroniser_sorties(fenetre);
    }

    /// Les sorties audio vues par le système, et l'alerte « nouvelle sortie ».
    fn synchroniser_sorties(&mut self, fenetre: &FenetrePrincipale) {
        let sorties = self.systeme.sorties.sorties();
        let defaut = sorties.iter().find(|s| s.par_defaut);
        let nom = defaut.map(|s| s.nom.clone()).unwrap_or_default();
        let appli = fenetre.global::<Appli>();
        if appli.get_sortie_audio() != nom.as_str() {
            appli.set_sortie_audio(nom.as_str().into());
            fenetre
                .global::<Visualiseur>()
                .set_source(nom.as_str().into());
            let reglages = fenetre.global::<Reglages>();
            reglages.set_sortie(nom.as_str().into());
            reglages.set_sorties(textes_modele(
                sorties.iter().map(|s| s.nom.clone()).collect(),
            ));
        }
        // Une sortie par défaut où le moteur n'est pas installé : on le dit une fois.
        let nouvelle = autotest::nouvelle_sortie(defaut, &self.controleur.sorties_moteur());
        if nouvelle.is_some() && nouvelle != self.sortie_signalee {
            if let Some(nom) = &nouvelle {
                let texte = textes::annonce_nouvelle_sortie(nom, self.controleur.langue());
                self.annoncer(fenetre, &texte, false);
            }
        }
        self.sortie_signalee = nouvelle;
    }

    fn annoncer(&mut self, fenetre: &FenetrePrincipale, texte: &str, erreur: bool) {
        let appli = fenetre.global::<Appli>();
        appli.set_annonce(texte.into());
        appli.set_annonce_erreur(erreur);
        let duree = if erreur {
            DUREE_ERREUR_MS
        } else {
            DUREE_ANNONCE_MS
        };
        self.annonce_jusqua_ms = horloge_systeme() + duree;
    }

    /// Affiche le message que le contrôleur a laissé, puis l'efface après quelques secondes.
    fn relever_annonce(&mut self, fenetre: &FenetrePrincipale) {
        if let Some(annonce) = self.controleur.prendre_annonce() {
            self.annoncer(fenetre, &annonce.texte, annonce.erreur);
        } else if self.annonce_jusqua_ms != 0 && horloge_systeme() > self.annonce_jusqua_ms {
            self.annonce_jusqua_ms = 0;
            fenetre.global::<Appli>().set_annonce("".into());
        }
        for _ in 0..self.controleur.prendre_bips() {
            self.systeme.bip.bip();
        }
    }

    // ----- Le minuteur : 30 fois par seconde -----

    fn image(&mut self, fenetre: &FenetrePrincipale) {
        self.images += 1;
        let t = self.maintenant();
        self.ecouter(fenetre);

        // Les raccourcis globaux.
        let mut actions = self
            .raccourcis
            .regler_touches_de_test(self.controleur.touches_de_test_actives());
        actions.extend(
            self.raccourcis
                .actions(t, self.controleur.reecoute_possible()),
        );
        let agi = !actions.is_empty();
        for action in actions {
            self.executer(fenetre, action, t);
        }

        // L'icône de la barre des tâches.
        let demandes = self
            .barre
            .as_ref()
            .map(Barre::evenements)
            .unwrap_or_default();
        for demande in demandes {
            match demande {
                EvenementBarre::Ouvrir => montrer_fenetre(fenetre),
                EvenementBarre::BasculerEgaliseur => {
                    self.controleur.basculer_egaliseur();
                    self.synchroniser(fenetre);
                }
                EvenementBarre::Quitter => {
                    let _ = slint::quit_event_loop();
                }
            }
        }

        // Une fois par seconde : ce qui coûte un appel au système.
        if self.images % 30 == 0 {
            self.observer(fenetre, t);
        }

        self.relever_choix(fenetre);
        let autoeq_fini = self.relever_autoeq(fenetre, t);
        if agi || autoeq_fini {
            self.synchroniser(fenetre);
        } else if self.controleur.test_en_cours() {
            self.controleur.tic(t);
            let fini = !self.controleur.test_en_cours();
            if fini {
                // Le test vient de se terminer : le résultat et la nouvelle version sont là.
                self.synchroniser(fenetre);
            } else {
                self.synchroniser_test(fenetre, t);
            }
        }
        self.relever_annonce(fenetre);
    }

    /// Vide le canal du son capté et met à jour le vumètre et le spectre.
    fn ecouter(&mut self, fenetre: &FenetrePrincipale) {
        let visualiseur = fenetre.global::<Visualiseur>();
        if self.trames.is_none() && self.images % 150 == 1 {
            // Pas de capture (aucune sortie, ou elle a changé) : on réessaie toutes les 5 s.
            self.trames = self.systeme.capture.demarrer().ok();
            visualiseur.set_capture_active(self.trames.is_some());
        }
        let Some(trames) = &self.trames else { return };
        let mut recu = false;
        loop {
            match trames.try_recv() {
                Ok(trame) => {
                    self.mesure.ajouter(&trame);
                    recu = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.trames = None;
                    visualiseur.set_capture_active(false);
                    break;
                }
            }
        }
        if !recu {
            return;
        }
        let niveau = self.mesure.niveau_db();
        visualiseur.set_niveau_db(niveau);
        visualiseur.set_crete_db(self.mesure.crete_db());
        fenetre.global::<Appli>().set_niveau_db(niveau);
        maj(&self.modeles.bandes, self.mesure.bandes().to_vec());
        if self.images % 6 == 0 {
            visualiseur.set_volume_lufs(niveau + ecart_pondere_db(self.mesure.spectre()));
        }
    }

    /// Ce qu'on regarde une fois par seconde : appli au premier plan, appel, plein écran,
    /// spectre pour la compensation de volume, sorties audio.
    fn observer(&mut self, fenetre: &FenetrePrincipale, t: u64) {
        self.controleur
            .signaler_plein_ecran(self.systeme.plein_ecran.exclusif());
        if self.controleur.test_en_cours() {
            let appli = self.systeme.premier_plan.appli();
            if appli != self.dernier_premier_plan {
                fenetre
                    .global::<Test>()
                    .set_appli(appli.clone().unwrap_or_default().into());
                self.controleur.signaler_premier_plan(appli.clone(), t);
                self.dernier_premier_plan = appli;
            }
            let appel = self.systeme.appel.appel_en_cours();
            if appel != self.dernier_appel {
                self.controleur.signaler_appel(appel, t);
                self.dernier_appel = appel;
            }
            // Un silence ne dit rien du son qui joue : on garde alors le dernier spectre utile.
            if self.mesure.niveau_db() > -70.0 {
                self.controleur.maj_spectre(self.mesure.spectre().clone());
            }
        } else {
            self.dernier_appel = false;
            self.dernier_premier_plan = None;
        }
        if self.images % 150 == 0 {
            self.synchroniser_sorties(fenetre);
            fenetre
                .global::<Appli>()
                .set_etat_moteur(self.controleur.etat_moteur().code());
        }
    }

    /// Les choix de l'écran « Nouveau test » sont des propriétés que l'interface modifie
    /// seule : on les relit, et on met le résumé à jour quand ils changent.
    fn relever_choix(&mut self, fenetre: &FenetrePrincipale) {
        let test = fenetre.global::<Test>();
        let choix = ChoixTest {
            mode: test.get_mode_choisi().into(),
            precis: test.get_precis(),
            intervalle_s: test.get_intervalle_s().max(0) as u32,
            zone: test.get_zone_choisie().max(0) as usize,
            autre_profil: test.get_autre_profil().into(),
        };
        if self.choix.as_ref() != Some(&choix) {
            self.controleur.choisir_test(choix.clone());
            self.choix = Some(choix);
            self.synchroniser_nouveau_test(fenetre);
        }
    }

    /// Exécute ce qu'un raccourci global demande.
    fn executer(&mut self, fenetre: &FenetrePrincipale, action: Action, t: u64) {
        let c = &mut self.controleur;
        match action {
            Action::Repondre(reponse) => c.repondre(reponse, t),
            Action::Reecouter { cote, appuye } => c.reecouter(cote, appuye, t),
            Action::PlusTard => c.plus_tard(t),
            Action::Annuler => c.annuler_reponse(t),
            Action::DemarrerOuReprendre => {
                self.relever_choix(fenetre);
                if self.controleur.demarrer_ou_reprendre(t) {
                    fenetre.global::<Appli>().set_ecran(Ecran::Duels);
                }
            }
            Action::ActiverProfil(rang) => c.activer_profil_numero(rang),
            Action::BasculerEgaliseur => c.basculer_egaliseur(),
        }
    }

    // ----- Scénario d'essai -----

    /// Joue une suite d'actions écrite dans la variable `RETRO_EQ_SCENARIO`, pour vérifier
    /// l'appli sans souris ni attente (captures d'écran, essais sous Linux). Sans cette
    /// variable, rien de tout ceci ne s'exécute. Les étapes, séparées par `;` :
    ///
    /// - `consentir` : le bouton « Continuer » du premier lancement ;
    /// - `fader:<bande>:<dB>`, `enregistrer`, `annuler` : l'égaliseur ;
    /// - `mode:<id>`, `test` : choisit un mode puis lance un test (bascule toutes les 10 s) ;
    /// - `avance:<secondes>` : fait passer le temps du test, par pas de 5 s ;
    /// - `repondre:a|b|pareil`, `pause`, `reprendre`, `arreter` ;
    /// - `finir` ou `finir:<n>` : écoute et répond (A, B, Pareil à tour de rôle) jusqu'à la
    ///   fin du test, ou à n duels ; `finir::graves` répond comme quelqu'un qui préfère
    ///   plus de graves et entend bien que les duels de contrôle sont identiques ;
    /// - `activer`, `revenir` : l'écran Résultat ;
    /// - `importer:<chemin>`, `exporter:<format>` ;
    /// - `langue:fr|en` : change la langue comme le fait le choix des Réglages (les textes sont
    ///   rechangés à la fin du scénario).
    pub fn jouer_scenario(&mut self, fenetre: &FenetrePrincipale, scenario: &str) {
        for etape in scenario.split(';').map(str::trim).filter(|e| !e.is_empty()) {
            let mut morceaux = etape.splitn(3, ':');
            let (commande, a, b) = (
                morceaux.next().unwrap_or_default(),
                morceaux.next().unwrap_or_default(),
                morceaux.next().unwrap_or_default(),
            );
            let t = self.maintenant();
            match commande {
                "consentir" => self.controleur.autoriser_moteur(),
                "fader" => {
                    if let (Ok(bande), Ok(gain)) = (a.parse(), b.parse()) {
                        self.controleur.regler_bande(bande, gain);
                    }
                }
                "enregistrer" => self.controleur.enregistrer_egaliseur(t),
                "annuler" => self.controleur.annuler_egaliseur(),
                "mode" => fenetre.global::<Test>().set_mode_choisi(a.into()),
                "test" => {
                    fenetre.global::<Test>().set_intervalle_s(10);
                    self.relever_choix(fenetre);
                    self.controleur.demarrer_test(t);
                }
                "avance" => self.avancer(a.parse().unwrap_or(0)),
                "repondre" => {
                    let reponse = match a {
                        "a" => Reponse::A,
                        "b" => Reponse::B,
                        _ => Reponse::Pareil,
                    };
                    self.controleur.repondre(reponse, t);
                }
                "pause" => self.controleur.mettre_en_pause(t),
                "reprendre" => self.controleur.reprendre(t),
                "arreter" => self.controleur.arreter_test(t),
                "finir" => {
                    let limite: u32 = a.parse().unwrap_or(u32::MAX);
                    let mut duels = 0;
                    while self.controleur.test_en_cours() && duels < limite && duels < 200 {
                        self.avancer(61);
                        let reponse = if b == "graves" {
                            self.reponse_qui_aime_les_graves()
                        } else {
                            [Reponse::A, Reponse::B, Reponse::Pareil][duels as usize % 3]
                        };
                        let t = self.maintenant();
                        self.controleur.repondre(reponse, t);
                        duels += 1;
                    }
                }
                "activer" => self.controleur.activer_resultat(),
                "revenir" => self.controleur.revenir_reference(),
                "importer" => {
                    self.controleur.importer(std::path::Path::new(a), t);
                }
                "exporter" => self.controleur.exporter_profil_actif(a),
                "langue" => self.controleur.choisir_langue(a),
                inconnue => eprintln!("scénario : étape inconnue « {inconnue} »"),
            }
        }
        self.synchroniser(fenetre);
        self.relever_annonce(fenetre);
    }

    /// Pour le scénario `finir::graves` seulement : la réponse d'un auditeur idéal qui
    /// préfère plus de graves. Elle triche en lisant le plan dans le journal du test, ce
    /// que l'interface ne fait jamais (le test doit rester à l'aveugle) : c'est un outil
    /// pour fabriquer un résultat lisible sur une capture, rien d'autre.
    fn reponse_qui_aime_les_graves(&self) -> Reponse {
        use retro_core::{Entree, LectureJournal, Sens, Sujet, Zone};
        let dossier = self.controleur.dossier_donnees().join("tests");
        let dernier = std::fs::read_dir(dossier)
            .ok()
            .and_then(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).max());
        let Some(Ok(lecture)) = dernier.map(LectureJournal::lire) else {
            return Reponse::Pareil;
        };
        let mut duels = Vec::new();
        for entree in lecture.entrees {
            match entree {
                Entree::TestDemarre { plan, .. } => duels = plan,
                Entree::DuelVerificationAjoute { duel, .. } => duels.push(duel),
                Entree::DuelRetire { duel, prevu, .. } => {
                    if let Some(place) = duels.get_mut(duel as usize) {
                        *place = prevu;
                    }
                }
                _ => {}
            }
        }
        let courant = self.controleur.vue_duels(self.maintenant()).duel as usize;
        let Some(duel) = courant.checked_sub(1).and_then(|i| duels.get(i)) else {
            return Reponse::Pareil;
        };
        let variante = match duel.sujet {
            Sujet::Zone {
                zone: Zone::Graves,
                sens,
            } => sens == Sens::Plus,
            Sujet::Verification => true,
            _ => return Reponse::Pareil,
        };
        match (variante, duel.reference_est_a) {
            (true, true) | (false, false) => Reponse::B,
            _ => Reponse::A,
        }
    }

    /// Fait passer `secondes` de temps de test, par pas de 5 s (un passage dure au moins 10 s).
    fn avancer(&mut self, secondes: u64) {
        let mut reste = secondes * 1000;
        while reste > 0 {
            let pas = reste.min(5_000);
            self.avance_ms += pas;
            reste -= pas;
            let t = self.maintenant();
            self.controleur.tic(t);
        }
    }
}

/// Écart entre le niveau brut et le niveau pondéré K (celui de la norme de mesure du volume
/// perçu), d'après le spectre : une estimation, pas une mesure LUFS normalisée.
fn ecart_pondere_db(spectre: &retro_dsp::Spectre) -> f32 {
    let (mut pondere, mut brut) = (0.0, 0.0);
    for (&f, &p) in spectre.frequences_hz.iter().zip(&spectre.puissances) {
        pondere += p * 10f64.powf(retro_dsp::ponderation_k_db(f) / 10.0);
        brut += p;
    }
    if pondere > 0.0 && brut > 0.0 {
        (10.0 * (pondere / brut).log10()) as f32
    } else {
        0.0
    }
}

fn pousser_duels(test: &Test, vue: &VueDuels) {
    test.set_etat(match vue.etat {
        EtatTestVue::Pret => crate::EtatTest::Pret,
        EtatTestVue::Ecoute => crate::EtatTest::Ecoute,
        EtatTestVue::AttenteReponse => crate::EtatTest::AttenteReponse,
        EtatTestVue::Pause => crate::EtatTest::Pause,
        EtatTestVue::PauseAppel => crate::EtatTest::PauseAppel,
        EtatTestVue::Termine => crate::EtatTest::Termine,
    });
    test.set_duel(vue.duel as i32);
    test.set_total(vue.total as i32);
    test.set_cote_actif(vue.cote_actif);
    test.set_passages_a(i32::from(vue.passages_a));
    test.set_passages_b(i32::from(vue.passages_b));
    test.set_avancement(vue.avancement as f32);
    test.set_compte_a_rebours(vue.compte_a_rebours.as_str().into());
    test.set_suite(vue.suite.as_str().into());
    test.set_pause_conseillee(vue.pause_conseillee);
    test.set_pause_sans_reponse(vue.pause_sans_reponse);
}

/// Ramène la fenêtre principale devant, depuis l'icône de la barre des tâches.
fn montrer_fenetre(fenetre: &FenetrePrincipale) {
    use i_slint_backend_winit::WinitWindowAccessor;
    let _ = fenetre.show();
    fenetre.window().set_minimized(false);
    fenetre.window().with_winit_window(|w| w.focus_window());
}

/// Ouvre un lien ou un dossier avec l'appli par défaut du système. Seuls les liens web et
/// les pages de réglages de Windows sont acceptés : rien d'autre ne doit pouvoir être lancé.
fn ouvrir_lien(lien: &str) {
    if ["https://", "http://", "ms-settings:"]
        .iter()
        .any(|debut| lien.starts_with(debut))
    {
        let _ = open::that_detached(lien);
    }
}

/// Demande un fichier à importer. Sous Windows, la boîte « Ouvrir » du système. Ailleurs,
/// il n'y a pas de boîte de dialogue : le chemin vient de la variable `RETRO_EQ_IMPORT`
/// (essais seulement).
fn choisir_fichier(langue: Langue) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        rfd::FileDialog::new()
            .add_filter(textes::filtre_fichiers(langue), &["peace", "txt"])
            .pick_file()
    }
    #[cfg(not(windows))]
    {
        let _ = langue;
        std::env::var_os("RETRO_EQ_IMPORT").map(PathBuf::from)
    }
}

/// Branche l'interface sur le pont, lance le minuteur, et rend le pont partagé.
///
/// Le minuteur rendu doit être gardé en vie par l'appelant : le lâcher l'arrête.
pub fn brancher(fenetre: &FenetrePrincipale, pont: Pont) -> (Rc<RefCell<Pont>>, slint::Timer) {
    // Les modèles sont donnés une fois pour toutes à l'interface.
    {
        let m = &pont.modeles;
        fenetre
            .global::<Egaliseur>()
            .set_gains(m.gains.clone().into());
        fenetre
            .global::<Visualiseur>()
            .set_bandes(m.bandes.clone().into());
        let test = fenetre.global::<Test>();
        test.set_journal(m.journal.clone().into());
        let profils = fenetre.global::<Profils>();
        profils.set_liste(m.profils.clone().into());
        profils.set_versions(m.versions.clone().into());
        if let Some(actif) = pont.controleur.vue_profils().into_iter().find(|p| p.actif) {
            profils.set_choisi(actif.id.into());
        }
        fenetre
            .global::<AutoEq>()
            .set_resultats(m.autoeq.clone().into());
        let resultat = fenetre.global::<Resultat>();
        resultat.set_zones(m.zones.clone().into());
        resultat.set_applis(m.applis.clone().into());
        fenetre
            .global::<Diagnostic>()
            .set_controles(m.controles.clone().into());
        fenetre
            .global::<Reglages>()
            .set_raccourcis(m.raccourcis.clone().into());
        let vue = pont.controleur.vue_reglages();
        fenetre.global::<Appli>().set_rail(vue.rail);
        fenetre
            .global::<Test>()
            .set_intervalle_s(vue.intervalle_s as i32);
    }

    let pont = Rc::new(RefCell::new(pont));

    // `agir` fabrique la fonction à brancher : elle retrouve la fenêtre et le pont, exécute
    // l'action, puis remet l'interface d'accord avec le contrôleur. `try_borrow_mut` : si le
    // pont est déjà en train de travailler (ce qui ne devrait pas arriver), on ignore l'appel
    // au lieu de faire planter l'appli.
    macro_rules! agir {
        (|$p:ident, $f:ident $(, $arg:ident)*| $corps:block) => {{
            let faible = fenetre.as_weak();
            let pont = pont.clone();
            move |$($arg),*| {
                let (Some($f), Ok(mut emprunt)) = (faible.upgrade(), pont.try_borrow_mut()) else {
                    return;
                };
                let $p: &mut Pont = &mut emprunt;
                let $f: &FenetrePrincipale = &$f;
                $corps
                $p.synchroniser($f);
                $p.relever_annonce($f);
            }
        }};
    }

    let appli = fenetre.global::<Appli>();
    appli.on_autoriser_moteur(agir!(|p, f| {
        p.controleur.autoriser_moteur();
        p.autotester(f);
    }));
    appli.on_basculer_egaliseur(agir!(|p, f| {
        p.controleur.basculer_egaliseur();
    }));
    appli.on_ouvrir_lien(|lien| ouvrir_lien(&lien));
    appli.on_fermer(agir!(|p, f| {
        fermer(p, f);
    }));
    {
        let pont = pont.clone();
        let faible = fenetre.as_weak();
        fenetre.window().on_close_requested(move || {
            let reduire = pont
                .try_borrow()
                .map(|p| p.peut_se_reduire())
                .unwrap_or(false);
            if !reduire {
                let _ = slint::quit_event_loop();
            }
            let _ = faible.upgrade();
            slint::CloseRequestResponse::HideWindow
        });
    }

    let egaliseur = fenetre.global::<Egaliseur>();
    egaliseur.on_regler(agir!(|p, f, bande, gain| {
        p.controleur
            .regler_bande(bande.max(0) as usize, f64::from(gain));
    }));
    egaliseur.on_regler_preampli(agir!(|p, f, db| {
        p.controleur.regler_preampli(f64::from(db));
    }));
    egaliseur.on_basculer_preampli_auto(agir!(|p, f| {
        p.controleur.basculer_preampli_auto();
    }));
    egaliseur.on_remettre_a_plat(agir!(|p, f| {
        p.controleur.remettre_a_plat();
    }));
    egaliseur.on_annuler(agir!(|p, f| {
        p.controleur.annuler_egaliseur();
    }));
    egaliseur.on_enregistrer(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.enregistrer_egaliseur(t);
    }));

    let profils = fenetre.global::<Profils>();
    profils.on_choisir(agir!(|p, f, id| {
        f.global::<Profils>().set_choisi(id);
    }));
    profils.on_activer(agir!(|p, f, id| {
        p.controleur.activer_profil(&id);
    }));
    profils.on_activer_version(agir!(|p, f, id, numero| {
        p.controleur.activer_version(&id, numero.max(0) as u32);
    }));
    profils.on_renommer(agir!(|p, f, id, nom| {
        p.controleur.renommer_profil(&id, &nom);
    }));
    profils.on_dupliquer(agir!(|p, f, id| {
        let t = p.maintenant();
        if let Some(nouveau) = p.controleur.dupliquer_profil(&id, t) {
            f.global::<Profils>().set_choisi(nouveau.into());
        }
    }));
    profils.on_creer_depuis_preset(agir!(|p, f, nom| {
        let t = p.maintenant();
        if let Some(nouveau) = p.controleur.creer_depuis_preset(&nom, t) {
            f.global::<Profils>().set_choisi(nouveau.into());
        }
    }));
    profils.on_importer(agir!(|p, f| {
        importer(p, f);
    }));

    // Base AutoEQ : le panneau « Chercher mon casque » (ADR-0013). Le réseau tourne sur un
    // fil de travail ; la réponse est relevée à chaque image (`relever_autoeq`).
    let autoeq = fenetre.global::<AutoEq>();
    autoeq.on_ouvrir(agir!(|p, f| {
        p.controleur.autoeq_ouvrir();
        f.global::<AutoEq>().set_ouvert(true);
    }));
    autoeq.on_fermer(agir!(|p, f| {
        f.global::<AutoEq>().set_ouvert(false);
    }));
    autoeq.on_mettre_a_jour(agir!(|p, f| {
        p.controleur.autoeq_mettre_a_jour();
    }));
    autoeq.on_chercher(agir!(|p, f, requete| {
        p.controleur.autoeq_chercher(&requete);
    }));
    autoeq.on_choisir(agir!(|p, f, rang| {
        let t = p.maintenant();
        if let Some(id) = p.controleur.autoeq_choisir(rang.max(0) as usize, t) {
            f.global::<Profils>().set_choisi(id.into());
        }
    }));
    profils.on_exporter(agir!(|p, f, id, format| {
        p.controleur.exporter_profil(&id, &format);
    }));
    profils.on_tester(agir!(|p, f, id| {
        // La référence d'un test est le profil actif : on l'active d'abord s'il ne l'est pas.
        let deja = p
            .controleur
            .vue_profils()
            .iter()
            .any(|v| v.actif && v.id == id.as_str());
        if !deja {
            p.controleur.activer_profil(&id);
        }
        f.global::<Appli>().set_ecran(Ecran::Tester);
    }));

    let test = fenetre.global::<Test>();
    test.on_demarrer(agir!(|p, f| {
        p.relever_choix(f);
        let t = p.maintenant();
        if p.controleur.demarrer_test(t) {
            f.global::<Appli>().set_ecran(Ecran::Duels);
        }
    }));
    test.on_pause(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.mettre_en_pause(t);
    }));
    test.on_reprendre(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.reprendre(t);
    }));
    test.on_passer(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.passer_duel(t);
    }));
    test.on_recommencer(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.recommencer_duel(t);
    }));
    test.on_arreter(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.arreter_test(t);
        f.global::<Appli>().set_ecran(Ecran::Tester);
    }));
    test.on_repondre(agir!(|p, f, choix| {
        let t = p.maintenant();
        let reponse = match choix {
            1 => Reponse::A,
            2 => Reponse::B,
            _ => Reponse::Pareil,
        };
        p.controleur.repondre(reponse, t);
    }));
    test.on_plus_tard(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.plus_tard(t);
    }));
    test.on_annuler_reponse(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.annuler_reponse(t);
    }));

    let resultat = fenetre.global::<Resultat>();
    resultat.on_activer(agir!(|p, f| {
        p.controleur.activer_resultat();
    }));
    resultat.on_revenir_reference(agir!(|p, f| {
        p.controleur.revenir_reference();
    }));
    resultat.on_exporter(agir!(|p, f, format| {
        p.controleur.exporter_resultat(&format);
    }));
    resultat.on_nouveau_test(agir!(|p, f| {
        f.global::<Appli>().set_ecran(Ecran::Tester);
    }));

    let diagnostic = fenetre.global::<Diagnostic>();
    diagnostic.on_relancer(agir!(|p, f| {
        p.autotester(f);
    }));
    diagnostic.on_agir(agir!(|p, f, rang| {
        match p
            .controles
            .get(rang.max(0) as usize)
            .and_then(|c| c.cible.clone())
        {
            Some(Cible::Lien(lien)) => ouvrir_lien(lien),
            Some(Cible::Dossier(dossier)) => {
                let _ = open::that_detached(dossier);
            }
            None => {}
        }
    }));
    diagnostic.on_ouvrir_dossier(|chemin| {
        // Seulement un dossier qui existe : ce texte vient de l'interface.
        let chemin = PathBuf::from(chemin.as_str());
        if chemin.is_dir() {
            let _ = open::that_detached(chemin);
        }
    });

    let reglages = fenetre.global::<Reglages>();
    reglages.on_choisir_langue(agir!(|p, f, code| {
        p.controleur.choisir_langue(&code);
    }));
    reglages.on_regler_rail(agir!(|p, f, rail| {
        p.controleur.regler_rail(rail);
        f.global::<Appli>().set_rail(rail);
    }));
    reglages.on_regler_fermer_reduit(agir!(|p, f, v| {
        p.controleur.regler_fermer_reduit(v);
    }));
    reglages.on_regler_mini_fenetre(agir!(|p, f, v| {
        p.controleur.regler_mini_fenetre(v);
    }));
    reglages.on_regler_bip(agir!(|p, f, v| {
        p.controleur.regler_bip(v);
    }));
    reglages.on_regler_pause_appels(agir!(|p, f, v| {
        p.controleur.regler_pause_appels(v);
    }));
    reglages.on_regler_intervalle(agir!(|p, f, secondes| {
        p.controleur.regler_intervalle(secondes.max(0) as u32);
        f.global::<Test>()
            .set_intervalle_s(p.controleur.vue_reglages().intervalle_s as i32);
    }));
    reglages.on_importer(agir!(|p, f| {
        importer(p, f);
    }));
    reglages.on_exporter(agir!(|p, f, format| {
        p.controleur.exporter_profil_actif(&format);
    }));
    reglages.on_rendre_son_origine(agir!(|p, f| {
        let t = p.maintenant();
        p.controleur.rendre_son_origine(t);
        p.autotester(f);
    }));

    // Premier affichage, puis le minuteur.
    if let Ok(mut p) = pont.try_borrow_mut() {
        p.synchroniser(fenetre);
        p.autotester(fenetre);
        p.relever_annonce(fenetre);
    }
    let minuteur = slint::Timer::default();
    {
        let faible = fenetre.as_weak();
        let pont = pont.clone();
        minuteur.start(slint::TimerMode::Repeated, PERIODE, move || {
            if let (Some(fenetre), Ok(mut p)) = (faible.upgrade(), pont.try_borrow_mut()) {
                p.image(&fenetre);
            }
        });
    }
    (pont, minuteur)
}

/// Le bouton ✕ : réduire dans la barre des tâches si c'est le réglage (et qu'une icône
/// existe pour revenir), sinon quitter. Dans les deux cas l'égaliseur reste comme il est :
/// le moteur garde la dernière courbe de référence.
fn fermer(pont: &mut Pont, fenetre: &FenetrePrincipale) {
    if pont.peut_se_reduire() {
        let _ = fenetre.hide();
    } else {
        let _ = slint::quit_event_loop();
    }
}

fn importer(pont: &mut Pont, fenetre: &FenetrePrincipale) {
    match choisir_fichier(pont.controleur.langue()) {
        Some(chemin) => {
            let t = pont.maintenant();
            if let Some(id) = pont.controleur.importer(&chemin, t) {
                fenetre.global::<Profils>().set_choisi(id.into());
            }
        }
        None if cfg!(windows) => {} // boîte de dialogue annulée
        None => {
            let texte = textes::annonce_import_indisponible(pont.controleur.langue());
            pont.annoncer(fenetre, texte, true);
        }
    }
}
