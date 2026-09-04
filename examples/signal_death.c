/* Exercises the runner's SIGNAL & STATUS SIGNAL directives: the kernel's default SIGTERM disposition kills the busy loop. */
// SIGNAL SIGTERM
// STATUS SIGNAL SIGTERM
int main(void) {
    volatile int running = 1;
    while (running) {
    }
    return 0;
}
