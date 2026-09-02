//  La moitié Objective-C du pont vers PhotoKit. Elle ne décide rien : elle
//  ouvre la bibliothèque système, nomme ce qu'elle contient, et écrit une
//  photo à un chemin. Toute la politique — l'ordre, les noms, le rapport, le
//  refus du réseau, l'annulation — vit dans photos.rs.
//
//  Pourquoi de l'Objective-C écrit à la main plutôt qu'un crate de liaisons :
//  la même raison qu'en tête du module `imageio` de colophon-core::heic. Les
//  frameworks du système sont stables, la surface dont on a besoin tient en
//  cinq fonctions, et un arbre de dépendances vide est ce qu'on peut relire.
//  Le vrai gain est ailleurs : PhotoKit est une API à blocs, et un bloc
//  Objective-C construit depuis Rust est un piège. Ici les blocs restent dans
//  la langue qui les possède, repliés derrière un sémaphore, et la frontière
//  Rust ne voit que du C plat.

#import <Foundation/Foundation.h>
#import <Photos/Photos.h>

// ---------------------------------------------------------------- outils

static char *dup_utf8(NSString *s) {
    const char *u = [s UTF8String];
    char *c = malloc(strlen(u) + 1);
    strcpy(c, u);
    return c;
}

/// Tout ce qui traverse la frontière la traverse en JSON : une chaîne, un
/// `free`, et aucune structure partagée entre deux gestions de mémoire.
static char *dump(id obj) {
    NSError *err = nil;
    NSData *d = [NSJSONSerialization dataWithJSONObject:obj options:0 error:&err];
    if (!d) return dup_utf8(@"{\"erreur\":\"serialisation\"}");
    return dup_utf8([[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding]);
}

void colophon_photos_liberer(char *p) {
    if (p) free(p);
}

// ------------------------------------------------------------ autorisation

/// Le statut, sans rien demander. 0 notDetermined, 1 restricted, 2 denied,
/// 3 authorized, 4 limited.
///
/// Se lit avant chaque requête, et c'est le piège central de ce module :
/// PhotoKit ne lève jamais pour un défaut d'autorisation, il rend un résultat
/// vide. Sans cette lecture, une bibliothèque interdite est indiscernable
/// d'une bibliothèque vide, et l'écran dit « aucun album » à quelqu'un qui
/// n'a simplement pas encore autorisé.
int colophon_photos_statut(void) {
    return (int)[PHPhotoLibrary authorizationStatusForAccessLevel:PHAccessLevelReadWrite];
}

/// Demande l'accès et attend la réponse de l'utilisateur.
///
/// `PHAccessLevelReadWrite` n'est pas un choix : l'énumération `PHAccessLevel`
/// n'offre que `AddOnly` et `ReadWrite`, et il n'existe aucun niveau lecture
/// seule. Colophon n'écrit jamais dans la photothèque, mais macOS annoncera
/// quand même qu'il le pourrait. Ce que l'utilisateur lit vraiment est la
/// phrase de `NSPhotoLibraryUsageDescription`, et c'est là que la vérité est
/// rétablie. Ne pas « corriger » ceci en `AddOnly` : plus rien ne se lirait.
int colophon_photos_demander(void) {
    __block PHAuthorizationStatus out = PHAuthorizationStatusNotDetermined;
    dispatch_semaphore_t sem = dispatch_semaphore_create(0);
    [PHPhotoLibrary requestAuthorizationForAccessLevel:PHAccessLevelReadWrite
                                               handler:^(PHAuthorizationStatus s) {
        out = s;
        dispatch_semaphore_signal(sem);
    }];
    dispatch_semaphore_wait(sem, DISPATCH_TIME_FOREVER);
    return (int)out;
}

// ----------------------------------------------------------------- albums

static PHFetchOptions *photos_seulement(void) {
    PHFetchOptions *o = [PHFetchOptions new];
    // Une vidéo n'entre pas dans un livre. Le filtre est posé ici plutôt que
    // côté Rust pour que les comptes affichés soient ceux qu'on importera.
    o.predicate = [NSPredicate predicateWithFormat:@"mediaType == %d", PHAssetMediaTypeImage];
    return o;
}

/// Le chemin de la bibliothèque *système*, tel que photolibraryd le déclare.
///
/// Undocumenté, et traité comme tel : il ne sert **jamais** à ouvrir quoi que
/// ce soit, uniquement à expliquer une liste vide. PhotoKit ne lit que cette
/// bibliothèque-là, pas celle qui est ouverte dans Photos.app, et le 02/09 ce
/// chemin pointait un dossier supprimé sur cette machine — autorisation
/// accordée, soixante-quatre échecs XPC, et zéro album. Sans cette phrase,
/// quelqu'un cherche le défaut dans Colophon pendant une heure. Si la clé
/// disparaît d'une version de macOS, on rend la chaîne vide et l'écran
/// retombe sur son message générique : rien ne casse.
static NSString *bibliotheque_systeme(void) {
    NSUserDefaults *d = [[NSUserDefaults alloc]
        initWithSuiteName:@"group.com.apple.photolibraryd.private"];
    return [d stringForKey:@"SystemLibraryPath"] ?: @"";
}

/// Les albums de l'utilisateur, puis les albums intelligents, ceux qui
/// portent au moins une photographie. Jamais « toute la photothèque » :
/// composer un livre depuis quarante mille photos n'est pas le produit, et
/// l'offrir ferait de l'import une invitation à cloner une bibliothèque.
///
/// Les albums intelligents sont le repli de qui n'a créé aucun album. Leur
/// titre remonte dans la langue de Photos, pas dans celle de Colophon
/// (mesuré : « Recents », « Selfies » sur un système français) : c'est le nom
/// que l'utilisateur voit dans Photos, on ne le traduit pas.
///
/// Le chemin de la bibliothèque système voyage avec la liste : c'est ce qui
/// permet de distinguer « vide » de « injoignable », deux causes qui rendent
/// la même liste vide.
char *colophon_photos_albums(void) {
    NSMutableArray *out = [NSMutableArray array];
    PHFetchOptions *img = photos_seulement();

    void (^ajouter)(PHAssetCollection *, NSString *) = ^(PHAssetCollection *c, NSString *genre) {
        PHFetchResult *a = [PHAsset fetchAssetsInAssetCollection:c options:img];
        if (a.count == 0) return;
        [out addObject:@{@"id": c.localIdentifier ?: @"",
                         @"nom": c.localizedTitle ?: @"",
                         @"intelligent": @([genre isEqualToString:@"intelligent"]),
                         @"photos": @(a.count)}];
    };

    for (PHAssetCollection *c in [PHAssetCollection fetchAssetCollectionsWithType:PHAssetCollectionTypeAlbum
                                                                          subtype:PHAssetCollectionSubtypeAny
                                                                          options:nil]) {
        ajouter(c, @"album");
    }
    for (PHAssetCollection *c in [PHAssetCollection fetchAssetCollectionsWithType:PHAssetCollectionTypeSmartAlbum
                                                                          subtype:PHAssetCollectionSubtypeAny
                                                                          options:nil]) {
        ajouter(c, @"intelligent");
    }
    NSString *sys = bibliotheque_systeme();
    BOOL existe = sys.length > 0 &&
        [[NSFileManager defaultManager] fileExistsAtPath:sys];
    return dump(@{@"albums": out,
                  @"bibliotheque_systeme": sys,
                  @"bibliotheque_presente": @(existe)});
}

// ----------------------------------------------------------------- photos

/// La ressource à écrire pour une photo : la version rendue si elle existe,
/// l'originale sinon.
///
/// C'est la règle de `-edited` tranchée en 5.1, sur l'autre bibliothèque :
/// quelqu'un qui a recadré dans Photos attend son recadrage dans le livre.
/// `PHAssetResourceTypeFullSizePhoto` n'existe que sur une photo modifiée.
static PHAssetResource *ressource_a_ecrire(PHAsset *a) {
    PHAssetResource *originale = nil, *rendue = nil;
    for (PHAssetResource *r in [PHAssetResource assetResourcesForAsset:a]) {
        if (r.type == PHAssetResourceTypePhoto) originale = r;
        if (r.type == PHAssetResourceTypeFullSizePhoto) rendue = r;
    }
    return rendue ?: originale;
}

/// Les photographies d'un album, dans l'ordre de l'album, sans lire un octet
/// de pixel. Rend de quoi nommer, compter et annoncer un import avant de le
/// lancer.
char *colophon_photos_lister(const char *album_id) {
    NSString *aid = [NSString stringWithUTF8String:album_id];
    PHFetchResult *cols = [PHAssetCollection fetchAssetCollectionsWithLocalIdentifiers:@[aid]
                                                                              options:nil];
    if (cols.count == 0) return dump(@{@"erreur": @"album introuvable"});

    PHFetchResult<PHAsset *> *assets =
        [PHAsset fetchAssetsInAssetCollection:cols[0] options:photos_seulement()];

    NSMutableArray *fiches = [NSMutableArray arrayWithCapacity:assets.count];
    for (NSUInteger i = 0; i < assets.count; i++) {
        PHAsset *a = assets[i];
        PHAssetResource *r = ressource_a_ecrire(a);
        [fiches addObject:@{
            @"id": a.localIdentifier ?: @"",
            @"nom": r.originalFilename ?: @"",
            @"modifiee": @(r && r.type == PHAssetResourceTypeFullSizePhoto),
        }];
    }
    return dump(@{@"nom": [(PHAssetCollection *)cols[0] localizedTitle] ?: @"",
                  @"photos": fiches});
}

/// Écrit une photographie au chemin donné.
///
/// `reseau == 0` refuse tout téléchargement : une photo dont les octets sont
/// dans iCloud et pas sur ce Mac échoue alors avec
/// `PHPhotosErrorNetworkAccessRequired` (3164), et c'est exactement le cas
/// qu'on veut compter plutôt que subir. Rien ne se télécharge sans que
/// l'utilisateur ait dit oui devant un compte et un poids.
///
/// Mesuré le 02/09 : le fichier écrit est identique à l'original octet pour
/// octet, et sur le même volume APFS il coûte quasiment zéro — PhotoKit
/// clone les blocs. C'est ce qui autorise l'import à matérialiser un vrai
/// dossier de photographies au lieu d'inventer un accès paresseux.
char *colophon_photos_exporter(const char *asset_id, const char *dest, int reseau) {
    NSString *aid = [NSString stringWithUTF8String:asset_id];
    NSString *chemin = [NSString stringWithUTF8String:dest];

    PHFetchResult<PHAsset *> *r = [PHAsset fetchAssetsWithLocalIdentifiers:@[aid] options:nil];
    if (r.count == 0) return dump(@{@"ok": @NO, @"motif": @"photo introuvable"});
    PHAssetResource *res = ressource_a_ecrire(r[0]);
    if (!res) return dump(@{@"ok": @NO, @"motif": @"aucune ressource image"});

    [[NSFileManager defaultManager] removeItemAtPath:chemin error:nil];

    PHAssetResourceRequestOptions *o = [PHAssetResourceRequestOptions new];
    o.networkAccessAllowed = reseau ? YES : NO;

    __block NSError *err = nil;
    dispatch_semaphore_t sem = dispatch_semaphore_create(0);
    [[PHAssetResourceManager defaultManager] writeDataForAssetResource:res
                                                                toFile:[NSURL fileURLWithPath:chemin]
                                                               options:o
                                                     completionHandler:^(NSError *e) {
        err = e;
        dispatch_semaphore_signal(sem);
    }];
    dispatch_semaphore_wait(sem, DISPATCH_TIME_FOREVER);

    if (err) {
        return dump(@{@"ok": @NO,
                      @"code": @(err.code),
                      // 3164 : les octets sont dans iCloud et pas ici. Le seul
                      // code que l'appelant distingue, parce que c'est le seul
                      // auquel un téléchargement répondrait.
                      @"absente_du_mac": @(err.code == 3164),
                      @"motif": err.localizedDescription ?: @"écriture refusée"});
    }
    NSDictionary *at = [[NSFileManager defaultManager] attributesOfItemAtPath:chemin error:nil];
    return dump(@{@"ok": @YES, @"octets": at[NSFileSize] ?: @0});
}
