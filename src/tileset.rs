use anyhow::{ Context,Result };
use bevy::{
    prelude::*, render::render_resource::{
        Extent3d,
        TextureDescriptor,
        TextureDimension,
        TextureFormat,
        TextureUsages
    }, scene::ron, tasks::{ IoTaskPool,Task }
};
use bevy_egui::{ egui,EguiUserTextures };
use serde::{
    de::{
        self,
        MapAccess,
        Visitor
    },
    ser::SerializeMap,
    Deserialize,
    Serialize
};
use std::{
    collections::HashMap,
    path::PathBuf
};

use crate::map;

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TileSet>()
            .register_type::<TileRef>()
            .register_type::<TileRotation>()
            .register_type::<Tile>()
            .register_type::<TileId>()
            .register_type::<Vec<TileId>>()
            .add_systems(Update, (update_tile_scene, update_tile_transform))
            .add_systems(Startup, (load_tiles, tileset_exporter, tileset_importer));
    }
}

pub type TileId = usize;

#[derive(Debug, Default, Clone, Component, Serialize, Deserialize)]
#[derive(Reflect)]
#[reflect(Component, FromReflect)]
pub struct Tile {
    pub id: TileId,
    pub name: String,
    pub path: PathBuf,
    pub transform: Transform,
    #[reflect(ignore)]
    #[serde(skip)]
    pub scene: Option<Handle<Scene>>,
    #[reflect(ignore)]
    #[serde(skip)]
    pub egui_texture_id: Option<egui::TextureId>
}

pub type TileSetId = usize;

#[derive(Component, Default, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct TileSet {
    pub name: String,
    pub tiles: HashMap<TileId, Tile>,
    pub tile_order: Vec<TileId>,
    tile_id_max: TileId
}

impl TileSet {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            tiles: HashMap::new(),
            tile_order: Vec::new(),
            tile_id_max: 0
        }
    }

    pub fn add_title(&mut self, path: PathBuf) {
        let tile = Tile {
            id: self.tile_id_max,
            name: path.file_stem().unwrap().to_string_lossy().into(),
            path,
            transform: Transform::IDENTITY,
            scene: None,
            egui_texture_id: None
        };
        self.tile_order.push(tile.id);
        self.tiles.insert(tile.id, tile);
        self.tile_id_max += 1;
    }
}

pub const TILESET_VERSION: usize = 1;

impl Serialize for TileSet {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("version", &TILESET_VERSION)?;
        map.serialize_entry("name", &self.name)?;

        let tiles: Vec<Tile> = self
                .tile_order
                .iter()
                .map(|i| self.tiles[i].clone())
                .collect();
        map.serialize_entry("tiles", &tiles)?;
        
        map.end()
    }
}

struct TileSetVisitor;

impl <'de> Visitor<'de> for TileSetVisitor {
    type Value = TileSet;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("{ \"version\": usize, \"name\": &str, \"tiles\": Vec<Tile> }")
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>, {
        let mut tileset = TileSet::default();

        if map.next_key::<&str>()? != Some("version") {
            return Err(de::Error::custom("expected \"version\" key"));
        };

        match map.next_value::<usize>()? {
            TILESET_VERSION => (),
            v => {
                return Err(de::Error::custom(format!("unsupported tileset version: {}", v)));
            }
        }

        if map.next_key::<&str>()? != Some("name") {
            return Err(de::Error::custom("expected \"name\" key"));
        };
        tileset.name = map.next_value::<String>()?;

        if map.next_key::<&str>()? != Some("tiles") {
            return Err(de::Error::custom("expected \"tiles\" key"));
        };
        let tiles = map.next_value::<Vec<Tile>>()?;

        for tile in tiles {
            tileset.tile_order.push(tile.id);
            tileset.tiles.insert(tile.id, tile);
        }

        Ok(tileset)
    }
}

impl <'de> Deserialize<'de> for TileSet {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: de::Deserializer<'de> {
        deserializer.deserialize_map(TileSetVisitor)
    }
}

#[derive(Component, Debug, Reflect, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileRef {
    pub tileset: Entity,
    pub tile: TileId
}

#[derive(Component, Default, Debug, Reflect, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TileRotation {
    #[default]
    None,
    Clockwise60,
    Clockwise120,
    Clockwise180,
    CounterClockwise120,
    CounterClockwise60
}

