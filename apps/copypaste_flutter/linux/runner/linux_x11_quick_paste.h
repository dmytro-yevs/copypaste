#ifndef FLUTTER_LINUX_X11_QUICK_PASTE_H_
#define FLUTTER_LINUX_X11_QUICK_PASTE_H_

#include "linux_portal.h"

#include <cstdint>
#include <functional>
#include <memory>
#include <string>

// Owns the X11-only parts of Quick Paste. Wayland must use the desktop portal
// and a compositor companion; X11 can safely use its native focus and XTEST
// APIs without either of those services.
class LinuxX11QuickPaste {
 public:
  LinuxX11QuickPaste();
  ~LinuxX11QuickPaste();

  LinuxX11QuickPaste(const LinuxX11QuickPaste&) = delete;
  LinuxX11QuickPaste& operator=(const LinuxX11QuickPaste&) = delete;

  bool available() const;
  bool input_available() const;

  // Returns and clears the timestamp from the matching raw X11 shortcut.
  // Zero means no usable user-activation time is pending.
  std::uint32_t take_activation_time();

  void register_shortcut(const ShortcutRequest& request,
                         std::function<void(ShortcutResult)> done,
                         std::function<void()> activated);
  void unregister_shortcut(const std::string& id,
                           std::function<void(bool)> done);

  // Returns a decimal XID for the window that was focused before Quick Paste
  // opened. It is consumed by the child context, never persisted.
  std::string focused_window() const;
  bool restore_focus(const std::string& window) const;
  bool restore_and_paste(const std::string& window) const;

 private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

#endif  // FLUTTER_LINUX_X11_QUICK_PASTE_H_
