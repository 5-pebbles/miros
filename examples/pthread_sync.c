/* Exercises the pthread synchronization gate with *static* initializers. */
#define _GNU_SOURCE
#include <pthread.h>
#include <unistd.h>

static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t recursive = PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP;
static pthread_rwlock_t rwlock = PTHREAD_RWLOCK_INITIALIZER;
static pthread_cond_t cond = PTHREAD_COND_INITIALIZER;
static pthread_once_t once_control = PTHREAD_ONCE_INIT;
static int ready = 0;

static void init_once(void) {
    write(1, "once\n", 5);
}

/* A detached thread frees its own stack region at exit; hammering the rwlock first makes a broken self-reap (stack use after unmap, double-free) likely to crash. */
static void *detached_worker(void *argument) {
    (void)argument;
    for (int i = 0; i < 100000; i++) {
        pthread_rwlock_rdlock(&rwlock);
        pthread_rwlock_unlock(&rwlock);
    }
    return NULL;
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
    pthread_mutex_lock(&recursive);
    pthread_mutex_lock(&recursive);
    pthread_mutex_unlock(&recursive);
    pthread_mutex_unlock(&recursive);

    pthread_rwlock_wrlock(&rwlock);
    pthread_rwlock_unlock(&rwlock);

    pthread_t detached;
    pthread_create(&detached, NULL, detached_worker, NULL);
    pthread_detach(detached);

    pthread_t thread;
    pthread_create(&thread, NULL, worker, NULL);
    /* races the worker; init must run exactly once */
    // WAIT-FOR "once"
    pthread_once(&once_control, init_once);

    pthread_mutex_lock(&mutex);
    while (!ready) {
        /* must not miss the worker's signal */
        pthread_cond_wait(&cond, &mutex);
    }
    pthread_mutex_unlock(&mutex);

    void *result = NULL;
    pthread_join(thread, &result);
    int ok = result == (void *)42;
    // WAIT-FOR "ok"
    write(1, ok ? "ok\n" : "bad\n", ok ? 3 : 4);
    return !ok;
}
