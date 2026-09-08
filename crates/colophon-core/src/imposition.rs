//! Cutting a composed spread into the single pages a binder reads one at a
//! time.
//!
//! The book is composed in spreads and always will be: no image crosses the
//! fold, the editor shows two facing pages, and every geometry in this crate
//! is expressed on a spread's media box. Some suppliers, though, bind one PDF
//! page as one page of the book ([`crate::printer::PrinterProfile::pages_simples`]),
//! and Cloudprinter's own template is a single 216 × 216 page with bleed on
//! all four sides.
//!
//! **The frame changes at the export, never at the composition.** Nothing
//! here touches [`crate::pdf::geometry`], `album.json`, the editor or a
//! fiche: this module is a translation of a scene that was already built, and
//! that immobility is what the gate proves. Two things happen to an object on
//! the way out, and nothing else:
//!
//! 1. a photograph whose cell **runs to the fold** runs `pli_mm` past it, the
//!    way it already runs past the three other edges. That is the whole of
//!    the interior bleed, and it is the one change that moves a number: the
//!    cover-crop shows a wider rectangle, so the effective resolution of that
//!    photograph drops. Which is why [`rect_exporte`] exists and why the
//!    preflight reads it rather than the composition rectangle;
//! 2. everything is translated into the coordinates of its own page.
//!
//! Text never bleeds, and neither does an object the reader placed: a block
//! butted against the fold by the editor stops at the fold, because bleeding
//! it would push it into the glue.

use crate::model::Album;
use crate::pdf::{Rect, SpreadGeometry};
use crate::printer::PrinterProfile;
use crate::scene::{Object, Role, Scene};

/// Which page of a spread. There is no third value: the fold is hard, and
/// every guard in the project already refuses anything that straddles it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cote {
    Gauche,
    Droite,
}

/// One page of the interior, in the order the file carries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// A page of spread `planche` (0-based).
    Planche(usize, Cote),
    /// The blank verso that closes the block.
    Blanche,
}

/// The sheet one single page prints on, in millimetres, origin bottom-left.
///
/// Square in the ordinary case and by construction: the outer bleed the album
/// was composed with on one side, the fold bleed the binder asks for on the
/// other, the trimmed page in between. At Cloudprinter that is 3 + 210 + 3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSimple {
    pub media_w: f64,
    pub media_h: f64,
    /// `[x0, y0, x1, y1]`, the finished page inside the sheet.
    pub trim: [f64; 4],
}

/// How far past the fold a photograph bleeds, for this supplier.
///
/// Zero for everyone who binds spreads: a spread has no edge at the fold, the
/// two pages being one surface, and this is exactly what
/// [`crate::printer::Bleed::dos`] has always meant. A supplier who cuts the
/// block page by page does have one, and Cloudprinter asks for the same 3 mm
/// there as everywhere else.
pub fn pli_mm(profil: &PrinterProfile) -> f64 {
    if profil.pages_simples {
        profil.bleed_mm.dos
    } else {
        0.0
    }
}

/// The faces spread `planche` contributes to the interior, in order.
///
/// **The imposition, in one place.** A naive cut would send the first spread's
/// left page out as page 1 and put the half-title on a verso, against the
/// doctrine of the page itself ([`crate::garde`] sets it on the recto, and
/// [`crate::colophon`] does the same at the other end). So the first spread
/// gives its recto alone — its left half is the page facing the inside of the
/// cover, which nothing composes and nothing prints — and a blank verso closes
/// the block:
///
/// ```text
/// p1    = planche 1, droite      (le faux-titre, recto)
/// p2k   = planche k+1, gauche
/// p2k+1 = planche k+1, droite    pour k = 1..n-1
/// p2n   = blanche                (verso du colophon)
/// ```
///
/// The count still lands on `2n`, every pair composed facing stays facing, and
/// the two pages a book sets on a recto — the half-title and the colophon —
/// come out odd. What the rule needs in exchange is that the left half of the
/// first spread be empty, and the preflight refuses rather than dropping ink
/// in silence.
pub fn faces(planche: usize) -> &'static [Cote] {
    if planche == 0 {
        &[Cote::Droite]
    } else {
        &[Cote::Gauche, Cote::Droite]
    }
}

