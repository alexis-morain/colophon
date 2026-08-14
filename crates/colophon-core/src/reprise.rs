//! The reprise metric: how much of the machine's proposal a human had to
//! correct by hand. It is the number the GO/NO-GO milestone hangs on — a
//! draft nobody rewrites is a draft that ships.
//!
//! `album.origin.json` is the composer's untouched proposal, written once at
//! the first build and never rewritten. `album.json` is what the album became
//! after editing. The distance between the two is the metric.
//!
//! The measure is a content diff, not the `edited` flag. The flag says a
//! spread was touched; the diff says whether the touch changed anything, and
//! **which class** of correction it was. That last part is the point: a
//! correction class that keeps coming back is a linter counter waiting to be
//! written.

use crate::model::{Album, Cover, Slot, Spread};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Below this share of corrected spreads, the automatic draft carries the
/// album on its own.
pub const BON: f64 = 0.10;
/// Above this share, the composer is not doing its job: the human is the
/// one composing.
pub const REDHIBITOIRE: f64 = 0.30;

/// Crops closer than this read as the same framing.
const CROP_EPSILON: f64 = 1e-6;

/// One class of hand correction. The wording matches the interface, not the
/// engine: these strings end up in front of a human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Classe {
    Gabarit,
    Photos,
    Recadrage,
    Legende,
    Texte,
    Ordre,
    Insertion,
    Suppression,
}

impl Classe {
    pub fn nom(self) -> &'static str {
        match self {
            Classe::Gabarit => "gabarit",
            Classe::Photos => "photos",
            Classe::Recadrage => "recadrage",
            Classe::Legende => "legende",
            Classe::Texte => "texte",
            Classe::Ordre => "ordre",
            Classe::Insertion => "insertion",
            Classe::Suppression => "suppression",
        }
    }

    /// The counter that would have caught this class before the human did.
    /// Empty when no counter could: an insertion is a taste call, not a
    /// defect.
    fn compteur_parent(self) -> &'static str {
        match self {
            Classe::Gabarit => "repetition_gabarit, orientation_trahie",
            Classe::Photos => "doublon_planche, sous_resolution",
            Classe::Recadrage => "visage_coupe",
            Classe::Legende => "legende_manquante, legende_sur_photo",
            Classe::Ordre => "chapitre_orphelin, rythme_plat",
            Classe::Texte | Classe::Insertion | Classe::Suppression => "",
        }
    }
}

impl Serialize for Classe {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

/// One corrected spread, named the way the ruler names it.
#[derive(Debug, Serialize)]
pub struct Reprise {
    /// 1-based index in the edited album. A deleted spread has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planche: Option<usize>,
    /// 1-based index in the composer's proposal. An inserted spread has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planche_origine: Option<usize>,
    pub classes: Vec<Classe>,
}

/// How often each class of correction came up, and which counter should have
/// caught it. Sorted by count: the top line is the next counter to write.
#[derive(Debug, Serialize)]
pub struct ClasseCount {
    pub classe: Classe,
    pub planches: usize,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub compteur_parent: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RepriseReport {
    pub album: String,
    pub planches_origine: usize,
    pub planches_actuelles: usize,
    /// Spreads carrying at least one correction, deletions and insertions
    /// included.
    pub planches_touchees: usize,
    /// `planches_touchees` over the composer's own spread count: the share of
    /// its proposal the machine got wrong.
    pub part: f64,
    pub pourcentage: f64,
    /// `bon` under 10 %, `à surveiller` up to 30 %, `rédhibitoire` past it.
    pub verdict: &'static str,
    /// False past the rédhibitoire threshold, so the shell can exit non-zero.
    pub ok: bool,
    /// What changed on the cover. Kept out of the percentage: a cover is one
    /// object, not a spread, and everyone retitles their own album.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub couverture: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub classes: Vec<ClasseCount>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<Reprise>,
}

/// Measure one album folder against the proposal it started from.
pub fn reprise(dir: &Path) -> Result<RepriseReport> {
    let album: Album = read_album(&dir.join("album.json"))?;
    let origine_path = dir.join("album.origin.json");
    if !origine_path.exists() {
        anyhow::bail!(
            "{} absent : cet album a été composé avant la mesure de reprise. \
             Recomposez le dossier pour en poser la référence.",
            origine_path.display()
        );
    }
    let origine: Album = read_album(&origine_path)?;
    Ok(compare(&album.title, &origine, &album))
}

fn read_album(path: &Path) -> Result<Album> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("lecture de {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} illisible", path.display()))
}

