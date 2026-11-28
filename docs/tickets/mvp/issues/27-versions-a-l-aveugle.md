# Comparer à l'aveugle deux versions d'un même profil

Status: resolved
Type: task

PRODUCT.md : « Versions de chaque profil (chaque test en crée une, comparables à l'aveugle, restaurables). »

Le test profil contre profil ne prend que la version active d'un autre profil. Depuis Mes profils, pouvoir lancer ce même test entre la version active et une autre version du même profil.

## Comments

Fait (worktree des tickets 26 à 28).

- Dans Mes profils, chaque version non active a un bouton **« Comparer à l'aveugle »** à côté de « Activer cette version ». Il active le profil s'il ne l'est pas (la référence d'un test est le profil actif, comme pour « Tester ce profil »), lance le test et ouvre l'écran Duels.
- Aucun second mécanisme : c'est le mode « profil contre profil » (`Mode::ProfilContreProfil`) avec la courbe de cette version de l'autre côté. Plan, duels de contrôle, tirage de A et B au hasard, compensation du volume et résultat sont ceux de ce mode. Nombre de duels et intervalle : ceux choisis sur l'écran « Nouveau test » (6 duels, 12 en précis).
- Aucune version n'est ajoutée. L'écran Résultat titre « Mon profil : version 3 contre version 2 » ; « Activer » rend l'autre version active, « Revenir à la référence » remet la version d'avant.
- Le journal ne garde que la courbe de l'autre côté (`Mode` n'est pas modifié, pour garder les journaux et le travail en parallèle compatibles) : au redémarrage, l'autre côté est cherché d'abord parmi les autres profils (comportement existant), puis parmi les versions du même profil. Si un autre profil a exactement la même courbe, il est pris à sa place : l'activer fait entendre la même chose.
- Refusé avec un message : la version active elle-même, une version inconnue, ou pendant un test.
