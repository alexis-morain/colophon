//! La commande chez Cloudprinter : devis, commande, état, annulation (K-s2).
//!
//! Le moteur parle à CloudCore 1.0 et à rien d'autre : POST JSON partout, la
//! clé dans le corps, jamais dans une URL. Tout passe par le trait [`Http`],
//! qu'un faux remplace en test : aucun test du gate ne touche le réseau.
//!
//! **Les types parlent la langue de l'API sans la supposer.** Le sandbox a
//! rendu le niveau `cp_limited`, absent de la doc : un niveau d'expédition
//! est donc une chaîne lue dans le devis, jamais une variante d'`enum`. Les
//! montants restent les chaînes que l'API écrit (`"11.8333"`), parce qu'un
//! prix n'a rien à faire dans un flottant.
//!
//! **Le mode n'est pas lu, il est déclaré.** Sondé le 03/10 avec la clé
//! sandbox : `orders/`, `products`, `products/info`, `shipping/countries` et
//! `orders/quote` ne disent nulle part s'ils répondent au sandbox, ni dans le
//! corps ni dans les en-têtes. Le [`Mode`] voyage donc avec la clé, et l'état
//! 501, que seul le sandbox atteint, le confirme après coup
//! ([`Etat::mode`]).

use crate::printer::{OptionsCommande, PrinterProfile};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::path::Path;

/// La base de CloudCore 1.0. Le sandbox a la même : c'est un mode de
/// l'interface, pas une autre adresse.
pub const BASE: &str = "https://api.cloudprinter.com/cloudcore/1.0/";

/// Le produit convenu : cartonné 210 × 210, intérieur en pages simples,
/// couverture séparée.
pub const PRODUIT: &str = "photobook_cw_s210_s_fc";

/// La devise demandée au devis.
pub const DEVISE: &str = "EUR";

/// La référence de l'unique article d'un devis et de sa commande. Le hash
/// d'un devis tient l'article par sa référence : la commande doit la
/// reprendre telle quelle, sinon CloudCore refuse le devis.
pub const ARTICLE: &str = "livre";

/// Les cotes qu'un produit a rendues à `products/info`, et les bornes de
/// pages que le devis a acceptées, relevées le 03/10 sur le compte
/// (`docs/mesures/2026-10-03-les-produits.json`). Gardées ici pour qu'un test
/// confronte chaque association au profil, au dixième de millimètre.
#[derive(Debug, Clone, Copy)]
pub struct CotesProduit {
    pub produit: &'static str,
    pub largeur_mm: f64,
    pub hauteur_mm: f64,
    pub fond_perdu_mm: f64,
    pub squeeze_mm: f64,
    pub overlap_mm: f64,
    pub wrap_mm: f64,
    /// Le plus petit compte pair que le devis accepte.
    pub pages_min: usize,
    /// Le plus grand compte pair que le devis accepte.
    pub pages_max: usize,
}

const fn cotes(produit: &'static str, largeur_mm: f64, hauteur_mm: f64, pages_min: usize) -> CotesProduit {
    CotesProduit {
        produit,
        largeur_mm,
        hauteur_mm,
        fond_perdu_mm: 3.0,
        squeeze_mm: 5.0,
        overlap_mm: 3.0,
        wrap_mm: 18.0,
        pages_min,
        pages_max: 846,
    }
}

/// Les quatre cartonnés retenus. Les quarante-trois du catalogue ont tous
/// le même fond perdu, squeeze, overlap et wrap : c'est la largeur et la
/// hauteur qui tranchent.
pub const COTES_DES_PRODUITS: [CotesProduit; 4] = [
    cotes("photobook_cw_s210_s_fc", 210.0, 210.0, 24),
    cotes("photobook_cw_s300_s_fc", 300.0, 300.0, 22),
    cotes("photobook_cw_a4_p_fc", 210.0, 297.0, 22),
    cotes("photobook_cw_a4_l_fc", 297.0, 210.0, 24),
];

/// Le produit Cloudprinter d'un format de Colophon (`format::FORMATS`), ou
/// aucun. Un format n'a de produit que si les cotes que l'API rend pour lui
/// égalent celles du profil au dixième de millimètre : un produit voisin
/// recevrait un fichier qu'il couperait ailleurs.
pub fn produit_pour(format: &str) -> Option<&'static str> {
    match format {
        "carre-21" => Some(PRODUIT),
        "carre-30" => Some("photobook_cw_s300_s_fc"),
        "portrait-a4" => Some("photobook_cw_a4_p_fc"),
        "paysage-a4" => Some("photobook_cw_a4_l_fc"),
        _ => None,
    }
}

// ---- le secret ---------------------------------------------------------------

/// La clé CloudCore. Elle dépense l'argent du compte : son `Debug` et son
/// `Display` rendent `***`, elle n'a pas de `Serialize`, et seul le corps
/// d'une requête la lit.
#[derive(Clone)]
pub struct Cle(String);

impl Cle {
    pub fn new(valeur: impl Into<String>) -> Self {
        Cle(valeur.into())
    }

    fn secret(&self) -> &str {
        &self.0
    }

    /// Retire la clé d'un texte venu d'ailleurs (un corps d'erreur, un
    /// message de transport) avant qu'il entre dans une [`Erreur`].
    fn masquer(&self, texte: &str) -> String {
        if self.0.is_empty() {
            texte.to_string()
        } else {
            texte.replace(&self.0, "***")
        }
    }
}

impl fmt::Debug for Cle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for Cle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Sandbox ou réel, déclaré avec la clé : aucune réponse de l'API ne le dit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Sandbox,
    Reel,
}

/// Une clé et le mode qu'on lui déclare.
#[derive(Debug, Clone)]
pub struct Compte {
    pub cle: Cle,
    pub mode: Mode,
}

// ---- le transport ------------------------------------------------------------

/// La seule porte vers le réseau : un POST JSON, rendu en statut et corps.
/// Une erreur de transport est un texte ; un statut HTTP, même 4xx, n'en est
/// pas une à ce niveau.
pub trait Http {
    fn post_json(&self, url: &str, corps: &Value) -> Result<(u16, String), String>;
}

/// Le transport réel, synchrone, TLS par rustls.
pub struct Ureq(ureq::Agent);

impl Default for Ureq {
    fn default() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(60)))
            .build()
            .into();
        Ureq(agent)
    }
}

impl Http for Ureq {
    fn post_json(&self, url: &str, corps: &Value) -> Result<(u16, String), String> {
        let mut r = self
            .0
            .post(url)
            .header("Content-Type", "application/json")
            .send(corps.to_string())
            .map_err(|e| e.to_string())?;
        let statut = r.status().as_u16();
        let texte = r.body_mut().read_to_string().map_err(|e| e.to_string())?;
        Ok((statut, texte))
    }
}

// ---- les erreurs -------------------------------------------------------------

