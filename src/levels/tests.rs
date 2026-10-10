use super::*;
use crate::ball::{BallSpeed, BALL_SPEED, BALL_SPEED_SCALE};
use crate::bricks::grid::{
    brick_translation, Brick, BrickHealth, BrickMaxHits, CarriesPowerUp, BRICK_WIDTH, SIDE_CHANNEL,
};
use crate::bricks::BrickCell;
use crate::game_state::{AppState, GameOutcome};
use crate::powerups::{PowerUp, PowerUpBrick};
use crate::run::{Lives, Score};
use crate::test_support::*;
use crate::world::PLAYFIELD_WIDTH;
use avian2d::prelude::LinearVelocity;

fn level(text: &str) -> LevelDef {
    match parse_level(text) {
        Ok(def) => def,
        Err(e) => panic!("{e}"),
    }
}

/// Every brick's cell and class, sorted.
fn layout(app: &mut App) -> Vec<(BrickCell, BrickClass)> {
    let mut bricks: Vec<_> = app
        .world_mut()
        .query_filtered::<(&BrickCell, &BrickClass), With<Brick>>()
        .iter(app.world())
        .map(|(cell, class)| (*cell, *class))
        .collect();
    bricks.sort_by_key(|(cell, _)| *cell);
    bricks
}

/// The brick in `cell`.
fn brick_at(app: &mut App, row: usize, col: usize) -> Entity {
    app.world_mut()
        .query::<(Entity, &BrickCell)>()
        .iter(app.world())
        .find(|(_, c)| **c == BrickCell { row, col })
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("no brick at ({row},{col})"))
}

/// Loses the run's last life, then presses R for a new run.
fn restart(app: &mut App) {
    app.world_mut().resource_mut::<Lives>().0 = 1;
    tap(app, KeyCode::Space);
    move_ball_below_screen(app);
    app.update();
    app.update();
    assert_eq!(app_state(app), AppState::GameOver);
    tap(app, KeyCode::KeyR);
    assert_eq!(app_state(app), AppState::InGame);
}

#[test]
fn the_fallback_board_is_seven_rows_of_random_with_six_power_ups() {
    let rows = ["??????????"; 7].join("\n");
    assert_eq!(
        parse_level(&format!("name: Random\npowerups: 6\ngrid:\n{rows}")),
        Ok(LevelDef::fallback())
    );
}

/// A shipped galaxy: its `# Galaxy:` name and its levels' files and defs.
struct Galaxy {
    name: String,
    levels: Vec<(String, LevelDef)>,
}

/// `campaign.txt` split into galaxies at its `# Galaxy: <name>` comments,
/// every level parsed with the real parser.
fn shipped_galaxies() -> Vec<Galaxy> {
    let manifest = include_str!("../../assets/levels/campaign.txt");
    let mut galaxies: Vec<Galaxy> = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("# Galaxy:") {
            galaxies.push(Galaxy {
                name: name.trim().to_string(),
                levels: Vec::new(),
            });
        } else if !line.is_empty() && !line.starts_with('#') {
            let path = format!("assets/levels/{line}");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let def = parse_level(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
            let galaxy = galaxies
                .last_mut()
                .unwrap_or_else(|| panic!("{line} is listed before any # Galaxy: comment"));
            galaxy.levels.push((line.to_string(), def));
        }
    }
    let listed: Vec<String> = galaxies
        .iter()
        .flat_map(|g| g.levels.iter().map(|(file, _)| file.clone()))
        .collect();
    assert_eq!(
        listed,
        parse_campaign(manifest).levels,
        "the galaxy groups are the campaign"
    );
    galaxies
}

/// Deliberate negative space: at least 25% of the cells are empty, and some
/// row has a run of 2+ empty cells with bricks on both sides of it.
fn has_negative_space(def: &LevelDef) -> bool {
    let cells = def.rows() * def.cols();
    let empty = def.grid.iter().flatten().filter(|c| c.is_none()).count();
    let enclosed_gap = def.grid.iter().any(|row| {
        let line: String = row
            .iter()
            .map(|c| if c.is_some() { 'b' } else { '.' })
            .collect();
        let inner = line.trim_matches('.');
        inner.split('b').any(|run| run.len() >= 2)
    });
    empty * 4 >= cells && enclosed_gap
}

