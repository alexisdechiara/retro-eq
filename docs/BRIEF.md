# Brief de départ

> Consigne d'origine d'Alexis, gardée telle quelle. `PRODUCT.md` reste la source de vérité sur le produit ; ce fichier fixe les décisions techniques, les règles produit à respecter dans le code et la méthode de travail.

Tu vas m'aider à construire **Retro EQ**, une appli Windows de bureau écrite en Rust. Lis d'abord `PRODUCT.md` en entier : c'est la source de vérité sur le produit. Le dossier `maquette/` contient les captures de tous les écrans validés (style « Station Pilleur »). Elles servent de référence visuelle, pas de code à copier : la maquette était en HTML pour le design seulement.

## Le produit en bref

Retro EQ trouve le réglage d'égaliseur que je préfère **à l'oreille**, en conditions réelles. Pendant que je joue, regarde une vidéo ou parle sur Discord, l'égaliseur bascule tout seul entre deux versions A et B, tirées au hasard. Après 3 passages de chacune, je réponds A, « Pareil » ou B avec une touche, sans quitter mon jeu. Une série de duels donne un profil que j'active tout de suite. L'appli a aussi un égaliseur 10 bandes, un visualiseur (niveau estimé en dB, spectre) et une page Réglages.

Mon matériel : Sony INZONE H9 II via son dongle USB, PC Windows. Je suis développeur front-end (Nuxt, Tailwind) et designer, **débutant en Rust** : explique tes choix Rust quand ils ne sont pas évidents.

## Décisions déjà prises (ne pas rediscuter sans raison forte)

- **100 % Rust, aucune WebView ni HTML.** Interface en **Slint** (fichiers `.slint`, aperçu en direct). Plan B gardé en réserve : iced.
- **Licence du projet : GPLv3.** N'ajoute que des dépendances compatibles (MIT, Apache 2.0, BSD, GPL). Signale toute dépendance sous une autre licence avant de l'ajouter.
- **Français et anglais dès la v1**, avec le système de traduction de Slint (`@tr(...)`). Aucun texte d'interface en dur hors des fonctions de traduction. Le français est la langue de référence.
- **Moteur audio v1 : Equalizer APO**, installé à côté et piloté par ses fichiers texte (il les recharge tout seul). L'appli parle à un **moteur interchangeable** (un trait Rust), pour pouvoir écrire plus tard notre propre APO en Rust sans toucher à l'interface.
- **Le calcul des filtres vit dans notre propre crate**, testable sans Windows : courbe de réponse, préampli automatique, compensation de volume.
- Crates prévues : `slint`, `wasapi` (capture loopback pour le visualiseur et la mesure de volume), `global-hotkey` (F6 à F10), `tray-icon` (barre des tâches), `windows` (appli au premier plan, fenêtres), `realfft` (spectre).
- Navigation : onglets dans une barre de titre personnalisée par défaut, rail latéral en option.

## Règles produit à respecter dans le code

- **Test à l'aveugle** : A et B sont tirées au hasard à chaque duel. 3 passages de chacune, intervalle réglable de 10 s à 2 min (20 s par défaut).
- **Volume égalisé pendant les tests** : A et B sont mises au même volume perçu, la compensation étant calculée puis vérifiée sur ce qui joue. Au quotidien : préampli automatique (le plus petit qui évite la saturation).
- **Bascule sans clic** : fondu court. À mesurer avec Equalizer APO.
- **Réversible** : la version B ne doit jamais rester active. Après un plantage ou si le PC s'éteint, la référence est remise au démarrage suivant. Chaque réponse est enregistrée dès l'appui sur la touche. Rien n'est supprimé : chaque test crée une nouvelle version du profil.
- **Échelle de confiance unique**, d'après le risque que le résultat soit dû au hasard (calcul binomial) : Indice (avis partagés), Probable (hasard ≤ 1 chance sur 4), Sûr (hasard < 6 %). 2 duels sur 12 sont des **duels de contrôle**, avec deux versions identiques.
- **Raccourcis** : F6 = A, F7 = Pareil, F8 = B, F9 = plus tard, F10 = annuler. Maintenir F6 ou F8 fait réécouter A ou B. Ctrl+Alt+Entrée démarre ou reprend. Ctrl+Alt+1 à 4 choisit un profil, Ctrl+Alt+0 coupe l'égaliseur. Signaler les conflits de touches.
- **Pause automatique** pendant les appels Discord. Pause conseillée après 40 minutes d'écoute.
- **Mini-fenêtre** par-dessus les jeux en plein écran fenêtré. En plein écran exclusif, aucune fenêtre ne peut s'afficher : un bip court confirme alors la réponse.
- Chaque passage enregistre **l'appli au premier plan**, pour montrer les préférences par activité.
- **Autotest** : casque détecté, moteur actif, égaliseur et son 360 d'INZONE Hub coupés, son spatial Windows coupé, améliorations audio Windows coupées, raccourcis libres.
- **Nouvelle sortie audio** (dongle branché sur un autre port) : proposer d'activer Retro EQ dessus.
- **Fermer la fenêtre** ne coupe pas l'égaliseur. Par défaut, l'appli se réduit dans la barre des tâches.
- **Désinstallation** : retirer le moteur de chaque sortie et rendre le son d'origine.
- **Import et export** : Peace `.peace`, Equalizer APO `.txt`, AutoEQ ParametricEQ, Wavelet.

## Design (tokens de la maquette)

