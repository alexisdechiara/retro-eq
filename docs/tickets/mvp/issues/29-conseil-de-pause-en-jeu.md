# Conseil de pause visible pendant le jeu

Status: resolved
Type: task

BRIEF : « Pause conseillée après 40 minutes d'écoute. »

Le conseil n'apparaît que sur l'écran Duels de la fenêtre principale (`Effet::ConseillerPause` est ignoré par le contrôleur). L'afficher aussi dans la mini-fenêtre.

## Comments

- La mini-fenêtre reçoit déjà la vue du test (`pousser_duels`) : elle affiche maintenant « Plus de 40 min d’écoute : F9 pour faire une pause » à la place de sa barre d'avancement, tant que le conseil vaut (jusqu'à la fin du test, comme sur l'écran Duels). F9 (« plus tard ») met le test en pause sans quitter le jeu.
- Même place que l'alerte des touches mortes (ticket 30), qui passe avant : une touche qui ne répond pas est plus urgente qu'une pause. La taille de la mini-fenêtre ne change pas (voir ticket 30).
- `Effet::ConseillerPause` reste sans effet dans le contrôleur : le conseil se lit dans la vue (`pause_conseillee`). Pas de bip en plein écran exclusif : le bip veut déjà dire « réponds maintenant », le doubler pour la pause brouillerait le signal.
- Tests : `controleur/tests/debut.rs` (`apres_40_minutes_d_ecoute_la_vue_du_test_conseille_une_pause`), `clics_tests.rs` (`la_mini_fenetre_conseille_la_pause_apres_40_minutes`, `dans_la_mini_fenetre_une_touche_morte_passe_avant_le_conseil_de_pause`).
- Capture : `RETRO_EQ_DEMO_TEST=pause scripts/capture.sh mini …`.
- À vérifier sur le PC : la ligne reste lisible par-dessus un jeu en plein écran fenêtré, à 125 % et 150 % de mise à l'échelle.
