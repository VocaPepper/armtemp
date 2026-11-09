//! Resolve the OS's raw per-core EfficiencyClass values into the two
//! `CoreKind` tiers the UI understands. Kept free of Win32 imports so the
//! classification rules can be unit-tested on any host.

use crate::sensors::types::CoreKind;

/// Maps each core's raw EfficiencyClass byte (in core-index order) to a P/E
/// `CoreKind`: cores in the fastest class become `Performance`, every other
/// class becomes `Efficiency`.
///
/// The OS's numeric class is not consistently ordered across ARM64 firmware.
/// On Snapdragon X the highest class is the faster one, but the Snapdragon
/// 855 reports class 0 for its 2.84/2.42 GHz Kryo Gold cluster and class 1
/// for its 1.78 GHz Silver cluster — treating the highest class as
/// Performance would badge the little cores "P" and the big cores "E". The
/// per-core `~MHz` values are a real, independent signal, so when they line
/// up with the class list they decide which class is actually faster. Without
/// usable clocks we fall back to the historical highest-class rule.
pub fn classes_to_kinds(classes: &[u8], per_core_mhz: &[u32]) -> Vec<CoreKind> {
    let max = classes.iter().copied().max().unwrap_or(0);
    let performance_class = fastest_class_by_clock(classes, per_core_mhz).unwrap_or(max);
    classes
        .iter()
        .map(|&c| {
            if c == performance_class {
                CoreKind::Performance
            } else {
                CoreKind::Efficiency
            }
        })
        .collect()
}

/// Return the EfficiencyClass whose members have the highest average nominal
/// `~MHz`. `None` when the clock list doesn't align with the class list or
/// contains no usable values, so callers can keep the class-number fallback.
fn fastest_class_by_clock(classes: &[u8], per_core_mhz: &[u32]) -> Option<u8> {
    if classes.is_empty() || classes.len() != per_core_mhz.len() {
        return None;
    }
    let mut best: Option<(u8, f64)> = None;
    for (i, &class) in classes.iter().enumerate() {
        if classes[..i].contains(&class) {
            continue; // already averaged this class
        }
        let mut sum = 0u64;
        let mut count = 0u32;
        for (j, &other) in classes.iter().enumerate() {
            let mhz = per_core_mhz[j];
            if other == class && mhz > 0 {
                sum += mhz as u64;
                count += 1;
            }
        }
        if count == 0 {
            continue;
        }
        let average = sum as f64 / count as f64;
        if best.map_or(true, |(_, previous)| average > previous) {
            best = Some((class, average));
        }
    }
    best.map(|(class, _)| class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_tier_highest_class_is_performance() {
        // X2E-78-100 real topology per issue #2: 6 x Oryon P1 (efficiency
        // class 0) + 6 x Oryon P2 (efficiency class 1) — order in the raw
        // class list doesn't matter, only which class is highest.
        let classes = [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1];
        let kinds = classes_to_kinds(&classes, &[]);
        assert_eq!(kinds[..6], [CoreKind::Efficiency; 6]);
        assert_eq!(kinds[6..], [CoreKind::Performance; 6]);
    }

    #[test]
    fn homogeneous_is_all_performance() {
        // X1 Elite / X2P-42: a single Efficiency Class for every core.
        let classes = [0u8; 8];
        let kinds = classes_to_kinds(&classes, &[]);
        assert!(kinds.iter().all(|k| *k == CoreKind::Performance));
    }

    #[test]
    fn three_tier_only_top_class_is_performance() {
        // Hypothetical 3-tier topology: everything below the max class is
        // Efficiency, not just the immediately-lower tier.
        let classes = [0, 1, 2];
        let kinds = classes_to_kinds(&classes, &[]);
        assert_eq!(kinds, [CoreKind::Efficiency, CoreKind::Efficiency, CoreKind::Performance]);
    }

    #[test]
    fn uses_real_clocks_to_resolve_snapdragon_855() {
        // Surface Duo / Snapdragon 855: the four Kryo Gold cores (0-3,
        // ~1632 MHz) report EfficiencyClass 0, while the slower Kryo Silver
        // cores (4-7, ~710/826 MHz) report class 1 — the opposite of the
        // Snapdragon X convention. The clock values must win so the big
        // cores are Performance, not Efficiency.
        let classes = [0, 0, 0, 0, 1, 1, 1, 1];
        let mhz = [1632, 1632, 1632, 1632, 710, 710, 710, 826];
        let kinds = classes_to_kinds(&classes, &mhz);
        assert_eq!(kinds[..4], [CoreKind::Performance; 4]);
        assert_eq!(kinds[4..], [CoreKind::Efficiency; 4]);
    }

    #[test]
    fn keeps_highest_class_when_clocks_agree_with_it() {
        // Snapdragon X1P-64-100: class 1 cores are the 3418 MHz ones, so the
        // clock check must preserve the existing class-based result.
        let classes = [0, 0, 0, 0, 1, 1, 1, 1, 1, 1];
        let mhz = [2976, 2976, 2976, 2976, 3418, 3418, 3418, 3418, 3418, 3418];
        let kinds = classes_to_kinds(&classes, &mhz);
        assert_eq!(kinds[..4], [CoreKind::Efficiency; 4]);
        assert_eq!(kinds[4..], [CoreKind::Performance; 6]);
    }

    #[test]
    fn falls_back_to_class_when_clocks_are_missing() {
        let classes = [0, 0, 1, 1];
        let kinds = classes_to_kinds(&classes, &[]);
        assert_eq!(
            kinds,
            [
                CoreKind::Efficiency,
                CoreKind::Efficiency,
                CoreKind::Performance,
                CoreKind::Performance
            ]
        );
    }
}
