//! Le dépôt S3 des deux PDF d'une commande (K-s2).
//!
//! Cloudprinter ne reçoit pas de fichier : il **tire** chaque URL HTTPS que
//! la commande lui donne. Le dépôt est donc un seau S3 dont la personne
//! donne les accès, et le moteur n'y fait que trois gestes, chacun par une
//! URL présignée SigV4 (`rusty-s3`) : poser un fichier, en donner une URL de
//! lecture de sept jours, le retirer.
//!
//! **Rien de propre à un fournisseur dans le code.** Cinq champs, ceux de
//! tout S3 : R2 les remplit avec `https://<compte>.r2.cloudflarestorage.com`
//! et la région `auto`, MinIO avec son adresse et sa région. Les URL sont
//! toujours en **chemin** (`<endpoint>/<seau>/<clé>`), jamais en sous-domaine :
//! R2 et MinIO l'acceptent tous deux, et un sous-domaine demanderait un DNS
//! que MinIO n'a pas sur un NAS.
//!
//! **Un fichier se pose en flux.** L'intérieur pèse 95 Mo : le corps du PUT
//! est le fichier ouvert, lu par le transport au fil de l'envoi, avec sa
//! longueur annoncée (S3 refuse un PUT présigné sans `Content-Length`).

use crate::commande::Etat;
use crate::export::{LIVRAISON_COUVERTURE, LIVRAISON_INTERIEUR};
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::time::Duration;

/// La durée d'une URL de lecture : sept jours, le maximum de SigV4.
pub const DUREE_DE_LECTURE: Duration = Duration::from_secs(7 * 24 * 3600);

/// La durée d'une URL d'écriture ou de retrait : le temps du geste.
const DUREE_DU_GESTE: Duration = Duration::from_secs(3600);

/// Au-delà, un objet se retire quel que soit l'état de sa commande.
pub const GARDE_JOURS: i64 = 30;

/// Le préfixe de toute clé d'objet posée par Colophon.
pub const PREFIXE: &str = "colophon";

/// Un accès au dépôt. Son `Debug` et son `Display` rendent `***`, il n'a pas
/// de `Serialize`.
#[derive(Clone)]
pub struct Acces(String);

impl Acces {
    pub fn new(valeur: impl Into<String>) -> Self {
        Acces(valeur.into())
    }
}

impl fmt::Debug for Acces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for Acces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Les réglages d'un dépôt, les cinq champs de tout S3.
#[derive(Debug, Clone)]
pub struct Reglages {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: Acces,
    pub secret_key: Acces,
}

/// Un objet posé, nommé par sa clé dans le seau.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objet {
    pub cle: String,
}

/// Un dépôt prêt à signer.
pub struct Depot {
    reglages: Reglages,
    seau: Bucket,
    agent: ureq::Agent,
}

impl fmt::Debug for Depot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Depot").field("reglages", &self.reglages).finish_non_exhaustive()
    }
}

