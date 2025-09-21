use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy_egui::egui::{self, emath::TSTransform, UiBuilder};
use leafwing_input_manager::clashing_inputs::BasicInputs;
use world_editor::{filepicker,map,tileset,ui,ui::widget::*};

use crate::{EditorState,EditorUiEvent};

#[derive(Default)]
pub struct EditorPanel;

#[derive(Default, Clone)]
pub struct TilesetPanel;

#[derive(Default, Clone)]
pub struct TilesetPanelHeader;

#[derive(Default, Clone)]
pub struct TilesetDropdown;

pub struct TilesetMenu;

#[derive(Default, Clone)]
pub struct RemoveTilesetButton;

impl BasicWidget for RemoveTilesetButton {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    } 

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let state = world.resource::<EditorState>();

        let Some(tileset_id) = state.active_tileset else {
            if ui
                .add_enabled(false, egui::Button::new("➖"))
                .clicked()
            {
                unreachable!();
            }
            return;
        };

        if ui
            .button("➖")
            .clicked() 
        {
            world.spawn()
        }
    }
}

#[derive(Default, Clone)]
pub struct TilesetViewer {
    height: f32 
}

impl BasicWidget for TilesetViewer {
    fn new(_world: &mut World, ui: &egui::Ui) -> Self {
        Self {
            height: ui.available_height() * 0.60
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        egui::ScrollArea::vertical()
            .max_height(self.height)
            .auto_shrink([false, false])
            .id_salt(id.with("vscroll"))
            .show(ui, |ui| {
                basic_widget::<TilePicker>(world, ui, id.with("tile_pricker"));
                ui.allocate_space(ui.available_size());
            });
        ui.separator();
        basic_widget::<TilesetPanelFooter>(world, ui, id.with("tileset_footer"));
        self.height = fn_widget::<ui::widgets::VDragHandle>(world, ui, id.with("drag_handle"), self.height);
    }
}

#[derive(Default, Clone)]
pub struct TilesetPanelFooter;

impl BasicWidget for TilesetPanelFooter {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        ui.horizontal(|ui| {
            basic_widget::<TilesetAddTiles>(world, ui, id.with("add_tiles"));
        });
    }
}

#[derive(Default, Clone)]
pub struct TilesetAddTiles;

impl BasicWidget for TilesetAddTiles {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let state = world.resource::<EditorState>();

        let Some(tileset_id) = state.active_tileset else {
            if ui
                .add_enabled(false, egui::Button::new("➖"))
                .clicked() 
            {
                unreachable!();
            }
            return;
        };

        if ui
            .button("➕")
            .clicked()
        {
            world.spawn(
                filepicker::Picker::for_many(crate::PickerEvent::AddTiles { tileset_id, files: None })
                .add_filter("GLTF", &["glb"])
                .build()
            );
        }
    }
}

pub struct TilePicker<'w: 'static, 's: 'static> {
    system_state: SystemState<(
        Res<'w, EditorState>,
        ResMut<'w, crate::TileSelection>,
        Query<'w, 's, &'static mut tileset::TileSet> 
    )>,
    tileset: Option<Entity>,
    start_range: Option<usize>,
    last_range: Option<Vec<tileset::TileRef>>,
    drag_start: Option<egui::Pos2>
}

impl<'w, 's> BasicWidget for TilePicker<'w, 's> {
    fn new(world: &mut World, _ui: &egui::Ui) -> Self {
        Self {
            system_state: SystemState::new(world),
            tileset: None,
            start_range: None,
            last_range: None,
            drag_start: None 
        }
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        use tileset::TileRef;

        let (state, mut selection, mut tilesets) = self.system_state.get_mut(world);

        if self.tileset != state.active_tileset {
            self.tileset = state.active_tileset;
            self.start_range = None;
            self.last_range = None;
            self.drag_start = None;
        }
        let Some(tileset_id) = state.active_tileset else {
            ui.label("no active tileset");
            return;
        };
        
        let modifiers = ui.input(|i| i.modifiers);
        let mut deselect_range = None;
        let mut select_range = None;
        let mut drop_index = None;

        let Result::Ok(tileset) = tilesets.get(tileset_id) else {
            ui.label(format!("invalid tileset {:?}", tileset_id));
            return;
        };

        let tile_size = egui::Vec2::splat(48.0);
        let layout = egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true);
        let drag_layer = egui::LayerId::new(egui::Order::Tooltip, id.with("dragging"));

