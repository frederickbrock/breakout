use super::*;

use LevelErrorKind::*;

fn fixed(class: BrickClass) -> Option<CellDef> {
    Some(CellDef {
        class: ClassSpec::Fixed(class),
        hits: None,
        powerup: false,
    })
}

fn err(text: &str) -> LevelError {
    match parse_level(text) {
        Ok(def) => panic!("expected an error, parsed {def:?}"),
        Err(e) => e,
    }
}

fn ok(text: &str) -> LevelDef {
    match parse_level(text) {
        Ok(def) => def,
        Err(e) => panic!("{e}"),
    }
}

const EXAMPLE: &str = "\
# comments start with #
name: Rainbow Bands
ball_speed: 300        # optional; default 300 (today's speed)
powerups: 6            # optional; N extra random bricks get a power-up; default 0
legend:                # optional; add or override symbols for this file
  k = titanium hits=4
  v = ceramic powerup
grid:
XXXXXXXXXX
CCCCCCCCCC
..TTTTTT..
";

#[test]
fn parses_the_format_example() {
    let def = ok(EXAMPLE);
    assert_eq!(def.name, "Rainbow Bands");
    assert_eq!(def.ball_speed, 300.0);
    assert_eq!(def.extra_powerups, 6);
    assert_eq!(def.grid.len(), 3);
    assert_eq!(def.cols(), 10);
    assert_eq!(
        def.grid[0][0],
        fixed(BrickClass::Explosive(ExplosiveKind::Charge))
    );
    assert_eq!(def.grid[1][9], fixed(BrickClass::Ceramic));
    assert_eq!(def.grid[2][0], None);
    assert_eq!(def.grid[2][2], fixed(BrickClass::Titanium));
    assert_eq!(def.grid[2][9], None);
}

#[test]
fn optional_keys_default() {
    let def = ok("grid:\nC");
    assert_eq!(def.ball_speed, DEFAULT_BALL_SPEED);
    assert_eq!(def.ball_speed, 300.0);
    assert_eq!(def.extra_powerups, 0);
    assert_eq!(def.name, "");
    assert_eq!(def.grid, vec![vec![fixed(BrickClass::Ceramic)]]);
}

#[test]
fn ball_speed_450_is_read() {
    assert_eq!(ok("ball_speed: 450\ngrid:\nC").ball_speed, 450.0);
    assert_eq!(ok("ball_speed:450.5\ngrid:\nC").ball_speed, 450.5);
}

#[test]
fn every_builtin_symbol_maps_to_its_class() {
    use BrickClass::*;
    let def = ok("grid:\nCTGXBDRSP?");
    let random = Some(CellDef {
        class: ClassSpec::Random,
        hits: None,
        powerup: false,
    });
    assert_eq!(
        def.grid[0],
        vec![
            fixed(Ceramic),
            fixed(Titanium),
            fixed(Tungsten),
            fixed(Explosive(ExplosiveKind::Charge)),
            fixed(Explosive(ExplosiveKind::Breach)),
            fixed(Explosive(ExplosiveKind::Demolition)),
            fixed(Regen),
            fixed(Shield),
            fixed(Reactor),
            random,
        ]
    );
    assert_eq!(ok("grid:\n.C").grid[0][0], None);
}

#[test]
fn legend_adds_and_overrides_symbols() {
    let def =
        ok("legend:\n  k = titanium hits=4\n  v = ceramic powerup\n  C = tungsten\ngrid:\nkvCT");
    assert_eq!(
        def.grid[0][0],
        Some(CellDef {
            class: ClassSpec::Fixed(BrickClass::Titanium),
            hits: Some(4),
            powerup: false,
        })
    );
    assert_eq!(
        def.grid[0][1],
        Some(CellDef {
            class: ClassSpec::Fixed(BrickClass::Ceramic),
            hits: None,
            powerup: true,
        })
    );
    assert_eq!(def.grid[0][2], fixed(BrickClass::Tungsten), "C overridden");
    assert_eq!(def.grid[0][3], fixed(BrickClass::Titanium), "T untouched");
}

#[test]
fn every_legend_class_name_parses() {
    use BrickClass::*;
    for (name, class) in [
        ("ceramic", ClassSpec::Fixed(Ceramic)),
        ("titanium", ClassSpec::Fixed(Titanium)),
        ("tungsten", ClassSpec::Fixed(Tungsten)),
        (
            "explosive:charge",
            ClassSpec::Fixed(Explosive(ExplosiveKind::Charge)),
        ),
        (
            "explosive:breach",
            ClassSpec::Fixed(Explosive(ExplosiveKind::Breach)),
        ),
        (
            "explosive:demolition",
            ClassSpec::Fixed(Explosive(ExplosiveKind::Demolition)),
        ),
        ("regen", ClassSpec::Fixed(Regen)),
        ("shield", ClassSpec::Fixed(Shield)),
        ("reactor", ClassSpec::Fixed(Reactor)),
        ("random", ClassSpec::Random),
    ] {
        let def = ok(&format!("legend:\nk = {name} hits=9 powerup\ngrid:\nk"));
        assert_eq!(
            def.grid[0][0],
            Some(CellDef {
                class,
                hits: Some(9),
                powerup: true,
            }),
            "{name}"
        );
    }
}

