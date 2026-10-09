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
- A run plays the levels in manifest order; clearing one shows a ~2 s
  `SECTOR NN // name` card and starts the next; clearing the last wins. A level
  that fails to load is skipped (logged); with none valid the run plays the
  built-in random board.
- The game has a built-in random board (7 rows × 10 of `?`, `powerups: 6`)
  and falls back to it whenever there is no valid level.
- The shipped campaign is the galaxy campaign: 25 planet levels in five
  galaxies of five (Milky Way, Andromeda, Triangulum, Large Magellanic Cloud,
  Sombrero), each group under a `# Galaxy: <name>` comment in `campaign.txt`.
  See [Galaxy campaign](#galaxy-campaign).

On the web, trunk copies the whole `assets/` folder into `dist/`
(`index.html`'s `copy-dir`), so `assets/levels/` ships with the build.

## Example

```
# comments start with #
name: Rainbow Bands
speed_factor: 1.5      # optional; ball speed = 300 x factor; default ramps 1.8, 1.9, ... 2.5 per round
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
| `speed_factor` | the round's ramp value (`speed_factor(round)` in `src/ball.rs`): 1.8 in round 1, +0.1 per round, capped at 2.5 from round 8 | The ball's speed factor: the ball moves at 300 × factor world units per second, so round 1's 1.8 is 540, round 2's 1.9 is 570, the 2.5 cap is 750, and `2.0` is 600. Setting it fixes this level's speed whatever its round. Must be a number greater than 0. It is independent of `GAME_SCALE`. There is no upper limit, but very high speeds can make the ball pass through bricks. |
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
line and the column, and the campaign skips that level (with no valid level
left, the run plays the built-in random board). It never crashes. For example:

```
Failed to load asset 'levels/01-milky-way-1-mercury.level' with asset loader '...LevelLoader': line 9, column 3: unknown symbol 'Z'
```

| Problem | Message (after `line L, column C:`) |
|---|---|
| Unknown symbol in the grid | `unknown symbol 'Z'` |
| Rows of different widths | `row is 8 wide, expected 10 (every row must be the same width)` |
| More than 10 rows | `too many rows (max 10)` |
| A row wider than 10 | `row is 11 wide (max 10)` |
| Bad legend entry (symbol, class, `hits=`, unknown attribute) | `bad legend entry: unknown class 'glass'` |
| Bad `speed_factor` / `powerups` value | `bad value 'fast' for speed_factor` |
| Unknown key | `unknown key 'speed'` |
| No `grid:` or no rows after it | `no grid: section with at least one row` |
| Only `.` cells | `the grid has no bricks` |

Lines and columns count from 1; columns count characters of the original line.

## Galaxy campaign

Each level is a planet, named by `name:`. Files are
`<gg>-<galaxy-slug>-<n>-<planet-slug>.level` (e.g. `01-milky-way-3-earth.level`).
The Milky Way's five are real Solar System planets; the other galaxies' are
invented, each with a plausible element profile.

A planet's bricks come from the elements it is most likely made of:

| Element group (examples) | Brick |
|---|---|
| rock, silicates, clay, carbonates | `ceramic` (`C`) |
| iron, nickel, titanium, iron oxide | `titanium` (`T`) |
| dense or heavy metals: tungsten, platinum group, metallic core | `tungsten` (`G`) |
| silica sand, water/ice, glassy crust | `shield` glass (`S`) |
| volatiles: light gas (hydrogen, methane, ammonia, CO2 frost) | `explosive` charge (`X`) |
| volatiles: sulfur, sulfuric acid | `explosive` breach (`B`) |
| volatiles: gas under deep pressure (metallic hydrogen, dense CO2) | `explosive` demolition (`D`) |
| radioactives: uranium, thorium, a hot core | `reactor` (`P`, drops a power-up) |
| organics, life, biofilm | `regen` alloy (`R`) |

Refinements every level follows:

- A denser or older layer of the same element is the same class with more
  hits, via a legend entry: pressure-hardened rock `ceramic hits=2`, a dense
  iron mantle `titanium hits=3`, the compressed heart of a core
  `tungsten hits=5`, ancient growth `regen hits=3`.
- `?` stands for mixed material (regolith, dust, debris) and is at most 20% of
  a level's bricks, so the theme shows.

Every level file:

- starts with a comment naming the planet's main elements and the brick each
  became;
- sets `powerups:` and no `speed_factor` (the speed ramp belongs to the galaxy
  structure);
- is harder than the one before it in its galaxy (more hits, shields, tougher
  layouts).

Each galaxy has at least one level with deliberate negative space: ≥25% of the
cells are `.`, and some row has a run of 2+ `.` with bricks on both sides (a
ring, a silhouette, channels, a hollow core). `src/levels/tests.rs` checks
these rules on every shipped level.

## Hot reload (native)

With `cargo run`, saving a level file (or `campaign.txt`) while the game runs
reloads it. An edited level applies the next time it starts (at the next run,
or when the run reaches it), never to the board in play. Saving an invalid file
logs the error, and that level is skipped until the file is fixed (with no valid
level left, runs play the random board).
