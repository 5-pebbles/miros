#include <pthread.h>
#include <stdint.h>
#include <unistd.h>

void *thread_fn(void *arg) {
  (void)arg;
  write(STDOUT_FILENO, "hello from thread\n", 18);
  return 0;
}

int main() {
  pthread_t thread;
  pthread_create(&thread, NULL, thread_fn, NULL);

  void *retval = (void *)(intptr_t)1;
  pthread_join(thread, &retval);
  write(STDOUT_FILENO, "thread joined\n", 14);

  return (int)(intptr_t)retval;
}
