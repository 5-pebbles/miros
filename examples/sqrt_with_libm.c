// gcc -o ./examples/sqrt_with_libm ./examples/sqrt_with_libm.c -lm -Wl,--dynamic-linker=./target/x86_64-unknown-linux-gnu/debug/miros
#include<math.h>

int main () {
  float s = sqrt(4.0);

  return 0;
}