#[test]
fn comments_and_blank_lines_are_ignored() {
    let def = ok("\
# a comment

name: Commented # trailing
powerups: 2 # two
   # indented comment
grid:   # the grid
CC  # row one

# between rows
  TT
");
    assert_eq!(def.name, "Commented");
    assert_eq!(def.extra_powerups, 2);
    assert_eq!(
        def.grid,
        vec![
            vec![fixed(BrickClass::Ceramic); 2],
            vec![fixed(BrickClass::Titanium); 2],
        ]
    );
}

#[test]
fn an_unknown_symbol_names_its_line_and_column() {
    assert_eq!(
        err("name: x\n\nlegend:\ngrid:\nCCZC"),
        LevelError {
            line: 5,
            col: 3,
            kind: UnknownSymbol('Z'),
        }
    );
    // Columns count the original line, indentation included.
    assert_eq!(
        err("grid:\n   CCZC"),
        LevelError {
            line: 2,
            col: 6,
            kind: UnknownSymbol('Z'),
        }
    );
    // A space inside a row is not a symbol.
    assert_eq!(err("grid:\nC C").kind, UnknownSymbol(' '));
}

#[test]
fn ragged_rows_are_rejected() {
    assert_eq!(
        err("grid:\nCCCCCCCCCC\nCCCCCCCC\n"),
        LevelError {
            line: 3,
            col: 9,
            kind: RaggedRow {
                expected: 10,
                found: 8,
            },
        }
    );
    assert_eq!(
        err("grid:\nCC\nCCC\n"),
        LevelError {
            line: 3,
            col: 3,
            kind: RaggedRow {
                expected: 2,
                found: 3,
            },
        }
    );
}

#[test]
fn eleven_rows_are_too_many() {
    let ten = "C\n".repeat(10);
    assert_eq!(ok(&format!("grid:\n{ten}")).grid.len(), 10);
    assert_eq!(
        err(&format!("grid:\n{ten}C\n")),
        LevelError {
            line: 12,
            col: 1,
            kind: TooManyRows,
        }
    );
}

#[test]
fn eleven_columns_are_too_many() {
    assert_eq!(ok("grid:\nCCCCCCCCCC").cols(), 10);
    assert_eq!(
        err("grid:\nCCCCCCCCCCC"),
        LevelError {
            line: 2,
            col: 11,
            kind: TooManyColumns(11),
        }
    );
    // Too wide wins over an unknown symbol.
    assert_eq!(err("grid:\nZZZZZZZZZZZZ").kind, TooManyColumns(12));
}

#[test]
fn bad_legend_entries_are_rejected() {
    for (entry, col) in [
        ("k = glass", 5),
        ("k = titanium hits=0", 14),
        ("k = titanium hits=10", 14),
        ("k = titanium hits=x", 14),
        ("k = ceramic shiny", 13),
        ("kk = ceramic", 1),
        (". = ceramic", 1),
        ("k =", 4),
        ("k ceramic", 1),
    ] {
        let e = err(&format!("legend:\n{entry}\ngrid:\nC"));
        assert!(matches!(e.kind, BadAttribute(_)), "{entry}: {e}");
        assert_eq!((e.line, e.col), (2, col), "{entry}: {e}");
    }
}

#[test]
fn bad_header_values_are_rejected() {
    for (line, key, value) in [
        ("ball_speed: fast", "ball_speed", "fast"),
        ("ball_speed: 0", "ball_speed", "0"),
        ("ball_speed: -5", "ball_speed", "-5"),
        ("ball_speed: inf", "ball_speed", "inf"),
        ("powerups: -1", "powerups", "-1"),
        ("powerups: x", "powerups", "x"),
    ] {
        assert_eq!(
            err(&format!("{line}\ngrid:\nC")),
            LevelError {
                line: 1,
                col: key.len() + 3,
                kind: BadValue {
                    key: key.into(),
                    value: value.into(),
                },
            },
            "{line}"
        );
    }
}

#[test]
fn unknown_keys_are_rejected() {
    assert_eq!(
        err("speed: 3\ngrid:\nC"),
        LevelError {
            line: 1,
            col: 1,
            kind: UnknownKey("speed".into()),
        }
    );
    // A header line that isn't `key: value` at all.
    assert_eq!(err("hello\ngrid:\nC").kind, UnknownKey("hello".into()));
}

#[test]
fn a_grid_of_dots_has_no_bricks() {
    assert_eq!(
        err("name: empty\ngrid:\n...\n...\n"),
        LevelError {
            line: 2,
            col: 1,
            kind: NoBricks,
        }
    );
}

#[test]
fn a_file_without_grid_rows_is_rejected() {
    assert_eq!(err("name: no grid\npowerups: 1\n").kind, NoGrid);
    assert_eq!(err("name: empty grid\ngrid:\n# nothing\n").kind, NoGrid);
    assert_eq!(err("").kind, NoGrid);
}

#[test]
fn errors_display_line_and_column() {
    let text = err("name: x\n\nlegend:\ngrid:\nCCZC").to_string();
    assert!(text.starts_with("line 5, column 3:"), "{text}");
    assert!(text.contains("'Z'"), "{text}");
    let ragged = err("grid:\nCC\nC").to_string();
    assert!(ragged.contains("expected 2"), "{ragged}");
}

#[test]
fn the_campaign_lists_files_in_order() {
    assert_eq!(
        parse_campaign("# c\n\n 01-a.level  # x\n02-b.level\n").levels,
        ["01-a.level", "02-b.level"]
    );
    assert!(parse_campaign("# nothing\n").levels.is_empty());
}
