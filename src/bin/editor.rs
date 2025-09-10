use std::fs::File;
use std::io::Write;

use anyhow::{ bail, Context, Ok, Result };
use bevy::ecs::event::EventReader;
use bevy::input::mouse::MouseWheel;
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    prelude::*
};
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
    
}