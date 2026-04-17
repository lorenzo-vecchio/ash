/*
 * ash_stdlib.c — C wrappers for stdlib functions used by the Ash compiler backend.
 *
 * Compiled programs link against this file alongside ash_runtime.c:
 *   clang program.ll ash_runtime.c ash_stdlib.c -o program -lm
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <io.h>
#define access _access
#define F_OK 0
#else
#include <unistd.h>
#endif

/* ─── file.* ─────────────────────────────────────────────────────────────────*/

char *ash_file_read(char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = malloc(size + 1);
    if (!buf) { fclose(f); return NULL; }
    fread(buf, 1, size, f);
    buf[size] = '\0';
    fclose(f);
    return buf;
}

void ash_file_write(char *path, char *data) {
    FILE *f = fopen(path, "w");
    if (!f) return;
    fputs(data, f);
    fclose(f);
}

long long ash_file_exists(char *path) {
    return access(path, F_OK) == 0 ? 1 : 0;
}

/* ─── env.* ──────────────────────────────────────────────────────────────────*/

char *ash_env_get(char *key) {
    char *v = getenv(key);
    return v ? v : "";
}