/// The whole metric, on two albums already in memory. Split out so the tests
/// need no folder on disk.
pub fn compare(titre: &str, origine: &Album, actuel: &Album) -> RepriseReport {
    let pairs = match_spreads(&origine.spreads, &actuel.spreads);

    // A spread that moved relative to its neighbours is a correction of the
    // narrative order. Only the spreads outside the longest increasing run
    // count: keeping the whole book and moving one spread must cost one, not
    // the length of the book.
    let mut ordered: Vec<(usize, usize)> = pairs.clone();
    ordered.sort_by_key(|(_, cur)| *cur);
    let kept = longest_increasing(&ordered.iter().map(|(o, _)| *o).collect::<Vec<_>>());
    let moved: HashSet<usize> = ordered
        .iter()
        .enumerate()
        .filter(|(i, _)| !kept.contains(i))
        .map(|(_, (o, _))| *o)
        .collect();

    let mut details: Vec<Reprise> = Vec::new();
    let matched_origin: HashSet<usize> = pairs.iter().map(|(o, _)| *o).collect();
    let matched_current: HashSet<usize> = pairs.iter().map(|(_, c)| *c).collect();
    // Spreads wearing the badge whose content came back to the proposal.
    let mut badge_sans_effet = 0usize;

    for (o, c) in &pairs {
        let mut classes = classify(&origine.spreads[*o], &actuel.spreads[*c]);
        if classes.is_empty() && actuel.spreads[*c].edited {
            badge_sans_effet += 1;
        }
        if moved.contains(o) {
            classes.push(Classe::Ordre);
        }
        if !classes.is_empty() {
            classes.sort();
            classes.dedup();
            details.push(Reprise {
                planche: Some(c + 1),
                planche_origine: Some(o + 1),
                classes,
            });
        }
    }
    for c in 0..actuel.spreads.len() {
        if !matched_current.contains(&c) {
            details.push(Reprise {
                planche: Some(c + 1),
                planche_origine: None,
                classes: vec![Classe::Insertion],
            });
        }
    }
    for o in 0..origine.spreads.len() {
        if !matched_origin.contains(&o) {
            details.push(Reprise {
                planche: None,
                planche_origine: Some(o + 1),
                classes: vec![Classe::Suppression],
            });
        }
    }
    details.sort_by_key(|d| (d.planche.unwrap_or(usize::MAX), d.planche_origine));

    let mut classes: Vec<ClasseCount> = {
        let mut tally: Vec<(Classe, usize)> = Vec::new();
        for d in &details {
            for c in &d.classes {
                match tally.iter_mut().find(|(k, _)| k == c) {
                    Some((_, n)) => *n += 1,
                    None => tally.push((*c, 1)),
                }
            }
        }
        tally
            .into_iter()
            .map(|(classe, planches)| ClasseCount {
                classe,
                planches,
                compteur_parent: classe.compteur_parent(),
            })
            .collect()
    };
    classes.sort_by(|a, b| b.planches.cmp(&a.planches).then(a.classe.cmp(&b.classe)));

    let touchees = details.len();
    let base = origine.spreads.len();
    let part = if base == 0 { 0.0 } else { touchees as f64 / base as f64 };
    let verdict = if part < BON {
        "bon"
    } else if part <= REDHIBITOIRE {
        "à surveiller"
    } else {
        "rédhibitoire"
    };

    let mut notes = Vec::new();
    // The badge and the diff can disagree, and the diff wins: a spread edited
    // then put back carries the badge with nothing changed. Worth saying out
    // loud, because the interface shows the badge and not the diff.
    if badge_sans_effet > 0 {
        notes.push(format!(
            "{badge_sans_effet} planches portent le badge « éditée à la main » sans différer de \
             la proposition : du travail annulé ou refait à l'identique, non compté"
        ));
    }
    let verrous = actuel.spreads.iter().filter(|s| s.locked && !s.edited).count();
    if verrous > 0 {
        notes.push(format!(
            "{verrous} planches épinglées sans modification : approuvées telles quelles, \
             elles ne comptent pas comme une reprise"
        ));
    }

    RepriseReport {
        album: titre.to_string(),
        planches_origine: base,
        planches_actuelles: actuel.spreads.len(),
        planches_touchees: touchees,
        part,
        pourcentage: (part * 1000.0).round() / 10.0,
        verdict,
        ok: verdict != "rédhibitoire",
        couverture: cover_diff(origine.cover.as_ref(), actuel.cover.as_ref()),
        notes,
        classes,
        details,
    }
}

