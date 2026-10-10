#include "linux_packagekit.h"

#include <fcntl.h>
#include <limits.h>
#include <sys/stat.h>
#include <unistd.h>

#include <array>
#include <cstring>
#include <memory>
#include <utility>

#include <gio/gio.h>
#include <glib/gstdio.h>

namespace {

constexpr char kPackageKitService[] = "org.freedesktop.PackageKit";
constexpr char kPackageKitPath[] = "/org/freedesktop/PackageKit";
constexpr char kPackageKitInterface[] = "org.freedesktop.PackageKit";
constexpr char kTransactionInterface[] = "org.freedesktop.PackageKit.Transaction";
constexpr char kMetadataPath[] = "/usr/lib/copypaste/package-metadata.json";
constexpr guint kPackageKitExitSuccess = 1;
constexpr gsize kMaximumMetadataBytes = 4096;

const char* compiled_architecture() {
#if defined(__x86_64__)
  return "x86_64";
#elif defined(__aarch64__)
  return "aarch64";
#else
  return nullptr;
#endif
}

bool safe_regular_file(const char* path, struct stat* status) {
  if (path == nullptr || lstat(path, status) != 0 || !S_ISREG(status->st_mode) ||
      S_ISLNK(status->st_mode)) {
    return false;
  }
  return true;
}

bool root_owned_immutable(const char* path) {
  struct stat status {};
  return safe_regular_file(path, &status) && status.st_uid == 0 &&
         (status.st_mode & (S_IWGRP | S_IWOTH)) == 0;
}

bool executable_has_system_identity() {
  std::array<char, PATH_MAX> executable{};
  const ssize_t length = readlink("/proc/self/exe", executable.data(),
                                  executable.size() - 1);
  if (length <= 0 || length >= static_cast<ssize_t>(executable.size() - 1)) {
    return false;
  }
  executable[static_cast<size_t>(length)] = '\0';
  return root_owned_immutable(executable.data());
}

LinuxPackageKind package_kind_from_metadata(const gchar* contents) {
  if (contents == nullptr) {
    return LinuxPackageKind::kUnavailable;
  }
  if (g_regex_match_simple("\\\"kind\\\"\\s*:\\s*\\\"deb\\\"", contents,
                           G_REGEX_RAW, G_REGEX_MATCH_NOTEMPTY)) {
    return LinuxPackageKind::kDeb;
  }
  if (g_regex_match_simple("\\\"kind\\\"\\s*:\\s*\\\"rpm\\\"", contents,
                           G_REGEX_RAW, G_REGEX_MATCH_NOTEMPTY)) {
    return LinuxPackageKind::kRpm;
  }
  return LinuxPackageKind::kUnavailable;
}

bool metadata_matches_architecture(const gchar* contents, const char* architecture) {
  if (contents == nullptr || architecture == nullptr) {
    return false;
  }
  g_autofree gchar* expression =
      g_strdup_printf("\\\"architecture\\\"\\s*:\\s*\\\"%s\\\"", architecture);
  return g_regex_match_simple(expression, contents, G_REGEX_RAW,
                              G_REGEX_MATCH_NOTEMPTY);
}

bool packagekit_available(std::string* reason) {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GDBusConnection) connection =
      g_bus_get_sync(G_BUS_TYPE_SYSTEM, nullptr, &error);
  if (connection == nullptr) {
    *reason = "The PackageKit system service is unavailable.";
    return false;
  }
  g_autoptr(GVariant) response = g_dbus_connection_call_sync(
      connection, kPackageKitService, kPackageKitPath, "org.freedesktop.DBus.Properties",
      "Get", g_variant_new("(ss)", kPackageKitInterface, "BackendName"),
      G_VARIANT_TYPE("(v)"),
      G_DBUS_CALL_FLAGS_NONE, 5000, nullptr, &error);
  if (response == nullptr) {
    *reason = "The PackageKit system service is unavailable.";
    return false;
  }
  g_autoptr(GVariant) backend_value = g_variant_get_child_value(response, 0);
  if (backend_value == nullptr || !g_variant_is_of_type(backend_value, G_VARIANT_TYPE_VARIANT)) {
    *reason = "The PackageKit system service returned an invalid package backend.";
    return false;
  }
  g_autoptr(GVariant) backend_unboxed = g_variant_get_variant(backend_value);
  const gchar* backend = backend_unboxed != nullptr &&
      g_variant_is_of_type(backend_unboxed, G_VARIANT_TYPE_STRING)
      ? g_variant_get_string(backend_unboxed, nullptr) : nullptr;
  if (backend == nullptr || *backend == '\0') {
    *reason = "The PackageKit system service has no package backend.";
    return false;
  }
  g_clear_error(&error);
  g_autoptr(GVariant) roles_response = g_dbus_connection_call_sync(
      connection, kPackageKitService, kPackageKitPath, "org.freedesktop.DBus.Properties",
      "Get", g_variant_new("(ss)", kPackageKitInterface, "Roles"),
      G_VARIANT_TYPE("(v)"), G_DBUS_CALL_FLAGS_NONE, 5000, nullptr, &error);
  if (roles_response == nullptr) {
    *reason = "The PackageKit system service cannot install local packages.";
    return false;
  }
  g_autoptr(GVariant) roles_value = g_variant_get_child_value(roles_response, 0);
  if (roles_value == nullptr || !g_variant_is_of_type(roles_value, G_VARIANT_TYPE_VARIANT)) {
    *reason = "The PackageKit system service returned invalid package roles.";
    return false;
  }
  g_autoptr(GVariant) roles_unboxed = g_variant_get_variant(roles_value);
  // PK_ROLE_ENUM_INSTALL_FILES is ordinal 10; PackageKit role bitfields use
  // one shifted bit per enum ordinal.
  constexpr guint64 kInstallFilesRole = G_GUINT64_CONSTANT(1) << 10;
  if (roles_unboxed == nullptr || !g_variant_is_of_type(roles_unboxed, G_VARIANT_TYPE_UINT64) ||
      (g_variant_get_uint64(roles_unboxed) & kInstallFilesRole) == 0) {
    *reason = "The PackageKit backend cannot install local packages.";
    return false;
  }
  return true;
}

