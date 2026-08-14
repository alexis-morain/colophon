//! Where a chapter happened, from the GPS the camera wrote.
//!
//! A chapter titled « 27 – 29 octobre 2013 » says when. « Porto-Vecchio,
//! 27 – 29 octobre » says where, which is what anybody actually remembers a
//! trip by. The coordinates are already in the files; all that was missing
//! was a gazetteer to turn them into a name.
//!
//! GeoNames `cities5000`, under CC BY 4.0 (see
//! `assets/cities5000-LICENSE.md`, which carries the attribution this
//! licence requires). Every populated place above five thousand
//! inhabitants: fine enough to name a Corsican village, coarse enough that
//! the nearest match is a place a reader has a chance of recognising.
//!
//! Nothing here decides anything on its own. A chapter whose photos carry no
//! GPS, or that sits far from any town, keeps its dates and says nothing
//! about a place it cannot name. Same rule as everywhere else in the engine:
//! a heuristic that is unsure stays quiet.

use std::sync::LazyLock;

/// The gazetteer, sorted by latitude so a lookup only reads a band of it.
const CITIES: &str = include_str!("../assets/cities5000.tsv");

/// How far a town may be and still name the place. Beyond this the chapter
/// happened somewhere else: a village 40 km up the coast is a wrong answer,
/// not an approximate one.
pub const MAX_KM: f64 = 30.0;

/// Share of a chapter's located photos that must agree on the same town for
/// it to become the title. A day trip that crosses three villages is not
/// « Bonifacio », it is just a day.
const AGREEMENT: f64 = 0.5;

#[derive(Debug, Clone, Copy)]
pub struct City {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
    /// ISO country code, for telling two Saint-Pierre apart in a log.
    pub country: &'static str,
    pub population: u32,
}

/// Parsed once. 70 000 lines of tab-separated text, a few milliseconds, and
/// only when an album actually carries coordinates.
static GAZETTEER: LazyLock<Vec<City>> = LazyLock::new(|| {
    CITIES
        .lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let name = f.next()?;
            let lat = f.next()?.parse().ok()?;
            let lon = f.next()?.parse().ok()?;
            let country = f.next()?;
            let population = f.next()?.parse().ok()?;
            Some(City { name, lat, lon, country, population })
        })
        .collect()
});

pub fn gazetteer() -> &'static [City] {
    &GAZETTEER
}

/// Kilometres between two coordinates, flat-earth style. Over the tens of
/// kilometres this module cares about, the error against a great circle is
/// centimetres, and the comparison runs seventy thousand times per photo.
pub fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const KM_PER_DEGREE: f64 = 111.195;
    let dlat = lat2 - lat1;
    // A degree of longitude shrinks towards the poles.
    let dlon = (lon2 - lon1) * ((lat1 + lat2) / 2.0).to_radians().cos();
    (dlat * dlat + dlon * dlon).sqrt() * KM_PER_DEGREE
}

/// How far a town's own name reasonably reaches, in kilometres, from its
/// population. A city of two million covers about ten; a village of five
/// thousand, a few hundred metres.
///
/// This exists because the gazetteer holds districts as ordinary towns:
/// « Paris 16 Passy » and « Paris 04 Hôtel-de-Ville » are entries of their
/// own, and a photo taken in the sixteenth really is nearer to the first
/// than to the point GeoNames calls Paris. Nobody names a holiday after an
/// arrondissement. Measuring each candidate against its own reach picks the
/// city there, and still picks Nanterre over Paris eleven kilometres out,
/// where the suburb is the true answer.
fn reach_km(population: u32) -> f64 {
    f64::from(population).sqrt() / 150.0
}

