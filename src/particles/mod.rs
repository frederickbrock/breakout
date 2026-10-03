//! The VFX layer: bevy_enoki particles for brick damage.
//!
//! **Gameplay fires events, particles observe.** Gameplay code never touches
//! particles. It triggers `BrickDamaged` / `BrickDestroyed` and changes
//! `BrickHealth`, and this module reacts:
//!
//! | trigger | effect file (`assets/particles/`) | colour |
//! |---|---|---|
//! | `BrickDamaged` | the class's `<class>_hit` burst at the contact point | its own `color_curve` |
//! | `BrickHealth` below max | child emitters: `brick_damage_smoke` + `brick_damage_sparks`, heavier with more damage | smoke grey / class glow |
//! | `BrickHealth` back at max (regen heal) | those child emitters removed | — |
//! | `BrickDestroyed` | the class's `<class>_break` burst at the brick's centre | its own `color_curve` |
//! | the ball, while not `Anchored` | child emitter `ball_trail`, left behind in world space | `BALL_TRAIL` |
//! | `BallBounced` | `ball_bounce` spark burst at the contact point | `BOUNCE_SPARK` (`EMITTER` off the paddle) |
//! | `PaddleHit` | two `paddle_flare` spawners on the paddle, one running to each end | `EMITTER` |
//! | `BrickExploded` | the variant's blast ([`blast_parts`]): charge round, breach a "+" of four jets, demolition big + debris + lingering smoke | `BLAST_RED` / `BLAST_ORANGE` / `BLAST_DEBRIS` / smoke |
//! | `ShieldDeflected` | `glass_glint` burst at the contact point | `GLASS_GLINT` + `GLASS_GLINT_LIGHT` |
//!
//! **Per-class hit and break bursts** (sim-rdl.7.8). Each brick material has
//! its own pair of effect files and its own particle sheet:
//! - `<class>_hit.particle.ron` and `<class>_break.particle.ron`
//! - the sheet `<class>.png`, a 4-frame 128×32 greyscale strip animated over
//!   each particle's life
//!
//! `<class>` is [`material_slug`]: `ceramic`, `titanium`, `tungsten`,
//! `reactor`, `explosive` (all three variants share it), `regen` or
//! `shield`. These files colour themselves with their `color_curve`, and the
//! sheet is drawn through a `SpriteParticle2dMaterial`. [`burst_look`]
//! picks the fallbacks:
//! - a sheet that isn't loaded (missing file): the same effect as plain
//!   quads (a white `ColorParticle2dMaterial`, so the curve's colour shows)
//! - an effect file that failed to load: pdp.1's generic `brick_hit` /
//!   `brick_break`, tinted by class
//!
//! The damage emitters and the generic effects are drawn white, and the
//! colour comes from the spawner's `ColorParticle2dMaterial` (one per class
//! and role, [`ParticleMaterials`]), which multiplies it. Editing a
//! `.particle.ron` while `cargo run` is live hot-reloads it.
//!
//! Two plugins:
//! - [`ParticlesPlugin`] (from `main()`; needs the renderer) adds
//!   `EnokiPlugin`, loads the effects and materials, and freezes particles
//!   while paused by pausing `Time<Virtual>` (bevy_enoki ticks on it) from
//!   `OnEnter(PlayState::Paused)` to `OnExit`.
//! - [`VfxPlugin`] (in `add_game`) spawns the spawners. It's a no-op without
//!   the resources above, so the headless tests run it with placeholder
//!   handles and count spawner entities.
//!
//! Every spawner is scoped to the run (a brick child, or
//! `DespawnOnExit(AppState::InGame)`), so leaving the run leaves none. A live
//! particle budget ([`damage_emitter_interval`]) slows the continuous
//! emitters when many bricks are damaged.
//!
//! The paddle flare reaches the paddle's end whatever its width
//! (Super-Sizer): each hit adds a copy of the flare effect with its speed and
//! particle count set from the distance to that end ([`flare_reaches`],
//! [`tuned_flare`]); the copy is freed with its spawner.
//!
//! The budget is [`DAMAGE_PARTICLE_BUDGET`], applied by stretching the damage
//! emitters' spawn interval rather than via `max_particles` (bevy_enoki stops
//! *moving* a spawner's particles once it's at that cap). Effect files are in
//! world units (sizes and speeds already × `GAME_SCALE`), and the
//! `brick_break` shatter falls with gravity. Pausing `Time<Virtual>` is safe
//! because gameplay gates on `PlayState::Playing` and physics has its own
//! clock. Headless tests insert placeholder `ParticleEffects` /
//! `ParticleMaterials` and count spawner entities.

