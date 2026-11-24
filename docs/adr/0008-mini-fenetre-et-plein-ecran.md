# Mini-fenêtre : fenêtre Slint toujours au-dessus, qui ne prend jamais le focus

La mini-fenêtre est une seconde fenêtre Slint, sans bordure, marquée « toujours au-dessus », « ne s'active pas » et absente de la barre des tâches (styles Windows `WS_EX_TOPMOST`, `WS_EX_NOACTIVATE`, `WS_EX_TOOLWINDOW`). Elle s'affiche au-dessus d'un jeu en plein écran fenêtré sans lui voler le clavier, et les réponses passent uniquement par les raccourcis globaux. On n'injecte rien dans le jeu (pas de surcouche façon Discord ou Steam) : c'est fragile et les anticheats le sanctionnent.

En plein écran exclusif, aucune fenêtre ne peut s'afficher. L'appli le détecte (`SHQueryUserNotificationState`) et confirme alors chaque réponse par un bip court.

Les raccourcis F6 à F10 ne sont enregistrés que pendant un test, pour ne pas les retirer aux jeux le reste du temps. Un raccourci déjà pris par une autre appli est signalé par l'autotest.
