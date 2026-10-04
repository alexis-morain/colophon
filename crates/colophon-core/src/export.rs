//! Le manifeste d'un export : de quel album il sort, et pour quel imprimeur.
//!
//! Le prévol sait ouvrir les PDF posés à côté de l'album et juger leur
//! géométrie. Il lui restait un angle mort : un fichier **juste de géométrie
//! et vieux de contenu** passait. Même format, mêmes pages, les photographies
//! d'avant la dernière retouche — le rapport disait vert et la presse
//! imprimait le livre d'hier.
//!
//! Fermer ça par une date de fichier a été refusé, et c'était juste : un
//! `mtime` ne se défend pas, un fichier plus vieux qu'`album.json` n'étant pas
//! forcément faux. Ce qui se défend, c'est que le fichier **dise de quel album
//! il sort**. `--print` et `--cover` posent donc `export.json` à côté
//! d'`album.json`, et le prévol le confronte à l'album du jour.
//!
//! **Un manifeste à côté de l'album, jamais une empreinte dans le PDF.** La
//! mettre dans `/ID` ou dans le XMP la ferait voyager avec le fichier, ce qui
//! est séduisant — et changerait les octets de **tous** les PDF exportés,
//! cassant le banc d'octets, forçant une remesure PDF/A-2b et touchant la
//! garantie de reproductibilité. Le prévol ne regarde jamais que des fichiers
//! du dossier de l'album : un manifeste posé là couvre toute sa portée pour
//! une fraction du risque.
//!
//! **Un fichier sans entrée au manifeste ne dit rien.** Silence complet, pas
//! même un avertissement : autrement tout export antérieur à ce module
//! deviendrait un bloquant, à commencer par les fichiers déjà vérifiés qu'on
//! s'apprête à commander. Le manifeste n'ajoute que de la précision ; il ne
//! rend jamais un fichier suspect par son absence. C'est aussi pourquoi un
//! `export.json` illisible se lit comme un manifeste vide plutôt que comme une
//! erreur : notre propre comptabilité ne doit refuser aucun album.
//!
//! **On ne hache pas les photographies.** 95 Mo à chaque prévol serait payer
//! très cher un cas rare. Une photo réécrite **hors** de Colophon n'est donc
//! pas vue, et un `root` repointé sur un autre dossier aux mêmes noms non
//! plus : c'est une limite assumée, à côté de celle que le contrôle de
//! géométrie a déjà laissée.

use crate::model::Album;
use crate::prevol::PrevolReport;
use crate::printer::PrinterProfile;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Le nom du manifeste, à côté d'`album.json`, de `curation.json` et de
/// `thumbs.json`.
pub const NOM: &str = "export.json";

/// Les deux noms de la **livraison** : ce que `--print` et `--cover` écrivent,
/// ce que le manifeste note, ce que le prévol juge, ce que le mail à
/// l'imprimeur nomme. Écrits une fois, parce qu'une règle qui lirait un
/// fichier sous un nom et le chercherait au manifeste sous un autre se tairait
/// pour toujours sans que rien ne rougisse. Les aperçus, eux, ont leurs
/// propres noms — `album.pdf` et `album-cover.apercu.pdf` — et n'entrent
/// jamais ici.
pub const LIVRAISON_INTERIEUR: &str = "album-print.pdf";
pub const LIVRAISON_COUVERTURE: &str = "album-cover.pdf";

/// Ce qu'un rendu a écrit : le fichier, pour qui, depuis quel album.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artefact {
    /// Nom du fichier posé à côté d'`album.json`, jamais un chemin.
    pub fichier: String,
    /// L'identifiant du profil imprimeur qui a décidé de sa géométrie.
    pub profil: String,
    /// L'empreinte de l'album tel qu'il était au rendu.
    pub album: String,
    /// La taille du fichier écrit. Gratuite, et elle attrape un fichier
    /// remplacé depuis ; hacher 95 Mo à chaque prévol pour la même
    /// information ne se justifierait pas.
    pub octets: u64,
    /// L'instant du rendu, sous la même horloge que le PDF lui-même, donc
    /// figée par `SOURCE_DATE_EPOCH` comme lui. Elle se lit, elle ne se juge
    /// pas : une date de fichier ne prouve rien, c'est tout le point de ce
    /// module.
    pub date: String,
}

/// Tout ce que les exports ont posé dans ce dossier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifeste {
    #[serde(default)]
    pub artefacts: Vec<Artefact>,
}

