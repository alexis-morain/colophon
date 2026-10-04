//! La commande depuis Envoi, côté app (K-s3) : ce que l'app garde sur le
//! poste, et l'enchaînement dépôt, URL, commande, balayage.
//!
//! **Le secret vit ici, et nulle part ailleurs.** La clé Cloudprinter et
//! les accès du dépôt sont dans `commande.json`, au dossier de données de
//! l'app, en `0600` sur Unix. Le front ne les relit jamais : il reçoit une
//! [`Vue`], où la clé n'est plus que ses quatre derniers caractères et les
//! accès du dépôt n'existent pas. Rien de ce module n'écrit au journal une
//! valeur, ni le nom du fichier : le rapport de Signaler cite le journal.
//!
//! **L'adresse ne reste pas.** Elle part chez Cloudprinter dans la commande ;
//! `commande.json` ne garde de chaque commande que sa référence, son
//! identifiant, son mode, sa date, ses deux objets et le dernier état lu.

use colophon_core::commande::{self as cc, Adresse, Commande, Compte, Etat, FichierCommande, Http, Livre, Mode};
use colophon_core::depot::{self, Acces, Depot, Objet, Reglages};
use colophon_core::export::{LIVRAISON_COUVERTURE, LIVRAISON_INTERIEUR};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Le nom du fichier, au dossier de données de l'app.
pub const FICHIER: &str = "commande.json";

/// La clé et le mode qu'on lui a déclaré.
#[derive(Clone, Serialize, Deserialize)]
pub struct CleGardee {
    pub cle: String,
    pub mode: Mode,
}

/// Les cinq champs d'un dépôt S3.
#[derive(Clone, Serialize, Deserialize)]
pub struct DepotGarde {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub identifiant: String,
    pub secret: String,
}

/// Ce que l'app garde d'une commande passée. Pas l'adresse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandeGardee {
    pub reference: String,
    /// L'identifiant que Cloudprinter a rendu à `orders/add`.
    pub identifiant: String,
    pub mode: Mode,
    /// Le jour du dépôt, `AAAA-MM-JJ`.
    pub date: String,
    pub objets: Vec<Objet>,
    /// Les objets ont quitté le dépôt.
    #[serde(default)]
    pub objets_retires: bool,
    /// Le dernier numéro d'état lu (`state` d'`orders/info`).
    #[serde(default)]
    pub etat: Option<i64>,
}

/// Le contenu de `commande.json`. Pas de `Debug` : il porte les secrets.
#[derive(Default, Serialize, Deserialize)]
pub struct Stock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloudprinter: Option<CleGardee>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depot: Option<DepotGarde>,
    #[serde(default)]
    pub commandes: Vec<CommandeGardee>,
}

impl Stock {
    /// Lit `commande.json` dans `dir`. Absent, le stock est vide.
    pub fn lire(dir: &Path) -> Result<Stock, String> {
        match std::fs::read_to_string(dir.join(FICHIER)) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("réglages de commande illisibles : {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Stock::default()),
            Err(e) => Err(format!("réglages de commande illisibles : {e}")),
        }
    }

    /// Écrit `commande.json` dans `dir` : un fichier temporaire créé en
    /// `0600`, puis renommé. Le fichier n'existe jamais lisible par d'autres,
    /// même un instant.
    pub fn ecrire(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let tmp = dir.join(format!("{FICHIER}.tmp"));
        let texte = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&tmp);
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        {
            use std::io::Write;
            let mut f = o.open(&tmp).map_err(|e| format!("écriture des réglages de commande : {e}"))?;
            f.write_all(texte.as_bytes()).map_err(|e| format!("écriture des réglages de commande : {e}"))?;
        }
        std::fs::rename(&tmp, dir.join(FICHIER)).map_err(|e| format!("écriture des réglages de commande : {e}"))
    }

    /// Le compte Cloudprinter, quand une clé est enregistrée.
    pub fn compte(&self) -> Option<Compte> {
        self.cloudprinter.as_ref().map(|c| Compte { cle: cc::Cle::new(c.cle.clone()), mode: c.mode })
    }

    /// Le dépôt, quand ses accès sont enregistrés.
    pub fn depot(&self) -> Option<Result<Depot, String>> {
        self.depot.as_ref().map(|d| {
            Depot::new(Reglages {
                endpoint: d.endpoint.clone(),
                region: d.region.clone(),
                bucket: d.bucket.clone(),
                access_key: Acces::new(d.identifiant.clone()),
                secret_key: Acces::new(d.secret.clone()),
            })
            .map_err(|e| format!("{e:#}"))
        })
    }

    /// Ce que la fenêtre a le droit de voir.
    pub fn vue(&self) -> Vue {
        Vue {
            cle: self.cloudprinter.as_ref().map(|c| VueCle { fin: fin(&c.cle), mode: c.mode }),
            depot: self.depot.as_ref().map(|d| VueDepot {
                endpoint: d.endpoint.clone(),
                region: d.region.clone(),
                bucket: d.bucket.clone(),
            }),
            commandes: self.commandes.iter().rev().map(VueCommande::de).collect(),
        }
    }
}

