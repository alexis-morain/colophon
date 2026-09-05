//! Album data model. `album.json` is the project's interchange format:
//! human-readable, diffable, hand-repairable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub version: u32,
    pub title: String,
    /// Absolute path of the scanned folder. Slot sources are relative to it,
    /// so an album reopened later still finds its photos.
    #[serde(default)]
    pub root: String,
    /// Trim size of a single page, in millimetres.
    pub trim_mm: Size,
    pub bleed_mm: f64,
    pub spreads: Vec<Spread>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<Cover>,
    /// The pace the album was composed at, chosen at the first build. Kept
    /// in the file so a recomposition keeps it: an album that changed
    /// density halfway would rebuild itself around spreads the user pinned
    /// under the other pace. Absent on albums composed before the choice
    /// existed, and read as the default.
    #[serde(default, skip_serializing_if = "crate::layout::Densite::is_default")]
    pub densite: crate::layout::Densite,
    /// What the composition knew about the album, kept for the colophon
    /// page. Stored rather than recomputed: the counts, the span, the towns
    /// and the cameras all cost a full reopen of every original, and the
    /// facts do not change when the album is edited. Absent on albums
    /// composed before the page existed, and the page is then simply not
    /// offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colophon: Option<crate::colophon::Faits>,
    /// Non-destructive adjustments, keyed by `Slot::src`. A property of the
    /// photograph, never of the cell: a photo on a spread and on the cover is
    /// adjusted once, a recomposition that rebuilds the spreads cannot lose
    /// it, and `reprise` — which reads `spreads` only — stays blind to it by
    /// construction. Applied where pixels are resolved (screen and export),
    /// never written to an original. An identity entry leaves the table at
    /// the edit that produced it, so absence means « no adjustment ».
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub reglages: std::collections::BTreeMap<String, Reglage>,
    /// The face the whole book is set in — captions, chapter titles,
    /// half-title, colophon, cover and spine. Absent means the face this
    /// crate ships, which is what every album composed before the picker
    /// existed says, and it needs no migration to say it: like `reglages`
    /// and `colophon` before it, the field is additive and absence is a
    /// meaning rather than a gap — the schema did not move for it, and when
    /// it later moved to 3 it was for [`Spread::objets`], not for this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub police: Option<Police>,
}

/// The face copied beside `album.json`, and who it was when it was chosen.
///
/// Three strings and no path: the file lives in the album's own folder under
/// one of two names, so a moved album carries its face with it and nothing
/// ever has to look for a font on the machine that opens it. The two names
/// are the whole vocabulary — [`crate::font::POLICE_TTF`] and
/// [`crate::font::POLICE_OTF`] — which is also what keeps a hand-repaired
/// `album.json` from naming a file outside the folder.
///
/// The names are kept because they are what a screen says and what a
/// colophon page could print one day. They are never used to *find* a face:
/// looking a font up by name on the opening machine is exactly the bug this
/// whole session exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Police {
    /// File name beside `album.json`, never a path.
    pub fichier: String,
    /// PostScript name of the face, as it goes in `/BaseFont`.
    pub postscript: String,
    /// Readable name at the time of the choice, family and style joined.
    pub nom: String,
}


/// One photograph's adjustments: exposure, contrast, black and white.
/// The transform these numbers name is defined once, in [`crate::reglage`],
/// and it is the CSS filter formula — which is what lets the editor show it
/// with one line of style.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Reglage {
    /// Exposure, in stops of the CSS `brightness(2^expo)`, clamped to ±1:
    /// enough to rescue a shot, not a darkroom.
    #[serde(default)]
    pub expo: f64,
    /// Contrast, `contrast(2^contraste)` around the 0,5 pivot, clamped to ±1.
    #[serde(default)]
    pub contraste: f64,
    /// Black and white: luma 709, the coefficients of `grayscale(1)`.
    #[serde(default)]
    pub nb: bool,
}

