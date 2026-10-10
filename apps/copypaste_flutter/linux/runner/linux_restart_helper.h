#ifndef FLUTTER_LINUX_RESTART_HELPER_H_
#define FLUTTER_LINUX_RESTART_HELPER_H_

#include <glib.h>

// Starts a small forked waiter which does not exec the replacement process
// until this process has exited. The replacement is the outer AppImage when
// applicable, the current executable otherwise, or the packaged launcher
// after an upgraded executable has been deleted.
bool linux_restart_helper_schedule_current(GError** error);

#endif  // FLUTTER_LINUX_RESTART_HELPER_H_