use crate::ball::{Anchored, Ball};
use crate::bricks::explosive::BrickExploded;
use crate::bricks::grid::{Brick, BrickHealth, BrickMaxHits};
use crate::bricks::{BrickClass, ExplosiveKind};
use crate::collision::{
    BallBounced, BounceSurface, BrickDamaged, BrickDestroyed, PaddleHit, ShieldDeflected,
};
use crate::game_state::{AppState, PlayState};
use crate::paddle::{Paddle, PADDLE_HEIGHT};
use crate::theme;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_enoki::prelude::*;

const HIT_PATH: &str = "particles/brick_hit.particle.ron";
const SMOKE_PATH: &str = "particles/brick_damage_smoke.particle.ron";
const SPARKS_PATH: &str = "particles/brick_damage_sparks.particle.ron";
const BREAK_PATH: &str = "particles/brick_break.particle.ron";
const TRAIL_PATH: &str = "particles/ball_trail.particle.ron";
const BOUNCE_PATH: &str = "particles/ball_bounce.particle.ron";
const FLARE_PATH: &str = "particles/paddle_flare.particle.ron";
const CHARGE_PATH: &str = "particles/blast_charge.particle.ron";
const BREACH_PATH: &str = "particles/blast_breach.particle.ron";
const DEMOLITION_PATH: &str = "particles/blast_demolition.particle.ron";
const DEBRIS_PATH: &str = "particles/blast_debris.particle.ron";
const BLAST_SMOKE_PATH: &str = "particles/blast_smoke.particle.ron";
const GLINT_PATH: &str = "particles/glass_glint.particle.ron";

/// In front of bricks (z 0) and the HUD's backdrop, behind menus.
const PARTICLE_Z: f32 = 0.6;
/// Upper bound on live particles from the continuous damage emitters, so a
/// board full of damaged bricks can't tank the frame rate (WSLg's software
/// renderer).
pub const DAMAGE_PARTICLE_BUDGET: f32 = 1500.0;
/// Seconds between damage-emitter waves at full damage (a brick on its last
/// hit); lighter damage emits proportionally less often.
const SMOKE_INTERVAL: f32 = 0.12;
const SPARKS_INTERVAL: f32 = 0.5;
/// Just behind the ball (local to it), so the trail never covers it.
const TRAIL_Z: f32 = -0.1;
/// A flare side shorter than this (the ball hit right at that end) is skipped.
const MIN_FLARE_REACH: f32 = 1.0;

/// Handles to the effect assets.
#[derive(Resource, Clone)]
pub struct ParticleEffects {
    pub hit: Handle<Particle2dEffect>,
    pub smoke: Handle<Particle2dEffect>,
    pub sparks: Handle<Particle2dEffect>,
    pub shatter: Handle<Particle2dEffect>,
    pub trail: Handle<Particle2dEffect>,
    pub bounce: Handle<Particle2dEffect>,
    pub flare: Handle<Particle2dEffect>,
    pub charge: Handle<Particle2dEffect>,
    pub breach: Handle<Particle2dEffect>,
    pub demolition: Handle<Particle2dEffect>,
    pub debris: Handle<Particle2dEffect>,
    pub blast_smoke: Handle<Particle2dEffect>,
    pub glint: Handle<Particle2dEffect>,
    /// Each class's own hit and break bursts (explosive variants share).
    pub class_hit: HashMap<BrickClass, Handle<Particle2dEffect>>,
    pub class_break: HashMap<BrickClass, Handle<Particle2dEffect>>,
}

