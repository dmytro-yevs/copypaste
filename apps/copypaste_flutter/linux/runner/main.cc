#include "my_application.h"
#include "linux_portal.h"

#include <cstdio>
#include <cstring>
#include <csignal>
#include <glib-unix.h>

namespace {
gboolean quit_requested(gpointer application) {
  g_application_quit(G_APPLICATION(application));
  return G_SOURCE_CONTINUE;
}
}  // namespace

int main(int argc, char** argv) {
  if (argc == 2 && std::strcmp(argv[1], "--version") == 0) {
    std::puts(COPYPASTE_VERSION);
    return 0;
  }
  bool quick_paste = false;
  for (int index = 1; index < argc; ++index) {
    if (std::strcmp(argv[index], "--copypaste-quick-paste") == 0) {
      quick_paste = true;
      break;
    }
  }
  g_autoptr(GDBusConnection) portal_identity = LinuxPortal::register_application();
  g_autoptr(MyApplication) app = my_application_new(quick_paste);
  const guint terminate_source = g_unix_signal_add(SIGTERM, quit_requested, app);
  const guint interrupt_source = g_unix_signal_add(SIGINT, quit_requested, app);
  const int result = g_application_run(G_APPLICATION(app), argc, argv);
  g_source_remove(terminate_source);
  g_source_remove(interrupt_source);
  return result;
}
