# cities5000.tsv, provenance et licence

Extrait du jeu de données GeoNames `cities5000` : les localités de plus de
5 000 habitants, 64 363 entrées après filtrage. Récupéré le 14/08/2026 sur

    https://download.geonames.org/export/dump/cities5000.zip

## Licence

Les données GeoNames sont publiées sous **Creative Commons Attribution 4.0
International (CC BY 4.0)**.

    https://creativecommons.org/licenses/by/4.0/

Elle autorise la copie, la redistribution et la modification, y compris à des
fins commerciales, à une condition : **créditer la source**.

## Attribution, telle qu'elle doit apparaître

> Noms de lieux : données GeoNames (https://www.geonames.org), sous licence
> Creative Commons Attribution 4.0.

Cette phrase doit rester visible partout où les titres de chapitre géographiques
sont utilisés : dans le README, dans l'app, et dans la page « à propos » du
site le jour où il existera. Une attribution qu'on retire par distraction est
une violation de licence, pas un détail de présentation.

## Ce qui a été modifié

Le fichier d'origine compte 19 colonnes. Cinq sont conservées, dans cet ordre,
séparées par des tabulations :

    nom	latitude	longitude	code pays	population

Les entrées sont filtrées sur la classe `P` (localités), débarrassées du code
`PPLX` (les quartiers : sans lui, un chapitre parisien s'intitulerait « Paris 04
Hôtel-de-Ville » au lieu de « Paris »), et triées par latitude croissante, ce
qui permet à `places.rs` de ne comparer qu'une bande de latitude au lieu des
64 363 lignes. Aucune valeur n'est retouchée : les coordonnées sont
arrondies à quatre décimales, soit environ onze mètres, très en deçà de la
précision utile ici.

Reconstruire l'extrait après une mise à jour amont demande de refaire ce
filtrage puis de repasser les tests de `places.rs`, qui vérifient la taille du
jeu et quelques repères connus.
