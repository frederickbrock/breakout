//! The `.level` text-grid parser: [`parse_level`] turns a file's text into a
//! [`LevelDef`] or a [`LevelError`] with a 1-based line and column (columns
//! count characters of the original line). Pure, so every rule is unit
//! tested; `docs/levels.md` is the format reference. [`parse_campaign`] reads
//! the manifest.

use std::collections::HashMap;
use std::fmt;

use super::{Campaign, CellDef, ClassSpec, LevelDef, DEFAULT_BALL_SPEED, MAX_COLS, MAX_ROWS};
use crate::bricks::{BrickClass, ExplosiveKind};

/// Why a level file was rejected, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelError {
    pub line: usize,
    pub col: usize,
    pub kind: LevelErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelErrorKind {
    UnknownSymbol(char),
    RaggedRow { expected: usize, found: usize },
    TooManyRows,
    TooManyColumns(usize),
    BadAttribute(String),
    BadValue { key: String, value: String },
    UnknownKey(String),
    NoGrid,
    NoBricks,
}

impl fmt::Display for LevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.col, self.kind)
    }
}

impl fmt::Display for LevelErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSymbol(c) => write!(f, "unknown symbol '{c}'"),
            Self::RaggedRow { expected, found } => write!(
                f,
                "row is {found} wide, expected {expected} (every row must be the same width)"
            ),
            Self::TooManyRows => write!(f, "too many rows (max {MAX_ROWS})"),
            Self::TooManyColumns(n) => write!(f, "row is {n} wide (max {MAX_COLS})"),
            Self::BadAttribute(msg) => write!(f, "bad legend entry: {msg}"),
            Self::BadValue { key, value } => write!(f, "bad value '{value}' for {key}"),
            Self::UnknownKey(key) => write!(f, "unknown key '{key}'"),
            Self::NoGrid => write!(f, "no grid: section with at least one row"),
            Self::NoBricks => write!(f, "the grid has no bricks"),
        }
    }
}

impl std::error::Error for LevelError {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Header,
    Legend,
    Grid,
}

/// Parses a `.level` file. `name` is left empty when the file has none (the
/// asset loader fills in the file stem).
pub fn parse_level(text: &str) -> Result<LevelDef, LevelError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut def = LevelDef {
        name: String::new(),
        ball_speed: DEFAULT_BALL_SPEED,
        extra_powerups: 0,
        grid: Vec::new(),
    };
    let mut legend: HashMap<char, CellDef> = HashMap::new();
    let mut section = Section::Header;
    let (mut grid_line, mut last_line) = (1, 1);

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        last_line = line;
        let at = Line { raw, line };
        // Everything from `#` on is a comment.
        let content = raw.find('#').map_or(raw, |i| &raw[..i]).trim_end();
        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }
        if section == Section::Grid {
            if def.grid.len() == MAX_ROWS {
                return Err(at.error(trimmed, 0, LevelErrorKind::TooManyRows));
            }
            let row = grid_row(content, &legend, def.grid.first().map(Vec::len), at)?;
            def.grid.push(row);
            continue;
        }
        if trimmed == "grid:" {
            section = Section::Grid;
            grid_line = line;
        } else if trimmed == "legend:" {
            section = Section::Legend;
        } else if let (Section::Legend, Some((symbol, attrs))) = (section, content.split_once('='))
        {
            let (symbol, cell) = legend_entry(symbol, attrs, at)?;
            legend.insert(symbol, cell);
        } else if let Some((key, value)) = content.split_once(':') {
            header_value(&mut def, key, value, at)?;
        } else if section == Section::Legend {
            return Err(at.error(
                trimmed,
                0,
                LevelErrorKind::BadAttribute("expected 'symbol = class'".into()),
            ));
        } else {
            return Err(at.error(trimmed, 0, LevelErrorKind::UnknownKey(trimmed.into())));
        }
    }

    if def.grid.is_empty() {
        return Err(LevelError {
            line: last_line,
            col: 1,
            kind: LevelErrorKind::NoGrid,
        });
    }
    if def.grid.iter().flatten().all(Option::is_none) {
        return Err(LevelError {
            line: grid_line,
            col: 1,
            kind: LevelErrorKind::NoBricks,
        });
    }
    Ok(def)
}

/// Reads the campaign manifest: one level file name per line, `#` comments
/// and blank lines ignored.
pub fn parse_campaign(text: &str) -> Campaign {
    let levels = text
        .lines()
        .map(|line| line.find('#').map_or(line, |i| &line[..i]).trim())
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    Campaign { levels }
}

/// The line being parsed, for error positions.
#[derive(Clone, Copy)]
struct Line<'a> {
    raw: &'a str,
    line: usize,
}

impl Line<'_> {
    /// An error at `offset` characters into `part`, a subslice of this line;
    /// the column counts characters of the original line, from 1.
    fn error(&self, part: &str, offset: usize, kind: LevelErrorKind) -> LevelError {
        let start = (part.as_ptr() as usize).saturating_sub(self.raw.as_ptr() as usize);
        let before = self
            .raw
            .get(..start.min(self.raw.len()))
            .map_or(0, |s| s.chars().count());
        LevelError {
            line: self.line,
            col: before + offset + 1,
            kind,
        }
    }
}

