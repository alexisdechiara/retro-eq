# Spec du MVP

Source de vérité produit : `PRODUCT.md`. Règles à respecter dans le code : `docs/BRIEF.md`. Décisions : `docs/adr/`. Vocabulaire : `GLOSSARY.md`. Ce fichier fixe le découpage en crates et les interfaces entre elles.

## Workspace

| Crate | Rôle | Dépend de |
|---|---|---|
| `retro-dsp` | Filtres, courbe de réponse, préampli automatique, volume perçu, compensation, analyse d'un signal (RMS, spectre) | `realfft` |
| `retro-core` | Domaine : profils, versions, zones, modes, plan de test, déroulement d'un test, confiance, résultat, stockage, import et export | `retro-dsp`, `serde` |
| `retro-engine` | Trait `Moteur`, implémentation `EqualizerApo`, moteur simulé pour les tests | `retro-dsp` |
| `retro-audio` | Capture loopback, appli au premier plan, détection d'appel, plein écran exclusif. Vraies implémentations sous Windows, simulées ailleurs | `retro-dsp`, `wasapi`, `windows` |
| `retro-app` | Interface Slint, raccourcis globaux, barre des tâches, mini-fenêtre, autotest. Orchestre tout le reste | toutes |

Règles : `retro-dsp` et `retro-core` ne dépendent d'aucune API système autre que les fichiers ; tout le workspace compile et se teste sous Linux (les parties Windows sont derrière `#[cfg(windows)]` avec une version simulée à côté) ; identifiants du domaine en français, sans accents ; `cargo clippy --workspace -- -D warnings` et `cargo fmt` propres ; pas de `unwrap()` hors des tests ; erreurs avec `thiserror`.

## `retro-dsp`

Types (déjà posés dans `crates/retro-dsp/src/courbe.rs`, à ne pas renommer) : `TypeFiltre { Cloche, PlateauBas, PlateauHaut }`, `Filtre { type_filtre, frequence_hz, gain_db, q }`, `Courbe { preampli_db, filtres }`.

À fournir :

- `Filtre::gain_db_a(&self, f_hz: f64) -> f64` : gain du filtre à une fréquence, d'après les coefficients biquad du « Audio EQ Cookbook » de R. Bristow-Johnson, à 48 kHz. Ce sont les formules d'Equalizer APO (`PK`, `LSC`, `HSC`) : nos courbes doivent coïncider avec ce que le moteur fait entendre.
- `Courbe::reponse_db(&self, f_hz) -> f64` : somme des filtres, **sans** le préampli. `Courbe::reponse_totale_db` : avec.
- `Courbe::gain_max_db(&self) -> f64` : maximum de `reponse_db` entre 20 Hz et 20 kHz (grille logarithmique dense, au moins 48 points par octave).
- `Courbe::preampli_auto_db(&self) -> f64` : `-max(0, gain_max_db)`, le plus petit préampli qui évite la saturation. `Courbe::avec_preampli_auto(self) -> Courbe`.
- `frequences_log(n: usize) -> Vec<f64>` : n fréquences de 20 Hz à 20 kHz.
- `Spectre { frequences_hz: Vec<f64>, puissances: Vec<f64> }` (puissance linéaire par point), `Spectre::rose(n)` (bruit rose : puissance égale par octave), `Spectre::divise_par(&self, courbe) -> Spectre`.
- `ponderation_k_db(f_hz) -> f64` : pondération K de la norme ITU-R BS.1770 (les deux filtres de la norme, évalués en fréquence).
- `volume_percu_db(courbe: &Courbe, spectre: &Spectre) -> f64` : variation de volume perçu que la courbe (préampli compris) produit sur ce spectre, en dB : `10·log10( Σ P·K·|H|² / Σ P·K )`.
- `compensation_db(reference, variante, spectre) -> f64` : ce qu'il faut ajouter au préampli de la variante pour égaler la référence.
- `egaliser_volumes(reference, variante, spectre) -> (Courbe, Courbe)` : applique la compensation à la variante, puis le même préampli de sécurité aux deux (aucune des deux ne dépasse 0 dB).
- Module `analyse` : `niveau_rms_db(echantillons: &[f32]) -> f32` (dBFS, plancher −120), `Analyseur::new(taille_fft, frequence_echantillonnage)`, `Analyseur::spectre(&mut self, echantillons: &[f32]) -> Vec<f32>` (fenêtre de Hann, magnitudes en dBFS), `bandes_log(magnitudes_db, fe, n_bandes) -> Vec<f32>` pour l'affichage, `Analyseur::vers_spectre(...) -> Spectre` pour la compensation, `Lissage` exponentiel.
- Module `biquad` : `Biquad::depuis(filtre, fe)` et `traiter(&mut self, x: f32) -> f32`, pour vérifier par les tests que la réponse calculée est bien celle du filtre réel (sinus en entrée, gain mesuré en sortie).

