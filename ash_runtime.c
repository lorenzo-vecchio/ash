/*
 * ash_runtime.c — small C runtime helpers for the Ash compiled backend.
 *
 * Compiled programs link against this file:
 *   clang program.ll ash_runtime.c -o program -lm
 *
 * Provides:
 *   - ash_list_*    heap-allocated dynamic list of i64 values
 *   - ash_str_*     string utilities (concat, from_int, from_float)
 *   - ash_map_*     simple string-keyed, i64-valued hash map (future)
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ─── List (dynamic array of int64_t) ───────────────────────────────────────*/

typedef struct {
    long long *data;
    long long  len;
    long long  cap;
} AshList;

AshList *ash_list_new(void) {
    AshList *l = malloc(sizeof(AshList));
    l->data = malloc(8 * sizeof(long long));
    l->len  = 0;
    l->cap  = 8;
    return l;
}

void ash_list_push(AshList *l, long long v) {
    if (l->len == l->cap) {
        l->cap *= 2;
        l->data = realloc(l->data, l->cap * sizeof(long long));
    }
    l->data[l->len++] = v;
}

long long ash_list_get(AshList *l, long long idx) {
    if (idx < 0 || idx >= l->len) return 0;
    return l->data[idx];
}

long long ash_list_len(AshList *l) {
    return l->len;
}

/* ─── String utilities ───────────────────────────────────────────────────────*/

char *ash_str_concat(const char *a, const char *b) {
    size_t la = strlen(a), lb = strlen(b);
    char *out = malloc(la + lb + 1);
    memcpy(out, a, la);
    memcpy(out + la, b, lb + 1);
    return out;
}

char *ash_str_from_int(long long n) {
    char *buf = malloc(32);
    snprintf(buf, 32, "%lld", n);
    return buf;
}

char *ash_str_from_float(double f) {
    char *buf = malloc(32);
    snprintf(buf, 32, "%g", f);
    return buf;
}

char *ash_str_from_bool(long long b) {
    return b ? "true" : "false";
}

long long ash_str_len(const char *s) {
    return (long long)strlen(s);
}

/* ─── Map (string-keyed, void* values) ──────────────────────────────────────*/

typedef struct { char *key; void *val; } AshMapEntry;
typedef struct { AshMapEntry *entries; long long len; long long cap; } AshMap;

AshMap *ash_map_new(void) {
    AshMap *m = malloc(sizeof(AshMap));
    m->entries = NULL;
    m->len = 0;
    m->cap = 0;
    return m;
}

void ash_map_set(AshMap *m, char *key, void *val) {
    for (long long i = 0; i < m->len; i++) {
        if (strcmp(m->entries[i].key, key) == 0) { m->entries[i].val = val; return; }
    }
    if (m->len == m->cap) {
        m->cap = m->cap ? m->cap * 2 : 4;
        m->entries = realloc(m->entries, m->cap * sizeof(AshMapEntry));
    }
    m->entries[m->len].key = key;
    m->entries[m->len].val = val;
    m->len++;
}

void *ash_map_get(AshMap *m, char *key) {
    for (long long i = 0; i < m->len; i++) {
        if (strcmp(m->entries[i].key, key) == 0) return m->entries[i].val;
    }
    return NULL;
}

long long ash_map_len(AshMap *m) { return m->len; }
