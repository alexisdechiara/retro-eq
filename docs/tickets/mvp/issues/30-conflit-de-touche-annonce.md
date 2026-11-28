# Annoncer un conflit de touche au démarrage d'un test

Status: resolved
Type: task

BRIEF : « Signaler les conflits de touches. »

`regler_touches_de_test` (`raccourcis.rs`) range en silence les touches F6 à F10 qu'il n'a pas pu réserver. Elles ne se voient que dans Réglages et l'autotest : un test peut démarrer avec des touches de réponse mortes. Le dire au démarrage du test, dans la fenêtre et la mini-fenêtre.

## Comments

- À chaque image, le pont passe au contrôleur les touches F6 à F10 que le système a refusées au moment où le test les a réservées (`Raccourcis::touches_de_test_mortes`, vide hors test). Le contrôleur l'annonce en erreur (le message en bas de la fenêtre) une fois par test, et de nouveau si la liste change ; un conflit déjà vu par l'autotest est redit quand un test commence.
- Pendant le test, l'écran Duels affiche une ligne rouge sous les touches (« Raccourcis sans effet pendant ce test : F8… »), et la mini-fenêtre remplace sa barre d'avancement par « Sans effet : F8 (une autre appli les garde) ». Choix : remplacer plutôt qu'ajouter une ligne, pour ne pas changer la taille de la mini-fenêtre, qui est placée une seule fois en bas à droite de l'écran (`mini.rs`).
- Tests : `controleur/tests/debut.rs` (trois tests « touches mortes »), `raccourcis.rs` (`seules_les_touches_de_reponse_comptent_dans_l_ordre_f6_a_f10`), `textes/debut.rs`.
- Captures : `--demo` avec `RETRO_EQ_DEMO_TEST=touches-mortes`, écrans `duels` et `mini` (nouvelle option `--ecran mini` de `scripts/capture.sh`).
- À vérifier sur le PC : qu'une touche réellement prise par une autre appli (par ex. F9 par OBS) est bien refusée par `global-hotkey` au moment où le test la réserve, et que l'annonce et la ligne de la mini-fenêtre apparaissent dès le premier duel.