## `retro-core`

### Modèle

```rust
pub enum Zone { SubGraves, Graves, BasMediums, Mediums, Presence, Aigus, Air }   // 20–60, 60–250, 250–500, 500–2k, 2k–5k, 5k–10k, 10k–20k Hz
pub enum Sens { Plus, Moins }
pub struct ReglagesVariantes { pas_initial_db: f64 /*4*/, pas_plancher_db: f64 /*1*/, .. }
pub fn filtre_de_zone(zone: Zone, gain_db: f64) -> Filtre   // PlateauBas 60 Hz Q 0.71 ; PlateauHaut 10 kHz Q 0.71 ; Cloche au centre géométrique, Q = centre / largeur
pub fn variante(reference: &Courbe, zone: Zone, sens: Sens, pas_db: f64) -> Courbe

pub struct Profil { id: String, nom: String, versions: Vec<Version>, active: u32, reduction_bruit: Option<bool> }
pub struct Version { numero: u32, creee_le: String /*RFC 3339*/, courbe: Courbe, origine: Origine, pas_par_zone: [f64; 7] }
pub enum Origine { Preset, Import { format: String }, Manuel, Test { test_id: String } }
```

Une version n'est jamais modifiée ni supprimée. `Profil::ajouter_version`, `Profil::activer(numero)`, `Profil::version_active()`. `courbe_10_bandes(gains: [f64; 10]) -> Courbe` et `Courbe` ↔ faders : les 10 bandes sont des cloches à 31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz, Q 1,41 ; les autres filtres d'une courbe sont des « filtres supplémentaires » que l'égaliseur conserve sans les montrer comme faders.

Presets de départ (`presets()`) : « À plat », plus les presets du H9 II quand Alexis fournira leurs valeurs (voir ticket 14).

### Plan

```rust
pub enum Mode { DuelsEnJeu { precis: bool }, UneSeuleZone { zone: Zone, duels: u32 }, ProfilContreProfil { autre: Courbe, duels: u32 } }
pub enum Sujet { Zone { zone: Zone, sens: Sens }, Controle, AutreProfil, Verification }
pub struct DuelPrevu { sujet: Sujet, reference_est_a: bool, ordre_passages: [Cote; 6] }
pub fn plan(mode: &Mode, version: &Version, historique_zones: &[Zone], hasard: &mut impl Hasard) -> Vec<DuelPrevu>
```

Règles (ADR-0005) : un duel sur six est de contrôle (arrondi, au moins 1 dès 6 duels), jamais le premier ni deux à la suite ; les autres duels sont distribués à tour de rôle entre les couples (zone, sens), zones les moins récemment testées d'abord ; l'ordre des duels est mélangé ; `reference_est_a` est tiré à pile ou face ; `ordre_passages` contient 3 A et 3 B dans un ordre tiré au hasard, sans jamais trois fois de suite le même côté. `Hasard` est un trait (`fn entier(&mut self, borne: u32) -> u32`) avec une implémentation à graine pour les tests et une vraie.

### Déroulement d'un test

Cœur pur, sans horloge ni entrée-sortie : l'appli lui envoie des événements avec l'heure, il rend des effets à exécuter.

```rust
pub struct Deroulement { .. }
pub enum Evenement { Demarrer, Tic, Reponse(Reponse), PlusTard, Annuler, Pause, Reprendre, PasserDuel, RecommencerDuel,
                     Reecouter { cote: Cote, appuye: bool }, AppelEnCours(bool), PremierPlan(String) }
pub enum Reponse { A, Pareil, B }
pub enum Effet { Appliquer { cote: Option<Cote>, courbe: Courbe }, Journaliser(Entree), DemanderReponse, MasquerQuestion, Bip, ConseillerPause, Termine(ResultatTest) }
impl Deroulement {
    pub fn nouveau(test_id, reference: Courbe, plan: Vec<DuelPrevu>, reglages: ReglagesTest, spectre: Spectre) -> Self
    pub fn traiter(&mut self, evt: Evenement, maintenant_ms: u64) -> Vec<Effet>
    pub fn vue(&self, maintenant_ms: u64) -> VueTest          // tout ce que l'interface affiche : duel n/N, côté actif, passages faits par côté, ms avant bascule, état
    pub fn reprendre_depuis(journal: &[Entree], ..) -> Self   // reprise après un arrêt brutal
    pub fn maj_spectre(&mut self, spectre: Spectre)
}
```

