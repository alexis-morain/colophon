//! Les ornements typographiques : un pack compilé, et le sous-ensemble SVG
//! qu'il a le droit de parler.
//!
//! Un ornement est un fleuron qui ferme un chapitre, un filet qui sépare deux
//! blocs. **Ce n'est pas un clipart** : `CONTRIBUTING.md` refuse toujours les
//! stickers, les masques, les fonds décoratifs et les cadres fantaisie, et le
//! mot lui-même a quitté le projet.
//!
//! **Ce module ne dessine rien.** Il rend un [`Dessin`] normalisé — des
//! coordonnées absolues, des droites et des cubiques, rien d'autre — et
//! laisse chaque rendu le poser : `pdf.rs` en opérateurs de flux, l'écran en
//! `<path>`. C'est la même division que pour un bloc de texte, où la scène
//! rend des lignes et personne ici ne sait ce qu'est une police.
//!
//! **Le pack est de la donnée, pas du code** : contribuer un ornement, c'est
//! une entrée dans `assets/ornements/pack.toml` et un `.svg` à côté. En
//! retour le dépôt exige une licence de la liste blanche, un auteur, une
//! source, et un fichier qui tient dans le sous-ensemble ci-dessous. Trois
//! tests refusent le reste, et ils refusent **à la fabrication** : un lecteur
//! d'album ne dépose pas de SVG, donc un message d'erreur à l'écran serait un
//! message que personne ne devrait lire.
//!
//! ## Le sous-ensemble, et pourquoi il est si petit
//!
//! Un SVG arbitraire est un langage entier ; un flux de contenu PDF est une
//! machine à empiler des segments et des cubiques. Le sous-ensemble retenu est
//! celui où la traduction est **littérale, commande par commande** :
//! `<svg viewBox>`, `<path d>`, `M m L l H h V v C c Z z`, `fill-rule`.
//! Mesuré le 05/09 sur 32 747 commandes de chemin de la catégorie source :
//! quatre étaient des arcs. **La géométrie est déjà dans le sous-ensemble ;
//! c'est l'emballage qui n'y est pas** — un `transform`, un `style`, un
//! `<circle>`. Un contributeur normalise dans Inkscape avant de déposer.

use std::fmt;
use std::sync::OnceLock;

/// Le manifeste et les trois actifs, compilés dans le binaire comme l'OFL,
/// l'ICC sRGB et GeoNames le sont déjà.
const MANIFESTE: &str = include_str!("../assets/ornements/pack.toml");
const SVG: [(&str, &str); 3] = [
    ("filet-flare.svg", include_str!("../assets/ornements/filet-flare.svg")),
    ("filet-fleche.svg", include_str!("../assets/ornements/filet-fleche.svg")),
    ("filet-losange.svg", include_str!("../assets/ornements/filet-losange.svg")),
];

/// Les licences qu'un ornement a le droit de porter, et il n'y en aura pas de
/// troisième sans que quelqu'un rouvre la question dans une note.
///
/// Une CC BY-SA contaminerait : elle poserait la question de savoir si le PDF
/// de quelqu'un devient une œuvre dérivée à repartager, et cette question n'a
/// pas sa place dans un logiciel dont la promesse est « export PDF gratuit,
/// hors ligne, sans compte, quoi qu'il arrive ». Une CC BY collerait une
/// attribution qui voyagerait avec le livre de quelqu'un d'autre.
pub const LICENCES: [&str; 2] = ["CC0-1.0", "PD"];

/// Où un ornement se range. Trois, et pas de quatrième : les cartouches
/// n'ont aucun gisement libre sur Commons, donc la famille n'existe pas tant
/// que personne n'a trouvé ou dessiné de quoi la remplir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Famille {
    Fleuron,
    Filet,
    Separateur,
}

impl Famille {
    fn lire(s: &str) -> Option<Self> {
        match s {
            "fleuron" => Some(Famille::Fleuron),
            "filet" => Some(Famille::Filet),
            "separateur" => Some(Famille::Separateur),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Famille::Fleuron => "fleuron",
            Famille::Filet => "filet",
            Famille::Separateur => "separateur",
        }
    }
}

/// Un ornement du pack : son identité, sa provenance, son dessin.
///
/// L'objet posé sur une planche n'en garde que `pack` et `id` — jamais le
/// dessin, jamais une couleur, jamais une échelle. La boîte porte déjà la
/// taille et l'angle, et un champ de plus serait une seconde source de vérité
/// pour une géométrie qu'`Objet` détient.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Ornement {
    pub id: String,
    pub famille: Famille,
    pub titre_fr: String,
    pub titre_en: String,
    /// Identifiant SPDX, ou `PD`. Toujours dans [`LICENCES`].
    pub licence: String,
    pub auteur: String,
    /// L'URL de la page d'origine, pas celle du fichier : c'est la page qui
    /// porte la preuve de licence.
    pub source: String,
    pub dessin: Dessin,
}

/// Un dessin normalisé : coordonnées absolues, droites et cubiques.
///
/// Tout ce qui pouvait être réduit l'a été à la lecture — un `h` devient une
/// ligne, un `c` relatif devient une cubique absolue —, si bien que les deux
/// rendus reçoivent exactement la même géométrie et qu'aucun des deux n'a de
/// SVG à interpréter. `viewbox` est `[min_x, min_y, largeur, hauteur]`, dans
/// le repère du SVG, **y vers le bas**.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Dessin {
    pub viewbox: [f64; 4],
    pub chemins: Vec<Chemin>,
}

