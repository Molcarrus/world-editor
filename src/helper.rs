use bevy::{
    ecs::system::BoxedSystem,
    prelude::*
};
use std::marker::PhantomData;

#[derive(Resource)]
struct InitializedSystem<I, O, S>
where 
    I: Send + 'static,
    O: Send + 'static,
    S: Send + 'static + Sync,
{
    system: BoxedSystem<I, O>,
    _phantom: PhantomData<S>
}

pub fn run_system<I, O, S, Marker>(
    world: &mut World,
    input: I::Inner<'_>,
    system: S 
) -> O 
where 
    I: SystemInput + Send + 'static,
    O: Send + 'static,
    S: IntoSystem<I, O, Marker> + Send + 'static + Sync,
{
    let mut system = match world.remove_resource::<InitializedSystem<I, O, S>>() {
        Some(system) => system,
        None => {
            let mut sys = IntoSystem::into_system(system);
            sys.initialize(world);
            InitializedSystem::<I, O, S> {
                system: Box::new(sys),
                _phantom: PhantomData::<S> {}
            }
        }
    };

    let result = system.system.run(input, world);

    system.system.apply_deferred(world);
    world.insert_resource(system);

    result
}