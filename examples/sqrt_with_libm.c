// gcc -o ./examples/sqrt_with_libm ./examples/sqrt_with_libm.c -lm -Wl,--dynamic-linker=./target/debug/libdryadv2.so
#include<math.h>

int main () {
  float s = sqrt(4.0);

  return 0;
}

