# Level files

A level is a small text file that describes the brick board a run plays and its
ball speed. The game reads it into a format-independent `LevelDef`
(`src/levels/`), so a later JSON or HTTP source can produce the same model.

## Where levels live

- Level files: `assets/levels/*.level`.
- The campaign manifest: `assets/levels/campaign.txt`, one level file name per
  line, in play order. `#` starts a comment; blank lines are ignored.
  The manifest exists because the web build can't list a directory: every level
  must be listed here.
- A run plays the **first** level in the manifest.
- The shipped `01-random.level` is 7 rows × 10 of `?` with `powerups: 6`,
  which is the classic random board. The game also has this board built in
  and falls back to it whenever there is no valid level.

On the web, trunk copies the whole `assets/` folder into `dist/`
(`index.html`'s `copy-dir`), so `assets/levels/` ships with the build.

## Example

```
# comments start with #
name: Rainbow Bands
ball_speed: 300        # optional; default 300
powerups: 6            # optional; N extra random bricks get a power-up; default 0
legend:                # optional; add or override symbols for this file
  k = titanium hits=4
  v = ceramic powerup
grid:
XXXXXXXXXX
CCCCCCCCCC
..TTTTTT..
```

## Keys

Keys come before `grid:`, one `key: value` per line.

| Key | Default | Meaning |
|---|---|---|
| `name` | the file name without extension | The level's name (shown in the log for now). |
| `ball_speed` | `300` | The ball's speed in design units. Must be a number greater than 0. The game multiplies it by `BALL_SPEED_SCALE` (1.8, in `src/ball.rs`) to get world units per second, so `300` is the default speed and `450` is 1.5 times faster. There is no upper limit, but very high speeds can make the ball pass through bricks. |
| `powerups` | `0` | How many extra bricks drop a power-up (see below). A whole number, 0 or more. |

Any other key is an error.

### How `powerups: N` is resolved

1. Up to N of the level's `?` cells become reactor (power-up) bricks, chosen at
   random.
2. If N is larger than the number of `?` cells, the rest are given to random
   other bricks as a `powerup` flag. These bricks keep their class (a
   hand-placed titanium stays titanium) and are never reactors or already
   flagged. A flagged brick looks like a normal brick of its class.
3. N is capped at the number of bricks, so a large N is never an error.

The remaining `?` cells get weighted-random classes (ceramic 30, titanium 20,
tungsten 12, explosive 16 split over the three variants, regen 12, shield 10).
Among the `?` cells, the game makes sure at least one of every class and
explosive variant appears, as far as their number allows (always, with at least
N + 8 `?` cells).

## Legend

`legend:` starts a block of symbol definitions. Each line is:

```
<symbol> = <class> [hits=N] [powerup]
```

- `<symbol>` is one character other than `.`, `#` or `=`, and not whitespace.
  An entry can define a new symbol or override a built-in one for this file.
  `.` always means empty.
- `<class>` is one of: `ceramic`, `titanium`, `tungsten`, `explosive:charge`,
  `explosive:breach`, `explosive:demolition`, `regen`, `shield`, `reactor`,
  `random`.
- `hits=N` (N is 1-9) sets how many hits the brick takes, instead of the class
  default. Regen bricks heal back to this number.
- `powerup` makes the brick drop a power-up when it breaks (kind picked from the
  weighted power-up registry). Reactor bricks always drop one.

### Built-in symbols

| Symbol | Brick | Default hits |
|---|---|---|
| `C` | ceramic | 1 |
| `T` | titanium | 2 |
| `G` | tungsten (gold) | 3 |
| `X` | explosive charge | 1 |
| `B` | explosive breach | 1 |
| `D` | explosive demolition | 1 |
| `R` | regen | 2 |
| `S` | shield glass | 1 |
| `P` | power-up (reactor) | 2 |
| `?` | weighted-random class | its class's |
| `.` | empty | - |

## Grid

`grid:` starts the board; every non-blank line after it is a row, top to bottom.

- 1-10 rows, and every row has the same width, 1-10 columns.
- At least one brick. Any class may appear, including shield glass.
- Rows are trimmed, so indentation is fine. A space inside a row is an error.
- Blank lines and `#` comments (whole-line or after a row) are ignored.
- The grid is centred horizontally under the top wall, at the usual top offset.
  Bricks keep their size, so a narrower grid leaves wider side channels.

## Errors

A file with a mistake is rejected. The game logs an error naming the file, the
line and the column, and the run plays the built-in random board instead. It
never crashes. For example:

```
Failed to load asset 'levels/01-random.level' with asset loader '...LevelLoader': line 9, column 3: unknown symbol 'Z'
```

| Problem | Message (after `line L, column C:`) |
|---|---|
| Unknown symbol in the grid | `unknown symbol 'Z'` |
| Rows of different widths | `row is 8 wide, expected 10 (every row must be the same width)` |
| More than 10 rows | `too many rows (max 10)` |
| A row wider than 10 | `row is 11 wide (max 10)` |
| Bad legend entry (symbol, class, `hits=`, unknown attribute) | `bad legend entry: unknown class 'glass'` |
| Bad `ball_speed` / `powerups` value | `bad value 'fast' for ball_speed` |
| Unknown key | `unknown key 'speed'` |
| No `grid:` or no rows after it | `no grid: section with at least one row` |
| Only `.` cells | `the grid has no bricks` |

Lines and columns count from 1; columns count characters of the original line.

## Hot reload (native)

With `cargo run`, saving a level file (or `campaign.txt`) while the game runs
reloads it. The change applies the next time a run starts (Start or Play
again), never to the board in play. Saving an invalid file logs the error, and
the next run falls back to the random board until the file is fixed.
