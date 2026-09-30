use super::*;

#[test]
fn walls_sit_on_the_playfield_edges() {
    assert_eq!(SIDE_PANEL_WIDTH, 240.0);
    assert_eq!(PLAYFIELD_WIDTH + 2.0 * SIDE_PANEL_WIDTH, WORLD_WIDTH);
    assert_eq!(PLAYFIELD_HEIGHT, WORLD_HEIGHT);
    let [left, right, top] = wall_specs();
    assert_eq!(left.0.x + left.1.x / 2.0, -PLAYFIELD_WIDTH / 2.0);
    assert_eq!(right.0.x - right.1.x / 2.0, PLAYFIELD_WIDTH / 2.0);
    assert_eq!(top.0.y - top.1.y / 2.0, PLAYFIELD_HEIGHT / 2.0);
    // The well is centred and the walls close its corners.
    assert_eq!(left.0.x, -right.0.x);
    assert_eq!(top.0.x, 0.0);
    assert_eq!(
        left.1,
        Vec2::new(WALL_THICKNESS, PLAYFIELD_HEIGHT + 2.0 * WALL_THICKNESS)
    );
    assert_eq!(
        top.1,
        Vec2::new(PLAYFIELD_WIDTH + 2.0 * WALL_THICKNESS, WALL_THICKNESS)
    );
}
