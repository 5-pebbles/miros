#[derive(Clone, Copy)]
pub struct TlsTemplate {
    pub template_pointer: *const u8,
    pub template_size: usize,
    pub block_size: usize,
    pub alignment: usize,
}

impl TlsTemplate {
    pub fn new(
        template_pointer: *const u8,
        template_size: usize,
        block_size: usize,
        alignment: usize,
    ) -> Self {
        Self {
            template_pointer,
            template_size,
            block_size,
            alignment,
        }
    }
}
