use std::ffi::c_void;

use crate::{
    error::MirosError,
    objects::{
        object_data::{ThreadLocalAllocation, ThreadLocalData},
        object_data_graph::ObjectDataGraph,
        strategies::Stratagem,
    },
    syscall::thread_pointer::get_thread_pointer,
    tls::{get_tls_allocator, template::TlsTemplate, TlsAllocator},
};

pub struct ThreadLocalStorage;

impl ThreadLocalStorage {
    unsafe fn allocate_tls_module(
        allocator: &mut TlsAllocator,
        tls_data: &mut ThreadLocalData,
        base: *const c_void,
        thread_pointer: *mut c_void,
    ) -> Result<(), MirosError> {
        let template = TlsTemplate::from_program_header(base, &tls_data.tls_program_header);

        let module_id = allocator
            .register_module(template, thread_pointer)
            .ok_or(MirosError::TlsAllocationFailed)?;
        let block_offset = allocator.module(module_id).block_offset;

        tls_data.thread_local_allocation =
            Some(ThreadLocalAllocation::new(module_id, block_offset));

        Ok(())
    }
}

impl Stratagem for ThreadLocalStorage {
    fn run(&self, object_data: &mut ObjectDataGraph) -> Result<(), MirosError> {
        let mutex = get_tls_allocator();
        let mut allocator = mutex.lock().unwrap();
        let thread_pointer = unsafe { get_thread_pointer() };

        object_data
            .iter_objects_mut()
            .filter_map(|object| {
                let base = object.base;
                object.tls_data.as_mut().map(|tls_data| (base, tls_data))
            })
            .try_for_each(|(base, tls_data)| unsafe {
                Self::allocate_tls_module(&mut allocator, tls_data, base, thread_pointer)
            })
    }
}
