//  Ce que le bac à sable du Mac App Store demande, et que le DMG ignore :
//  retrouver un dossier choisi au lancement suivant, et montrer un fichier ou
//  une page sans lancer de processus. Compilé dans la seule saveur `mas`
//  (build.rs). Même manière que photos.m : du C plat à la frontière, aucune
//  décision ici, la politique vit dans signet.rs et lib.rs.

#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>

// ------------------------------------------------------------ marque-pages

/// Le marque-page de sécurité, de portée app, d'un dossier auquel le
/// processus a accès en ce moment (la personne vient de le choisir, ou un
/// marque-page précédent est ouvert). Rend un tampon `malloc` et sa longueur,
/// NULL si le système refuse.
uint8_t *colophon_signet_creer(const char *chemin, size_t *longueur) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:chemin
                                                    isDirectory:YES
                                                  relativeToURL:nil];
        NSError *err = nil;
        NSData *d = [url bookmarkDataWithOptions:NSURLBookmarkCreationWithSecurityScope
                  includingResourceValuesForKeys:nil
                                   relativeToURL:nil
                                           error:&err];
        if (!d || d.length == 0) return NULL;
        uint8_t *tampon = malloc(d.length);
        memcpy(tampon, d.bytes, d.length);
        *longueur = d.length;
        return tampon;
    }
}

void colophon_signet_liberer(void *p) {
    if (p) free(p);
}

/// Résout un marque-page et ouvre l'accès au dossier qu'il désigne. Rend une
/// poignée à passer à `colophon_signet_finir`, NULL si le marque-page ne se
/// résout pas ou si le système refuse l'accès. `chemin` reçoit le chemin
/// d'aujourd'hui (un dossier déplacé se suit), à libérer ; `perime` dit que
/// le marque-page doit être réécrit.
void *colophon_signet_commencer(const uint8_t *octets, size_t longueur,
                                char **chemin, int *perime) {
    @autoreleasepool {
        NSData *d = [NSData dataWithBytes:octets length:longueur];
        BOOL vieux = NO;
        NSError *err = nil;
        NSURL *url = [NSURL URLByResolvingBookmarkData:d
                                               options:NSURLBookmarkResolutionWithSecurityScope |
                                                       NSURLBookmarkResolutionWithoutUI
                                         relativeToURL:nil
                                   bookmarkDataIsStale:&vieux
                                                 error:&err];
        if (!url) return NULL;
        if (![url startAccessingSecurityScopedResource]) return NULL;
        const char *fs = url.fileSystemRepresentation;
        *chemin = strdup(fs ? fs : "");
        *perime = vieux ? 1 : 0;
        return (void *)CFBridgingRetain(url);
    }
}

/// Referme l'accès ouvert par `colophon_signet_commencer`.
void colophon_signet_finir(void *poignee) {
    if (!poignee) return;
    NSURL *url = CFBridgingRelease(poignee);
    [url stopAccessingSecurityScopedResource];
}

// ------------------------------------------------------------- NSWorkspace

/// Montre un fichier dans le Finder, sélectionné : l'équivalent de `open -R`,
/// sans processus.
int colophon_bureau_reveler(const char *chemin) {
    @autoreleasepool {
        NSURL *url = [NSURL fileURLWithFileSystemRepresentation:chemin
                                                    isDirectory:NO
                                                  relativeToURL:nil];
        [[NSWorkspace sharedWorkspace] activateFileViewerSelectingURLs:@[ url ]];
        return 1;
    }
}

/// Ouvre un dossier dans le Finder (`dossier` à 1) ou une adresse web dans le
/// navigateur par défaut (`dossier` à 0). 1 si le système a accepté.
int colophon_bureau_ouvrir(const char *cible, int dossier) {
    @autoreleasepool {
        NSURL *url = dossier
            ? [NSURL fileURLWithFileSystemRepresentation:cible isDirectory:YES relativeToURL:nil]
            : [NSURL URLWithString:[NSString stringWithUTF8String:cible]];
        if (!url) return 0;
        return [[NSWorkspace sharedWorkspace] openURL:url] ? 1 : 0;
    }
}
