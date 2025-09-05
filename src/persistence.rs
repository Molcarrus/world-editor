use std::{
    collections::{ BTreeMap,HashMap },
    fs::File,
    path::PathBuf
};
use anyhow::{ bail,Context,Result };
use bevy::{
    ecs::system::{ Command,EntityCommands }, prelude::*, scene::ron::{self, ser::{to_writer_pretty, PrettyConfig}}, tasks::{ IoTaskPool,Task }
};
use futures_lite::future;
use hexx::HexLayout;
use serde:: {
    de::Visitor,
    Deserialize,
    Serialize
};

use crate::{ map,tileset };

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        
    }
}

#[derive(Clone, Component, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Reflect)]
pub struct SaveId(usize);

impl Serialize for SaveId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        serializer.serialize_u64(self.0 as u64)
    }
}

struct SaveIdVisitor;

impl <'de> Visitor<'de> for SaveIdVisitor {
    type Value = SaveId;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expecting unsigned integer")
    }

    fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error, {
        Ok(SaveId(v as usize))
    }
}

impl <'de> Deserialize<'de> for SaveId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        deserializer.deserialize_u64(SaveIdVisitor)
    }
}

impl std::ops::Add<usize> for SaveId {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::ops::AddAssign<usize> for SaveId {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

pub trait WorldSaveIdExt {
    fn save_id_next(&mut self) -> SaveId;

    fn assign_save_ids(
        &mut self,
        entities: impl Iterator<Item = Entity>
    ) -> Result<HashMap<Entity, SaveId>>;
}

impl WorldSaveIdExt for &mut World {
    fn save_id_next(&mut self) -> SaveId {
        let mut query = self.query::<&SaveId>();
        query
            .iter(self)
            .max()
            .map(|id| *id + 1)
            .unwrap_or(SaveId(0))
    }

