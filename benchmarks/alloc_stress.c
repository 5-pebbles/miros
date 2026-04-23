// Single-threaded allocation stress test.
//
// Build twice from the same source — the only difference is the dynamic linker:
//
//   gcc -O2 -o alloc_stress_glibc alloc_stress.c
//   gcc -O2 -Wl,--dynamic-linker=path/to/miros -o alloc_stress_miros alloc_stress.c
//
// Output is CSV on stdout: phase,iterations,total_ns

#include <string.h>

#include "bench.h"

// ── phases ──────────────────────────────────────────────────────────────

static void tight_32(void) {
    const uint64_t iterations = 5000000;
    uint64_t start = monotonic_ns();
    for (uint64_t index = 0; index < iterations; index++) {
        void *pointer = malloc(32);
        if (!pointer) abort();
        do_not_optimize(pointer);
        free(pointer);
    }
    emit("tight_32", iterations, monotonic_ns() - start);
}

static void mixed_small(void) {
    const uint64_t iterations = 2000000;
    uint64_t start = monotonic_ns();
    for (uint64_t index = 0; index < iterations; index++) {
        size_t size = (bench_lcg_next() % 2048) + 1;
        void *pointer = malloc(size);
        if (!pointer) abort();
        do_not_optimize(pointer);
        free(pointer);
    }
    emit("mixed_1_to_2048", iterations, monotonic_ns() - start);
}

static void realloc_growth(void) {
    const uint64_t trials = 500000;
    uint64_t operations = 0;
    uint64_t start = monotonic_ns();
    for (uint64_t trial = 0; trial < trials; trial++) {
        void *pointer = malloc(32);
        if (!pointer) abort();
        for (size_t size = 64; size <= 8192; size *= 2) {
            void *grown = realloc(pointer, size);
            if (!grown) abort();
            pointer = grown;
            operations++;
        }
        free(pointer);
    }
    emit("realloc_32_to_8192", operations, monotonic_ns() - start);
}

static void large_256k(void) {
    const uint64_t iterations = 50000;
    const size_t large_size = 256 * 1024;
    uint64_t start = monotonic_ns();
    for (uint64_t index = 0; index < iterations; index++) {
        void *pointer = malloc(large_size);
        if (!pointer) abort();
        memset(pointer, 0xcc, large_size);
        do_not_optimize(pointer);
        free(pointer);
    }
    emit("large_256K", iterations, monotonic_ns() - start);
}

#define CHURN_BATCH 5000
static void churn_shuffled(void) {
    static void *pointers[CHURN_BATCH];
    const uint64_t rounds = 200;
    uint64_t start = monotonic_ns();
    for (uint64_t round = 0; round < rounds; round++) {
        for (size_t index = 0; index < CHURN_BATCH; index++) {
            size_t size = (bench_lcg_next() % 2048) + 1;
            pointers[index] = malloc(size);
            if (!pointers[index]) abort();
        }
        // Fisher-Yates shuffle so frees don't happen in allocation order.
        for (size_t index = CHURN_BATCH - 1; index > 0; index--) {
            size_t swap_index = bench_lcg_next() % (index + 1);
            void *tmp = pointers[index];
            pointers[index] = pointers[swap_index];
            pointers[swap_index] = tmp;
        }
        for (size_t index = 0; index < CHURN_BATCH; index++) {
            free(pointers[index]);
        }
    }
    emit("churn_shuffled", rounds * CHURN_BATCH, monotonic_ns() - start);
}

static void calloc_mixed(void) {
    const uint64_t iterations = 2000000;
    uint64_t start = monotonic_ns();
    for (uint64_t index = 0; index < iterations; index++) {
        size_t count = (bench_lcg_next() % 256) + 1;
        size_t size = (bench_lcg_next() % 64) + 1;
        void *pointer = calloc(count, size);
        if (!pointer) abort();
        do_not_optimize(pointer);
        free(pointer);
    }
    emit("calloc_mixed", iterations, monotonic_ns() - start);
}

// ── main ────────────────────────────────────────────────────────────────

int main(void) {
    uint64_t start = monotonic_ns();
    tight_32();
    mixed_small();
    realloc_growth();
    large_256k();
    churn_shuffled();
    calloc_mixed();
    emit("TOTAL", 1, monotonic_ns() - start);
    return 0;
}