impl Reglage {
    /// The adjustment that adjusts nothing. Never stored: an identity entry
    /// leaves the table at the edit that produced it.
    pub fn est_identite(&self) -> bool {
        self.expo == 0.0 && self.contraste == 0.0 && !self.nb
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spread {
    pub template: String,
    pub slots: Vec<Slot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    /// Free text of a `texte` spread. Lines are printed as typed: the editor
    /// signals overlong lines, nothing is ever wrapped or cut silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Touched by hand in the editor. A recomposition never rebuilds it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub edited: bool,
    /// Pinned by the user without being edited. Same recomposition shield.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Free objects, the first thing this file has ever stored that no
    /// template can produce (wave 6.2). **Their order is their depth**, like
    /// every other depth in this project: object *n* prints over object
    /// *n − 1*, and all of them print over what the template produced.
    ///
    /// The field is additive and absent when empty, so an album that carries
    /// none is byte-identical to one written before free objects existed —
    /// the precedent is `Album::police`. **The schema still moved for it**
    /// (wave 6.4, [`SCHEMA`] 3), and that is the one case where an additive
    /// field is not enough: absence has a meaning here too, but a build that
    /// does not know the field drops what is present, at the reader's next
    /// save, without a word. The version is what lets such a build refuse
    /// instead. The linter counters and the preflight refusal landed with it,
    /// because that is where the consequences of a stored object are paid
    /// together rather than one at a time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objets: Vec<Objet>,
}

impl Spread {
    /// Survives a recomposition untouched.
    pub fn pinned(&self) -> bool {
        self.edited || self.locked
    }
}

/// One free object on a spread: a box, an angle, and what fills it.
///
/// The box is in the engine's own frame — millimetres, origin bottom-left of
/// the media box — like every rectangle this crate computes. It is stored
/// rather than derived because nothing else can produce it: a template owns
/// its slots, and this is precisely what no template owns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Objet {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Degrees, counter-clockwise, **around the rectangle's centre**. An
    /// angle and an origin, never an arbitrary matrix: everything that
    /// measures this object has to be able to recover its four corners, and
    /// a matrix would let a shear in through the same door.
    ///
    /// Absent = 0, so an album whose objects are all upright is byte-identical
    /// to one written before the field existed.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
    /// What the object is. One variant today; a clipart arrives in 6.3 as
    /// another, and the tag that tells them apart is already in the file.
    #[serde(flatten)]
    pub contenu: Contenu,
}

/// What fills a free object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Contenu {
    /// A block of text the reader placed. Unlike the three text *pages*,
    /// which print their lines as typed, a block has a width the reader drew,
    /// and a width is what a block means: it wraps at word boundaries, in the
    /// album's own face. Nothing is wrapped in silence — what does not fit
    /// runs past the bottom, the scene says so, and a single word wider than
    /// the box is reported rather than cut.
    Texte {
        texte: String,
        taille_pt: f64,
        /// Absent = the natural leading of that size (1.35 x), the ratio this
        /// engine has drawn a line box with since the first caption.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interligne_mm: Option<f64>,
        #[serde(default, skip_serializing_if = "Alignement::est_defaut")]
        alignement: Alignement,
    },
}

/// Where a wrapped line sits inside the box it was wrapped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignement {
    #[default]
    Gauche,
    Centre,
    Droite,
}

impl Alignement {
    fn est_defaut(&self) -> bool {
        matches!(self, Alignement::Gauche)
    }
}

