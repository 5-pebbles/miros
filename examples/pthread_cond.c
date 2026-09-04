#include <pthread.h>
#include <stdio.h>

#define SLOTS 8
#define ITEMS 10000
#define CONSUMERS 3

static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t not_full = PTHREAD_COND_INITIALIZER;
static pthread_cond_t not_empty = PTHREAD_COND_INITIALIZER;

static int buffer[SLOTS];
static int count = 0;
static int head = 0;
static int tail = 0;
static int producer_done = 0;

static long consumed_sum[CONSUMERS];
static long consumed_count[CONSUMERS];

static void *producer(void *arg) {
    (void)arg;
    for (int item = 1; item <= ITEMS; item++) {
        pthread_mutex_lock(&mutex);
        while (count == SLOTS)
            pthread_cond_wait(&not_full, &mutex);
        buffer[tail] = item;
        tail = (tail + 1) % SLOTS;
        count++;
        pthread_cond_signal(&not_empty);
        pthread_mutex_unlock(&mutex);
    }
    pthread_mutex_lock(&mutex);
    producer_done = 1;
    pthread_cond_broadcast(&not_empty);
    pthread_mutex_unlock(&mutex);
    return NULL;
}

static void *consumer(void *arg) {
    long index = (long)arg;
    for (;;) {
        pthread_mutex_lock(&mutex);
        while (count == 0 && !producer_done)
            pthread_cond_wait(&not_empty, &mutex);
        if (count == 0 && producer_done) {
            pthread_mutex_unlock(&mutex);
            return NULL;
        }
        consumed_sum[index] += buffer[head];
        consumed_count[index]++;
        head = (head + 1) % SLOTS;
        count--;
        pthread_cond_signal(&not_full);
        pthread_mutex_unlock(&mutex);
    }
}

int main(void) {
    pthread_t producer_thread;
    pthread_t consumer_threads[CONSUMERS];

    pthread_create(&producer_thread, NULL, producer, NULL);
    for (long index = 0; index < CONSUMERS; index++)
        pthread_create(&consumer_threads[index], NULL, consumer, (void *)index);

    pthread_join(producer_thread, NULL);
    for (int index = 0; index < CONSUMERS; index++)
        pthread_join(consumer_threads[index], NULL);

    long total_count = 0;
    long total_sum = 0;
    for (int index = 0; index < CONSUMERS; index++) {
        total_count += consumed_count[index];
        total_sum += consumed_sum[index];
    }
    long expected_sum = (long)ITEMS * (ITEMS + 1) / 2;
    if (total_count != ITEMS || total_sum != expected_sum) {
        printf("cond mismatch: count %ld sum %ld (expected %d %ld)\n",
               total_count, total_sum, ITEMS, expected_sum);
        return 1;
    }
    // WAIT-FOR "cond ok"
    printf("cond ok\n");
    return 0;
}
