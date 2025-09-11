use std::fs::File;
use std::io::Write;

use anyhow::{ bail, Context, Ok, Result };
use bevy::ecs::event::EventReader;
use bevy::input::mouse::MouseWheel;
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    input::mouse::MouseButton
};
use bevy_dolly::dolly::rig;
use bevy_dolly::prelude::*;
use bevy_egui::egui::epaint::tessellator::path;
use bevy_egui::{ egui, EguiContexts };
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_picking::prelude::*;
use leafwing_input_manager::axislike::DualAxisDirection;
use leafwing_input_manager::prelude::*;
use std::path::PathBuf;

use crate::backend::prelude::PickSet;

use world_editor::{
    filepicker,
    map,
    persistence,
    prelude::*,
    tileset
};

mod editor_ui;

fn main() {
    
}

fn dump_main_schedule(app: &mut App) -> Result<()> {
    let dot = bevy_mod_debugdump::schedule_graph_dot(
        app, 
        Main, 
        &bevy_mod_debugdump::schedule_graph::Settings {
        ..default()
        }
        .filter_name(|name| {
            name.contains("egui") || name.contains("leafwing") || name.contains("editor")
        })
    );
    
    let now: chrono::DateTime<chrono::Local> = chrono::Local::now();
    let mut f = File::create(format!(
        "schedule-order_{}.dot",
        now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    ))?;
    f.write_all(&dot.into_bytes())?;

    Ok(())
}

fn setup(
    mut commands: Commands,  
    mut contexts: EguiContexts,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {

}

#[derive(Resource, Debug)]
struct EditorState {
    inspector: bool,
    right_panel: bool,
    egui_visuals_window: bool,
    properties_window: bool,
    egui_debug: bool,
    new_tileset_window: bool,
    map_path: Option<std::path::PathBuf>,
    unsaved_changes: bool,
    active_layer: Option<Entity>,
    active_tileset: Option<Entity>
}

fn inspector_enabled(state: Res<EditorState>) -> bool {
    state.inspector
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            inspector: false,
            right_panel: true,
            egui_visuals_window: false,
            properties_window: true,
            egui_debug: false,
            new_tileset_window: false,
            map_path: None,
            active_tileset: None,
            active_layer: None,
            unsaved_changes: false
        }
    }
}

#[derive(Default, Debug, Reflect, Clone)]
enum EditorSelection {
    #[default]
    None,
    TilesetTile(tileset::TileRef),
    TilesetTiles {
        tileset: Entity,
        tiles: Vec<tileset::TileId>,
    },
    Layer(Entity),
    TileSet(Entity)
}

#[derive(Resource, Default, Debug)]
struct TileSelection {
    tiles: std::collections::HashSet<tileset::TileRef>
}

impl TileSelection {
    pub fn active_tile(&self) -> Option<&tileset::TileRef> {
        self.tiles.iter().next()
    }
}

#[derive(Debug, Clone, Event)]
enum EditorUiEvent {
    MapNew,
    MapClose,
    MapSave(PathBuf),
    MapLoad(PathBuf),
    MapSaveAs,
    DeleteTileset(Entity),
    RedrawMapTiles
}

#[derive(Event, Debug, Clone, Copy)]
struct MapCursorMoveEvent(Vec3);

#[derive(Debug, Event)]
enum PickerEvent {
    AddTiles {
        tileset_id: Entity,
        files: Option<Vec<PathBuf>>
    },
    MapSave(Option<PathBuf>),
    MapLoad(Option<PathBuf>),
    TilesetImport(Option<Vec<PathBuf>>),
    TilesetExport(Entity, Option<PathBuf>)
}