bool valid_sha256(const std::string& value) {
  if (value.size() != 64) {
    return false;
  }
  for (const char character : value) {
    if (!g_ascii_isxdigit(character)) {
      return false;
    }
  }
  return true;
}

struct FileIdentity {
  dev_t device = 0;
  ino_t inode = 0;
  off_t size = 0;
  time_t modified = 0;
};

bool same_identity(const struct stat& status, const FileIdentity& identity) {
  return status.st_dev == identity.device && status.st_ino == identity.inode &&
         status.st_size == identity.size && status.st_mtime == identity.modified;
}

bool hash_regular_file(const std::string& path, const std::string& expected_sha256,
                       FileIdentity* identity) {
  if (!valid_sha256(expected_sha256)) {
    return false;
  }
  const int descriptor = open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0) {
    return false;
  }
  struct stat before {};
  if (fstat(descriptor, &before) != 0 || !S_ISREG(before.st_mode)) {
    close(descriptor);
    return false;
  }
  g_autoptr(GChecksum) checksum = g_checksum_new(G_CHECKSUM_SHA256);
  std::array<guint8, 64 * 1024> buffer{};
  while (true) {
    const ssize_t read_count = read(descriptor, buffer.data(), buffer.size());
    if (read_count < 0) {
      close(descriptor);
      return false;
    }
    if (read_count == 0) {
      break;
    }
    g_checksum_update(checksum, buffer.data(), static_cast<gsize>(read_count));
  }
  struct stat after {};
  const bool unchanged = fstat(descriptor, &after) == 0 &&
                         before.st_dev == after.st_dev && before.st_ino == after.st_ino &&
                         before.st_size == after.st_size && before.st_mtime == after.st_mtime;
  close(descriptor);
  struct stat current {};
  if (!unchanged || !safe_regular_file(path.c_str(), &current) ||
      current.st_dev != before.st_dev || current.st_ino != before.st_ino ||
      current.st_size != before.st_size || current.st_mtime != before.st_mtime ||
      g_ascii_strcasecmp(g_checksum_get_string(checksum), expected_sha256.c_str()) != 0) {
    return false;
  }
  *identity = {before.st_dev, before.st_ino, before.st_size, before.st_mtime};
  return true;
}

