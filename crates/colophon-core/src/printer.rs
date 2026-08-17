//! Printer profiles, as data.
//!
//! Every supplier differs on every field: bleed per edge, colour space, one
//! file or two, who computes the spine, how many pages the binding accepts.
//! None of that is ever hard-coded in the renderer: a profile is a value the
//! export and the preflight both read.
//!
//! Some numbers are confirmed by the supplier, others are our own reading of
//! their specification while the pre-sales answer is pending. That difference
//! is carried in the data ([`Certitude`]) rather than buried in a comment, so
//! the spec sheet handed to a human can say which is which.

use serde::Serialize;

/// How much we trust a field. A provisional value is usable, printable, and
/// flagged everywhere it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Certitude {
    /// Written down by the supplier, in their specification or in a mail.
    Confirme,
    /// Our reading, pending their answer. Bounded by measurement on the first
    /// printed album if the answer keeps not coming.
    Provisoire,
}

/// The PDF conformance a supplier asks for. X-1a is deliberately absent: it
/// would force our own CMYK conversion, the trap identified early on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PdfX {
    /// PDF/X-4:2010. Live transparency allowed, RGB allowed with an intent.
    X4,
    /// No conformance asked: a plain PDF is accepted.
    Aucun,
}

/// Working colour space of the delivered file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Espace {
    /// sRGB, converted by the printer's own RIP.
    Rgb,
    /// Coated FOGRA39, the European sheet-fed standard.
    Fogra39,
}

impl Espace {
    /// The OutputIntent identifier that goes in the PDF.
    pub fn output_intent(self) -> &'static str {
        match self {
            Espace::Rgb => "sRGB IEC61966-2.1",
            Espace::Fogra39 => "Coated FOGRA39 (ISO 12647-2:2004)",
        }
    }
}

/// Bleed per edge, in millimetres. Asymmetric on purpose: a supplier binding
/// the interior itself wants nothing on the spine side, and a symmetric value
/// there would push the image into the glue.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bleed {
    pub haut: f64,
    pub bas: f64,
    /// Outer edge, away from the binding.
    pub exterieur: f64,
    /// Spine side. Zero when the supplier binds and trims it themselves.
    pub dos: f64,
}

impl Bleed {
    pub const fn uniforme(mm: f64) -> Self {
        Self { haut: mm, bas: mm, exterieur: mm, dos: mm }
    }

    pub const fn aucun() -> Self {
        Self::uniforme(0.0)
    }

    pub fn max(&self) -> f64 {
        self.haut.max(self.bas).max(self.exterieur).max(self.dos)
    }
}

/// How the interior and the cover are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fichiers {
    /// One PDF, cover included as its first and last pages.
    Un,
    /// Interior and cover as two separate PDFs. The cover is one wide page:
    /// back + spine + front.
    Deux,
}

/// Who works out the spine width, and from what.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Dos {
    /// The supplier builds the cover and needs no spine from us.
    Fourni,
    /// We compute it: `feuilles × mm_par_feuille + constante`, where a sheet
    /// is two pages. The constant covers the boards and the glue.
    Calcule {
        mm_par_feuille: f64,
        constante_mm: f64,
        certitude: Certitude,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct PrinterProfile {
    pub id: &'static str,
    pub nom: &'static str,
    pub pdf_x: PdfX,
    pub espace: Espace,
    pub bleed_mm: Bleed,
    /// Keep-clear margin inside the trim. Text or a face closer than this to
    /// the cut is at the mercy of the trimming tolerance.
    pub safe_mm: f64,
    pub fichiers: Fichiers,
    /// The supplier reads one page of the PDF as one page of the book.
    ///
    /// False is the ordinary case: the interior travels as spreads and the
    /// press imposes them. True is a supplier who binds the file as it comes,
    /// which our spread-composed interior cannot satisfy yet, and which the
    /// preflight has to stop rather than let a half-length book go to press.
    pub pages_simples: bool,
    pub dos: Dos,
    pub pages_min: usize,
    pub pages_max: usize,
    /// Pages must be a multiple of this: a folded signature has no odd count.
    pub pas_pagination: usize,
    /// Effective resolution the supplier refuses to print below.
    pub min_ppi: f64,
    /// Overall trust in the profile, worst of its fields.
    pub certitude: Certitude,
    /// What still has to be confirmed, in words, for the spec sheet.
    pub reserves: &'static [&'static str],
}

impl PrinterProfile {
    /// Spine width in millimetres for a given page count and paper weight.
    /// `None` when the supplier builds the cover themselves.
    pub fn dos_mm(&self, pages: usize, grammage: f64) -> Option<f64> {
        match self.dos {
            Dos::Fourni => None,
            Dos::Calcule { mm_par_feuille, constante_mm, .. } => {
                // A sheet is two pages; the reference weight of the coefficient
                // is 150 g/m², so a heavier paper thickens the spine pro rata.
                let feuilles = pages as f64 / 2.0;
                Some(feuilles * mm_par_feuille * (grammage / GRAMMAGE_REFERENCE) + constante_mm)
            }
        }
    }

