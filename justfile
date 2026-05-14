build_debug *args:
    cargo rustc {{args}} -- -C link-arg=-nostartfiles -C link-arg=-Wl,-Bsymbolic -C link-arg=-Wl,-e,_start -Z tls-model=initial-exec

build_release *args:
    RUSTFLAGS="-C target-cpu=native -Z unstable-options -C panic=immediate-abort -Z tls-model=initial-exec" \
        cargo rustc -Z build-std=core,alloc,std --target x86_64-unknown-linux-gnu --release {{args}} -- \
        -C link-arg=-nostartfiles -C link-arg=-Wl,-Bsymbolic -C link-arg=-Wl,-e,_start

check *args:
    cargo check {{args}}

test *args:
    cargo test {{args}}

bench *args:
    cargo xtask bench {{args}}
