//! The proposed spread caption: what the machine may say about a spread
//! whose caption is empty, and nothing else. Same doctrine as `places` and
//! `colophon`: never invent, never impose, stay silent in doubt. The
//! proposal only speaks when it adds to what the chapter line already says:
//! the spread's town when it diverges from the chapter's, the spread's day
//! when the chapter covers several. Never a path, never a coordinate: the
//! words come from the gazetteer and `date_fr`, nowhere else.

use crate::build::date_fr;
use crate::meta;
use crate::model::Album;
use crate::places;
use chrono::NaiveDate;
use std::path::Path;

/// The caption proposed for one spread, or None: silence is a full answer.
/// Reads the EXIF of the chapter's originals on the spot; there is no
/// analysis cache to reread (`.cache` only holds thumbnails), and a chapter's
/// worth of EXIF headers costs nothing next to opening one original.
pub fn proposition(album: &Album, planche: usize) -> Option<String> {
    let spread = album.spreads.get(planche)?;
    // A captioned spread needs nothing; a photo-less one has nothing to say.
    if spread.caption.is_some() || spread.slots.is_empty() {
        return None;
    }

    // The chapter is what the reader sees: it runs from the last captioned
    // spread up to the next one. Spreads before any caption still form a
    // segment, with an empty title.
    let start = album.spreads[..planche]
        .iter()
        .rposition(|s| s.caption.is_some())
        .unwrap_or(0);
    let end = album.spreads[planche + 1..]
        .iter()
        .position(|s| s.caption.is_some())
        .map(|i| planche + 1 + i)
        .unwrap_or(album.spreads.len());
    let titre = album.spreads[start].caption.as_deref().unwrap_or("");

    let root = Path::new(&album.root);
    let faits = |spreads: &[crate::model::Spread]| {
        let mut points = Vec::new();
        let mut jours: Vec<NaiveDate> = Vec::new();
        for s in spreads {
            for slot in &s.slots {
                let m = meta::read(&root.join(&slot.src));
                if let Some(gps) = m.gps {
                    points.push(gps);
                }
                if m.taken_reliable {
                    jours.push(m.taken.date());
                }
            }
        }
        (points, jours)
    };

    let (points_p, jours_p) = faits(std::slice::from_ref(spread));
    let (points_c, jours_c) = faits(&album.spreads[start..end]);

    let place_p = places::place_of(&points_p).map(|c| c.name);
    let place_c = places::place_of(&points_c).map(|c| c.name);
    let jour_p = jour_unique(&jours_p);
    let bornes_c = bornes(&jours_c);

    texte(place_p, place_c, titre, jour_p, bornes_c)
}

/// The day every trusted date of the spread agrees on. A spread shot across
/// midnight has no single day, and the proposal stays quiet about dates.
fn jour_unique(jours: &[NaiveDate]) -> Option<NaiveDate> {
    let first = *jours.first()?;
    jours.iter().all(|d| *d == first).then_some(first)
}

fn bornes(jours: &[NaiveDate]) -> Option<(NaiveDate, NaiveDate)> {
    let first = *jours.first()?;
    Some(
        jours
            .iter()
            .fold((first, first), |(lo, hi), d| (lo.min(*d), hi.max(*d))),
    )
}

