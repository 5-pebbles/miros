#[repr(usize)]
pub enum FutexOperation {
    Wait = 0,
    Wake = 1,
}