/// Les quatre derniers caractères d'une clé, ce que l'écran en montre.
fn fin(cle: &str) -> String {
    let c: Vec<char> = cle.chars().collect();
    c[c.len().saturating_sub(4)..].iter().collect()
}

/// La clé, telle que la fenêtre la voit.
#[derive(Debug, Serialize)]
pub struct VueCle {
    /// Les quatre derniers caractères.
    pub fin: String,
    pub mode: Mode,
}

/// Le dépôt, sans ses accès.
#[derive(Debug, Serialize)]
pub struct VueDepot {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
}

/// Une commande, telle que la fenêtre la voit.
#[derive(Debug, Serialize)]
pub struct VueCommande {
    pub reference: String,
    pub identifiant: String,
    pub mode: Mode,
    pub date: String,
    pub etat: Option<i64>,
    pub objets_retires: bool,
}

impl VueCommande {
    fn de(c: &CommandeGardee) -> VueCommande {
        VueCommande {
            reference: c.reference.clone(),
            identifiant: c.identifiant.clone(),
            mode: c.mode,
            date: c.date.clone(),
            etat: c.etat,
            objets_retires: c.objets_retires,
        }
    }
}

/// Tout ce que la fenêtre reçoit des réglages et des commandes.
#[derive(Debug, Serialize)]
pub struct Vue {
    pub cle: Option<VueCle>,
    pub depot: Option<VueDepot>,
    pub commandes: Vec<VueCommande>,
}

// ---- le journal ----------------------------------------------------------------

/// Ce que la commande écrit au journal. Des phrases fixes : aucune valeur
/// saisie, aucun nom de fichier, rien qu'un rapport de Signaler ne puisse
/// citer.
#[derive(Debug, Clone, Copy)]
pub enum Evenement {
    CleEnregistree,
    CleRetiree,
    DepotEnregistre,
    DepotRetire,
    CommandePassee,
    CommandeEchouee,
    CommandeAnnulee,
}

pub fn journal(e: Evenement) {
    colophon_core::log::line(match e {
        Evenement::CleEnregistree => "commande : clé Cloudprinter enregistrée",
        Evenement::CleRetiree => "commande : clé Cloudprinter retirée",
        Evenement::DepotEnregistre => "commande : dépôt enregistré",
        Evenement::DepotRetire => "commande : dépôt retiré",
        Evenement::CommandePassee => "commande : passée",
        Evenement::CommandeEchouee => "commande : en échec, objets déposés retirés",
        Evenement::CommandeAnnulee => "commande : annulée",
    });
}

// ---- l'enchaînement ----------------------------------------------------------

/// Les trois gestes du dépôt, derrière un trait pour qu'un faux les rejoue.
pub trait Gestes {
    fn deposer(&self, reference: &str, fichier: &Path) -> Result<Objet, String>;
    fn url_de_lecture(&self, objet: &Objet) -> String;
    fn retirer(&self, objet: &Objet) -> Result<(), String>;
}

impl Gestes for Depot {
    fn deposer(&self, reference: &str, fichier: &Path) -> Result<Objet, String> {
        Depot::deposer(self, reference, fichier).map_err(|e| format!("{e:#}"))
    }
    fn url_de_lecture(&self, objet: &Objet) -> String {
        Depot::url_de_lecture(self, objet)
    }
    fn retirer(&self, objet: &Objet) -> Result<(), String> {
        Depot::retirer(self, objet).map_err(|e| format!("{e:#}"))
    }
}

/// Une étape de la commande, pour la progression à l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Etape {
    Interieur,
    Couverture,
    Commande,
}