/// Ce que CloudCore refuse, typé par son code, et ce que le transport rate.
/// Aucune ne recopie la clé : tout texte venu de dehors passe par
/// [`Cle::masquer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erreur {
    /// 400 : la requête est refusée, avec la raison que l'API donne.
    Refusee(String),
    /// 403 : la clé n'est pas reconnue.
    Cle,
    /// 409 : la commande n'est plus dans un état qui le permet.
    TropTard,
    /// 410 : aucune commande sous cette référence.
    Introuvable,
    /// Un autre statut, que la doc ne nomme pas.
    Statut(u16),
    /// Le réseau, avant tout statut.
    Reseau(String),
    /// Un corps que le moteur ne sait pas lire.
    Reponse(String),
}

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erreur::Refusee(raison) => write!(f, "Cloudprinter refuse la requête : {raison}"),
            Erreur::Cle => f.write_str("Cloudprinter ne reconnaît pas la clé"),
            Erreur::TropTard => {
                f.write_str("la production a commencé, l'annulation n'est plus possible")
            }
            Erreur::Introuvable => f.write_str("Cloudprinter ne connaît aucune commande sous cette référence"),
            Erreur::Statut(s) => write!(f, "Cloudprinter répond {s}, un statut que sa doc ne nomme pas"),
            Erreur::Reseau(e) => write!(f, "Cloudprinter injoignable : {e}"),
            Erreur::Reponse(e) => write!(f, "réponse de Cloudprinter illisible : {e}"),
        }
    }
}

impl std::error::Error for Erreur {}

// ---- ce qu'on imprime --------------------------------------------------------

/// Une option du produit, sous le nom que `products/info` lui donne
/// (`reference`), avec son compte. Les catégories (`type_main_paper`,
/// `type_cover_paper`, `type_book_cover_finish`, `type_total_pages`) ne
/// voyagent pas : la commande nomme l'option elle-même.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OptionLivre {
    pub option: String,
    pub compte: u32,
}

/// Le livre tel que Cloudprinter le facture : un produit, ses options, et
/// combien d'exemplaires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Livre {
    pub produit: String,
    pub options: Vec<OptionLivre>,
    pub exemplaires: u32,
}

impl Livre {
    /// Le livre de `pages` pages chez un fournisseur qui a des codes de
    /// commande (`PrinterProfile::commande`), `None` chez les autres, à un
    /// exemplaire du produit convenu.
    pub fn pour(profil: &PrinterProfile, pages: u32) -> Option<Livre> {
        let OptionsCommande { papier, couverture, finition } = profil.commande?;
        Some(Livre::choisi(PRODUIT, papier, couverture, finition, pages, 1))
    }

    /// Le livre tel que l'écran Commander le compose : un produit, et les
    /// options que son `products/info` propose. Le papier compte les pages,
    /// comme `scripts/compare-prix.py` l'a mesuré le 26/09 ; la couverture et
    /// sa finition comptent un.
    pub fn choisi(
        produit: &str,
        papier: &str,
        couverture: &str,
        finition: &str,
        pages: u32,
        exemplaires: u32,
    ) -> Livre {
        let option = |o: &str, compte| OptionLivre { option: o.to_string(), compte };
        Livre {
            produit: produit.to_string(),
            options: vec![
                option(papier, pages),
                option(couverture, 1),
                option(finition, 1),
                option("total_pages", pages),
            ],
            exemplaires,
        }
    }
}

/// Le corps JSON des options, dans la forme mesurée au sandbox : des
/// comptes écrits en chaînes.
fn options_json(livre: &Livre) -> Value {
    livre
        .options
        .iter()
        .map(|o| json!({"type": o.option, "count": o.compte.to_string()}))
        .collect()
}

/// Envoie `corps` (la clé y est ajoutée) à `point` et rend le corps d'un
/// 200 ou d'un 201, ou l'erreur typée de tout autre statut.
fn appeler(http: &dyn Http, compte: &Compte, point: &str, mut corps: Value) -> Result<String, Erreur> {
    corps["apikey"] = Value::String(compte.cle.secret().to_string());
    let (statut, texte) = http
        .post_json(&format!("{BASE}{point}"), &corps)
        .map_err(|e| Erreur::Reseau(compte.cle.masquer(&e)))?;
    match statut {
        200 | 201 => Ok(texte),
        400 => Err(Erreur::Refusee(compte.cle.masquer(&raison(&texte)))),
        403 => Err(Erreur::Cle),
        409 => Err(Erreur::TropTard),
        410 => Err(Erreur::Introuvable),
        autre => Err(Erreur::Statut(autre)),
    }
}

/// La raison d'un refus, telle que CloudCore l'écrit
/// (`{"error": {"type", "info"}}`), ou le corps brut s'il dit autre chose.
fn raison(texte: &str) -> String {
    let lu: Option<Value> = serde_json::from_str(texte).ok();
    let e = lu.as_ref().map(|v| &v["error"]);
    match e.map(|e| (e["type"].as_str(), e["info"].as_str())) {
        Some((Some(t), Some(i))) => format!("{t} : {i}"),
        Some((Some(t), None)) => t.to_string(),
        Some((None, Some(i))) => i.to_string(),
        _ => texte.trim().to_string(),
    }
}

/// Lit un corps JSON, ou dit pourquoi il ne se lit pas. Le message de serde
/// peut citer une valeur du corps : il passe par le masque lui aussi.
fn lire<T: serde::de::DeserializeOwned>(compte: &Compte, texte: &str) -> Result<T, Erreur> {
    serde_json::from_str(texte).map_err(|e| Erreur::Reponse(compte.cle.masquer(&e.to_string())))
}

// ---- le devis ----------------------------------------------------------------

/// Un nombre que l'API écrit tantôt en chaîne, tantôt en nombre, gardé tel
/// qu'écrit.
fn texte_ou_nombre<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    match Value::deserialize(d)? {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        autre => Err(serde::de::Error::custom(format!("ni texte ni nombre : {autre}"))),
    }
}

fn texte_ou_nombre_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(autre) => Err(serde::de::Error::custom(format!("ni texte ni nombre : {autre}"))),
    }
}

/// Le devis d'`orders/quote`, valable jusqu'à `expire_date` (48 h mesurées).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Devis {
    /// Les articles, hors taxe.
    #[serde(rename(deserialize = "price"), deserialize_with = "texte_ou_nombre")]
    pub prix: String,
    #[serde(rename(deserialize = "vat"), deserialize_with = "texte_ou_nombre")]
    pub tva: String,
    #[serde(rename(deserialize = "currency"))]
    pub devise: String,
    pub expire_date: String,
    /// Les expéditions telles que rendues, chacune avec ses offres.
    #[serde(rename(deserialize = "shipments"), default)]
    pub expeditions: Vec<Expedition>,
}

/// Une expédition du devis : un colis, et les façons de l'envoyer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expedition {
    #[serde(rename(deserialize = "total_weight"), default, deserialize_with = "texte_ou_nombre_opt")]
    pub poids_g: Option<String>,
    #[serde(rename(deserialize = "quotes"), default)]
    pub offres: Vec<Offre>,
}

/// Une offre d'expédition. `quote` est le hash à poser sur la commande à la
/// place d'un niveau : il fixe le prix et l'expédition ensemble.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offre {
    pub quote: String,
    /// Le niveau, tel que rendu (`cp_limited` n'est pas dans la doc).
    #[serde(rename(deserialize = "shipping_level"))]
    pub niveau: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(rename(deserialize = "shipping_option"), default)]
    pub transporteur: Option<String>,
    #[serde(rename(deserialize = "price"), deserialize_with = "texte_ou_nombre")]
    pub prix: String,
    #[serde(rename(deserialize = "vat"), deserialize_with = "texte_ou_nombre")]
    pub tva: String,
    #[serde(rename(deserialize = "currency"))]
    pub devise: String,
}

