#include "linux_restart_helper.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#include <atomic>
#include <string>

namespace {

constexpr char kCanonicalPackageLauncher[] = "/usr/bin/copypaste";
constexpr char kPackagedExecutable[] = "/usr/lib/copypaste/copypaste";
constexpr char kDeletedExecutableSuffix[] = " (deleted)";

std::atomic_bool restart_scheduled = false;

bool is_regular_executable(const std::string& path, uid_t expected_owner,
                           bool reject_symlinks) {
  if (path.empty() || path.front() != '/') return false;
  struct stat metadata = {};
  const int result = reject_symlinks ? lstat(path.c_str(), &metadata)
                                     : stat(path.c_str(), &metadata);
  return result == 0 && S_ISREG(metadata.st_mode) &&
      metadata.st_uid == expected_owner && access(path.c_str(), X_OK) == 0;
}

bool resolve_restart_executable(std::string* executable, GError** error) {
  const gchar* appimage = g_getenv("APPIMAGE");
  if (appimage != nullptr && *appimage != '\0') {
    const std::string appimage_path(appimage);
    if (is_regular_executable(appimage_path, getuid(), true)) {
      *executable = appimage_path;
      return true;
    }
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_NOENT,
                "The AppImage restart launcher is unavailable.");
    return false;
  }

  char target[PATH_MAX] = {};
  const ssize_t size = readlink("/proc/self/exe", target, sizeof(target) - 1);
  if (size <= 0 || size >= static_cast<ssize_t>(sizeof(target) - 1)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_NOENT,
                "The current executable is unavailable.");
    return false;
  }
  target[size] = '\0';
  std::string current(target);
  if (g_str_has_suffix(current.c_str(), kDeletedExecutableSuffix)) {
    const std::string deleted_packaged_executable =
        std::string(kPackagedExecutable) + kDeletedExecutableSuffix;
    if (current != deleted_packaged_executable ||
        !is_regular_executable(kCanonicalPackageLauncher, 0, false)) {
      g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_NOENT,
                  "The packaged CopyPaste launcher is unavailable after update.");
      return false;
    }
    *executable = kCanonicalPackageLauncher;
    return true;
  }
  const bool is_packaged_executable = current == kPackagedExecutable;
  if (!is_regular_executable(current,
                             is_packaged_executable ? 0 : getuid(), false)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_NOENT,
                "The current executable is unavailable.");
    return false;
  }
  *executable = std::move(current);
  return true;
}

void wait_for_parent_exit_and_exec(int read_fd, const char* executable) {
  char buffer[64];
  for (;;) {
    const ssize_t received = read(read_fd, buffer, sizeof(buffer));
    if (received == 0) break;
    if (received < 0 && errno == EINTR) continue;
    if (received < 0) _exit(126);
  }
  close(read_fd);
  char* const argv[] = {const_cast<char*>(executable), nullptr};
  execv(executable, argv);
  _exit(127);
}

}  // namespace

bool linux_restart_helper_schedule_current(GError** error) {
  bool expected = false;
  if (!restart_scheduled.compare_exchange_strong(expected, true)) {
    g_set_error(error, G_FILE_ERROR, G_FILE_ERROR_FAILED,
                "CopyPaste restart is already scheduled.");
    return false;
  }

  std::string executable;
  if (!resolve_restart_executable(&executable, error)) {
    restart_scheduled.store(false);
    return false;
  }

  int completion_pipe[2] = {-1, -1};
  if (pipe(completion_pipe) != 0 ||
      fcntl(completion_pipe[0], F_SETFD, FD_CLOEXEC) != 0 ||
      fcntl(completion_pipe[1], F_SETFD, FD_CLOEXEC) != 0) {
    const int saved_errno = errno;
    if (completion_pipe[0] >= 0) close(completion_pipe[0]);
    if (completion_pipe[1] >= 0) close(completion_pipe[1]);
    restart_scheduled.store(false);
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(saved_errno),
                "Unable to prepare the restart helper: %s", g_strerror(saved_errno));
    return false;
  }

  // Keep this pointer materialized before fork. The child only uses POSIX
  // async-signal-safe operations and never touches C++ string state.
  const char* const replacement = executable.c_str();
  const pid_t helper = fork();
  if (helper < 0) {
    const int saved_errno = errno;
    close(completion_pipe[0]);
    close(completion_pipe[1]);
    restart_scheduled.store(false);
    g_set_error(error, G_FILE_ERROR, g_file_error_from_errno(saved_errno),
                "Unable to start the restart helper: %s", g_strerror(saved_errno));
    return false;
  }
  if (helper == 0) {
    close(completion_pipe[1]);
    wait_for_parent_exit_and_exec(completion_pipe[0], replacement);
  }

  // Keep the write end open until normal process teardown. Its EOF is the
  // synchronization point: the helper cannot start the replacement early.
  close(completion_pipe[0]);
  return true;
}
