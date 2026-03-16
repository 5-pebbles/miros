build *args:
    cargo rustc {{args}} -- -C link-arg=-nostartfiles

check *args:
    cargo check {{args}}

test *args:
    cargo test {{args}}