bool matches_expected_package_name(const std::string& path,
                                   const LinuxPackageInstallation& installation) {
  const char* extension = installation.kind == LinuxPackageKind::kDeb ? ".deb" : ".rpm";
  const gchar* basename = g_path_get_basename(path.c_str());
  const bool matches = g_str_has_suffix(basename, extension) &&
                       g_strstr_len(basename, -1,
                                    ("-linux-" + installation.architecture + ".").c_str()) !=
                           nullptr;
  g_free(const_cast<gchar*>(basename));
  return matches;
}

bool stage_verified_package(const std::string& source,
                            const std::string& expected_sha256,
                            std::string* staged_path,
                            std::string* staging_directory,
                            FileIdentity* identity) {
  struct stat source_status {};
  if (!g_path_is_absolute(source.c_str()) ||
      !safe_regular_file(source.c_str(), &source_status)) {
    return false;
  }
  g_autoptr(GError) error = nullptr;
  g_autofree gchar* directory =
      g_dir_make_tmp("copypaste-packagekit-XXXXXX", &error);
  if (directory == nullptr) {
    return false;
  }
  g_autofree gchar* basename = g_path_get_basename(source.c_str());
  g_autofree gchar* destination = g_build_filename(directory, basename, nullptr);
  g_autoptr(GFile) source_file = g_file_new_for_path(source.c_str());
  g_autoptr(GFile) destination_file = g_file_new_for_path(destination);
  if (!g_file_copy(source_file, destination_file, G_FILE_COPY_NONE, nullptr,
                   nullptr, nullptr, &error) ||
      g_chmod(destination, S_IRUSR | S_IWUSR) != 0 ||
      !hash_regular_file(destination, expected_sha256, identity)) {
    g_remove(destination);
    g_rmdir(directory);
    return false;
  }
  *staged_path = destination;
  *staging_directory = directory;
  return true;
}

bool indicates_permission_problem(const std::string& message) {
  g_autofree gchar* lower = g_ascii_strdown(message.c_str(), -1);
  return g_strstr_len(lower, -1, "not authorized") != nullptr ||
         g_strstr_len(lower, -1, "authentication") != nullptr ||
         g_strstr_len(lower, -1, "permission denied") != nullptr;
}

struct InstallRequest {
  gint references = 1;
  std::string package_path;
  std::string staging_directory;
  std::string expected_sha256;
  FileIdentity identity;
  std::function<void(LinuxPackageInstallResult, std::string)> done;
  GDBusConnection* connection = nullptr;
  gchar* transaction_path = nullptr;
  guint signal_subscription = 0;
  bool finished = false;
  bool install_reply_received = false;
  bool transaction_finished = false;
  bool permission_required = false;
  std::string failure;
  LinuxPackageInstallResult terminal_result = LinuxPackageInstallResult::kFailed;
  std::string terminal_message;
  GCancellable* cancellable = g_cancellable_new();
  guint terminal_timeout = 0;

  ~InstallRequest() {
    if (cancellable != nullptr) g_object_unref(cancellable);
  }
};

void retain(InstallRequest* request) { g_atomic_int_inc(&request->references); }

void release(InstallRequest* request) {
  if (!g_atomic_int_dec_and_test(&request->references)) return;
  delete request;
}

void complete(InstallRequest* request, LinuxPackageInstallResult result,
              std::string message) {
  if (request->finished) {
    return;
  }
  request->finished = true;
  if (request->terminal_timeout != 0) g_source_remove(request->terminal_timeout);
  if (request->signal_subscription != 0 && request->connection != nullptr) {
    g_dbus_connection_signal_unsubscribe(request->connection,
                                         request->signal_subscription);
  }
  if (request->connection != nullptr) {
    g_object_unref(request->connection);
    request->connection = nullptr;
  }
  g_free(request->transaction_path);
  request->transaction_path = nullptr;
  if (!request->package_path.empty()) {
    g_remove(request->package_path.c_str());
  }
  if (!request->staging_directory.empty()) {
    g_rmdir(request->staging_directory.c_str());
  }
  request->done(result, std::move(message));
  release(request);
}

gboolean terminal_timeout(gpointer data) {
  auto* request = static_cast<InstallRequest*>(data);
  request->terminal_timeout = 0;
  if (!request->finished) {
    g_cancellable_cancel(request->cancellable);
    complete(request, LinuxPackageInstallResult::kFailed,
             "PackageKit did not reach a terminal state.");
  }
  return G_SOURCE_REMOVE;
}