/// The town a point belongs to, with its distance, or `None` past
/// [`MAX_KM`]. Not simply the nearest: the one whose own reach the point
/// falls furthest inside.
pub fn nearest(lat: f64, lon: f64) -> Option<(&'static City, f64)> {
    let cities = gazetteer();
    // The list is sorted by latitude: only the band that could hold a match
    // is worth measuring, which turns 64 000 comparisons into a few hundred.
    let margin = MAX_KM / 111.0;
    let lo = cities.partition_point(|c| c.lat < lat - margin);
    let hi = cities.partition_point(|c| c.lat < lat + margin);

    let mut best: Option<(&'static City, f64, f64)> = None;
    for c in &cities[lo..hi] {
        let d = distance_km(lat, lon, c.lat, c.lon);
        if d > MAX_KM {
            continue;
        }
        let score = d - reach_km(c.population);
        let better = match best {
            None => true,
            // Ties by name, so the answer never depends on the file's order.
            Some((bc, _, bs)) => score < bs || (score == bs && c.name < bc.name),
        };
        if better {
            best = Some((c, d, score));
        }
    }
    best.map(|(c, d, _)| (c, d))
}

/// The place a set of photos was taken, or `None` when they disagree, when
/// too few carry coordinates, or when there is simply no town nearby.
///
/// Every located photo votes for its own nearest town rather than the whole
/// chapter voting once on an average position: the midpoint of two ends of a
/// bay is the sea, and the sea has no name in this file.
pub fn place_of(points: &[(f64, f64)]) -> Option<&'static City> {
    if points.is_empty() {
        return None;
    }
    let mut votes: Vec<(&'static City, usize)> = Vec::new();
    for (lat, lon) in points {
        let Some((city, _)) = nearest(*lat, *lon) else { continue };
        match votes.iter_mut().find(|(c, _)| std::ptr::eq(*c, city)) {
            Some((_, n)) => *n += 1,
            None => votes.push((city, 1)),
        }
    }
    // Ties go to the more populous town: between a hamlet and the town next
    // door, the one a reader places on a map wins.
    let (winner, count) = votes
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(a.0.population.cmp(&b.0.population)))?;
    (count as f64 >= points.len() as f64 * AGREEMENT).then_some(winner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gazetteer_loads_whole() {
        let g = gazetteer();
        assert!(g.len() > 60_000, "{} villes", g.len());
        // Sorted by latitude, which is what the band lookup relies on.
        assert!(g.windows(2).all(|w| w[0].lat <= w[1].lat), "jeu non trié");
        assert!(g.iter().all(|c| !c.name.is_empty()));
        assert!(g.iter().all(|c| (-90.0..=90.0).contains(&c.lat)));
        assert!(g.iter().all(|c| (-180.0..=180.0).contains(&c.lon)));
    }

    /// Real coordinates from the Corsican test set land on the right town.
    #[test]
    fn a_known_point_finds_its_town() {
        let (c, d) = nearest(41.5912, 9.2795).expect("Porto-Vecchio trouvée");
        assert_eq!(c.name, "Porto-Vecchio", "{c:?}");
        assert!(d < 1.0, "{d} km");

        let (c, _) = nearest(48.8566, 2.3522).unwrap();
        assert_eq!(c.name, "Paris");
        assert_eq!(c.country, "FR");
    }

    /// The middle of the Mediterranean is not a place. A gazetteer that
    /// always answers would title a chapter after a town nobody visited.
    #[test]
    fn the_open_sea_has_no_name() {
        assert!(nearest(39.0, 6.0).is_none());
        assert!(nearest(-40.0, -140.0).is_none(), "Pacifique sud");
    }

    /// A stay in one place is named; a chapter scattered across a region is
    /// not, because no town holds half its photos.
    #[test]
    fn only_an_agreeing_chapter_gets_a_name() {
        let porto_vecchio = vec![(41.591, 9.279), (41.594, 9.283), (41.588, 9.271)];
        assert_eq!(place_of(&porto_vecchio).unwrap().name, "Porto-Vecchio");

        // One photo in Corsica, one in Paris, one in Marseille: nobody wins.
        let scattered = vec![(41.591, 9.279), (48.8566, 2.3522), (43.2965, 5.3698)];
        assert!(place_of(&scattered).is_none());

        // And a chapter with no coordinates at all stays silent.
        assert!(place_of(&[]).is_none());
    }

    /// A single located photo among many unlocated ones still names the
    /// chapter: the caller only passes the photos that carry coordinates,
    /// and one real coordinate beats none.
    #[test]
    fn one_point_is_enough_to_name_a_place() {
        assert_eq!(place_of(&[(41.591, 9.279)]).unwrap().name, "Porto-Vecchio");
    }

    /// The band lookup reads the same candidates a full sweep would. The
    /// optimisation is exactly the kind that silently drops the right town.
    #[test]
    fn the_band_lookup_agrees_with_the_whole_list() {
        for (lat, lon) in [(41.591, 9.279), (48.8566, 2.3522), (-33.86, 151.2), (64.14, -21.94)] {
            let mut brute: Vec<&City> = gazetteer()
                .iter()
                .filter(|c| distance_km(lat, lon, c.lat, c.lon) <= MAX_KM)
                .collect();
            brute.sort_by_key(|c| c.name);
            let band = {
                let margin = MAX_KM / 111.0;
                let g = gazetteer();
                let lo = g.partition_point(|c| c.lat < lat - margin);
                let hi = g.partition_point(|c| c.lat < lat + margin);
                let mut v: Vec<&City> = g[lo..hi]
                    .iter()
                    .filter(|c| distance_km(lat, lon, c.lat, c.lon) <= MAX_KM)
                    .collect();
                v.sort_by_key(|c| c.name);
                v
            };
            assert_eq!(
                band.iter().map(|c| c.name).collect::<Vec<_>>(),
                brute.iter().map(|c| c.name).collect::<Vec<_>>(),
                "{lat},{lon}"
            );
        }
    }

    /// A city beats its own districts, which the gazetteer lists as towns.
    /// Both points below are nearer to an arrondissement's centre than to
    /// the one GeoNames calls Paris.
    #[test]
    fn a_city_wins_over_its_own_districts() {
        for (lat, lon) in [(48.8566, 2.3522), (48.8637, 2.2769), (48.8331, 2.3264)] {
            let (c, _) = nearest(lat, lon).unwrap();
            assert_eq!(c.name, "Paris", "{lat},{lon} → {c:?}");
        }
    }

    /// But a suburb is not the city: eleven kilometres out, the town whose
    /// name people actually use wins, and forty kilometres out Paris claims
    /// nothing at all.
    #[test]
    fn a_suburb_keeps_its_own_name() {
        assert_eq!(nearest(48.8924, 2.2069).unwrap().0.name, "Nanterre");
        assert_eq!(nearest(48.9474, 2.2482).unwrap().0.name, "Argenteuil");
        assert!(nearest(48.35, 1.6).is_none_or(|(c, _)| c.name != "Paris"));
    }
}
