use super::*;

const PERIOD: f32 = 2160.0;

fn planet() -> LayerSpec {
    *LAYERS.iter().find(|l| !l.tiled).unwrap()
}
const VIEW: f32 = PLAYFIELD_HEIGHT;

/// The texture row shown `y` px below the top of the view.
fn row_at(slices: &[Slice; 2], y: f32) -> f32 {
    let slice = slices
        .iter()
        .find(|s| s.height > 0.0 && y >= s.screen_top && y < s.screen_top + s.height)
        .expect("every row of the view is covered");
    slice.src_top + (y - slice.screen_top)
}

#[test]
fn the_offset_wraps_every_period() {
    assert_eq!(tile_offset(0.0, 28.0, PERIOD), 0.0);
    assert!((tile_offset(10.0, 28.0, PERIOD) - 280.0).abs() < 1e-3);
    // One full period later it's back where it was.
    let period_secs = PERIOD / 28.0;
    let a = tile_offset(3.0, 28.0, PERIOD);
    let b = tile_offset(3.0 + period_secs, 28.0, PERIOD);
    assert!((a - b).abs() < 1e-2, "{a} {b}");
    for t in [0.0, 1.0, 77.1, 1000.0] {
        let o = tile_offset(t, 28.0, PERIOD);
        assert!((0.0..PERIOD).contains(&o));
    }
}

#[test]
fn the_slices_always_fill_the_view_with_no_gap() {
    for offset in [0.0, 1.0, 500.0, 1079.0, 1080.0, 1081.0, 2000.0, 2159.0] {
        let slices = tile_slices(offset, PERIOD, VIEW);
        let total: f32 = slices.iter().map(|s| s.height).sum();
        assert_eq!(total, VIEW, "offset {offset}");
        assert_eq!(slices[1].screen_top, slices[0].height);
        // Consecutive screen rows show consecutive texture rows (mod period):
        // the wrap is seamless.
        for y in 0..VIEW as usize - 1 {
            let (a, b) = (row_at(&slices, y as f32), row_at(&slices, y as f32 + 1.0));
            assert_eq!((a + 1.0).rem_euclid(PERIOD), b, "offset {offset} row {y}");
        }
        for s in &slices {
            assert!(s.src_top >= 0.0 && s.src_top + s.height <= PERIOD);
        }
    }
}

#[test]
fn the_content_drifts_down_with_no_jump_across_the_wrap() {
    // A texture row seen on screen moves down by exactly the drift, even as
    // the offset wraps from just under the period back to 0.
    let screen_y_of = |offset: f32, row: f32| {
        let slices = tile_slices(offset, PERIOD, VIEW);
        (0..VIEW as usize)
            .map(|y| y as f32)
            .find(|&y| row_at(&slices, y) == row)
    };
    let before = screen_y_of(PERIOD - 2.0, 10.0).unwrap();
    let after = screen_y_of((PERIOD - 2.0 + 5.0).rem_euclid(PERIOD), 10.0).unwrap();
    assert_eq!(after - before, 5.0);
    // At offset 0 the view shows rows 0.. from the top.
    assert_eq!(row_at(&tile_slices(0.0, PERIOD, VIEW), 0.0), 0.0);
}

#[test]
fn the_planet_drifts_down_and_re_enters_from_above() {
    let size = 640.0;
    let start = planet().start.y;
    assert_eq!(planet_y(0.0, 2.0, start, size), start);
    assert!((planet_y(10.0, 2.0, start, size) - (start - 20.0)).abs() < 1e-3);
    // Fully below the well: re-enters just above the top, no stop.
    let bottom = -PLAYFIELD_HEIGHT / 2.0 - size / 2.0;
    let top = PLAYFIELD_HEIGHT / 2.0 + size / 2.0;
    let to_bottom = (start - bottom) / 2.0;
    assert!((planet_y(to_bottom - 0.5, 2.0, start, size) - (bottom + 1.0)).abs() < 1e-2);
    assert!((planet_y(to_bottom + 0.5, 2.0, start, size) - (top - 1.0)).abs() < 1e-2);
    // A full cycle (about 14 minutes) brings it back to the start.
    let cycle = (PLAYFIELD_HEIGHT + size) / 2.0;
    assert!((13.0 * 60.0..15.0 * 60.0).contains(&cycle));
    assert!((planet_y(cycle, 2.0, start, size) - start).abs() < 1e-2);
}

