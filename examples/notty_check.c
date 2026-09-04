// NO-TTY STDOUT
// NO-TTY STDERR
#include <stdio.h>
#include <unistd.h>

int main(void) {
    // WAIT-FOR "stdout tty=0"
    printf("stdout tty=%d\n", isatty(STDOUT_FILENO));
    // STDERR
    // WAIT-FOR "stderr tty=0"
    fprintf(stderr, "stderr tty=%d\n", isatty(STDERR_FILENO));
    return 0;
}
