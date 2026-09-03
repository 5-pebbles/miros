#include <stdio.h>

__thread int counter = 42;
__thread int zero_init;

int main() {
    // WAIT-FOR "counter = 42"
    printf("counter = %d\n", counter);
    // WAIT-FOR "zero_init = 0"
    printf("zero_init = %d\n", zero_init);

    counter += 1;
    zero_init = 7;

    // WAIT-FOR "counter = 43"
    printf("counter = %d\n", counter);
    // WAIT-FOR "zero_init = 7"
    printf("zero_init = %d\n", zero_init);

    return 0;
}