#[test]
fn the_planet_starts_lower_right_partly_cut_off() {
    let planet = planet();
    assert!(!planet.tiled);
    let inside_right = PLAYFIELD_WIDTH / 2.0 - planet.start.x;
    assert!((80.0..=120.0).contains(&inside_right));
    assert!(planet.start.y < 0.0, "lower half");
    let (world, texture) = crop_to_well(planet.start, Vec2::splat(640.0)).unwrap();
    assert_eq!(world.max.x, PLAYFIELD_WIDTH / 2.0, "cut at the right edge");
    assert!(world.width() < 640.0);
    assert_eq!(texture.size(), world.size());
    assert_eq!(texture.min, Vec2::ZERO, "keeps its left part");
}

#[test]
fn cropping_keeps_every_pixel_inside_the_well() {
    let well = well();
    let size = Vec2::splat(640.0);
    for centre in [
        Vec2::new(620.0, -340.0),
        Vec2::new(620.0, -800.0),
        Vec2::new(-700.0, 520.0),
        Vec2::ZERO,
    ] {
        let (world, texture) = crop_to_well(centre, size).unwrap();
        assert_eq!(world.union(well), well, "{centre}");
        assert_eq!(texture.size(), world.size());
        assert!(texture.min.cmpge(Vec2::ZERO).all() && texture.max.cmple(size).all());
    }
    // Below the bottom edge: the texture's bottom rows are cut, its top kept.
    let (world, texture) = crop_to_well(Vec2::new(0.0, -540.0), size).unwrap();
    assert_eq!(world.min.y, -540.0);
    assert_eq!(texture.min.y, 0.0);
    assert_eq!(texture.max.y, 320.0);
    assert!(crop_to_well(Vec2::new(0.0, -900.0), size).is_none());
}

#[test]
fn the_layers_sit_between_the_background_and_the_frame() {
    let mut z = -10.0;
    for layer in LAYERS {
        assert!(
            layer.z > z && layer.z < crate::frame::FRAME_Z,
            "{}",
            layer.path
        );
        z = layer.z;
    }
    let speeds: Vec<f32> = LAYERS
        .iter()
        .filter(|l| l.tiled && !l.glow)
        .map(|l| l.speed)
        .collect();
    assert_eq!(speeds, [8.0, 16.0, 28.0], "stars slowest, wisps fastest");
}

#[test]
fn each_placeholder_is_at_its_contract_size_and_small() {
    for layer in LAYERS {
        let path = format!("assets/{}", layer.path);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(bytes.len() <= 1024 * 1024, "{path}: {} bytes", bytes.len());
        // PNG IHDR: width and height, big-endian, at bytes 16..24.
        let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        let expected = if layer.tiled {
            (1440, 2160)
        } else {
            (640, 640)
        };
        assert_eq!((w, h), expected, "{path}");
    }
}

#[test]
fn every_layer_gets_its_sprites_hidden_until_its_image_loads() {
    let mut app = crate::test_support::app();
    app.update();
    let world = app.world_mut();
    let sprites: Vec<(usize, Visibility)> = world
        .query::<(&ParallaxSprite, &Visibility)>()
        .iter(world)
        .map(|(p, v)| (p.layer, *v))
        .collect();
    assert_eq!(sprites.len(), 2 + 2 + 2 + 2 + 1);
    assert!(sprites.iter().all(|(_, v)| *v == Visibility::Hidden));
}

#[test]
fn the_glow_rides_with_the_cloud_layer_between_l1_and_l2() {
    let by_path = |p: &str| *LAYERS.iter().find(|l| l.path == p).unwrap();
    let (l1, glow, l2) = (
        by_path("parallax/space_l1.png"),
        by_path("parallax/space_l1_glow.png"),
        by_path("parallax/space_l2.png"),
    );
    assert!(glow.glow && glow.tiled);
    assert_eq!(LAYERS.iter().filter(|l| l.glow).count(), 1);
    assert_eq!(glow.speed, l1.speed, "same drift, so it stays in register");
    assert!(l1.z < glow.z && glow.z < l2.z);
    // Same speed and texture height: the same slices at every moment.
    for t in [0.0, 12.5, 135.0, 400.0] {
        assert_eq!(
            tile_offset(t, glow.speed, PERIOD),
            tile_offset(t, l1.speed, PERIOD)
        );
    }
}