/// The whole interior, page by page. Built from [`faces`] rather than beside
/// it: two statements of an imposition is one too many.
pub fn ordre(planches: usize) -> Vec<Page> {
    if planches == 0 {
        return Vec::new();
    }
    let mut v = Vec::with_capacity(planches * 2);
    for planche in 0..planches {
        v.extend(faces(planche).iter().map(|c| Page::Planche(planche, *c)));
    }
    v.push(Page::Blanche);
    v
}

/// The sheet a single page prints on.
///
/// The fold bleed sits on the side the fold is on — the right of a left page,
/// the left of a right page — so the trim box is not centred on the sheet
/// unless the two bleeds happen to be equal, which at Cloudprinter they are.
pub fn page_simple(album: &Album, cote: Cote, pli_mm: f64) -> PageSimple {
    let b = album.bleed_mm;
    let (w, h) = (album.trim_mm.w, album.trim_mm.h);
    let x0 = match cote {
        Cote::Gauche => b,
        Cote::Droite => pli_mm,
    };
    PageSimple {
        media_w: w + b + pli_mm,
        media_h: h + b * 2.0,
        trim: [x0, b, x0 + w, b + h],
    }
}

/// Which page of the spread an object belongs to.
///
/// By the centre of its box, which is unambiguous because nothing may cross
/// the fold: photographs stop at it by construction, and a free object is
/// stopped by the editor and refused by the preflight.
pub fn cote_de(objet: &Object, g: &SpreadGeometry) -> Cote {
    if objet.rect.x + objet.rect.w / 2.0 < g.media_w / 2.0 {
        Cote::Gauche
    } else {
        Cote::Droite
    }
}

/// A rectangle exactly at the fold is a rectangle that runs to it. The
/// comparison is an equality — both numbers come from `g.media_w / 2.0`,
/// [`crate::pdf::full_page`] being the only thing that reaches the fold — and
/// the epsilon guards the float, not the intent.
const AU_PLI: f64 = 1e-9;

/// The rectangle the export draws an object in, still in the spread's frame.
///
/// Only a photograph is widened, and only when its cell runs to the fold.
/// This is the function the preflight has to measure resolution on: a
/// full-page photograph gains `pli_mm` of width without gaining a pixel, so
/// its effective ppi drops, and reading the composition rectangle would
/// promise 250 where the press receives less.
pub fn rect_exporte(objet: &Object, g: &SpreadGeometry, pli_mm: f64) -> Rect {
    let r = objet.rect;
    if pli_mm <= 0.0 || !matches!(objet.role, Role::Photo { .. }) {
        return r;
    }
    let pli = g.media_w / 2.0;
    match cote_de(objet, g) {
        Cote::Gauche if (r.x + r.w - pli).abs() < AU_PLI => Rect { w: r.w + pli_mm, ..r },
        Cote::Droite if (r.x - pli).abs() < AU_PLI => {
            Rect { x: r.x - pli_mm, w: r.w + pli_mm, ..r }
        }
        _ => r,
    }
}

/// How far one page's frame is shifted from the spread's.
fn decalage(cote: Cote, g: &SpreadGeometry, pli_mm: f64) -> f64 {
    match cote {
        Cote::Gauche => 0.0,
        Cote::Droite => pli_mm - g.media_w / 2.0,
    }
}

/// One page's objects, in the page's own coordinates, paint order kept.
///
/// A copy rather than a view: the emitter draws from a scene and only from a
/// scene, so the cheapest way to keep one drawing loop for both modes is to
/// hand it a scene that is already imposed.
pub fn page(scene: &Scene, cote: Cote, g: &SpreadGeometry, pli_mm: f64) -> Vec<Object> {
    let dx = decalage(cote, g, pli_mm);
    scene
        .objects
        .iter()
        .filter(|o| cote_de(o, g) == cote)
        .map(|o| {
            let mut o = o.clone();
            o.rect = rect_exporte(&o, g, pli_mm);
            o.rect.x += dx;
            deplacer(&mut o.role, dx);
            o
        })
        .collect()
}

