#include "linux_x11_quick_paste.h"

#include <gdk/gdk.h>
#include <gdk/gdkx.h>

#include <X11/XKBlib.h>
#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/extensions/XTest.h>

#include <linux/input-event-codes.h>

#include <algorithm>
#include <cerrno>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <limits>
#include <map>
#include <utility>
#include <vector>

namespace {

constexpr guint kHidKeyboardPage = 0x70000;
constexpr guint kHidUsageMask = 0xffff;
int grab_error = 0;

int record_grab_error(Display*, XErrorEvent* event) {
  grab_error = event->error_code;
  return 0;
}

int evdev_keycode(guint usage) {
  static constexpr int kLetters[] = {30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44};
  static constexpr int kDigits[] = {2, 3, 4, 5, 6, 7, 8, 9, 10, 11};
  static constexpr int kFunctionKeys[] = {
      KEY_F1,  KEY_F2,  KEY_F3,  KEY_F4,  KEY_F5,  KEY_F6,
      KEY_F7,  KEY_F8,  KEY_F9,  KEY_F10, KEY_F11, KEY_F12,
      KEY_F13, KEY_F14, KEY_F15, KEY_F16, KEY_F17, KEY_F18,
      KEY_F19, KEY_F20, KEY_F21, KEY_F22, KEY_F23, KEY_F24,
  };
  if (usage >= 0x04 && usage <= 0x1d) return kLetters[usage - 0x04];
  if (usage >= 0x1e && usage <= 0x27) return kDigits[usage - 0x1e];
  if (usage >= 0x3a && usage <= 0x45) return kFunctionKeys[usage - 0x3a];
  if (usage >= 0x68 && usage <= 0x73) {
    return kFunctionKeys[12 + usage - 0x68];
  }
  switch (usage) { case 0x28: return 28; case 0x29: return 1; case 0x2a: return 14; case 0x2b: return 15; case 0x2c: return 57; case 0x2d: return 12; case 0x2e: return 13; case 0x2f: return 26; case 0x30: return 27; case 0x31: return 43; case 0x33: return 39; case 0x34: return 40; case 0x35: return 41; case 0x36: return 51; case 0x37: return 52; case 0x38: return 53; case 0x49: return 110; case 0x4a: return 102; case 0x4b: return 104; case 0x4c: return KEY_DELETE; case 0x4d: return KEY_END; case 0x4e: return KEY_PAGEDOWN; case 0x4f: return 106; case 0x50: return KEY_LEFT; case 0x51: return 108; case 0x52: return KEY_UP; default: return 0; }
}

unsigned int modifier_mask(const std::vector<std::string>& modifiers) {
  unsigned int mask = 0;
  for (const std::string& modifier : modifiers) {
    if (modifier == "control") {
      mask |= ControlMask;
    } else if (modifier == "shift") {
      mask |= ShiftMask;
    } else if (modifier == "alt") {
      mask |= Mod1Mask;
    } else if (modifier == "meta") {
      mask |= Mod4Mask;
    }
  }
  return mask;
}

unsigned int ignored_lock_mask(Display* display) {
  unsigned int mask = LockMask;
  XModifierKeymap* map = XGetModifierMapping(display);
  if (map == nullptr) return mask;
  for (int modifier = 0; modifier < 8; ++modifier) {
    for (int index = 0; index < map->max_keypermod; ++index) {
      const KeyCode keycode = map->modifiermap[modifier * map->max_keypermod + index];
      if (keycode != 0 && XkbKeycodeToKeysym(display, keycode, 0, 0) == XK_Num_Lock) {
        mask |= 1U << modifier;
      }
    }
  }
  XFreeModifiermap(map);
  return mask;
}

std::vector<unsigned int> lock_subsets(unsigned int mask) {
  std::vector<unsigned int> subsets = {0};
  for (unsigned int bit = 1; bit != 0; bit <<= 1) {
    if ((mask & bit) == 0) continue;
    const std::size_t count = subsets.size();
    for (std::size_t index = 0; index < count; ++index) {
      subsets.push_back(subsets[index] | bit);
    }
  }
  return subsets;
}

bool parse_xid(const std::string& value, Window* window) {
  if (value.empty() || window == nullptr) return false;
  errno = 0;
  char* end = nullptr;
  const unsigned long parsed = std::strtoul(value.c_str(), &end, 10);
  if (errno != 0 || end == value.c_str() || *end != '\0' || parsed == 0 ||
      parsed > std::numeric_limits<Window>::max()) {
    return false;
  }
  *window = static_cast<Window>(parsed);
  return true;
}

}  // namespace