/// `name: ...`, `ball_speed: ...` or `powerups: ...`.
fn header_value(def: &mut LevelDef, key: &str, value: &str, at: Line) -> Result<(), LevelError> {
    let key = key.trim();
    let value = value.trim();
    let bad = || {
        at.error(
            value,
            0,
            LevelErrorKind::BadValue {
                key: key.into(),
                value: value.into(),
            },
        )
    };
    match key {
        "name" => def.name = value.into(),
        "ball_speed" => {
            let speed: f32 = value.parse().map_err(|_| bad())?;
            if !speed.is_finite() || speed <= 0.0 {
                return Err(bad());
            }
            def.ball_speed = speed;
        }
        "powerups" => def.extra_powerups = value.parse().map_err(|_| bad())?,
        _ => return Err(at.error(key, 0, LevelErrorKind::UnknownKey(key.into()))),
    }
    Ok(())
}

/// `sym = class [hits=N] [powerup]`.
fn legend_entry(symbol: &str, attrs: &str, at: Line) -> Result<(char, CellDef), LevelError> {
    let bad = |part: &str, msg: String| at.error(part, 0, LevelErrorKind::BadAttribute(msg));
    let sym = symbol.trim();
    let mut chars = sym.chars();
    let symbol = match (chars.next(), chars.next()) {
        (Some(c), None) if !matches!(c, '.' | '#' | '=') => c,
        _ => {
            let part = if sym.is_empty() { attrs } else { sym };
            return Err(bad(
                part,
                "symbol must be one character other than . # =".into(),
            ));
        }
    };
    let mut tokens = attrs.split_whitespace();
    let Some(class_name) = tokens.next() else {
        return Err(bad(attrs, "missing class".into()));
    };
    let class = class_spec(class_name)
        .ok_or_else(|| bad(class_name, format!("unknown class '{class_name}'")))?;
    let mut cell = CellDef {
        class,
        hits: None,
        powerup: false,
    };
    for token in tokens {
        if token == "powerup" {
            cell.powerup = true;
        } else if let Some(n) = token.strip_prefix("hits=") {
            let hits = match n.as_bytes() {
                [d @ b'1'..=b'9'] => d - b'0',
                _ => return Err(bad(token, "hits must be 1-9".into())),
            };
            cell.hits = Some(hits);
        } else {
            return Err(bad(token, format!("unknown attribute '{token}'")));
        }
    }
    Ok((symbol, cell))
}

fn class_spec(name: &str) -> Option<ClassSpec> {
    let class = match name {
        "ceramic" => BrickClass::Ceramic,
        "titanium" => BrickClass::Titanium,
        "tungsten" => BrickClass::Tungsten,
        "explosive:charge" => BrickClass::Explosive(ExplosiveKind::Charge),
        "explosive:breach" => BrickClass::Explosive(ExplosiveKind::Breach),
        "explosive:demolition" => BrickClass::Explosive(ExplosiveKind::Demolition),
        "regen" => BrickClass::Regen,
        "shield" => BrickClass::Shield,
        "reactor" => BrickClass::Reactor,
        "random" => return Some(ClassSpec::Random),
        _ => return None,
    };
    Some(ClassSpec::Fixed(class))
}

/// The built-in symbols: `Some(None)` is an empty cell, `None` unknown.
fn builtin(symbol: char) -> Option<Option<CellDef>> {
    let class = match symbol {
        '.' => return Some(None),
        '?' => ClassSpec::Random,
        'C' => ClassSpec::Fixed(BrickClass::Ceramic),
        'T' => ClassSpec::Fixed(BrickClass::Titanium),
        'G' => ClassSpec::Fixed(BrickClass::Tungsten),
        'X' => ClassSpec::Fixed(BrickClass::Explosive(ExplosiveKind::Charge)),
        'B' => ClassSpec::Fixed(BrickClass::Explosive(ExplosiveKind::Breach)),
        'D' => ClassSpec::Fixed(BrickClass::Explosive(ExplosiveKind::Demolition)),
        'R' => ClassSpec::Fixed(BrickClass::Regen),
        'S' => ClassSpec::Fixed(BrickClass::Shield),
        'P' => ClassSpec::Fixed(BrickClass::Reactor),
        _ => return None,
    };
    Some(Some(CellDef {
        class,
        hits: None,
        powerup: false,
    }))
}

/// One grid row, trimmed both sides. Size errors are reported before symbol
/// errors, so an 11-wide row is "too wide", not "unknown symbol".
fn grid_row(
    content: &str,
    legend: &HashMap<char, CellDef>,
    expected: Option<usize>,
    at: Line,
) -> Result<Vec<Option<CellDef>>, LevelError> {
    let row = content.trim();
    let width = row.chars().count();
    if width > MAX_COLS {
        return Err(at.error(row, MAX_COLS, LevelErrorKind::TooManyColumns(width)));
    }
    if let Some(expected) = expected {
        if width != expected {
            return Err(at.error(
                row,
                width.min(expected),
                LevelErrorKind::RaggedRow {
                    expected,
                    found: width,
                },
            ));
        }
    }
    row.chars()
        .enumerate()
        .map(|(i, c)| {
            legend
                .get(&c)
                .map(|cell| Some(*cell))
                .or_else(|| builtin(c))
                .ok_or_else(|| at.error(row, i, LevelErrorKind::UnknownSymbol(c)))
        })
        .collect()
}
