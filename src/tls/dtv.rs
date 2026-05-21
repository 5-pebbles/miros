use std::sync::atomic::AtomicUsize;

static GLOBAL_GENERATION_COUNTER: AtomicUsize = AtomicUsize::new(0);