struct LinuxX11QuickPaste::Impl {
  struct Binding {
    KeyCode keycode = 0;
    unsigned int modifiers = 0;
    std::vector<unsigned int> masks;
    std::function<void()> activated;
  };

  Display* display = nullptr;
  Window root = 0;
  int x11_event_base = 0;
  bool xtest_available = false;
  std::uint32_t activation_time = 0;
  std::map<std::string, Binding> bindings;

  Impl() {
    GdkDisplay* gdk_display = gdk_display_get_default();
    if (gdk_display == nullptr || !GDK_IS_X11_DISPLAY(gdk_display)) return;
    display = GDK_DISPLAY_XDISPLAY(gdk_display);
    root = DefaultRootWindow(display);
    int error_base = 0;
    int major = 0;
    int minor = 0;
    xtest_available = XTestQueryExtension(display, &x11_event_base, &error_base,
                                          &major, &minor) != 0;
    gdk_window_add_filter(nullptr, &Impl::filter, this);
  }

  ~Impl() {
    clear();
    gdk_window_remove_filter(nullptr, &Impl::filter, this);
  }

  static GdkFilterReturn filter(GdkXEvent* xevent, GdkEvent*, gpointer user_data) {
    auto* self = static_cast<Impl*>(user_data);
    if (self->display == nullptr || self->bindings.empty()) return GDK_FILTER_CONTINUE;
    auto* event = static_cast<XEvent*>(xevent);
    if (event->type != KeyPress) return GDK_FILTER_CONTINUE;
    const auto matches_key = [event](const auto& entry) {
      return entry.second.keycode == event->xkey.keycode;
    };
    if (std::none_of(self->bindings.begin(), self->bindings.end(), matches_key)) {
      return GDK_FILTER_CONTINUE;
    }
    const unsigned int state = event->xkey.state & ~ignored_lock_mask(self->display);
    const auto binding = std::find_if(
        self->bindings.begin(), self->bindings.end(), [event, state](const auto& entry) {
          return entry.second.keycode == event->xkey.keycode &&
                 entry.second.modifiers == state;
        });
    if (binding == self->bindings.end()) return GDK_FILTER_CONTINUE;
    self->activation_time = event->xkey.time;
    // The callback may replace or remove this binding.
    const std::function<void()> activated = binding->second.activated;
    if (self->activation_time != CurrentTime && activated != nullptr) activated();
    return GDK_FILTER_REMOVE;
  }

  void ungrab(const Binding& binding) {
    if (display == nullptr) return;
    for (unsigned int lock : binding.masks) {
      XUngrabKey(display, binding.keycode, binding.modifiers | lock, root);
    }
    if (!binding.masks.empty()) XSync(display, False);
  }

  void remove(const std::string& id) {
    const auto binding = bindings.find(id);
    if (binding == bindings.end()) return;
    ungrab(binding->second);
    bindings.erase(binding);
    if (bindings.empty()) activation_time = 0;
  }

  void clear() {
    for (const auto& entry : bindings) ungrab(entry.second);
    bindings.clear();
    activation_time = 0;
  }

