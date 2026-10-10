#ifndef FLUTTER_LINUX_GNOME_SHORTCUTS_H_
#define FLUTTER_LINUX_GNOME_SHORTCUTS_H_

#include "linux_portal.h"

#include <functional>
#include <memory>
#include <string>

// Talks to the optional GNOME 46/47 companion. It is deliberately separate
// from the XDG portal because the companion is only a fallback when that
// portal does not export GlobalShortcuts.
class LinuxGnomeShortcuts {
 public:
  explicit LinuxGnomeShortcuts(
      std::function<void(const std::string& id)> activated = {});
  ~LinuxGnomeShortcuts();

  LinuxGnomeShortcuts(const LinuxGnomeShortcuts&) = delete;
  LinuxGnomeShortcuts& operator=(const LinuxGnomeShortcuts&) = delete;

  // True only after the current well-known-name owner has confirmed version 1.
  bool is_available() const;
  bool shortcut_registered() const;

  void register_shortcut(const ShortcutRequest& request,
                         std::function<void(ShortcutResult)> done);
  void unregister_shortcut(const std::string& id,
                           std::function<void(bool)> done);

 private:
  struct Impl;
  std::shared_ptr<Impl> impl_;
};

#endif  // FLUTTER_LINUX_GNOME_SHORTCUTS_H_
