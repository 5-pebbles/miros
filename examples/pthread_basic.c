#include <pthread.h>
#include <unistd.h>

void *thread_fn(void *arg) {
  (void)arg;
  write(STDOUT_FILENO, "hello from thread\n", 18);
  return NULL;
}

int main() {
  pthread_t t;
  pthread_create(&t, NULL, thread_fn, NULL);
  pthread_join(t, NULL);
  write(STDOUT_FILENO, "thread joined\n", 14);
  return 0;
}
