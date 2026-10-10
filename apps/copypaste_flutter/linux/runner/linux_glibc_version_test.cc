#include "linux_glibc_version.h"

#include <gnu/libc-version.h>
#include <glib.h>

#include <string>

void test_current_glibc_version() {
  const auto version = linux_glibc_version();
  g_assert_true(version.has_value());
  g_assert_cmpuint(version->major, >, 0);
  const std::string normalized = version->ToString();
  g_assert_true(g_regex_match_simple("^[0-9]+\\.[0-9]+\\.[0-9]+$",
                                     normalized.c_str(), G_REGEX_RAW,
                                     G_REGEX_MATCH_NOTEMPTY));

  const gchar* reported = gnu_get_libc_version();
  const std::string prefix = std::to_string(version->major) + "." +
      std::to_string(version->minor);
  g_assert_true(g_str_has_prefix(reported, prefix.c_str()));
}

int main(int argc, char** argv) {
  g_test_init(&argc, &argv, nullptr);
  g_test_add_func("/linux/glibc_version/current", test_current_glibc_version);
  return g_test_run();
}