impl filepicker::PickerEvent for PickerEvent {
    fn set_result(&mut self, result: Vec<std::path::PathBuf>) {
        *self = match *self {
            PickerEvent::AddTiles { 
                tileset_id, .. 
            } => PickerEvent::AddTiles { 
                tileset_id, 
                files: Some(result) 
            },
            PickerEvent::MapSave(_) => PickerEvent::MapSave(Some(result[0].clone())),
            PickerEvent::MapLoad(_) => PickerEvent::MapLoad(Some(result[0].clone())),
            PickerEvent::TilesetImport(_) => PickerEvent::TilesetImport(Some(result)),
            PickerEvent::TilesetExport(t, _) => PickerEvent::TilesetExport(t, Some(result[0].clone())) 
        };
    }
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct GridSelectionPlane;

#[derive(Component, Default, Debug, Reflect)]
struct MapCursor {
    position: Vec3,
    grid_location: map::Location,
    tile_transform: tileset::TileTransform
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum InputActions {
    MouseMove,
    MouseScrollY,
    LeftClick,
    CameraPan,
    CameraScale,
    CameraRotateCW,
    CameraRotateCCW,
    ResetCamera,
    ZeroCamera,
    TileRotateCW,
    TileRotateCCW
}

fn input_map() -> InputMap<InputActions> {
    let mouse_move = MouseMove::default();
    let mouse_wheel = MouseScroll::default();
    InputMap::new([
        (InputActions::CameraRotateCW, KeyCode::BracketRight),
        (InputActions::CameraRotateCCW, KeyCode::BracketLeft),
        (InputActions::ResetCamera, KeyCode::KeyZ),
        (InputActions::ZeroCamera, KeyCode::KeyO),
        (InputActions::CameraPan, KeyCode::Space),
        (InputActions::TileRotateCW, KeyCode::KeyQ),
        (InputActions::TileRotateCCW, KeyCode::KeyE)
    ])
    .insert(InputActions::LeftClick, MouseButton::Left)
    .insert_dual_axis(InputActions::CameraScale,mouse_wheel)
    .insert_dual_axis(InputActions::MouseMove, mouse_move)
    .clone()
}

#[derive(Component)]
pub struct RigComponent(Rig);

fn handle_input(
    action_state: Query<&ActionState<InputActions>>,
    mut cursor: Query<&mut tileset::TileTransform, With<MapCursor>>,
    mut camera: Query<(&mut RigComponent, &mut Projection, &Transform), With<MainCamera>>,
    mut egui_contexts: EguiContexts
) {
    let actions = action_state.single().unwrap();
    let (mut rig, mut projection, transform) = camera.single_mut().unwrap();
    let Projection::Orthographic(ref mut projection) = *projection else { panic!("wrong scaling mode") };

    let mouse_input = !egui_contexts.ctx_mut().unwrap().is_pointer_over_area();

    if mouse_input && actions.pressed(&InputActions::CameraPan) {
        let vector = actions.axis_pair(&InputActions::MouseMove).xy() * -0.02 * projection.scale;

        let (mut euler, axis_angle) = transform.rotation.to_axis_angle();

        euler.x = 0.0;
        euler.z = 0.0;
        let rotation = Quat::from_axis_angle(euler, axis_angle);

        if let Some(pos) = rig.0.try_driver_mut::<Position>() {
            pos.translate(rotation * Vec3::new(vector.x, 0.0, vector.y));
        }
    }

    let camera_yp = rig.0.driver_mut::<YawPitch>();
    if actions.just_pressed(&InputActions::CameraRotateCW) {
        let yaw = camera_yp.yaw_degrees + 60.0;
        camera_yp.yaw_degrees = yaw.rem_euclid(360.0);
    } else if actions.just_pressed(&InputActions::CameraRotateCCW) {
        let yaw = camera_yp.yaw_degrees - 60.0;
        camera_yp.yaw_degrees = yaw.rem_euclid(360.0);
    }

    if actions.just_pressed(&InputActions::ResetCamera) {
        camera_yp.yaw_degrees = 45.0;
        camera_yp.pitch_degrees = -30.0;
        projection.scale = 1.0;
    }

    if actions.just_pressed(&InputActions::ZeroCamera) {
        camera_yp.yaw_degrees = 0.0;
        camera_yp.pitch_degrees = -90.0;
        projection.scale = 1.0;
    }

    let scale = actions.value(&InputActions::CameraScale);
    if mouse_input && scale != 0.0 {
        projection.scale = (projection.scale * (1.0 - scale * 0.005)).clamp(0.001, 15.0);
    }

    let mut tile_transform = cursor.single_mut().unwrap();
    if actions.just_pressed(&InputActions::TileRotateCW) {
        tile_transform.rotation = tile_transform.rotation.clockwise();
    }

    if actions.just_pressed(&InputActions::TileRotateCCW) {
        tile_transform.rotation = tile_transform.rotation.counter_clockwise();
    }
}

trait ResultLogger {
    fn log_err(&self);
}

impl<T> ResultLogger for Result<T> {
    fn log_err(&self) {
        if let Err(e) = self {
            error!("{:?}", e);
        }
    }
}

fn handle_ui_events(world: &mut World) {
    use world_editor::helper::run_system;
    use EditorUiEvent::*;

    let mut events = world.remove_resource::<Events<EditorUiEvent>>().unwrap();

    for event in events.drain() {
        match event {
            MapNew => {
                run_system(world, (), close_map);
                run_system(world, (), create_map);
            }
            MapClose => run_system(world, (), close_map),
            MapSaveAs => {
                world.spawn(filepicker::Picker::save_dialog(PickerEvent::MapSave(None)).build());
            }
            MapSave(path) => run_system(world, path.clone(), save_map),
            MapLoad(path) => run_system(world, path.clone(), load_map),
            RedrawMapTiles => run_system(world, (), redraw_map_tiles),
            DeleteTileset(entity) => run_system(world, entity, remove_tileset)
        }
    }

    world.insert_resource(events);
}

fn save_map(
    In(path): In<std::path::PathBuf>,
    mut commands: Commands,  
    mut state: ResMut<EditorState>,
    map: Query<Entity, With<map::Map>>
) {
    let Result::Ok(entity) = map.single() else {
        warn!("no map loaded");
        return;
    };
    info!("save map to {}", path.to_string_lossy());
    commands.queue(persistence::SaveMapCommand::new(path, entity));
    state.unsaved_changes = false;
}

fn load_map(
    In(path): In<std::path::PathBuf>,
    mut commands: Commands
) {
    info!("load map {}", path.to_string_lossy());
    commands.spawn(persistence::MapImporter::new(path));
}

fn close_map(
    mut commands: Commands,
    mut state: ResMut<EditorState>,
    mut tile_selection: ResMut<TileSelection>,
    map: Query<Entity, With<map::Map>>,
    cursor: Query<Entity, With<MapCursor>>
) {
    if state.unsaved_changes {
        info!("closing map {:?}; discarding changes", state.map_path);
    } else {
        info!("closing map {:?}", state.map_path);
    }

    let cursor = cursor.single().unwrap();

    commands
        .entity(cursor)
        .remove::<(tileset::TileRef, SceneRoot)>()
        .despawn();

    tile_selection.tiles.clear();

    if let Result::Ok(entity) = map.single() {
        commands.entity(entity).despawn();
    }

    state.map_path = None;
    state.unsaved_changes = false;
    state.active_tileset = None;
    state.active_layer = None;
}

fn create_map(
    mut commands: Commands,  
    mut state: ResMut<EditorState>
) {
    info!("create new map");

    commands
        .spawn((
            Name::new("map"),
            map::Map::default(),
            Transform::default(),
            Visibility::default()
        ))
        .with_children(|map| {
            let layer = map
                .spawn((
                    Name::new("layer"),
                    map::Layer::new("Background".into()),
                    Transform::default(),
                    Visibility::default()
                ))  
                .id();

            state.active_layer = Some(layer);
            let tileset = map
                .spawn((
                    Name::new("tileset"),
                    tileset::TileSet::new("Default Tileset")
                ))
                .id();

            state.active_tileset = Some(tileset);
        });

    state.map_path = None;
    state.unsaved_changes = false;
}

fn map_loaded(
    mut state: ResMut<EditorState>,
    map: Query<&Children, Added<map::Map>>,
    tilesets: Query<&mut tileset::TileSet>,
    layers: Query<&mut map::Layer>
) {
    let Result::Ok(map_children) = map.single() else { return; };

    for child in map_children {
        if state.active_tileset.is_none() && tilesets.get(*child).is_ok() {
            state.active_tileset = Some(*child);
        }

        if state.active_layer.is_none() && layers.get(*child).is_ok() {
            state.active_layer = Some(*child);
        }
    }
}

fn remove_tileset(
    In(tileset_id): In<Entity>,
    mut state: ResMut<EditorState>,
    mut commands: Commands,  
    tilesets: Query<Entity, With<tileset::TileSet>>
) {
    commands.entity(tileset_id).despawn();
    state.active_tileset = tilesets.iter().find(|entity| *entity != tileset_id);
}

fn redraw_map_tiles(
    mut commands: Commands,  
    tile_selection: Res<TileSelection>,
    tiles: Query<(
        Entity,
        &tileset::TileRef,
        &tileset::TileTransform,
        &map::Location
    )>,
    tilesets: Query<&tileset::TileSet>,
    map: Query<&map::Map>
) {
    let Result::Ok(map) = map.single() else { return; };

    for (entity, tile_ref, tile_transform, location) in &tiles {
        if !tile_selection.tiles.contains(tile_ref) {
            continue;
        }
        let Result::Ok(tileset) = tilesets.get(tile_ref.tileset) else {
            warn!("unknown tileset {:?} in entity {:?}", tile_ref.tileset, entity);
            continue;
        };

        let bundle = tileset::TileBundle::new(map, *location, tile_transform.clone(), tileset, tile_ref.tileset, tile_ref.tile);

        commands.entity(entity).insert(bundle);
    }
}

fn handle_picker_events(
    mut commands: Commands,  
    mut picker_events: EventReader<PickerEvent>,
    mut state: ResMut<EditorState>,
    mut tilesets: Query<&mut tileset::TileSet>,
    mut editor_events: EventWriter<EditorUiEvent>,
    map: Query<Entity, With<map::Map>>
) {
    for event in picker_events.read() {
        match event {
            PickerEvent::AddTiles { tileset_id,files } => {
                let Result::Ok(mut tileset) = tilesets.get_mut(*tileset_id) else { continue; };
                let Some(paths) = files else { continue; };
                for path in paths {
                    tileset.add_title(path.clone());
                } 
                state.unsaved_changes = true;
            }
            PickerEvent::MapSave(path) => {
                let Some(path) = path else { continue; };
                if state.map_path.is_none() {
                    state.map_path = Some(path.clone());
                }

                editor_events.write(EditorUiEvent::MapSave(path.clone()));
            }
            PickerEvent::MapLoad(path) => {
                let Some(path) = path else { continue; };
                if state.map_path.is_none() {
                    state.map_path = Some(path.clone());
                }

                editor_events.write(EditorUiEvent::MapLoad(path.clone()));
            }
            PickerEvent::TilesetImport(paths) => {
                let Some(paths) = paths else { continue; };
                let Result::Ok(map) = map.single() else {
                    error!("no map foundl not loading tileset");
                    continue;
                };
                commands.entity(map).with_children(|map| {
                    for path in paths {
                        let id = map.spawn(tileset::TilesetImporter::new(path.clone())).id();
                        state.active_tileset = Some(id);
                    }
                });
            }
            PickerEvent::TilesetExport(tileset_id, path) => {
                let Some(path) = path else { continue; };
                let Result::Ok(tileset) = tilesets.get(*tileset_id) else {
                    warn!("tileset not found: {:?}", event);
                    continue;
                };
                commands.spawn(tileset::TilesetExporter::new(path.clone(), tileset.clone()));
            }
        }
    }

    picker_events.clear();
}

fn handle_map_cursor_events(
    mut commands: Commands,
    mut events: EventReader<MapCursorMoveEvent>,
    state: Res<EditorState>,
    map: Query<&map::Map>,
    buttons: Res<PickSet::Input<MouseButton>>,
    cursor: Query<(Entity, &tileset::TileRef, &tileset::TileTransform), With<MapCursor>>,
    tiles: Query<
        (
            Entity,
            &map::Location,
            &tileset::TileRef,
            &tileset::TileTransform,
            &ChildOf
        ),
        Without<MapCursor>
    >
) -> Result<()> {

}