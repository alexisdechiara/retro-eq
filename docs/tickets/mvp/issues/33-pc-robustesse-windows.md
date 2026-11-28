# Sur le PC : ce que la relecture n'a pas pu vérifier sans Windows

Status: ready-for-human
Type: prototype
Blocked by: 15, 16

- `ecrire_atomique` (`equalizer_apo.rs`) ne réessaie pas le renommage final. Si Equalizer APO ou un antivirus tient le fichier un instant, l'écriture échoue : mesurer si cela arrive, et réessayer si oui.
- `CaptureLoopback::demarrer` attend jusqu'à 5 s sur le fil de l'interface et est relancée toutes les 5 s : si WASAPI ne répond pas, l'interface et les touches se figent.
- Le garde d'instance unique (mutex Windows) et la remise de la référence à l'arrêt de Windows n'ont jamais été compilés ni essayés sur une vraie machine.
- La fenêtre d'import (`rfd`) suspend-elle les tics du test tant qu'elle est ouverte ?
- Licences des dépendances propres à Windows : les contrôler avec `cargo deny` ou `cargo metadata` en ligne.
