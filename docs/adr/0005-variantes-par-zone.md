# Variantes : un filtre par zone, pas de 4 dB qui se réduit de test en test

Une variante est la référence plus un seul filtre, posé sur une des 7 zones (plateau bas pour les sub-graves, plateau haut pour l'air, cloche ailleurs), vers le haut ou vers le bas. Le pas vaut 4 dB au premier test d'un profil : en plein jeu, une différence plus petite sur une seule zone ne s'entend pas de façon fiable. Chaque version retient un pas par zone ; il est divisé par deux (plancher 1 dB) quand un test n'a retenu aucun changement dans la zone, et gardé tel quel quand un changement a été retenu. La convergence se fait donc de test en test, la référence restant fixe pendant un test (ADR-0004).

Le nombre de duels appartient au mode, pas au nombre de zones. Un duel sur six est un duel de contrôle (2 sur 12, 5 sur 30). Les duels restants vont par paires : chaque couple (zone, sens) retenu pour le test est joué **deux fois**, en commençant par les couples testés le moins récemment. Un test rapide couvre donc 5 couples, un test précis 12.

À la fin, une zone ne change que si la variante y a gagné au moins deux duels sans en perdre aucun, dans un seul sens. Une seule victoire est un indice : rien ne change, et ce couple passe en tête du test suivant. « Pareil » ne compte pour personne. Si la moitié au moins des duels de contrôle ont reçu A ou B, les réponses ne distinguent pas deux versions identiques : aucun changement n'est retenu pour ce test. Si le duel de vérification donne la préférence à la référence, la nouvelle version est créée mais n'est pas proposée à l'activation.

Ces règles viennent d'une simulation faite pendant la relecture : avec un duel par couple et un changement retenu dès la première victoire, quelqu'un qui répond au hasard obtenait un profil modifié dans 97 % des tests.

Un test s'arrête à la fin de son plan ; un profil est « stable » quand toutes ses zones sont au pas plancher sans changement retenu.

## Options écartées

- **Une bande de l'égaliseur par duel** : lisible, mais 10 bandes ne correspondent pas aux 7 zones de la maquette et une bande seule est trop étroite pour s'entendre en jeu.
- **Recherche adaptative dans le test** (la gagnante devient la référence) : écartée par l'ADR-0004.

## À vérifier à l'oreille

Le pas de 4 dB, le plancher de 1 dB et la largeur des filtres sont des valeurs de départ. Elles sont regroupées dans une seule structure de réglages pour être ajustées après les premiers vrais tests.

## Ce que dit la confiance

Indice, Probable et Sûr mesurent une comparaison prise seule : « cette comparaison avait au plus 1 chance sur 4 (ou moins de 6 %) de tomber ainsi par hasard ». Ils ne corrigent pas le fait qu'un test fait plusieurs comparaisons, et ne veulent pas dire « ce changement est probablement réel ». L'écran de résultat doit le dire dans ces termes. « Sûr » demande 5 victoires sur 5 : il n'est atteignable qu'en mode « Une seule zone » ou en cumulant des tests.

## Note du 28/11/2026 : la confiance après le duel de vérification

Jusqu'ici, un duel de vérification répondu donnait à lui seul la confiance du test. Un seul duel ne dépasse jamais Indice : confirmer la nouvelle version faisait donc tomber le test à Indice, alors que passer la vérification gardait la confiance la plus basse des zones changées (Sûr après Graves 5 sur 5). Répondre honnêtement était puni. Règle retenue : une vérification qui préfère la nouvelle version laisse la confiance du test à celle de la zone changée la plus basse, comme une vérification passée, et la version est recommandée ; une vérification qui préfère la référence, ou « Pareil », donne Indice et la version n'est pas recommandée.