impl Objet {
    /// The natural leading of a size, in millimetres: what an absent
    /// `interligne_mm` means. One place says it, so the two renderers and the
    /// emitter cannot each pick their own.
    pub fn interligne(&self) -> f64 {
        match &self.contenu {
            Contenu::Texte { taille_pt, interligne_mm, .. } => {
                interligne_mm.unwrap_or(taille_pt / (72.0 / 25.4) * 1.35)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slot {
    /// Path relative to the album root (the scanned folder).
    pub src: String,
    /// Focal point in [0,1]x[0,1], (0,0) = top-left. Cover-crop anchors here.
    pub focal: [f64; 2],
    /// Manual zoom past the cover fill, 1.0 = exact fill. Albums from before
    /// the crop editor carry no field and read as 1.0.
    #[serde(default = "default_zoom", skip_serializing_if = "is_default_zoom")]
    pub zoom: f64,
    /// Caption printed under the photo. Absent = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

impl Slot {
    pub fn new(src: String, focal: [f64; 2]) -> Self {
        Self { src, focal, zoom: 1.0, caption: None }
    }
}

/// The book's cover. Absent on albums composed before the cover editor;
/// the interface then seeds it from the album title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cover {
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
    /// Front-cover photo, cropped like any slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<Slot>,
    /// Back-cover text (the quatrième de couverture), optional.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub back_text: String,
}

pub fn default_zoom() -> f64 {
    1.0
}

/// The `album.json` schema this build writes.
///
/// 1 read `focal` as a fraction of the leftover room inside the cell, which
/// is cell-dependent: the same number showed a different part of the photo
/// as soon as the format changed, and a bascule therefore destroyed manual
/// work in silence. 2 reads it as a point of the image, which is a property
/// of the photograph and survives any ratio.
///
/// 3 means **this file may carry free objects**. Nothing in it reads
/// differently from 2 — a schema-2 album and a schema-3 album with no
/// `objets` say exactly the same thing — and the bump is therefore not about
/// meaning at all. It is about refusal: a build that does not know
/// [`Spread::objets`] drops them at the first save, in silence, and the only
/// way to stop that is a number it can compare. The refusal lives in
/// [`crate::build::migrate_album_folder`], which is what every reader of an
/// album folder passes through.
///
/// **A version is not a feature flag.** 3 says the file *may* carry objects,
/// never that it does: an album stamped 3 with no free object is the ordinary
/// case, and a build that only reads 2 would still be wrong to open it — it
/// would be right today and wrong at the reader's next save. The clipart of
/// 6.3 arrives as a second [`Contenu`] under this same 3, because it falls
/// into the exact same hole and the door is now shut.
pub const SCHEMA: u32 = 3;

/// The schema `focal` changed meaning at: below it, and only below it, the
/// number in the file is a fraction of the leftover room and has to be
/// converted by [`point_from_room`].
///
/// It exists so the migration can be **staged**. `migrate_album_folder` used
/// to convert for any version under `SCHEMA`, which was right while `SCHEMA`
/// was 2 and became a data-loss bug the instant it was not: every album at
/// schema 2 would have had its focals converted a second time, and the
/// migration says of itself that « une double migration abîme ce qu'une
/// simple réparait ». One conversion, one boundary, named.
pub const SCHEMA_FOCAL_POINT: u32 = 2;

/// One schema-1 `focal`, converted into a schema-2 `focal`.
///
/// A schema-1 focal placed the window at `x0 = (iw - vw) · focal`, so the
/// point of the image that window was centred on is `(x0 + vw/2) / iw`.
/// Written with `r = vw / iw` that is `focal · (1 - r) + r/2`, and `r` needs
/// no pixel count at all — only the two aspect ratios and the zoom:
///
/// ```text
/// rx = min(1, cellule / image) / zoom      ry = min(1, image / cellule) / zoom
/// ```
///
/// A thumbnail therefore answers as well as the original, its aspect ratio
/// being the original's to within one rounded pixel — about 0,03 % on a
/// 1600 px box, which moves a focal by 2·10⁻⁴.
///
/// There is no case to write for a photo with no room: `r` is 1 there, and
/// the formula gives 0,5 — the centre, which is exactly what "the whole axis
/// shows" means. The value stored under schema 1 was dead data; this reads
/// it as what it always meant.
pub fn point_from_room(
    cell_ratio: f64,
    image_ratio: f64,
    focal: [f64; 2],
    zoom: f64,
) -> [f64; 2] {
    if !(cell_ratio > 0.0) || !(image_ratio > 0.0) {
        return focal;
    }
    let z = zoom.max(1.0);
    let rx = ((cell_ratio / image_ratio).min(1.0) / z).clamp(0.0, 1.0);
    let ry = ((image_ratio / cell_ratio).min(1.0) / z).clamp(0.0, 1.0);
    [
        focal[0].clamp(0.0, 1.0) * (1.0 - rx) + rx / 2.0,
        focal[1].clamp(0.0, 1.0) * (1.0 - ry) + ry / 2.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An album.json written before S2 (no zoom, no captions per photo, no
    /// edited/locked flags, no cover) must open unchanged: zoom reads 1.0
    /// and every new field defaults quiet.
    #[test]
    fn pre_s2_album_json_still_opens() {
        let json = r#"{
            "version": 1,
            "title": "corse",
            "root": "/photos/corse",
            "trim_mm": { "w": 210.0, "h": 210.0 },
            "bleed_mm": 3.0,
            "spreads": [{
                "template": "duo",
                "slots": [
                    { "src": "a.jpg", "focal": [0.5, 0.42] },
                    { "src": "b.jpg", "focal": [0.2, 0.5] }
                ],
                "caption": "12 mars 2013"
            }]
        }"#;
        let album: Album = serde_json::from_str(json).expect("ancien album lisible");
        let slot = &album.spreads[0].slots[0];
        assert_eq!(slot.zoom, 1.0);
        assert!(slot.caption.is_none());
        assert!(!album.spreads[0].pinned());
        assert!(album.cover.is_none());

        // And the new fields stay off the file until they carry something:
        // album.json remains diffable across the schema change.
        let out = serde_json::to_string(&album).unwrap();
        assert!(!out.contains("zoom"));
        assert!(!out.contains("edited"));
        assert!(!out.contains("locked"));
        assert!(!out.contains("cover"));
        assert!(!out.contains("reglages"));
        assert!(!out.contains("police"));
    }

    /// The chosen face survives the round trip, an album that chose none
    /// writes no field, and **the schema does not move for either**. The
    /// precedent is `reglages`: additive, absent by default, read as
    /// « the face of the project ». A migration here would migrate nothing
    /// and age every album ever composed.
    ///
    /// The number is read from [`SCHEMA`] rather than written down, because
    /// the claim is « the picker did not move it », not « it is 2 ». It moved
    /// later, for the free objects of 6.4, and this test has nothing to say
    /// about that — an assertion on the literal would have turned red at that
    /// bump and been repaired by editing the number, which is the opposite of
    /// what it is for.
    #[test]
    fn la_police_est_additive_et_le_schema_ne_bouge_pas() {
        let mut album = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        assert!(album.police.is_none());
        assert_eq!(album.version, SCHEMA, "un album neuf porte le schéma courant");

        album.police = Some(Police {
            fichier: crate::font::POLICE_TTF.into(),
            postscript: "HelveticaNeue".into(),
            nom: "Helvetica Neue Regular".into(),
        });
        let back: Album =
            serde_json::from_str(&serde_json::to_string(&album).unwrap()).unwrap();
        let p = back.police.expect("la police survit");
        assert_eq!(p.fichier, "police.ttf");
        assert_eq!(p.postscript, "HelveticaNeue");
        assert_eq!(back.version, SCHEMA, "choisir une face ne migre rien");

        // And an album written before the picker reads as « the project's
        // face », with nothing to repair.
        let ancien: Album = serde_json::from_str(
            r#"{ "version": 2, "title": "t", "root": "/p",
                 "trim_mm": { "w": 210.0, "h": 210.0 }, "bleed_mm": 3.0,
                 "spreads": [] }"#,
        )
        .expect("un album d'avant le sélecteur s'ouvre tel quel");
        assert!(ancien.police.is_none());
    }


    /// The adjustments table survives the round trip, and an album without
    /// one writes no field at all: `reglages` is additive, absence means
    /// « no adjustment », the schema stays at 2.
    #[test]
    fn reglages_round_trip_et_absence_muette() {
        let mut album = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        assert!(album.reglages.is_empty());
        album.reglages.insert(
            "a.jpg".into(),
            Reglage { expo: 0.5, contraste: -0.25, nb: true },
        );
        let back: Album =
            serde_json::from_str(&serde_json::to_string(&album).unwrap()).unwrap();
        let r = back.reglages.get("a.jpg").expect("l'entrée survit");
        assert_eq!(r.expo, 0.5);
        assert_eq!(r.contraste, -0.25);
        assert!(r.nb);
        // A hand-repaired entry may name one field only: the others default.
        let partiel: Reglage = serde_json::from_str(r#"{ "nb": true }"#).unwrap();
        assert_eq!(partiel, Reglage { expo: 0.0, contraste: 0.0, nb: true });
        assert!(Reglage { expo: 0.0, contraste: 0.0, nb: false }.est_identite());
        assert!(!partiel.est_identite());
    }

    /// La conversion d'un focal de schéma 1 vers le schéma 2 se lit sur la
    /// fenêtre : le point rendu doit être celui sur lequel l'ancienne fenêtre
    /// était centrée. On le vérifie en refaisant les deux arithmétiques à la
    /// main, sans passer par crop_window, pour que les deux se surveillent.
    #[test]
    fn le_focal_migre_tombe_au_centre_de_lancienne_fenetre() {
        let (iw, ih) = (4000.0_f64, 3000.0_f64);
        let (rw, rh) = (300.0_f64, 200.0_f64);
        for zoom in [1.0_f64, 1.6, 3.0] {
            for f in [[0.0, 0.0], [0.42, 0.5], [1.0, 1.0], [0.73, 0.18]] {
                let s = (rw / iw).max(rh / ih) * zoom;
                let (vw, vh) = (rw / s, rh / s);
                // Ce que le schéma 1 montrait.
                let x0 = ((iw - vw) * f[0]).clamp(0.0, (iw - vw).max(0.0));
                let y0 = ((ih - vh) * f[1]).clamp(0.0, (ih - vh).max(0.0));
                let attendu = [(x0 + vw / 2.0) / iw, (y0 + vh / 2.0) / ih];

                let got = point_from_room(rw / rh, iw / ih, f, zoom);
                assert!(
                    (got[0] - attendu[0]).abs() < 1e-9,
                    "zoom {zoom} focal {f:?} : x {} attendu {}", got[0], attendu[0]
                );
                assert!(
                    (got[1] - attendu[1]).abs() < 1e-9,
                    "zoom {zoom} focal {f:?} : y {} attendu {}", got[1], attendu[1]
                );
            }
        }
    }

    /// Sans jeu, la valeur du schéma 1 ne voulait rien dire et n'était lue par
    /// personne. La migration rend le centre, et n'a aucun cas particulier.
    #[test]
    fn sans_jeu_la_migration_rend_le_centre() {
        // Cellule au ratio exact de l'image, zoom 1 : aucun jeu sur aucun axe.
        let got = point_from_room(4.0 / 3.0, 4.0 / 3.0, [0.13, 0.87], 1.0);
        assert!((got[0] - 0.5).abs() < 1e-12, "{got:?}");
        assert!((got[1] - 0.5).abs() < 1e-12, "{got:?}");
    }

    /// Un album neuf porte le schéma courant, jamais un littéral. Sans cette
    /// assertion, `Album::new` peut retomber sur un `1` et une composition
    /// toute neuve se ferait convertir au premier rendu comme si elle portait
    /// l'ancien sens de `focal`.
    #[test]
    fn un_album_neuf_porte_le_schema_courant() {
        let a = Album::new(
            "t",
            std::path::Path::new("/p"),
            Size { w: 210.0, h: 210.0 },
        );
        assert_eq!(a.version, SCHEMA);
    }

    /// A manual crop survives the round trip.
    #[test]
    fn zoomed_slot_round_trips() {
        let mut album = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        let mut slot = Slot::new("a.jpg".into(), [0.3, 0.6]);
        slot.zoom = 1.8;
        slot.caption = Some("la plage".into());
        album.spreads.push(Spread {
            template: "solo".into(),
            slots: vec![slot],
            caption: None,
            text: None,
            edited: true,
            locked: false,
            objets: Vec::new(),
        });
        let back: Album =
            serde_json::from_str(&serde_json::to_string(&album).unwrap()).unwrap();
        assert_eq!(back.spreads[0].slots[0].zoom, 1.8);
        assert_eq!(back.spreads[0].slots[0].caption.as_deref(), Some("la plage"));
        assert!(back.spreads[0].edited);
    }
}

fn is_default_zoom(z: &f64) -> bool {
    (*z - 1.0).abs() < 1e-9
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// One photo the curation set aside, and why. `curation.json` holds the
/// full list next to album.json: it is the sorting view's input, everything
/// the album does not show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discard {
    /// Path relative to the album root, like `Slot::src`.
    pub src: String,
    /// Machine-readable reason: `rejetee`, `parasite`, `panorama`,
    /// `definition`, `doublon`, `jumeau`, `meme_moment`, `hors_budget`,
    /// `originale_editee`.
    pub reason: String,
    /// The photo that won over this one, when the drop came from a
    /// comparison. Relative to the album root too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
    /// Focal point, same convention as `Slot::focal`, so a rescued photo is
    /// cropped like any other. Face anchor when one was found.
    #[serde(default = "default_focal")]
    pub focal: [f64; 2],
}

/// Same default as the composer: slightly above centre.
pub fn default_focal() -> [f64; 2] {
    [0.5, 0.42]
}

impl Album {
    pub fn new(title: &str, root: &std::path::Path, trim_mm: Size) -> Self {
        Self {
            // Le schéma courant, jamais un littéral : un album neuf estampillé
            // 1 serait « migré » au premier rendu, donc converti comme s'il
            // portait l'ancien sens de `focal`. Le pire des deux mondes.
            version: SCHEMA,
            title: title.to_string(),
            root: root.to_string_lossy().to_string(),
            trim_mm,
            bleed_mm: 3.0,
            spreads: Vec::new(),
            cover: None,
            densite: crate::layout::Densite::default(),
            colophon: None,
            reglages: std::collections::BTreeMap::new(),
            police: None,
        }

    }

    /// Portrait, landscape or square page. Drives the template choice.
    pub fn page_aspect(&self) -> f64 {
        self.trim_mm.w / self.trim_mm.h
    }
}
