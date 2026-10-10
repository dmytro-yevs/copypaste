#include "linux_release_page.h"

bool is_linux_release_page_url(const gchar* url) {
  if (url == nullptr) return false;
  g_autoptr(GError) error = nullptr;
  g_autoptr(GUri) uri = g_uri_parse(url, G_URI_FLAGS_NONE, &error);
  if (uri == nullptr || g_strcmp0(g_uri_get_scheme(uri), "https") != 0 ||
      g_strcmp0(g_uri_get_host(uri), "github.com") != 0 ||
      g_uri_get_userinfo(uri) != nullptr ||
      (g_uri_get_port(uri) != -1 && g_uri_get_port(uri) != 443)) return false;
  const gchar* path = g_uri_get_path(uri);
  if (path == nullptr ||
      !g_str_has_prefix(path, "/dmytro-yevs/copypaste/releases/")) return false;
  g_auto(GStrv) parts = g_strsplit(path, "/", -1);
  for (gchar** part = parts; *part != nullptr; ++part) {
    if (g_strcmp0(*part, ".") == 0 || g_strcmp0(*part, "..") == 0)
      return false;
  }
  return true;
}
