// gcc -o ./examples/thread_local ./examples/thread_local.c -Wl,--dynamic-linker=./target/x86_64-unknown-linux-gnu/release/libmiros.so
#include <stdio.h>

__thread int counter = 42;
__thread int zero_init;

int main() {
    printf("counter = %d\n", counter);
    printf("zero_init = %d\n", zero_init);

    counter += 1;
    zero_init = 7;

    printf("counter = %d\n", counter);
    printf("zero_init = %d\n", zero_init);

    return 0;
}
