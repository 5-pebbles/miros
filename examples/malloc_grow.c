#include <stdio.h>
#include <stdlib.h>

/* The 128 KiB class fills its 16 GiB window with 16384 spans of 8 slots, so 131072 untouched allocations exhaust it without committing a single data page. */
/* The allocations past that boundary force the allocator to mint a second window, which must come from a different 16 GiB-aligned base. */
#define WINDOW_CAPACITY 131072
#define PAST_BOUNDARY 16

static unsigned long long window_of(unsigned char *pointer) {
    return (unsigned long long)pointer >> 34;
}

int main(void) {
    unsigned char **pointers = malloc((WINDOW_CAPACITY + PAST_BOUNDARY) * sizeof *pointers);
    if (!pointers) {
        puts("grow failed: pointer array allocation returned null");
        return 1;
    }

    for (int index = 0; index < WINDOW_CAPACITY + PAST_BOUNDARY; index++) {
        /* Deliberately never written: the claims stay in the span bitmaps and metadata. */
        unsigned char *block = malloc(131072);
        if (!block) {
            puts("grow failed: allocation returned null");
            return 1;
        }
        pointers[index] = block;
    }

    unsigned long long first_window = window_of(pointers[0]);
    unsigned long long last_window = window_of(pointers[WINDOW_CAPACITY + PAST_BOUNDARY - 1]);
    if (first_window == last_window) {
        printf("grow failed: no window minted, base %llu\n", first_window);
        return 1;
    }

    // EXPECT "grow ok"
    puts("grow ok");
    return 0;
}