/// Pourquoi une commande ne part pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Echec {
    /// Le livre demande un autre papier intérieur que celui du dossier
    /// préparé : la couverture porte le dos de ce papier-là, un autre
    /// ferait un livre plus épais ou plus mince que sa couverture.
    PapierRefuse { attendu: String, demande: String },
    /// Le dépôt, Cloudprinter ou la lecture d'un fichier.
    Autre(String),
}

impl std::fmt::Display for Echec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Echec::PapierRefuse { attendu, demande } => {
                write!(f, "papier_refuse : le dossier est préparé pour {attendu}, pas pour {demande}")
            }
            Echec::Autre(e) => f.write_str(e),
        }
    }
}

/// Dépose les deux PDF du `dossier` préparé, pose leurs URL de lecture de
/// sept jours et passe la commande sous le hash du devis. Une erreur à
/// mi-chemin retire ce qui a été déposé. Un livre dont le papier n'est pas
/// `papier_prepare` est refusé avant tout geste. Rend l'identifiant de
/// Cloudprinter et les deux objets posés.
#[allow(clippy::too_many_arguments)]
pub fn passer(
    http: &dyn Http,
    compte: &Compte,
    gestes: &dyn Gestes,
    reference: &str,
    dossier: &Path,
    papier_prepare: &str,
    livre: Livre,
    quote: String,
    adresse: Adresse,
    progres: &dyn Fn(Etape),
) -> Result<(String, Vec<Objet>), Echec> {
    // Le papier est la première option du livre (`Livre::choisi`).
    let demande = livre.options.first().map(|o| o.option.clone()).unwrap_or_default();
    if demande != papier_prepare {
        return Err(Echec::PapierRefuse { attendu: papier_prepare.to_string(), demande });
    }
    let mut poses: Vec<Objet> = Vec::new();
    let mut fichiers: Vec<FichierCommande> = Vec::new();
    let resultat = (|| {
        for (nom, etape) in [(LIVRAISON_INTERIEUR, Etape::Interieur), (LIVRAISON_COUVERTURE, Etape::Couverture)] {
            progres(etape);
            let chemin = dossier.join(nom);
            let objet = gestes.deposer(reference, &chemin)?;
            poses.push(objet.clone());
            fichiers.push(
                FichierCommande::depuis(gestes.url_de_lecture(&objet), &chemin).map_err(|e| format!("{e:#}"))?,
            );
        }
        progres(Etape::Commande);
        let couverture = fichiers.pop().expect("deux fichiers");
        let interieur = fichiers.pop().expect("deux fichiers");
        let commande = Commande { reference: reference.to_string(), adresse, livre, quote, couverture, interieur };
        cc::commander(http, compte, &commande).map_err(|e| e.to_string())
    })();
    match resultat {
        Ok(id) => Ok((id, poses)),
        Err(e) => {
            for o in &poses {
                if let Err(r) = gestes.retirer(o) {
                    colophon_core::log::line(&format!("commande : un objet reste au dépôt après l'échec ({r})"));
                }
            }
            Err(Echec::Autre(e))
        }
    }
}

/// Relit l'état de chaque commande dont les objets sont encore au dépôt (ou
/// de la seule `reference`), et retire ces objets quand `depot::a_retirer`
/// le dit. Une lecture qui échoue n'arrête pas les autres ; la règle des
/// trente jours s'applique alors seule. Rend la première erreur de lecture.
pub fn relire(
    http: &dyn Http,
    compte: &Compte,
    gestes: Option<&dyn Gestes>,
    stock: &mut Stock,
    reference: Option<&str>,
    aujourdhui: chrono::NaiveDate,
) -> Option<String> {
    let mut premiere = None;
    for c in stock.commandes.iter_mut() {
        if reference.is_some_and(|r| r != c.reference) {
            continue;
        }
        if reference.is_none() && c.objets_retires {
            continue;
        }
        let compte = Compte { cle: compte.cle.clone(), mode: c.mode };
        let lu = match cc::etat(http, &compte, &c.reference) {
            Ok(e) => {
                c.etat = Some(code_de(e, c.etat));
                Some(e)
            }
            Err(e) => {
                premiere.get_or_insert(e.to_string());
                None
            }
        };
        if c.objets_retires {
            continue;
        }
        let depose = chrono::NaiveDate::parse_from_str(&c.date, "%Y-%m-%d").unwrap_or(aujourdhui);
        if depot::a_retirer(lu.unwrap_or(Etat::Inconnu(0)), depose, aujourdhui) {
            if let Some(g) = gestes {
                let tous = c.objets.iter().all(|o| g.retirer(o).is_ok());
                c.objets_retires = tous;
            }
        }
    }
    premiere
}