/// A brick class's sheet image and the sprite material drawing it.
#[derive(Clone)]
pub struct ClassSheet {
    pub image: Handle<Image>,
    pub material: Handle<SpriteParticle2dMaterial>,
}

impl ParticleEffects {
    fn blast(&self, effect: BlastEffect) -> Handle<Particle2dEffect> {
        match effect {
            BlastEffect::Charge => &self.charge,
            BlastEffect::Breach => &self.breach,
            BlastEffect::Demolition => &self.demolition,
            BlastEffect::Debris => &self.debris,
            BlastEffect::Smoke => &self.blast_smoke,
        }
        .clone()
    }
}

/// One tint material per brick class and role, plus the smoke's, the
/// blasts', the glass glints' and the ball/paddle effects'.
#[derive(Resource, Clone)]
pub struct ParticleMaterials {
    pub glow: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub face: HashMap<BrickClass, Handle<ColorParticle2dMaterial>>,
    pub smoke: Handle<ColorParticle2dMaterial>,
    pub trail: Handle<ColorParticle2dMaterial>,
    pub bounce: Handle<ColorParticle2dMaterial>,
    pub flare: Handle<ColorParticle2dMaterial>,
    pub blast_red: Handle<ColorParticle2dMaterial>,
    pub blast_orange: Handle<ColorParticle2dMaterial>,
    pub debris: Handle<ColorParticle2dMaterial>,
    pub glint: Handle<ColorParticle2dMaterial>,
    pub glint_light: Handle<ColorParticle2dMaterial>,
    /// Each class's particle sheet, for its own hit and break bursts.
    pub sheets: HashMap<BrickClass, ClassSheet>,
    /// White: a class burst whose sheet is missing, drawn as plain quads in
    /// its `color_curve`'s colours.
    pub plain: Handle<ColorParticle2dMaterial>,
}

impl ParticleMaterials {
    fn glow(&self, class: BrickClass) -> Handle<ColorParticle2dMaterial> {
        self.glow.get(&class).cloned().unwrap_or_default()
    }

    fn face(&self, class: BrickClass) -> Handle<ColorParticle2dMaterial> {
        self.face.get(&class).cloned().unwrap_or_default()
    }

    fn blast(&self, tint: BlastTint) -> Handle<ColorParticle2dMaterial> {
        match tint {
            BlastTint::Red => &self.blast_red,
            BlastTint::Orange => &self.blast_orange,
            BlastTint::Debris => &self.debris,
            BlastTint::Smoke => &self.smoke,
        }
        .clone()
    }
}

/// Every brick class, for building the per-class materials.
fn all_classes() -> [BrickClass; 9] {
    use crate::bricks::ExplosiveKind::*;
    [
        BrickClass::Ceramic,
        BrickClass::Titanium,
        BrickClass::Tungsten,
        BrickClass::Reactor,
        BrickClass::Explosive(Charge),
        BrickClass::Explosive(Breach),
        BrickClass::Explosive(Demolition),
        BrickClass::Regen,
        BrickClass::Shield,
    ]
}

/// Frames in each class sheet (one row).
const SHEET_FRAMES: u32 = 4;

/// The name a brick class's particle files go by. The three explosive
/// variants share `explosive`.
pub fn material_slug(class: BrickClass) -> &'static str {
    match class {
        BrickClass::Ceramic => "ceramic",
        BrickClass::Titanium => "titanium",
        BrickClass::Tungsten => "tungsten",
        BrickClass::Reactor => "reactor",
        BrickClass::Explosive(_) => "explosive",
        BrickClass::Regen => "regen",
        BrickClass::Shield => "shield",
    }
}

