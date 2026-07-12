#include <stdio.h>

// Built with -O2, glibc inlines putchar_unlocked to `*stdout->_IO_write_ptr++ = c`,
// baking the offset of _IO_write_ptr (40) into this binary's .text. Correct output proves
// miros's FILE places write_ptr at that offset and its flush drains the same field.
int main() {
    for (int i = 0; i < 5; i++)
        putchar_unlocked('A' + i);
    putchar_unlocked('\n');
    return 0;
}
