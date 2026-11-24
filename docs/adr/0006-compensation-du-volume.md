# Compensation du volume : calculée sur le spectre de ce qui joue

Un égaliseur qui monte une zone rend le son plus fort, et plus fort est presque toujours préféré : sans correction, le test mesurerait le volume. Pendant un duel, A et B sont donc mises au même volume perçu. On calcule l'écart de volume que chaque courbe produit sur un spectre pondéré comme l'oreille (pondération K, celle de la mesure LUFS), et on corrige la variante par son préampli. Le spectre utilisé est celui de ce qui joue, capté par le visualiseur et lissé sur les dernières secondes ; à défaut de capture, un bruit rose. Les deux côtés reçoivent ensuite le même préampli de sécurité, le plus petit qui évite la saturation des deux.

On ne règle pas le volume par une boucle de mesure en direct (mesurer A, mesurer B, corriger) : en jeu, le contenu change trop d'un passage à l'autre pour que deux mesures successives soient comparables. La capture sert à nourrir le calcul et à afficher l'écart mesuré, pas à asservir le volume.

## Conséquences

- Le calcul est pur et vit dans `retro-dsp` ; il se teste sans Windows.
- Reste à vérifier sur le PC : la capture loopback se fait-elle avant ou après Equalizer APO ? Si elle est après, le spectre capté est divisé par la courbe active avant le calcul.