impl Dessin {
    /// Le rapport largeur/hauteur du `viewBox`, que la boîte d'un ornement
    /// garde : le redimensionnement est proportionnel, jamais libre.
    ///
    /// La conséquence est recherchée. La boîte **est** l'encre, donc « le
    /// rectangle d'un objet libre est sa boîte » reste vrai sans exception, et
    /// le pli comme la coupe mesurent exactement ce qui s'imprime. Un fleuron
    /// étiré est laid ; un fleuron dont la boîte ment au prévol est un livre
    /// gâché.
    pub fn rapport(&self) -> f64 {
        self.viewbox[2] / self.viewbox[3]
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Chemin {
    pub segments: Vec<Segment>,
    /// `fill-rule="evenodd"`. Le PDF a l'opérateur qu'il faut (`f*`), donc
    /// c'est le seul attribut de style que le sous-ensemble garde : sans lui
    /// un fleuron ajouré se remplirait plein.
    pub evenodd: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Segment {
    /// `m` en PDF.
    Vers { x: f64, y: f64 },
    /// `l` en PDF.
    Ligne { x: f64, y: f64 },
    /// `c` en PDF : deux contrôles et un point.
    Courbe { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    /// `h` en PDF.
    Ferme,
}

/// Pourquoi un fichier n'entre pas dans le pack.
///
/// **Chaque cas nomme ce qu'il a vu**, parce que le destinataire est un
/// contributeur qui doit savoir quoi normaliser, pas un lecteur d'album à qui
/// on annonce un échec.
#[derive(Debug, Clone, PartialEq)]
pub enum Refus {
    /// Un `<svg>` sans `viewBox` : sans lui, rien ne dit à quelle échelle le
    /// dessin est fait, et le poser dans une boîte serait deviner.
    PasDeViewBox,
    ViewBoxIllisible(String),
    /// Une balise hors du sous-ensemble : `<circle>`, `<g>`, `<text>`…
    Balise(String),
    /// `transform` ou `style` : ce qui se replierait dans la géométrie si
    /// quelqu'un le repliait, et qui reste sinon une seconde façon de bouger
    /// un point.
    Attribut { balise: String, attribut: String },
    /// Une commande de chemin hors du sous-ensemble : un arc, une cubique
    /// lisse, une quadratique.
    Commande(char),
    Nombre(String),
    /// Un `d` qui commence par autre chose qu'un `M` ou un `m`.
    SansDepart,
    /// Un nombre suit un `Z` sans commande entre les deux. SVG ne donne aucun
    /// paramètre à `Z` : c'est un fichier mal formé, pas une forme rare.
    ApresFermeture,
    /// Un `<svg>` qui ne porte aucun `<path>`.
    AucunChemin,
    /// Un `<path>` sans `d`, ou dont le `d` ne pose aucun segment.
    CheminVide,
}

impl fmt::Display for Refus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refus::PasDeViewBox => write!(f, "l'élément <svg> n'a pas de viewBox"),
            Refus::ViewBoxIllisible(v) => write!(f, "viewBox illisible : « {v} »"),
            Refus::Balise(b) => write!(
                f,
                "balise <{b}> hors du sous-ensemble : convertir en <path> \
                 (Inkscape : objet en chemin)"
            ),
            Refus::Attribut { balise, attribut } => write!(
                f,
                "attribut {attribut} sur <{balise}> : aplatir avant de déposer"
            ),
            Refus::Commande(c) => write!(
                f,
                "commande de chemin « {c} » hors du sous-ensemble \
                 (M m L l H h V v C c Z z seulement)"
            ),
            Refus::Nombre(n) => write!(f, "nombre illisible dans un chemin : « {n} »"),
            Refus::SansDepart => write!(f, "un chemin qui ne commence pas par M ou m"),
            Refus::ApresFermeture => write!(
                f,
                "un nombre suit un Z sans commande : Z ne prend aucun paramètre"
            ),
            Refus::AucunChemin => write!(f, "aucun <path> dans le fichier"),
            Refus::CheminVide => write!(f, "un <path> qui ne dessine rien"),
        }
    }
}

/// Le pack, lu une fois pour la vie du processus.
///
/// **Il est réputé conforme, et c'est un test du dépôt qui le prouve**, pas
/// une vérification à chaud : un pack compilé qui ne passe pas le
/// sous-ensemble ne devrait pas exister. Le lecteur échoue donc bruyamment
/// plutôt que de deviner — un `expect` ici est un défaut de code du dépôt, pas
/// un état qu'un utilisateur peut atteindre.
pub fn pack() -> &'static [Ornement] {
    static PACK: OnceLock<Vec<Ornement>> = OnceLock::new();
    PACK.get_or_init(|| lire_pack(MANIFESTE, &SVG).expect("le pack livré est conforme"))
}

/// Un ornement par son identifiant, ou `None` : un `album.json` est réparable
/// à la main, donc un identifiant inconnu est un état atteignable et il ne
/// fait pas tomber un export.
pub fn par_id(id: &str) -> Option<&'static Ornement> {
    pack().iter().find(|o| o.id == id)
}

/// Le nom du seul pack livré. Un objet porte `pack` **et** `id` pour qu'un
/// second pack puisse exister un jour sans que les identifiants aient à être
/// uniques entre eux.
pub const PACK_INTERNE: &str = "colophon";

