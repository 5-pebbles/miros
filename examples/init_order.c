// CFLAGS "-Wl,-init,init_function"
#include <stdio.h>

static void preinit(void) {
    // WAIT-FOR "preinit"
    puts("preinit");
}

__attribute__((section(".preinit_array"), used))
static void (*const preinit_entry)(void) = preinit;

void init_function(void) {
    // WAIT-FOR "init function"
    puts("init function");
}

__attribute__((constructor)) static void init_array_first(void) {
    // WAIT-FOR "init array first"
    puts("init array first");
}

__attribute__((constructor)) static void init_array_second(void) {
    // WAIT-FOR "init array second"
    puts("init array second");
}

int main(void) {
    // WAIT-FOR "main"
    puts("main");
    return 0;
}
