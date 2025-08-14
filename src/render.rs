use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::{camera::RenderTarget, view::RenderLayers},
    scene::SceneInstance
};
use std::collections::VecDeque;

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup, render_thumbnails));
    }
}

fn setup(mut commands: Commands) {
    commands.insert_resource(RenderQueue::default());

    commands.spawn((
        Name::new("thumbnail_render::camera"),
        ThumbnailCamera,
        bevy::render::view::RenderLayers::layer(1),
        Camera3d::default(),
    ));
}

#[derive(Resource, Default, Debug)]
pub struct RenderQueue {
    queue: VecDeque<(Handle<Image>, Handle<Scene>)>,
    scene: Option<Entity>
}

impl RenderQueue {
    pub fn push(
        &mut self,
        image: Handle<Image>,
        scene: Handle<Scene>
    ) {
        self.queue.push_back((image, scene));
    }
}

#[derive(Component)]
struct ThumbnailCamera;

#[derive(Clone, Copy, Component)]
struct ThumbnailScene;

fn render_thumbnails(
    mut commands: Commands,  
    mut render_queue: ResMut<RenderQueue>,
    mut camera: Query<(&mut Camera, &RenderLayers), With<ThumbnailCamera>>,
    scene_instances: Query<&SceneInstance, With<ThumbnailScene>>,
    scene_manager: Res<SceneSpawner>
) {
    let (mut camera, render_layers) = camera 
        .single_mut()
        .expect("a single Thumbnail Camera to exist");

    if let Some(scene) = render_queue.scene {
        if let Ok(instance) = scene_instances.get(scene) {
            if !scene_manager.instance_is_ready(**instance) {
                debug!("scene not loaded {:?}", scene);
                return;
            }

            for entity in scene_manager.iter_instance_entities(**instance) {
                commands.entity(entity).insert(render_layers.clone());
            }

            debug!("render thumbnail {:?}", scene);
            camera.is_active = true;
            commands
                .entity(scene)
                .remove::<ThumbnailScene>()
                .insert(Visibility::Visible);

            return;
        } else {
            debug!("despawn thumbnail {:?}", scene);
            camera.is_active = false;
            commands.entity(scene).despawn();
            render_queue.scene = None;
        }
    }

    let Some((image, scene)) = render_queue.queue.pop_front() else { return };

    camera.target = RenderTarget::Image(image.into());

    let entity = commands
        .spawn((
            ThumbnailScene,
            SceneRoot(scene),
            render_layers.clone()
        ))
        .id();

    render_queue.scene = Some(entity);
    debug!("spawn thumbnail {:?}", entity);
}