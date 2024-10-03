pub(crate) const DT_NULL: usize = 0;
pub(crate) const DT_RELA: usize = 7;
pub(crate) const DT_REL: usize = 17;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct DynamicArrayItem {
    pub d_tag: usize,
    pub d_val: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct DynamicArrayIter(*const DynamicArrayItem);

impl DynamicArrayIter {
    pub(crate) fn new(dynamic_array_pointer: *const DynamicArrayItem) -> Self {
        Self(dynamic_array_pointer)
    }

    pub(crate) fn into_inner(self) -> *const DynamicArrayItem {
        self.0
    }
}

impl Iterator for DynamicArrayIter {
    type Item = DynamicArrayItem;

    fn next(&mut self) -> Option<Self::Item> {
        let this = unsafe { *self.0 };
        if this.d_tag == DT_NULL {
            return None;
        }
        self.0 = unsafe { self.0.add(1) };
        Some(this)
    }
}