// Compiles and runs under scripts/test-linux-x11-quick-paste.sh.
// It requires a private Xvfb server because global X11 grabs and XTEST input
// cannot be validated against a mock display.

#include "linux_x11_quick_paste.h"

#include <gdk/gdkx.h>
#include <glib.h>
#include <gtk/gtk.h>

#include <X11/Xatom.h>
#include <X11/keysym.h>
#include <X11/extensions/XTest.h>
#include <linux/input-event-codes.h>

#include <functional>
#include <string>
#include <utility>

namespace {

constexpr guint kHidKeyboardPage = 0x70000;
constexpr char kClipboardText[] = "X11 Quick Paste fixture";

void drain_main_context() {
  while (g_main_context_pending(nullptr)) {
    g_main_context_iteration(nullptr, false);
  }
}

bool wait_until(const std::function<bool()>& condition) {
  const gint64 deadline = g_get_monotonic_time() + G_TIME_SPAN_SECOND;
  while (g_get_monotonic_time() < deadline) {
    drain_main_context();
    if (condition()) return true;
    g_usleep(1000);
  }
  drain_main_context();
  return condition();
}

void send_shortcut(Display* display, KeyCode keycode) {
  const KeyCode control = XKeysymToKeycode(display, XK_Control_L);
  const KeyCode shift = XKeysymToKeycode(display, XK_Shift_L);
  g_assert_cmpuint(control, !=, 0);
  g_assert_cmpuint(shift, !=, 0);
  g_assert_cmpuint(keycode, !=, 0);
  g_assert_true(XTestFakeKeyEvent(display, control, True, CurrentTime));
  g_assert_true(XTestFakeKeyEvent(display, shift, True, CurrentTime));
  g_assert_true(XTestFakeKeyEvent(display, keycode, True, CurrentTime));
  g_assert_true(XTestFakeKeyEvent(display, keycode, False, CurrentTime));
  g_assert_true(XTestFakeKeyEvent(display, shift, False, CurrentTime));
  g_assert_true(XTestFakeKeyEvent(display, control, False, CurrentTime));
  XSync(display, False);
}

void register_and_activate(LinuxX11QuickPaste* quick_paste, Display* display,
                           guint usage, int evdev_key) {
  bool completed = false;
  ShortcutResult result;
  int activations = 0;
  const ShortcutRequest request{
      "x11-quick-paste-test", "X11 Quick Paste test",
      std::to_string(kHidKeyboardPage | usage), {"control", "shift"},
      "Ctrl+Shift+test"};
  quick_paste->register_shortcut(
      request,
      [&completed, &result](ShortcutResult registration) {
        completed = true;
        result = std::move(registration);
      },
      [&activations] { ++activations; });

  g_assert_true(completed);
  g_assert_true(result.registered);
  // Xorg's evdev keycodes include its eight reserved protocol keycodes.
  // Inject the physical key independently of the server's logical keysyms.
  send_shortcut(display, static_cast<KeyCode>(evdev_key + 8));
  g_assert_true(wait_until([&activations] { return activations == 1; }));
  g_assert_cmpint(activations, ==, 1);
  bool removed = false;
  quick_paste->unregister_shortcut(request.id, [&removed](bool result) {
    removed = result;
  });
  g_assert_true(removed);
}

void set_active_window(Display* display, Window root, Window window) {
  const Atom active = XInternAtom(display, "_NET_ACTIVE_WINDOW", False);
  g_assert_cmpuint(active, !=, None);
  XChangeProperty(display, root, active, XA_WINDOW, 32, PropModeReplace,
                  reinterpret_cast<const unsigned char*>(&window), 1);
  XSync(display, False);
}

Window xid_for(GtkWidget* widget) {
  GdkWindow* window = gtk_widget_get_window(widget);
  g_assert_nonnull(window);
  return gdk_x11_window_get_xid(window);
}

Window focused_xid(Display* display) {
  Window focused = 0;
  int revert = 0;
  XGetInputFocus(display, &focused, &revert);
  return focused;
}

void test_x11_quick_paste() {
  int argc = 0;
  char** argv = nullptr;
  g_assert_true(gtk_init_check(&argc, &argv));

  GdkDisplay* gdk_display = gdk_display_get_default();
  g_assert_true(GDK_IS_X11_DISPLAY(gdk_display));
  Display* display = GDK_DISPLAY_XDISPLAY(gdk_display);
  const Window root = DefaultRootWindow(display);

  GtkWidget* target_window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  GtkWidget* entry = gtk_entry_new();
  gtk_container_add(GTK_CONTAINER(target_window), entry);
  gtk_widget_show_all(target_window);
  g_assert_true(wait_until([target_window] {
    return gtk_widget_get_window(target_window) != nullptr;
  }));
  gtk_widget_grab_focus(entry);

  GtkWidget* other_window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_widget_show_all(other_window);
  g_assert_true(wait_until([other_window] {
    return gtk_widget_get_window(other_window) != nullptr;
  }));

  const Window target_xid = xid_for(target_window);
  set_active_window(display, root, target_xid);
  XSetInputFocus(display, target_xid, RevertToPointerRoot, CurrentTime);
  XSync(display, False);
  g_assert_cmpuint(focused_xid(display), ==, target_xid);

  LinuxX11QuickPaste quick_paste;
  g_assert_true(quick_paste.available());
  g_assert_true(quick_paste.input_available());
  register_and_activate(&quick_paste, display, 0x06, KEY_C);
  register_and_activate(&quick_paste, display, 0x3a, KEY_F1);
  register_and_activate(&quick_paste, display, 0x73, KEY_F24);

  const std::string target_id = std::to_string(target_xid);
  const std::string focused_id = quick_paste.focused_window();
  g_assert_cmpstr(focused_id.c_str(), ==, target_id.c_str());

  const Window other_xid = xid_for(other_window);
  XSetInputFocus(display, other_xid, RevertToPointerRoot, CurrentTime);
  XSync(display, False);
  g_assert_cmpuint(focused_xid(display), ==, other_xid);

  GtkClipboard* clipboard = gtk_clipboard_get(GDK_SELECTION_CLIPBOARD);
  gtk_clipboard_set_text(clipboard, kClipboardText, -1);
  g_assert_true(quick_paste.restore_and_paste(target_id));
  g_assert_cmpuint(focused_xid(display), ==, target_xid);
  g_assert_true(wait_until([entry] {
    return std::string(gtk_entry_get_text(GTK_ENTRY(entry))) == kClipboardText;
  }));
  g_assert_cmpstr(gtk_entry_get_text(GTK_ENTRY(entry)), ==, kClipboardText);

  gtk_widget_destroy(other_window);
  gtk_widget_destroy(target_window);
  drain_main_context();
}

}  // namespace

int main() {
  test_x11_quick_paste();
  return 0;
}
