use std::{ffi::c_void, mem::MaybeUninit, ptr, sync::Mutex};

use crate::{
    tls::{
        layout_allocator::TlsLayoutAllocator,
        module_registry::{ModuleAllocation, ModuleRegistry},
        template::TlsTemplate,
        thread_control_block::ThreadControlBlock,
    },
    utils::linked_list::LinkedList,
};

mod layout_allocator;
pub mod module_registry;
pub mod template;
pub mod thread_control_block;

pub const TLS_RESERVE_SIZE: usize = 8 * 1024 * 1024;
static mut TLS_ALLOCATOR: MaybeUninit<Mutex<TlsAllocator>> = MaybeUninit::uninit();

pub unsafe fn set_tls_allocator(miros_template: Option<TlsTemplate>) {
    #[allow(static_mut_refs)]
    TLS_ALLOCATOR.write(Mutex::new(TlsAllocator::new(miros_template)));
}

pub fn get_tls_allocator() -> &'static Mutex<TlsAllocator> {
    #[allow(static_mut_refs)]
    unsafe {
        TLS_ALLOCATOR.assume_init_ref()
    }
}

pub struct TlsAllocator {
    generation: usize,
    layout: TlsLayoutAllocator,
    registry: ModuleRegistry,
    threads: LinkedList<ThreadControlBlock>,
    miros_template: Option<TlsTemplate>,
}

impl TlsAllocator {
    fn new(miros_template: Option<TlsTemplate>) -> Self {
        Self {
            generation: 0,
            layout: TlsLayoutAllocator::new(),
            registry: ModuleRegistry::new(),
            threads: LinkedList::new(),
            miros_template,
        }
    }

    pub fn generation(&self) -> usize {
        self.generation
    }

    pub fn miros_template(&self) -> Option<&TlsTemplate> {
        self.miros_template.as_ref()
    }

    pub unsafe fn register_module(
        &mut self,
        template: TlsTemplate,
        thread_pointer: *mut c_void,
    ) -> Option<usize> {
        let block_offset = self
            .layout
            .allocate_block(template.block_size, template.alignment)?;
        self.generation += 1;
        let module_id = self.registry.count();
        self.registry.push(ModuleAllocation {
            block_offset,
            template,
            generation: self.generation,
        });
        // TODO: iterate `self.threads` and initialize this block on every existing thread if not using dynamic model
        Self::initialize_block(&template, block_offset, thread_pointer);
        Some(module_id)
    }

    pub unsafe fn initialize_thread_tls(&self, thread_pointer: *mut c_void) {
        if let Some(template) = &self.miros_template {
            let miros_offset = size_of::<ThreadControlBlock>() as isize;
            Self::initialize_block(template, miros_offset, thread_pointer);
        }
        // TODO: Init thread control block...

        for allocation in self.registry.iter() {
            Self::initialize_block(
                &allocation.template,
                allocation.block_offset,
                thread_pointer,
            );
        }
    }

    unsafe fn initialize_block(
        template: &TlsTemplate,
        block_offset: isize,
        thread_pointer: *mut c_void,
    ) {
        let block_destination = thread_pointer.byte_offset(block_offset) as *mut u8;
        ptr::copy_nonoverlapping(
            template.template_pointer,
            block_destination,
            template.template_size,
        );
        ptr::write_bytes(
            block_destination.add(template.template_size),
            0,
            template.block_size - template.template_size,
        );
    }

    pub fn module(&self, module_id: usize) -> &ModuleAllocation {
        self.registry.get(module_id)
    }

    pub fn modules_since(
        &self,
        generation: usize,
    ) -> impl Iterator<Item = (usize, &ModuleAllocation)> {
        self.registry.since(generation)
    }
}