impl TileRotation {
    pub fn clockwise(self) -> Self {
        match self {
            TileRotation::None => TileRotation::Clockwise60,
            TileRotation::Clockwise60 => TileRotation::Clockwise120,
            TileRotation::Clockwise120 => TileRotation::Clockwise180,
            TileRotation::Clockwise180 => TileRotation::CounterClockwise120,
            TileRotation::CounterClockwise120 => TileRotation::Clockwise60,
            TileRotation::CounterClockwise60 => TileRotation::None
        }
    }

    pub fn counter_clockwise(self) -> Self {
        match self {
            TileRotation::None => TileRotation::CounterClockwise60,
            TileRotation::CounterClockwise60 => TileRotation::CounterClockwise120,
            TileRotation::CounterClockwise120 => TileRotation::Clockwise180,
            TileRotation::Clockwise180 => TileRotation::Clockwise120,
            TileRotation::Clockwise120 => TileRotation::Clockwise60,
            TileRotation::Clockwise60 => TileRotation::None
        }
    }
}

impl From<TileRotation> for f32 {
    fn from(value: TileRotation) -> Self {
        use std::f32::consts::TAU;

        match value {
            TileRotation::None => 0.0,
            TileRotation::Clockwise60 => TAU / 6.0,
            TileRotation::Clockwise120 => TAU / 3.0,
            TileRotation::Clockwise180 => TAU / 2.0,
            TileRotation::CounterClockwise120 => -TAU / 3.0,
            TileRotation::CounterClockwise60 => -TAU / 6.0,
        }
    }
}

#[derive(Component, Default, Debug, Reflect, Clone, PartialEq, Eq)]
pub struct TileTransform {
    pub rotation: TileRotation
}

#[derive(Bundle)]
pub struct TileBundle {
    tile_ref: TileRef,
    location: map::Location,
    tile_transform: TileTransform,
    scene: SceneRoot
}

impl TileBundle {
    pub fn new(
        map: &map::Map,
        location: map::Location,
        tile_transform: TileTransform,
        tileset: &TileSet,
        tileset_entity: Entity,
        tile_id: TileId
    ) -> Self {
        let tile = tileset
            .tiles
            .get(&tile_id)
            .unwrap_or_else(|| panic!("TileId {} in TileSet {}", tile_id, tileset.name));

        let _transform = map.tile_transform(tile, location, &tile_transform);
        let scene = tile.scene.as_ref().unwrap().clone();

        TileBundle {
            location,
            tile_ref: TileRef { tileset: tileset_entity, tile: tile_id },
            tile_transform,
            scene: SceneRoot(scene)
        }
    }
}

#[derive(Component)]
pub struct TileScene(Handle<Scene>);

fn tile_ref_changed(
    mut commands: Commands,  
    tiles: Query<Entity, (With<TileScene>, Changed<TileRef>)>
) {
    for entity in &tiles {
        commands.entity(entity).remove::<TileScene>();
    }
}

fn update_tile_scene(
    mut commands: Commands,
    tiles: Query<(Entity, &TileRef), Without<TileScene>>,
    tilesets: Query<&mut TileSet>
) {
    for (entity, tile_ref) in &tiles {
        let Ok(tileset) = tilesets.get(tile_ref.tileset) else {
            warn!("unknown tileset for tile {:?}: {:?}; removing entity", entity, tile_ref);
            commands.entity(entity).despawn();
            continue;
        };
        let Some(tile) = tileset.tiles.get(&tile_ref.tile) else {
            warn!("unknown tile for tile {:?}: {:?}; removing entity", entity, tile_ref);
            commands.entity(entity).despawn();
            continue;
        };
        let Some(scene) = tile.scene.as_ref() else {
            debug!("scene not present for {:?}: {:?}", entity, tile_ref);
            continue;
        };
        commands.entity(entity).insert(SceneRoot(scene.clone()));
    }
}

fn update_tile_transform(
    mut commands: Commands,
    map: Query<&map::Map>,
    tile_transforms: Query<
        (Entity, &TileRef, &TileTransform, &map::Location),
        Or<(Changed<TileTransform>, Changed<map::Location>)>
    >,
    tilesets: Query<&mut TileSet>
) {
    let Ok(map) = map.single() else { return; };
    for (entity, tile_ref, tile_transform, location) in &tile_transforms {
        let Ok(tileset) = tilesets.get(tile_ref.tileset) else {
            warn!("unknow tileset for tile: {:?}: {:?}; removing entity", entity, tile_ref);
            commands.entity(entity).despawn();
            continue;
        };
        let Some(tile) = tileset.tiles.get(&tile_ref.tile) else {
            warn!("unknown tile for tile {:?}: {:?}; removing entity", entity, tile_ref);
            commands.entity(entity).despawn();
            continue;
        };
        let transform = map.tile_transform(tile, *location, tile_transform);
        commands.entity(entity).insert(transform);
    }
}

