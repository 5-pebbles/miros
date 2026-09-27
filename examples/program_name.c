#define _GNU_SOURCE
#include <stdio.h>
#include <string.h>
#include <sys/auxv.h>

extern char *program_invocation_name;
extern char *program_invocation_short_name;

static int same(const char *left, const char *right) {
    size_t left_length = strlen(left);
    return left_length == strlen(right) && memcmp(left, right, left_length) == 0;
}

static const char *base_name(const char *path) {
    const char *name = path;
    while (*path != '\0') {
        if (*path++ == '/')
            name = path;
    }
    return name;
}

int main(int argc, char **argv) {
    const char *name = argc > 0 ? argv[0] : "";
    const char *execfn = (const char *)getauxval(AT_EXECFN);

    // EXPECT "short=program_name"
    printf("short=%s\n", program_invocation_short_name);
    // EXPECT "execfn-basename=program_name"
    printf("execfn-basename=%s\n", base_name(execfn));
    // EXPECT "full-matches-argv0=1"
    printf("full-matches-argv0=%d\n", same(program_invocation_name, name));
    // EXPECT "execfn-matches-argv0=1"
    printf("execfn-matches-argv0=%d\n", same(execfn, name));
    return 0;
}
