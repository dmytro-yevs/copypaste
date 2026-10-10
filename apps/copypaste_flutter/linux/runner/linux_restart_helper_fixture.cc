#include "linux_restart_helper.h"

#include <fcntl.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cstdlib>
#include <cstring>
#include <string>

namespace {

constexpr char kMarkerEnvironment[] = "COPYPASTE_RESTART_FIXTURE_MARKER";

bool copy_self_to_private_executable(char* destination_template) {
  const int destination = mkstemp(destination_template);
  if (destination < 0) return false;
  const int source = open("/proc/self/exe", O_RDONLY | O_CLOEXEC);
  if (source < 0) {
    close(destination);
    unlink(destination_template);
    return false;
  }
  char buffer[8192];
  bool copied = true;
  for (;;) {
    const ssize_t received = read(source, buffer, sizeof(buffer));
    if (received == 0) break;
    if (received < 0) {
      copied = false;
      break;
    }
    for (ssize_t offset = 0; offset < received;) {
      const ssize_t written = write(destination, buffer + offset, received - offset);
      if (written <= 0) {
        copied = false;
        break;
      }
      offset += written;
    }
    if (!copied) break;
  }
  const bool executable = copied && fchmod(destination, 0700) == 0;
  close(source);
  close(destination);
  if (!executable) unlink(destination_template);
  return executable;
}

bool marker_contains(const char* marker, const char* expected) {
  gchar* contents = nullptr;
  gsize length = 0;
  const bool matched = g_file_get_contents(marker, &contents, &length, nullptr) &&
      std::strcmp(contents, expected) == 0;
  g_free(contents);
  return matched;
}

}  // namespace

// This fixture is compiled outside the production target. It copies itself to
// a private executable, schedules a restart from a simulated root process,
// and verifies that the copied replacement runs only after that root has
// exited. It never adds test-only behavior to the production helper.
int main(int argc, char** argv) {
  const char* marker_from_environment = g_getenv(kMarkerEnvironment);
  if (marker_from_environment != nullptr && *marker_from_environment != '\0') {
    const int fd = open(marker_from_environment, O_WRONLY | O_APPEND | O_CREAT, 0600);
    if (fd < 0) return 2;
    const char started[] = "replacement\n";
    const ssize_t written = write(fd, started, sizeof(started) - 1);
    close(fd);
    return written == static_cast<ssize_t>(sizeof(started) - 1) ? 0 : 3;
  }

  if (argc != 2 || std::strcmp(argv[1], "--verify") != 0) return 64;
  char executable[] = "/tmp/copypaste-restart-executable-XXXXXX";
  char marker[] = "/tmp/copypaste-restart-marker-XXXXXX";
  const int marker_fd = mkstemp(marker);
  if (marker_fd < 0 || !copy_self_to_private_executable(executable)) {
    if (marker_fd >= 0) {
      close(marker_fd);
      unlink(marker);
    }
    return 4;
  }
  close(marker_fd);
  if (setenv("APPIMAGE", executable, 1) != 0 ||
      setenv(kMarkerEnvironment, marker, 1) != 0) {
    unlink(executable);
    unlink(marker);
    return 5;
  }

  const pid_t root = fork();
  if (root < 0) {
    unlink(executable);
    unlink(marker);
    return 6;
  }
  if (root == 0) {
    GError* error = nullptr;
    const bool scheduled = linux_restart_helper_schedule_current(&error);
    if (!scheduled) {
      if (error != nullptr) g_error_free(error);
      _exit(7);
    }
    const int fd = open(marker, O_WRONLY | O_APPEND);
    if (fd < 0 || write(fd, "parent\n", 7) != 7) _exit(8);
    close(fd);
    _exit(0);
  }

  int status = 0;
  if (waitpid(root, &status, 0) != root || !WIFEXITED(status) ||
      WEXITSTATUS(status) != 0) {
    unlink(executable);
    unlink(marker);
    return 9;
  }
  for (int attempt = 0; attempt < 200; ++attempt) {
    if (marker_contains(marker, "parent\nreplacement\n")) {
      unlink(executable);
      unlink(marker);
      return 0;
    }
    usleep(10 * 1000);
  }
  unlink(executable);
  unlink(marker);
  return 10;
}
