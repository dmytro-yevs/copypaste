// Native caller fixture. Its two builds share a signing identity, not a cdhash.
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>
#ifndef FIXTURE_VERSION
#define FIXTURE_VERSION 1
#endif
int main(int argc, char **argv) {
    if (argc != 3) return 90;
    int input[2], output[2];
    if (pipe(input) || pipe(output)) return 91;
    pid_t child = fork();
    if (child == 0) {
        dup2(input[0], STDIN_FILENO);
        dup2(output[1], STDOUT_FILENO);
        close(input[0]); close(input[1]); close(output[0]); close(output[1]);
        execl(argv[1], argv[1], argv[2], NULL);
        _exit(92);
    }
    if (child < 0) return 93;
    close(input[0]); close(output[1]);
    unsigned char expected[32], actual[33];
    memset(expected, 0x42, sizeof(expected));
    if (!strcmp(argv[2], "create") && write(input[1], expected, sizeof(expected)) != sizeof(expected)) return 94;
    close(input[1]);
    size_t count = 0;
    while (count < sizeof(actual)) {
        ssize_t n = read(output[0], actual + count, sizeof(actual) - count);
        if (n <= 0) break;
        count += (size_t)n;
    }
    close(output[0]);
    int status;
    if (waitpid(child, &status, 0) < 0 || !WIFEXITED(status)) return 95;
    if (WEXITSTATUS(status)) return WEXITSTATUS(status);
    if (!strcmp(argv[2], "load") && (count != sizeof(expected) || memcmp(actual, expected, sizeof(expected)))) return 96;
    memset_s(actual, sizeof(actual), 0, sizeof(actual));
    printf("authenticated fixture build %d passed\n", FIXTURE_VERSION);
    return 0;
}