/// Slide an object's anchor with its box.
///
/// An exhaustive match on purpose: the day a role carries a second point, this
/// stops compiling instead of quietly leaving that point in the old frame.
fn deplacer(role: &mut Role, dx: f64) {
    match role {
        Role::PhotoCaption { at, .. }
        | Role::ChapterCaption { at, .. }
        | Role::Text { at, .. }
        | Role::FreeText { at, .. } => at.x += dx,
        Role::Photo { .. } | Role::Ornement { .. } => {}
    }
}

/// How many objects sit on the left page of a spread.
///
/// The preflight's question about the first spread, and only about it: that
/// page is the one the imposition does not print.
pub fn objets_a_gauche(scene: &Scene, g: &SpreadGeometry) -> usize {
    scene.objects.iter().filter(|o| cote_de(o, g) == Cote::Gauche).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Size, Slot, Spread};
    use crate::pdf;

    fn planche(template: &str, photos: usize) -> Spread {
        Spread {
            template: template.into(),
            slots: (0..photos).map(|i| Slot::new(format!("{i}.jpg"), [0.5, 0.5])).collect(),
            caption: None,
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        }
    }

    fn album(template: &str, photos: usize) -> Album {
        let mut a = Album::new("t", std::path::Path::new("/p"), Size { w: 210.0, h: 210.0 });
        a.bleed_mm = 3.0;
        a.spreads.push(planche(template, photos));
        a
    }

    /// L'imposition de la décision 3, page par page : le recto seul en tête,
    /// une blanche en queue, le compte qui tombe sur 2n, et les deux pages
    /// composées en regard qui restent en regard.
    #[test]
    fn l_imposition_pose_le_faux_titre_en_page_un() {
        let pages = ordre(48);
        assert_eq!(pages.len(), 96, "48 planches font 96 pages");
        assert_eq!(pages[0], Page::Planche(0, Cote::Droite), "p1 est un recto");
        assert_eq!(pages[1], Page::Planche(1, Cote::Gauche), "p2");
        assert_eq!(pages[2], Page::Planche(1, Cote::Droite), "p3");
        assert_eq!(pages[93], Page::Planche(47, Cote::Gauche), "p94");
        assert_eq!(pages[94], Page::Planche(47, Cote::Droite), "p95, le colophon");
        assert_eq!(pages[95], Page::Blanche, "p96, le verso du colophon");

        // La moitié gauche de la première planche est la seule qui ne
        // s'imprime pas, et c'est la seule.
        assert!(!pages.contains(&Page::Planche(0, Cote::Gauche)));
        for p in 1..48 {
            assert!(pages.contains(&Page::Planche(p, Cote::Gauche)), "{p} à gauche");
            assert!(pages.contains(&Page::Planche(p, Cote::Droite)), "{p} à droite");
        }

        // Les deux pages d'une planche restent adjacentes, et dans l'ordre.
        for (i, p) in pages.iter().enumerate() {
            if let Page::Planche(planche, Cote::Gauche) = p {
                assert_eq!(pages[i + 1], Page::Planche(*planche, Cote::Droite), "p{}", i + 1);
            }
        }

        // Le faux-titre est en p1 et le colophon en p95 : deux impaires, donc
        // deux rectos, comme dans un livre.
        let recto = |i: usize| (i + 1) % 2 == 1;
        assert!(recto(0) && recto(94));
        assert!(ordre(0).is_empty(), "un album sans planche n'a pas de blanche");
    }

    /// La feuille d'une page simple : le fond perdu extérieur d'un côté,
    /// celui du pli de l'autre, la page finie entre les deux. Le gabarit de
    /// Cloudprinter dit 216 × 216 avec 3 mm partout, et c'est ce qui sort.
    #[test]
    fn une_page_simple_fait_la_feuille_du_gabarit() {
        let a = album("duo", 2);
        let gauche = page_simple(&a, Cote::Gauche, 3.0);
        let droite = page_simple(&a, Cote::Droite, 3.0);
        assert_eq!((gauche.media_w, gauche.media_h), (216.0, 216.0));
        assert_eq!(gauche, droite, "à fond perdu égal les deux feuilles coïncident");
        assert_eq!(gauche.trim, [3.0, 3.0, 213.0, 213.0]);

        // Le pli est du côté du pli : à fond perdu inégal la page finie n'est
        // plus centrée sur la feuille, et pas du même côté à gauche et à
        // droite.
        let g = page_simple(&a, Cote::Gauche, 1.0);
        let d = page_simple(&a, Cote::Droite, 1.0);
        assert_eq!((g.media_w, g.media_h), (214.0, 216.0));
        assert_eq!(g.trim, [3.0, 3.0, 213.0, 213.0], "le pli est à droite d'une page gauche");
        assert_eq!(d.trim, [1.0, 3.0, 211.0, 213.0], "et à gauche d'une page droite");
    }

    /// Le fond perdu intérieur : une photo pleine page en donne trois
    /// millimètres de plus vers le pli, et rien d'autre ne bouge. C'est la
    /// seule chose que l'export change à un rectangle.
    #[test]
    fn seule_une_photo_qui_atteint_le_pli_saigne_vers_le_pli() {
        let a = album("full1", 1); // page de gauche vide, pleine page à droite
        let g = pdf::geometry(&a);
        let scene = Scene::of(&a.spreads[0], &g);
        let photo = &scene.objects[0];
        assert_eq!((photo.rect.x, photo.rect.w), (213.0, 213.0), "elle s'arrête pile au pli");
        let r = rect_exporte(photo, &g, 3.0);
        assert_eq!((r.x, r.w), (210.0, 216.0), "et elle part maintenant 3 mm avant");

        let a = album("full1_verso", 1); // la même, retournée
        let scene = Scene::of(&a.spreads[0], &g);
        let photo = &scene.objects[0];
        assert_eq!((photo.rect.x, photo.rect.w), (0.0, 213.0));
        let r = rect_exporte(photo, &g, 3.0);
        assert_eq!((r.x, r.w), (0.0, 216.0), "elle va de la coupe au pli, plus 3");

        // Sans pli, rien ne bouge : c'est ce qui rend l'export d'un profil en
        // planches doubles identique à ce qu'il était.
        let r = rect_exporte(photo, &g, 0.0);
        assert_eq!((r.x, r.w), (photo.rect.x, photo.rect.w));

        // Une case en marge n'atteint pas le pli et ne saigne pas.
        let a = album("duo", 2);
        for o in Scene::of(&a.spreads[0], &g).objects.iter() {
            assert_eq!(rect_exporte(o, &g, 3.0).w, o.rect.w, "une case margée ne saigne pas");
        }
    }

    /// Ni un texte ni un objet posé à la main ne saigne : le pli est de la
    /// colle, et l'éditeur y bute exprès. La preuve se fait sur un bloc dont
    /// la boîte touche le pli au millimètre — l'état où `retenirAuPli` laisse
    /// un objet glissé jusqu'au bout.
    #[test]
    fn ni_un_texte_ni_un_objet_libre_ne_saigne_au_pli() {
        use crate::model::{Alignement, Contenu, Objet};
        let mut a = album("duo", 2);
        let g = pdf::geometry(&a);
        a.spreads[0].objets = vec![Objet {
            x: g.media_w / 2.0 - 40.0,
            y: 60.0,
            w: 40.0,
            h: 20.0,
            angle: 0.0,
            contenu: Contenu::Texte {
                texte: "Calvi".into(),
                taille_pt: 10.0,
                interligne_mm: Some(5.0),
                alignement: Alignement::Gauche,
            },
        }];
        let scene = Scene::of(&a.spreads[0], &g);
        let bloc = scene
            .objects
            .iter()
            .find(|o| matches!(o.role, Role::FreeText { .. }))
            .expect("le bloc est dans la scène");
        assert_eq!(bloc.rect.x + bloc.rect.w, g.media_w / 2.0, "il bute sur le pli");
        assert_eq!(rect_exporte(bloc, &g, 3.0).w, bloc.rect.w, "et il s'y arrête");
    }

    /// La découpe : chaque objet part sur sa page, dans les coordonnées de
    /// cette page, l'ordre de peinture gardé, et l'ancre d'un texte suit sa
    /// boîte.
    #[test]
    fn la_decoupe_range_chaque_objet_dans_sa_page() {
        let mut a = album("duo", 2);
        a.spreads[0].slots[0].caption = Some("Calvi".into());
        a.spreads[0].slots[1].caption = Some("Bonifacio".into());
        let g = pdf::geometry(&a);
        let scene = Scene::of(&a.spreads[0], &g);

        let gauche = page(&scene, Cote::Gauche, &g, 3.0);
        let droite = page(&scene, Cote::Droite, &g, 3.0);
        assert_eq!(gauche.len() + droite.len(), scene.objects.len(), "rien ne se perd");
        assert_eq!(gauche.len(), 2, "une photo et sa légende");

        // La page gauche ne bouge pas : son bord extérieur est déjà l'origine.
        let cellule = pdf::page_box(false, &g);
        assert_eq!(gauche[0].rect.x, cellule.x);
        // La page droite est ramenée sur sa propre feuille : le pli, qui était
        // à 213, devient le fond perdu de 3.
        let droite_avant = pdf::page_box(true, &g);
        assert!((droite[0].rect.x - (droite_avant.x - 210.0)).abs() < 1e-9, "{:?}", droite[0].rect);
        let Role::PhotoCaption { at, .. } = &droite[1].role else { panic!("une légende") };
        assert!((at.x - (droite_avant.x - 210.0)).abs() < 1e-9, "l'ancre suit la boîte : {}", at.x);

        // Et tout reste dans la feuille de sa page.
        let feuille = page_simple(&a, Cote::Droite, 3.0);
        for o in droite.iter().chain(gauche.iter()) {
            assert!(
                o.rect.x >= -1e-9 && o.rect.x + o.rect.w <= feuille.media_w + 1e-9,
                "{:?} déborde de la feuille",
                o.rect
            );
        }
    }

    /// La page que l'imposition n'imprime pas doit être vide, et c'est le
    /// prévol qui le refuse. Ici on mesure seulement ce qu'il lit : une page
    /// de garde ne porte rien à gauche, une planche de photos si.
    #[test]
    fn la_moitie_gauche_d_une_page_de_garde_est_vide() {
        let a = album("duo", 2);
        let g = pdf::geometry(&a);
        assert_eq!(
            objets_a_gauche(&Scene::of(&a.spreads[0], &g), &g),
            1,
            "un duo porte une photo à gauche"
        );

        let mut garde = planche(crate::garde::TEMPLATE, 0);
        garde.text = Some("Corse".into());
        assert_eq!(
            objets_a_gauche(&Scene::of(&garde, &g), &g),
            0,
            "le faux-titre est au recto, sa page de gauche est blanche"
        );
    }

    /// Le pli ne vient que du profil, et seulement d'un imprimeur qui relie
    /// page par page. Personne d'autre n'en reçoit un millimètre.
    #[test]
    fn le_pli_ne_vient_que_d_un_relieur_page_par_page() {
        for p in PrinterProfile::tous() {
            let attendu = if p.pages_simples { p.bleed_mm.dos } else { 0.0 };
            assert_eq!(pli_mm(p), attendu, "{}", p.id);
        }
        let cp = PrinterProfile::par_id("cloudprinter").unwrap();
        assert!(cp.pages_simples, "il relie page par page");
        assert_eq!(pli_mm(cp), 3.0, "leur gabarit dessine 3 mm sur les quatre bords");
        let gen = PrinterProfile::par_id("generique").unwrap();
        assert_eq!(pli_mm(gen), 0.0, "une planche n'a pas de bord au pli");
    }
}
