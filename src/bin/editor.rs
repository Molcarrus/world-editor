use std::fs::File;
use std::io::Write;

use anyhow::{ bail, Context, Ok, Result };
use bevy::ecs::event::EventReader;
use bevy::input::mouse::MouseWheel;
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    prelude::*
};
use bevy_dolly::dolly::rig;
use bevy_dolly::prelude::*;
use bevy_egui::{ egui, EguiContexts };
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_picking::prelude::*;
use leafwing_input_manager::axislike::DualAxisDirection;
use leafwing_input_manager::prelude::*;
use std::path::PathBuf;

use world_editor::{
    filepicker,
    map,
    persistence,
    prelude::*,
    tileset
};

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

#[derive(Debug, Clone, Copy)]
struct MapCursorMoveEvent(Vec3);

fn handle_editor_ui_events(mut reader: EventReader<EditorUiEvent>) {
    for event in reader.read() {
        
    }
}

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