/// Pair each proposal spread with what it became. Photos are a spread's
/// identity: the pair sharing the most of them is the same spread, however
/// much its template or its framing moved. Text and empty spreads carry no
/// photo, so they fall back to the leftovers, matched in reading order.
fn match_spreads(origine: &[Spread], actuel: &[Spread]) -> Vec<(usize, usize)> {
    let mut scored: Vec<(f64, usize, usize)> = Vec::new();
    for (o, so) in origine.iter().enumerate() {
        let a: HashSet<&str> = srcs(so);
        if a.is_empty() {
            continue;
        }
        for (c, sc) in actuel.iter().enumerate() {
            let b: HashSet<&str> = srcs(sc);
            if b.is_empty() {
                continue;
            }
            let inter = a.intersection(&b).count();
            if inter == 0 {
                continue;
            }
            let union = a.union(&b).count();
            scored.push((inter as f64 / union as f64, o, c));
        }
    }
    // Best overlap first, and ties resolved by proximity so a photo appearing
    // twice cannot drag a match across the book.
    scored.sort_by(|x, y| {
        y.0.partial_cmp(&x.0)
            .unwrap()
            .then_with(|| x.1.abs_diff(x.2).cmp(&y.1.abs_diff(y.2)))
    });

    let mut used_o: HashSet<usize> = HashSet::new();
    let mut used_c: HashSet<usize> = HashSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (_, o, c) in scored {
        if used_o.insert(o) {
            if used_c.insert(c) {
                pairs.push((o, c));
            } else {
                used_o.remove(&o);
            }
        }
    }

    // Photoless spreads left over, paired in order: two text spreads that kept
    // their place stay the same spread even after a full rewrite.
    let free_o: Vec<usize> = (0..origine.len())
        .filter(|o| !used_o.contains(o) && srcs(&origine[*o]).is_empty())
        .collect();
    let free_c: Vec<usize> = (0..actuel.len())
        .filter(|c| !used_c.contains(c) && srcs(&actuel[*c]).is_empty())
        .collect();
    for (o, c) in free_o.into_iter().zip(free_c) {
        pairs.push((o, c));
    }

    pairs.sort();
    pairs
}

fn srcs(s: &Spread) -> HashSet<&str> {
    s.slots.iter().map(|sl| sl.src.as_str()).collect()
}

/// Everything that changed between a spread and what it became.
fn classify(origine: &Spread, actuel: &Spread) -> Vec<Classe> {
    let mut out = Vec::new();
    if origine.template != actuel.template {
        out.push(Classe::Gabarit);
    }
    if srcs(origine) != srcs(actuel) {
        out.push(Classe::Photos);
    }
    // Framing is only comparable on the photos both versions hold.
    for slot in &actuel.slots {
        if let Some(before) = origine.slots.iter().find(|s| s.src == slot.src) {
            if !same_crop(before, slot) {
                out.push(Classe::Recadrage);
                break;
            }
        }
    }
    let caption_moved = origine.caption != actuel.caption
        || actuel.slots.iter().any(|slot| {
            origine
                .slots
                .iter()
                .find(|s| s.src == slot.src)
                .is_some_and(|before| before.caption != slot.caption)
        });
    if caption_moved {
        out.push(Classe::Legende);
    }
    if origine.text != actuel.text {
        out.push(Classe::Texte);
    }
    out
}

fn same_crop(a: &Slot, b: &Slot) -> bool {
    (a.focal[0] - b.focal[0]).abs() < CROP_EPSILON
        && (a.focal[1] - b.focal[1]).abs() < CROP_EPSILON
        && (a.zoom - b.zoom).abs() < CROP_EPSILON
}

fn cover_diff(origine: Option<&Cover>, actuel: Option<&Cover>) -> Vec<&'static str> {
    match (origine, actuel) {
        (None, None) => Vec::new(),
        (None, Some(_)) => vec!["couverture ajoutée"],
        (Some(_), None) => vec!["couverture retirée"],
        (Some(a), Some(b)) => {
            let mut out = Vec::new();
            if a.title != b.title {
                out.push("titre");
            }
            if a.subtitle != b.subtitle {
                out.push("sous-titre");
            }
            let photo_moved = match (&a.photo, &b.photo) {
                (Some(x), Some(y)) => x.src != y.src || !same_crop(x, y),
                (None, None) => false,
                _ => true,
            };
            if photo_moved {
                out.push("photo");
            }
            if a.back_text != b.back_text {
                out.push("quatrième");
            }
            out
        }
    }
}

