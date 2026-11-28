# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

> Note : le MVP est une application de bureau **Windows uniquement**. « web » désigne ici le langage d'interface (fenêtre d'appli, pas d'interface native iOS ou Android). À revoir si la technologie retenue impose l'interface native Windows.

## Stack

Piste retenue : **100 % Rust, sans HTML ni WebView**. Interface : **Slint** retenu (langage d'interface déclaratif `.slint` compilé en Rust, aperçu en direct), utilisé sous GPLv3. **Licence de Retro EQ : GPLv3**, code source public. Les crates prévues (MIT / Apache) sont compatibles. Alternative tout-Rust gardée en réserve : **iced** (MIT, encore expérimental avant 1.0). Briques : `wasapi` (capture loopback et par appli pour le visualiseur), `global-hotkey` (raccourcis F6 à F10), `tray-icon` (barre des tâches), `windows` (fenêtre au premier plan pour la mini-fenêtre, appli active), FFT en Rust pour le spectre. Moteur audio réaliste : Equalizer APO installé en coulisses par l'installateur (GPLv2, distribué à côté, pas mélangé au code) et piloté par ses fichiers texte ; à terme, un moteur maison « Retro EQ APO » écrit en Rust (DLL chargée par audiodg.exe, enregistrée comme Equalizer APO, sans pilote noyau) pourra le remplacer. Pour préparer ce passage, le traitement du son (filtres) vit dès la v1 dans notre propre crate, et l'appli parle à un « moteur » interchangeable (fichiers Equalizer APO d'abord, puis notre APO).

## Users

Utilisateur principal : **Alexis lui-même** (développeur front-end Nuxt et designer UX/UI), possesseur d'un casque Sony INZONE H9 II sur PC Windows. Il veut retrouver un son clair (référence : son ancien ASUS ROG Delta de 1ʳᵉ génération) pour jouer, regarder des vidéos YouTube et du streaming, et parler sur Discord. Un partage à d'autres personnes est possible plus tard, sans engagement.

## Product Purpose

Trouver le réglage d'égaliseur qu'on préfère **à l'oreille**, par duels A/B à l'aveugle, au lieu de régler des curseurs au hasard ou de copier une courbe mesurée qui ne correspond pas à sa tête. Succès : un profil Retro EQ que l'utilisateur garde au quotidien et qu'il préfère à ses réglages précédents. Retro EQ réunit trois outils : tests A/B, égaliseur intégré et visualiseur (niveau estimé en dB, spectre, réglages de base).

## Positioning

