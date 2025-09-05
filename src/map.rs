use anyhow::{ Context,Result };
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use hexx::*;
use serde::{ Deserialize,Serialize };
use std::collections::HashMap;

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        
    }
}

#[derive(Component, Default, Debug, PartialEq, Reflect, Eq, Hash, Copy, Clone, Serialize, Deserialize)]
#[reflect(Component, Hash, Serialize, Deserialize)]
pub struct Location {
    pub x: i32,
    pub y: i32
}

impl Location {
    pub fn hex(&self) -> Hex {
        Hex::from(*self)
    }
}

impl From<Hex> for Location {
    fn from(value: Hex) -> Location {
        Location { x: value.x, y: value.y }
    }
}

impl From<(i32, i32)> for Location {
    fn from(value: (i32, i32)) -> Location {
        Location { x: value.0, y: value.1 }
    }
}

impl From<Location> for Hex {
    fn from(value: Location) -> Hex {
        Hex { x: value.x, y: value.y }
    }
}

#[derive(Component, Default)]
pub struct Map {
    pub layout: HexLayout
}

pub trait WorldMapExt: Sized {
    fn get_map(&mut self) -> Result<&Map>;
}

impl WorldMapExt for &mut World {
    fn get_map(&mut self) -> Result<&Map> {
        let mut query = self.query::<&Map>();

        query
            .single(self)
            .context("failed to get single Map entity")
    }
}

#[derive(Component)]
pub struct UpdateLocation;

impl Map {
    pub fn new() -> Self {
        Map::default()
    }

    pub fn snap_to_grid(&self, pos: Vec3) -> (Vec3, Location) {
        let hex = self.layout.world_pos_to_hex(pos.xz());
        let snapped = self.layout.hex_to_world_pos(hex);

        (Vec3::new(snapped.x, pos.y, snapped.y), hex.into())
    }

    pub fn translation(&self, location: Location) -> Vec3 {
        let pos = self.layout.hex_to_world_pos(location.into());

        Vec3::new(pos.x, 0.0, pos.y)
    }

    
}