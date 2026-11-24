# Variantes : un filtre par zone, pas de 4 dB qui se réduit de test en test

Une variante est la référence plus un seul filtre, posé sur une des 7 zones (plateau bas pour les sub-graves, plateau haut pour l'air, cloche ailleurs), vers le haut ou vers le bas. Le pas vaut 4 dB au premier test d'un profil : en plein jeu, une différence plus petite sur une seule zone ne s'entend pas de façon fiable. Chaque version retient un pas par zone ; il est divisé par deux (plancher 1 dB) quand un test n'a retenu aucun changement dans la zone, et gardé tel quel quand un changement a été retenu. La convergence se fait donc de test en test, la référence restant fixe pendant un test (ADR-0004).

Le nombre de duels appartient au mode, pas au nombre de zones. Un duel sur six est un duel de contrôle (2 sur 12, 5 sur 30). Les duels restants sont distribués à tour de rôle entre les couples (zone, sens), en commençant par les zones testées le moins récemment, puis en repassant sur les mêmes couples si le budget le permet.

À la fin, une zone change si la variante y a gagné plus de duels que la référence dans un sens, et pas dans les deux à la fois (réponses contradictoires : aucun changement, c'est signalé). « Pareil » ne compte pour personne. Un test s'arrête à la fin de son plan ; un profil est « stable » quand toutes ses zones sont au pas plancher sans changement retenu.

## Options écartées

- **Une bande de l'égaliseur par duel** : lisible, mais 10 bandes ne correspondent pas aux 7 zones de la maquette et une bande seule est trop étroite pour s'entendre en jeu.
- **Recherche adaptative dans le test** (la gagnante devient la référence) : écartée par l'ADR-0004.

## À vérifier à l'oreille

Le pas de 4 dB, le plancher de 1 dB et la largeur des filtres sont des valeurs de départ. Elles sont regroupées dans une seule structure de réglages pour être ajustées après les premiers vrais tests.
