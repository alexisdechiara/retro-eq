# Demander la réduction de bruit au début du test

Status: resolved
Type: task

PRODUCT.md : « La réduction de bruit du casque change son son et ne peut pas être lue par l'appli : on la demande au début du test et le profil la retient. »

Le champ existe dans `retro-core/src/profil.rs` (`reduction_bruit`, `definir_reduction_bruit`) mais rien ne l'appelle dans `retro-app`. L'écran Nouveau test pose la question (préremplie avec la valeur retenue par le profil), le contrôleur l'enregistre dans le profil au démarrage du test.

## Comments

- La question est dans la carte « Réglages du test » de l'écran Nouveau test : deux boutons, « Activée » et « Coupée ». Préremplie avec ce que le profil actif a retenu ; tant que le profil n'a jamais répondu, aucune option n'est cochée.
- Répondre n'est pas obligatoire pour démarrer : sans réponse, le profil garde « jamais répondu ». Choix le plus simple, qui ne bloque pas le départ d'un test pendant une partie ; à revoir si Alexis veut l'imposer.
- Le choix n'est écrit dans le profil qu'au démarrage du test (pas de version créée : ce n'est pas un réglage du son). Il est noté avec l'identifiant du profil actif : changer de profil avant de démarrer reprend la valeur de l'autre profil, sans lui imposer celle choisie pour le premier.
- Tests : `controleur/tests/debut.rs` (quatre tests « réduction de bruit ») et `clics_tests.rs` (`un_clic_sur_la_reduction_de_bruit_la_choisit_au_premier_clic`).
- À voir sur le PC : rien de propre à Windows. La valeur retenue ne sert encore à rien d'autre qu'à préremplir la question (ni l'Empreinte ni le Résultat ne l'affichent).