/// Indices of a longest increasing subsequence. Patience sorting, so a book
/// whose spreads mostly kept their order costs what it should.
fn longest_increasing(v: &[usize]) -> HashSet<usize> {
    if v.is_empty() {
        return HashSet::new();
    }
    // `tails[k]` = index in `v` of the smallest tail of an increasing run of
    // length k+1; `prev` threads each element back to its predecessor.
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; v.len()];
    for i in 0..v.len() {
        let pos = tails.partition_point(|&t| v[t] < v[i]);
        if pos > 0 {
            prev[i] = Some(tails[pos - 1]);
        }
        if pos == tails.len() {
            tails.push(i);
        } else {
            tails[pos] = i;
        }
    }
    let mut out = HashSet::new();
    let mut cur = tails.last().copied();
    while let Some(i) = cur {
        out.insert(i);
        cur = prev[i];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Size;

    fn album(spreads: Vec<Spread>) -> Album {
        let mut a = Album::new("t", Path::new("/p"), Size { w: 210.0, h: 210.0 });
        a.spreads = spreads;
        a
    }

    fn spread(template: &str, srcs: &[&str]) -> Spread {
        Spread {
            template: template.into(),
            slots: srcs
                .iter()
                .map(|s| Slot::new((*s).into(), [0.5, 0.42]))
                .collect(),
            caption: None,
            text: None,
            edited: false,
            locked: false,
        }
    }

    /// An album nobody touched costs nothing.
    #[test]
    fn untouched_album_scores_zero() {
        let a = album(vec![spread("duo", &["a.jpg", "b.jpg"]), spread("solo", &["c.jpg"])]);
        let b = album(vec![spread("duo", &["a.jpg", "b.jpg"]), spread("solo", &["c.jpg"])]);
        let r = compare("t", &a, &b);
        assert_eq!(r.planches_touchees, 0);
        assert_eq!(r.pourcentage, 0.0);
        assert_eq!(r.verdict, "bon");
        assert!(r.ok);
    }

    /// Each class is named for what it is, and one spread carrying two
    /// corrections is still one corrected spread.
    #[test]
    fn classes_name_the_correction() {
        let a = album(vec![spread("duo", &["a.jpg", "b.jpg"])]);
        let mut edited = spread("trio", &["a.jpg", "b.jpg", "c.jpg"]);
        edited.slots[0].zoom = 1.6;
        edited.edited = true;
        let b = album(vec![edited]);
        let r = compare("t", &a, &b);
        assert_eq!(r.planches_touchees, 1);
        let classes = &r.details[0].classes;
        assert!(classes.contains(&Classe::Gabarit));
        assert!(classes.contains(&Classe::Photos));
        assert!(classes.contains(&Classe::Recadrage));
    }

    /// A recrop is found even when the spread kept everything else, and it
    /// points at the counter that should have caught it.
    #[test]
    fn recrop_points_at_the_face_counter() {
        let a = album(vec![spread("solo", &["a.jpg"])]);
        let mut moved = spread("solo", &["a.jpg"]);
        moved.slots[0].focal = [0.3, 0.2];
        let b = album(vec![moved]);
        let r = compare("t", &a, &b);
        assert_eq!(r.details[0].classes, vec![Classe::Recadrage]);
        assert_eq!(r.classes[0].compteur_parent, "visage_coupe");
    }

    /// Moving one spread costs one, not the length of the book: the other
    /// spreads kept their order even though their index shifted.
    #[test]
    fn moving_one_spread_costs_one() {
        let a = album(vec![
            spread("solo", &["a.jpg"]),
            spread("solo", &["b.jpg"]),
            spread("solo", &["c.jpg"]),
            spread("solo", &["d.jpg"]),
        ]);
        // d moved to the front, a b c kept their relative order.
        let b = album(vec![
            spread("solo", &["d.jpg"]),
            spread("solo", &["a.jpg"]),
            spread("solo", &["b.jpg"]),
            spread("solo", &["c.jpg"]),
        ]);
        let r = compare("t", &a, &b);
        assert_eq!(r.planches_touchees, 1);
        assert_eq!(r.details[0].classes, vec![Classe::Ordre]);
    }

    /// Insertions and deletions are corrections too, each counted once.
    #[test]
    fn insertion_and_deletion_count() {
        let a = album(vec![spread("solo", &["a.jpg"]), spread("solo", &["b.jpg"])]);
        let b = album(vec![spread("solo", &["a.jpg"]), spread("solo", &["z.jpg"])]);
        let r = compare("t", &a, &b);
        assert_eq!(r.planches_touchees, 2);
        let kinds: HashSet<Classe> =
            r.details.iter().flat_map(|d| d.classes.clone()).collect();
        assert!(kinds.contains(&Classe::Insertion));
        assert!(kinds.contains(&Classe::Suppression));
    }

    /// The thresholds are the GO/NO-GO wording, and the verdict follows the
    /// share rather than the count.
    #[test]
    fn thresholds_follow_the_milestone() {
        let base: Vec<Spread> = (0..20)
            .map(|i| spread("solo", &[Box::leak(format!("{i}.jpg").into_boxed_str())]))
            .collect();
        let a = album(base.clone());

        // 1 spread over 20 = 5 %, good enough to ship.
        let mut one = base.clone();
        one[0].slots[0].zoom = 2.0;
        let r = compare("t", &a, &album(one));
        assert_eq!(r.pourcentage, 5.0);
        assert_eq!(r.verdict, "bon");

        // 4 over 20 = 20 %, worth watching but not fatal.
        let mut four = base.clone();
        for s in four.iter_mut().take(4) {
            s.slots[0].zoom = 2.0;
        }
        let r = compare("t", &a, &album(four));
        assert_eq!(r.verdict, "à surveiller");
        assert!(r.ok);

        // 8 over 20 = 40 %: the human is the one composing.
        let mut eight = base.clone();
        for s in eight.iter_mut().take(8) {
            s.slots[0].zoom = 2.0;
        }
        let r = compare("t", &a, &album(eight));
        assert_eq!(r.verdict, "rédhibitoire");
        assert!(!r.ok);
    }

    /// The badge is not the measure: a spread edited then put back costs
    /// nothing, and the report says why the two numbers disagree.
    #[test]
    fn undone_edit_costs_nothing_but_is_reported() {
        let a = album(vec![spread("solo", &["a.jpg"])]);
        let mut back = spread("solo", &["a.jpg"]);
        back.edited = true;
        let r = compare("t", &a, &album(vec![back]));
        assert_eq!(r.planches_touchees, 0);
        assert!(r.notes.iter().any(|n| n.contains("badge")));
    }

    /// The badge note counts badges without effect, one by one. Counting
    /// totals instead would let a badge that changed nothing cancel out a
    /// change that carries no badge, and report a clean album.
    #[test]
    fn badge_note_does_not_cancel_out() {
        let a = album(vec![
            spread("solo", &["a.jpg"]),
            spread("solo", &["b.jpg"]),
            spread("solo", &["c.jpg"]),
        ]);
        let mut undone = spread("solo", &["a.jpg"]);
        undone.edited = true; // badge, contenu identique
        let b = album(vec![
            undone,
            spread("solo", &["c.jpg"]), // déplacée, sans badge
            spread("solo", &["b.jpg"]),
        ]);
        let r = compare("t", &a, &b);
        assert_eq!(r.planches_touchees, 1);
        assert_eq!(r.details[0].classes, vec![Classe::Ordre]);
        assert!(r.notes.iter().any(|n| n.starts_with("1 planches")));
    }

    /// A locked but untouched spread is an approval, not a correction.
    #[test]
    fn lock_alone_is_not_a_correction() {
        let a = album(vec![spread("solo", &["a.jpg"])]);
        let mut pinned = spread("solo", &["a.jpg"]);
        pinned.locked = true;
        let r = compare("t", &a, &album(vec![pinned]));
        assert_eq!(r.planches_touchees, 0);
        assert!(r.notes.iter().any(|n| n.contains("épinglées")));
    }

    /// Retitling the book is not a composition defect, so it stays out of
    /// the percentage and gets its own line.
    #[test]
    fn cover_is_reported_apart() {
        let mut a = album(vec![spread("solo", &["a.jpg"])]);
        a.cover = Some(Cover {
            title: "Corse".into(),
            subtitle: String::new(),
            photo: None,
            back_text: String::new(),
        });
        let mut b = album(vec![spread("solo", &["a.jpg"])]);
        b.cover = Some(Cover {
            title: "Corse 2013".into(),
            subtitle: "juillet".into(),
            photo: None,
            back_text: String::new(),
        });
        let r = compare("t", &a, &b);
        assert_eq!(r.planches_touchees, 0);
        assert_eq!(r.couverture, vec!["titre", "sous-titre"]);
    }
}
