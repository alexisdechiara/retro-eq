# Sur le PC : délai, clic et droits d'écriture d'Equalizer APO

Status: ready-for-human
Type: prototype
Blocked by: 02

Sauvegarder config.txt avant, le rendre après. Mesurer le délai entre l'écriture et le changement audible, écouter s'il y a un clic à la bascule, essayer le fondu par paliers, vérifier que le dossier config est inscriptible sans droits d'administrateur et qu'un remplacement par renommage est bien rechargé.

À vérifier aussi (relevé par la relecture) : nos plateaux (`LSC` et `HSC` écrits avec un `Q`) donnent-ils dans Equalizer APO la même courbe que celle que `retro-dsp` calcule ? La documentation d'Equalizer APO ne donne pas la formule. Méthode : activer un plateau bas de +6 dB à 100 Hz, Q 0,71, ouvrir le graphique de l'éditeur d'Equalizer APO et comparer à la courbe affichée par Retro EQ. Si elles diffèrent, les variantes des zones Sub-graves et Air ne sonnent pas comme l'appli le croit.