/// Ce qui manque à une entrée de manifeste pour être un ornement.
#[derive(Debug, Clone, PartialEq)]
pub enum RefusPack {
    Champ { id: String, champ: &'static str },
    Licence { id: String, licence: String },
    Famille { id: String, famille: String },
    Fichier { id: String, fichier: String },
    Svg { fichier: String, refus: Refus },
    Doublon(String),
    Manifeste(String),
}

impl fmt::Display for RefusPack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusPack::Champ { id, champ } => {
                write!(f, "l'ornement « {id} » n'a pas de {champ}")
            }
            RefusPack::Licence { id, licence } => write!(
                f,
                "l'ornement « {id} » porte la licence « {licence} », hors de {LICENCES:?}"
            ),
            RefusPack::Famille { id, famille } => write!(
                f,
                "l'ornement « {id} » se dit de la famille « {famille} » \
                 (fleuron, filet ou separateur)"
            ),
            RefusPack::Fichier { id, fichier } => {
                write!(f, "l'ornement « {id} » nomme « {fichier} », qui n'est pas compilé")
            }
            RefusPack::Svg { fichier, refus } => write!(f, "{fichier} : {refus}"),
            RefusPack::Doublon(id) => write!(f, "deux ornements portent l'identifiant « {id} »"),
            RefusPack::Manifeste(l) => write!(f, "ligne de manifeste illisible : « {l} »"),
        }
    }
}

/// Lire un manifeste et les fichiers qu'il nomme.
///
/// Séparé de [`pack`] pour que les tests de refus puissent lui donner un
/// manifeste tordu sans toucher au pack livré.
pub fn lire_pack(manifeste: &str, fichiers: &[(&str, &str)]) -> Result<Vec<Ornement>, RefusPack> {
    let entrees = toml_ornements(manifeste)?;
    let mut ornements = Vec::with_capacity(entrees.len());
    for e in entrees {
        let champ = |c: &'static str| -> Result<String, RefusPack> {
            e.get(c)
                .filter(|v| !v.is_empty())
                .cloned()
                .ok_or_else(|| RefusPack::Champ { id: e.get("id").cloned().unwrap_or_default(), champ: c })
        };
        let id = champ("id")?;
        let fichier = champ("fichier")?;
        let licence = champ("licence")?;
        let auteur = champ("auteur")?;
        let source = champ("source")?;
        let famille_brute = champ("famille")?;

        if !LICENCES.contains(&licence.as_str()) {
            return Err(RefusPack::Licence { id, licence });
        }
        let Some(famille) = Famille::lire(&famille_brute) else {
            return Err(RefusPack::Famille { id, famille: famille_brute });
        };
        if ornements.iter().any(|o: &Ornement| o.id == id) {
            return Err(RefusPack::Doublon(id));
        }
        let Some((_, svg)) = fichiers.iter().find(|(n, _)| *n == fichier) else {
            return Err(RefusPack::Fichier { id, fichier });
        };
        let dessin = lire_svg(svg).map_err(|refus| RefusPack::Svg { fichier: fichier.clone(), refus })?;

        ornements.push(Ornement {
            id,
            famille,
            titre_fr: champ("titre.fr")?,
            titre_en: champ("titre.en")?,
            licence,
            auteur,
            source,
            dessin,
        });
    }
    Ok(ornements)
}

// ---------------------------------------------------------------------------
// Le manifeste
// ---------------------------------------------------------------------------

/// Les tables `[[ornement]]` d'un manifeste, chacune en paires clef/valeur.
///
/// **C'est du TOML lu à la main, et volontairement.** Le manifeste est de la
/// donnée du dépôt dont la grammaire nous appartient : trois formes de ligne,
/// et rien d'autre. Un crate de TOML entier ferait entrer une demi-douzaine de
/// dépendances transitives pour lire cinquante lignes que nous écrivons — le
/// même arbitrage que les liaisons Objective-C écrites à la main dans `app/`.
///
/// Le prix à payer est qu'il **refuse au lieu d'ignorer** : toute ligne qui
/// n'est ni vide, ni un commentaire, ni `[[ornement]]`, ni `clef = "valeur"`
/// est un refus nommé. Un lecteur tolérant serait celui qui laisse passer une
/// faute de frappe dans une licence.
fn toml_ornements(src: &str) -> Result<Vec<std::collections::HashMap<String, String>>, RefusPack> {
    let mut tables: Vec<std::collections::HashMap<String, String>> = Vec::new();
    for ligne in src.lines() {
        let l = ligne.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if l == "[[ornement]]" {
            tables.push(std::collections::HashMap::new());
            continue;
        }
        let Some((clef, valeur)) = l.split_once('=') else {
            return Err(RefusPack::Manifeste(l.into()));
        };
        let clef = clef.trim();
        let valeur = valeur.trim();
        // Une valeur est une chaîne entre guillemets doubles, sans échappement :
        // aucun champ de ce manifeste n'en a besoin, et accepter `\"` serait
        // accepter d'en discuter.
        let Some(valeur) = valeur.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
            return Err(RefusPack::Manifeste(l.into()));
        };
        if valeur.contains('"') || clef.is_empty() {
            return Err(RefusPack::Manifeste(l.into()));
        }
        let Some(table) = tables.last_mut() else {
            return Err(RefusPack::Manifeste(l.into()));
        };
        table.insert(clef.into(), valeur.into());
    }
    Ok(tables)
}

// ---------------------------------------------------------------------------
// Le SVG
// ---------------------------------------------------------------------------