#[test]
fn the_shipped_campaign_is_five_galaxies_of_five_valid_themed_levels() {
    let galaxies = shipped_galaxies();
    let names: Vec<&str> = galaxies.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Milky Way",
            "Andromeda",
            "Triangulum",
            "Large Magellanic Cloud",
            "Sombrero"
        ]
    );
    for galaxy in &galaxies {
        assert_eq!(galaxy.levels.len(), 5, "{}: 5 levels", galaxy.name);
        assert!(
            galaxy.levels.iter().any(|(_, def)| has_negative_space(def)),
            "{}: a level with negative space",
            galaxy.name
        );
        for (file, def) in &galaxy.levels {
            let bricks: Vec<&CellDef> = def.grid.iter().flatten().flatten().collect();
            assert!(!bricks.is_empty(), "{file} has a brick");
            let random = bricks
                .iter()
                .filter(|c| c.class == ClassSpec::Random)
                .count();
            assert!(
                random * 5 <= bricks.len(),
                "{file}: {random} of {} bricks are '?' (max 20%)",
                bricks.len()
            );
            assert_eq!(def.speed_factor, None, "{file} sets no speed_factor");
        }
    }
    let milky_way: Vec<&str> = galaxies[0]
        .levels
        .iter()
        .map(|(_, d)| d.name.as_str())
        .collect();
    assert_eq!(milky_way, ["Mercury", "Mars", "Earth", "Venus", "Jupiter"]);
}

#[test]
fn negative_space_needs_enough_empty_cells_and_an_enclosed_gap() {
    // 50% empty, but only as a gap row and side margins: no enclosed run.
    assert!(!has_negative_space(&level("grid:\nCCCC\n....\n.CC.\n....")));
    // An enclosed run of 2, but under 25% empty.
    assert!(!has_negative_space(&level("grid:\nC..C\nCCCC\nCCCC")));
    assert!(has_negative_space(&level("grid:\nC..C\nCCCC")));
}

#[test]
fn the_default_ball_speed_is_the_ball_speed_constant() {
    assert_eq!(
        BallSpeed::from_factor(BALL_SPEED_SCALE),
        BallSpeed::default()
    );
    assert_eq!(BallSpeed::default().0, 720.0);
    assert_eq!(BallSpeed::from_factor(2.0).0, 800.0);
}

#[test]
fn a_hand_written_shape_spawns_exactly_that_shape_centred() {
    let rows = ["...CC...", "..TXXT..", ".CSSSSC.", "..TXXT..", "...CC..."];
    let def = level(&format!("grid:\n{}", rows.join("\n")));
    let mut expected = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        for (col, symbol) in line.chars().enumerate() {
            let class = match symbol {
                'C' => BrickClass::Ceramic,
                'T' => BrickClass::Titanium,
                'X' => BrickClass::Explosive(crate::bricks::ExplosiveKind::Charge),
                'S' => BrickClass::Shield,
                _ => continue,
            };
            expected.push((BrickCell { row, col }, class));
        }
    }
    let mut app = app_with_level(def);
    assert_eq!(layout(&mut app), expected);

    let world = app.world_mut();
    let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
    for (cell, transform) in world
        .query_filtered::<(&BrickCell, &Transform), With<Brick>>()
        .iter(world)
    {
        assert_eq!(transform.translation, brick_translation(*cell, 8));
        left = left.min(transform.translation.x - BRICK_WIDTH / 2.0);
        right = right.max(transform.translation.x + BRICK_WIDTH / 2.0);
    }
    // The outermost bricks are in columns 1 and 6 of 8, so the shape is
    // centred too.
    let left_channel = left + PLAYFIELD_WIDTH / 2.0;
    let right_channel = PLAYFIELD_WIDTH / 2.0 - right;
    assert!((left_channel - right_channel).abs() <= 0.5);
    assert!(left_channel >= SIDE_CHANNEL);
}

#[test]
fn a_hits_override_needs_that_many_hits() {
    let mut app = app_with_level(level("legend:\nk = titanium hits=4\ngrid:\nk"));
    let brick = brick_at(&mut app, 0, 0);
    assert_eq!(app.world().get::<BrickHealth>(brick).map(|h| h.0), Some(4));
    assert_eq!(app.world().get::<BrickMaxHits>(brick).map(|m| m.0), Some(4));
    tap(&mut app, KeyCode::Space);
    for left in [3, 2, 1] {
        hit(&mut app, brick);
        assert_eq!(
            app.world().get::<BrickHealth>(brick).map(|h| h.0),
            Some(left)
        );
    }
    hit(&mut app, brick);
    assert!(app.world().get_entity(brick).is_err());
    assert_eq!(app.world().resource::<Score>().0, 40);
    app.update();
    app.update();
    assert_eq!(app_state(&app), AppState::GameOver);
    assert_eq!(
        app.world().get_resource::<GameOutcome>(),
        Some(&GameOutcome::Won)
    );
}

