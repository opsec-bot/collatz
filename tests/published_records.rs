//! Path records checked against the published table.
//!
//! Values are from Bařina's table of path records below 2^71
//! (<https://pcbarina.fit.vutbr.cz/path-records.htm>), which gives peaks under
//! the shortcut map `T`. Under the standard map the peak is the `3n+1` value
//! one step earlier, exactly twice as large, except for the orbits of 1 and 2,
//! which never take an odd step.

use collatz::records::path_records;

/// Every path record below 2^32: (n, peak under the shortcut map).
const PUBLISHED: [(u128, u128); 45] = [
    (1, 1),
    (2, 2),
    (3, 8),
    (7, 26),
    (15, 80),
    (27, 4616),
    (255, 6560),
    (447, 19682),
    (639, 20762),
    (703, 125252),
    (1819, 638468),
    (4255, 3405068),
    (4591, 4076810),
    (9663, 13557212),
    (20895, 25071632),
    (26623, 53179010),
    (31911, 60506432),
    (60975, 296639576),
    (77671, 785412368),
    (113383, 1241055674),
    (138367, 1399161680),
    (159487, 8601188876),
    (270271, 12324038948),
    (665215, 26241642656),
    (704511, 28495741760),
    (1042431, 45119577824),
    (1212415, 69823368404),
    (1441407, 75814787186),
    (1875711, 77952174848),
    (1988859, 78457189112),
    (2643183, 95229909242),
    (2684647, 176308906472),
    (3041127, 311358950810),
    (3873535, 429277584788),
    (4637979, 659401147466),
    (5656191, 1206246808304),
    (6416623, 2399998472684),
    (6631675, 30171305459816),
    (19638399, 153148462601876),
    (38595583, 237318849425546),
    (80049391, 1092571914585050),
    (120080895, 1638950788059290),
    (210964383, 3202398580560632),
    (319804831, 707118223359971240),
    (1410123943, 3562942561397226080),
];

fn standard_peak(n: u128, shortcut_peak: u128) -> u128 {
    if n <= 2 {
        shortcut_peak
    } else {
        2 * shortcut_peak
    }
}

#[test]
fn matches_published_path_records() {
    // Debug builds are slow, so check the 41 records below 2^27 by default;
    // `cargo test --release -- --ignored` checks all 45 below 2^32.
    let limit = 1 << 27;
    let expected: Vec<(u128, u128)> = PUBLISHED
        .iter()
        .filter(|&&(n, _)| n < limit)
        .map(|&(n, peak)| (n, standard_peak(n, peak)))
        .collect();
    let found: Vec<(u128, u128)> = path_records(limit, 20, 4)
        .iter()
        .map(|r| (r.n, r.peak.to_u128().unwrap()))
        .collect();
    assert_eq!(found, expected);
}

#[test]
#[ignore = "takes a few seconds in release mode"]
fn matches_all_published_path_records_below_2_32() {
    let expected: Vec<(u128, u128)> = PUBLISHED
        .iter()
        .map(|&(n, peak)| (n, standard_peak(n, peak)))
        .collect();
    let found: Vec<(u128, u128)> = path_records(1 << 32, 24, 4)
        .iter()
        .map(|r| (r.n, r.peak.to_u128().unwrap()))
        .collect();
    assert_eq!(found, expected);
}
