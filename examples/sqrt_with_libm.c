#include <math.h>
#include <stdio.h>

int main() {
    float value = sqrt(4.0);
    if (value != 2.0f) {
        printf("sqrt FAILED\n");
        return 1;
    }
    // WAIT-FOR "sqrt ok"
    printf("sqrt ok\n");
    return 0;
}
