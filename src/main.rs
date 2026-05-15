use bevy::input::mouse::MouseMotion;
use bevy::math::primitives::{Capsule3d, Cuboid, Plane3d, Sphere};
use bevy::prelude::*;
use rand::prelude::*;

const PLAYER_SPEED: f32 = 8.0;
const BULLET_SPEED: f32 = 35.0;
const BULLET_LIFETIME: f32 = 1.8;
const SHOOT_COOLDOWN: f32 = 0.2;
const TOWER_HP: f32 = 300.0;
const PLAYER_HP: f32 = 100.0;
const TOWER_RADIUS: f32 = 2.5;
const TOWER_FIRE_RANGE: f32 = 18.0;
const TOWER_FIRE_COOLDOWN: f32 = 0.8;
const ARENA_SIZE: f32 = 45.0;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.09)))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 350.0,
        })
        .insert_resource(MouseSettings::default())
        .insert_resource(RespawnTimer(Timer::from_seconds(2.5, TimerMode::Repeating)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BrawlRust Prototype".into(),
                resolution: (1440., 900.).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                player_look,
                player_movement,
                player_shoot,
                bot_ai,
                tower_ai,
                bullet_movement,
                bullet_hits,
                update_timers,
                respawn_dead_players,
            ),
        )
        .run();
}

#[derive(Component)]
struct Player {
    id: usize,
    team: usize,
    hp: f32,
    is_human: bool,
    alive: bool,
}

#[derive(Component)]
struct ShootTimer(Timer);

#[derive(Component)]
struct Bot;

#[derive(Component)]
struct Tower {
    team: usize,
    hp: f32,
}

#[derive(Component)]
struct TowerShootTimer(Timer);

#[derive(Component)]
struct Bullet {
    direction: Vec3,
    team: usize,
    damage: f32,
    ttl: Timer,
}

#[derive(Component)]
struct MainCamera;

#[derive(Resource, Default)]
struct MouseSettings {
    yaw: f32,
    pitch: f32,
}

#[derive(Resource)]
struct RespawnTimer(Timer);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::default().mesh().size(ARENA_SIZE * 2.0, ARENA_SIZE * 2.0)),
        material: materials.add(Color::srgb(0.15, 0.18, 0.17)),
        ..default()
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 25000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(30.0, 45.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let tower_positions = [
        Vec3::new(-25.0, 1.8, -25.0),
        Vec3::new(25.0, 1.8, -25.0),
        Vec3::new(-25.0, 1.8, 25.0),
        Vec3::new(25.0, 1.8, 25.0),
    ];

    for (team, pos) in tower_positions.into_iter().enumerate() {
        commands
            .spawn(PbrBundle {
                mesh: meshes.add(Cuboid::new(4.0, 4.0, 4.0)),
                material: materials.add(team_color(team).mix(&Color::WHITE, 0.2)),
                transform: Transform::from_translation(pos),
                ..default()
            })
            .insert(Tower { team, hp: TOWER_HP })
            .insert(TowerShootTimer(Timer::from_seconds(TOWER_FIRE_COOLDOWN, TimerMode::Repeating)));
    }

    for i in 0..12 {
        let team = i / 3;
        let start = spawn_point_for_team(team) + Vec3::new((i % 3) as f32 * 1.5, 0.8, 0.0);

        let mut entity = commands.spawn(PbrBundle {
            mesh: meshes.add(Capsule3d::new(0.45, 1.2)),
            material: materials.add(team_color(team)),
            transform: Transform::from_translation(start),
            ..default()
        });

        entity.insert(Player {
            id: i,
            team,
            hp: PLAYER_HP,
            is_human: i == 0,
            alive: true,
        });
        entity.insert(ShootTimer(Timer::from_seconds(SHOOT_COOLDOWN, TimerMode::Repeating)));

        if i != 0 {
            entity.insert(Bot);
        }
    }

    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(0.0, 13.0, 13.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
            ..default()
        })
        .insert(MainCamera);
}