/// `particles/<class>_hit.particle.ron`.
pub fn class_hit_path(class: BrickClass) -> String {
    format!("particles/{}_hit.particle.ron", material_slug(class))
}

/// `particles/<class>_break.particle.ron`.
pub fn class_break_path(class: BrickClass) -> String {
    format!("particles/{}_break.particle.ron", material_slug(class))
}

/// `particles/<class>.png`, the class's 4-frame sheet.
pub fn class_sheet_path(class: BrickClass) -> String {
    format!("particles/{}.png", material_slug(class))
}

/// How a class hit/break burst is drawn, given whether its effect file
/// failed to load and whether its sheet image is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurstLook {
    /// The class's own effect, textured with its sheet.
    Textured,
    /// The class's own effect as plain quads (its sheet is missing).
    PlainQuads,
    /// pdp.1's generic effect, tinted by class (its effect file is missing).
    Generic,
}

pub fn burst_look(effect_failed: bool, sheet_loaded: bool) -> BurstLook {
    match (effect_failed, sheet_loaded) {
        (true, _) => BurstLook::Generic,
        (false, true) => BurstLook::Textured,
        (false, false) => BurstLook::PlainQuads,
    }
}

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EnokiPlugin)
            .add_systems(Startup, load_effects)
            .add_systems(OnEnter(PlayState::Paused), freeze_particles)
            .add_systems(OnExit(PlayState::Paused), thaw_particles);
    }
}

fn load_effects(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorParticle2dMaterial>>,
    mut sprite_materials: ResMut<Assets<SpriteParticle2dMaterial>>,
) {
    let per_class = |path: fn(BrickClass) -> String| {
        all_classes()
            .into_iter()
            .map(|class| (class, assets.load(path(class))))
            .collect()
    };
    commands.insert_resource(ParticleEffects {
        hit: assets.load(HIT_PATH),
        smoke: assets.load(SMOKE_PATH),
        sparks: assets.load(SPARKS_PATH),
        shatter: assets.load(BREAK_PATH),
        trail: assets.load(TRAIL_PATH),
        bounce: assets.load(BOUNCE_PATH),
        flare: assets.load(FLARE_PATH),
        charge: assets.load(CHARGE_PATH),
        breach: assets.load(BREACH_PATH),
        demolition: assets.load(DEMOLITION_PATH),
        debris: assets.load(DEBRIS_PATH),
        blast_smoke: assets.load(BLAST_SMOKE_PATH),
        glint: assets.load(GLINT_PATH),
        class_hit: per_class(class_hit_path),
        class_break: per_class(class_break_path),
    });
    let sheets = all_classes()
        .into_iter()
        .map(|class| {
            let image: Handle<Image> = assets.load(class_sheet_path(class));
            let material = sprite_materials.add(SpriteParticle2dMaterial::new(
                image.clone(),
                SHEET_FRAMES,
                1,
            ));
            (class, ClassSheet { image, material })
        })
        .collect();
    let mut tint = |color: Color| materials.add(ColorParticle2dMaterial::new(color.into()));
    let glow = all_classes()
        .into_iter()
        .map(|class| (class, tint(theme::brick_glow(class))))
        .collect();
    let face = all_classes()
        .into_iter()
        .map(|class| (class, tint(theme::brick_color(class))))
        .collect();
    commands.insert_resource(ParticleMaterials {
        glow,
        face,
        smoke: tint(theme::SMOKE),
        trail: tint(theme::BALL_TRAIL),
        bounce: tint(theme::BOUNCE_SPARK),
        flare: tint(theme::EMITTER),
        blast_red: tint(theme::BLAST_RED),
        blast_orange: tint(theme::BLAST_ORANGE),
        debris: tint(theme::BLAST_DEBRIS),
        glint: tint(theme::GLASS_GLINT),
        glint_light: tint(theme::GLASS_GLINT_LIGHT),
        sheets,
        plain: tint(theme::UNTINTED),
    });
}

