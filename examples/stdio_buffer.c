#include <stdio.h>

// Exercises the shared buffered stdout: printf, fputs, fwrite, putchar, fprintf, puts.
// The leading printf has no newline, so it only reaches the fd if the stream is flushed
// at exit — the whole point of the buffer.
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
