# Duel d'essai au premier lancement

Status: resolved
Type: task

PRODUCT.md : « Premier lancement avec autotest, puis duel d'essai (différence énorme, vérifie de bout en bout que le son change). »

Absent du code. Après l'autotest du premier lancement, proposer un duel unique entre la référence et une variante très marquée, avec les mêmes touches que les vrais duels. Il ne crée aucune version et n'entre dans aucun résultat. Une réponse « Pareil » indique que le son ne change pas : renvoyer vers le Diagnostic. La vérification à l'oreille sur le PC reste à faire par Alexis.

## Comments

- **Domaine** (`retro-core/src/essai.rs`) : le duel d'essai prend la forme d'un mode existant, « profil contre profil » avec un seul duel, l'« autre profil » étant la référence plus une cloche de +6 dB sur la zone Graves (`variante`, préampli de la référence gardé ; A et B mis au même volume perçu comme dans tout duel). Ce mode n'ajoute jamais de duel de vérification : l'essai se termine dès la réponse. `BilanEssai::depuis` traduit la réponse : graves reconnues, différence entendue sans les reconnaître, « Pareil », ou passé.
- **Même machine à états** que les vrais duels (`Deroulement`) et mêmes chemins du contrôleur (`tic`, `repondre`, `mettre_en_pause`, `arreter_test`, `avant_de_quitter`) : référence remise à la pause, à la fin, à l'abandon et en quittant ; mêmes touches F6 à F9 (réécoute comprise), mini-fenêtre, pause pendant les appels.
- **Pas de journal** pour l'essai (`TestEnCours::journal` vaut `None`) : sinon il serait repris au démarrage suivant et lu par le plan des vrais tests. Après un arrêt brutal, `etat-moteur.json` (qui ne contient jamais une variante) remet la référence ; rien n'est repris, l'essai dure une minute. À la fin : aucune version, le dernier résultat ne bouge pas, seul le bilan est gardé pour la session.
- **Question posée** : « Laquelle a plus de graves ? » (et non « Laquelle préfères-tu ? »), sur l'écran d'essai et dans la mini-fenêtre. Le côté des graves est révélé dans le bilan : l'essai ne compte pas.
- **Passages de 10 s** (le minimum) quel que soit le réglage : six passages et la question tiennent en une minute.
- **Écran** : nouvel écran `essai`, ouvert par « Continuer » du premier lancement (après l'accord pour écrire dans Equalizer APO). Plus simple que la maquette `00b-duel-d-essai.png` : pas de frise des trois étapes ni de carte « Ce que l'essai vérifie » (l'appli ne sait pas encore vérifier elle-même les touches et la mini-fenêtre). Un bandeau selon l'état (prêt / écoute / question / pause / bilan), les deux platines, les touches. « Passer l'essai » mène à l'accueil. Bilan : « Continuer » et « Refaire l'essai » ; si aucune différence n'est entendue, « Ouvrir le Diagnostic » passe en bouton principal, avec « Continuer quand même ».
- L'essai n'est proposé qu'au premier lancement (pas d'entrée depuis Réglages ou Diagnostic) : à ajouter si Alexis veut pouvoir le refaire plus tard.
- Ajout au glossaire : « Duel d'essai ».
- Tests : `retro-core/src/essai.rs` (4 tests, dont la fin dès la réponse sur la référence), `controleur/tests/debut.rs` (11 tests « essai » : touches, pas de version ni de résultat ni de journal, pause, arrêt, quitter, arrêt brutal, dernier résultat intact, Ctrl+Alt+Entrée, refus sans accord ou pendant un vrai test), `clics_tests.rs` (3 tests), `textes/debut.rs`.
- Captures : `RETRO_EQ_DEMO_ESSAI=pret|question|reconnue|confondue|pareil scripts/capture.sh essai …` ; `RETRO_EQ_DEMO_ESSAI=question scripts/capture.sh mini …`.
- **À faire par Alexis sur le PC, casque sur les oreilles** : entendre nettement la différence de graves de l'essai (sur de la musique et dans un jeu), vérifier qu'aucun clic ni saturation n'accompagne la bascule vers la version aux graves (+6 dB avec le préampli de la référence), que la référence revient bien à la réponse, à la pause (F9) et en quittant, et qu'une réponse « Pareil » renvoie au Diagnostic.