/// Demande un devis pour `livre` (ses exemplaires compris) livré dans `pays`
/// (code ISO à deux lettres).
pub fn devis(http: &dyn Http, compte: &Compte, pays: &str, livre: &Livre) -> Result<Devis, Erreur> {
    let corps = json!({
        "country": pays,
        "currency": DEVISE,
        "items": [{
            "reference": ARTICLE,
            "product": livre.produit,
            "count": livre.exemplaires.to_string(),
            "options": options_json(livre),
        }],
    });
    lire(compte, &appeler(http, compte, "orders/quote", corps)?)
}

// ---- le catalogue -------------------------------------------------------------

/// Un produit du catalogue du compte (`products`), tel que listé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Produit {
    pub reference: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Le catalogue que le compte a activé. Rien d'autre n'y est lu que la
/// liste : c'est la preuve que la clé ouvre le compte.
pub fn produits(http: &dyn Http, compte: &Compte) -> Result<Vec<Produit>, Erreur> {
    lire(compte, &appeler(http, compte, "products", json!({}))?)
}

/// Le type d'option du papier intérieur.
pub const TYPE_PAPIER: &str = "type_main_paper";
/// Le type d'option du papier de couverture.
pub const TYPE_COUVERTURE: &str = "type_cover_paper";
/// Le type d'option de la finition de couverture.
pub const TYPE_FINITION: &str = "type_book_cover_finish";

/// Une option d'un produit, telle que `products/info` la rend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionProduit {
    pub reference: String,
    #[serde(rename(deserialize = "type"))]
    pub type_: String,
    #[serde(default)]
    pub note: Option<String>,
    /// Le choix par défaut du produit chez Cloudprinter (`default: 1`).
    #[serde(rename(deserialize = "default"), default, deserialize_with = "drapeau")]
    pub defaut: bool,
}

/// Une cote d'un produit : sa référence et sa valeur, telle qu'écrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    pub reference: String,
    #[serde(deserialize_with = "texte_ou_nombre")]
    pub value: String,
}

/// Ce que `products/info` dit d'un produit : ses options et ses cotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfosProduit {
    pub reference: String,
    #[serde(default)]
    pub options: Vec<OptionProduit>,
    #[serde(default)]
    pub specs: Vec<Spec>,
}

impl InfosProduit {
    /// Les options d'un type, dans l'ordre où l'API les rend.
    pub fn options_de<'a>(&'a self, type_: &'a str) -> impl Iterator<Item = &'a OptionProduit> + 'a {
        self.options.iter().filter(move |o| o.type_ == type_)
    }

    /// La valeur d'une cote, telle qu'écrite.
    pub fn spec(&self, reference: &str) -> Option<&str> {
        self.specs.iter().find(|s| s.reference == reference).map(|s| s.value.as_str())
    }
}

/// `0` ou `1`, en nombre comme en texte, ou un booléen.
fn drapeau<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_i64() == Some(1),
        Value::String(s) => s.trim() == "1",
        _ => false,
    })
}

/// Les options et les cotes d'un produit.
pub fn infos_produit(http: &dyn Http, compte: &Compte, reference: &str) -> Result<InfosProduit, Erreur> {
    lire(compte, &appeler(http, compte, "products/info", json!({"reference": reference}))?)
}

/// Un pays où Cloudprinter livre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pays {
    #[serde(rename(deserialize = "country_reference"))]
    pub code: String,
    #[serde(rename(deserialize = "note"))]
    pub nom: String,
    /// Le pays demande un État ou une province dans l'adresse.
    #[serde(rename(deserialize = "require_state"), default, deserialize_with = "drapeau")]
    pub etat_requis: bool,
}

/// Les pays de livraison (`shipping/countries`).
pub fn pays(http: &dyn Http, compte: &Compte) -> Result<Vec<Pays>, Erreur> {
    lire(compte, &appeler(http, compte, "shipping/countries", json!({}))?)
}

// ---- la commande -------------------------------------------------------------

/// L'adresse de livraison, aux champs que `orders/add` demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adresse {
    pub prenom: String,
    pub nom: String,
    pub rue: String,
    pub rue2: Option<String>,
    pub code_postal: String,
    pub ville: String,
    /// Code ISO à deux lettres.
    pub pays: String,
    pub email: String,
    pub telephone: String,
}

/// Un fichier que Cloudprinter tirera à l'acceptation : son URL et son MD5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierCommande {
    pub url: String,
    pub md5: String,
}

impl FichierCommande {
    /// Le fichier posé à `url`, dont le MD5 se lit sur `chemin` par
    /// [`crate::export::md5_du_fichier`], le hachage de K-s1 et pas un autre.
    pub fn depuis(url: impl Into<String>, chemin: &Path) -> anyhow::Result<FichierCommande> {
        Ok(FichierCommande { url: url.into(), md5: crate::export::md5_du_fichier(chemin)? })
    }
}

/// Une commande, d'autant d'exemplaires que son livre en porte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commande {
    /// La référence, de notre côté ; c'est elle qui nomme les objets du dépôt.
    pub reference: String,
    pub adresse: Adresse,
    pub livre: Livre,
    /// Le hash d'une [`Offre`] du devis, à la place d'un niveau d'expédition.
    pub quote: String,
    pub couverture: FichierCommande,
    pub interieur: FichierCommande,
}

/// Passe la commande (`orders/add`) et rend l'identifiant que Cloudprinter
/// lui donne.
pub fn commander(http: &dyn Http, compte: &Compte, commande: &Commande) -> Result<String, Erreur> {
    let a = &commande.adresse;
    let mut adresse = json!({
        "type": "delivery",
        "firstname": a.prenom,
        "lastname": a.nom,
        "street1": a.rue,
        "zip": a.code_postal,
        "city": a.ville,
        "country": a.pays,
        "email": a.email,
        "phone": a.telephone,
    });
    if let Some(rue2) = &a.rue2 {
        adresse["street2"] = Value::String(rue2.clone());
    }
    let fichier = |type_: &str, f: &FichierCommande| json!({"type": type_, "url": f.url, "md5sum": f.md5});
    let corps = json!({
        "reference": commande.reference,
        "email": a.email,
        "addresses": [adresse],
        "items": [{
            "reference": ARTICLE,
            "product": commande.livre.produit,
            "quote": commande.quote,
            "count": commande.livre.exemplaires.to_string(),
            "files": [fichier("cover", &commande.couverture), fichier("book", &commande.interieur)],
            "options": options_json(&commande.livre),
        }],
    });
    #[derive(Deserialize)]
    struct Reponse {
        #[serde(deserialize_with = "texte_ou_nombre")]
        order: String,
    }
    let r: Reponse = lire(compte, &appeler(http, compte, "orders/add", corps)?)?;
    Ok(r.order)
}

// ---- l'état ------------------------------------------------------------------

/// L'état d'une commande, rangé depuis le `state` d'`orders/info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Etat {
    /// 1.
    Nouvelle,
    /// 5 à 7.
    Verification,
    /// 10 à 15 : Cloudprinter tire les fichiers.
    Fichiers,
    /// 30 à 45.
    Rendu,
    /// 100.
    Expediee,
    /// 500.
    Annulee,
    /// 501 : une commande du sandbox arrivée au bout.
    SandboxTerminee,
    /// Un code que la doc ne range pas : jamais une erreur.
    Inconnu(i64),
}