#[test]
fn breathing_stays_between_its_floor_and_top_and_is_smooth() {
    let mut prev = breathing(0.0);
    assert!((prev - GLOW_FLOOR).abs() < 1e-5, "starts at the floor");
    assert!((breathing(GLOW_BREATH_SECS / 2.0) - GLOW_BREATH_TOP).abs() < 1e-5);
    for i in 1..=2000 {
        let b = breathing(i as f32 * 0.01);
        assert!((GLOW_FLOOR - 1e-5..=GLOW_BREATH_TOP + 1e-5).contains(&b));
        assert!((b - prev).abs() < 0.005, "no jumps at {}", i as f32 * 0.01);
        prev = b;
    }
}

#[test]
fn the_glow_never_leaves_its_floor_and_cap() {
    let mut schedule = StrikeSchedule::seeded(7);
    for i in 0..30_000 {
        let t = i as f32 * 0.01;
        schedule.advance(t);
        // The schedule's deque goes straight in, as `pulse_glow` passes it (no per-frame Vec).
        let g = glow_intensity(t, &schedule.strikes);
        assert!(
            (GLOW_FLOOR - 1e-5..=GLOW_PEAK + 1e-5).contains(&g),
            "{g} at {t}"
        );
    }
}

#[test]
fn a_strike_flashes_then_fades_back_to_breathing() {
    let strike = Strike {
        start: 10.0,
        flicker: 0.4,
        flashes: 3,
    };
    let strikes = [strike];
    // Before it: just breathing.
    assert_eq!(glow_intensity(9.9, &strikes), breathing(9.9));
    // Lit at the start of each flash, reaching the cap.
    for k in 0..3 {
        let t = 10.0 + 0.4 * k as f32 / 3.0 + 0.01;
        assert!(
            (glow_intensity(t, &strikes) - GLOW_PEAK).abs() < 1e-5,
            "flash {k}"
        );
    }
    // Counting the rising edges gives the flash count.
    let mut lit = false;
    let mut flashes = 0;
    for i in 0..=400 {
        let on = strike.envelope(10.0 + i as f32 * 0.001) > 0.99;
        if on && !lit {
            flashes += 1;
        }
        lit = on;
    }
    assert_eq!(flashes, 3);
    // The fade falls smoothly to breathing over STRIKE_FADE_SECS.
    let mut prev = strike.envelope(10.4);
    for i in 1..=100 {
        let e = strike.envelope(10.4 + i as f32 * 0.01);
        assert!(e <= prev + 1e-6 && prev - e < 0.05);
        prev = e;
    }
    assert_eq!(
        glow_intensity(strike.end() + 0.01, &strikes),
        breathing(strike.end() + 0.01)
    );
}

#[test]
fn strikes_come_at_varied_intervals_within_their_ranges() {
    let mut schedule = StrikeSchedule::seeded(42);
    let mut starts = Vec::new();
    let mut t = 0.0;
    while t < 600.0 {
        schedule.advance(t);
        for s in &schedule.strikes {
            if !starts.contains(&s.start) {
                starts.push(s.start);
                assert!((STRIKE_FLICKER_SECS.0..=STRIKE_FLICKER_SECS.1).contains(&s.flicker));
                assert!((STRIKE_FLASHES.0..=STRIKE_FLASHES.1).contains(&s.flashes));
            }
        }
        t += 0.25;
    }
    assert!(starts[0] <= STRIKE_GAP_SECS.1, "the first within 15 s");
    let gaps: Vec<f32> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.len() > 30);
    for g in &gaps {
        assert!((STRIKE_GAP_SECS.0..=STRIKE_GAP_SECS.1).contains(g), "{g}");
    }
    let (min, max) = gaps
        .iter()
        .fold((f32::MAX, 0.0f32), |(a, b), &g| (a.min(g), b.max(g)));
    assert!(max - min > 4.0, "not a metronome: {min}..{max}");
    // The same seed gives the same schedule.
    let mut again = StrikeSchedule::seeded(42);
    again.advance(0.0);
    assert_eq!(again.strikes[0].start, starts[0]);
}
