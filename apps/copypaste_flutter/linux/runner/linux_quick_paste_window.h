#ifndef COPYPASTE_LINUX_QUICK_PASTE_WINDOW_H_
#define COPYPASTE_LINUX_QUICK_PASTE_WINDOW_H_

#include <gtk/gtk.h>

struct LinuxQuickPasteFrame {
  int x;
  int y;
  int width;
  int height;
};

// GTK uses logical monitor coordinates, matching the shared Flutter layout.
LinuxQuickPasteFrame linux_quick_paste_frame(
    const LinuxQuickPasteFrame& current,
    const LinuxQuickPasteFrame& work_area, bool inspector_visible);
bool resize_linux_quick_paste_window(GtkWindow* window, bool inspector_visible);
void configure_linux_quick_paste_window(GtkWindow* window);

#endif  // COPYPASTE_LINUX_QUICK_PASTE_WINDOW_H_