    /// Page count accepted by the binding, parity included.
    pub fn pagination_ok(&self, pages: usize) -> bool {
        pages >= self.pages_min && pages <= self.pages_max && pages % self.pas_pagination == 0
    }

    pub fn tous() -> &'static [PrinterProfile] {
        PROFILS
    }

    pub fn par_id(id: &str) -> Option<&'static PrinterProfile> {
        PROFILS.iter().find(|p| p.id == id)
    }
}

/// Reference paper weight of the spine coefficients, in g/m².
pub const GRAMMAGE_REFERENCE: f64 = 150.0;

/// Default paper weight until a profile carries its own catalogue.
pub const GRAMMAGE_DEFAUT: f64 = 150.0;

static PROFILS: &[PrinterProfile] = &[
    // Main supplier. Accepts FOGRA39 or RGB, binds from two files, and wants
    // the spine from us. The exact coefficient is the pre-sales question.
    PrinterProfile {
        id: "cloudprinter",
        nom: "Cloudprinter",
        pdf_x: PdfX::X4,
        espace: Espace::Rgb,
        bleed_mm: Bleed { haut: 3.0, bas: 3.0, exterieur: 3.0, dos: 0.0 },
        safe_mm: 5.0,
        fichiers: Fichiers::Deux,
        pages_simples: false,
        // Their own formula, docs.cloudprinter.com : gsm × main d'œuvre × (pages
        // / 2) / 1000 + 2 × épaisseur du carton. À 150 g/m² et main 0,80 (MCG),
        // la feuille pèse 0,12 mm ; le carton fait 3 mm, deux plats font 6.
        dos: Dos::Calcule {
            mm_par_feuille: 0.12,
            constante_mm: 6.0,
            certitude: Certitude::Provisoire,
        },
        pages_min: 24,
        pages_max: 200,
        pas_pagination: 2,
        min_ppi: 250.0,
        certitude: Certitude::Provisoire,
        reserves: &[
            "main d'œuvre 0,80 relevée pour le papier MCG : celle du MCS commandé reste à confirmer",
            "leur documentation prévient que la main du papier et le format du carton varient d'un imprimeur à l'autre : le dos calculé ici est une moyenne",
            "fond perdu de photobook_cw_s210_s_fc en cours de vérification chez eux, réponse du 14/08 en attente",
        ],
    },
    // Second supplier, and the only one that takes a single file and builds
    // the spine itself. That is why it is the fallback for the paper test.
    // Bornes et marge relevées sur leur guide « Hardcover photo books, file
    // set up guidelines » (8 pages, éd. 14/08/2026), confirmées par mail.
    PrinterProfile {
        id: "prodigi",
        nom: "Prodigi",
        pdf_x: PdfX::X4,
        espace: Espace::Rgb,
        // Ils fabriquent le fond perdu eux-mêmes et refusent les traits de
        // coupe : « do not add bleed or cut marks ».
        bleed_mm: Bleed::aucun(),
        // 10 mm depuis le bord, pas le quart de pouce supposé jusqu'ici.
        safe_mm: 10.0,
        fichiers: Fichiers::Un,
        pages_simples: true,
        dos: Dos::Fourni,
        // Bornes du SKU carré BOOK-FE-8_3-SQ-HARD-G (210 × 210), comptées sur
        // le PDF entier, couverture comprise.
        pages_min: 24,
        pages_max: 500,
        pas_pagination: 2,
        min_ppi: 250.0,
        certitude: Certitude::Provisoire,
        reserves: &[
            "bornes du SKU carré 210 × 210 : le 294 × 294 s'arrête à 298 pages, un autre produit aura d'autres bornes",
            "profil de repli, non livrable en l'état : notre intérieur sort en planches doubles et leur reliure lit une page de PDF par page de livre, le prévol le refuse (règle planches_doubles)",
            "ils recommandent un contrôle X-4 en FOGRA39 tout en demandant des images RVB : notre intention de sortie reste sRGB",
        ],
    },
    // Kept as a comparison point: symmetric bleed and CMYK, the opposite of
    // Prodigi on every field, which is exactly why the profile is data.
    PrinterProfile {
        id: "lulu",
        nom: "Lulu",
        pdf_x: PdfX::X4,
        espace: Espace::Fogra39,
        bleed_mm: Bleed::uniforme(3.0),
        safe_mm: 12.7,
        fichiers: Fichiers::Deux,
        pages_simples: false,
        dos: Dos::Calcule {
            mm_par_feuille: 0.2,
            constante_mm: 0.0,
            certitude: Certitude::Provisoire,
        },
        pages_min: 32,
        pages_max: 800,
        pas_pagination: 2,
        min_ppi: 250.0,
        certitude: Certitude::Provisoire,
        reserves: &["profil non testé : aucune commande passée chez eux"],
    },
    // The one that owes nothing to anybody: the file you hand to the printer
    // down the street. Loosest constraints, no conformance demanded.
    PrinterProfile {
        id: "generique",
        nom: "Imprimeur local (PDF sans contrainte)",
        pdf_x: PdfX::Aucun,
        espace: Espace::Rgb,
        bleed_mm: Bleed::uniforme(3.0),
        safe_mm: 5.0,
        fichiers: Fichiers::Un,
        pages_simples: false,
        dos: Dos::Fourni,
        pages_min: 2,
        pages_max: 1000,
        pas_pagination: 2,
        min_ppi: 250.0,
        certitude: Certitude::Confirme,
        reserves: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing about a supplier lives in code: every profile answers the
    /// same questions, and they genuinely disagree.
    #[test]
    fn profiles_disagree_on_every_field() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        let pr = PrinterProfile::par_id("prodigi").unwrap();
        let lu = PrinterProfile::par_id("lulu").unwrap();

        // Bleed: asymmetric, absent, symmetric.
        assert_eq!(cp.bleed_mm.dos, 0.0);
        assert!(cp.bleed_mm.exterieur > 0.0);
        assert_eq!(pr.bleed_mm.max(), 0.0);
        assert_eq!(lu.bleed_mm.dos, lu.bleed_mm.exterieur);

        // Files, colour space, spine: all different.
        assert_eq!(pr.fichiers, Fichiers::Un);
        assert_eq!(cp.fichiers, Fichiers::Deux);
        assert_eq!(lu.espace, Espace::Fogra39);
        assert_eq!(cp.espace, Espace::Rgb);
        assert!(pr.dos_mm(80, GRAMMAGE_DEFAUT).is_none());
        assert!(cp.dos_mm(80, GRAMMAGE_DEFAUT).is_some());
    }

    /// The spine grows with the page count and with the paper.
    #[test]
    fn spine_follows_pages_and_paper() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        // 80 pages = 40 sheets at 0.12 mm, plus the two 3 mm boards.
        let d = cp.dos_mm(80, 150.0).unwrap();
        assert!((d - (40.0 * 0.12 + 6.0)).abs() < 1e-9, "{d}");
        // Twice the pages is thicker, heavier paper is thicker.
        assert!(cp.dos_mm(160, 150.0).unwrap() > d);
        assert!(cp.dos_mm(80, 200.0).unwrap() > d);
    }

    /// Cloudprinter publishes a formula, not a coefficient: gsm × bulk ×
    /// sheets / 1000 + two boards. Our two numbers are that formula folded
    /// down to a 150 g/m² reference, and the fold has to stay exact or the
    /// spine drifts silently on every other paper weight.
    #[test]
    fn cloudprinter_spine_matches_the_published_formula() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        const BULK_MCG: f64 = 0.80;
        const CARTON_MM: f64 = 3.0;
        for (pages, gsm) in [(24usize, 150.0), (96, 150.0), (200, 170.0), (96, 200.0)] {
            let leur = gsm * BULK_MCG * (pages as f64 / 2.0) / 1000.0 + 2.0 * CARTON_MM;
            let notre = cp.dos_mm(pages, gsm).unwrap();
            assert!((notre - leur).abs() < 1e-9, "{pages} p à {gsm} g : {notre} ≠ {leur}");
        }
    }

    /// Prodigi's guide is the authority on its own file: 10 mm of safe area,
    /// 24 to 500 pages on the square SKU, no bleed of ours. A profile that
    /// drifts from the guide passes files their press will refuse.
    #[test]
    fn prodigi_matches_its_published_guide() {
        let pr = PrinterProfile::par_id("prodigi").unwrap();
        assert_eq!(pr.safe_mm, 10.0, "marge de sécurité du guide");
        assert_eq!(pr.bleed_mm.max(), 0.0, "ils refusent notre fond perdu");
        assert!(!pr.pagination_ok(22), "sous les 24 pages du guide");
        assert!(pr.pagination_ok(24));
        assert!(pr.pagination_ok(500));
        assert!(!pr.pagination_ok(502), "au-dessus des 500 pages du guide");
        assert!(!pr.pagination_ok(25), "pagination impaire");
    }

    /// A binding refuses an odd page count and anything out of its range.
    #[test]
    fn pagination_bounds_are_enforced() {
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        assert!(cp.pagination_ok(96));
        assert!(!cp.pagination_ok(97), "pagination impaire");
        assert!(!cp.pagination_ok(12), "sous le minimum");
        assert!(!cp.pagination_ok(400), "au-dessus du maximum");
    }

    /// A provisional profile says what it is waiting for. Silence would let a
    /// guessed spine reach a print run.
    #[test]
    fn provisional_profiles_carry_their_reservations() {
        for p in PrinterProfile::tous() {
            if p.certitude == Certitude::Provisoire {
                assert!(!p.reserves.is_empty(), "{} ne dit pas ce qu'il attend", p.id);
            }
        }
        // And a computed spine flags itself where the number is used.
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        match cp.dos {
            Dos::Calcule { certitude, .. } => assert_eq!(certitude, Certitude::Provisoire),
            Dos::Fourni => panic!("Cloudprinter calcule son dos"),
        }
    }
}
