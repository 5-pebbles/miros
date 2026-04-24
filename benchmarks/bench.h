// Shared benchmark infrastructure.
//
// Timing goes through a direct clock_gettime syscall (number 228) so both
// glibc and miros builds pay the same overhead — glibc otherwise takes the
// vDSO fast path while miros does not expose a vDSO symbol.

#ifndef BENCH_H
#define BENCH_H

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static uint64_t bench_lcg_state = 0x9e3779b97f4a7c15ULL;

static inline uint32_t bench_lcg_next(void) {
    bench_lcg_state = bench_lcg_state * 6364136223846793005ULL + 1442695040888963407ULL;
    return (uint32_t)(bench_lcg_state >> 32);
}

struct kernel_timespec {
    long tv_sec;
    long tv_nsec;
};

static inline uint64_t monotonic_ns(void) {
    struct kernel_timespec ts;
    long result;
    __asm__ volatile (
        "syscall"
        : "=a"(result)
        : "0"(228), "D"(4), "S"(&ts)  // SYS_clock_gettime, CLOCK_MONOTONIC_RAW
        : "rcx", "r11", "memory"
    );
    (void)result;
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

// Force the compiler to treat `pointer` as observed, preventing it from
// eliding malloc/free pairs whose results aren't otherwise used.
static inline void do_not_optimize(void *pointer) {
    __asm__ volatile ("" : : "r"(pointer) : "memory");
}

// Emit a single phase result as CSV: phase,iterations,total_ns
static void emit(const char *phase, uint64_t iterations, uint64_t elapsed_ns) {
    printf("%s,%llu,%llu\n",
           phase,
           (unsigned long long)iterations,
           (unsigned long long)elapsed_ns);
}

#endif // BENCH_H
