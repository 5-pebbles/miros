#include <stdio.h>

// Built -O2: glibc inlines write_ptr at offset 40, hardcoded into .text.
int main() {
    for (int i = 0; i < 5; i++)
        putchar_unlocked('A' + i);
    putchar_unlocked('\n');
    return 0;
}
