#include <stdint.h>
#include <stdio.h>

static void **puts_plt_slot(void) {
    unsigned char *plt;
    __asm__("lea puts@PLT(%%rip), %0" : "=r"(plt));

    /* A PLT entry starts with jmp *disp32(%rip). */
    int32_t displacement;
    __builtin_memcpy(&displacement, plt + 2, sizeof(displacement));
    return (void **)(plt + 6 + displacement);
}

int main(void) {
    void **slot = puts_plt_slot();
    void *before = *slot;

    // EXPECT "lazy binding ok"
    puts("lazy binding ok");
    return before == *slot;
}
