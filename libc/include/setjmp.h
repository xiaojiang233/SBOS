#ifndef _SBOS_SETJMP_H
#define _SBOS_SETJMP_H
#include <stdint.h>

typedef struct { uint64_t _registers[8]; } __sbos_jmp_buf;
typedef __sbos_jmp_buf jmp_buf[1];

int setjmp(jmp_buf environment) __attribute__((returns_twice));
int _setjmp(jmp_buf environment) __attribute__((returns_twice));
void longjmp(jmp_buf environment, int value) __attribute__((noreturn));
void _longjmp(jmp_buf environment, int value) __attribute__((noreturn));

#endif