/// Traduire un SVG du sous-ensemble en [`Dessin`].
pub fn lire_svg(src: &str) -> Result<Dessin, Refus> {
    let mut viewbox: Option<[f64; 4]> = None;
    let mut chemins = Vec::new();

    for (balise, attributs) in balises(src) {
        match balise.as_str() {
            "svg" => {
                refuse_habillage(&balise, &attributs)?;
                let v = attributs
                    .iter()
                    .find(|(k, _)| k == "viewBox")
                    .map(|(_, v)| v.clone())
                    .ok_or(Refus::PasDeViewBox)?;
                viewbox = Some(lire_viewbox(&v)?);
            }
            "path" => {
                refuse_habillage(&balise, &attributs)?;
                let d = attributs
                    .iter()
                    .find(|(k, _)| k == "d")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let evenodd = attributs
                    .iter()
                    .any(|(k, v)| k == "fill-rule" && v.trim() == "evenodd");
                let segments = lire_chemin(&d)?;
                // **Un `<path>` qui ne dessine rien se refuse**, au lieu
                // d'entrer comme un chemin vide. C'est la doctrine du module —
                // refuser plutôt qu'ignorer — et c'est aussi ce qui empêche
                // l'émetteur de poser un `f` sans le moindre tracé devant :
                // un fragment de flux dégénéré que rien n'aurait signalé.
                if segments.is_empty() {
                    return Err(Refus::CheminVide);
                }
                chemins.push(Chemin { segments, evenodd });
            }
            autre => return Err(Refus::Balise(autre.into())),
        }
    }

    let viewbox = viewbox.ok_or(Refus::PasDeViewBox)?;
    if chemins.is_empty() {
        return Err(Refus::AucunChemin);
    }
    Ok(Dessin { viewbox, chemins })
}

/// `transform` et `style` : les deux façons de bouger ou de repeindre un point
/// sans le dire dans sa commande. Refusées sur toute balise.
fn refuse_habillage(balise: &str, attributs: &[(String, String)]) -> Result<(), Refus> {
    for (k, _) in attributs {
        if k == "transform" || k == "style" {
            return Err(Refus::Attribut { balise: balise.into(), attribut: k.clone() });
        }
    }
    Ok(())
}

/// Les balises ouvrantes d'un document, avec leurs attributs.
///
/// Ce n'est pas un analyseur XML et ça n'a pas à l'être : le sous-ensemble a
/// deux balises et aucun contenu textuel. La déclaration `<?xml?>`, les
/// commentaires et le DOCTYPE se sautent ; les fermantes ne portent rien.
fn balises(src: &str) -> Vec<(String, Vec<(String, String)>)> {
    let o: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut sortie = Vec::new();
    while i < o.len() {
        if o[i] != '<' {
            i += 1;
            continue;
        }
        // `<?xml ... ?>`, `<!-- ... -->`, `<!DOCTYPE ...>`, `</svg>`.
        if matches!(o.get(i + 1), Some('?') | Some('!') | Some('/')) {
            // **Un commentaire se ferme sur `-->`, jamais sur le premier `>`
            // venu.** Un commentaire qui contient une balise en contient un au
            // milieu, et s'arrêter là faisait reprendre la lecture *à
            // l'intérieur* du commentaire : `<!-- ancien <path/> <path d="…"/>
            // -->` rendait deux chemins au lieu d'aucun, et du dessin qu'on
            // venait de désactiver revenait à l'encre. Commenter un chemin est
            // la chose la plus ordinaire du monde dans un `.svg` retouché à la
            // main ou exporté par un outil.
            if o[i..].starts_with(&['<', '!', '-', '-']) {
                i += 4;
                while i < o.len() && !o[i..].starts_with(&['-', '-', '>']) {
                    i += 1;
                }
                i = (i + 3).min(o.len());
                continue;
            }
            while i < o.len() && o[i] != '>' {
                i += 1;
            }
            i += 1;
            continue;
        }
        i += 1;
        let debut = i;
        while i < o.len() && !o[i].is_whitespace() && o[i] != '>' && o[i] != '/' {
            i += 1;
        }
        let nom: String = o[debut..i].iter().collect();
        let mut attributs = Vec::new();
        loop {
            while i < o.len() && o[i].is_whitespace() {
                i += 1;
            }
            if i >= o.len() || o[i] == '>' || o[i] == '/' {
                break;
            }
            let dk = i;
            while i < o.len() && o[i] != '=' && !o[i].is_whitespace() && o[i] != '>' {
                i += 1;
            }
            let clef: String = o[dk..i].iter().collect();
            while i < o.len() && o[i].is_whitespace() {
                i += 1;
            }
            if i < o.len() && o[i] == '=' {
                i += 1;
                while i < o.len() && o[i].is_whitespace() {
                    i += 1;
                }
                // `o.get(i)` plutôt qu'un défaut : un document qui s'arrête
                // sur son `=` n'a pas de guillemet, et lui en inventer un
                // faisait trancher une plage hors bornes — un `.svg` tronqué
                // faisait paniquer le test qui devait le nommer.
                if let Some(quote) = o.get(i).copied().filter(|q| *q == '"' || *q == '\'') {
                    i += 1;
                    let dv = i;
                    while i < o.len() && o[i] != quote {
                        i += 1;
                    }
                    attributs.push((clef, o[dv..i.min(o.len())].iter().collect()));
                    i += 1;
                    continue;
                }
            }
            attributs.push((clef, String::new()));
        }
        while i < o.len() && o[i] != '>' {
            i += 1;
        }
        i += 1;
        // `xmlns` et compagnie ne sont pas des attributs de dessin : ils
        // décrivent le document, pas un point, et les garder ferait refuser
        // tous les SVG du monde pour une raison qui n'en est pas une.
        attributs.retain(|(k, _)| !k.starts_with("xmlns") && k != "version" && k != "id");
        sortie.push((nom, attributs));
    }
    sortie
}

