use anyhow::{ Context,Result };
use bevy::{
    prelude::*,
    tasks::{ IoTaskPool,Task }
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
        
    }
}

pub type TileId = usize;

#[derive(Debug, Default, Clone, Reflect, FromReflect, Component, Serialize, Deserialize)]
#[reflect(Component)]
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