Règles :

- États : `Pret`, `EnEcoute { duel, passage }`, `AttenteReponse`, `EnPause { raison }`, `Termine`. Raisons de pause : manuelle, appel, « plus tard ».
- Un duel joue ses 6 passages dans l'ordre prévu, chacun de `intervalle_ms` (10 000 à 120 000, 20 000 par défaut). À chaque début de passage : `Effet::Appliquer` avec la courbe du côté (référence et variante passées par `egaliser_volumes`), et une entrée de journal qui note le côté et l'appli au premier plan.
- Après le 6ᵉ passage : la référence est remise, `DemanderReponse`. Une réponse peut aussi arriver avant la fin des passages : elle clôt le duel.
- `Reecouter { cote, appuye: true }` pendant l'attente applique ce côté ; `appuye: false` remet la référence.
- `Reponse` : journalisée **avant** tout autre effet, puis duel suivant. `Bip` est émis si le réglage « bip » est actif (plein écran exclusif).
- `PlusTard` : met le test en pause, le duel en cours sera rejoué. `Annuler` : annule la dernière réponse et rejoue ce duel. `PasserDuel` : le duel est noté passé, sans réponse. `RecommencerDuel` : recommence ses passages.
- `AppelEnCours(true)` : pause, référence remise ; `false` : reprise au début du passage interrompu.
- Toute sortie de l'écoute (pause, fin, annulation) émet un `Appliquer` de la référence : une variante ne reste jamais active.
- Après 40 minutes d'écoute cumulée sans pause : `ConseillerPause`, une seule fois.
- Dernier duel répondu : en mode `DuelsEnJeu` et `UneSeuleZone`, si le résultat change au moins une zone, un duel `Verification` (référence contre nouvelle version) est ajouté et joué. Puis `Termine(ResultatTest)`.
- `reprendre_depuis` : relit le journal, garde les réponses, rejoue le duel interrompu depuis son premier passage, démarre en pause.

### Confiance et résultat

```rust
pub fn risque_hasard(victoires: u32, total: u32) -> f64    // P(X >= victoires) pour X ~ Binomiale(total, 1/2)
pub enum Confiance { Indice, Probable, Sur }               // Sur : risque < 0,06 ; Probable : risque <= 0,25 ; sinon Indice
pub struct ResultatZone { zone, gain_db: f64, duels: u32, pour_variante: u32, pour_reference: u32, pareil: u32, confiance: Confiance, contradictoire: bool }
pub struct ResultatTest { zones: Vec<ResultatZone>, controles: BilanControles, nouvelle_courbe: Courbe, nouveaux_pas: [f64; 7], verification: Option<Reponse>, confiance: Confiance, par_appli: Vec<PreferenceAppli> }
```

Règles (ADR-0005) : une zone change dans le sens où la variante a gagné plus de duels que la référence ; si elle gagne dans les deux sens, `contradictoire` et aucun changement. `total` du calcul binomial = duels hors « Pareil » dans le sens retenu. Si au moins la moitié des duels de contrôle ont reçu A ou B, la confiance de chaque zone et du test baisse d'un cran. La confiance du test est celle de la vérification quand elle existe (préférée 1 fois sur 1 : Indice), sinon la plus basse des zones changées. `nouvelle_courbe` = référence + filtres de zone retenus (les filtres identiques en type, fréquence et Q sont fusionnés en sommant les gains), avec préampli automatique. `par_appli` : pour chaque appli au premier plan, combien de duels et quelle préférence.

### Stockage (ADR-0007)

`Depot::ouvrir(dossier)` : `profils/<id>.json`, `reglages.json`, `etat-moteur.json`, `tests/<id>.jsonl`. Écritures atomiques. `Journal::ajouter(&Entree)` ajoute une ligne et force l'écriture sur le disque (`sync_data`). `Depot::tests_interrompus()` pour la reprise.

### Import et export (`retro_core::formats`)