fn player_movement(
    kb: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut q_players: Query<(&mut Transform, &Player), Without<Bot>>,
    mut q_camera: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok((mut player_tf, player)) = q_players.get_single_mut() else { return };
    if !player.alive {
        return;
    }

    let Ok(mut cam_tf) = q_camera.get_single_mut() else { return };

    let mut input = Vec3::ZERO;
    if kb.pressed(KeyCode::KeyW) { input.z -= 1.0; }
    if kb.pressed(KeyCode::KeyS) { input.z += 1.0; }
    if kb.pressed(KeyCode::KeyA) { input.x -= 1.0; }
    if kb.pressed(KeyCode::KeyD) { input.x += 1.0; }

    if input.length_squared() > 0.0 {
        input = input.normalize();
    }

    let yaw = cam_tf.rotation;
    let forward = yaw * Vec3::new(0.0, 0.0, -1.0);
    let right = yaw * Vec3::X;
    let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let flat_right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

    let dir = flat_forward * input.z + flat_right * input.x;
    player_tf.translation += dir * PLAYER_SPEED * time.delta_seconds();
    clamp_to_arena(&mut player_tf.translation);

    cam_tf.translation = player_tf.translation + Vec3::new(0.0, 12.0, 12.0);
}

fn player_look(
    mut mouse_ev: EventReader<MouseMotion>,
    mouse_btn: Res<ButtonInput<MouseButton>>,
    mut settings: ResMut<MouseSettings>,
    mut q_cam: Query<&mut Transform, With<MainCamera>>,
    q_player: Query<&Transform, (With<Player>, Without<MainCamera>, Without<Bot>)>,
) {
    if !mouse_btn.pressed(MouseButton::Right) {
        return;
    }

    let mut delta = Vec2::ZERO;
    for ev in mouse_ev.read() {
        delta += ev.delta;
    }

    settings.yaw -= delta.x * 0.004;
    settings.pitch = (settings.pitch - delta.y * 0.002).clamp(-0.9, 0.2);

    let Ok(player_tf) = q_player.get_single() else { return };
    let Ok(mut cam_tf) = q_cam.get_single_mut() else { return };
    let radius = 15.0;
    let offset = Quat::from_axis_angle(Vec3::Y, settings.yaw)
        * Quat::from_axis_angle(Vec3::X, settings.pitch)
        * Vec3::new(0.0, 6.0, radius);

    cam_tf.translation = player_tf.translation + offset;
    cam_tf.look_at(player_tf.translation + Vec3::new(0.0, 0.8, 0.0), Vec3::Y);
}

fn player_shoot(
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q_players: Query<(&Transform, &Player, &mut ShootTimer), Without<Bot>>,
    q_camera: Query<&Transform, With<MainCamera>>,
) {
    let Ok((tf, player, mut timer)) = q_players.get_single_mut() else { return };
    timer.0.tick(time.delta());
    if !player.alive || !mouse.pressed(MouseButton::Left) || !timer.0.finished() {
        return;
    }

    let Ok(cam_tf) = q_camera.get_single() else { return };
    let dir = cam_tf.forward();
    spawn_bullet(&mut commands, &mut meshes, &mut materials, tf.translation + Vec3::Y * 0.8, dir, player.team, 12.0);
    timer.0.reset();
}

fn bot_ai(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_targets: Query<(Entity, &Transform, &Player), Without<Bot>>,
    mut q_bots: Query<(&mut Transform, &mut ShootTimer, &Player), With<Bot>>,
) {
    let maybe_human = q_targets.iter().next();
    if maybe_human.is_none() {
        return;
    }

    for (mut tf, mut timer, bot) in &mut q_bots {
        if !bot.alive {
            continue;
        }
        timer.0.tick(time.delta());

        let enemy_tower = tower_target_position(bot.team);
        let toward = (enemy_tower - tf.translation).normalize_or_zero();
        tf.translation += toward * PLAYER_SPEED * 0.55 * time.delta_seconds();
        clamp_to_arena(&mut tf.translation);

        if timer.0.finished() {
            spawn_bullet(&mut commands, &mut meshes, &mut materials, tf.translation + Vec3::Y * 0.8, toward, bot.team, 8.0);
            timer.0.reset();
        }
    }
}

fn tower_ai(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut towers: Query<(&Transform, &Tower, &mut TowerShootTimer)>,
    players: Query<(&Transform, &Player)>,
) {
    for (tower_tf, tower, mut timer) in &mut towers {
        timer.0.tick(time.delta());
        if !timer.0.finished() || tower.hp <= 0.0 {
            continue;
        }

        let target = players
            .iter()
            .filter(|(_, p)| p.alive && p.team != tower.team)
            .min_by(|(a_tf, _), (b_tf, _)| {
                tower_tf
                    .translation
                    .distance_squared(a_tf.translation)
                    .partial_cmp(&tower_tf.translation.distance_squared(b_tf.translation))
                    .unwrap()
            });

        if let Some((enemy_tf, _)) = target {
            let delta = enemy_tf.translation - tower_tf.translation;
            if delta.length() <= TOWER_FIRE_RANGE {
                spawn_bullet(&mut commands, &mut meshes, &mut materials, tower_tf.translation + Vec3::Y, delta.normalize_or_zero(), tower.team, 10.0);
                timer.0.reset();
            }
        }
    }
}

