# Breakout

A Breakout clone built with [Bevy](https://bevyengine.org/) 0.19 and
[Avian2D](https://github.com/Jondolf/avian) physics. The ball, paddle, and bricks are
real rigid bodies, and power-ups (such as Super-Sizer, which widens the paddle) drop from
destroyed bricks.

**Play in the browser:** https://frederickbrock.github.io/breakout/

## Building

The web build is the primary target.

### Web (wasm)

One-time setup:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

Then:

```sh
trunk serve          # dev server at http://localhost:8080
trunk build --release  # output in dist/
```

### Native desktop

```sh
cargo run              # dev
cargo build --release  # release
```

On Linux you need Bevy's system dependencies, e.g. on Debian/Ubuntu:

```sh
sudo apt-get install libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
```

## CI

`.github/workflows/build.yml` builds native and web on every pull request and push to
`master`, and deploys the web build to GitHub Pages from `master`.

## License

[MIT](LICENSE)
