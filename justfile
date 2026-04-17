build_debug *args:
    cargo rustc {{args}} -- -C link-arg=-nostartfiles

build_release *args:
    RUSTFLAGS="-C target-cpu=native -Z unstable-options -C panic=immediate-abort" \
        cargo rustc -Z build-std=core,alloc,std --target x86_64-unknown-linux-gnu --release {{args}} -- -C link-arg=-nostartfiles

check *args:
    cargo check {{args}}

test *args:
    cargo test {{args}}