        ui.with_layout(layout, |ui| {
            let mut spacing = ui.spacing_mut();
            spacing.item_spacing = egui::vec2(0.0, 0.0);
            spacing.button_padding = egui::vec2(0.0, 0.0);
            
            let mut visuals = ui.visuals_mut();
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;

            for (index, tile_id) in tileset.tile_order.iter().enumerate() {
                let Some(tile) = tileset.tiles.get(tile_id) else {
                    warn!("unknown tile if in tileset order; \
                    tileset \"{}\" ({:?}), tile id {}", tileset.name, tileset_id, tile_id);
                    continue;
                };
                let Some(texture) = tile.egui_texture_id else { continue };
                let tile_ref = TileRef {
                    tileset: tileset_id,
                    tile: *tile_id
                };
                let selected = selection.tiles.contains(&tile_ref);

                let button = egui::ImageButton::new((texture, tile_size))
                    .selected(selected)
                    .sense(egui::Sense::click_and_drag());

                if self.drag_start.is_some() {
                    let res = if selected {
                        ui.scope_builder(UiBuilder::new().layer_id(drag_layer), |ui| ui.add(button)).response
                    } else {
                        ui.add(button)
                    };

                    if res.hovered() && ui.input(|i| i.pointer.any_released()) {
                        drop_index = Some(index);
                        self.drag_start = None;
                    }
                    continue;
                }

                let res = ui.add(button);
                if res.clicked() {
                    if modifiers.shift_only() {
                        deselect_range = self.last_range.take();
                        if let Some(start) = &self.start_range {
                            let range = if *start < index {
                                *start..=index
                            } else {
                                index..=*start
                            };
                            select_range = Some(range);
                        } else {
                            selection.tiles.insert(tile_ref);
                            self.start_range = Some(index);
                        }
                    } else if modifiers.command_only() {
                        if selected {
                            selection.tiles.remove(&tile_ref);
                            self.start_range = None;
                        } else {
                            selection.tiles.insert(tile_ref);
                            self.start_range = Some(index);
                        }
                        self.last_range = None;
                    } else {
                        selection.tiles.clear();
                        selection.tiles.insert(tile_ref);
                        self.start_range = Some(index);
                        self.last_range = None;
                    }
                } else if res.drag_delta().length() > 4.0 {
                    if !selected {
                        selection.tiles.clear();
                        selection.tiles.insert(tile_ref);
                        self.start_range = None;
                        self.last_range = None;
                    }
                    self.drag_start = Some(res.rect.center());
                }
            }
        });

        if let Some(range) = deselect_range {
            for tile_ref in range {
                selection.tiles.remove(&tile_ref);
            }
        }

        if let Some(range) = select_range {
            let mut added = Vec::new();
            for index in range {
                let tile_id = tileset.tile_order.get(index).unwrap();
                let tile_ref = TileRef {
                    tileset: tileset_id,
                    tile: *tile_id
                };
                added.push(tile_ref.clone());
                selection.tiles.insert(tile_ref);
            }
            self.last_range = Some(added);
        }

        if let Some(drag_start) = self.drag_start {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                let delta = pos - drag_start;
                let mut delta_transform = TSTransform::default();
                delta_transform.translation = delta;
                ui.ctx().transform_layer_shapes(drag_layer, delta_transform);
            }
        }

        if let Some(mut insert_index) = drop_index {
            let mut tileset = tilesets.get_mut(tileset_id).unwrap();
            let mut moved = Vec::new();

            for (index, tile_id) in tileset.tile_order.iter().enumerate() {
                let tile_ref = TileRef {
                    tileset: tileset_id,
                    tile: *tile_id
                };
                if selection.tiles.contains(&tile_ref) {
                    moved.push((*tile_id, index));
                    if index < insert_index {
                        insert_index -= 1;
                    }
                }
            }

            moved.reverse();

            for (_, index) in moved.iter() {
                tileset.tile_order.remove(*index);
            }
            for (tile_id, _) in moved.iter() {
                tileset.tile_order.insert(insert_index, *tile_id);
            }
        }
    }
}

#[derive(Default)]
pub struct LayersPanel;

impl BasicWidget for LayersPanel {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        fn_widget::<ui::widgets::PanelTitle>(world, ui, id.with("title"), "Layers");
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 25.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                basic_widget::<LayersList>(world, ui, id.with("layer_list"));
                ui.allocate_space(ui.available_size());
            });
        basic_widget::<LayersButton>(world, ui, id.with("layers_buttons"));
    }
}

#[derive(Default)]
pub struct LayersList;

impl BasicWidget for LayersList {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let state = world.resource::<EditorState>();
        let mut active_layer = state.active_layer.unwrap_or(Entity::PLACEHOLDER);
        let mut layers= world.query::<(Entity, &map::Layer)>();
        let mut changed = false;
        let layout = egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true);

        ui.with_layout(layout, |ui| {
            for (layer_id, layer) in layers.iter(world) {
                changed |= ui
                    .selectable_value(&mut active_layer, layer_id, &layer.name)
                    .changed();
            }
        });

        if changed {
            let mut state = world.resource_mut::<EditorState>();
            state.active_layer = Some(active_layer);
        }
    }
}

#[derive(Default)]
pub struct LayersButton {
    show_popup: bool
}

impl BasicWidget for LayersButton {
    fn new(_world: &mut World, _ui: &egui::Ui) -> Self {
        Self::default()
    }

    fn draw(&mut self, world: &mut World, ui: &mut egui::Ui, id: egui::Id) {
        let res = ui.button("➕");
        if res.clicked() {
            self.show_popup = true;
        }

        popup_widget::<CreateLayerPopup>(&mut self.show_popup, &res, world, ui, id.with("popup"));
    }
}

#[derive(Default, Clone)]
pub struct CreateLayerPopup {
    name: String 
}

impl PopupWidget for CreateLayerPopup {
    fn new(_world: &mut World, _ui: &mut egui::Ui) -> Self {
        Self {
            name: "New Layer".to_string()
        }
    }

    fn draw(
            &mut self,
            world: &mut World,
            ui: &mut egui::Ui,
            id: egui::Id
    ) -> bool {
        ui.horizontal(|ui| {
            ui.set_width(200.0);
            let res = ui.text_edit_singleline(&mut self.name);
            if ui.button("Create").clicked() {
                let mut query = world.query_filtered::<Entity, With<map::Map>>();
                let map = query.single(world).unwrap();

                world 
                    .spawn((
                        Name::new(format!("layer: {}", self.name)),
                        map::Layer::new(std::mem::take(&mut self.name)),
                        Transform::default(),
                        Visibility::default()
                    ))
                    .insert(ChildOf(map));

                return false;
            }
            res.request_focus();
            true 
        })    
        .inner
    }
}