# Equalizer APO comme premier moteur, derrière un trait `Moteur`

Pour égaliser tout le son de Windows, il faut un composant chargé par le service audio (un APO). En écrire un est un chantier à part ; la v1 s'appuie donc sur Equalizer APO, installé à côté et piloté par ses fichiers texte, qu'il recharge tout seul. L'appli ne lui parle jamais directement : elle passe par un trait Rust `Moteur`, et tout le calcul des filtres (courbe de réponse, préampli automatique, compensation de volume) vit dans notre propre crate, pour pouvoir remplacer Equalizer APO par un APO maison sans toucher à l'interface ni au domaine.

## Conséquences

- Equalizer APO est sous GPLv2 : il est distribué à côté de Retro EQ par l'installateur, jamais lié ni mélangé à notre code.
- Le délai entre l'écriture du fichier et le changement audible, et la possibilité d'un fondu sans clic, dépendent d'Equalizer APO : à mesurer par prototype avant de figer le déroulement d'un duel.
- Ce qu'Equalizer APO ne sait pas faire seul (fondu, mesure du volume) doit rester exprimable dans le trait sans le trahir.