impl Etat {
    pub fn du_code(code: i64) -> Etat {
        match code {
            1 => Etat::Nouvelle,
            5..=7 => Etat::Verification,
            10..=15 => Etat::Fichiers,
            30..=45 => Etat::Rendu,
            100 => Etat::Expediee,
            500 => Etat::Annulee,
            501 => Etat::SandboxTerminee,
            autre => Etat::Inconnu(autre),
        }
    }

    /// Ce que l'état dit du mode : seul le sandbox atteint 501.
    pub fn mode(self) -> Option<Mode> {
        (self == Etat::SandboxTerminee).then_some(Mode::Sandbox)
    }
}

/// Relit l'état de la commande `reference` (`orders/info`). Le numéro est
/// dans `state` ; `state_code` en est le nom (`order_state_sandbox_done`),
/// relevé au sandbox le 03/10.
pub fn etat(http: &dyn Http, compte: &Compte, reference: &str) -> Result<Etat, Erreur> {
    #[derive(Deserialize)]
    struct Reponse {
        #[serde(deserialize_with = "texte_ou_nombre")]
        state: String,
    }
    let r: Reponse = lire(compte, &appeler(http, compte, "orders/info", json!({"reference": reference}))?)?;
    let code = r
        .state
        .trim()
        .parse()
        .map_err(|_| Erreur::Reponse(compte.cle.masquer(&format!("state n'est pas un entier : {}", r.state))))?;
    Ok(Etat::du_code(code))
}

/// Annule la commande `reference` (`orders/cancel`). Un 409 dit que la
/// production a commencé.
pub fn annuler(http: &dyn Http, compte: &Compte, reference: &str) -> Result<(), Erreur> {
    appeler(http, compte, "orders/cancel", json!({"reference": reference})).map(|_| ())
}

// ---- K10, les causes d'annulation --------------------------------------------

/// Une cause d'annulation côté imprimeur, et la règle de prévol qui y répond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CauseAnnulation {
    /// Le code tel que le signal `ItemCanceled` le porte.
    pub code: &'static str,
    /// La règle du prévol (`Defaut::regle`), `None` quand aucune ne mesure
    /// cette cause.
    pub regle: Option<&'static str>,
    /// Pourquoi cette règle, ou pourquoi aucune.
    pub pourquoi: &'static str,
}

