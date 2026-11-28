# Annoncer un conflit de touche au démarrage d'un test

Status: ready-for-agent
Type: task

BRIEF : « Signaler les conflits de touches. »

`regler_touches_de_test` (`raccourcis.rs`) range en silence les touches F6 à F10 qu'il n'a pas pu réserver. Elles ne se voient que dans Réglages et l'autotest : un test peut démarrer avec des touches de réponse mortes. Le dire au démarrage du test, dans la fenêtre et la mini-fenêtre.
