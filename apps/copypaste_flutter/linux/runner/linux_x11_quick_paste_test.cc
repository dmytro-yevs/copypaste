// Compiles and runs under scripts/test-linux-x11-quick-paste.sh.
// It requires a private Xvfb server because global X11 grabs and XTEST input
// cannot be validated against a mock display.

#include "linux_x11_quick_paste.h"

#include <gdk/gdkx.h>
#include <glib.h>
#include <gtk/gtk.h>

#include <X11/Xatom.h>
#include <X11/XKBlib.h>
#include <X11/keysym.h>
#include <X11/extensions/XTest.h>
#include <linux/input-event-codes.h>

#include <array>
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
  guint32 activation_time = GDK_CURRENT_TIME;
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
      [&activations, &activation_time, quick_paste] {
        activation_time = quick_paste->take_activation_time();
        ++activations;
      });

  g_assert_true(completed);
  g_assert_true(result.registered);
  // Xorg's evdev keycodes include its eight reserved protocol keycodes.
  // Inject the physical key independently of the server's logical keysyms.
  send_shortcut(display, static_cast<KeyCode>(evdev_key + 8));
  g_assert_true(wait_until([&activations] { return activations == 1; }));
  g_assert_cmpint(activations, ==, 1);
  g_assert_cmpuint(activation_time, !=, GDK_CURRENT_TIME);
  g_assert_cmpuint(quick_paste->take_activation_time(), ==, GDK_CURRENT_TIME);
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

unsigned int modifier_mask_for(Display* display, KeySym keysym) {
  XModifierKeymap* map = XGetModifierMapping(display);
  g_assert_nonnull(map);
  unsigned int mask = 0;
  for (int modifier = 0; modifier < 8; ++modifier) {
    for (int index = 0; index < map->max_keypermod; ++index) {
      const KeyCode keycode = map->modifiermap[modifier * map->max_keypermod + index];
      if (keycode != 0 && XkbKeycodeToKeysym(display, keycode, 0, 0) == keysym) {
        mask |= 1U << modifier;
      }
    }
  }
  XFreeModifiermap(map);
  return mask;
}

unsigned int current_modifiers(Display* display) {
  Window root = 0;
  Window child = 0;
  int root_x = 0;
  int root_y = 0;
  int window_x = 0;
  int window_y = 0;
  unsigned int modifiers = 0;
  g_assert_true(XQueryPointer(display, DefaultRootWindow(display), &root, &child,
                              &root_x, &root_y, &window_x, &window_y,
                              &modifiers));
  return modifiers;
}

void set_lock(Display* display, KeySym keysym, unsigned int mask, bool enabled) {
  const bool current = (current_modifiers(display) & mask) != 0;
  if (current == enabled) return;
  const KeyCode keycode = XKeysymToKeycode(display, keysym);
  g_assert_cmpuint(keycode, !=, 0);
  g_assert_true(XTestFakeKeyEvent(display, keycode, True, CurrentTime));
  g_assert_true(XTestFakeKeyEvent(display, keycode, False, CurrentTime));
  XSync(display, False);
  g_assert_cmpint((current_modifiers(display) & mask) != 0, ==, enabled);
}

void drain_events(Display* display) {
  while (XPending(display) > 0) {
    XEvent event {};
    XNextEvent(display, &event);
  }
}

bool received_keypress(Display* display, KeyCode keycode) {
  bool received = false;
  while (XPending(display) > 0) {
    XEvent event {};
    XNextEvent(display, &event);
    received |= event.type == KeyPress && event.xkey.keycode == keycode;
  }
  return received;
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

void test_x11_shortcut_grabs_all_lock_subsets() {
  int argc = 0;
  char** argv = nullptr;
  g_assert_true(gtk_init_check(&argc, &argv));

  GdkDisplay* gdk_display = gdk_display_get_default();
  g_assert_true(GDK_IS_X11_DISPLAY(gdk_display));
  Display* display = GDK_DISPLAY_XDISPLAY(gdk_display);
  Display* foreign_display = XOpenDisplay(nullptr);
  g_assert_nonnull(foreign_display);
  const Window foreign_window = XCreateSimpleWindow(
      foreign_display, DefaultRootWindow(foreign_display), 0, 0, 1, 1, 0, 0, 0);
  XSelectInput(foreign_display, foreign_window, KeyPressMask);
  XMapRaised(foreign_display, foreign_window);
  XSetInputFocus(foreign_display, foreign_window, RevertToPointerRoot, CurrentTime);
  XSync(foreign_display, False);

  const unsigned int caps_mask = modifier_mask_for(display, XK_Caps_Lock);
  const unsigned int num_mask = modifier_mask_for(display, XK_Num_Lock);
  g_assert_cmpuint(caps_mask, !=, 0);
  g_assert_cmpuint(num_mask, !=, 0);
  const bool caps_initial = (current_modifiers(display) & caps_mask) != 0;
  const bool num_initial = (current_modifiers(display) & num_mask) != 0;

  LinuxX11QuickPaste quick_paste;
  bool completed = false;
  ShortcutResult result;
  int activations = 0;
  const ShortcutRequest request{
      "x11-lock-subsets", "X11 lock subsets",
      std::to_string(kHidKeyboardPage | 0x06), {"control", "shift"},
      "Ctrl+Shift+C"};
  quick_paste.register_shortcut(
      request,
      [&completed, &result](ShortcutResult registration) {
        completed = true;
        result = std::move(registration);
      },
      [&activations, &quick_paste] {
        g_assert_cmpuint(quick_paste.take_activation_time(), !=, GDK_CURRENT_TIME);
        ++activations;
      });
  g_assert_true(completed);
  g_assert_true(result.registered);

  const KeyCode shortcut_key = XKeysymToKeycode(display, XK_C);
  g_assert_cmpuint(shortcut_key, !=, 0);
  const std::array<std::pair<bool, bool>, 4> lock_states = {
      {{false, false}, {true, false}, {false, true}, {true, true}}};
  for (const auto& [caps, num] : lock_states) {
    set_lock(display, XK_Caps_Lock, caps_mask, caps);
    set_lock(display, XK_Num_Lock, num_mask, num);
    XSetInputFocus(foreign_display, foreign_window, RevertToPointerRoot, CurrentTime);
    XSync(foreign_display, False);
    drain_events(foreign_display);
    send_shortcut(display, shortcut_key);
    const int expected_activations = activations + 1;
    g_assert_true(wait_until([&activations, expected_activations] {
      return activations == expected_activations;
    }));
    g_assert_false(received_keypress(foreign_display, shortcut_key));
  }

  bool removed = false;
  quick_paste.unregister_shortcut(request.id, [&removed](bool registered) {
    removed = registered;
  });
  g_assert_true(removed);
  set_lock(display, XK_Caps_Lock, caps_mask, caps_initial);
  set_lock(display, XK_Num_Lock, num_mask, num_initial);
  XDestroyWindow(foreign_display, foreign_window);
  XCloseDisplay(foreign_display);
}

}  // namespace

int main() {
  test_x11_quick_paste();
  test_x11_shortcut_grabs_all_lock_subsets();
  return 0;
}
