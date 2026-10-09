# Vision and feel briefs

This folder holds the owner's taste, so the specs the pm writes carry it.
The pm reads it before it specs any polish or "feel" work, and asks instead
of inventing taste that isn't written here.

- [`vision.md`](vision.md): the game's pillars, target feel, references and
  anti-goals. One page and slow-changing.
- [`feel-brief-template.md`](feel-brief-template.md): copy it to write a
  brief for one moment (a brick breaking, the paddle hit...).
- [`briefs/`](briefs/): one file per moment. The three there now are stubs to
  rewrite:
  [paddle hit](briefs/paddle-hit.md), [brick break](briefs/brick-break.md),
  [ball trail](briefs/ball-trail.md).

## How the lane works

1. **You write or edit a brief** in `briefs/`, starting from the template.
   Rough is fine; the levers and checks matter most.
2. **You ask the pm** to spec it: "spec the brick-break brief".
3. **The pm reads `docs/vision/`**, interviews you on anything the brief
   leaves open, and turns the brief into a small epic at `stage:spec`. Each
   acceptance criterion uses the brief's *concrete levers*, with numbers:
   "shake 6 px for 80 ms", not "feels punchy".
4. **You approve** at the usual gate. The code team builds it from there.

If a brief has no number for a lever, the pm asks you. It doesn't guess.
