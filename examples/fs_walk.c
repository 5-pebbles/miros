// Exercises the fs layer under miros: relative-path open (AT_FDCWD), lseek64, and statx.
// Run from the repo root so the default path resolves.
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(int argc, char **argv) {
  const char *file_path = argc > 1 ? argv[1] : "./examples/list_dir.c";

  // A relative path must resolve against the CWD (AT_FDCWD), not fd 0.
  int file_descriptor = open(file_path, O_RDONLY);
  if (file_descriptor < 0) {
    puts("relative open FAILED");
    return 1;
  }

  // lseek64: jump to offset 2, then back to the start, and read the first byte.
  if (lseek64(file_descriptor, 2, SEEK_SET) != 2) {
    puts("lseek64 forward FAILED");
    return 1;
  }
  if (lseek64(file_descriptor, 0, SEEK_SET) != 0) {
    puts("lseek64 rewind FAILED");
    return 1;
  }
  char first_byte = 0;
  if (read(file_descriptor, &first_byte, 1) != 1) {
    puts("read FAILED");
    return 1;
  }
  close(file_descriptor);

  // statx on the same path.
  struct statx status;
  if (statx(AT_FDCWD, file_path, 0, STATX_SIZE, &status) != 0) {
    puts("statx FAILED");
    return 1;
  }
  if (status.stx_size == 0) {
    puts("statx empty size FAILED");
    return 1;
  }

  printf("fs ok: %s first byte='%c', size=%llu\n", file_path, first_byte,
         (unsigned long long)status.stx_size);
  return 0;
}