Le test se fait **en conditions réelles** : l'égaliseur bascule automatiquement entre deux versions (A et B) pendant que l'utilisateur fait lui-même autre chose sur son PC (jeu, vidéo, Discord). Après 3 passages de chaque version, il répond A, B ou « pareil » par un raccourci, sans quitter son activité. Les outils existants (SoundID, tests en navigateur, tests d'audition) font écouter un extrait dédié, hors usage réel.

## Operating Context

- Windows, casque branché via son dongle. Retro EQ remplace Peace et les autres interfaces : leurs fichiers (`.peace`, `.txt` Equalizer APO, AutoEQ ParametricEQ, Wavelet) se gèrent dans Réglages › Importer et exporter.
- L'égaliseur d'INZONE Hub doit être à plat pendant les tests (sinon il s'ajoute au nôtre) : vérifié par l'autotest.
- Usage pendant des jeux (FPS notamment), des vidéos YouTube / streaming et des appels Discord.
- Le test dure plusieurs minutes à une heure et peut se faire en plusieurs sessions.
- Pendant les tests, A et B sont mises au **même volume perçu** (compensation calculée et vérifiée sur ce qui joue, via la capture du visualiseur) ; au quotidien, préampli automatique (−7 dB pour le profil actuel).
- Bascule A/B en fondu court, sans clic. Autres traitements à couper pendant les tests (vérifiés par l'autotest) : égaliseur et son 360 d'INZONE Hub, son spatial Windows (Windows Sonic, Dolby Atmos), améliorations audio Windows (égalisation du volume).
- La réduction de bruit du casque change son son et ne peut pas être lue par l'appli : on la demande au début du test et le profil la retient.

## Capabilities and Constraints

MVP confirmé :
- Choisir A (un preset existant) et B (variantes automatiques zone par zone, ou un autre preset).
- Bascule automatique A/B à intervalle réglable (10 s à 2 min), 3 passages chacun par duel, ordre tiré au hasard.
- Réponse par raccourcis globaux et mini-fenêtre par-dessus les jeux ; pause, passer et recommencer un duel ; pause automatique pendant les appels Discord ; remise de la référence à la fin.
- Point de départ : presets de base calculés pour le casque, import depuis AutoEQ (recherche du casque dans la base, gardée en local) ou d'un fichier Peace / Equalizer APO.
- Résultat proposé comme nouvelle version du profil : elle s'active d'un clic et l'ancienne reste restaurable ; export possible en Peace, Equalizer APO, AutoEQ, Wavelet.
- Premier lancement avec autotest, puis **duel d'essai** (différence énorme, vérifie de bout en bout que le son change) ; page Diagnostic, page Réglages ; échelle de confiance unique (Indice / Probable / Sûr selon le risque de hasard).
- **Français et anglais dès la v1** (langue de Windows par défaut, choix au premier lancement et dans Réglages › Apparence). Textes traduits avec le système de traduction intégré de Slint ; le français, plus long, sert de cas le plus exigeant pour les mises en page.
- Fiabilité : reprise après plantage ou PC éteint (référence remise au démarrage, réponses gardées, duel interrompu rejoué), détection d'une nouvelle sortie audio (dongle sur un autre port), fermeture de la fenêtre sans couper l'égaliseur (réduction dans la barre des tâches par défaut), désinstallation qui rend le son d'origine.
- Chaque passage note l'appli au premier plan : l'Empreinte montre les préférences par activité (jeu, vidéo, Discord), base des profils par appli.
- Versions de chaque profil (chaque test en crée une, comparables à l'aveugle, restaurables).
- Page À propos et licences (GPLv3, code source, composants tiers dont Equalizer APO) ; installateur signé gratuitement via SignPath Foundation ; vérification des mises à jour sur GitHub Releases.

Contraintes : Windows seulement ; plus de dépendance à Peace ; A et B tirés au hasard (le test reste à l'aveugle même si les lettres sont affichées).

Hors MVP (prévu plus tard) : connexions de services (Spotify, Deezer, YouTube Music, Apple Music, Tidal, fichiers locaux), assistant IA par clé API (Claude, OpenAI, Mistral, modèle local), synchronisation via compte Google, Discord, Sony Sound Connect, export Wavelet. Mac et Linux : non prévus pour le MVP.

## Brand Commitments

- Nom : **Retro EQ** (ex-EQ Duel). Logo : rond avec barres d'égaliseur asymétriques et fentes de coucher de soleil, « EQ » en Anton.
- Direction visuelle retenue : **Station Pilleur**, sombre et rétro-futuriste années 80, avec des **représentations d'appareils audio** (platines cassette A/B, rack à faders, vumètre à aiguille, cassettes pour les tests et les profils, touches de clavier en relief). Navigation : barre de titre à onglets par défaut, rail latéral en option (Réglages › Apparence). Les choix précis relèvent de DESIGN.md.
- Préférence de travail : lors d'une modification importante d'un écran de maquette, conserver l'ancienne version dans une section « Archives ».

## Evidence on Hand

- La maquette est en HTML pour le design seulement : l'appli sera refaite en Slint.
- Maquette en cours : canevas « Maquette – Test A/B d'égaliseur », page « Complet · Station Pilleur (choisi) » (31 écrans, dont la rangée « Ajouts du 24/11 »), pages Logo et Navigation, et Archives.
- Mesures utilisées pour les presets : INZONE H9 II (GadgetryTech, squig.link, avec et sans ANC) et cible Harman 2018 du même banc ; ASUS ROG Delta 1ʳᵉ génération (Reference Audio Analyzer, coussinets cuir et tissu).
- Presets Peace réels : 1 Quotidien, 2 Voix et Discord, 3 Harman (mesuré), 4 ROG Delta précis et variantes 4a à 4d.
- Aucun utilisateur externe, témoignage, chiffre d'usage ni étude : ne rien inventer de ce genre.

## Product Principles

1. **L'oreille décide.** Les mesures servent de point de départ ; le choix final vient des préférences entendues en usage réel.
2. **Ne pas interrompre.** Le test doit se dérouler pendant que l'utilisateur joue ou regarde ; chaque interaction tient en un raccourci.
3. **Comparer à armes égales.** Même préampli, ordre aléatoire, test à l'aveugle.
4. **Réversible.** La référence est toujours récupérable ; rien n'est supprimé (chaque test crée une version).
5. **Honnête sur les limites.** Signaler les approximations (bancs de mesure différents, aigus peu fiables) plutôt que promettre une précision qui n'existe pas.
