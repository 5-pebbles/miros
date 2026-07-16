// Exercises the pthread synchronization gate with *static* initializers — the exact shape tokio's
// runtime relies on (PTHREAD_MUTEX_INITIALIZER etc. are all-zero blobs, never passed to _init).
// Expected output: "once\n" exactly once, then "ok\n".
#define _GNU_SOURCE
#include <pthread.h>
#include <unistd.h>

static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t cond = PTHREAD_COND_INITIALIZER;
static pthread_once_t once_control = PTHREAD_ONCE_INIT;
static int ready = 0;

static void init_once(void) {
    write(1, "once\n", 5);
}

static void *worker(void *argument) {
    (void)argument;
    pthread_setname_np(pthread_self(), "worker");
    pthread_once(&once_control, init_once);

    pthread_mutex_lock(&mutex);
    ready = 1;
    pthread_cond_signal(&cond);
    pthread_mutex_unlock(&mutex);
    return (void *)42;
}

int main(void) {
    pthread_t thread;
    pthread_create(&thread, NULL, worker, NULL);
    pthread_once(&once_control, init_once); // races the worker; init must run exactly once

    pthread_mutex_lock(&mutex);
    while (!ready) {
        pthread_cond_wait(&cond, &mutex); // must not miss the worker's signal
    }
    pthread_mutex_unlock(&mutex);

    void *result = NULL;
    pthread_join(thread, &result);
    write(1, result == (void *)42 ? "ok\n" : "bad\n", result == (void *)42 ? 3 : 4);
    return 0;
}
