// gcc -o ./examples/print_deadbeef ./examples/print_deadbeef.c -lm -Wl,--dynamic-linker=./target/debug/libdryadv2.so
#include <stdio.h>

int main(){
  int deadbeef = 0xdeadbeef;
  printf("0x%x\n", deadbeef);
  return 0;
}