- `equalizer_apo` : lit et écrit `Preamp: -6 dB` et `Filter 1: ON PK Fc 1000 Hz Gain -3 dB Q 1.41` (`PK`, `LSC`, `HSC` ; `LS`/`HS` acceptés à la lecture ; lignes `OFF`, commentaires `#` et lignes inconnues ignorées sans erreur, mais comptées dans un rapport).
- `autoeq` : le fichier `ParametricEQ.txt` d'AutoEQ est ce même format ; module qui délègue et nomme le profil d'après le fichier.
- `peace` : lit et écrit les fichiers `.peace` (format INI de Peace). À défaut de spécification publique, implémenter d'après des fichiers réels et documenter ce qui n'est pas géré.
- `wavelet` : export seulement, ligne `GraphicEQ: 20 -1.2; 21 -1.1; …` sur les 127 fréquences d'AutoEQ.
- `detecter_format(contenu, nom_de_fichier)`.

## `retro-engine`

```rust
pub trait Moteur: Send {
    fn appliquer(&mut self, courbe: &Courbe) -> Result<(), ErreurMoteur>;
    fn couper(&mut self) -> Result<(), ErreurMoteur>;           // égaliseur à plat, moteur toujours en place
    fn etat(&self) -> EtatMoteur;                                // Absent, Inactif, Actif
    fn sorties(&self) -> Result<Vec<SortieAudio>, ErreurMoteur>; // nom, identifiant, moteur installé dessus ou non
    fn retirer(&mut self) -> Result<(), ErreurMoteur>;           // rend la configuration d'origine
}
```

`EqualizerApo::nouveau(dossier_config)` : écrit la courbe dans `retro-eq.txt` (écriture atomique) et s'assure que `config.txt` contient la ligne `Include: retro-eq.txt`. Avant sa première modification, `config.txt` est copié en `config.txt.avant-retro-eq` ; les autres lignes actives de `config.txt` sont mises en commentaire avec un préfixe reconnaissable (`# [retro-eq] `) pour qu'elles ne s'ajoutent pas à notre courbe, et `retirer()` rend le fichier d'origine à l'octet près. `EqualizerApo::detecter()` (Windows : registre `HKLM\SOFTWARE\EqualizerAPO`, valeur `InstallPath`). `texte_config(courbe)` est une fonction pure testée. `MoteurSimule` enregistre les appels, pour les tests et pour faire tourner l'appli sans Equalizer APO. Option `fondu` : nombre de paliers intermédiaires écrits entre deux courbes (0 par défaut, à régler après mesure sur le PC).

## `retro-audio`

Traits : `Capture` (`demarrer() -> Receiver<Trame>`, trames mono `f32` avec la fréquence d'échantillonnage), `PremierPlan` (`appli() -> Option<String>`), `DetecteurAppel` (`appel_en_cours() -> bool`), `PleinEcran` (`exclusif() -> bool`). Sous Windows : loopback WASAPI sur la sortie par défaut, `GetForegroundWindow` + nom du processus, session de capture micro active pour `Discord.exe`, `SHQueryUserNotificationState`. Ailleurs : versions simulées (signal de synthèse) pour que l'appli tourne et se teste.

## `retro-app`

Interface Slint fidèle à `maquette/`. Tokens dans un seul fichier `ui/theme.slint`. Polices embarquées (Antonio, Libre Franklin, DM Mono, Anton, sous OFL). Tous les textes dans `@tr(...)`, français en source, anglais dans `lang/en/LC_MESSAGES/retro-app.po`, traductions embarquées à la compilation. Fenêtre sans bordure avec barre de titre à onglets (rail latéral en option). **Deux tailles seulement** (décision d'Alexis) : 1280 × 800, ou agrandie ; pas de redimensionnement libre. Chaque écran est mis au point à 1280 de large et doit rester propre une fois agrandi. Écrans du MVP : Premier lancement et autotest, Tester (cassettes), Nouveau test, Duels, Résultat, Mes profils, Égaliseur, Visualiseur, Réglages, Diagnostic, À propos ; mini-fenêtre ; icône dans la barre des tâches.

Un seul fil d'exécution pour l'interface ; la capture et les raccourcis tournent à part et parlent à l'interface par des canaux. `Controleur` (Rust pur, sans Slint) porte l'état de l'appli et traduit les effets du `Deroulement` en appels au moteur, au journal et à l'interface : il se teste avec `MoteurSimule`.

## Ce qui ne peut pas être vérifié hors du PC d'Alexis

Délai et clic à la bascule d'Equalizer APO, droits d'écriture du dossier `config`, capture loopback avant ou après le moteur, raccourcis en plein écran exclusif, mini-fenêtre au-dessus d'un jeu, menu Snap de Windows 11, détection d'appel Discord, installateur. Chacun a un ticket `ready-for-human`.
