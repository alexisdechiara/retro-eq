# La référence reste fixe pendant tout un test

Dans un test, chaque duel oppose la référence à une variante qui n'en diffère que par une zone. On aurait pu faire avancer la référence dès qu'une variante gagne (recherche adaptative, convergence plus rapide). On garde au contraire la même référence du premier au dernier duel : les modifications préférées sont combinées à la fin dans la nouvelle version du profil, et la convergence se fait d'un test au suivant.

Raisons : chaque duel est indépendant, donc on peut le passer, le rejouer ou le reprendre après un plantage sans fausser les autres ; « remettre la référence » garde un sens unique ; une réponse donnée par erreur ne contamine pas la suite.

## Conséquences

- L'effet combiné des zones n'est pas entendu pendant le test. Un duel final « référence contre nouvelle version » vérifie le résultat avant qu'il soit activé.
- Les duels de contrôle (deux fois la référence) restent comparables entre eux tout au long du test.
