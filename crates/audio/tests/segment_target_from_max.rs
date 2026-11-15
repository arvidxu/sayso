use audio::{AudioConfig, SegmentationTuning};

fn tuning(target_fraction_of_max: f32) -> SegmentationTuning {
    SegmentationTuning {
        target_fraction_of_max,
        ..AudioConfig::bundled()
            .expect("the bundled configuration loads")
            .segmentation
    }
}

#[test]
fn the_target_is_the_configured_fraction_of_the_maximum() {
    let cases = [
        (0.5, 30_000, 15_000),
        (0.25, 30_000, 7_500),
        (0.75, 20_000, 15_000),
        (1.0, 12_345, 12_345),
        (0.1, 1_000, 100),
    ];
    for (fraction, max_ms, target_ms) in cases {
        let limits = tuning(fraction).for_max(max_ms);
        assert_eq!(
            limits.target_segment_ms, target_ms,
            "{fraction} of {max_ms}"
        );
        assert_eq!(limits.max_segment_ms, max_ms);
    }
}

#[test]
fn the_target_is_at_least_a_millisecond_and_never_above_the_maximum() {
    for fraction in [f32::MIN_POSITIVE, 0.000_1, 0.01, 0.5, 0.999, 1.0] {
        for max_ms in [1, 2, 3, 10, 999, 30_000, 86_400_000] {
            let limits = tuning(fraction).for_max(max_ms);
            assert!(limits.target_segment_ms >= 1, "{fraction} of {max_ms}");
            assert!(
                limits.target_segment_ms <= max_ms,
                "{fraction} of {max_ms} gives {}",
                limits.target_segment_ms
            );
            limits
                .validate()
                .unwrap_or_else(|err| panic!("{fraction} of {max_ms} is unusable: {err}"));
        }
    }
}

#[test]
fn a_larger_fraction_never_gives_a_shorter_target() {
    let mut previous = 0;
    for step in 1..=100 {
        let target = tuning(step as f32 / 100.0)
            .for_max(30_000)
            .target_segment_ms;
        assert!(target >= previous, "{step}% gives a shorter target");
        previous = target;
    }
}

#[test]
fn a_fraction_outside_its_range_is_refused() {
    for bad in [0.0, -0.1, 1.001, 2.0, f32::NAN, f32::INFINITY] {
        assert!(tuning(bad).validate().is_err(), "{bad} was accepted");
    }
    for good in [f32::MIN_POSITIVE, 0.5, 1.0] {
        tuning(good).validate().expect("within the range");
    }
}

#[test]
fn the_bundled_fraction_gives_usable_limits() {
    let segmentation = AudioConfig::bundled().unwrap().segmentation;
    for max_ms in [1, 1_000, 30_000] {
        segmentation
            .for_max(max_ms)
            .validate()
            .expect("usable limits");
    }
}
