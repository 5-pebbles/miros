#define _LARGEFILE64_SOURCE
#include <dirent.h>
#include <stdio.h>

int main(int argc, char **argv) {
    const char *path = argc > 1 ? argv[1] : ".";

    DIR *dir = opendir(path);
    if (dir == NULL) {
        puts("opendir failed");
        return 1;
    }
    if (dirfd(dir) < 0) {
        puts("dirfd FAILED");
        return 1;
    }
    // WAIT-FOR "dirfd ok"
    puts("dirfd ok");

    // EXPECT "alpha.txt"
    // EXPECT "omega.txt"
    int count = 0;
    struct dirent *entry;
    while ((entry = readdir(dir)) != NULL) {
        if (entry->d_name[0] == '.')
            continue;
        printf("%s\n", entry->d_name);
        count++;
    }
    // WAIT-FOR "2 entries"
    printf("%d entries\n", count);

    struct dirent64 *entry64;
    DIR *dir64 = opendir(path);
    int count64 = 0;
    while ((entry64 = readdir64(dir64)) != NULL) {
        if (entry64->d_name[0] == '.')
            continue;
        count64++;
    }
    if (count64 != count) {
        puts("readdir64 count mismatch");
        return 1;
    }

    closedir(dir);
    closedir(dir64);
    // WAIT-FOR "list ok"
    puts("list ok");
    return 0;
}
