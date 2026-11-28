# Sur le PC : l'autotest vérifie vraiment le casque et les traitements Windows

Status: ready-for-human
Type: task
Blocked by: 11

BRIEF : « casque détecté, égaliseur et son 360 d'INZONE Hub coupés, son spatial Windows coupé, améliorations audio Windows coupées ».

`autotest.rs` affiche toujours « Attention » pour INZONE Hub, le son spatial et les améliorations (à vérifier soi-même), et « casque détecté » vérifie seulement qu'une sortie par défaut existe, pas que c'est le H9 II. Lire les propriétés de la sortie et l'état du son spatial demande les API Windows.