- **Couleurs :**

  | Rôle | Valeur |
  |---|---|
  | Fond | `#1B1E22` |
  | Surface | `#262A30` |
  | Texte | `#F2E8D5` |
  | Texte secondaire | `#C2B8A3` |
  | Traits | `#3E434B` |
  | Accent ambre | `#F3A712` (texte dessus : `#1B1E22`) |
  | Version A | `#2AB7A9` |
  | Version B | `#FF8A1F` |
  | OK | `#7BD389` |
  | Erreur | `#FF6F59` |
  | Bouton principal | crème `#F2E8D5`, texte `#1B1E22` |

- **Polices :** Antonio pour les titres (majuscules), Libre Franklin pour le texte, DM Mono pour les chiffres et les écrans LCD, Anton pour « EQ » dans le logo. Toutes sont sous licence OFL, à embarquer dans l'appli.
- **Univers :** sombre, rétro-futuriste années 80, avec des représentations d'appareils audio : platines cassette A et B, rack à faders, vumètre à aiguille, cassettes pour les tests et les profils, écrans LCD verts, touches de clavier en relief.
- **Couleurs à usage réservé :** l'ambre sert à la sélection et à l'action, A et B gardent toujours leurs couleurs.
- **Accessibilité :** texte lisible (contraste AA), tout utilisable au clavier, focus visible.

## Comment on travaille : skills de Matt Pocock

Utilise les skills `mattpocock-skills`, dans cet ordre :

1. **`/setup-matt-pocock-skills`** : configure le dépôt. Suivi des tickets en **fichiers locaux** dans `docs/tickets/` (pas de GitHub Issues pour l'instant), et emplacement de la doc de domaine.
2. **`/grill-with-docs`** : interroge-moi sur les questions ouvertes ci-dessous. Note chaque décision dans un ADR et chaque terme dans `GLOSSARY.md`. Le vocabulaire est déjà fixé : passage, duel, test, profil, version, référence, variante, sortie audio, moteur. Une question à la fois, en français.
3. **`/prototype`** : lève les risques techniques avant de construire. Fais un petit prototype jetable par question (voir plus bas), et donne-moi un verdict par écrit.
4. **`/to-spec`** puis **`/to-tickets`** : transforme tout ça en spec, puis en tickets « tracer bullet » avec leurs dépendances. Le premier ticket doit livrer une tranche fine mais complète, du moteur jusqu'à l'interface.
5. **`/implement`** ticket par ticket, avec **`/tdd`** pour toute la logique pure : DSP, machine à états d'un test, statistiques de confiance, import et export. Termine chaque ticket par **`/code-review`**.
6. En cas de bug : **`/diagnosing-bugs`**. Avant de clore une session longue : **`/handoff`**.

Si le travail dépasse visiblement une session, utilise **`/wayfinder`** pour tenir la carte des décisions.

## Architecture proposée (à challenger pendant le grilling)

Un workspace Cargo avec des modules profonds et des interfaces étroites :

| Crate | Rôle | Testable sans Windows |
|---|---|---|
| `retro-dsp` | filtres biquad, courbe de réponse, préampli auto, compensation de volume | oui |
| `retro-core` | domaine : profils et versions, déroulement d'un test (machine à états), duels de contrôle, échelle de confiance, persistance, import et export | oui |
| `retro-engine` | trait `Moteur` et implémentation `EqualizerApo` (écriture de la config, retour à la référence, détection des sorties) | partiellement (simulation du système de fichiers) |
| `retro-audio` | capture WASAPI loopback, FFT, niveau estimé, mesure de volume perçu | non |
| `retro-app` | interface Slint, barre des tâches, raccourcis globaux, mini-fenêtre, autotest | non |

`retro-core` ne dépend d'aucune API Windows. Le moteur et l'audio sont injectés derrière des traits, pour pouvoir tester le déroulement complet d'un test avec de fausses implémentations.

## Prototypes à faire en premier

1. **Equalizer APO** : écrire une config depuis Rust, mesurer le délai entre l'écriture et le changement audible, entendre s'il y a un clic, et vérifier les droits d'écriture du dossier `config`.
2. **WASAPI loopback** : capter ce qui joue sur la sortie du H9 II, calculer un niveau RMS et un volume perçu, puis un spectre.
3. **Raccourcis globaux** : F6 à F10 reçus pendant un jeu en plein écran fenêtré, puis en plein écran exclusif.
4. **Slint** : fenêtre sans bordure avec une barre de titre à onglets (déplacement, réduire, agrandir avec le menu Snap de Windows 11, fermer), une cassette dessinée et un fader. Vérifier la qualité du rendu.

## Questions ouvertes à trancher pendant le grilling

- Comment générer les variantes B zone par zone : pas en dB, nombre de bandes touchées par duel, convergence, quand s'arrêter.
- La méthode exacte de compensation du volume (calcul théorique sur la courbe, ou mesure en direct, ou les deux).
- Le format de stockage local des profils, des versions et des réponses (SQLite, ou fichiers JSON ou TOML).
- La stratégie pour la mini-fenêtre au-dessus des jeux.
- L'installateur (WiX, Inno Setup ou NSIS), qui installe aussi Equalizer APO, puis la signature via SignPath Foundation.
- Les cibles minimales : version de Windows, architectures (x64, ARM64 ?).

## Contraintes de travail

- Vérifie d'abord l'environnement (`rustc`, `cargo`, outils de compilation MSVC) et dis-moi ce qui manque avant d'installer quoi que ce soit.
- Ne touche jamais à la configuration audio réelle de mon PC sans me prévenir. Pour le prototype Equalizer APO, sauvegarde d'abord `config.txt` et rends-le à la fin.
- N'exécute aucun installeur et ne modifie aucun réglage Windows sans mon accord explicite.
- Petits commits clairs, en français.
- Commence par l'étape 1 (`/setup-matt-pocock-skills`), puis passe à l'étape 2.
