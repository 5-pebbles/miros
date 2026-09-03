#include <stdio.h>

int main() {
    int deadbeef = 0xdeadbeef;
    printf("0x%x\n", deadbeef);
    // WAIT-FOR "0xdeadbeef"
    return 0;
}
