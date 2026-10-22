#include <dlfcn.h>
#include <stdio.h>

int main(void) {
    void *handle = dlopen("libm.so.6", RTLD_LAZY);
    if (handle == NULL) {
        printf("dlopen failed: %s\n", dlerror());
        return 1;
    }

    double (*square_root)(double) = dlsym(handle, "sqrt");
    char *error = dlerror();
    if (error != NULL) {
        printf("dlsym failed: %s\n", error);
        return 1;
    }

    double value = square_root(2.0);
    if (value < 1.4142135 || value > 1.4142136) {
        printf("sqrt value wrong\n");
        return 1;
    }

    dlclose(handle);

    // WAIT-FOR "sqrt ok"
    printf("sqrt ok\n");
    return 0;
}