/// bevy_enoki moves particles on `Time<Virtual>`; stopping it while paused
/// freezes every particle and spawner. (Gameplay systems gate on
/// `PlayState::Playing` and physics has its own clock, so nothing else
/// depends on virtual time advancing while paused.)
fn freeze_particles(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn thaw_particles(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

/// Marks a damaged brick's continuous emitter (a child of the brick).
#[derive(Component)]
pub struct DamageEmitter {
    role: EmitterRole,
    /// Fraction of the brick's hits taken, in (0, 1].
    damage: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EmitterRole {
    Smoke,
    Sparks,
}

/// Seconds between a damage emitter's spawn waves. It emits more often the
/// more damage the brick has taken (`damage` in (0, 1]), but never so often
/// that `emitters` of them would exceed [`DAMAGE_PARTICLE_BUDGET`] live
/// particles, given each wave's `particles_per_wave` and `lifetime`.
pub fn damage_emitter_interval(
    base: f32,
    damage: f32,
    emitters: usize,
    particles_per_wave: u32,
    lifetime: f32,
) -> f32 {
    let wanted = base / damage.max(0.05);
    // Live particles per emitter ≈ per_wave × lifetime / interval.
    let budgeted = particles_per_wave as f32 * lifetime * emitters as f32 / DAMAGE_PARTICLE_BUDGET;
    wanted.max(budgeted)
}

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spark_on_damage)
            .add_observer(shatter_on_break)
            .add_observer(spark_on_bounce)
            .add_observer(flare_on_paddle_hit)
            .add_observer(blast_on_explosion)
            .add_observer(glint_on_deflect)
            .add_systems(
                Update,
                (
                    (sync_damage_emitters, tune_damage_emitters).chain(),
                    attach_ball_trail,
                )
                    .run_if(resource_exists::<ParticleEffects>)
                    .run_if(resource_exists::<ParticleMaterials>),
            )
            .add_systems(Update, sync_ball_trail.after(attach_ball_trail));
    }
}

fn burst<M: Particle2dMaterial>(
    material: Handle<M>,
    effect: Handle<Particle2dEffect>,
    position: Vec2,
) -> impl Bundle {
    burst_at(
        material,
        effect,
        Transform::from_translation(position.extend(PARTICLE_Z)),
    )
}

/// A one-shot burst placed (and turned) by `transform`.
fn burst_at<M: Particle2dMaterial>(
    material: Handle<M>,
    effect: Handle<Particle2dEffect>,
    transform: Transform,
) -> impl Bundle {
    (
        ParticleSpawner(material),
        ParticleEffectHandle(effect),
        OneShot::Despawn,
        transform,
        DespawnOnExit(AppState::InGame),
    )
}

/// Which of a class's bursts to play.
#[derive(Clone, Copy)]
enum Burst {
    Hit,
    Break,
}

/// What a class burst needs to see whether its files loaded. Absent in
/// headless tests (no asset server state, no images).
type LoadInfo<'a> = (Option<Res<'a, AssetServer>>, Option<Res<'a, Assets<Image>>>);

/// Plays `class`'s own hit or break burst at `position`, or a fallback
/// ([`burst_look`]).
fn spawn_class_burst(
    commands: &mut Commands,
    effects: &ParticleEffects,
    materials: &ParticleMaterials,
    (server, images): &LoadInfo,
    class: BrickClass,
    which: Burst,
    position: Vec2,
) {
    let (own, generic, tint) = match which {
        Burst::Hit => (&effects.class_hit, &effects.hit, materials.glow(class)),
        Burst::Break => (
            &effects.class_break,
            &effects.shatter,
            materials.face(class),
        ),
    };
    let own = own.get(&class);
    let failed = own.is_none_or(|handle| {
        server
            .as_ref()
            .is_some_and(|server| server.get_load_state(handle).is_some_and(|s| s.is_failed()))
    });
    let sheet = materials.sheets.get(&class);
    let sheet_loaded = sheet.is_some_and(|sheet| {
        images
            .as_ref()
            .is_some_and(|images| images.contains(&sheet.image))
    });
    match (burst_look(failed, sheet_loaded), own, sheet) {
        (BurstLook::Textured, Some(own), Some(sheet)) => {
            commands.spawn(burst(sheet.material.clone(), own.clone(), position));
        }
        (BurstLook::PlainQuads, Some(own), _) => {
            commands.spawn(burst(materials.plain.clone(), own.clone(), position));
        }
        _ => {
            commands.spawn(burst(tint, generic.clone(), position));
        }
    }
}

