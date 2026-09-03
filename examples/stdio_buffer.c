#include <stdio.h>

/* printf with no trailing newline only hits the fd on exit-flush. */
int main() {
    // WAIT-FOR "printf-no-newline fputs fwrite !"
    printf("printf-no-newline ");
    fputs("fputs ", stdout);
    fwrite("fwrite ", 1, 7, stdout);
    putchar('!');
    puts("");
    // WAIT-FOR "puts-line"
    puts("puts-line");
    // WAIT-FOR "fprintf 42 ok"
    fprintf(stdout, "fprintf %d %s\n", 42, "ok");
    return 0;
}