impl Depot {
    pub fn new(reglages: Reglages) -> Result<Depot> {
        let endpoint = reglages
            .endpoint
            .parse()
            .with_context(|| format!("adresse du dépôt illisible : {}", reglages.endpoint))?;
        let seau = Bucket::new(endpoint, UrlStyle::Path, reglages.bucket.clone(), reglages.region.clone())
            .map_err(|e| anyhow::anyhow!("adresse du dépôt refusée ({e:?}) : {}", reglages.endpoint))?;
        // Pas de délai global : 95 Mo sur une ligne lente dépassent n'importe
        // quel chiffre qu'on choisirait. La connexion, elle, se borne.
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(30)))
            .build()
            .into();
        Ok(Depot { reglages, seau, agent })
    }

    fn acces(&self) -> Credentials {
        Credentials::new(self.reglages.access_key.0.clone(), self.reglages.secret_key.0.clone())
    }

    /// Retire les deux accès d'un texte venu du transport. Une URL présignée
    /// porte la clé d'accès en clair (`X-Amz-Credential`), et un message
    /// d'erreur peut recopier l'URL.
    fn masquer(&self, texte: &str) -> String {
        let mut t = texte.to_string();
        for a in [&self.reglages.access_key.0, &self.reglages.secret_key.0] {
            if !a.is_empty() {
                t = t.replace(a.as_str(), "***");
            }
        }
        t
    }

    /// Envoie une requête présignée et rend son statut, sans jamais citer
    /// l'URL dans une erreur.
    fn envoyer(&self, geste: &str, requete: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<u16> {
        let reponse = requete.map_err(|e| anyhow::anyhow!("{geste} : {}", self.masquer(&e.to_string())))?;
        Ok(reponse.status().as_u16())
    }

    /// La clé d'objet d'un fichier de la livraison pour la commande
    /// `reference` : `colophon/<référence>/album-print.pdf` ou
    /// `album-cover.pdf`. Refuse tout autre nom, et toute référence qui
    /// ouvrirait un autre chemin.
    pub fn objet(reference: &str, nom: &str) -> Result<Objet> {
        if nom != LIVRAISON_INTERIEUR && nom != LIVRAISON_COUVERTURE {
            bail!("le dépôt ne reçoit que {LIVRAISON_INTERIEUR} et {LIVRAISON_COUVERTURE}, pas « {nom} »");
        }
        let sure = !reference.is_empty()
            && reference.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !sure {
            bail!("référence de commande refusée pour un nom d'objet : « {reference} » (lettres, chiffres, - et _ seulement)");
        }
        Ok(Objet { cle: format!("{PREFIXE}/{reference}/{nom}") })
    }

    /// Pose `fichier` sous la référence de sa commande, par PUT présigné.
    pub fn deposer(&self, reference: &str, fichier: &Path) -> Result<Objet> {
        let nom = fichier.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let objet = Depot::objet(reference, nom)?;
        let corps = std::fs::File::open(fichier)
            .with_context(|| format!("lecture de {}", fichier.display()))?;
        let acces = self.acces();
        let url = self.seau.put_object(Some(&acces), &objet.cle).sign(DUREE_DU_GESTE);
        // Le fichier ouvert est le corps : le transport le lit au fil de
        // l'envoi et annonce sa longueur.
        let statut = self.envoyer("dépôt", self.agent.put(url.as_str()).send(corps))?;
        if !(200..300).contains(&statut) {
            bail!("le dépôt refuse {} : statut {statut}", objet.cle);
        }
        Ok(objet)
    }

    /// Vérifie les accès : pose un objet de zéro octet,
    /// `colophon/verification`, puis le retire. C'est le bouton « Vérifier »
    /// des préférences ; il ne touche à aucun objet d'une commande.
    pub fn verifier(&self) -> Result<()> {
        let objet = Objet { cle: format!("{PREFIXE}/verification") };
        let acces = self.acces();
        let url = self.seau.put_object(Some(&acces), &objet.cle).sign(DUREE_DU_GESTE);
        let statut = self.envoyer("vérification", self.agent.put(url.as_str()).send(&[][..]))?;
        if !(200..300).contains(&statut) {
            bail!("le dépôt refuse un objet de vérification : statut {statut}");
        }
        self.retirer(&objet)
    }

    /// L'URL de lecture de sept jours à poser dans la commande.
    pub fn url_de_lecture(&self, objet: &Objet) -> String {
        self.url_de_lecture_a(objet, &jiff::Timestamp::now())
    }

    fn url_de_lecture_a(&self, objet: &Objet, quand: &jiff::Timestamp) -> String {
        let acces = self.acces();
        self.seau
            .get_object(Some(&acces), &objet.cle)
            .sign_with_time(DUREE_DE_LECTURE, quand)
            .to_string()
    }

    /// Retire l'objet, par DELETE présigné. Un objet déjà absent n'est pas
    /// une erreur : S3 rend 204 dans les deux cas.
    pub fn retirer(&self, objet: &Objet) -> Result<()> {
        let acces = self.acces();
        let url = self.seau.delete_object(Some(&acces), &objet.cle).sign(DUREE_DU_GESTE);
        let statut = self.envoyer("retrait", self.agent.delete(url.as_str()).call())?;
        if !(200..300).contains(&statut) {
            bail!("le dépôt refuse de retirer {} : statut {statut}", objet.cle);
        }
        Ok(())
    }
}

