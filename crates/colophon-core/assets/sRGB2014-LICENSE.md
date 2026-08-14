# sRGB2014.icc, licence de redistribution

Profil ICC v2 publié par l'International Color Consortium, encodage
sRGB IEC 61966-2.1. Récupéré le 14/08/2026 sur le registre officiel :

    https://registry.color.org/rgb-registry/profiles/sRGB2014.icc

SHA-256 : `384b832de3412066743b52a75ee906b6fb9fb8d9e09e936fc2c43223815c6e0a`
Taille : 3 024 octets. Version ICC 2.0.0, classe `mntr`, espace `RGB `,
PCS `XYZ `, description interne `sRGB2014`, tag `cprt` « Copyright
International Color Consortium, 2015 ».

## Termes, cités depuis le registre ICC

> This profile is made available by the International Color Consortium, and
> may be copied, distributed, embedded, made, used, and sold without
> restriction. Altered versions of this profile shall have the original
> identification and copyright information removed and shall not be
> misrepresented as the original profile.

Source des termes : https://registry.color.org/profile-library/

## Ce que ça autorise ici

Copie, redistribution et **incorporation** dans un fichier produit, sans
redevance et sans clause virale : le profil peut donc voyager dans le dépôt
sous GPL-3.0 comme dans chaque PDF exporté, en `DestOutputProfile` de
l'OutputIntent.

Le fichier est incorporé **tel quel**, jamais modifié : le seul cas que la
licence encadre est celui d'une version altérée, qui devrait perdre son
identification et son copyright. Toute substitution de ce profil repasse par
les tests de `icc.rs`, qui relisent l'en-tête et refusent un fichier qui ne
serait pas un profil RGB de version 2 ou moins.
