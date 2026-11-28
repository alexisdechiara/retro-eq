# Reprise sur le PC Windows

Prompt à coller dans une session Claude Code **locale**, ouverte à la racine du dépôt sur le PC Windows, casque INZONE H9 II branché. Il couvre tout ce qu'une session cloud (Linux) ne peut pas faire.

---

Tu reprends Retro EQ sur mon PC Windows, casque INZONE H9 II branché sur son dongle. Lis d'abord `CLAUDE.md`, `docs/BRIEF.md` et `docs/agents/issue-tracker.md`. Réponds-moi en français.

## Ce qui a été fait sans Windows

La branche `claude/upbeat-galileo-f1yj6a` contient, en plus du MVP (tickets 01 à 13, 22, 23) :

- **Réversibilité** : quitter pendant un duel remet la référence ; une seconde instance est refusée (fichier verrou, plus mutex Windows) ; des réglages illisibles sont mis de côté au lieu d'être écrasés ; la référence est remise avant tout ce qui peut empêcher l'appli de s'ouvrir ; une référence refusée est retentée chaque seconde ; la désinstallation retire le moteur même si le dossier de données est inutilisable ; pas d'import pendant un test.
- **Imports** : valeurs absurdes et fréquences au-delà de 22 050 Hz signalées, import Wavelet / GraphicEQ (approché sur les 10 bandes, écart maximal annoncé), préampli automatique sûr à 44,1, 48 et 96 kHz.
- **Règle** : confirmer la nouvelle version au duel de vérification ne baisse plus la confiance (note dans l'ADR-0005).
- **Tickets 24 à 30** : question de la réduction de bruit, duel d'essai du premier lancement, écart de volume mesuré entre A et B, comparaison à l'aveugle de deux versions, Empreinte par appli, conseil de pause et conflit de touche visibles dans la mini-fenêtre.

Tout cela est testé avec le moteur et la capture **simulés** (688 tests) et le code Windows compile, mais **rien n'a tourné sur un vrai PC ni été entendu**.

## Avant de commencer

1. `git fetch` puis `git checkout claude/upbeat-galileo-f1yj6a`.
2. `rustup update stable` : le projet demande maintenant Rust 1.89 au minimum.
3. `cargo test --workspace` puis `cargo build --release -p retro-app`. Si quelque chose casse sous Windows, corrige-le avant tout le reste (`/diagnosing-bugs` si ce n'est pas évident).
4. Rappel des règles : ne touche jamais à ma configuration audio sans me prévenir, sauvegarde `config.txt` d'Equalizer APO avant tout essai et rends-le à la fin, n'exécute aucun installeur et ne change aucun réglage Windows sans mon accord explicite.

## Le travail, dans cet ordre

Travaille un ticket à la fois (`docs/tickets/mvp/issues/`), écris ce que tu observes sous `## Comments`, passe le ticket à `resolved` quand c'est tranché, et fais un petit commit en français par ticket. Pour chaque essai, dis-moi exactement quoi lancer, quoi écouter ou regarder, et attends ma réponse : c'est moi qui ai les oreilles.

Le script `scripts/essais-pc.sh` (à lancer dans Git Bash) me guide déjà en 12 étapes sur une bonne partie de ces essais : propose-le-moi en premier, puis complète avec ce qu'il ne couvre pas.

1. **Ticket 15, Equalizer APO** : délai entre l'écriture et le changement audible, clic à la bascule, fondu par paliers, dossier `config` inscriptible sans droits d'administrateur, remplacement par renommage bien rechargé. Comparer aussi un plateau bas de +6 dB à 100 Hz (Q 0,71) entre le graphique de l'éditeur d'Equalizer APO et la courbe de Retro EQ. L'exemple `crates/retro-engine/examples/sonde_apo.rs` sert à ces mesures.
2. **Ticket 16, capture loopback** : avant ou après le moteur ? (bruit rose, courbe très marquée, comparer le spectre capté). Décide si l'ADR-0006 tient.
3. **Ticket 33, robustesse** : le renommage final échoue-t-il parfois quand Equalizer APO ou l'antivirus tient le fichier ? L'interface se fige-t-elle si la capture WASAPI ne répond pas ? La fenêtre d'import suspend-elle les tics ? Lancer l'appli deux fois : la seconde doit se fermer sans toucher au son. Éteindre ou redémarrer Windows pendant un duel : la référence doit être revenue au lancement suivant.
4. **Tickets 17 et 18, raccourcis et fenêtres** : F6 à F10 en plein écran fenêtré puis exclusif, bip en exclusif ; fenêtre sans bordure, deux tailles seulement, pas de menu Snap ; mini-fenêtre au-dessus d'un jeu sans lui prendre le clavier, lisible à 125 % et 150 %.
5. **Nouveautés à entendre et à voir** :
   - Duel d'essai (premier lancement) : les +6 dB de graves s'entendent nettement, pas de clic, la référence revient après la réponse, après F9 et en quittant ; « Pareil » mène au Diagnostic.
   - Quitter par la barre des tâches en plein duel : la référence est bien revenue.
   - Écart de volume mesuré : un changement de préampli connu (−6 dB) déplace l'écart d'autant ; 2 s après une bascule suffisent-elles pour que la mesure se stabilise ; le seuil de 2 dB est-il le bon en jeu.
   - Empreinte : répondre dans la fenêtre ou la mini-fenêtre n'enregistre aucune appli ; un jeu est bien noté dès le premier passage ; le nom de l'exécutable s'affiche correctement.
   - Conflit de touche : une touche vraiment prise par une autre appli (F9 par OBS, par exemple) est annoncée au premier duel.
   - Import d'un vrai fichier Wavelet.
6. **Ticket 19, Discord** : la session micro n'est-elle active que pendant un appel ? Sinon, trouver un autre signal.
7. **Tickets 14 et 20, Peace** : j'importe mes presets Peace réels depuis l'appli (ne jamais inventer leurs valeurs) ; ouvrir dans Peace un fichier exporté par Retro EQ.
8. **Ticket 31, autotest** : lire vraiment l'état du son spatial, des améliorations audio et reconnaître le H9 II, au lieu de « à vérifier soi-même ».
9. **Ticket 32, nouvelle sortie audio** : proposer d'activer Retro EQ dessus (droits d'administrateur).
10. **Ticket 21, installateur** : suivre « Jamais essayé sur une vraie machine » dans `installateur/README.md` (compilation par la CI, installation, désinstallation qui rend le son d'origine). La demande à SignPath est à faire par moi (`docs/signature-signpath.md`).
11. **Licences** : `cargo metadata --format-version 1` en ligne (ou `cargo deny`) pour contrôler les licences des dépendances propres à Windows, que la session cloud n'a pas pu lire.

## Trois choix laissés ouverts

Signale-les-moi quand tu arrives dessus, je trancherai :

- L'Empreinte regroupe par exécutable, pas par activité (jeu, vidéo, Discord).
- Le duel d'essai n'est proposé qu'au premier lancement, sans entrée depuis Réglages ou Diagnostic.
- Une seconde instance se ferme en silence, sans ramener la première fenêtre au premier plan.

## Pour finir

Quand tout est tranché : `/code-review` sur la branche, puis `/retro` dans la même session.