fn load_tiles(
    asset_server: Res<AssetServer>,
    mut tilesets: Query<&mut TileSet, Changed<TileSet>>,
    mut images: ResMut<Assets<Image>>,
    mut render_queue: ResMut<crate::render::RenderQueue>,
    mut egui_user_textures: ResMut<EguiUserTextures>
) {
    for mut tileset in &mut tilesets {
        for tile in tileset.tiles.values_mut() {
            let scene = match tile.scene {
                Some(_) => continue,
                None => {
                    let scene = asset_server.load(format!("{}#Scene0", tile.path.to_string_lossy()));
                    tile.scene = Some(scene.clone());
                    scene
                }
            };

            match tile.egui_texture_id {
                Some(_) => continue,
                None => {
                    let image = alloc_render_image(48 * 2, 48 * 2);
                    let handle = images.add(image);
                    tile.egui_texture_id = Some(egui_user_textures.add_image(handle.clone()));
                    render_queue.push(handle, scene);
                }
            }
        }
    }
}

fn alloc_render_image(width: u32, height: u32) -> Image {
    let size = Extent3d {
        width,
        height,
        ..default()
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor { 
            label: None, 
            size, 
            mip_level_count: 1, 
            sample_count: 1, 
            dimension: TextureDimension::D2, 
            format: TextureFormat::Bgra8UnormSrgb, 
            usage: TextureUsages::TEXTURE_BINDING 
                | TextureUsages::COPY_DST 
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[] 
        },
        ..default()
    };

    image.resize(size);
    
    image
}

#[derive(Component, Debug)]
pub struct TilesetImporter {
    path: PathBuf,
    task: Task<Result<TileSet>>
}

impl TilesetImporter {
    pub fn new(path: PathBuf) -> Self {
        let task_pool = IoTaskPool::get();
        let path_copy = path.clone();
        let task = task_pool.spawn(async move {
            let f = std::fs::File::open(path).context("failed to open file")?;
            let tileset: TileSet = ron::de::from_reader(f).context("failed to parse tileset")?;

            Ok::<TileSet, anyhow::Error>(tileset)
        });

        Self {
            path: path_copy,
            task
        }
    }
}

fn tileset_importer(
    mut commands: Commands,
    mut tileset_importers: Query<(Entity, &mut TilesetImporter)>
) {
    for (entity, mut importer) in &mut tileset_importers {
        let Some(result) = futures_lite::future::block_on(futures_lite::future::poll_once(&mut importer.task)) else { continue };
        match result {
            Err(e) => {
                warn!("failed to load tileset: {}: {:?}", importer.path.to_string_lossy(), e);
                commands.entity(entity).despawn();
            }
            Ok(tileset) => {
                let name = importer.path.file_stem().unwrap().to_string_lossy();
                commands
                    .entity(entity)
                    .remove::<TilesetImporter>()
                    .insert((Name::new(format!("tileset: {}", name)), tileset));
            }
        }
    }
}

#[derive(Component, Debug)]
pub struct TilesetExporter {
    task: Task<Result<()>>
}

impl TilesetExporter {
    pub fn new(
        path: PathBuf,
        tileset: TileSet 
    ) -> Self {
        let task_pool = IoTaskPool::get();
        let task = task_pool.spawn(async move {
            let f = std::fs::File::create(path.clone()).context(format!("open tileset {:?}", path))?;
            ron::ser::to_writer_pretty(f, &tileset, ron::ser::PrettyConfig::default())
                .context(format!("writing tileset to {:?}", path))?;

            Ok::<(), anyhow::Error>(())
        });

        Self { task }
    }
}

fn tileset_exporter(
    mut commands: Commands,
    mut tileset_exporters: Query<(Entity, &mut TilesetExporter)>
) {
    for (entity, mut exporter) in &mut tileset_exporters {
        let Some(result) = futures_lite::future::block_on(futures_lite::future::poll_once(&mut exporter.task)) else { continue };
        if let Err(e) = result {
            warn!("failed to export tileset: {:#?}", e);
        }
        commands.entity(entity).despawn();
    }
}