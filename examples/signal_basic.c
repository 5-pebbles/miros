#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static volatile sig_atomic_t caught = 0;
static volatile sig_atomic_t ran_on_alt_stack = 0;
static char alternate_stack[SIGSTKSZ];

static void on_signal(int signal_number) {
    int frame_marker = 0;
    uintptr_t frame_address = (uintptr_t)&frame_marker;
    uintptr_t stack_low = (uintptr_t)alternate_stack;
    uintptr_t stack_high = stack_low + sizeof alternate_stack;
    if (signal_number == SIGUSR2 && frame_address >= stack_low && frame_address < stack_high) {
        ran_on_alt_stack = 1;
    }
    caught = signal_number;
}

int main(void) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = on_signal;
    if (sigaction(SIGUSR1, &action, NULL) != 0) {
        puts("sigaction FAILED");
        return 1;
    }
    raise(SIGUSR1);
    if (caught != SIGUSR1) {
        puts("sigaction FAILED");
        return 1;
    }

    if (signal(SIGUSR2, on_signal) == SIG_ERR) {
        puts("signal FAILED");
        return 1;
    }
    raise(SIGUSR2);
    if (caught != SIGUSR2) {
        puts("signal FAILED");
        return 1;
    }

    stack_t new_stack;
    new_stack.ss_sp = alternate_stack;
    new_stack.ss_flags = 0;
    new_stack.ss_size = sizeof alternate_stack;
    if (sigaltstack(&new_stack, NULL) != 0) {
        puts("sigaltstack FAILED");
        return 1;
    }
    struct sigaction on_stack_action;
    memset(&on_stack_action, 0, sizeof on_stack_action);
    on_stack_action.sa_handler = on_signal;
    on_stack_action.sa_flags = SA_ONSTACK;
    if (sigaction(SIGUSR2, &on_stack_action, NULL) != 0) {
        puts("sigaltstack FAILED");
        return 1;
    }
    raise(SIGUSR2);
    if (!ran_on_alt_stack) {
        puts("sigaltstack FAILED");
        return 1;
    }

    // WAIT-FOR "signals ok"
    puts("signals ok");
    return 0;
}
