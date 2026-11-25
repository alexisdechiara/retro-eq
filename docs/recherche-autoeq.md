# Recherche : la base AutoEQ

Relevé le 25/11/2026, en lisant les vraies pages (pas de mémoire). Ce qui n'a pas pu être
vérifié est dit comme tel.

## Licence

- **Le code** d'AutoEQ est sous licence MIT (« Copyright (c) 2018-2022 Jaakko Pasanen ») :
  <https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/LICENSE>.
- **Les données** (mesures et réglages calculés dans `results/`) : le dépôt ne dit **rien**.
  Ni fichier de licence propre aux données, ni README dans `measurements/`, ni mention dans
  le README principal, qui dit seulement que les mesures viennent d'autres bases : oratory1990,
  crinacle, Innerfidelity, Rtings, headphone.com (ancien), et « this project sources the
  measurements from other databases » (<https://github.com/jaakkopasanen/AutoEq>).
- Le dossier `measurements/` compte une vingtaine de sources (oratory1990, crinacle, Rtings,
  Filk, HypetheSonics, Super Review, ToneDeafMonk, Kuulokenurkka, Regan Cipher, etc.).
  Chacune a ses propres conditions, que le dépôt ne reprend pas.
- Conclusion : la licence MIT couvre le code, **pas** les mesures. Rien n'autorise
  clairement à les redistribuer dans notre dépôt GPLv3 ni dans l'installateur. Les réglages
  calculés (`results/`) dérivent de ces mesures. On ne les embarque pas (voir ADR 0013).

## Disposition de `results/`

Confirmée par des requêtes réelles (toutes en 200) :

```
results/<source>/<plateforme>/<Nom>/<Nom> ParametricEQ.txt
```

- `<source>` : qui a mesuré (`Rtings`, `oratory1990`, `crinacle`, `Filk`…).
- `<plateforme>` : le banc de mesure puis le type, par exemple `HMS II.3 over-ear`,
  `GRAS 43AG-7 over-ear`, `711 in-ear`, ou seulement `over-ear` / `in-ear` / `earbud` quand
  le banc n'est pas précisé (oratory1990, Innerfidelity, Filk…).
- `<Nom>` : le nom du casque, qui peut contenir `&`, des parenthèses et des espaces
  (`Bang & Olufsen Beoplay H9`, `ASUS ROG Delta S Core (fabric earpads)`). Le fichier porte le
  même nom que son dossier, plus ` ParametricEQ.txt`.
- Dans un dossier : `<Nom>.csv` (42 Ko pour l'INZONE H9), `<Nom> ParametricEQ.txt` (environ
  480 octets), `FixedBandEQ.txt`, `GraphicEQ.txt`, des `.wav`, un `README.md`, une image.
- Format du `ParametricEQ.txt` (vu sur l'INZONE H9) : `Preamp: -6.4 dB` puis
  `Filter 1: ON LSC Fc 105 Hz Gain 8.0 dB Q 0.70`, avec des types `LSC`, `PK`, `HSC`.
  C'est la grammaire qu'`equalizer_apo`/`autoeq` lit déjà.

## L'index

- `results/INDEX.md` existe : <https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/INDEX.md>.
  **851 757 octets**, 8 854 lignes, **8 850 entrées**, soit environ 0,85 Mo. Servi en
  gzip : 110 731 octets (environ 0,11 Mo) quand le client accepte `Accept-Encoding: gzip`.
- Forme d'une ligne (les 8 850 suivent le même schéma, vérifié par une expression
  régulière) :
  `- [Sony INZONE H9](./Rtings/HMS%20II.3%20over-ear/Sony%20INZONE%20H9) by Rtings on HMS II.3`
  Le chemin est relatif à `results/`, avec des `%20` ; `&` et les parenthèses restent bruts
  dans l'index. Le « by … on … » est facultatif.
- Pas d'index JSON généré. `results/README.md` (486 Ko) est la liste « recommandée » : un
  seul résultat par casque, 6 033 casques, sans le nom de la source : moins utile pour nous,
  qui voulons montrer toutes les sources.
- Plus de vingt sources ; les plus fournies : crinacle (522 entrées), Super Review (386),
  Innerfidelity (228), ToneDeafMonk (209), Kuulokenurkka (182), oratory1990 (158), Rtings (32).

## Les casques demandés

| Recherche | Dans l'index ? |
|---|---|
| Sony INZONE H9 | Oui, une seule source : **Rtings** sur HMS II.3 (over-ear) |
| Sony INZONE H9 II | **Non**. Absent le 25/11/2026 |
| ASUS ROG Delta | **Non** pour le modèle simple. Présents : `ASUS ROG Delta S Core (fabric earpads)` et `(leather earpads)`, source **Filk**. Aucune entrée « Delta S » ni « Delta Pro » |
| Autres INZONE | Buds (Rtings, B&K 5128), H3 et H7 (Rtings, HMS II.3) |

Un casque absent de la base reste un cas courant : l'appli doit le dire plainement
(« aucun résultat ») et ne rien inventer.

## Récupérer les fichiers

- **Un fichier ou l'index** : `https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/…`
  en HTTPS, sans compte. Vérifié : index, `ParametricEQ.txt` d'oratory1990, de Rtings et de
  Filk, noms avec `&` (écrit `%26`) et parenthèses (brutes ou `%28`/`%29`, les deux marchent).
  Pas de jeton ni de limite d'appels documentée pour `raw.githubusercontent.com`.
- **API GitHub** (`api.github.com/repos/jaakkopasanen/AutoEq/git/trees/master?recursive=1`) :
  **non vérifiée** ici, le proxy de cette session la bloque (403 « accès non activé », pas une
  limite de GitHub). D'après la documentation de GitHub, sans identifiant la limite est de
  60 requêtes par heure et par IP, et l'arbre récursif d'un dépôt de cette taille est tronqué.
  On n'en a pas besoin : `INDEX.md` suffit.
- **Taille de tout `results/`** : non mesurable ici (même blocage). Un seul dossier contient
  déjà environ 100 Ko (csv, wav, png), et il y en a 8 850 : plusieurs centaines de Mo au
  moins. À ne surtout pas embarquer ni cloner.
- La branche s'appelle `master`. Le dépôt a changé de disposition plusieurs fois (mise à jour
  du 29/10/2023 : « Improved and unified naming conventions… reorganized directory
  structure »), donc le code doit échouer calmement si un fichier n'existe plus.

## Fiabilité des mesures

Les sources n'utilisent pas le même banc de mesure (HMS II.3, GRAS 43AG-7, 711, B&K 5128…)
et leurs courbes ne sont pas comparables entre elles. Au-delà d'environ 8 kHz, une mesure sur
banc est peu fiable (résonances du conduit artificiel, position du casque). Un réglage AutoEQ
est un point de départ à juger à l'oreille, pas une vérité (principe 5 de `PRODUCT.md`).