#[test]
fn a_powerup_flagged_ceramic_drops_a_power_up() {
    let mut app = app_with_level(level("legend:\nv = ceramic powerup\ngrid:\nvC"));
    let (v, c) = (brick_at(&mut app, 0, 0), brick_at(&mut app, 0, 1));
    assert_eq!(app.world().get::<BrickClass>(v), Some(&BrickClass::Ceramic));
    assert!(app.world().entity(v).contains::<CarriesPowerUp>());
    assert!(app.world().entity(v).contains::<PowerUpBrick>());
    assert!(!app.world().entity(c).contains::<CarriesPowerUp>());
    assert!(!app.world().entity(c).contains::<PowerUpBrick>());
    tap(&mut app, KeyCode::Space);
    hit(&mut app, v);
    assert!(app.world().get_entity(v).is_err());
    assert_eq!(count::<With<PowerUp>>(&mut app), 1);
}

#[test]
fn a_level_speed_factor_sets_the_serve_speed() {
    let mut app = app_with_level(level("speed_factor: 2.0\ngrid:\nC"));
    let speed = 800.0;
    assert_eq!(app.world().resource::<BallSpeed>().0, speed);
    tap(&mut app, KeyCode::Space);
    let ball = ball(&mut app);
    let v = app.world().get::<LinearVelocity>(ball).map(|v| v.0);
    assert!(
        v.is_some_and(|v| (v.length() - speed).abs() < 1e-2),
        "{v:?}"
    );
}

#[test]
fn without_a_level_the_run_uses_the_fallback() {
    let mut app = app();
    assert!(app.world().get_resource::<CampaignLevels>().is_none());
    assert_eq!(count::<With<Brick>>(&mut app), 70);
    assert_eq!(count::<With<PowerUpBrick>>(&mut app), 6);
    assert_eq!(count::<With<CarriesPowerUp>>(&mut app), 0);
    assert_eq!(app.world().resource::<BallSpeed>().0, BALL_SPEED);
}

#[test]
fn a_changed_level_applies_at_the_next_run_not_mid_board() {
    let mut app = app_with_level(level("speed_factor: 1.0\ngrid:\nCC"));
    app.insert_resource(CampaignLevels(vec![level("speed_factor: 2.0\ngrid:\nCCC")]));
    app.update();
    assert_eq!(count::<With<Brick>>(&mut app), 2, "not mid-board");
    assert_eq!(app.world().resource::<BallSpeed>().0, 400.0);
    restart(&mut app);
    assert_eq!(count::<With<Brick>>(&mut app), 3);
    assert_eq!(app.world().resource::<BallSpeed>().0, 800.0);
}

#[test]
fn removing_the_campaign_falls_back_to_the_random_board() {
    let mut app = app_with_level(level("grid:\nCC"));
    app.world_mut().remove_resource::<CampaignLevels>();
    restart(&mut app);
    assert_eq!(count::<With<Brick>>(&mut app), 70);
    assert_eq!(count::<With<PowerUpBrick>>(&mut app), 6);
}

#[test]
fn without_a_campaign_the_fallback_is_the_only_level() {
    assert_eq!(campaign_level(None, 0), Some(LevelDef::fallback()));
    assert_eq!(campaign_level(None, 1), None);
    let empty = CampaignLevels(vec![]);
    assert_eq!(campaign_level(Some(&empty), 0), Some(LevelDef::fallback()));
    assert_eq!(campaign_level(Some(&empty), 1), None);
}

#[test]
fn campaign_level_indexes_the_campaign_in_order() {
    let a = level("name: A\ngrid:\nC");
    let b = level("name: B\ngrid:\nT");
    let campaign = CampaignLevels(vec![a.clone(), b.clone()]);
    assert_eq!(campaign_level(Some(&campaign), 0), Some(a));
    assert_eq!(campaign_level(Some(&campaign), 1), Some(b));
    assert_eq!(campaign_level(Some(&campaign), 2), None);
}