/// La liste de ce que le prévol doit couvrir. Aucune règle n'y a été ajoutée
/// pour la remplir : une cause sans règle le dit.
pub const CAUSES_D_ANNULATION: [CauseAnnulation; 6] = [
    CauseAnnulation {
        code: "CancelCoverSpine",
        regle: Some("fichier_couverture"),
        pourquoi: "la feuille de couverture posée doit mesurer la largeur que le dos de ce \
                   compte de pages lui donne ; un dos faux change la largeur de la feuille, \
                   et la règle la confronte au profil",
    },
    CauseAnnulation {
        code: "CancelBookSize",
        regle: Some("fichier_interieur"),
        pourquoi: "chaque page de l'intérieur posé est confrontée, page par page, au format \
                   attendu fond perdu compris",
    },
    CauseAnnulation {
        code: "CancelPageCount",
        regle: Some("pagination"),
        pourquoi: "le compte déclaré à la commande est borné (24 à 800) et pair chez \
                   Cloudprinter ; fichier_interieur confronte en plus le fichier à ce compte",
    },
    CauseAnnulation {
        code: "CancelUnknownFont",
        regle: Some("conformite"),
        pourquoi: "l'émetteur embarque toujours la face entière, et la règle bloque un rendu \
                   qui ne se déclarerait pas PDF/X-4, dont les polices embarquées sont une \
                   condition ; aucune règle n'ouvre le fichier pour compter ses polices",
    },
    CauseAnnulation {
        code: "CancelInkjetCoverage",
        regle: None,
        pourquoi: "aucune règle ne mesure l'encre déposée : couverture_vide lit l'album et non \
                   le raster, et la sonde d'encre vit dans scripts/, hors du prévol",
    },
    CauseAnnulation {
        code: "CancelDownloadError",
        regle: None,
        pourquoi: "le téléchargement a lieu après le prévol, entre le dépôt et Cloudprinter : \
                   ce qui y répond est le md5sum de la commande et l'URL de lecture de sept \
                   jours, pas une règle",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const CLE: &str = "cle-de-test-0123456789abcdef";

    /// Un faux CloudCore : rejoue une réponse écrite à la main d'après la
    /// note API du 26/09 et garde ce qu'on lui a envoyé.
    struct Faux {
        reponse: (u16, String),
        recu: RefCell<Vec<(String, Value)>>,
    }

    impl Faux {
        fn new(statut: u16, corps: &str) -> Faux {
            Faux { reponse: (statut, corps.to_string()), recu: RefCell::new(Vec::new()) }
        }

        fn dernier(&self) -> (String, Value) {
            self.recu.borrow().last().cloned().expect("une requête")
        }
    }

    impl Http for Faux {
        fn post_json(&self, url: &str, corps: &Value) -> Result<(u16, String), String> {
            self.recu.borrow_mut().push((url.to_string(), corps.clone()));
            Ok(self.reponse.clone())
        }
    }

    /// Un transport qui tombe, et dont le message recopie la clé.
    struct Coupe;
    impl Http for Coupe {
        fn post_json(&self, _: &str, corps: &Value) -> Result<(u16, String), String> {
            Err(format!("connexion refusée en envoyant {corps}"))
        }
    }

    fn compte() -> Compte {
        Compte { cle: Cle::new(CLE), mode: Mode::Sandbox }
    }

    fn livre() -> Livre {
        Livre::pour(PrinterProfile::par_id("cloudprinter").unwrap(), 96).unwrap()
    }

    /// La réponse du sandbox du 26/09, réécrite, plus une seconde offre à un
    /// niveau qu'aucune doc ne nomme.
    const DEVIS: &str = r#"{
        "subtotals": {"currency": "EUR", "items": "11.8333", "fee": "0.0000", "app_fee": "0.0000"},
        "price": "11.8333", "vat": "2.3667", "vat_rate": 20, "currency": "EUR",
        "expire_date": "2026-09-28T21:00:00.000000Z",
        "shipments": [{
            "total_weight": "627",
            "items": [{"reference": "devis"}],
            "quotes": [
                {"quote": "a1b2", "service": "Limited", "shipping_level": "cp_limited",
                 "shipping_option": "Chronopost - France", "price": "5.6320", "vat": "1.1264",
                 "currency": "EUR"},
                {"quote": "c3d4", "shipping_level": "cp_jamais_vu", "price": 9.5, "vat": 1.9,
                 "currency": "EUR"}
            ]
        }],
        "invoice_currency": "EUR", "production_sla_days": 4
    }"#;

    fn commande_exemple() -> Commande {
        Commande {
            reference: "colophon-2026-10-03-a".into(),
            adresse: Adresse {
                prenom: "Alexis".into(),
                nom: "Morain".into(),
                rue: "1 rue de l'Essai".into(),
                rue2: None,
                code_postal: "75001".into(),
                ville: "Paris".into(),
                pays: "FR".into(),
                email: "alexis@example.org".into(),
                telephone: "+33100000000".into(),
            },
            livre: livre(),
            quote: "a1b2".into(),
            couverture: FichierCommande {
                url: "https://depot.example/colophon/colophon-2026-10-03-a/album-cover.pdf?X-Amz-Signature=1".into(),
                md5: "0123456789abcdef0123456789abcdef".into(),
            },
            interieur: FichierCommande {
                url: "https://depot.example/colophon/colophon-2026-10-03-a/album-print.pdf?X-Amz-Signature=2".into(),
                md5: "fedcba9876543210fedcba9876543210".into(),
            },
        }
    }

    #[test]
    fn le_livre_cloudprinter_porte_ses_trois_codes_et_ses_pages() {
        let l = livre();
        assert_eq!(l.produit, PRODUIT);
        let opts: Vec<(&str, u32)> = l.options.iter().map(|o| (o.option.as_str(), o.compte)).collect();
        assert_eq!(
            opts,
            [("pageblock_150mcs", 96), ("cover_130mcg", 1), ("cover_finish_matte", 1), ("total_pages", 96)]
        );
        assert!(Livre::pour(PrinterProfile::par_id("prodigi").unwrap(), 96).is_none());
    }

    #[test]
    fn le_devis_parle_la_langue_de_l_api() {
        let http = Faux::new(200, DEVIS);
        let d = devis(&http, &compte(), "FR", &livre()).unwrap();
        assert_eq!((d.prix.as_str(), d.tva.as_str(), d.devise.as_str()), ("11.8333", "2.3667", "EUR"));
        assert_eq!(d.expire_date, "2026-09-28T21:00:00.000000Z");
        assert_eq!(d.expeditions.len(), 1);
        assert_eq!(d.expeditions[0].poids_g.as_deref(), Some("627"));
        let offres = &d.expeditions[0].offres;
        // Les niveaux tels que rendus, y compris celui qu'aucune doc ne nomme.
        let niveaux: Vec<&str> = offres.iter().map(|o| o.niveau.as_str()).collect();
        assert_eq!(niveaux, ["cp_limited", "cp_jamais_vu"]);
        assert_eq!(offres[0].transporteur.as_deref(), Some("Chronopost - France"));
        assert_eq!((offres[0].prix.as_str(), offres[1].prix.as_str()), ("5.6320", "9.5"));

        let (url, corps) = http.dernier();
        assert_eq!(url, format!("{BASE}orders/quote"));
        assert_eq!(corps["apikey"], CLE);
        assert_eq!(corps["country"], "FR");
        assert_eq!(corps["currency"], DEVISE);
        let item = &corps["items"][0];
        assert_eq!(item["product"], PRODUIT);
        assert_eq!(item["count"], "1");
        assert_eq!(
            item["options"],
            json!([
                {"type": "pageblock_150mcs", "count": "96"},
                {"type": "cover_130mcg", "count": "1"},
                {"type": "cover_finish_matte", "count": "1"},
                {"type": "total_pages", "count": "96"}
            ])
        );
    }

    #[test]
    fn un_devis_illisible_est_une_erreur_de_reponse() {
        let http = Faux::new(200, r#"{"price": "1"}"#);
        assert!(matches!(devis(&http, &compte(), "FR", &livre()), Err(Erreur::Reponse(_))));
    }

    #[test]
    fn la_commande_pose_le_quote_et_les_deux_fichiers() {
        let http = Faux::new(201, r#"{"order": "CP-1234567"}"#);
        let c = commande_exemple();
        assert_eq!(commander(&http, &compte(), &c).unwrap(), "CP-1234567");

        let (url, corps) = http.dernier();
        assert_eq!(url, format!("{BASE}orders/add"));
        assert_eq!(corps["apikey"], CLE);
        assert_eq!(corps["reference"], "colophon-2026-10-03-a");
        assert_eq!(corps["email"], "alexis@example.org");
        let a = &corps["addresses"][0];
        assert_eq!(a["type"], "delivery");
        assert_eq!(
            (&a["firstname"], &a["lastname"], &a["street1"], &a["zip"], &a["city"], &a["country"], &a["phone"]),
            (
                &json!("Alexis"),
                &json!("Morain"),
                &json!("1 rue de l'Essai"),
                &json!("75001"),
                &json!("Paris"),
                &json!("FR"),
                &json!("+33100000000")
            )
        );
        assert!(a.get("street2").is_none(), "pas de seconde ligne vide : {a}");
        let item = &corps["items"][0];
        assert_eq!(item["product"], PRODUIT);
        assert_eq!(item["quote"], "a1b2");
        assert!(item.get("shipping_level").is_none(), "le quote remplace le niveau : {item}");
        assert_eq!(item["count"], "1");
        assert_eq!(
            item["files"],
            json!([
                {"type": "cover", "url": c.couverture.url, "md5sum": c.couverture.md5},
                {"type": "book", "url": c.interieur.url, "md5sum": c.interieur.md5}
            ])
        );
        assert_eq!(item["options"][3], json!({"type": "total_pages", "count": "96"}));
    }

    #[test]
    fn un_identifiant_de_commande_numerique_se_lit_aussi() {
        let http = Faux::new(200, r#"{"order": 98765}"#);
        assert_eq!(commander(&http, &compte(), &commande_exemple()).unwrap(), "98765");
    }

    #[test]
    fn l_etat_range_le_code_et_garde_l_inconnu() {
        let table = [
            (1, Etat::Nouvelle),
            (5, Etat::Verification),
            (7, Etat::Verification),
            (10, Etat::Fichiers),
            (15, Etat::Fichiers),
            (30, Etat::Rendu),
            (45, Etat::Rendu),
            (100, Etat::Expediee),
            (500, Etat::Annulee),
            (501, Etat::SandboxTerminee),
            (0, Etat::Inconnu(0)),
            (2, Etat::Inconnu(2)),
            (8, Etat::Inconnu(8)),
            (16, Etat::Inconnu(16)),
            (29, Etat::Inconnu(29)),
            (46, Etat::Inconnu(46)),
            (99, Etat::Inconnu(99)),
            (502, Etat::Inconnu(502)),
        ];
        for (code, attendu) in table {
            assert_eq!(Etat::du_code(code), attendu, "code {code}");
        }
    }

    /// La forme relevée au sandbox le 03/10 : le numéro est dans `state`,
    /// `state_code` est un texte (`order_state_sandbox_done`). K-s2 lisait le
    /// numéro dans `state_code`, d'après la note : aucune commande réelle ne
    /// se relisait.
    #[test]
    fn l_etat_se_lit_dans_orders_info_en_nombre_comme_en_texte() {
        for corps in [
            r#"{"reference": "colophon-a", "state": 501, "state_code": "order_state_sandbox_done",
                "is_cancelable": true, "items": [{"reference": "livre", "state": 75}]}"#,
            r#"{"reference": "colophon-a", "state": "501", "state_code": "order_state_sandbox_done"}"#,
        ] {
            let http = Faux::new(200, corps);
            assert_eq!(etat(&http, &compte(), "colophon-a").unwrap(), Etat::SandboxTerminee);
            let (url, envoye) = http.dernier();
            assert_eq!(url, format!("{BASE}orders/info"));
            assert_eq!(envoye, json!({"apikey": CLE, "reference": "colophon-a"}));
        }
        let http = Faux::new(200, r#"{"state": 8, "state_code": "order_state_x"}"#);
        assert_eq!(etat(&http, &compte(), "colophon-a").unwrap(), Etat::Inconnu(8));
        let http = Faux::new(200, r#"{"state_code": "order_state_sandbox_done"}"#);
        assert!(matches!(etat(&http, &compte(), "colophon-a"), Err(Erreur::Reponse(_))));
    }

    #[test]
    fn seul_l_etat_501_dit_le_mode() {
        assert_eq!(Etat::SandboxTerminee.mode(), Some(Mode::Sandbox));
        for e in [Etat::Nouvelle, Etat::Expediee, Etat::Annulee, Etat::Inconnu(501 + 1)] {
            assert_eq!(e.mode(), None, "{e:?}");
        }
    }

    #[test]
    fn annuler_rend_rien_et_le_409_dit_que_la_production_a_commence() {
        let http = Faux::new(200, "{}");
        annuler(&http, &compte(), "colophon-a").unwrap();
        let (url, envoye) = http.dernier();
        assert_eq!(url, format!("{BASE}orders/cancel"));
        assert_eq!(envoye, json!({"apikey": CLE, "reference": "colophon-a"}));

        let e = annuler(&Faux::new(409, ""), &compte(), "colophon-a").unwrap_err();
        assert_eq!(e, Erreur::TropTard);
        assert_eq!(e.to_string(), "la production a commencé, l'annulation n'est plus possible");
    }

    #[test]
    fn les_codes_http_deviennent_des_erreurs_typees() {
        let refus = r#"{"error": {"type": "missing_required_parameter", "info": "The required parameter 'reference' is missing"}}"#;
        assert_eq!(
            etat(&Faux::new(400, refus), &compte(), "x").unwrap_err(),
            Erreur::Refusee(
                "missing_required_parameter : The required parameter 'reference' is missing".into()
            )
        );
        assert_eq!(etat(&Faux::new(403, ""), &compte(), "x").unwrap_err(), Erreur::Cle);
        assert_eq!(etat(&Faux::new(409, ""), &compte(), "x").unwrap_err(), Erreur::TropTard);
        assert_eq!(etat(&Faux::new(410, ""), &compte(), "x").unwrap_err(), Erreur::Introuvable);
        assert_eq!(etat(&Faux::new(502, "<html>"), &compte(), "x").unwrap_err(), Erreur::Statut(502));
        assert!(matches!(etat(&Coupe, &compte(), "x").unwrap_err(), Erreur::Reseau(_)));
    }

    #[test]
    fn la_cle_ne_fuit_ni_par_debug_ni_par_une_erreur() {
        let c = compte();
        assert_eq!(format!("{:?}", c.cle), "***");
        assert_eq!(c.cle.to_string(), "***");
        assert!(!format!("{c:?}").contains(CLE));
        assert!(!format!("{:?}", commande_exemple()).contains(CLE));

        // Le faux reçoit la clé, et la renvoie dans son refus : l'erreur ne la
        // recopie pas.
        let echo = format!(r#"{{"error": {{"type": "invalid", "info": "clé {CLE} inconnue"}}}}"#);
        let mut erreurs = vec![
            etat(&Faux::new(400, &echo), &c, "x").unwrap_err(),
            etat(&Coupe, &c, "x").unwrap_err(),
            devis(&Faux::new(200, &format!("pas du json {CLE}")), &c, "FR", &livre()).unwrap_err(),
        ];
        for statut in [403, 409, 410, 502] {
            erreurs.push(annuler(&Faux::new(statut, &echo), &c, "x").unwrap_err());
        }
        for e in &erreurs {
            assert!(!format!("{e:?}").contains(CLE), "{e:?}");
            assert!(!e.to_string().contains(CLE), "{e}");
        }
        let http = Faux::new(400, &echo);
        let _ = etat(&http, &c, "x");
        assert_eq!(http.dernier().1["apikey"], CLE, "la clé voyage dans le corps");
        assert!(!http.dernier().0.contains(CLE), "jamais dans l'URL");
    }

    /// La table de la décision 0, mesurée le 03/10 sur le catalogue du compte
    /// (`docs/mesures/2026-10-03-les-produits.json`) : un format n'a un
    /// produit que si largeur, hauteur, fond perdu, squeeze, overlap et wrap
    /// lus par `products/info` égalent au dixième ceux du profil.
    #[test]
    fn chaque_format_a_son_produit_ou_aucun() {
        let table: Vec<(&str, Option<&str>)> =
            crate::format::FORMATS.iter().map(|(f, ..)| (*f, produit_pour(f))).collect();
        assert_eq!(
            table,
            [
                ("carre-21", Some("photobook_cw_s210_s_fc")),
                ("carre-30", Some("photobook_cw_s300_s_fc")),
                ("portrait-a4", Some("photobook_cw_a4_p_fc")),
                ("paysage-a4", Some("photobook_cw_a4_l_fc")),
                // Aucun cartonné du compte ne fait 280 × 210.
                ("paysage-28x21", None),
                // Le 8 × 10 pouces de Cloudprinter fait 203,2 de large, le
                // format 203,0 : 0,2 mm d'écart, au-delà du dixième.
                ("portrait-20x25", None),
            ]
        );
        assert_eq!(produit_pour("carre-21"), Some(PRODUIT));
        assert_eq!(produit_pour("210 × 210 mm"), None);
        // Chaque produit retenu a ses cotes à côté, celles qui l'ont fait
        // retenir, égales au profil au dixième.
        let p = PrinterProfile::par_id("cloudprinter").unwrap();
        for (format, w, h, ..) in crate::format::FORMATS {
            let Some(produit) = produit_pour(format) else { continue };
            let c = COTES_DES_PRODUITS.iter().find(|c| c.produit == produit).expect(produit);
            let egal = |a: f64, b: f64| (a - b).abs() < 0.05;
            assert!(egal(c.largeur_mm, *w) && egal(c.hauteur_mm, *h), "{format}");
            assert!(egal(c.fond_perdu_mm, p.bleed_mm.max()), "{format}");
            assert!(egal(c.squeeze_mm, p.mors_mm), "{format}");
            assert!(egal(c.overlap_mm, p.debord_mm), "{format}");
            assert!(egal(c.wrap_mm, p.rempli_mm), "{format}");
            assert!(c.pages_min <= p.pages_min && c.pages_max >= p.pages_max, "{format}");
        }
    }

    #[test]
    fn un_livre_se_choisit_par_produit_papier_et_finition() {
        let l = Livre::choisi("photobook_cw_a4_p_fc", "pageblock_170mcg", "cover_130mcg", "cover_finish_gloss", 48, 3);
        assert_eq!(l.produit, "photobook_cw_a4_p_fc");
        assert_eq!(l.exemplaires, 3);
        let opts: Vec<(&str, u32)> = l.options.iter().map(|o| (o.option.as_str(), o.compte)).collect();
        assert_eq!(
            opts,
            [("pageblock_170mcg", 48), ("cover_130mcg", 1), ("cover_finish_gloss", 1), ("total_pages", 48)]
        );
        // Le livre du profil reste celui de K-s2, à un exemplaire.
        assert_eq!(livre().exemplaires, 1);
    }

    /// Mesuré au sandbox le 03/10 : un devis demandé pour l'article
    /// « devis » puis posé sur une commande dont l'article s'appelle
    /// autrement est refusé (`quote_validate_one_or_more_items_not_found_in_quote`).
    /// Le hash du devis tient l'article par sa référence.
    #[test]
    fn le_devis_et_la_commande_nomment_l_article_pareil() {
        let http = Faux::new(200, DEVIS);
        devis(&http, &compte(), "FR", &livre()).unwrap();
        let au_devis = http.dernier().1["items"][0]["reference"].clone();
        let http = Faux::new(201, r#"{"order": "CP-1"}"#);
        commander(&http, &compte(), &commande_exemple()).unwrap();
        assert_eq!(http.dernier().1["items"][0]["reference"], au_devis);
        assert_eq!(au_devis, ARTICLE);
    }

    #[test]
    fn le_nombre_d_exemplaires_voyage_au_devis_et_a_la_commande() {
        let mut l = livre();
        l.exemplaires = 4;
        let http = Faux::new(200, DEVIS);
        devis(&http, &compte(), "FR", &l).unwrap();
        assert_eq!(http.dernier().1["items"][0]["count"], "4");
        let http = Faux::new(201, r#"{"order": "CP-1"}"#);
        let mut c = commande_exemple();
        c.livre = l;
        commander(&http, &compte(), &c).unwrap();
        assert_eq!(http.dernier().1["items"][0]["count"], "4");
    }

    #[test]
    fn le_catalogue_se_lit_sans_rien_supposer() {
        let http = Faux::new(
            200,
            r#"[{"category": "Photobook", "name": "Photobook CW S210 mm S FC TNR",
                 "reference": "photobook_cw_s210_s_fc", "availability": "Regional"},
                {"name": "Business card", "reference": "businesscard_ss"}]"#,
        );
        let p = produits(&http, &compte()).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].reference, "photobook_cw_s210_s_fc");
        let (url, corps) = http.dernier();
        assert_eq!(url, format!("{BASE}products"));
        assert_eq!(corps, json!({"apikey": CLE}));
    }

    /// Une réponse de `products/info` réduite à ce que l'écran lit, dans la
    /// forme relevée le 03/10.
    const INFO: &str = r#"{
        "name": "Photobook CW S210 mm S FC TNR", "reference": "photobook_cw_s210_s_fc",
        "options": [
            {"reference": "pageblock_200mcg", "note": "Pageblock paper 200gsm Machine Coated Gloss",
             "type": "type_main_paper", "type_name": "Main paper", "default": 1},
            {"reference": "pageblock_150mcs", "note": "Pageblock paper 150gsm Machine Coated Silk",
             "type": "type_main_paper", "type_name": "Main paper", "default": 0},
            {"reference": "cover_130mcg", "note": "Cover paper 130gsm Machine Coated Gloss",
             "type": "type_cover_paper", "default": 1},
            {"reference": "cover_finish_gloss", "note": "Cover lamination Gloss finish",
             "type": "type_book_cover_finish", "default": 1},
            {"reference": "cover_finish_matte", "note": "Cover lamination Matte finish",
             "type": "type_book_cover_finish", "default": 0},
            {"reference": "total_pages", "type": "type_total_pages", "default": 0}
        ],
        "specs": [
            {"reference": "exact_width_in_mm", "value": "210.0"},
            {"reference": "cover_wrap_in_mm", "value": "18"}
        ]
    }"#;

    #[test]
    fn les_options_d_un_produit_se_rangent_par_type() {
        let http = Faux::new(200, INFO);
        let i = infos_produit(&http, &compte(), "photobook_cw_s210_s_fc").unwrap();
        assert_eq!(http.dernier().1["reference"], "photobook_cw_s210_s_fc");
        let refs = |t: &str| -> Vec<(String, bool)> {
            i.options_de(t).map(|o| (o.reference.clone(), o.defaut)).collect()
        };
        assert_eq!(
            refs(TYPE_PAPIER),
            [("pageblock_200mcg".to_string(), true), ("pageblock_150mcs".to_string(), false)]
        );
        assert_eq!(refs(TYPE_COUVERTURE), [("cover_130mcg".to_string(), true)]);
        assert_eq!(refs(TYPE_FINITION).len(), 2);
        assert_eq!(i.spec("exact_width_in_mm"), Some("210.0"));
        assert_eq!(i.spec("absente"), None);
    }

    #[test]
    fn les_pays_viennent_de_shipping_countries() {
        let http = Faux::new(
            200,
            r#"[{"country_reference": "FR", "note": "France", "require_state": 0},
                {"country_reference": "US", "note": "United States", "require_state": 1}]"#,
        );
        let p = pays(&http, &compte()).unwrap();
        assert_eq!(http.dernier().0, format!("{BASE}shipping/countries"));
        assert_eq!(p[0], Pays { code: "FR".into(), nom: "France".into(), etat_requis: false });
        assert!(p[1].etat_requis);
    }

    /// Retire d'une réponse tout ce qui ne doit pas entrer dans une mesure :
    /// les URL (une URL présignée porte l'identifiant du dépôt), les champs
    /// d'adresse, et toute occurrence d'un des secrets.
    fn expurger(v: &Value, secrets: &[&str]) -> Value {
        const PERSONNEL: [&str; 10] = [
            "addresses", "email", "phone", "firstname", "lastname", "street1", "street2", "zip",
            "city", "name",
        ];
        match v {
            Value::String(s) if s.starts_with("http") => Value::String("<url>".into()),
            Value::String(s) => {
                let mut t = s.clone();
                for x in secrets.iter().filter(|x| !x.is_empty()) {
                    t = t.replace(x, "***");
                }
                Value::String(t)
            }
            Value::Array(a) => Value::Array(a.iter().map(|x| expurger(x, secrets)).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .filter(|(k, _)| !PERSONNEL.contains(&k.as_str()))
                    .map(|(k, x)| (k.clone(), expurger(x, secrets)))
                    .collect(),
            ),
            autre => autre.clone(),
        }
    }

    /// Le bout en bout, au sandbox seulement (décision 8). Lit
    /// `CLOUDPRINTER_SANDBOX_API_KEY` et les quatre `R2_*` de
    /// l'environnement, n'en affiche aucun, dépose les deux PDF de
    /// `.albums/papier` (lecture seule), demande un devis France, commande au
    /// sandbox, relit l'état jusqu'à 501 ou dix minutes, puis retire les
    /// objets quoi qu'il arrive. Ce qu'il voit va dans `COLOPHON_MESURE`.
    ///
    /// ```text
    /// (set -a; . <.env>; set +a; COLOPHON_MESURE=docs/mesures/2026-10-03-la-commande-sandbox.json \
    ///   cargo test -p colophon-core --lib banc_commande_sandbox -- --ignored --nocapture)
    /// ```
    #[test]
    #[ignore]
    fn banc_commande_sandbox() {
        use crate::depot::{Acces, Depot, Reglages};
        use crate::export::{LIVRAISON_COUVERTURE, LIVRAISON_INTERIEUR};
        let env = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("{n} absente de l'environnement"));
        let (cle, compte_r2, seau, acces, secret) = (
            env("CLOUDPRINTER_SANDBOX_API_KEY"),
            env("R2_ACCOUNT_ID"),
            env("R2_BUCKET"),
            env("R2_ACCESS_KEY_ID"),
            env("R2_SECRET_ACCESS_KEY"),
        );
        let secrets = [cle.as_str(), compte_r2.as_str(), acces.as_str(), secret.as_str()];
        let compte = Compte { cle: Cle::new(cle.clone()), mode: Mode::Sandbox };
        let depot = Depot::new(Reglages {
            endpoint: format!("https://{compte_r2}.r2.cloudflarestorage.com"),
            region: "auto".into(),
            bucket: seau.clone(),
            access_key: Acces::new(acces.clone()),
            secret_key: Acces::new(secret.clone()),
        })
        .unwrap();
        let papier = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.albums/papier");
        let profil = PrinterProfile::par_id("cloudprinter").unwrap();
        let pages = crate::prevol::prevol(&papier, profil).unwrap().fiche.pages_fichier as u32;
        let livre = Livre::pour(profil, pages).unwrap();
        let debut = std::time::Instant::now();
        let reference = format!("colophon-banc-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
        let http = Ureq::default();

        let mut deposes = Vec::new();
        let mut mesure = json!({
            "date_utc": chrono::Utc::now().format("%Y-%m-%d").to_string(),
            "mode": "Sandbox",
            "reference": reference,
            "produit": livre.produit,
            "pages": pages,
            "fichiers": {},
        });
        let resultat = (|| -> Result<(), String> {
            let mut fichiers = Vec::new();
            for nom in [LIVRAISON_INTERIEUR, LIVRAISON_COUVERTURE] {
                let chemin = papier.join(nom);
                let t = std::time::Instant::now();
                let objet = depot.deposer(&reference, &chemin).map_err(|e| format!("{e:#}"))?;
                let octets = std::fs::metadata(&chemin).unwrap().len();
                let f = FichierCommande::depuis(depot.url_de_lecture(&objet), &chemin).unwrap();
                mesure["fichiers"][nom] = json!({
                    "objet": objet.cle, "octets": octets, "md5": f.md5,
                    "depot_s": (t.elapsed().as_secs_f64() * 10.0).round() / 10.0,
                });
                deposes.push(objet);
                fichiers.push(f);
            }
            let d = devis(&http, &compte, "FR", &livre).map_err(|e| e.to_string())?;
            let offre = d.expeditions[0].offres[0].clone();
            mesure["devis"] = json!({
                "articles_ht": d.prix, "tva": d.tva, "devise": d.devise, "expire_date": d.expire_date,
                "offre": {"niveau": offre.niveau, "transporteur": offre.transporteur, "prix_ht": offre.prix},
            });
            let interieur = fichiers.remove(0);
            let couverture = fichiers.remove(0);
            let c = Commande {
                reference: reference.clone(),
                adresse: Adresse {
                    prenom: "Banc".into(),
                    nom: "Colophon".into(),
                    rue: "1 rue de l'Essai".into(),
                    rue2: None,
                    code_postal: "75001".into(),
                    ville: "Paris".into(),
                    pays: "FR".into(),
                    email: "banc@example.org".into(),
                    telephone: "+33100000000".into(),
                },
                livre: livre.clone(),
                quote: offre.quote,
                couverture,
                interieur,
            };
            let id = commander(&http, &compte, &c).map_err(|e| e.to_string())?;
            mesure["commande"] = json!({"identifiant_rendu": !id.is_empty()});
            let mut etats: Vec<Value> = Vec::new();
            let mut dernier = None;
            // Dix minutes, la borne de la décision 8 ; `COLOPHON_BANC_LIMITE_S`
            // la raccourcit quand le banc doit tenir dans une fenêtre plus courte.
            let limite = std::time::Duration::from_secs(
                std::env::var("COLOPHON_BANC_LIMITE_S").ok().and_then(|s| s.parse().ok()).unwrap_or(600),
            );
            let t0 = std::time::Instant::now();
            loop {
                let (statut, corps) = http
                    .post_json(&format!("{BASE}orders/info"), &json!({"apikey": cle, "reference": reference}))
                    .map_err(|e| compte.cle.masquer(&e))?;
                let lu: Value = serde_json::from_str(&corps).unwrap_or(Value::Null);
                let code = lu["state"].as_i64().or_else(|| lu["state"].as_str()?.parse().ok());
                if code != dernier {
                    etats.push(json!({
                        "apres_s": t0.elapsed().as_secs(),
                        "statut_http": statut,
                        "state": code,
                        "state_code": lu["state_code"],
                        "etat": format!("{:?}", code.map(Etat::du_code)),
                    }));
                    mesure["orders_info_au_passage"][code.map_or("aucun".to_string(), |c| c.to_string())] =
                        expurger(&lu, &secrets);
                    dernier = code;
                }
                if code == Some(501) || code == Some(500) || t0.elapsed() > limite {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
            assert_eq!(etat(&http, &compte, &reference).ok(), dernier.map(Etat::du_code));
            mesure["etats"] = Value::Array(etats);
            let (_, log) = http
                .post_json(&format!("{BASE}orders/log"), &json!({"apikey": cle, "reference": reference}))
                .map_err(|e| compte.cle.masquer(&e))?;
            let log: Value = serde_json::from_str(&log).unwrap_or(Value::Null);
            // Combien de temps l'étape des fichiers (10 à 15) a duré avant le
            // rendu (30 et plus) : Cloudprinter y tire les deux URL.
            let quand = |pred: &dyn Fn(i64) -> bool| {
                log.as_array()?.iter().find(|l| l["state"].as_i64().is_some_and(pred))?["create_date"]
                    .as_str()
                    .and_then(|d| chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M:%S").ok())
            };
            if let (Some(a), Some(b)) = (quand(&|e| (10..=15).contains(&e)), quand(&|e| e >= 30)) {
                mesure["etape_fichiers_s"] = json!((b - a).num_seconds());
            }
            mesure["orders_log"] = expurger(&log, &secrets);
            Ok(())
        })();
        // Les objets quittent le dépôt quoi qu'il soit arrivé.
        let mut retires = Vec::new();
        for o in &deposes {
            retires.push(json!({"objet": o.cle, "retire": depot.retirer(o).is_ok()}));
        }
        mesure["objets_retires"] = Value::Array(retires);
        mesure["duree_s"] = json!(debut.elapsed().as_secs());
        mesure["resultat"] = json!(resultat.as_ref().map(|_| "ok".to_string()).unwrap_or_else(|e| e.clone()));
        let mut texte = serde_json::to_string_pretty(&mesure).unwrap();
        for s in secrets.iter().filter(|s| !s.is_empty()) {
            texte = texte.replace(s, "***");
        }
        let sortie = std::env::var("COLOPHON_MESURE").unwrap_or_else(|_| "/dev/null".into());
        std::fs::write(&sortie, texte + "\n").unwrap();
        resultat.unwrap();
    }

    #[test]
    fn la_table_des_causes_d_annulation_est_epinglee() {
        let table: Vec<(&str, Option<&str>)> =
            CAUSES_D_ANNULATION.iter().map(|c| (c.code, c.regle)).collect();
        assert_eq!(
            table,
            [
                ("CancelCoverSpine", Some("fichier_couverture")),
                ("CancelBookSize", Some("fichier_interieur")),
                ("CancelPageCount", Some("pagination")),
                ("CancelUnknownFont", Some("conformite")),
                ("CancelInkjetCoverage", None),
                ("CancelDownloadError", None),
            ]
        );
        // Chaque règle nommée existe au prévol, sous ce nom-là.
        let prevol = include_str!("prevol.rs");
        for c in &CAUSES_D_ANNULATION {
            assert!(!c.pourquoi.is_empty(), "{} dit pourquoi", c.code);
            if let Some(r) = c.regle {
                assert!(prevol.contains(&format!("regle: \"{r}\"")), "{r} n'est pas une règle du prévol");
            }
        }
    }
}