fn spark_on_damage(
    on: On<BrickDamaged>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
    load: LoadInfo,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    let (class, position) = (on.class, on.position);
    spawn_class_burst(
        &mut commands,
        &effects,
        &materials,
        &load,
        class,
        Burst::Hit,
        position,
    );
}

fn shatter_on_break(
    on: On<BrickDestroyed>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
    load: LoadInfo,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    let (class, position) = (on.class, on.position);
    spawn_class_burst(
        &mut commands,
        &effects,
        &materials,
        &load,
        class,
        Burst::Break,
        position,
    );
}

fn spark_on_bounce(
    on: On<BallBounced>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    // Off the paddle the sparks take its cyan, to sit with the flare.
    let material = match on.surface {
        BounceSurface::Paddle => materials.flare.clone(),
        BounceSurface::Wall | BounceSurface::Brick => materials.bounce.clone(),
    };
    commands.spawn(burst(material, effects.bounce.clone(), on.position));
}

/// Marks the ball's trail emitter (a child of the ball).
#[derive(Component)]
pub struct BallTrail;

/// The ball already has its [`BallTrail`].
#[derive(Component)]
struct HasTrail;

/// Gives a ball without one its trail emitter, starting inactive;
/// [`sync_ball_trail`] turns it on while the ball is in flight.
fn attach_ball_trail(
    mut commands: Commands,
    effects: Res<ParticleEffects>,
    materials: Res<ParticleMaterials>,
    balls: Query<Entity, (With<Ball>, Without<HasTrail>)>,
) {
    for ball in &balls {
        commands.entity(ball).insert(HasTrail).with_child((
            BallTrail,
            ParticleSpawner(materials.trail.clone()),
            ParticleEffectHandle(effects.trail.clone()),
            ParticleSpawnerState {
                active: false,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, TRAIL_Z),
        ));
    }
}

/// The trail emits only while its ball is in flight (not `Anchored`). Its
/// particles live in world space, so they fade where they were left.
fn sync_ball_trail(
    mut trails: Query<(&mut ParticleSpawnerState, &ChildOf), With<BallTrail>>,
    balls: Query<Has<Anchored>, With<Ball>>,
) {
    for (mut state, child_of) in &mut trails {
        let Ok(anchored) = balls.get(child_of.parent()) else {
            continue;
        };
        if state.active == anchored {
            state.active = !anchored;
        }
    }
}

/// One side of a paddle flare (a child of the paddle); the left one is
/// rotated half a turn.
#[derive(Component)]
pub struct PaddleFlare;

/// How far a flare from `hit_x` runs to each end of a paddle `width` wide
/// centred at `paddle_x`, as `(side, reach)` for the left and right.
pub fn flare_reaches(hit_x: f32, paddle_x: f32, width: f32) -> [(f32, f32); 2] {
    let half = width / 2.0;
    [
        (-1.0, (hit_x - (paddle_x - half)).clamp(0.0, width)),
        (1.0, ((paddle_x + half) - hit_x).clamp(0.0, width)),
    ]
}

/// The flare effect tuned to run `reach` along the paddle: particles spread
/// evenly from the hit point out to `reach` (speed ×(1 ± randomness) over
/// the lifetime), with a count in proportion to the distance relative to
/// the half-paddle the file is drawn for.
pub fn tuned_flare(base: &Particle2dEffect, reach: f32) -> Particle2dEffect {
    let mut effect = base.clone();
    let lifetime = effect.lifetime.0.max(f32::EPSILON);
    // Rval speed is v·(1 ± r); with r ≈ 1 the fastest particle travels 2v.
    effect.linear_speed = Some(Rval(reach / (2.0 * lifetime), 0.95));
    let share = reach / (crate::paddle::PADDLE_WIDTH / 2.0);
    effect.spawn_amount = ((base.spawn_amount as f32 * share).round() as u32).max(2);
    effect
}