    fn assign_save_ids(
        &mut self,
        entities: impl Iterator<Item = Entity>
    ) -> Result<HashMap<Entity, SaveId>> {
        let mut next_id = self.save_id_next();
        let mut entity_map = HashMap::new();

        for entity in entities {
            let mut entity_ref = self
                .get_entity_mut(entity)
                .context(format!("unknown entity: {:?}", entity))?;

            let id = match entity_ref.get::<SaveId>() {
                Some(id) => *id,
                None => {
                    let id = next_id;
                    entity_ref.insert(id);
                    next_id += 1;
                    id
                }
            };

            entity_map.insert(entity, id);
        }

        Ok(entity_map)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Tile {
    location: map::Location,
    tileset: SaveId,
    tile_id: tileset::TileId,
    rotation: tileset::TileRotation
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Layer {
    name: String,
    tiles: Vec<Tile>
}

impl From<&map::Layer> for Layer {
    fn from(value: &map::Layer) -> Self {
        Self {
            name: value.name.clone(),
            tiles: Vec::new()
        }
    }
}

impl From<&Layer> for map::Layer {
    fn from(value: &Layer) -> Self {
        Self {
            name: value.name.clone(),
            tiles: HashMap::new()
        }
    }
}

const MAP_FORMAT_VERSION: usize = 1;

#[derive(Default, Debug, Serialize, Deserialize)]
struct MapFormat {
    version: usize,
    layout: HexLayout,
    tilesets: BTreeMap<SaveId, tileset::TileSet>,
    layers: Vec<Layer>,
    #[serde(skip)]
    entity_map: HashMap<Entity, SaveId>
}

impl MapFormat {
    fn try_new(world: &mut World, root: Entity) -> Result<Self> {
        let mut map = Self {
            version: MAP_FORMAT_VERSION,
            ..default()
        };

        let root_entity = world.entity(root);
        map.layout = root_entity
            .get::<map::Map>()
            .context(format!(
                "failed to get Map component for map root {:?}", root 
            ))?
            .layout
            .clone();

        map.add_tilesets(world, root)?
            .add_layers(world, root)?;

        Ok(map)
    }

    fn add_tilesets(
        &mut self,
        mut world: &mut World,
        root: Entity 
    ) -> Result<&mut Self> {
        let mut query = world.query_filtered::<(Entity, &ChildOf), With<tileset::TileSet>>();
        let tilesets: Vec<Entity> = query
            .iter(world)
            .filter_map(|(entity, child_of)| {
                if child_of.parent() == root {
                    Some(entity)
                } else {
                    None 
                }
            })
            .collect();
        self.entity_map = world.assign_save_ids(tilesets.iter().cloned())?;

        let mut query = world.query::<&tileset::TileSet>();
        for entity in tilesets {
            let id = self
                .entity_map
                .get(&entity)
                .context(format!("failed to get SaveId for TileSet {:?}", entity))?;
            let tileset = query.get(world, entity)?;
            self.tilesets.insert(*id, tileset.clone());
        }

        Ok(self)
    }

    fn add_layers(
        &mut self,
        world: &mut World,
        root: Entity 
    ) -> Result<&mut Self> {
        let mut query = world.query::<(&map::Layer, &ChildOf, &Children)>();
        let mut tiles = world.query::<(&map::Location, &tileset::TileRef, &tileset::TileTransform)>();

        for (layer, child_of, children) in query.iter(world) {
            if child_of.parent() != root {
                continue;
            }
            let mut layer: Layer = layer.into();

            for child in children {
                let Ok((location, tile_ref, tile_transform)) = tiles.get(world, *child) else { continue };
                let tileset = self
                    .entity_map
                    .get(&tile_ref.tileset)
                    .context(format!("tileset SaveId not found: {:?}", tile_ref))?;

                let tile = Tile {
                    location: *location,
                    tileset: *tileset,
                    tile_id: tile_ref.tile,
                    rotation: tile_transform.rotation
                };
                layer.tiles.push(tile);
            }
            self.layers.push(layer);
        }

        Ok(self)
    }

    pub fn try_spawn(
        &self,
        root: &mut EntityCommands
    ) -> Result<()> {
        if self.version != MAP_FORMAT_VERSION {
            bail!(
                "unsupported map version: {} != {}",
                self.version,
                MAP_FORMAT_VERSION
            );
        }
        debug!("loading map into {:?}", root.id());

        let map = map::Map {
            layout: self.layout.clone()
        };

        let mut entity_map = HashMap::new();
        for (id, tileset) in &self.tilesets {
            let entity = root
                .commands()
                .spawn((Name::new("tileset"), tileset.clone()))
                .id();
            root.add_child(entity);
            entity_map.insert(id, entity);
        }

        for layer in &self.layers {
            let layer_component: map::Layer = layer.into();
            let layer_entity = root
                .commands()
                .spawn((
                    Name::new("layer"),
                    layer_component,
                    Transform::default(),
                    Visibility::default()
                ))
                .id();
            root.add_child(layer_entity);

            let mut tiles = Vec::new();

            for tile in &layer.tiles {
                let tile_ref = tileset::TileRef {
                    tileset: *entity_map.get(&tile.tileset).unwrap(),
                    tile: tile.tile_id,
                };

                let tile_entity = root
                    .commands()
                    .spawn((
                        tile.location,
                        tile_ref,
                        tileset::TileTransform {
                            rotation: tile.rotation
                        },
                        Transform::default(),
                        Visibility::default()
                    ))
                    .id();
                tiles.push(tile_entity);
            }

            root
                .commands()
                .entity(layer_entity)
                .add_children(&tiles);
        }

        root.insert((Transform::default(), Visibility::default(), map));

        Ok(())
    }
}

pub struct SaveMapCommand {
    path: PathBuf,
    map: Entity,
}

impl SaveMapCommand {
    pub fn new(path: PathBuf, map: Entity) -> Self {
        Self { path,map }
    }
}

impl Command for SaveMapCommand {
    fn apply(self, world: &mut World) {
        let map = match MapFormat::try_new(world, self.map) {
            Ok(map) => map,
            Err(err) => {
                warn!("failed to save map: {:#?}", err);
                return;
            }
        };

        let task_pool = IoTaskPool::get();
        let task = task_pool.spawn(async move {
            let f = File::create(self.path.clone()).context(format!("open map: {:?}", self.path))?;
            to_writer_pretty(f, &map, PrettyConfig::default()).context(format!("writing map to {:?}", self.path))?;

            Ok::<(), anyhow::Error>(())
        });

        world.spawn(MapWriterTask(task));
    }
}

#[derive(Component)]
struct MapWriterTask(Task<Result<()>>);

fn map_writers(
    mut commands: Commands,  
    mut map_writers: Query<(Entity, &mut MapWriterTask)>
) {
    for (entity, mut writer) in &mut map_writers {
        let Some(result) = future::block_on(future::poll_once(&mut writer.0)) else { continue };
        if let Err(e) = result {
            warn!("{:#?}", e);
        }
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
pub struct MapImporter {
    path: PathBuf,
    task: Task<Result<MapFormat>>
}

impl MapImporter {
    pub fn new(path: PathBuf) -> Self {
        let path_copy = path.clone();
        let task_pool = IoTaskPool::get();
        let task = task_pool.spawn(async move {
            let buf = std::fs::read_to_string(path).context("failed to read file")?;
            let map = ron::from_str(&buf).context("failed to parse map")?;

            Ok(map)
        });

        Self {
            path: path_copy,
            task
        }
    }
}

fn map_importer(
    mut commands: Commands,
    mut map_importers: Query<(Entity, &mut MapImporter)>
) {
    for (entity, mut importer) in &mut map_importers {
        let Some(result) = future::block_on(future::poll_once(&mut importer.task)) else { continue };
        match result {
            Err(e) => {
                warn!(
                    "failed to load map{}: {:?}",
                    importer.path.to_string_lossy(),
                    e 
                );
                commands.entity(entity).despawn();
            }
            Ok(map) => {
                let name = importer.path.file_stem().unwrap().to_string_lossy();
                let mut entity_ref = commands.entity(entity);

                if let Err(e) = map.try_spawn(&mut entity_ref) {
                    error!(
                        "failed to spawn map {}: {:?}",
                        importer.path.to_string_lossy(),
                        e 
                    );
                    entity_ref.despawn();
                    continue;
                }

                entity_ref
                    .remove::<MapImporter>()
                    .insert(Name::new(format!("map: {}", name)));
            }
        };
    }
}