fn lire_viewbox(v: &str) -> Result<[f64; 4], Refus> {
    let n: Vec<f64> = v
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();
    if n.len() != 4 || n[2] <= 0.0 || n[3] <= 0.0 {
        return Err(Refus::ViewBoxIllisible(v.into()));
    }
    Ok([n[0], n[1], n[2], n[3]])
}

/// Un attribut `d` en segments absolus.
///
/// Trois choses que la traduction fait et qu'il ne faut pas défaire :
/// **le relatif devient absolu** (un rendu ne devrait jamais avoir à suivre un
/// point courant) ; **`H` et `V` deviennent des lignes** (le PDF n'a pas les
/// deux opérateurs) ; et **une paire de coordonnées qui suit un `M` est un
/// `L`**, comme le veut SVG — c'est la forme que portent deux des trois actifs
/// livrés, donc l'oublier ne se verrait pas à la compilation, seulement à
/// l'impression.
pub fn lire_chemin(d: &str) -> Result<Vec<Segment>, Refus> {
    let mut lex = Lexeur::new(d);
    let mut segments: Vec<Segment> = Vec::new();
    let (mut cx, mut cy) = (0.0f64, 0.0f64);
    let (mut dx, mut dy) = (0.0f64, 0.0f64); // début du sous-chemin courant
    let mut commande: Option<char> = None;

    loop {
        lex.espaces();
        if lex.fini() {
            break;
        }
        if let Some(c) = lex.commande() {
            if !"MmLlHhVvCcZz".contains(c) {
                return Err(Refus::Commande(c));
            }
            commande = Some(c);
        } else {
            // Une coordonnée sans commande devant : la précédente se répète,
            // sauf après un `M`, qui se répète en `L`.
            //
            // **`Z` ne se répète pas**, et le refuser n'est pas du zèle : SVG
            // ne lui donne aucun paramètre, donc `M0,0 Z5` n'a pas de sens —
            // mais le laisser répéter faisait empiler des fermetures sans
            // jamais consommer le `5`, c'est-à-dire une boucle infinie qui
            // mangeait la mémoire au lieu de nommer le fichier fautif.
            commande = match commande {
                Some('M') => Some('L'),
                Some('m') => Some('l'),
                Some('Z') | Some('z') => return Err(Refus::ApresFermeture),
                Some(c) => Some(c),
                None => return Err(Refus::SansDepart),
            };
        }
        let c = commande.expect("une commande vient d'être posée");
        let relatif = c.is_ascii_lowercase();
        let (ox, oy) = if relatif { (cx, cy) } else { (0.0, 0.0) };

        match c.to_ascii_uppercase() {
            'M' => {
                let (x, y) = (lex.nombre()? + ox, lex.nombre()? + oy);
                segments.push(Segment::Vers { x, y });
                (cx, cy) = (x, y);
                (dx, dy) = (x, y);
            }
            'L' => {
                let (x, y) = (lex.nombre()? + ox, lex.nombre()? + oy);
                segments.push(Segment::Ligne { x, y });
                (cx, cy) = (x, y);
            }
            'H' => {
                let x = lex.nombre()? + ox;
                segments.push(Segment::Ligne { x, y: cy });
                cx = x;
            }
            'V' => {
                let y = lex.nombre()? + oy;
                segments.push(Segment::Ligne { x: cx, y });
                cy = y;
            }
            'C' => {
                let x1 = lex.nombre()? + ox;
                let y1 = lex.nombre()? + oy;
                let x2 = lex.nombre()? + ox;
                let y2 = lex.nombre()? + oy;
                let x = lex.nombre()? + ox;
                let y = lex.nombre()? + oy;
                segments.push(Segment::Courbe { x1, y1, x2, y2, x, y });
                (cx, cy) = (x, y);
            }
            'Z' => {
                segments.push(Segment::Ferme);
                (cx, cy) = (dx, dy);
            }
            _ => return Err(Refus::Commande(c)),
        }
        if segments.first().is_some_and(|s| !matches!(s, Segment::Vers { .. })) {
            return Err(Refus::SansDepart);
        }
    }
    Ok(segments)
}

/// Le lecteur de nombres d'un attribut `d`.
///
/// Un `d` sépare ses nombres par des espaces, des virgules, ou par rien du
/// tout quand le signe suffit : `c0,0.931-33.845,3.722` porte six nombres et
/// deux séparateurs. C'est la raison pour laquelle ce lexeur existe au lieu
/// d'un `split_whitespace`.
struct Lexeur {
    o: Vec<char>,
    i: usize,
}

impl Lexeur {
    fn new(s: &str) -> Self {
        Lexeur { o: s.chars().collect(), i: 0 }
    }

    fn fini(&self) -> bool {
        self.i >= self.o.len()
    }

    fn espaces(&mut self) {
        while self.i < self.o.len() && (self.o[self.i].is_whitespace() || self.o[self.i] == ',') {
            self.i += 1;
        }
    }

