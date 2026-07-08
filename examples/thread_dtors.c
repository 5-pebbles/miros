// Verifies thread-exit destructor parity with glibc: __cxa_thread_atexit_impl
// LIFO drain (a destructor may register another), thread_local dtors before
// pthread key dtors, key values nulled before their destructor runs, re-armed
// keys bounded at PTHREAD_DESTRUCTOR_ITERATIONS (4) rounds, and the main
// thread's tls dtors running at exit. Prints "thread dtors ok" then "main dtor".
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int __cxa_thread_atexit_impl(void (*)(void *), void *, void *);

static char order[64];
static int order_len = 0;
static void record(char c) { order[order_len++] = c; }

static void dtor_c(void *arg) { record('c'); }
static void dtor_b(void *arg) {
    record('b');
    __cxa_thread_atexit_impl(dtor_c, NULL, NULL);
}
static void dtor_a(void *arg) {
    record('a');
    free(arg);
}

static pthread_key_t key;
static int key_rounds = 0;
static void key_dtor(void *value) {
    record(pthread_getspecific(key) == NULL ? 'k' : 'X');
    if (++key_rounds < 10)
        pthread_setspecific(key, (void *)1);
}

static void *thread_fn(void *arg) {
    __cxa_thread_atexit_impl(dtor_a, malloc(32), NULL);
    __cxa_thread_atexit_impl(dtor_b, NULL, NULL);
    pthread_setspecific(key, (void *)1);
    return NULL;
}

static void main_dtor(void *arg) { printf("main dtor\n"); }

int main(void) {
    pthread_key_create(&key, key_dtor);

    pthread_t thread;
    pthread_create(&thread, NULL, thread_fn, NULL);
    pthread_join(thread, NULL);

    if (memcmp(order, "bcakkkk", 8) != 0 || key_rounds != 4) {
        printf("FAIL order=%s rounds=%d\n", order, key_rounds);
        return 1;
    }
    printf("thread dtors ok\n");

    __cxa_thread_atexit_impl(main_dtor, NULL, NULL);
    return 0;
}