/// The ball hit the paddle: a cyan flare runs from the hit point to each end
/// of the paddle's top edge, riding along with the paddle.
fn flare_on_paddle_hit(
    on: On<PaddleHit>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
    mut assets: Option<ResMut<Assets<Particle2dEffect>>>,
    paddles: Query<(&Transform, &Paddle)>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    let Ok((transform, paddle)) = paddles.get(on.paddle) else {
        return;
    };
    let centre = transform.translation;
    for (side, reach) in flare_reaches(on.position.x, centre.x, paddle.width) {
        if reach < MIN_FLARE_REACH {
            continue;
        }
        // A copy of the effect tuned to this reach; without the loaded asset
        // (headless tests, or still loading) the file's own speed is used.
        let handle = match assets.as_deref_mut() {
            Some(assets) => match assets.get(&effects.flare).cloned() {
                Some(base) => assets.add(tuned_flare(&base, reach)),
                None => effects.flare.clone(),
            },
            None => effects.flare.clone(),
        };
        // Local to the paddle, on its top edge; the left side is turned
        // round so the file's rightward direction runs left.
        let local = Vec3::new(on.position.x - centre.x, PADDLE_HEIGHT / 2.0, PARTICLE_Z);
        let facing = if side < 0.0 {
            Quat::from_rotation_z(std::f32::consts::PI)
        } else {
            Quat::IDENTITY
        };
        commands.entity(on.paddle).with_child((
            PaddleFlare,
            ParticleSpawner(materials.flare.clone()),
            ParticleEffectHandle(handle),
            OneShot::Despawn,
            Transform::from_translation(local).with_rotation(facing),
        ));
    }
}

/// The effect files a blast is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlastEffect {
    Charge,
    Breach,
    Demolition,
    Debris,
    Smoke,
}

/// A blast part's tint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlastTint {
    Red,
    Orange,
    Debris,
    Smoke,
}

/// One spawner of a blast: its effect, its tint, and the angle (radians)
/// it's turned to (the effect file's direction is rotated by it).
pub type BlastPart = (BlastEffect, BlastTint, f32);

/// A `kind` explosion's blast, shaped like its rule: charge a round
/// red/orange burst over its 8 neighbours, breach four jets along the
/// orthogonals (a "+"), demolition a big burst with debris and lingering
/// smoke.
pub fn blast_parts(kind: ExplosiveKind) -> Vec<BlastPart> {
    use std::f32::consts::FRAC_PI_2;
    use BlastEffect as E;
    use BlastTint as T;
    match kind {
        ExplosiveKind::Charge => vec![(E::Charge, T::Red, 0.0), (E::Charge, T::Orange, 0.0)],
        ExplosiveKind::Breach => (0..4)
            .map(|quarter| {
                let tint = if quarter % 2 == 0 { T::Red } else { T::Orange };
                (E::Breach, tint, quarter as f32 * FRAC_PI_2)
            })
            .collect(),
        ExplosiveKind::Demolition => vec![
            (E::Demolition, T::Red, 0.0),
            (E::Demolition, T::Orange, 0.0),
            (E::Debris, T::Debris, 0.0),
            (E::Smoke, T::Smoke, 0.0),
        ],
    }
}

/// Marks a blast spawner.
#[derive(Component)]
pub struct BlastBurst;