    /// La lettre de commande sous le curseur, s'il y en a une.
    fn commande(&mut self) -> Option<char> {
        let c = *self.o.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    fn nombre(&mut self) -> Result<f64, Refus> {
        self.espaces();
        let debut = self.i;
        if matches!(self.o.get(self.i), Some('+') | Some('-')) {
            self.i += 1;
        }
        while matches!(self.o.get(self.i), Some(c) if c.is_ascii_digit() || *c == '.') {
            self.i += 1;
        }
        // Un exposant : `1e-3`. Rare dans un ornement, gratuit à accepter, et
        // le refuser transformerait un fichier valide en refus incompréhensible.
        if matches!(self.o.get(self.i), Some('e') | Some('E')) {
            let sauve = self.i;
            self.i += 1;
            if matches!(self.o.get(self.i), Some('+') | Some('-')) {
                self.i += 1;
            }
            if matches!(self.o.get(self.i), Some(c) if c.is_ascii_digit()) {
                while matches!(self.o.get(self.i), Some(c) if c.is_ascii_digit()) {
                    self.i += 1;
                }
            } else {
                self.i = sauve;
            }
        }
        let brut: String = self.o[debut..self.i].iter().collect();
        brut.parse::<f64>().map_err(|_| Refus::Nombre(brut))
    }
}

/// L'inventaire des licences du pack, tel que `assets/ornements/LICENCES.md`
/// le porte.
///
/// **Il est engendré, et sa fraîcheur est un test.** Un actif sous licence
/// garde sa licence à côté de lui — l'OFL, l'ICC sRGB et GeoNames le font déjà
/// dans le même dossier —, et un inventaire tenu à la main serait faux au
/// premier ajout. Celui-ci ne peut pas mentir : il est le manifeste, relu.
pub fn licences_md() -> String {
    let mut out = String::new();
    for l in [
        "# Ornements — provenance et licences",
        "",
        "Ce fichier est **engendré** depuis `pack.toml` par",
        "`ornement::licences_md`, et un test du dépôt rougit s'il a vieilli.",
        "Ne pas l'écrire à la main.",
        "",
        "Chaque ornement livré avec Colophon est dans le domaine public ou sous",
        "CC0. Aucune autre licence n'entre : une CC BY-SA contaminerait le PDF",
        "de la personne qui l'utilise, une CC BY y collerait une attribution qui",
        "voyagerait avec son livre.",
        "",
    ] {
        out.push_str(l);
        out.push('\n');
    }
    for o in pack() {
        out.push_str(&format!(
            "## {}\n\n- identifiant : `{}`\n- famille : {}\n- licence : {}\n- auteur : {}\n- source : <{}>\n\n",
            o.titre_fr,
            o.id,
            o.famille.code(),
            o.licence,
            o.auteur,
            o.source,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le pack livré se lit, et il se lit **entièrement** : trois actifs, leur
    /// provenance, leur dessin. C'est ce test qui rend légitime le `expect` de
    /// [`pack`] — la conformité est prouvée ici, pas devinée à chaud.
    #[test]
    fn le_pack_livre_est_conforme() {
        let p = pack();
        assert_eq!(p.len(), 3, "trois actifs de démonstration");
        for o in p {
            assert!(LICENCES.contains(&o.licence.as_str()), "{} : {}", o.id, o.licence);
            assert!(o.source.starts_with("https://commons.wikimedia.org/wiki/File:"));
            assert!(!o.auteur.is_empty());
            assert!(!o.titre_fr.is_empty() && !o.titre_en.is_empty());
            assert!(!o.dessin.chemins.is_empty());
            assert!(o.dessin.rapport() > 0.0);
        }
        assert!(par_id("filet-flare").is_some());
        assert!(par_id("celui-la-n-existe-pas").is_none());
    }

    fn manifeste(corps: &str) -> String {
        format!(
            "[[ornement]]\nid = \"x\"\nfichier = \"x.svg\"\nfamille = \"filet\"\n\
             titre.fr = \"X\"\ntitre.en = \"X\"\n{corps}"
        )
    }

    const CARRE: &str = r#"<svg viewBox="0 0 10 10"><path d="M0,0 H10 V10 H0 Z"/></svg>"#;

    /// **Cinq refus prouvés, pas une validation qui passe.** Ce sont eux qui
    /// tiennent la promesse du manifeste : une licence hors liste, un champ
    /// absent et quatre emballages que le PDF ne saurait pas traduire.
    #[test]
    fn le_pack_refuse_une_licence_hors_liste() {
        let m = manifeste("licence = \"CC-BY-SA-4.0\"\nauteur = \"A\"\nsource = \"u\"\n");
        let e = lire_pack(&m, &[("x.svg", CARRE)]).unwrap_err();
        assert!(matches!(e, RefusPack::Licence { .. }), "{e}");
        assert!(e.to_string().contains("CC-BY-SA-4.0"));
    }

    #[test]
    fn le_pack_refuse_une_entree_sans_licence_sans_auteur_ou_sans_source() {
        for absent in ["licence", "auteur", "source"] {
            let mut corps = String::new();
            for (c, v) in [("licence", "PD"), ("auteur", "A"), ("source", "u")] {
                if c != absent {
                    corps.push_str(&format!("{c} = \"{v}\"\n"));
                }
            }
            let e = lire_pack(&manifeste(&corps), &[("x.svg", CARRE)]).unwrap_err();
            assert!(
                matches!(&e, RefusPack::Champ { champ, .. } if *champ == absent),
                "sans {absent} : {e}"
            );
        }
    }

    #[test]
    fn le_svg_refuse_ce_que_le_pdf_ne_saurait_pas_traduire() {
        let cas: [(&str, fn(&Refus) -> bool); 6] = [
            (
                r#"<svg viewBox="0 0 10 10"><g transform="translate(2)"><path d="M0,0 H1 Z"/></g></svg>"#,
                |r| matches!(r, Refus::Balise(b) if b == "g"),
            ),
            (
                r#"<svg viewBox="0 0 10 10"><path transform="scale(2)" d="M0,0 H1 Z"/></svg>"#,
                |r| matches!(r, Refus::Attribut { attribut, .. } if attribut == "transform"),
            ),
            (
                r#"<svg viewBox="0 0 10 10"><path style="fill:#f00" d="M0,0 H1 Z"/></svg>"#,
                |r| matches!(r, Refus::Attribut { attribut, .. } if attribut == "style"),
            ),
            (
                r#"<svg viewBox="0 0 10 10"><path d="M0,0 A5,5 0 0 1 10,10 Z"/></svg>"#,
                |r| matches!(r, Refus::Commande('A')),
            ),
            (
                r#"<svg viewBox="0 0 10 10"><text x="0">non</text></svg>"#,
                |r| matches!(r, Refus::Balise(b) if b == "text"),
            ),
            (
                r#"<svg width="10" height="10"><path d="M0,0 H1 Z"/></svg>"#,
                |r| matches!(r, Refus::PasDeViewBox),
            ),
        ];
        for (svg, attendu) in cas {
            let r = lire_svg(svg).unwrap_err();
            assert!(attendu(&r), "{svg} a rendu {r:?}");
            // Un refus se lit : le destinataire est un contributeur qui doit
            // savoir quoi normaliser.
            assert!(!r.to_string().is_empty());
        }
        // Un `<circle>` est le cas le plus courant du gisement : trois des
        // quinze actifs candidats mesurés le 05/09 en portaient un.
        assert!(matches!(
            lire_svg(r#"<svg viewBox="0 0 10 10"><circle r="4"/></svg>"#).unwrap_err(),
            Refus::Balise(b) if b == "circle"
        ));
    }

    /// Le relatif devient absolu, `H` et `V` deviennent des lignes, et une
    /// paire qui suit un `M` est un `L`. Les trois se lisent ici sur la même
    /// figure, parce que les trois se perdraient en silence.
    #[test]
    fn un_chemin_se_normalise_en_absolu() {
        let s = lire_chemin("M 10,10 20,10 h 10 v 10 l -10,0 z").unwrap();
        assert_eq!(
            s,
            vec![
                Segment::Vers { x: 10.0, y: 10.0 },
                Segment::Ligne { x: 20.0, y: 10.0 }, // le M implicite
                Segment::Ligne { x: 30.0, y: 10.0 }, // h relatif
                Segment::Ligne { x: 30.0, y: 20.0 }, // v relatif
                Segment::Ligne { x: 20.0, y: 20.0 },
                Segment::Ferme,
            ]
        );
    }

    /// `c0,0.931-33.845,3.722` : six nombres, deux séparateurs, et le signe
    /// qui en tient lieu. C'est la forme qu'un des trois actifs livrés porte,
    /// donc un lexeur qui ne la lit pas casse le pack sans casser un test.
    #[test]
    fn les_nombres_colles_se_lisent() {
        let s = lire_chemin("M0,0c0,0.931-33.845,3.722-59.23,3.722").unwrap();
        let Segment::Courbe { x1, y1, x2, y2, x, y } = s[1] else {
            panic!("attendu une cubique, vu {:?}", s[1]);
        };
        assert_eq!((x1, y1), (0.0, 0.931));
        assert_eq!((x2, y2), (-33.845, 3.722));
        assert_eq!((x, y), (-59.23, 3.722));
    }

    /// Un `Z` ramène le point courant au départ du sous-chemin : sans ça, un
    /// `h` qui suit une fermeture part d'ailleurs, et le dessin dérive sans
    /// qu'aucune commande soit fausse.
    #[test]
    fn une_fermeture_ramene_au_depart() {
        let s = lire_chemin("M 5,5 h 10 z h 2").unwrap();
        assert_eq!(s.last(), Some(&Segment::Ligne { x: 7.0, y: 5.0 }));
    }

    /// **Deux défauts d'analyseur, trouvés en relisant, et qui ne se seraient
    /// pas vus autrement.** Ni l'un ni l'autre n'est atteignable depuis un
    /// album — le pack est compilé —, mais tous deux frappent la personne à
    /// qui ce module doit précisément un message clair : le contributeur qui
    /// dépose un `.svg` mal formé.
    ///
    /// Un fichier qui s'arrête sur son `=` faisait trancher une plage hors
    /// bornes, donc paniquer le test chargé de le nommer. Et un nombre après
    /// un `Z` faisait répéter la fermeture sans jamais consommer le nombre :
    /// une boucle infinie qui empilait des segments jusqu'à la mémoire.
    #[test]
    fn un_fichier_mal_forme_se_nomme_au_lieu_de_paniquer_ou_de_boucler() {
        for tronque in [
            r#"<svg viewBox="0 0 1 1"><path d="M0,0 Z"/><x a="#,
            r#"<svg viewBox="0 0 1 1"><path d="M0,0 Z"/><x a=""#,
            r#"<svg viewBox="#,
        ] {
            // Ce qui compte est qu'il rende, quel que soit le verdict.
            let _ = lire_svg(tronque);
        }
        assert_eq!(lire_chemin("M0,0 Z5").unwrap_err(), Refus::ApresFermeture);
        assert_eq!(lire_chemin("M0,0 z 5,5").unwrap_err(), Refus::ApresFermeture);
        // Et un `Z` suivi d'une vraie commande reste légitime : c'est la
        // forme qu'un chemin à plusieurs sous-chemins prend.
        assert_eq!(lire_chemin("M0,0 H1 Z M5,5 H6 Z").unwrap().len(), 6);
    }

    /// **Un commentaire ne fuit pas dans le dessin.**
    ///
    /// Commenter un chemin est la chose la plus ordinaire du monde dans un
    /// `.svg` retouché à la main. Sauter le commentaire jusqu'au premier `>`
    /// venu faisait reprendre la lecture à l'intérieur, et le tracé qu'on
    /// venait de désactiver revenait à l'encre — sans erreur, sans trace, et
    /// sans qu'aucun des trois actifs livrés ne le montre.
    #[test]
    fn un_chemin_commente_ne_revient_pas_a_l_encre() {
        let svg = "<svg viewBox=\"0 0 10 10\">\n\
                   <!-- ancien <path/> <path d=\"M9,9 L8,8\"/> -->\n\
                   <path d=\"M2,2 L3,3\"/>\n</svg>";
        let d = lire_svg(svg).unwrap();
        assert_eq!(d.chemins.len(), 1, "le commenté est entré : {:?}", d.chemins);
        assert_eq!(d.chemins[0].segments[0], Segment::Vers { x: 2.0, y: 2.0 });

        // Un commentaire non fermé avale la fin du fichier, ce qui est
        // exactement ce qu'il est : ce qui le suit ne compte pas, et le
        // lecteur s'arrête sans boucler ni paniquer.
        let ouvert = lire_svg(
            "<svg viewBox=\"0 0 1 1\"><path d=\"M0,0 H1 Z\"/><!-- fin <path d=\"M5,5 H6 Z\"/>",
        )
        .unwrap();
        assert_eq!(ouvert.chemins.len(), 1);
        assert_eq!(ouvert.viewbox, [0.0, 0.0, 1.0, 1.0]);
    }

    /// Un `<path>` qui ne dessine rien se refuse, plutôt que d'entrer comme un
    /// chemin vide : c'est la doctrine du module, et c'est ce qui empêche
    /// l'émetteur de poser un `f` sans le moindre tracé devant.
    #[test]
    fn un_chemin_qui_ne_dessine_rien_se_refuse() {
        for svg in [
            r#"<svg viewBox="0 0 10 10"><path/></svg>"#,
            r#"<svg viewBox="0 0 10 10"><path d=""/></svg>"#,
            r#"<svg viewBox="0 0 10 10"><path d="   "/></svg>"#,
        ] {
            assert_eq!(lire_svg(svg).unwrap_err(), Refus::CheminVide, "{svg}");
        }
    }

    #[test]
    fn un_chemin_qui_ne_commence_pas_par_un_deplacement_se_refuse() {
        assert_eq!(lire_chemin("L 10,10").unwrap_err(), Refus::SansDepart);
        assert_eq!(lire_chemin("10,10").unwrap_err(), Refus::SansDepart);
    }

    /// `fill-rule` est le seul attribut de style que le sous-ensemble garde,
    /// parce que le PDF a l'opérateur qu'il faut. Un fleuron ajouré rempli
    /// plein est une tache noire.
    #[test]
    fn la_regle_de_remplissage_voyage() {
        let d = lire_svg(
            r#"<svg viewBox="0 0 10 10"><path fill-rule="evenodd" d="M0,0 H1 Z"/></svg>"#,
        )
        .unwrap();
        assert!(d.chemins[0].evenodd);
        assert!(!lire_svg(CARRE).unwrap().chemins[0].evenodd);
    }

    /// L'inventaire des licences est à jour. C'est le même régime que la
    /// fixture de scène ou les fiches : un fichier engendré dont la
    /// fraîcheur est testée, parce qu'un inventaire de licences périmé est
    /// exactement le genre de fichier que personne ne relit.
    #[test]
    fn l_inventaire_des_licences_est_frais() {
        let sur_disque = include_str!("../assets/ornements/LICENCES.md");
        assert_eq!(
            sur_disque,
            licences_md(),
            "LICENCES.md a vieilli : le régénérer avec \
             `cargo test -p colophon-core --release banc_licences_ornements -- --ignored`"
        );
    }

    /// Réécrit `LICENCES.md`. Ignoré : c'est un outil, pas une vérification.
    #[test]
    #[ignore]
    fn banc_licences_ornements() {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/ornements/LICENCES.md");
        std::fs::write(p, licences_md()).unwrap();
        println!("{p} réécrit");
    }

    /// Le manifeste refuse au lieu d'ignorer : une ligne qu'il ne comprend
    /// pas est une faute de frappe dans une licence en puissance.
    #[test]
    fn le_manifeste_refuse_une_ligne_qu_il_ne_comprend_pas() {
        let m = "[[ornement]]\nid = filet\n";
        assert!(matches!(toml_ornements(m), Err(RefusPack::Manifeste(_))));
        let m = "id = \"orphelin\"\n";
        assert!(matches!(toml_ornements(m), Err(RefusPack::Manifeste(_))));
    }
}


