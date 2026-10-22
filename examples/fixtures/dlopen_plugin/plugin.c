#include <dlfcn.h>
#include <stddef.h>
#include <string.h>

int plugin_answer(void) {
    return 42 + (int)strlen("");
}

__thread int plugin_tls = 7;

int plugin_tls_value(void) {
    return plugin_tls;
}

static int ifunc_impl(void) {
    return 5;
}

static void *ifunc_resolver(void) {
    void *handle = dlopen(NULL, RTLD_NOW);
    (void)handle;
    return (void *)ifunc_impl;
}

int plugin_ifunc(void) __attribute__((ifunc("ifunc_resolver")));

int plugin_ifunc_value(void) {
    return plugin_ifunc();
}
