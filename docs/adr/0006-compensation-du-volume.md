# Compensation du volume : calculée sur le spectre de ce qui joue

Un égaliseur qui monte une zone rend le son plus fort, et plus fort est presque toujours préféré : sans correction, le test mesurerait le volume. Pendant un duel, A et B sont donc mises au même volume perçu. On calcule l'écart de volume que chaque courbe produit sur un spectre pondéré comme l'oreille (pondération K, celle de la mesure LUFS), et on corrige la variante par son préampli. Le spectre utilisé est celui de ce qui joue, capté par le visualiseur et lissé sur les dernières secondes ; à défaut de capture, un bruit rose. Les deux côtés reçoivent ensuite le même préampli de sécurité, le plus petit qui évite la saturation des deux.

On ne règle pas le volume par une boucle de mesure en direct (mesurer A, mesurer B, corriger) : en jeu, le contenu change trop d'un passage à l'autre pour que deux mesures successives soient comparables. La capture sert à nourrir le calcul et à afficher l'écart mesuré, pas à asservir le volume.

## Conséquences

- Le calcul est pur et vit dans `retro-dsp` ; il se teste sans Windows.
- Vérifié sur le PC (ticket 16, 25/11/2026) : la capture loopback se fait **après** Equalizer APO (un préampli de −12 dB baisse le niveau capté de 12 dB). Le contrôleur retient donc la courbe qu'il a envoyée au moteur et divise le spectre capté par elle (`Spectre::divise_par`) avant de s'en servir. Une mesure prise pendant un changement de courbe pourrait mêler les deux : le spectre n'est retenu que si la courbe n'a pas changé depuis la mesure précédente (une seconde plus tôt).
- Le visualiseur, lui, montre le son capté tel quel : c'est ce qu'on entend.