  bool send_ctrl_v() const {
    const KeyCode control = XKeysymToKeycode(display, XK_Control_L);
    const KeyCode v = XKeysymToKeycode(display, XK_v);
    if (control == 0 || v == 0) return false;
    const bool control_down = XTestFakeKeyEvent(display, control, True, CurrentTime) != 0;
    const bool v_down = control_down &&
        XTestFakeKeyEvent(display, v, True, CurrentTime) != 0;
    const bool v_up = v_down && XTestFakeKeyEvent(display, v, False, CurrentTime) != 0;
    const bool control_up = control_down &&
        XTestFakeKeyEvent(display, control, False, CurrentTime) != 0;
    XSync(display, False);
    return control_down && v_down && v_up && control_up;
  }
};

LinuxX11QuickPaste::LinuxX11QuickPaste() : impl_(std::make_unique<Impl>()) {}

LinuxX11QuickPaste::~LinuxX11QuickPaste() = default;

bool LinuxX11QuickPaste::available() const { return impl_->display != nullptr; }

bool LinuxX11QuickPaste::input_available() const {
  return impl_->display != nullptr && impl_->xtest_available;
}

std::uint32_t LinuxX11QuickPaste::take_activation_time() {
  const std::uint32_t time = impl_->activation_time;
  impl_->activation_time = 0;
  return time;
}

void LinuxX11QuickPaste::register_shortcut(
    const ShortcutRequest& request, std::function<void(ShortcutResult)> done,
    std::function<void()> activated) {
  impl_->remove(request.id);
  if (!available() || request.id.empty() || request.description.empty() ||
      request.usage.empty() || activated == nullptr) {
    done({false, {}, "X11 shortcut registration is unavailable."});
    return;
  }
  char* end = nullptr;
  const unsigned long usage = std::strtoul(request.usage.c_str(), &end, 10);
  if (end == request.usage.c_str() || *end != '\0' ||
      (usage & ~kHidUsageMask) != kHidKeyboardPage ||
      (usage & kHidUsageMask) == 0 || (usage & kHidUsageMask) > 0xff) {
    done({false, {}, "The selected shortcut has no X11 HID mapping."});
    return;
  }
  const int evdev = evdev_keycode(static_cast<guint>(usage & kHidUsageMask));
  const unsigned long x11_keycode = evdev == 0 ? 0 : static_cast<unsigned long>(evdev + 8);
  Impl::Binding binding;
  binding.keycode = static_cast<KeyCode>(x11_keycode);
  binding.modifiers = modifier_mask(request.modifiers);
  if (binding.modifiers == 0 || binding.keycode == 0 || x11_keycode > 255) {
    done({false, {}, "The selected shortcut is invalid on X11."});
    return;
  }
  for (const auto& entry : impl_->bindings) {
    if (entry.second.keycode == binding.keycode &&
        entry.second.modifiers == binding.modifiers) {
      done({false, {}, "That X11 shortcut is already in use."});
      return;
    }
  }
  const std::vector<unsigned int> masks = lock_subsets(ignored_lock_mask(impl_->display));
  grab_error = 0;
  XErrorHandler previous = XSetErrorHandler(record_grab_error);
  for (unsigned int lock : masks) {
    XGrabKey(impl_->display, binding.keycode, binding.modifiers | lock, impl_->root, True,
             GrabModeAsync, GrabModeAsync);
    XSync(impl_->display, False);
    if (grab_error != 0) break;
    binding.masks.push_back(lock);
  }
  XSetErrorHandler(previous);
  if (grab_error != 0) {
    impl_->ungrab(binding);
    done({false, {}, "That X11 shortcut is already in use."});
    return;
  }
  binding.activated = std::move(activated);
  impl_->bindings[request.id] = std::move(binding);
  done({true, request.preferred_trigger, {}});
}

void LinuxX11QuickPaste::unregister_shortcut(
    const std::string& id, std::function<void(bool)> done) {
  impl_->remove(id);
  done(true);
}