/// The wording rule, pure so it tests without photos. A part is only worth
/// printing when it says something the chapter line does not:
/// - the town, when the spread's photos agree on one that is neither the
///   chapter's own town nor already written in its title (a title may name
///   the town while the chapter's photos disagree: the excursion broke the
///   agreement, not the stay);
/// - the day, when the spread has exactly one and the chapter spans several.
fn texte(
    place_planche: Option<&str>,
    place_chapitre: Option<&str>,
    titre_chapitre: &str,
    jour_planche: Option<NaiveDate>,
    jours_chapitre: Option<(NaiveDate, NaiveDate)>,
) -> Option<String> {
    let place = place_planche
        .filter(|p| place_chapitre != Some(*p) && !titre_chapitre.contains(*p));
    let jour = match (jour_planche, jours_chapitre) {
        (Some(d), Some((lo, hi))) if lo != hi => Some(d),
        _ => None,
    };
    match (place, jour) {
        (Some(p), Some(d)) => Some(format!("{p}, {}", date_fr(d, true))),
        (Some(p), None) => Some(p.to_string()),
        (None, Some(d)) => Some(date_fr(d, true)),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2013, 10, d).unwrap()
    }

    #[test]
    fn se_tait_quand_la_planche_dit_comme_son_chapitre() {
        // Même ville, chapitre d'un seul jour : rien à ajouter.
        assert_eq!(
            texte(Some("Calvi"), Some("Calvi"), "Calvi, 27 octobre 2013", Some(j(27)), Some((j(27), j(27)))),
            None
        );
    }

    #[test]
    fn nomme_l_excursion() {
        // Bonifacio dans un chapitre Porto-Vecchio qui court sur trois jours.
        assert_eq!(
            texte(
                Some("Bonifacio"),
                Some("Porto-Vecchio"),
                "Porto-Vecchio, 27 – 30 octobre 2013",
                Some(j(28)),
                Some((j(27), j(30))),
            ),
            Some("Bonifacio, 28 octobre 2013".into())
        );
    }

    #[test]
    fn date_seule_quand_le_chapitre_couvre_plusieurs_jours() {
        assert_eq!(
            texte(Some("Calvi"), Some("Calvi"), "Calvi", Some(j(29)), Some((j(27), j(31)))),
            Some("29 octobre 2013".into())
        );
    }

    #[test]
    fn le_titre_qui_nomme_deja_la_ville_la_fait_taire() {
        // L'excursion a cassé l'accord GPS du chapitre (place_chapitre None),
        // mais le titre imprimé dit déjà « Porto-Vecchio » : le répéter est
        // du bruit.
        assert_eq!(
            texte(Some("Porto-Vecchio"), None, "Porto-Vecchio, octobre 2013", Some(j(27)), Some((j(27), j(27)))),
            None
        );
    }

    #[test]
    fn ville_seule_quand_le_chapitre_tient_en_un_jour() {
        assert_eq!(
            texte(Some("Bonifacio"), Some("Calvi"), "Calvi, 27 octobre 2013", Some(j(27)), Some((j(27), j(27)))),
            Some("Bonifacio".into())
        );
    }

    #[test]
    fn silence_sans_gps_ni_pluralite_de_jours() {
        assert_eq!(texte(None, None, "", Some(j(27)), Some((j(27), j(27)))), None);
        assert_eq!(texte(None, None, "", None, None), None);
    }

    #[test]
    fn une_planche_a_cheval_sur_minuit_tait_sa_date() {
        assert_eq!(jour_unique(&[j(27), j(28)]), None);
        assert_eq!(jour_unique(&[j(27), j(27)]), Some(j(27)));
        assert_eq!(jour_unique(&[]), None);
    }

    /// Les interdits de `colophon.rs` : jamais un chemin, jamais une
    /// coordonnée. La chaîne sort du gazetteer et de `date_fr`, le test
    /// verrouille la forme.
    #[test]
    fn jamais_un_chemin_ni_une_coordonnee() {
        let s = texte(
            Some("Bonifacio"),
            Some("Calvi"),
            "Calvi",
            Some(j(28)),
            Some((j(27), j(30))),
        )
        .unwrap();
        assert!(!s.contains('/') && !s.contains('\\'));
        assert!(!s.contains('.'), "une coordonnée décimale n'a rien à faire ici : {s}");
    }

    /// La plomberie entière se tait sur un album dont les originaux
    /// manquent : des meta sans fiabilité ne produisent rien.
    #[test]
    fn silence_quand_les_originaux_manquent() {
        let mut album = Album::new(
            "t",
            Path::new("/nulle/part"),
            crate::model::Size { w: 210.0, h: 210.0 },
        );
        album.spreads.push(crate::model::Spread {
            template: "duo".into(),
            slots: vec![
                crate::model::Slot::new("a.jpg".into(), [0.5, 0.42]),
                crate::model::Slot::new("b.jpg".into(), [0.5, 0.42]),
            ],
            caption: None,
            text: None,
            edited: false,
            locked: false,
            objets: Vec::new(),
        });
        assert_eq!(proposition(&album, 0), None);
        // Hors bornes, planche légendée : mêmes silences.
        assert_eq!(proposition(&album, 7), None);
    }
}
