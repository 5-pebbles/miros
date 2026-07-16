// Exercises commit 4's filesystem layer under miros: the DIR/getdents64 implementation
// (opendir/readdir/closedir/dirfd) and relative-path open/fstat/statx — the AT_FDCWD fix.
// Run from the repo root so the default relative paths resolve.
#define _GNU_SOURCE
#include <dirent.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(int argc, char **argv) {
    const char *directory_path = argc > 1 ? argv[1] : ".";
    const char *file_path = argc > 2 ? argv[2] : "./justfile";

    // Directory iteration via getdents64.
    DIR *directory = opendir(directory_path);
    if (!directory) {
        puts("opendir FAILED");
        return 1;
    }
    int entry_count = 0;
    while (readdir(directory) != NULL) {
        entry_count++;
    }
    int directory_fd = dirfd(directory);
    printf("opendir(%s): %d entries, dirfd=%d\n", directory_path, entry_count, directory_fd);
    closedir(directory);
    if (entry_count < 2 || directory_fd < 0) {
        puts("dirent FAILED");
        return 1;
    }

    // A relative path must resolve against the CWD (AT_FDCWD), not fd 0.
    int file_descriptor = open(file_path, O_RDONLY);
    if (file_descriptor < 0) {
        puts("relative open FAILED");
        return 1;
    }

    struct stat status;
    if (fstat(file_descriptor, &status) != 0 || status.st_size <= 0) {
        puts("fstat FAILED");
        return 1;
    }
    printf("open+fstat(%s): fd=%d, size=%ld\n", file_path, file_descriptor, (long)status.st_size);
    close(file_descriptor);

    // statx on the same path — the metadata source Rust std actually uses.
    struct statx statx_status;
    if (statx(AT_FDCWD, file_path, 0, STATX_SIZE, &statx_status) != 0) {
        puts("statx FAILED");
        return 1;
    }
    printf("statx(%s): size=%llu\n", file_path, (unsigned long long)statx_status.stx_size);
    if (statx_status.stx_size != (unsigned long long)status.st_size) {
        puts("statx size mismatch");
        return 1;
    }

    puts("fs ok");
    return 0;
}
