//! Delayed [`World`] mutation
//!
//! # Usage
//!
//! Command has mainly two usages:
//!
//! 1. Parallel execution  
//! We can create mutation commands in parallel and then apply them all in a sync point.
//!
//! 2. Mutate the world while borrowing some data from it  
//! Inserting/removing data from the [`World`] requires `&mut World`, and we can delay the mutation
//! with commands until we get the whole `&mut World`. For the same purpose, we also have
//! [`World::res_scope`].
//!
//! # Attribution
//! The source code is copied from [Bevy Engine][bevy]
//!
//! [bevy]: https://github.com/bevyengine/bevy

use std::{fmt, marker::PhantomData};

use crate::{
    world::{ent::Entity, ComponentSet},
    World,
};

/// A [`World`] mutation.
pub trait Command: Send + Sync + 'static {
    fn write(self, world: &mut World);
}

struct CommandMeta {
    offset: usize,
    func: unsafe fn(value: *mut u8, world: &mut World),
}

/// A queue of [`Command`]s
//
// NOTE: [`CommandQueue`] is implemented via a `Vec<u8>` over a `Vec<Box<dyn Command>>`
// as an optimization. Since commands are used frequently in systems as a way to spawn
// entities/components/resources, and it's not currently possible to parallelize these
// due to mutable [`World`] access, maximizing performance for [`CommandQueue`] is
// preferred to simplicity of implementation.
#[derive(Default)]
pub struct CommandQueue {
    bytes: Vec<u8>,
    metas: Vec<CommandMeta>,
}

impl fmt::Debug for CommandQueue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CommandQueue").finish()
    }
}

// SAFE: All commands [`Command`] implement [`Send`]
unsafe impl Send for CommandQueue {}

// SAFE: `&CommandQueue` never gives access to the inner commands.
unsafe impl Sync for CommandQueue {}

impl CommandQueue {
    /// Push a [`Command`] onto the queue.
    #[inline]
    pub fn push<C>(&mut self, command: C)
    where
        C: Command,
    {
        /// SAFE: This function is only every called when the `command` bytes is the associated
        /// [`Commands`] `T` type. Also this only reads the data via `read_unaligned` so unaligned
        /// accesses are safe.
        unsafe fn write_command<T: Command>(command: *mut u8, world: &mut World) {
            let command = command.cast::<T>().read_unaligned();
            command.write(world);
        }

        let size = std::mem::size_of::<C>();
        let old_len = self.bytes.len();

        self.metas.push(CommandMeta {
            offset: old_len,
            func: write_command::<C>,
        });

        if size > 0 {
            self.bytes.reserve(size);

            // SAFE: The internal `bytes` vector has enough storage for the
            // command (see the call the `reserve` above), and the vector has
            // its length set appropriately.
            // Also `command` is forgotten at the end of this function so that
            // when `apply` is called later, a double `drop` does not occur.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &command as *const C as *const u8,
                    self.bytes.as_mut_ptr().add(old_len),
                    size,
                );
                self.bytes.set_len(old_len + size);
            }
        }

        std::mem::forget(command);
    }

    /// Execute the queued [`Command`]s in the world.
    /// This clears the queue.
    #[inline]
    pub fn apply(&mut self, world: &mut World) {
        // flush the previously queued entities
        world.synchronize();

        // SAFE: In the iteration below, `meta.func` will safely consume and drop each pushed command.
        // This operation is so that we can reuse the bytes `Vec<u8>`'s internal storage and prevent
        // unnecessary allocations.
        unsafe { self.bytes.set_len(0) };

        let byte_ptr = if self.bytes.as_mut_ptr().is_null() {
            // SAFE: If the vector's buffer pointer is `null` this mean nothing has been pushed to its bytes.
            // This means either that:
            //
            // 1) There are no commands so this pointer will never be read/written from/to.
            //
            // 2) There are only zero-sized commands pushed.
            //    According to https://doc.rust-lang.org/std/ptr/index.html
            //    "The canonical way to obtain a pointer that is valid for zero-sized accesses is NonNull::dangling"
            //    therefore it is safe to call `read_unaligned` on a pointer produced from `NonNull::dangling` for
            //    zero-sized commands.
            unsafe { std::ptr::NonNull::dangling().as_mut() }
        } else {
            self.bytes.as_mut_ptr()
        };

        for meta in self.metas.drain(..) {
            // SAFE: The implementation of `write_command` is safe for the according Command type.
            // The bytes are safely cast to their original type, safely read, and then dropped.
            unsafe {
                (meta.func)(byte_ptr.add(meta.offset), world);
            }
        }
    }
}

impl<F> Command for F
where
    F: FnOnce(&mut World) + Send + Sync + 'static,
{
    fn write(self, world: &mut World) {
        self(world);
    }
}

/// Spanws an [`Entity`] with components onto [`World`]
#[derive(Debug)]
pub struct Spawn<T> {
    pub comp: T,
}

impl<T> Spawn<T> {
    pub fn new(components: T) -> Self {
        Self { comp: components }
    }
}

impl<T: ComponentSet> Command for Spawn<T> {
    fn write(self, world: &mut World) {
        let entity = world.spawn_empty();
        world.insert_set(entity, self.comp);
    }
}

/// Despawns an [`Entity`] from the [`World`]
#[derive(Debug)]
pub struct Despawn {
    pub entity: Entity,
}

impl Command for Despawn {
    fn write(self, world: &mut World) {
        if !world.despawn(self.entity) {
            log::warn!("Could not despawn entity {:?} because it doesn't exist in this World.\n\
                    If this command was added to a newly spawned entity, ensure that you have not despawned that entity within the same stage.\n\
                    This may have occurred due to system order ambiguity, or if the spawning system has multiple command buffers", self.entity);
        }
    }
}

/// Inserts [`ComponentSet`] to the [`World`]
#[derive(Debug)]
pub struct Insert<T> {
    pub entity: Entity,
    pub comp: T,
}

impl<T> Command for Insert<T>
where
    T: ComponentSet,
{
    fn write(self, world: &mut World) {
        if world.contains(self.entity) {
            world.insert_set(self.entity, self.comp);
        } else {
            panic!("Could not add a component (of type `{}`) to entity {:?} because it doesn't exist in this World.\n\
                    If this command was added to a newly spawned entity, ensure that you have not despawned that entity within the same stage.\n\
                    This may have occurred due to system order ambiguity, or if the spawning system has multiple command buffers", std::any::type_name::<T>(), self.entity);
        }
    }
}

/// Removes [`ComponentSet`] of an entity from the [`World`]
#[derive(Debug)]
pub struct Remove<T> {
    pub entity: Entity,
    pub _ty: PhantomData<T>,
}

impl<T: ComponentSet> Command for Remove<T> {
    fn write(self, world: &mut World) {
        if world.contains(self.entity) {
            // remove intersection to gracefully handle components that were removed before running
            // this command
            world.remove_set::<T>(self.entity);
        }
    }
}

// TODO: `Resource` must be send and sync
// pub struct SetResource<R: Resource> {
//     pub res: R,
// }
//
// impl<R: Resource> Command for SetResource<R> {
//     fn write(self, world: &mut World) {
//         world.set_res(self.res);
//     }
// }
//
// pub struct TakeResource<R: Resource> {
//     pub phantom: PhantomData<R>,
// }
//
// impl<R: Resource> Command for TakeResource<R> {
//     fn write(self, world: &mut World) {
//         let _ = world.take_res::<R>();
//     }
// }
