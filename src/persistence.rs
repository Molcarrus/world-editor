use std::{
    collections::{ BTreeMap,HashMap },
    fs::File,
    path::PathBuf
};
use anyhow::{ bail,Context,Result };
use bevy::{
    ecs::system::{ Command,EntityCommands },
    prelude::*,
    tasks::{ IoTaskPool,Task }
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
        mut world: &mut World,
        root: Entity 
    ) -> Result<&mut Self> {
        let mut query = world.query::<(&map::Layer, &ChildOf, &Children)>();
        let mut tiles = world.query::<(&map::Location, &tileset::TileRef, &tileset::TileTransform)>();

        for (layer, child_of, children) in query.iter(world) {
            if child_of.parent() != root {
                
            }
        }
    }
}