/// Le numéro d'un état, pour `commande.json`. `Etat` range un intervalle
/// dans une variante : le numéro exact se garde quand il est connu.
fn code_de(e: Etat, avant: Option<i64>) -> i64 {
    match e {
        Etat::Nouvelle => 1,
        Etat::Verification => avant.filter(|c| (5..=7).contains(c)).unwrap_or(5),
        Etat::Fichiers => avant.filter(|c| (10..=15).contains(c)).unwrap_or(10),
        Etat::Rendu => avant.filter(|c| (30..=45).contains(c)).unwrap_or(30),
        Etat::Expediee => 100,
        Etat::Annulee => 500,
        Etat::SandboxTerminee => 501,
        Etat::Inconnu(c) => c,
    }
}

/// Une référence de commande sûre pour un nom d'objet : lettres, chiffres
/// et tirets.
pub fn reference_neuve(maintenant: chrono::DateTime<chrono::Utc>) -> String {
    format!("colophon-{}", maintenant.format("%Y%m%d-%H%M%S"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::cell::RefCell;
    use std::path::PathBuf;

    const CLE: &str = "cle-de-test-0123456789abcd";
    const SECRET: &str = "secret-du-depot-0123456789";
    const IDENTIFIANT: &str = "IDENTIFIANT-0001";

    fn dossier(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("colophon-commande-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn stock_plein() -> Stock {
        Stock {
            cloudprinter: Some(CleGardee { cle: CLE.into(), mode: Mode::Sandbox }),
            depot: Some(DepotGarde {
                endpoint: "https://compte.r2.cloudflarestorage.com".into(),
                region: "auto".into(),
                bucket: "seau-de-test".into(),
                identifiant: IDENTIFIANT.into(),
                secret: SECRET.into(),
            }),
            commandes: vec![],
        }
    }

    #[test]
    fn commande_json_se_relit_et_n_est_lisible_que_par_soi() {
        let d = dossier("perm");
        assert!(Stock::lire(&d).unwrap().cloudprinter.is_none(), "absent, le stock est vide");
        stock_plein().ecrire(&d).unwrap();
        let relu = Stock::lire(&d).unwrap();
        assert_eq!(relu.cloudprinter.unwrap().cle, CLE);
        assert_eq!(relu.depot.unwrap().secret, SECRET);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(d.join(FICHIER)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{mode:o}");
            // Réécrit par-dessus, il reste en 0600.
            stock_plein().ecrire(&d).unwrap();
            let mode = std::fs::metadata(d.join(FICHIER)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{mode:o}");
        }
        assert!(!d.join(format!("{FICHIER}.tmp")).exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn la_vue_ne_porte_aucun_secret() {
        let mut s = stock_plein();
        s.commandes.push(CommandeGardee {
            reference: "colophon-20261003-120000".into(),
            identifiant: "CP-1".into(),
            mode: Mode::Sandbox,
            date: "2026-10-03".into(),
            objets: vec![Objet { cle: "colophon/colophon-20261003-120000/album-print.pdf".into() }],
            objets_retires: false,
            etat: Some(10),
        });
        let v = serde_json::to_string(&s.vue()).unwrap();
        assert!(!v.contains(CLE) && !v.contains(SECRET) && !v.contains(IDENTIFIANT), "{v}");
        let v: Value = serde_json::from_str(&v).unwrap();
        assert_eq!(v["cle"], json!({"fin": "abcd", "mode": "Sandbox"}));
        assert_eq!(v["depot"]["bucket"], "seau-de-test");
        assert_eq!(v["commandes"][0]["etat"], 10);
        assert!(v["commandes"][0].get("objets").is_none());
        assert_eq!(fin("ab"), "ab");
    }

    /// Un faux CloudCore : une réponse par point d'entrée.
    struct Faux {
        reponses: Vec<(&'static str, u16, String)>,
        recu: RefCell<Vec<(String, Value)>>,
    }

    impl Http for Faux {
        fn post_json(&self, url: &str, corps: &Value) -> Result<(u16, String), String> {
            self.recu.borrow_mut().push((url.to_string(), corps.clone()));
            let (_, s, c) = self
                .reponses
                .iter()
                .find(|(p, ..)| url.ends_with(p))
                .unwrap_or_else(|| panic!("point inattendu : {url}"));
            Ok((*s, c.clone()))
        }
    }

    fn faux(reponses: Vec<(&'static str, u16, &str)>) -> Faux {
        Faux {
            reponses: reponses.into_iter().map(|(p, s, c)| (p, s, c.to_string())).collect(),
            recu: RefCell::new(Vec::new()),
        }
    }

    /// Un faux dépôt : garde ce qu'on pose et ce qu'on retire, et refuse le
    /// fichier qu'on lui nomme.
    #[derive(Default)]
    struct FauxDepot {
        refuse: Option<&'static str>,
        poses: RefCell<Vec<String>>,
        retires: RefCell<Vec<String>>,
    }

    impl Gestes for FauxDepot {
        fn deposer(&self, reference: &str, fichier: &Path) -> Result<Objet, String> {
            let nom = fichier.file_name().unwrap().to_str().unwrap();
            if self.refuse == Some(nom) {
                return Err("le dépôt refuse : statut 403".into());
            }
            let o = Depot::objet(reference, nom).map_err(|e| e.to_string())?;
            self.poses.borrow_mut().push(o.cle.clone());
            Ok(o)
        }
        fn url_de_lecture(&self, objet: &Objet) -> String {
            format!("https://depot.example/{}?X-Amz-Signature=1", objet.cle)
        }
        fn retirer(&self, objet: &Objet) -> Result<(), String> {
            self.retires.borrow_mut().push(objet.cle.clone());
            Ok(())
        }
    }

    fn prepare(tag: &str) -> PathBuf {
        let d = dossier(tag);
        std::fs::write(d.join(LIVRAISON_INTERIEUR), b"%PDF interieur").unwrap();
        std::fs::write(d.join(LIVRAISON_COUVERTURE), b"%PDF couverture").unwrap();
        d
    }

    fn adresse() -> Adresse {
        Adresse {
            prenom: "A".into(),
            nom: "B".into(),
            rue: "1 rue".into(),
            rue2: None,
            code_postal: "75001".into(),
            ville: "Paris".into(),
            pays: "FR".into(),
            email: "a@example.org".into(),
            telephone: "+33100000000".into(),
        }
    }

    fn livre() -> Livre {
        Livre::choisi("photobook_cw_s210_s_fc", "pageblock_150mcs", "cover_130mcg", "cover_finish_matte", 96, 2)
    }

    fn compte() -> Compte {
        Compte { cle: cc::Cle::new(CLE), mode: Mode::Sandbox }
    }

    #[test]
    fn passer_depose_les_deux_pdf_puis_commande_avec_le_devis() {
        let d = prepare("passer");
        let http = faux(vec![("orders/add", 201, r#"{"order": "CP-77"}"#)]);
        let g = FauxDepot::default();
        let etapes = RefCell::new(Vec::new());
        let (id, objets) = passer(&http, &compte(), &g, "colophon-x", &d, "pageblock_150mcs", livre(), "h4sh".into(), adresse(), &|e| {
            etapes.borrow_mut().push(e)
        })
        .unwrap();
        assert_eq!(id, "CP-77");
        assert_eq!(objets.len(), 2);
        assert_eq!(*etapes.borrow(), [Etape::Interieur, Etape::Couverture, Etape::Commande]);
        assert!(g.retires.borrow().is_empty());
        let (_, corps) = http.recu.borrow().last().cloned().unwrap();
        let item = &corps["items"][0];
        assert_eq!(item["quote"], "h4sh");
        assert_eq!(item["count"], "2");
        assert_eq!(item["files"][0]["type"], "cover");
        assert!(item["files"][0]["url"].as_str().unwrap().contains("album-cover.pdf"));
        assert_eq!(item["files"][1]["md5sum"], colophon_core::export::md5_du_fichier(&d.join(LIVRAISON_INTERIEUR)).unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Décision d'Alexis (04/10) : le papier intérieur est celui du dossier
    /// préparé, `pageblock_150mcs`, dont la couverture porte le dos. Un autre
    /// papier est refusé avant tout dépôt et toute commande.
    #[test]
    fn passer_refuse_un_autre_papier_que_celui_du_dossier() {
        let d = prepare("papier");
        let http = faux(vec![("orders/add", 201, r#"{"order": "CP-1"}"#)]);
        let g = FauxDepot::default();
        let autre =
            Livre::choisi("photobook_cw_s210_s_fc", "pageblock_200mcg", "cover_130mcg", "cover_finish_matte", 96, 1);
        let e = passer(&http, &compte(), &g, "colophon-p", &d, "pageblock_150mcs", autre, "h".into(), adresse(), &|_| {})
            .unwrap_err();
        assert_eq!(
            e,
            Echec::PapierRefuse { attendu: "pageblock_150mcs".into(), demande: "pageblock_200mcg".into() }
        );
        assert!(g.poses.borrow().is_empty(), "rien n'est déposé");
        assert!(http.recu.borrow().is_empty(), "rien n'est commandé");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn une_erreur_a_mi_chemin_retire_ce_qui_a_ete_depose() {
        let d = prepare("mi-chemin");
        // La couverture est refusée : l'intérieur, déjà posé, est retiré.
        let g = FauxDepot { refuse: Some(LIVRAISON_COUVERTURE), ..Default::default() };
        let http = faux(vec![]);
        assert!(passer(&http, &compte(), &g, "colophon-y", &d, "pageblock_150mcs", livre(), "h".into(), adresse(), &|_| {}).is_err());
        assert_eq!(*g.retires.borrow(), ["colophon/colophon-y/album-print.pdf"]);
        assert!(http.recu.borrow().is_empty(), "aucune commande sans les deux fichiers");

        // Cloudprinter refuse la commande : les deux objets sont retirés.
        let g = FauxDepot::default();
        let http = faux(vec![("orders/add", 400, r#"{"error": {"type": "quote_expired"}}"#)]);
        let e = passer(&http, &compte(), &g, "colophon-z", &d, "pageblock_150mcs", livre(), "h".into(), adresse(), &|_| {}).unwrap_err();
        assert!(e.to_string().contains("quote_expired"), "{e}");
        assert_eq!(g.retires.borrow().len(), 2);
        std::fs::remove_dir_all(&d).unwrap();
    }

    fn gardee(reference: &str, date: &str) -> CommandeGardee {
        CommandeGardee {
            reference: reference.into(),
            identifiant: "CP".into(),
            mode: Mode::Sandbox,
            date: date.into(),
            objets: vec![
                Depot::objet(reference, LIVRAISON_INTERIEUR).unwrap(),
                Depot::objet(reference, LIVRAISON_COUVERTURE).unwrap(),
            ],
            objets_retires: false,
            etat: None,
        }
    }

    #[test]
    fn relire_garde_l_etat_et_retire_les_objets_d_une_commande_finie() {
        let jour = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        let mut s = stock_plein();
        s.commandes.push(gardee("colophon-a", "2026-10-03"));
        let http = faux(vec![("orders/info", 200, r#"{"state": 501, "state_code": "order_state_sandbox_done"}"#)]);
        let g = FauxDepot::default();
        assert_eq!(relire(&http, &compte(), Some(&g), &mut s, None, jour), None);
        assert_eq!(s.commandes[0].etat, Some(501));
        assert!(s.commandes[0].objets_retires);
        assert_eq!(g.retires.borrow().len(), 2);

        // En cours, les objets restent ; à trente jours, ils partent même
        // sans état lisible.
        let mut s = stock_plein();
        s.commandes.push(gardee("colophon-b", "2026-10-03"));
        s.commandes.push(gardee("colophon-c", "2026-09-01"));
        let http = faux(vec![("orders/info", 200, r#"{"state": 10}"#)]);
        let g = FauxDepot::default();
        relire(&http, &compte(), Some(&g), &mut s, None, jour);
        assert_eq!(s.commandes[0].etat, Some(10));
        assert!(!s.commandes[0].objets_retires);
        assert!(s.commandes[1].objets_retires);

        let http = faux(vec![("orders/info", 403, "")]);
        let mut s = stock_plein();
        s.commandes.push(gardee("colophon-d", "2026-09-01"));
        let g = FauxDepot::default();
        let e = relire(&http, &compte(), Some(&g), &mut s, None, jour).unwrap();
        assert!(e.contains("clé"), "{e}");
        assert!(s.commandes[0].objets_retires, "trente jours suffisent");
    }

    #[test]
    fn la_reference_est_un_nom_d_objet_sur() {
        let t = chrono::DateTime::parse_from_rfc3339("2026-10-03T14:05:09Z").unwrap().with_timezone(&chrono::Utc);
        let r = reference_neuve(t);
        assert_eq!(r, "colophon-20261003-140509");
        assert!(Depot::objet(&r, LIVRAISON_INTERIEUR).is_ok());
    }
}