/// Faut-il retirer les objets d'une commande ? Oui quand elle est expédiée,
/// annulée ou finie au sandbox, et de toute façon trente jours après le
/// dépôt. Le balayage, qui tient la liste des commandes, est l'affaire de
/// l'app (K-s3).
pub fn a_retirer(etat: Etat, depose_le: NaiveDate, aujourdhui: NaiveDate) -> bool {
    matches!(etat, Etat::Expediee | Etat::Annulee | Etat::SandboxTerminee)
        || (aujourdhui - depose_le).num_days() >= GARDE_JOURS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    const ACCES: &str = "ACCES-DE-TEST-0001";
    const SECRET: &str = "secret/de+test/0123456789abcdefghijkl";

    fn reglages(endpoint: &str) -> Reglages {
        Reglages {
            endpoint: endpoint.into(),
            region: "auto".into(),
            bucket: "albums".into(),
            access_key: Acces::new(ACCES),
            secret_key: Acces::new(SECRET),
        }
    }

    /// Ce que le serveur local a vu d'une requête.
    #[derive(Debug)]
    struct Vu {
        methode: String,
        chemin: String,
        requete: String,
        longueur_annoncee: Option<u64>,
        octets_recus: u64,
    }

    /// Un serveur S3 d'une requête, sur un port libre : il lit la ligne de
    /// requête, les en-têtes et le corps annoncé, répond `statut`, et rend
    /// ce qu'il a vu.
    fn serveur(statut: &'static str) -> (String, mpsc::Receiver<Vu>) {
        let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
        let adresse = format!("http://{}", ecoute.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (flux, _) = ecoute.accept().unwrap();
            let mut lecteur = BufReader::new(flux.try_clone().unwrap());
            let mut ligne = String::new();
            lecteur.read_line(&mut ligne).unwrap();
            let mut morceaux = ligne.split_whitespace();
            let methode = morceaux.next().unwrap().to_string();
            let cible = morceaux.next().unwrap().to_string();
            let (chemin, requete) = cible.split_once('?').unwrap_or((&cible, ""));
            let mut longueur = None;
            loop {
                let mut h = String::new();
                lecteur.read_line(&mut h).unwrap();
                if h == "\r\n" {
                    break;
                }
                if let Some((nom, valeur)) = h.split_once(':') {
                    if nom.eq_ignore_ascii_case("content-length") {
                        longueur = Some(valeur.trim().parse().unwrap());
                    }
                }
            }
            let octets_recus =
                std::io::copy(&mut (&mut lecteur).take(longueur.unwrap_or(0)), &mut std::io::sink())
                    .unwrap();
            let mut flux = flux;
            write!(flux, "HTTP/1.1 {statut}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            tx.send(Vu {
                methode,
                chemin: chemin.to_string(),
                requete: requete.to_string(),
                longueur_annoncee: longueur,
                octets_recus,
            })
            .unwrap();
        });
        (adresse, rx)
    }

    /// Le même serveur pour plusieurs requêtes à la suite, une réponse par
    /// requête.
    fn serveur_de(statuts: &'static [&'static str]) -> (String, mpsc::Receiver<Vu>) {
        let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
        let adresse = format!("http://{}", ecoute.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for statut in statuts {
                let (flux, _) = ecoute.accept().unwrap();
                let mut lecteur = BufReader::new(flux.try_clone().unwrap());
                let mut ligne = String::new();
                lecteur.read_line(&mut ligne).unwrap();
                let mut morceaux = ligne.split_whitespace();
                let methode = morceaux.next().unwrap().to_string();
                let cible = morceaux.next().unwrap().to_string();
                let (chemin, requete) = cible.split_once('?').unwrap_or((&cible, ""));
                let mut longueur = None;
                loop {
                    let mut h = String::new();
                    lecteur.read_line(&mut h).unwrap();
                    if h == "\r\n" {
                        break;
                    }
                    if let Some((nom, valeur)) = h.split_once(':') {
                        if nom.eq_ignore_ascii_case("content-length") {
                            longueur = Some(valeur.trim().parse().unwrap());
                        }
                    }
                }
                let octets_recus = std::io::copy(
                    &mut (&mut lecteur).take(longueur.unwrap_or(0)),
                    &mut std::io::sink(),
                )
                .unwrap();
                let mut flux = flux;
                write!(flux, "HTTP/1.1 {statut}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                let _ = tx.send(Vu {
                    methode,
                    chemin: chemin.to_string(),
                    requete: requete.to_string(),
                    longueur_annoncee: longueur,
                    octets_recus,
                });
            }
        });
        (adresse, rx)
    }

    #[test]
    fn la_cle_d_objet_porte_la_reference_et_le_nom_de_livraison() {
        assert_eq!(
            Depot::objet("colophon-2026-10-03-a", LIVRAISON_INTERIEUR).unwrap().cle,
            "colophon/colophon-2026-10-03-a/album-print.pdf"
        );
        assert_eq!(
            Depot::objet("CP_42", LIVRAISON_COUVERTURE).unwrap().cle,
            "colophon/CP_42/album-cover.pdf"
        );
        for nom in ["album.pdf", "album-cover.apercu.pdf", "../album-print.pdf", ""] {
            assert!(Depot::objet("a", nom).is_err(), "{nom}");
        }
        for reference in ["", "..", "a/b", "a b", "../x", "é"] {
            assert!(Depot::objet(reference, LIVRAISON_INTERIEUR).is_err(), "{reference:?}");
        }
    }

    #[test]
    fn retirer_a_l_expedition_a_l_annulation_au_bout_du_sandbox_ou_a_trente_jours() {
        let jour = |j: u32| NaiveDate::from_ymd_opt(2026, 10, j).unwrap();
        let depose = jour(1);
        for etat in [Etat::Expediee, Etat::Annulee, Etat::SandboxTerminee] {
            assert!(a_retirer(etat, depose, depose), "{etat:?}");
        }
        for etat in [
            Etat::Nouvelle,
            Etat::Verification,
            Etat::Fichiers,
            Etat::Rendu,
            Etat::Inconnu(77),
        ] {
            assert!(!a_retirer(etat, depose, depose), "{etat:?} le jour même");
            assert!(!a_retirer(etat, depose, jour(30)), "{etat:?} à 29 jours");
            assert!(a_retirer(etat, depose, jour(31)), "{etat:?} à 30 jours");
        }
        // Une horloge qui recule ne retire rien de plus.
        assert!(!a_retirer(Etat::Fichiers, jour(31), jour(1)));
    }

    /// Le vecteur de la doc AWS (« Authenticating Requests: Using Query
    /// Parameters », exemple GET de `test.txt` le 24/05/2013) : `rusty-s3`
    /// le rend au caractère près. Il est en sous-domaine, comme la doc ;
    /// notre propre forme, en chemin, est épinglée par le test suivant.
    #[test]
    fn rusty_s3_signe_le_vecteur_de_la_doc_aws() {
        let seau = Bucket::new(
            "https://s3.amazonaws.com".parse().unwrap(),
            UrlStyle::VirtualHost,
            "examplebucket",
            "us-east-1",
        )
        .unwrap();
        let acces =
            Credentials::new("AKIAIOSFODNN7EXAMPLE", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
        let quand: jiff::Timestamp = "2013-05-24T00:00:00Z".parse().unwrap();
        let url = seau
            .get_object(Some(&acces), "test.txt")
            .sign_with_time(Duration::from_secs(86400), &quand);
        assert_eq!(
            url.as_str(),
            "https://examplebucket.s3.amazonaws.com/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256\
             &X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20130524%2Fus-east-1%2Fs3%2Faws4_request\
             &X-Amz-Date=20130524T000000Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host\
             &X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"
        );
    }

    /// La signature épinglée n'est pas un relevé de `rusty-s3` recopié : elle
    /// a été recalculée le 03/10 par une SigV4 écrite à part, en Python et
    /// bibliothèque standard (`hmac`, `hashlib`), qui rend aussi le vecteur
    /// AWS ci-dessus.
    #[test]
    fn l_url_de_lecture_est_en_chemin_et_vaut_sept_jours() {
        let depot = Depot::new(reglages("https://compte.r2.cloudflarestorage.com")).unwrap();
        let objet = Depot::objet("colophon-2026-10-03-a", LIVRAISON_INTERIEUR).unwrap();
        let quand: jiff::Timestamp = "2026-10-03T12:00:00Z".parse().unwrap();
        let url = depot.url_de_lecture_a(&objet, &quand);
        assert!(url.contains("X-Amz-Expires=604800&"), "{url}");
        assert_eq!(
            url,
            "https://compte.r2.cloudflarestorage.com/albums/colophon/colophon-2026-10-03-a/album-print.pdf\
             ?X-Amz-Algorithm=AWS4-HMAC-SHA256\
             &X-Amz-Credential=ACCES-DE-TEST-0001%2F20261003%2Fauto%2Fs3%2Faws4_request\
             &X-Amz-Date=20261003T120000Z&X-Amz-Expires=604800&X-Amz-SignedHeaders=host\
             &X-Amz-Signature=c45819b01d5a51f24a2b576dad2dd073a70f774a36e1dac098066c0b75cacb26"
        );
    }

    #[test]
    fn deposer_pousse_le_fichier_en_flux_par_put_presigne() {
        let (adresse, vu) = serveur("200 OK");
        let depot = Depot::new(reglages(&adresse)).unwrap();
        let dossier = std::env::temp_dir().join(format!("colophon-depot-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let fichier = dossier.join(LIVRAISON_INTERIEUR);
        let taille = 3 * 1024 * 1024 + 17;
        std::fs::write(&fichier, vec![0x25u8; taille]).unwrap();

        let objet = depot.deposer("colophon-2026-10-03-a", &fichier).unwrap();
        std::fs::remove_dir_all(&dossier).unwrap();
        assert_eq!(objet.cle, "colophon/colophon-2026-10-03-a/album-print.pdf");
        let vu = vu.recv().unwrap();
        assert_eq!(vu.methode, "PUT");
        assert_eq!(vu.chemin, "/albums/colophon/colophon-2026-10-03-a/album-print.pdf");
        assert_eq!(vu.longueur_annoncee, Some(taille as u64));
        assert_eq!(vu.octets_recus, taille as u64);
        assert!(vu.requete.contains("X-Amz-Signature="), "{}", vu.requete);
    }

    #[test]
    fn un_put_refuse_est_une_erreur_qui_ne_dit_pas_les_acces() {
        let (adresse, _vu) = serveur("403 Forbidden");
        let depot = Depot::new(reglages(&adresse)).unwrap();
        let dossier = std::env::temp_dir().join(format!("colophon-depot-refus-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let fichier = dossier.join(LIVRAISON_COUVERTURE);
        std::fs::write(&fichier, b"%PDF-1.6").unwrap();
        let e = depot.deposer("a", &fichier).unwrap_err();
        std::fs::remove_dir_all(&dossier).unwrap();
        let texte = format!("{e:?} {e:#}");
        assert!(texte.contains("403"), "{texte}");
        assert!(!texte.contains(ACCES) && !texte.contains(SECRET), "{texte}");
    }

    #[test]
    fn verifier_pose_un_objet_vide_puis_le_retire() {
        let (adresse, vu) = serveur_de(&["200 OK", "204 No Content"]);
        let depot = Depot::new(reglages(&adresse)).unwrap();
        depot.verifier().unwrap();
        let put = vu.recv().unwrap();
        assert_eq!((put.methode.as_str(), put.chemin.as_str()), ("PUT", "/albums/colophon/verification"));
        assert_eq!((put.longueur_annoncee, put.octets_recus), (Some(0), 0));
        let del = vu.recv().unwrap();
        assert_eq!((del.methode.as_str(), del.chemin.as_str()), ("DELETE", "/albums/colophon/verification"));
    }

    #[test]
    fn une_verification_refusee_le_dit_sans_les_acces() {
        let (adresse, _vu) = serveur_de(&["403 Forbidden"]);
        let depot = Depot::new(reglages(&adresse)).unwrap();
        let e = depot.verifier().unwrap_err();
        let texte = format!("{e:?} {e:#}");
        assert!(texte.contains("403"), "{texte}");
        assert!(!texte.contains(ACCES) && !texte.contains(SECRET), "{texte}");
    }

    #[test]
    fn retirer_envoie_un_delete_presigne() {
        let (adresse, vu) = serveur("204 No Content");
        let depot = Depot::new(reglages(&adresse)).unwrap();
        depot.retirer(&Depot::objet("a", LIVRAISON_COUVERTURE).unwrap()).unwrap();
        let vu = vu.recv().unwrap();
        assert_eq!(vu.methode, "DELETE");
        assert_eq!(vu.chemin, "/albums/colophon/a/album-cover.pdf");
    }

    #[test]
    fn les_acces_ne_fuient_ni_par_debug_ni_par_une_erreur() {
        let r = reglages("https://compte.r2.cloudflarestorage.com");
        assert_eq!(format!("{:?} {}", r.access_key, r.secret_key), "*** ***");
        let depot = Depot::new(r).unwrap();
        let texte = format!("{depot:?}");
        assert!(!texte.contains(ACCES) && !texte.contains(SECRET), "{texte}");

        // Un port fermé : l'erreur de transport ne recopie aucun accès.
        let ferme = TcpListener::bind("127.0.0.1:0").unwrap();
        let adresse = format!("http://{}", ferme.local_addr().unwrap());
        drop(ferme);
        let depot = Depot::new(reglages(&adresse)).unwrap();
        let e = depot.retirer(&Depot::objet("a", LIVRAISON_INTERIEUR).unwrap()).unwrap_err();
        let texte = format!("{e:?} {e:#}");
        assert!(!texte.contains(ACCES) && !texte.contains(SECRET), "{texte}");
        // Un message de transport qui recopierait l'URL présignée (la clé
        // d'accès y est en clair) passe par le masque. Le port fermé ne le
        // provoque pas, d'où l'appel direct.
        assert_eq!(
            depot.masquer(&format!("PUT {adresse}/?X-Amz-Credential={ACCES}%2F… {SECRET}")),
            format!("PUT {adresse}/?X-Amz-Credential=***%2F… ***")
        );
        // Une adresse qui n'est pas une URL est refusée à la construction.
        assert!(Depot::new(reglages("pas une url")).is_err());
    }
}
