/* Exercises the runner's INPUT & EOF directives: lines arrive on stdin, then ^D closes it. */
// INPUT "one\n"
// INPUT "two\n"
// EOF
#include <stdio.h>
#include <unistd.h>

int main(void) {
    char line[16];
    int lines = 0;
    while (read(STDIN_FILENO, line, sizeof(line)) > 0) {
        lines++;
    }
    // WAIT-FOR "read 2 lines"
    printf("read %d lines\n", lines);
    return 0;
}
