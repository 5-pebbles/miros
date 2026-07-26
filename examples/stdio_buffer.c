#include <stdio.h>

// printf with no trailing newline only hits the fd on exit-flush.
int main() {
    printf("printf-no-newline ");
    fputs("fputs ", stdout);
    fwrite("fwrite ", 1, 7, stdout);
    putchar('!');
    puts("");
    puts("puts-line");
    fprintf(stdout, "fprintf %d %s\n", 42, "ok");
    return 0;
}
