// NOTE: The thread locals within the tls blocks referenced by the dtv can't register destructors for thread exit.
// The solution is LocalKey<T> used by the thread_local! macro, these rely on pthread_key_create,
// pthread_setspecific, pthread_getspecific, and pthread_key_delete as defined below.
// WARN: As a side effect of that, none of this code can use thread_local!, but it can use the #[thread_local] attribute macro.

use std::{cell::Cell, ffi::c_void, ptr::null_mut, sync::RwLock};

use crate::signature_matches_libc;

#[cfg_attr(not(test), no_mangle)]
static PTHREAD_KEYS_MAX: usize = 128;

pub(super) const PTHREAD_DESTRUCTOR_ITERATIONS: usize = 4;

#[derive(Default, Copy, Clone)]
enum GlobalEntryState {
    #[default]
    Free,
    Allocated {
        destructor: Option<unsafe extern "C" fn(*mut c_void)>,
    },
}

#[derive(Default, Copy, Clone)]
struct GlobalEntry {
    current_generation: usize,
    state: GlobalEntryState,
}

#[derive(Default, Copy, Clone)]
struct ThreadLocalEntry {
    generation: usize,
    value: *mut c_void,
}

static GLOBAL_ENTRIES: RwLock<[GlobalEntry; 128]> = RwLock::new(
    [GlobalEntry {
        current_generation: 0,
        state: GlobalEntryState::Free,
    }; PTHREAD_KEYS_MAX],
);

#[thread_local]
static THREAD_LOCAL_ENTRIES: Cell<[ThreadLocalEntry; 128]> = Cell::new(
    [ThreadLocalEntry {
        generation: 0,
        value: null_mut(),
    }; PTHREAD_KEYS_MAX],
);

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_key_create(
    mut_key_index: *mut u32,
    destructor: Option<unsafe extern "C" fn(*mut c_void)>,
) -> i32 {
    signature_matches_libc!(libc::pthread_key_create(mut_key_index, destructor));

    let mut all_entries = GLOBAL_ENTRIES.write().unwrap();
    all_entries
        .iter_mut()
        .enumerate()
        .find(|(_, entry)| matches!(entry.state, GlobalEntryState::Free))
        .map(|(index, entry)| {
            entry.current_generation += 1;
            entry.state = GlobalEntryState::Allocated { destructor };
            *mut_key_index = index as u32;
            0
        })
        .unwrap_or(libc::EAGAIN)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_key_delete(key_index: u32) -> i32 {
    signature_matches_libc!(libc::pthread_key_delete(key_index));

    let mut all_entries = GLOBAL_ENTRIES.write().unwrap();

    all_entries
        .get_mut(key_index as usize)
        .map(|entry| {
            entry.state = GlobalEntryState::Free;
            entry.current_generation += 1;
            0
        })
        .unwrap_or(libc::EINVAL)
}

fn entry_cells(entries: &Cell<[ThreadLocalEntry; 128]>) -> &[Cell<ThreadLocalEntry>] {
    let entries: &Cell<[ThreadLocalEntry]> = entries;
    entries.as_slice_of_cells()
}

fn live_destructor(
    key_index: usize,
    generation: usize,
) -> Option<unsafe extern "C" fn(*mut c_void)> {
    // Re-read at call time: an earlier destructor this round may have deleted the key.
    let global_entry = *GLOBAL_ENTRIES.read().unwrap().get(key_index)?;
    match global_entry.state {
        GlobalEntryState::Allocated { destructor }
            if global_entry.current_generation == generation =>
        {
            destructor
        }
        _ => None,
    }
}

/// One scan of every key. A destructor that re-arms its key defers to the next round, each key runs at most once per scan.
pub fn run_key_destructor_round() -> bool {
    entry_cells(&THREAD_LOCAL_ENTRIES).iter().enumerate().fold(
        false,
        |ran_any, (key_index, entry_cell)| {
            let local_entry = entry_cell.get();
            if local_entry.value.is_null() {
                return ran_any;
            }

            let Some(destructor) = live_destructor(key_index, local_entry.generation) else {
                return ran_any;
            };

            entry_cell.set(ThreadLocalEntry {
                value: null_mut(),
                ..local_entry
            });
            unsafe { destructor(local_entry.value) };

            true
        },
    )
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_getspecific(key_index: u32) -> *mut c_void {
    signature_matches_libc!(libc::pthread_getspecific(key_index));

    GLOBAL_ENTRIES
        .read()
        .unwrap()
        .get(key_index as usize)
        .map(
            |GlobalEntry {
                 current_generation, ..
             }| *current_generation,
        )
        .and_then(|current_generation| {
            entry_cells(&THREAD_LOCAL_ENTRIES)
                .get(key_index as usize)
                .map(Cell::get)
                .filter(|ThreadLocalEntry { generation, .. }| *generation == current_generation)
                .map(|ThreadLocalEntry { value, .. }| value)
        })
        .unwrap_or(null_mut())
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn pthread_setspecific(key_index: u32, value: *const c_void) -> i32 {
    signature_matches_libc!(libc::pthread_setspecific(key_index, value));

    GLOBAL_ENTRIES
        .read()
        .unwrap()
        .get(key_index as usize)
        .map(
            |GlobalEntry {
                 current_generation, ..
             }| *current_generation,
        )
        .and_then(|current_generation| {
            entry_cells(&THREAD_LOCAL_ENTRIES)
                .get(key_index as usize)
                .map(|entry_cell| {
                    entry_cell.set(ThreadLocalEntry {
                        generation: current_generation,
                        value: value.cast_mut(),
                    });
                    0
                })
        })
        .unwrap_or(libc::EINVAL)
}
