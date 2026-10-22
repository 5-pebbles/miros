#include <dlfcn.h>
#include <stdio.h>

// SHELL "gcc -shared -fPIC -o libplugin.so plugin.c"

int main(void) {
    void *handle = dlopen("./libplugin.so", RTLD_NOW);
    if (handle == NULL) {
        printf("dlopen failed: %s\n", dlerror());
        return 1;
    }

    int (*answer)(void) = dlsym(handle, "plugin_answer");
    int (*tls_value)(void) = dlsym(handle, "plugin_tls_value");
    int (*ifunc_value)(void) = dlsym(handle, "plugin_ifunc_value");
    if (answer == NULL || tls_value == NULL || ifunc_value == NULL) {
        printf("dlsym failed: %s\n", dlerror());
        return 1;
    }

    if (answer() != 42 || tls_value() != 7 || ifunc_value() != 5) {
        printf("plugin value wrong\n");
        return 1;
    }

    dlclose(handle);

    // WAIT-FOR "plugin ok"
    printf("plugin ok\n");
    return 0;
}
