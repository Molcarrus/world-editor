use bevy::prelude::*;
use bevy_egui::egui::{self, Ui};

use world_editor::{ filepicker, prelude::*, ui::widget::* };

use crate::EditorUiEvent;

#[derive(Default, Clone)]
pub struct EditorMenuBar;

impl BasicWidget for EditorMenuBar {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let id = ui.id().with("file");
                basic_widget::<MapNew>(world, ui, id.with("map_new"));
                basic_widget::<MapOpen>(world, ui, id.with("map_open"));
                basic_widget::<MapSave>(world, ui, id.with("map_save"));
                basic_widget::<MapSaveAs>(world, ui, id.with("map_save_as"));
                basic_widget::<MapClose>(world, ui, id.with("map_close"));
                basic_widget::<Quit>(world, ui, id.with("quit"));
            });
            ui.menu_button("Edit", |ui| {
                let id = ui.id().with("edit");
                basic_widget::<Undo>(world, ui, id.with("undo"));
                basic_widget::<Redo>(world, ui, id.with("redo"));
                basic_widget::<Cut>(world, ui, id.with("cut"));
                basic_widget::<MenuCopy>(world, ui, id.with("copy"));
                basic_widget::<Paste>(world, ui, id.with("paste"));
            });
            ui.menu_button("View", |ui| {
                let mut state = world.resource_mut::<crate::EditorState>();

                if ui
                    .checkbox(&mut state.right_panel, "Right Panel")
                    .clicked() 
                {
                    ui.close();
                }
                if ui
                    .checkbox(&mut state.properties_window, "Properties")
                    .clicked()
                {
                    ui.close();
                }
                ui.separator();
                if ui
                    .checkbox(&mut state.inspector, "World Inspector")
                    .clicked() 
                {
                    ui.close();
                }
                if ui
                    .checkbox(&mut state.egui_visuals_window, "egui settings")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .checkbox(&mut state.egui_debug, "egui debug")
                    .clicked()
                {
                    ui.close();
                }
            })
        });
    }
}

#[derive(Default, Clone)]
pub struct MapNew;

impl BasicWidget for MapNew {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        if !ui
            .button("New Map")
            .clicked()
        {
            return;
        }

        let state = world.resource::<crate::EditorState>();
        if state.unsaved_changes {
            let (save_label, save_event) = match &state.map_path {
                Some(path) => ("Save", EditorUiEvent::MapSave(path.clone())),
                None => ("Save As...", EditorUiEvent::MapSaveAs)
            };
        }
    }
}

#[derive(Default, Clone)]
pub struct MapOpen;

#[derive(Default, Clone)]
pub struct MapSave;

#[derive(Default, Clone)]
pub struct MapSaveAs;

#[derive(Default, Clone)]
pub struct MapClose;

#[derive(Default, Clone)]
pub struct Quit;

#[derive(Default, Clone)]
pub struct Undo;

#[derive(Default, Clone)]
pub struct Redo;

#[derive(Default, Clone)]
pub struct Cut;

#[derive(Default, Clone)]
pub struct MenuCopy;

#[derive(Default, Clone)]
pub struct Paste;