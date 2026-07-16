// Exercises commit 3's two risk areas under miros:
//   1. a signal handler returning through sa_restorer -> rt_sigreturn (the ABI trap),
//   2. an eventfd registered on an epoll and observed ready (reactor syscall numbers).
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/syscall.h>
#include <unistd.h>

static volatile sig_atomic_t caught_signal = 0;

static void on_signal(int signal_number) {
    caught_signal = signal_number;
}

int main(void) {
    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = on_signal;
    if (sigaction(SIGUSR1, &action, NULL) != 0) {
        puts("sigaction FAILED");
        return 1;
    }

    // Delivered via tgkill rather than raise(), which has an unrelated tid bug.
    // If the trampoline is wrong, control never returns here — it faults on handler return.
    syscall(SYS_tgkill, getpid(), syscall(SYS_gettid), SIGUSR1);
    if (caught_signal != SIGUSR1) {
        puts("signal FAILED");
        return 1;
    }
    puts("signal ok");

    int event_fd = eventfd(0, EFD_NONBLOCK);
    int epoll_fd = epoll_create1(0);
    struct epoll_event registration = {.events = EPOLLIN, .data.fd = event_fd};
    epoll_ctl(epoll_fd, EPOLL_CTL_ADD, event_fd, &registration);

    uint64_t token = 1;
    write(event_fd, &token, sizeof token);

    struct epoll_event ready[1];
    int ready_count = epoll_wait(epoll_fd, ready, 1, 1000);
    if (ready_count != 1 || ready[0].data.fd != event_fd) {
        puts("reactor FAILED");
        return 1;
    }
    puts("reactor ok");
    return 0;
}
