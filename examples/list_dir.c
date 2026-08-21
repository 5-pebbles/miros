#define _LARGEFILE64_SOURCE
#include <dirent.h>
#include <stdio.h>
#include <string.h>

// Lists the directory named by argv[1] (".": one entry per line), then re-walks it via readdir64 to use both entry points.
int main(int argc, char **argv) {
  const char *path = argc > 1 ? argv[1] : ".";

  DIR *dir = opendir(path);
  if (dir == NULL) {
    puts("opendir failed");
    return 1;
  }
  printf("dirfd %d\n", dirfd(dir));

  int count = 0;
  struct dirent *entry;
  while ((entry = readdir(dir)) != NULL) {
    printf("%s\n", entry->d_name);
    count++;
  }
  printf("%d entries\n", count);

  struct dirent64 *entry64;
  DIR *dir64 = opendir(path);
  int count64 = 0;
  while ((entry64 = readdir64(dir64)) != NULL) {
    count64++;
  }
  if (count64 != count) {
    puts("readdir64 count mismatch");
    return 1;
  }

  closedir(dir);
  closedir(dir64);
  puts("list ok");
  return 0;
}