/// Each explosion, the chain's included (one `BrickExploded` per exploding
/// brick, in chain order), plays its variant's blast where it went off.
fn blast_on_explosion(
    on: On<BrickExploded>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    for (effect, tint, angle) in blast_parts(on.kind) {
        let at = Transform::from_translation(on.position.extend(PARTICLE_Z))
            .with_rotation(Quat::from_rotation_z(angle));
        commands.spawn((
            BlastBurst,
            burst_at(materials.blast(tint), effects.blast(effect), at),
        ));
    }
}

/// Marks a glass-glint spawner.
#[derive(Component)]
pub struct GlassGlint;

/// Shield glass deflected the ball: cyan glints at the contact point, in
/// its two glass tones.
fn glint_on_deflect(
    on: On<ShieldDeflected>,
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    materials: Option<Res<ParticleMaterials>>,
) {
    let (Some(effects), Some(materials)) = (effects, materials) else {
        return;
    };
    for material in [&materials.glint, &materials.glint_light] {
        commands.spawn((
            GlassGlint,
            burst(material.clone(), effects.glint.clone(), on.position),
        ));
    }
}

/// Bricks whose health changed this frame.
type HealthChanged = (With<Brick>, Changed<BrickHealth>);

/// Keeps each brick's damage emitters in line with its health: added when it
/// drops below full, updated as it takes more, removed when it's back to
/// full (regen).
fn sync_damage_emitters(
    mut commands: Commands,
    effects: Res<ParticleEffects>,
    materials: Res<ParticleMaterials>,
    bricks: Query<
        (
            Entity,
            &BrickClass,
            &BrickHealth,
            &BrickMaxHits,
            Option<&Children>,
        ),
        HealthChanged,
    >,
    mut emitters: Query<&mut DamageEmitter>,
) {
    for (brick, &class, health, max_hits, children) in &bricks {
        let max = max_hits.0;
        let existing: Vec<Entity> = children
            .into_iter()
            .flatten()
            .copied()
            .filter(|child| emitters.contains(*child))
            .collect();
        if health.0 == 0 || health.0 >= max {
            for emitter in existing {
                commands.entity(emitter).despawn();
            }
            continue;
        }
        let damage = f32::from(max - health.0) / f32::from(max);
        if existing.is_empty() {
            // Local to the brick, just in front of it.
            let at = Transform::from_xyz(0.0, 0.0, PARTICLE_Z);
            commands.entity(brick).with_children(|brick| {
                brick.spawn((
                    DamageEmitter {
                        role: EmitterRole::Smoke,
                        damage,
                    },
                    ParticleSpawner(materials.smoke.clone()),
                    ParticleEffectHandle(effects.smoke.clone()),
                    at,
                ));
                brick.spawn((
                    DamageEmitter {
                        role: EmitterRole::Sparks,
                        damage,
                    },
                    ParticleSpawner(materials.glow(class)),
                    ParticleEffectHandle(effects.sparks.clone()),
                    at,
                ));
            });
        } else {
            for emitter in existing {
                if let Ok(mut emitter) = emitters.get_mut(emitter) {
                    emitter.damage = damage;
                }
            }
        }
    }
}

/// Sets each damage emitter's spawn interval from its brick's damage and the
/// particle budget. Runs every frame because bevy_enoki re-copies the effect
/// (and so the file's `spawn_rate`) on hot reload.
fn tune_damage_emitters(mut emitters: Query<(&DamageEmitter, &mut ParticleEffectInstance)>) {
    let count = emitters.iter().count();
    for (emitter, mut instance) in &mut emitters {
        let Some(effect) = instance.0.as_mut() else {
            continue;
        };
        let base = match emitter.role {
            EmitterRole::Smoke => SMOKE_INTERVAL,
            EmitterRole::Sparks => SPARKS_INTERVAL,
        };
        let lifetime = effect.lifetime.0 * (1.0 + effect.lifetime.1);
        let interval =
            damage_emitter_interval(base, emitter.damage, count, effect.spawn_amount, lifetime);
        if (effect.spawn_rate - interval).abs() > f32::EPSILON {
            effect.spawn_rate = interval;
        }
    }
}

#[cfg(test)]
mod tests;
