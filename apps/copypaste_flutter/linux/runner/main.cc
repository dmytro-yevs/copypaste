#include "my_application.h"

#include <cstdio>
#include <cstring>

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
  g_autoptr(MyApplication) app = my_application_new(quick_paste);
  return g_application_run(G_APPLICATION(app), argc, argv);
}