fn bullet_movement(mut commands: Commands, time: Res<Time>, mut bullets: Query<(Entity, &mut Transform, &mut Bullet)>) {
    for (e, mut tf, mut bullet) in &mut bullets {
        tf.translation += bullet.direction * BULLET_SPEED * time.delta_seconds();
        bullet.ttl.tick(time.delta());
        if bullet.ttl.finished() {
            commands.entity(e).despawn_recursive();
        }
    }
}

fn bullet_hits(
    mut commands: Commands,
    mut bullets: Query<(Entity, &Transform, &Bullet)>,
    mut players: Query<(Entity, &Transform, &mut Player)>,
    mut towers: Query<(&Transform, &mut Tower)>,
) {
    for (b_entity, b_tf, bullet) in &mut bullets {
        let mut consumed = false;

        for (_p_entity, p_tf, mut player) in &mut players {
            if !player.alive || player.team == bullet.team {
                continue;
            }
            if b_tf.translation.distance(p_tf.translation) < 0.9 {
                player.hp -= bullet.damage;
                if player.hp <= 0.0 {
                    player.alive = false;
                }
                consumed = true;
                break;
            }
        }

        if consumed {
            commands.entity(b_entity).despawn_recursive();
            continue;
        }

        for (t_tf, mut tower) in &mut towers {
            if tower.team == bullet.team || tower.hp <= 0.0 {
                continue;
            }
            if b_tf.translation.distance(t_tf.translation) < TOWER_RADIUS {
                tower.hp -= bullet.damage;
                consumed = true;
                break;
            }
        }

        if consumed {
            commands.entity(b_entity).despawn_recursive();
        }
    }
}

fn update_timers(time: Res<Time>, mut respawn: ResMut<RespawnTimer>) {
    respawn.0.tick(time.delta());
}

fn respawn_dead_players(mut players: Query<(&mut Transform, &mut Player)>, respawn: Res<RespawnTimer>) {
    if !respawn.0.just_finished() {
        return;
    }

    for (mut tf, mut player) in &mut players {
        if !player.alive {
            player.hp = PLAYER_HP;
            player.alive = true;
            tf.translation = spawn_point_for_team(player.team);
        }
    }
}

fn spawn_point_for_team(team: usize) -> Vec3 {
    match team {
        0 => Vec3::new(-28.0, 0.8, -28.0),
        1 => Vec3::new(28.0, 0.8, -28.0),
        2 => Vec3::new(-28.0, 0.8, 28.0),
        _ => Vec3::new(28.0, 0.8, 28.0),
    }
}

fn tower_target_position(team: usize) -> Vec3 {
    let mut rng = rand::thread_rng();
    let possible: Vec<Vec3> = (0..4)
        .filter(|t| *t != team)
        .map(spawn_point_for_team)
        .collect();
    possible[rng.gen_range(0..possible.len())]
}

fn team_color(team: usize) -> Color {
    match team {
        0 => Color::srgb(0.95, 0.25, 0.25),
        1 => Color::srgb(0.2, 0.45, 0.98),
        2 => Color::srgb(0.2, 0.82, 0.3),
        _ => Color::srgb(0.95, 0.83, 0.2),
    }
}

fn clamp_to_arena(pos: &mut Vec3) {
    pos.x = pos.x.clamp(-ARENA_SIZE, ARENA_SIZE);
    pos.z = pos.z.clamp(-ARENA_SIZE, ARENA_SIZE);
    pos.y = 0.8;
}

fn spawn_bullet(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    origin: Vec3,
    direction: Vec3,
    team: usize,
    damage: f32,
) {
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Sphere::new(0.2).mesh().uv(16, 12)),
            material: materials.add(team_color(team)),
            transform: Transform::from_translation(origin),
            ..default()
        })
        .insert(Bullet {
            direction: direction.normalize_or_zero(),
            team,
            damage,
            ttl: Timer::from_seconds(BULLET_LIFETIME, TimerMode::Once),
        });
}