void complete_when_terminal(InstallRequest* request) {
  if (!request->transaction_finished || !request->install_reply_received) {
    return;
  }
  complete(request, request->terminal_result,
           std::move(request->terminal_message));
}

void on_transaction_signal(GDBusConnection*, const gchar*, const gchar*,
                           const gchar*, const gchar* signal_name,
                           GVariant* parameters, gpointer user_data) {
  auto* request = static_cast<InstallRequest*>(user_data);
  if (request->finished) {
    return;
  }
  if (g_strcmp0(signal_name, "ErrorCode") == 0) {
    guint code = 0;
    const gchar* details = "PackageKit could not install the update.";
    g_variant_get(parameters, "(u&s)", &code, &details);
    (void)code;
    request->failure = details;
    request->permission_required = indicates_permission_problem(details);
    return;
  }
  if (g_strcmp0(signal_name, "Finished") != 0) {
    return;
  }
  guint exit_code = 0;
  guint runtime = 0;
  g_variant_get(parameters, "(uu)", &exit_code, &runtime);
  (void)runtime;
  if (exit_code == kPackageKitExitSuccess && request->failure.empty()) {
    request->terminal_result = LinuxPackageInstallResult::kRestartRequired;
    request->terminal_message = "The package update completed.";
  } else if (request->permission_required) {
    request->terminal_result = LinuxPackageInstallResult::kPermissionRequired;
    request->terminal_message = request->failure.empty()
                                    ? "PackageKit requires authentication."
                                    : request->failure;
  } else {
    request->terminal_result = LinuxPackageInstallResult::kFailed;
    request->terminal_message = request->failure.empty()
                                    ? "PackageKit did not complete the update."
                                    : request->failure;
  }
  request->transaction_finished = true;
  complete_when_terminal(request);
}

void on_install_files_ready(GObject* source, GAsyncResult* result,
                            gpointer user_data) {
  auto* request = static_cast<InstallRequest*>(user_data);
  struct Guard { InstallRequest* request; ~Guard() { release(request); } } guard{request};
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) response = g_dbus_connection_call_finish(
      G_DBUS_CONNECTION(source), result, &error);
  if (request->finished) return;
  if (response != nullptr) {
    request->install_reply_received = true;
    complete_when_terminal(request);
    return;
  }
  const std::string message = error == nullptr
                                  ? "PackageKit could not start the update."
                                  : error->message;
  complete(request,
           indicates_permission_problem(message)
               ? LinuxPackageInstallResult::kPermissionRequired
               : LinuxPackageInstallResult::kFailed,
           message);
}

void on_create_transaction_ready(GObject* source, GAsyncResult* result,
                                 gpointer user_data) {
  auto* request = static_cast<InstallRequest*>(user_data);
  struct Guard { InstallRequest* request; ~Guard() { release(request); } } guard{request};
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) response = g_dbus_connection_call_finish(
      G_DBUS_CONNECTION(source), result, &error);
  if (request->finished) return;
  if (response == nullptr) {
    const std::string message = error == nullptr
                                    ? "PackageKit could not create an update transaction."
                                    : error->message;
    complete(request, LinuxPackageInstallResult::kFailed, message);
    return;
  }
  const gchar* transaction_path = nullptr;
  g_variant_get(response, "(&o)", &transaction_path);
  if (transaction_path == nullptr || *transaction_path == '\0') {
    complete(request, LinuxPackageInstallResult::kFailed,
             "PackageKit returned an invalid update transaction.");
    return;
  }
  request->transaction_path = g_strdup(transaction_path);
  retain(request);
  request->signal_subscription = g_dbus_connection_signal_subscribe(
      request->connection, kPackageKitService, kTransactionInterface, nullptr,
      request->transaction_path, nullptr, G_DBUS_SIGNAL_FLAGS_NONE,
      on_transaction_signal, request, [](gpointer data) {
        release(static_cast<InstallRequest*>(data));
      });

  struct stat current {};
  if (!safe_regular_file(request->package_path.c_str(), &current) ||
      !same_identity(current, request->identity) ||
      !hash_regular_file(request->package_path, request->expected_sha256,
                         &request->identity)) {
    complete(request, LinuxPackageInstallResult::kFailed,
             "The update package changed before PackageKit received it.");
    return;
  }
  GVariantBuilder files;
  g_variant_builder_init(&files, G_VARIANT_TYPE("as"));
  g_variant_builder_add(&files, "s", request->package_path.c_str());
  g_dbus_connection_call(request->connection, kPackageKitService,
                         request->transaction_path, kTransactionInterface,
                         "InstallFiles", g_variant_new("(t@as)",
                                                       static_cast<guint64>(0),
                                                       g_variant_builder_end(&files)),
                         G_VARIANT_TYPE_UNIT,
                         G_DBUS_CALL_FLAGS_ALLOW_INTERACTIVE_AUTHORIZATION, -1,
                         request->cancellable, on_install_files_ready, (retain(request), request));
}

