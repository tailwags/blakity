/* Freestanding stand-in for <assert.h>: asserts compile out (as with NDEBUG). */
#pragma once
#define assert(x) ((void)0)