bool LinuxX11QuickPaste::keys_released() const {
  if (!available()) return true;
  char keys[32] = {};
  XQueryKeymap(impl_->display, keys);
  const auto pressed = [&keys](KeyCode keycode) {
    return keycode != 0 && (keys[keycode / 8] & (1 << (keycode % 8))) != 0;
  };
  for (const auto& entry : impl_->bindings) {
    if (pressed(entry.second.keycode)) return false;
  }
  XModifierKeymap* map = XGetModifierMapping(impl_->display);
  if (map == nullptr) return true;
  bool released = true;
  for (int index = 0; index < 8 * map->max_keypermod && released; ++index) {
    released = !pressed(map->modifiermap[index]);
  }
  XFreeModifiermap(map);
  return released;
}

bool LinuxX11QuickPaste::paste_into_focused() const {
  return input_available() && !focused_window().empty() && impl_->send_ctrl_v();
}

bool LinuxX11QuickPaste::move_caret_left(int count) const {
  if (count <= 0) return count == 0;
  if (!input_available()) return false;
  const KeyCode left = XKeysymToKeycode(impl_->display, XK_Left);
  if (left == 0) return false;
  bool sent = true;
  for (int index = 0; index < count && sent; ++index) {
    sent = XTestFakeKeyEvent(impl_->display, left, True, CurrentTime) != 0 &&
        XTestFakeKeyEvent(impl_->display, left, False, CurrentTime) != 0;
  }
  XSync(impl_->display, False);
  return sent;
}

std::string LinuxX11QuickPaste::focused_window() const {
  if (!available()) return {};
  Atom active = XInternAtom(impl_->display, "_NET_ACTIVE_WINDOW", True);
  if (active == None) return {};
  Atom actual_type = None;
  int actual_format = 0;
  unsigned long count = 0;
  unsigned long remaining = 0;
  unsigned char* bytes = nullptr;
  const int status = XGetWindowProperty(impl_->display, impl_->root, active, 0,
                                        1, False, XA_WINDOW, &actual_type,
                                        &actual_format, &count, &remaining,
                                        &bytes);
  if (status != Success || actual_type != XA_WINDOW || actual_format != 32 ||
      count != 1 || bytes == nullptr) {
    if (bytes != nullptr) XFree(bytes);
    return {};
  }
  const Window window = *reinterpret_cast<Window*>(bytes);
  XFree(bytes);
  return window == 0 ? std::string() : std::to_string(window);
}

bool LinuxX11QuickPaste::restore_focus(const std::string& window) const {
  Window target = 0;
  if (!available() || !parse_xid(window, &target)) return false;
  XWindowAttributes attributes {};
  GdkDisplay* gdk_display = gdk_display_get_default();
  gdk_x11_display_error_trap_push(gdk_display);
  const bool viewable = XGetWindowAttributes(impl_->display, target, &attributes) != 0 &&
      attributes.map_state == IsViewable;
  const int attributes_error = gdk_x11_display_error_trap_pop(gdk_display);
  if (!viewable || attributes_error != 0) return false;
  gdk_x11_display_error_trap_push(gdk_display);
  XRaiseWindow(impl_->display, target);
  XSetInputFocus(impl_->display, target, RevertToPointerRoot, CurrentTime);
  XSync(impl_->display, False);
  Window focused = 0; int revert = 0;
  XGetInputFocus(impl_->display, &focused, &revert);
  return gdk_x11_display_error_trap_pop(gdk_display) == 0 && focused == target;
}

std::string LinuxX11QuickPaste::window_class(const std::string& window) const {
  Window target = 0;
  if (!available() || !parse_xid(window, &target)) return {};
  GdkDisplay* gdk_display = gdk_display_get_default();
  gdk_x11_display_error_trap_push(gdk_display);
  XClassHint hint {};
  const bool found = XGetClassHint(impl_->display, target, &hint) != 0;
  std::string value = found && hint.res_class != nullptr ? hint.res_class : "";
  if (hint.res_name != nullptr) XFree(hint.res_name);
  if (hint.res_class != nullptr) XFree(hint.res_class);
  return gdk_x11_display_error_trap_pop(gdk_display) == 0 ? value : std::string();
}

bool LinuxX11QuickPaste::restore_and_paste(const std::string& window) const {
  return input_available() && restore_focus(window) && impl_->send_ctrl_v();
}