void on_system_bus_ready(GObject* source, GAsyncResult* result,
                         gpointer user_data) {
  auto* request = static_cast<InstallRequest*>(user_data);
  struct Guard { InstallRequest* request; ~Guard() { release(request); } } guard{request};
  g_autoptr(GError) error = nullptr;
  request->connection = g_bus_get_finish(result, &error);
  if (request->finished) {
    if (request->connection != nullptr) {
      g_object_unref(request->connection);
      request->connection = nullptr;
    }
    return;
  }
  if (request->connection == nullptr) {
    complete(request, LinuxPackageInstallResult::kFailed,
             error == nullptr ? "The PackageKit system service is unavailable."
                              : error->message);
    return;
  }
  g_dbus_connection_call(request->connection, kPackageKitService,
                         kPackageKitPath, kPackageKitInterface,
                         "CreateTransaction", nullptr, G_VARIANT_TYPE("(o)"),
                         G_DBUS_CALL_FLAGS_NONE, -1, request->cancellable,
                         on_create_transaction_ready, (retain(request), request));
  (void)source;
}

}  // namespace

LinuxPackageInstallation LinuxPackageKit::Detect() const {
  LinuxPackageInstallation installation;
  const char* architecture = compiled_architecture();
  if (architecture == nullptr) {
    installation.reason = "This Linux architecture cannot update packages.";
    return installation;
  }
  if (!root_owned_immutable(kMetadataPath) || !executable_has_system_identity()) {
    installation.reason = "This installation has no trusted package provenance.";
    return installation;
  }
  gchar* contents = nullptr;
  gsize length = 0;
  g_autoptr(GError) error = nullptr;
  if (!g_file_get_contents(kMetadataPath, &contents, &length, &error) ||
      length > kMaximumMetadataBytes) {
    g_free(contents);
    installation.reason = "This installation has invalid package provenance.";
    return installation;
  }
  installation.kind = package_kind_from_metadata(contents);
  if (installation.kind == LinuxPackageKind::kUnavailable ||
      !metadata_matches_architecture(contents, architecture)) {
    g_free(contents);
    installation.kind = LinuxPackageKind::kUnavailable;
    installation.reason = "This installation has invalid package provenance.";
    return installation;
  }
  g_free(contents);
  installation.architecture = architecture;
  if (!packagekit_available(&installation.reason)) {
    installation.kind = LinuxPackageKind::kUnavailable;
  }
  return installation;
}

void LinuxPackageKit::Install(
    const std::string& package_path, const std::string& expected_sha256,
    std::function<void(LinuxPackageInstallResult, std::string)> done) {
  LinuxPackageInstallation installation = Detect();
  if (!installation.available()) {
    done(LinuxPackageInstallResult::kFailed, installation.reason);
    return;
  }
  if (!matches_expected_package_name(package_path, installation)) {
    done(LinuxPackageInstallResult::kFailed,
         "The update package does not match this installation.");
    return;
  }
  auto request = std::make_unique<InstallRequest>();
  request->expected_sha256 = expected_sha256;
  request->done = std::move(done);
  if (!stage_verified_package(package_path, request->expected_sha256,
                              &request->package_path,
                              &request->staging_directory,
                              &request->identity)) {
    request->done(LinuxPackageInstallResult::kFailed,
                  "The update package failed its integrity check.");
    return;
  }
  InstallRequest* raw = request.release();
  raw->terminal_timeout = g_timeout_add_seconds(90, terminal_timeout, raw);
  g_bus_get(G_BUS_TYPE_SYSTEM, raw->cancellable, on_system_bus_ready,
            (retain(raw), raw));
}
