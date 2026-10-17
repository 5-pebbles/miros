use std::ptr;

use crate::objects::object_data::ObjectData;

/// Applies RELR relocations. RELR is the implicit-addend form: each slot holds its own link-time address, so the base is added in place. A bitmap entry covers the 63 words after the previous address.
#[cfg(target_arch = "x86_64")]
pub(super) unsafe fn apply(object_data: &ObjectData) {
    let Some(entries) = object_data.dynamic_fields.relr_slice() else {
        return;
    };

    let base = object_data.base.addr() as usize;
    let mut location: *mut usize = ptr::null_mut();

    for &entry in entries {
        if entry & 1 == 0 {
            location = (base + entry as usize) as *mut usize;
            *location = (*location).wrapping_add(base);
            location = location.add(1);
            continue;
        }

        // Bit 0 is the bitmap marker; each remaining set bit selects one of the next 63 words.
        let mut bitmap = entry >> 1;
        while bitmap != 0 {
            let slot = location.add(bitmap.trailing_zeros() as usize);
            *slot = (*slot).wrapping_add(base);
            bitmap &= bitmap - 1;
        }
        location = location.add(63);
    }
}
