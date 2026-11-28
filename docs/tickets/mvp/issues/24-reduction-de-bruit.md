# Demander la réduction de bruit au début du test

Status: ready-for-agent
Type: task

PRODUCT.md : « La réduction de bruit du casque change son son et ne peut pas être lue par l'appli : on la demande au début du test et le profil la retient. »

Le champ existe dans `retro-core/src/profil.rs` (`reduction_bruit`, `definir_reduction_bruit`) mais rien ne l'appelle dans `retro-app`. L'écran Nouveau test pose la question (préremplie avec la valeur retenue par le profil), le contrôleur l'enregistre dans le profil au démarrage du test.
