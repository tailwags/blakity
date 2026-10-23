/* Freestanding stand-in for <string.h>: what upstream/c uses. compiler_builtins
 * provides the definitions. */
#pragma once
#include <stddef.h>
void *memcpy(void *restrict dest, const void *restrict src, size_t n);
void *memset(void *dest, int c, size_t n);
int memcmp(const void *a, const void *b, size_t n);
size_t strlen(const char *s);