impl Manifeste {
    /// Le manifeste du dossier, ou un manifeste vide s'il n'y en a pas — ou
    /// s'il ne se lit pas. Voir l'en-tête : absent veut dire « rien à dire ».
    pub fn lire(dir: &Path) -> Manifeste {
        fs::read_to_string(dir.join(NOM))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Ce que le manifeste dit d'un fichier, s'il en dit quelque chose.
    pub fn artefact(&self, fichier: &str) -> Option<&Artefact> {
        self.artefacts.iter().find(|a| a.fichier == fichier)
    }
}

/// Note dans le manifeste le fichier que le rendu vient d'écrire dans `dir`.
///
/// Une entrée par fichier : un second `--cover` sous un autre profil remplace
/// la première, il ne s'empile pas derrière elle. Les entrées sont triées par
/// nom, pour qu'un manifeste ne dépende pas de l'ordre des deux commandes.
pub fn noter(dir: &Path, album: &Album, fichier: &str, profil: &str) -> Result<()> {
    let chemin = dir.join(fichier);
    let octets = fs::metadata(&chemin)
        .with_context(|| format!("{} : le rendu n'a rien laissé à noter", chemin.display()))?
        .len();
    let mut m = Manifeste::lire(dir);
    m.artefacts.retain(|a| a.fichier != fichier);
    m.artefacts.push(Artefact {
        fichier: fichier.to_string(),
        profil: profil.to_string(),
        album: empreinte_album(album),
        octets,
        date: crate::pdfx::stamp().to_rfc3339(),
    });
    m.artefacts.sort_by(|a, b| a.fichier.cmp(&b.fichier));
    let json = serde_json::to_string_pretty(&m)?;
    let out = dir.join(NOM);
    fs::write(&out, json + "\n").with_context(|| format!("écriture de {}", out.display()))?;
    Ok(())
}

/// L'empreinte de ce que l'album **montre**, en 32 chiffres hexadécimaux.
///
/// `version` et `root` sont neutralisés, et rien d'autre. `version` est
/// l'estampille de schéma : une migration la change sans que le livre change.
/// `root` est l'endroit où vivent les photographies, pas ce que montre le
/// livre — et `scripts/identite-fiches.py` le neutralise déjà pour exactement
/// cette raison. Tout le reste entre, parce que tout le reste se voit à
/// l'impression : le titre, le format, le fond perdu, les planches, la
/// couverture, le rythme, le colophon, les réglages, la police.
pub fn empreinte_album(album: &Album) -> String {
    let mut a = album.clone();
    a.version = 0;
    a.root = String::new();
    let octets = serde_json::to_vec(&a).expect("un album se sérialise toujours");
    hex(&empreinte(&octets))
}

/// Seize octets qui diffèrent d'un contenu à l'autre.
///
/// FNV-1a, deux fois, avec un décalage du biais de départ. C'est l'algorithme
/// que `pdfx` écrivait déjà pour le `/ID` d'un PDF, avec sa raison : « a
/// document identifier needs to differ between documents, not to resist an
/// adversary ». Le besoin est le même ici, donc la fonction est ici et les
/// deux appelants la partagent — le workspace n'a aucun crate de hachage, et
/// il n'en gagne pas un.
pub fn empreinte(octets: &[u8]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for round in 0..2usize {
        let mut h: u64 =
            0xcbf2_9ce4_8422_2325 ^ (round as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for b in octets {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        out[round * 8..round * 8 + 8].copy_from_slice(&h.to_be_bytes());
    }
    out
}

fn hex(octets: &[u8]) -> String {
    let mut s = String::with_capacity(octets.len() * 2);
    for b in octets {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---- le dossier prêt pour l'imprimeur ---------------------------------------

/// La fiche lisible posée dans le dossier préparé, à côté des deux PDF.
pub const FICHE: &str = "fiche.txt";

/// Ce que « Préparer » a écrit, et ce que le prévol en dit une fois relu.
#[derive(Debug)]
pub struct Preparation {
    /// Les noms des fichiers posés, jamais un chemin.
    pub fichiers: Vec<String>,
    /// Le prévol relu sur ces fichiers-là.
    pub rapport: PrevolReport,
}

/// Rend l'intérieur et la couverture de l'album `dir` pour `profil` dans le
/// dossier `dest`, les note dans son `export.json`, relit le prévol sur ce
/// dossier et y pose `fiche.txt`.
///
/// C'est l'asymétrie de 6.4 qui se ferme : la CLI notait ses rendus, l'app
/// écrivait où on lui disait et ne notait rien, donc un fichier sorti de la
/// fenêtre n'était jugé par personne. Ici le manifeste s'écrit par
/// [`noter`], la même fonction que la CLI appelle, mais dans le dossier de
/// la commande : il voyage avec les deux PDF qu'il décrit, et le prévol relu
/// dessus est celui des fichiers réellement écrits.
///
/// Rien n'est écrit à côté de l'album. Le dossier de l'album porte ses
/// aperçus et, parfois, la livraison d'une commande en cours ; le dossier
/// préparé est ailleurs, choisi par la personne.
pub fn preparer(
    dir: &Path,
    profil: &'static PrinterProfile,
    dest: &Path,
    progress: &dyn Fn(&str),
    cancel: crate::print::CancelFlag,
) -> Result<Preparation> {
    anyhow::ensure!(
        profil.fichiers == crate::printer::Fichiers::Deux,
        "{} relie un seul fichier : le dossier prêt ne sert que chez qui attend deux fichiers",
        profil.nom
    );
    let json = dir.join("album.json");
    let album: Album = serde_json::from_str(
        &fs::read_to_string(&json).with_context(|| format!("lecture de {}", json.display()))?,
    )
    .context("album.json illisible")?;
    fs::create_dir_all(dest).with_context(|| format!("création de {}", dest.display()))?;

    crate::print::render_print_pdf(dir, profil, &dest.join(LIVRAISON_INTERIEUR), progress, cancel)?;
    noter(dest, &album, LIVRAISON_INTERIEUR, profil.id)?;
    progress("cover: couverture");
    crate::cover::render_cover_pdf(dir, profil, &dest.join(LIVRAISON_COUVERTURE))
        .context("couverture")?;
    noter(dest, &album, LIVRAISON_COUVERTURE, profil.id)?;

    let rapport = crate::prevol::prevol_dossier(dir, dest, profil)?;

    let mut livres = Vec::new();
    for f in [LIVRAISON_INTERIEUR, LIVRAISON_COUVERTURE] {
        livres.push(empreintes_fichier(&dest.join(f), f)?);
    }
    let fiche = fiche_txt(&album, profil, &rapport, &livres);
    fs::write(dest.join(FICHE), fiche)
        .with_context(|| format!("écriture de {}", dest.join(FICHE).display()))?;

    Ok(Preparation {
        fichiers: [LIVRAISON_INTERIEUR, LIVRAISON_COUVERTURE, NOM, FICHE]
            .map(String::from)
            .to_vec(),
        rapport,
    })
}

/// Un fichier livré, tel que la fiche le décrit.
struct Livre {
    nom: &'static str,
    octets: u64,
    sha256: String,
    md5: String,
}

/// Lit le fichier une fois, par tranches, et nourrit les deux empreintes.
fn empreintes_fichier(chemin: &Path, nom: &'static str) -> Result<Livre> {
    let (mut md5, mut sha) = (Md5::new(), Sha256::new());
    let octets = par_tranches(chemin, |t| {
        md5.update(t);
        sha.update(t);
    })?;
    Ok(Livre { nom, octets, sha256: hex(&sha.finish()), md5: hex(&md5.finish()) })
}

/// Le MD5 d'un fichier livré, en hexadécimal : le `md5sum` que la commande
/// Cloudprinter porte pour chaque URL qu'elle tirera. Le même hachage que la
/// fiche, lu par les mêmes tranches.
pub fn md5_du_fichier(chemin: &Path) -> Result<String> {
    let mut md5 = Md5::new();
    par_tranches(chemin, |t| md5.update(t))?;
    Ok(hex(&md5.finish()))
}

/// Lit un fichier par tranches d'un mébioctet, jamais d'un bloc.
fn par_tranches(chemin: &Path, mut f: impl FnMut(&[u8])) -> Result<u64> {
    use std::io::Read;
    let mut fichier =
        fs::File::open(chemin).with_context(|| format!("lecture de {}", chemin.display()))?;
    let mut tampon = vec![0u8; 1 << 20];
    let mut octets = 0u64;
    loop {
        let n = fichier.read(&mut tampon)?;
        if n == 0 {
            return Ok(octets);
        }
        f(&tampon[..n]);
        octets += n as u64;
    }
}

/// Un nombre de millimètres comme la fiche de l'écran Envoi l'écrit.
fn mm(v: f64, decimales: usize) -> String {
    format!("{v:.decimales$}").replace('.', ",")
}

/// Des octets lisibles, groupés par trois avec une espace fine.
fn groupe(n: u64) -> String {
    let brut = n.to_string();
    let mut out = String::new();
    for (i, c) in brut.chars().enumerate() {
        if i > 0 && (brut.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// La fiche du dossier préparé, en clair : ce que l'imprimeur demande au
/// téléphone et ce que son formulaire de commande demande de recopier.
fn fiche_txt(album: &Album, profil: &PrinterProfile, r: &PrevolReport, livres: &[Livre]) -> String {
    let f = &r.fiche;
    let titre = crate::cover::titre_du_livre(album);
    let mut l: Vec<String> = Vec::new();
    l.push(format!("{titre}, pour {}", profil.nom));
    l.push(String::new());
    l.push(format!(
        "Format d'une page : {} × {} mm",
        mm(f.format_page_mm[0], 1),
        mm(f.format_page_mm[1], 1)
    ));
    l.push(format!(
        "Intérieur : {} planches, {} pages (le compte à déclarer, total_pages)",
        f.planches, f.pages_fichier
    ));
    let b = &f.fond_perdu_mm;
    l.push(format!(
        "Fond perdu : haut {}, bas {}, extérieur {}, dos {} mm",
        mm(b.haut, 1),
        mm(b.bas, 1),
        mm(b.exterieur, 1),
        mm(b.dos, 1)
    ));
    if let Some([w, h]) = f.feuille_couverture_mm {
        l.push(format!("Feuille de couverture : {} × {} mm", mm(w, 2), mm(h, 2)));
    }
    if let Some(d) = f.dos_mm {
        l.push(format!(
            "Dos : {} mm pour {} pages à {} g/m²",
            mm(d, 2),
            f.pages_interieur,
            f.grammage_g_m2
        ));
    }
    if let Some(c) = &profil.commande {
        l.push(format!("Papier de l'intérieur : {}", c.papier));
        l.push(format!("Papier de la couverture : {}", c.couverture));
        l.push(format!("Finition de la couverture : {}", c.finition));
    }
    l.push(format!(
        "Prévol relu sur ce dossier : {}",
        if r.ok {
            "rien ne s'oppose à l'impression".to_string()
        } else {
            format!("{} défaut(s) bloquant(s), voir l'écran Envoi", r.bloquants)
        }
    ));
    for livre in livres {
        l.push(String::new());
        l.push(livre.nom.to_string());
        l.push(format!("  taille : {} octets", groupe(livre.octets)));
        l.push(format!("  SHA-256 : {}", livre.sha256));
        l.push(format!("  MD5 : {}", livre.md5));
    }
    l.join("\n") + "\n"
}

// ---- les empreintes d'un fichier livré --------------------------------------
//
// L'API de Cloudprinter demande, pour chaque fichier qu'elle va tirer, son
// `md5sum` ; la fiche donne aussi le SHA-256, l'empreinte que la release
// publie déjà pour ses binaires. Le workspace n'a aucun crate de hachage et
// n'en gagne pas un pour hacher deux fichiers : les deux algorithmes sont
// écrits ici, contre leurs vecteurs publiés (RFC 1321, FIPS 180-4). Ni l'un
// ni l'autre ne sert à résister à quelqu'un : ils servent à ce que
// l'imprimeur sache qu'il a téléchargé le bon fichier.

/// MD5, RFC 1321. Par morceaux, pour hacher 95 Mo sans les tenir en mémoire.
pub struct Md5 {
    etat: [u32; 4],
    bloc: [u8; 64],
    rempli: usize,
    octets: u64,
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

impl Md5 {
    pub fn new() -> Self {
        Md5 {
            etat: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            bloc: [0; 64],
            rempli: 0,
            octets: 0,
        }
    }

    pub fn update(&mut self, donnees: &[u8]) {
        self.octets = self.octets.wrapping_add(donnees.len() as u64);
        let etat = &mut self.etat;
        par_blocs(&mut self.bloc, &mut self.rempli, donnees, |b| md5_bloc(etat, b));
    }

    pub fn finish(mut self) -> [u8; 16] {
        // Un 1, des zéros jusqu'à 56 modulo 64, puis la longueur en bits,
        // petit-boutiste : MD5 est petit-boutiste partout.
        let bits = self.octets.wrapping_mul(8);
        let queue = bourrage(self.octets);
        let etat = &mut self.etat;
        par_blocs(&mut self.bloc, &mut self.rempli, &queue, |b| md5_bloc(etat, b));
        par_blocs(&mut self.bloc, &mut self.rempli, &bits.to_le_bytes(), |b| md5_bloc(etat, b));
        let mut out = [0u8; 16];
        for (i, mot) in self.etat.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&mot.to_le_bytes());
        }
        out
    }
}

/// Les décalages des quatre rondes, RFC 1321 § 3.4.
const MD5_S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, //
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, //
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, //
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// `T[i] = ⌊2³² × |sin(i + 1)|⌋`, recopiée de la RFC plutôt que recalculée :
/// un sinus en virgule flottante n'a pas à décider d'une empreinte.
const MD5_T: [u32; 64] = [
    0xd76a_a478, 0xe8c7_b756, 0x2420_70db, 0xc1bd_ceee, 0xf57c_0faf, 0x4787_c62a, 0xa830_4613,
    0xfd46_9501, 0x6980_98d8, 0x8b44_f7af, 0xffff_5bb1, 0x895c_d7be, 0x6b90_1122, 0xfd98_7193,
    0xa679_438e, 0x49b4_0821, 0xf61e_2562, 0xc040_b340, 0x265e_5a51, 0xe9b6_c7aa, 0xd62f_105d,
    0x0244_1453, 0xd8a1_e681, 0xe7d3_fbc8, 0x21e1_cde6, 0xc337_07d6, 0xf4d5_0d87, 0x455a_14ed,
    0xa9e3_e905, 0xfcef_a3f8, 0x676f_02d9, 0x8d2a_4c8a, 0xfffa_3942, 0x8771_f681, 0x6d9d_6122,
    0xfde5_380c, 0xa4be_ea44, 0x4bde_cfa9, 0xf6bb_4b60, 0xbebf_bc70, 0x289b_7ec6, 0xeaa1_27fa,
    0xd4ef_3085, 0x0488_1d05, 0xd9d4_d039, 0xe6db_99e5, 0x1fa2_7cf8, 0xc4ac_5665, 0xf429_2244,
    0x432a_ff97, 0xab94_23a7, 0xfc93_a039, 0x655b_59c3, 0x8f0c_cc92, 0xffef_f47d, 0x8584_5dd1,
    0x6fa8_7e4f, 0xfe2c_e6e0, 0xa301_4314, 0x4e08_11a1, 0xf753_7e82, 0xbd3a_f235, 0x2ad7_d2bb,
    0xeb86_d391,
];

fn md5_bloc(etat: &mut [u32; 4], bloc: &[u8; 64]) {
    let mut x = [0u32; 16];
    for (i, m) in x.iter_mut().enumerate() {
        *m = u32::from_le_bytes([bloc[i * 4], bloc[i * 4 + 1], bloc[i * 4 + 2], bloc[i * 4 + 3]]);
    }
    let [mut a, mut b, mut c, mut d] = *etat;
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let tourne = a
            .wrapping_add(f)
            .wrapping_add(MD5_T[i])
            .wrapping_add(x[g])
            .rotate_left(MD5_S[i]);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(tourne);
    }
    etat[0] = etat[0].wrapping_add(a);
    etat[1] = etat[1].wrapping_add(b);
    etat[2] = etat[2].wrapping_add(c);
    etat[3] = etat[3].wrapping_add(d);
}

/// SHA-256, FIPS 180-4. Par morceaux, comme [`Md5`].
pub struct Sha256 {
    etat: [u32; 8],
    bloc: [u8; 64],
    rempli: usize,
    octets: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Sha256 {
            etat: [
                0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c,
                0x1f83_d9ab, 0x5be0_cd19,
            ],
            bloc: [0; 64],
            rempli: 0,
            octets: 0,
        }
    }

    pub fn update(&mut self, donnees: &[u8]) {
        self.octets = self.octets.wrapping_add(donnees.len() as u64);
        let etat = &mut self.etat;
        par_blocs(&mut self.bloc, &mut self.rempli, donnees, |b| sha256_bloc(etat, b));
    }

    pub fn finish(mut self) -> [u8; 32] {
        // Le même bourrage que MD5, la longueur gros-boutiste.
        let bits = self.octets.wrapping_mul(8);
        let queue = bourrage(self.octets);
        let etat = &mut self.etat;
        par_blocs(&mut self.bloc, &mut self.rempli, &queue, |b| sha256_bloc(etat, b));
        par_blocs(&mut self.bloc, &mut self.rempli, &bits.to_be_bytes(), |b| sha256_bloc(etat, b));
        let mut out = [0u8; 32];
        for (i, mot) in self.etat.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&mot.to_be_bytes());
        }
        out
    }
}

/// Les 64 constantes de FIPS 180-4 § 4.2.2.
const SHA256_K: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4,
    0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe,
    0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f,
    0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da, 0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7,
    0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc,
    0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
    0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070, 0x19a4_c116,
    0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
    0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7,
    0xc671_78f2,
];

fn sha256_bloc(etat: &mut [u32; 8], bloc: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([bloc[i * 4], bloc[i * 4 + 1], bloc[i * 4 + 2], bloc[i * 4 + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *etat;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in etat.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

/// Le bourrage commun aux deux : un bit à 1, puis des zéros jusqu'à ce qu'il
/// reste huit octets pour la longueur dans le dernier bloc.
fn bourrage(octets: u64) -> Vec<u8> {
    let reste = (octets % 64) as usize;
    let zeros = if reste < 56 { 55 - reste } else { 119 - reste };
    let mut v = vec![0u8; 1 + zeros];
    v[0] = 0x80;
    v
}

/// Remplit le bloc courant et appelle `compresse` à chaque bloc plein.
fn par_blocs(
    bloc: &mut [u8; 64],
    rempli: &mut usize,
    mut donnees: &[u8],
    mut compresse: impl FnMut(&[u8; 64]),
) {
    while !donnees.is_empty() {
        let n = (64 - *rempli).min(donnees.len());
        bloc[*rempli..*rempli + n].copy_from_slice(&donnees[..n]);
        *rempli += n;
        donnees = &donnees[n..];
        if *rempli == 64 {
            compresse(bloc);
            *rempli = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Police, Reglage, Size, Slot, Spread, SCHEMA};
    use std::path::PathBuf;

    fn md5_hex(octets: &[u8]) -> String {
        let mut h = Md5::new();
        h.update(octets);
        hex(&h.finish())
    }

    fn sha256_hex(octets: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(octets);
        hex(&h.finish())
    }

    /// Les sept vecteurs de l'annexe A.5 de la RFC 1321, tels quels.
    #[test]
    fn md5_rend_les_vecteurs_de_la_rfc_1321() {
        let vecteurs: [(&str, &str); 7] = [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            ("abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ];
        for (message, attendu) in vecteurs {
            assert_eq!(md5_hex(message.as_bytes()), attendu, "{message:?}");
        }
    }

    /// Les vecteurs de FIPS 180-4 (exemples du NIST) : un bloc, deux blocs,
    /// et le message vide.
    #[test]
    fn sha256_rend_les_vecteurs_du_nist() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Un million de « a » : le vecteur long du NIST, qui traverse des
        // milliers de blocs et les bornes de morceaux du lecteur.
        let mut h = Sha256::new();
        for _ in 0..1000 {
            h.update(&[b'a'; 1000]);
        }
        assert_eq!(
            hex(&h.finish()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// Un fichier se hache par morceaux de taille quelconque, et le découpage
    /// ne change rien : c'est ce qui permet de lire 95 Mo par tranches.
    #[test]
    fn le_decoupage_ne_change_pas_l_empreinte() {
        let donnees: Vec<u8> = (0..10_000u32).map(|i| (i * 31 % 251) as u8).collect();
        let (md5, sha) = (md5_hex(&donnees), sha256_hex(&donnees));
        for taille in [1usize, 3, 55, 56, 63, 64, 65, 127, 4096] {
            let mut a = Md5::new();
            let mut b = Sha256::new();
            for morceau in donnees.chunks(taille) {
                a.update(morceau);
                b.update(morceau);
            }
            assert_eq!(hex(&a.finish()), md5, "md5 par {taille}");
            assert_eq!(hex(&b.finish()), sha, "sha256 par {taille}");
        }
    }

    /// Le MD5 que la commande porte est celui de la fiche, lu sur le disque
    /// au-delà d'une tranche d'un mébioctet.
    #[test]
    fn le_md5_du_fichier_est_celui_de_ses_octets() {
        let donnees: Vec<u8> = (0..(3 << 20) + 5u32).map(|i| (i * 7 % 253) as u8).collect();
        let chemin = std::env::temp_dir().join(format!("colophon-md5-{}.bin", std::process::id()));
        fs::write(&chemin, &donnees).unwrap();
        let lu = md5_du_fichier(&chemin);
        fs::remove_file(&chemin).unwrap();
        assert_eq!(lu.unwrap(), md5_hex(&donnees));
        assert!(md5_du_fichier(Path::new("/nulle/part/album-print.pdf")).is_err());
    }

    fn album() -> Album {
        let mut a = Album::new("Corse", Path::new("/photos/corse"), Size { w: 210.0, h: 210.0 });
        for i in 0..4 {
            a.spreads.push(Spread {
                template: "solo".into(),
                slots: vec![Slot::new(format!("{i}.jpg"), [0.5, 0.5])],
                caption: None,
                text: None,
                edited: false,
                locked: false,
                objets: Vec::new(),
            });
        }
        a
    }

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir()
            .join(format!("colophon-export-{nom}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// L'empreinte est celle de ce que l'album **montre**. Une migration de
    /// schéma change l'estampille sans changer une ligne du livre, et un
    /// dossier de photographies déplacé ne change rien de ce qui s'imprime :
    /// les deux sortent de l'empreinte, et rien d'autre n'en sort.
    #[test]
    fn ni_le_schema_ni_le_dossier_des_photos_ne_changent_l_empreinte() {
        let a = album();
        let reference = empreinte_album(&a);

        let mut ailleurs = a.clone();
        ailleurs.root = "/Volumes/disque-externe/corse".into();
        assert_eq!(empreinte_album(&ailleurs), reference, "le dossier des photos");

        let mut migre = a.clone();
        migre.version = SCHEMA + 7;
        assert_eq!(empreinte_album(&migre), reference, "l'estampille de schéma");

        // Et relire puis réécrire `album.json` sans rien changer ne la bouge
        // pas non plus : c'est ce qui rend un manifeste utilisable après une
        // simple ouverture dans l'éditeur.
        let json = serde_json::to_string_pretty(&a).unwrap();
        let relu: Album = serde_json::from_str(&json).unwrap();
        assert_eq!(empreinte_album(&relu), reference, "l'aller-retour par le disque");
    }

    /// Une famille de changement par test, parce qu'une seule assertion
    /// groupée passerait encore le jour où l'empreinte cesserait de voir l'une
    /// d'entre elles.
    #[test]
    fn une_legende_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.spreads[2].slots[0].caption = Some("Bonifacio, le matin".into());
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    #[test]
    fn un_recadrage_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.spreads[1].slots[0].focal = [0.42, 0.5];
        assert_ne!(empreinte_album(&b), empreinte_album(&a));

        let mut c = a.clone();
        c.spreads[1].slots[0].zoom = 1.2;
        assert_ne!(empreinte_album(&c), empreinte_album(&a));
    }

    #[test]
    fn un_reglage_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.reglages.insert(
            "1.jpg".into(),
            Reglage { expo: 0.3, contraste: 0.0, nb: false },
        );
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    #[test]
    fn la_police_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.police = Some(Police {
            fichier: "police.ttf".into(),
            postscript: "HelveticaNeue".into(),
            nom: "Helvetica Neue Regular".into(),
        });
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    #[test]
    fn le_titre_change_l_empreinte() {
        let a = album();
        let mut b = a.clone();
        b.title = "Corse, 2013".into();
        assert_ne!(empreinte_album(&b), empreinte_album(&a));
    }

    /// Une entrée par fichier, et un second rendu remplace la première au lieu
    /// de s'empiler derrière elle : le prévol lirait sinon le profil d'un
    /// fichier qui n'existe plus.
    #[test]
    fn le_manifeste_garde_une_entree_par_fichier() {
        let dir = dossier("noter");
        let a = album();
        fs::write(dir.join("album-print.pdf"), b"x".repeat(11)).unwrap();
        fs::write(dir.join("album-cover.pdf"), b"y".repeat(7)).unwrap();

        noter(&dir, &a, "album-print.pdf", "generique").unwrap();
        noter(&dir, &a, "album-cover.pdf", "cloudprinter").unwrap();
        noter(&dir, &a, "album-print.pdf", "cloudprinter").unwrap();

        let m = Manifeste::lire(&dir);
        assert_eq!(m.artefacts.len(), 2, "{m:?}");
        // Triées par nom : deux commandes dans l'autre ordre rendent le même
        // fichier.
        assert_eq!(m.artefacts[0].fichier, "album-cover.pdf");
        let print = m.artefact("album-print.pdf").unwrap();
        assert_eq!(print.profil, "cloudprinter");
        assert_eq!(print.octets, 11);
        assert_eq!(print.album, empreinte_album(&a));
        assert_eq!(m.artefact("album-cover.pdf").unwrap().octets, 7);
        assert!(m.artefact("album.pdf").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Notre propre comptabilité ne refuse aucun album : un `export.json`
    /// qu'on ne comprend pas se lit comme un manifeste vide, donc le prévol se
    /// tait, au lieu de bloquer une commande sur un fichier annexe.
    #[test]
    fn un_manifeste_illisible_se_lit_comme_vide() {
        let dir = dossier("illisible");
        assert!(Manifeste::lire(&dir).artefacts.is_empty());
        fs::write(dir.join(NOM), "{ ceci n'est pas du JSON").unwrap();
        assert!(Manifeste::lire(&dir).artefacts.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Un album de douze planches aux vraies photographies, assez grandes
    /// pour Cloudprinter : chaque planche une pleine page au recto, et une
    /// couverture composée. Le plus petit album que leur prévol laisse
    /// passer, 24 pages.
    ///
    /// 2540 px de côté : sous les 2551 qu'une page de 216 mm demande à
    /// 300 dpi, donc l'intérieur passe la photo telle quelle au lieu de la
    /// rééchantillonner (soixante-dix secondes en debug à 3000 px, une et demie
    /// ici), et au-dessus des 250 ppi que la feuille cartonnée exige.
    fn album_imprimable(nom: &str) -> PathBuf {
        let dir = dossier(nom);
        let photos = dir.join("photos");
        fs::create_dir_all(&photos).unwrap();
        let img = image::RgbImage::from_fn(2540, 2540, |x, y| {
            image::Rgb([(x / 12) as u8, (y / 12) as u8, 140])
        });
        image::DynamicImage::ImageRgb8(img)
            .save_with_format(photos.join("a.jpg"), image::ImageFormat::Jpeg)
            .unwrap();
        let mut a = Album::new("Corse", &photos, Size { w: 210.0, h: 210.0 });
        a.bleed_mm = 3.0;
        for _ in 0..12 {
            a.spreads.push(Spread {
                template: "full1".into(),
                slots: vec![Slot::new("a.jpg".into(), [0.5, 0.5])],
                caption: None,
                text: None,
                edited: false,
                locked: false,
                objets: Vec::new(),
            });
        }
        a.cover = Some(crate::model::Cover {
            title: "Corse".into(),
            subtitle: "Du 21 au 29 octobre 2013".into(),
            photo: Some(Slot::new("a.jpg".into(), [0.5, 0.5])),
            back_text: String::new(),
        });
        fs::write(dir.join("album.json"), serde_json::to_string_pretty(&a).unwrap()).unwrap();
        dir
    }

    /// Le dossier prêt : les quatre fichiers, et rien d'autre ; le prévol
    /// relu **sur ce dossier** rend `ok` ; la fiche porte les empreintes que
    /// l'API demande, et elles sont justes. Et rien n'est écrit à côté de
    /// l'album : le dossier de l'album n'est pas le dossier de la commande.
    #[test]
    fn preparer_pose_quatre_fichiers_que_le_prevol_relu_laisse_passer() {
        let dir = album_imprimable("preparer");
        let dest = dir.join("Corse – Cloudprinter");
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();

        let p = preparer(&dir, cp, &dest, &|_| {}, &|| false).expect("le dossier se prépare");

        let mut poses: Vec<String> = fs::read_dir(&dest)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        poses.sort();
        assert_eq!(poses, ["album-cover.pdf", "album-print.pdf", "export.json", "fiche.txt"]);
        assert_eq!(p.fichiers, ["album-print.pdf", "album-cover.pdf", "export.json", "fiche.txt"]);
        for absent in [LIVRAISON_INTERIEUR, LIVRAISON_COUVERTURE, NOM, FICHE] {
            assert!(!dir.join(absent).exists(), "{absent} écrit à côté de l'album");
        }

        assert!(p.rapport.ok, "{:?}", p.rapport.defauts);
        assert_eq!(p.rapport.bloquants, 0);

        // Le manifeste dit que les deux sortent de cet album, pour ce profil.
        let m = Manifeste::lire(&dest);
        let album: Album =
            serde_json::from_str(&fs::read_to_string(dir.join("album.json")).unwrap()).unwrap();
        for f in [LIVRAISON_INTERIEUR, LIVRAISON_COUVERTURE] {
            let a = m.artefact(f).expect(f);
            assert_eq!(a.profil, "cloudprinter");
            assert_eq!(a.album, empreinte_album(&album));
            assert_eq!(a.octets, fs::metadata(dest.join(f)).unwrap().len());
        }

        // La fiche : ce qu'on déclare à la commande, et les empreintes
        // recalculées ici sur les octets du disque.
        let fiche = fs::read_to_string(dest.join(FICHE)).unwrap();
        for attendu in [
            "pageblock_150mcs",
            "cover_130mcg",
            "cover_finish_matte",
            "24 pages",
            "12 planches",
            "Feuille de couverture :",
            "Dos :",
            "Fond perdu :",
        ] {
            assert!(fiche.contains(attendu), "{attendu:?} manque :\n{fiche}");
        }
        for f in [LIVRAISON_INTERIEUR, LIVRAISON_COUVERTURE] {
            let octets = fs::read(dest.join(f)).unwrap();
            assert!(fiche.contains(&md5_hex(&octets)), "MD5 de {f} :\n{fiche}");
            assert!(fiche.contains(&sha256_hex(&octets)), "SHA-256 de {f} :\n{fiche}");
        }
        assert!(!fiche.contains('—'), "pas de tiret cadratin");

        // Et le prévol relu mord bien sur ce dossier-là : l'album change
        // après la préparation, les deux fichiers sont périmés.
        let mut retouche = album.clone();
        retouche.title = "Corse, 2013".into();
        fs::write(dir.join("album.json"), serde_json::to_string_pretty(&retouche).unwrap())
            .unwrap();
        let r = crate::prevol::prevol_dossier(&dir, &dest, cp).unwrap();
        let perimes: Vec<_> = r.defauts.iter().filter(|d| d.regle == "fichier_perime").collect();
        assert_eq!(perimes.len(), 2, "{:?}", r.defauts);
        // Alors que le prévol de l'album seul, qui ne voit aucun fichier à
        // côté de lui, n'a rien à en dire.
        let seul = crate::prevol::prevol(&dir, cp).unwrap();
        assert!(!seul.defauts.iter().any(|d| d.regle.starts_with("fichier_")), "{:?}", seul.defauts);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Préparer ne sert que chez qui relie deux fichiers : ailleurs, la
    /// couverture voyage dans l'intérieur et le dossier n'aurait pas de sens.
    #[test]
    fn preparer_refuse_un_profil_a_un_seul_fichier() {
        let dir = dossier("preparer-un");
        let dest = dir.join("dest");
        let gen = PrinterProfile::par_id("generique").unwrap();
        let err = preparer(&dir, gen, &dest, &|_| {}, &|| false).unwrap_err();
        assert!(format!("{err:#}").contains("deux fichiers"), "{err:#}");
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// L'empreinte est la fonction que `pdfx` écrivait pour le `/ID` d'un PDF,
    /// sortie de son module. Ces octets-là sont ceux d'avant le déménagement :
    /// s'ils bougeaient, chaque PDF exporté changerait, et le banc d'octets le
    /// dirait une heure plus tard au lieu d'ici.
    #[test]
    fn l_empreinte_est_celle_que_le_pdf_ecrivait_deja() {
        assert_eq!(
            hex(&empreinte(b"Corse|2023-11-14T23:13:20+01:00")),
            "932a4a473e384f8e388482902a03a995"
        );
        assert_eq!(hex(&empreinte(b"")), "cbf29ce48422232555c5e55dfb685f30");
    }
}
