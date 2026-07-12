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

miros := "./target/x86_64-unknown-linux-gnu/release/libmiros.so"
linker_flag := "--dynamic-linker=" + miros

examples: build_release
    mkdir -p examples/bin
    gcc -o examples/bin/print_deadbeef examples/print_deadbeef.c -lm -Wl,{{linker_flag}}
    gcc -o examples/bin/sqrt_with_libm examples/sqrt_with_libm.c -lm -Wl,{{linker_flag}}
    gcc -o examples/bin/thread_local examples/thread_local.c -Wl,{{linker_flag}}
    gcc -o examples/bin/pthread_basic examples/pthread_basic.c -lpthread -Wl,{{linker_flag}}
    gcc -fno-builtin -o examples/bin/thread_dtors examples/thread_dtors.c -lpthread -Wl,{{linker_flag}}
    gcc -o examples/bin/stdio_buffer examples/stdio_buffer.c -Wl,{{linker_flag}}
    gcc -O2 -o examples/bin/putchar_unlocked_o2 examples/putchar_unlocked_o2.c -Wl,{{linker_flag}}
    cargo build --release --manifest-path examples/hello_world/Cargo.toml
