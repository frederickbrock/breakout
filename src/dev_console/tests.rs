use std::ops::RangeInclusive;

use super::sections::{self, Controls, NEXT_BOARD};
use crate::tuning::{parse_tuning, tuning_file_text, Tuning};

/// Moves every control to a value different from the one it shows (the end
/// of its range, or the start if it's already there; bools flip), and
/// records the labels and headings it saw.
#[derive(Default)]
struct Nudge {
    labels: Vec<String>,
    headings: Vec<String>,
}

impl Controls for Nudge {
    fn f32(&mut self, label: &str, value: &mut f32, range: RangeInclusive<f32>) {
        assert!(
            range.contains(value),
            "{label}: default {value} outside {range:?}"
        );
        *value = if *value == *range.end() {
            *range.start()
        } else {
            *range.end()
        };
        self.labels.push(label.into());
    }
    fn u32(&mut self, label: &str, value: &mut u32, range: RangeInclusive<u32>) {
        assert!(
            range.contains(value),
            "{label}: default {value} outside {range:?}"
        );
        *value = if *value == *range.end() {
            *range.start()
        } else {
            *range.end()
        };
        self.labels.push(label.into());
    }
    fn toggle(&mut self, label: &str, value: &mut bool) {
        *value = !*value;
        self.labels.push(label.into());
    }
    fn heading(&mut self, text: &str) {
        self.headings.push(text.into());
    }
}

fn nudged() -> (Tuning, Nudge) {
    let mut t = Tuning::default();
    let mut nudge = Nudge::default();
    sections::bricks(&mut nudge, &mut t.bricks);
    sections::powerups(&mut nudge, &mut t.powerups);
    (t, nudge)
}

/// `(path, value)` for every leaf field of `tuning`'s saved text, e.g.
/// `("bricks.hits.ceramic", "1")`.
fn leaves(tuning: &Tuning) -> Vec<(String, String)> {
    let mut path: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for line in tuning_file_text(tuning).lines().map(str::trim) {
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        if line.starts_with(')') {
            path.pop();
        } else if let Some(key) = line.strip_suffix(": (") {
            path.push(key.into());
        } else if let Some((key, value)) = line.split_once(": ") {
            let full = path
                .iter()
                .chain([&key.to_string()])
                .cloned()
                .collect::<Vec<_>>();
            out.push((full.join("."), value.trim_end_matches(',').to_string()));
        }
    }
    out
}

#[test]
fn every_bricks_and_power_ups_field_has_a_control() {
    let before = leaves(&Tuning::default());
    let (t, _) = nudged();
    let after = leaves(&t);
    assert_eq!(before.len(), after.len());
    let in_sections: Vec<_> = before
        .iter()
        .zip(&after)
        .filter(|((path, _), _)| path.starts_with("bricks.") || path.starts_with("powerups."))
        .collect();
    assert!(in_sections.len() >= 30, "{} fields", in_sections.len());
    for ((path, old), (_, new)) in in_sections {
        assert_ne!(old, new, "{path} has no control in the panel");
    }
    // The other sections are untouched.
    assert_eq!(t.ball, Tuning::default().ball);
    assert_eq!(t.paddle, Tuning::default().paddle);
}

#[test]
fn next_board_values_are_marked_and_live_ones_are_not() {
    let (_, nudge) = nudged();
    let marked = |s: &String| s.ends_with(NEXT_BOARD);
    for heading in ["Hits per class", "Random fill weights"] {
        assert!(
            nudge
                .headings
                .iter()
                .any(|h| h.starts_with(heading) && marked(h)),
            "{heading}: {:?}",
            nudge.headings
        );
    }
    assert!(nudge
        .labels
        .iter()
        .any(|l| l.starts_with("fallback board power-ups") && marked(l)));
    for live in [
        "regen heal (s)",
        "breach radius",
        "duration (s)",
        "drop chance (ordinary bricks)",
    ] {
        assert!(
            nudge.labels.iter().any(|l| l == live),
            "{live}: {:?}",
            nudge.labels
        );
    }
    for heading in ["Blasts", "Drops", "Super-Sizer"] {
        assert!(
            nudge.headings.iter().any(|h| h == heading),
            "{heading} unmarked"
        );
    }
}

#[test]
fn save_round_trips_every_edited_field() {
    let (t, _) = nudged();
    assert_ne!(t, Tuning::default());
    assert_eq!(parse_tuning(&tuning_file_text(&t)), Ok(t));
}
