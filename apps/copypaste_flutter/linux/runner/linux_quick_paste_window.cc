#include "linux_quick_paste_window.h"

#include <algorithm>

namespace {
// Keep these native dimensions aligned with AppLayoutSize and the other
// desktop presentation hosts: 448 menu + 8 gap + 360 inspector.
constexpr int kMenuWidth = 448;
constexpr int kExpandedWidth = 816;
constexpr int kHeight = 800;
}  // namespace

LinuxQuickPasteFrame linux_quick_paste_frame(
    const LinuxQuickPasteFrame& current,
    const LinuxQuickPasteFrame& work_area, bool inspector_visible) {
  const int width = std::min(inspector_visible ? kExpandedWidth : kMenuWidth,
                             std::max(1, work_area.width));
  const int height = std::min(kHeight, std::max(1, work_area.height));
  return {std::clamp(current.x, work_area.x,
                     work_area.x + std::max(0, work_area.width - width)),
          std::clamp(current.y, work_area.y,
                     work_area.y + std::max(0, work_area.height - height)),
          width, height};
}

bool resize_linux_quick_paste_window(GtkWindow* window, bool inspector_visible) {
  if (window == nullptr) return false;
  GtkWidget* widget = GTK_WIDGET(window);
  GdkDisplay* display = gtk_widget_get_display(widget);
  if (display == nullptr) return false;
  GdkWindow* surface = gtk_widget_get_window(widget);
  GdkMonitor* monitor = surface == nullptr
      ? gdk_display_get_primary_monitor(display)
      : gdk_display_get_monitor_at_window(display, surface);
  if (monitor == nullptr && gdk_display_get_n_monitors(display) > 0)
    monitor = gdk_display_get_monitor(display, 0);
  if (monitor == nullptr) return false;
  GdkRectangle work{};
  gdk_monitor_get_workarea(monitor, &work);
  if (work.width <= 0 || work.height <= 0) return false;
  int x = work.x;
  int y = work.y;
  if (surface != nullptr) gtk_window_get_position(window, &x, &y);
  const auto frame = linux_quick_paste_frame(
      {x, y, kMenuWidth, kHeight}, {work.x, work.y, work.width, work.height},
      inspector_visible);
  gtk_window_set_default_size(window, frame.width, frame.height);
  gtk_window_resize(window, frame.width, frame.height);
  // Wayland positions top-levels through the compositor. GTK keeps the resize
  // contract there and applies the clamped position where the backend allows it.
  gtk_window_move(window, frame.x, frame.y);
  return true;
}

void configure_linux_quick_paste_window(GtkWindow* window) {
  gtk_window_set_default_size(window, kMenuWidth, kHeight);
  gtk_window_set_role(window, "copypaste-quick-paste");
  gtk_window_set_decorated(window, FALSE);
  gtk_window_set_resizable(window, FALSE);
  gtk_window_set_skip_taskbar_hint(window, TRUE);
  gtk_window_set_keep_above(window, TRUE);
  gtk_window_set_type_hint(window, GDK_WINDOW_TYPE_HINT_DIALOG);
  GtkWidget* widget = GTK_WIDGET(window);
  GdkVisual* visual = gdk_screen_get_rgba_visual(gtk_widget_get_screen(widget));
  if (visual != nullptr) gtk_widget_set_visual(widget, visual);
  gtk_widget_set_app_paintable(widget, TRUE);
  resize_linux_quick_paste_window(window, false);
}
