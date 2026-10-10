#ifndef FLUTTER_LINUX_PORTAL_H_
#define FLUTTER_LINUX_PORTAL_H_

#include <functional>
#include <memory>
#include <string>
#include <vector>

typedef struct _GtkWindow GtkWindow;

// A shortcut definition sent by Flutter. `usage` is a USB HID usage name and
// `modifiers` contains the logical modifiers requested by the user. The portal
// may choose a different trigger; callers must render ShortcutResult instead of
// assuming preferred_trigger was accepted.
struct ShortcutRequest {
  std::string id;
  std::string description;
  std::string usage;
  std::vector<std::string> modifiers;
  std::string preferred_trigger;
};

struct ShortcutResult {
  bool registered = false;
  std::string trigger_description;
  std::string reason;
};

struct LinuxPortalCallbacks {
  // The activation token belongs to the compositor. The runner must preserve
  // it while handing the activation to its companion; this helper never opens
  // a window itself.
  std::function<void(const std::string& shortcut_id,
                     const std::string& activation_token)>
      shortcut_activated;
  std::function<void()> session_closed;
};

// GIO implementation of the XDG GlobalShortcuts and RemoteDesktop portals.
// It owns portal sessions but not the GtkWindow passed to it.
class LinuxPortal {
 public:
  explicit LinuxPortal(GtkWindow* parent, LinuxPortalCallbacks callbacks = {});
  ~LinuxPortal();

  LinuxPortal(const LinuxPortal&) = delete;
  LinuxPortal& operator=(const LinuxPortal&) = delete;

  // True only when GlobalShortcuts is exported on the session bus. This does
  // not report a shortcut as registered and does not request user consent.
  bool is_available() const;

  // True after the portal has granted a keyboard RemoteDesktop session and a
  // GlobalShortcuts binding. The host must also require a live compositor
  // companion before advertising Quick Paste itself as available.
  bool quick_paste_ready() const;

  bool remote_desktop_active() const;
  bool remote_desktop_available() const;
  bool shortcut_registered() const;

  // Starts a keyboard-only RemoteDesktop request. The caller must invoke this
  // from an explicit Settings action, because Start may present consent UI.
  void request_remote_desktop(std::function<void(bool)> done);

  // Registers one portal shortcut. A successful result contains the portal's
  // actual user-facing trigger description, not the preferred trigger.
  void register_shortcut(const ShortcutRequest& request,
                         std::function<void(ShortcutResult)> done);
  void unregister_shortcut(const std::string& id,
                           std::function<void(bool)> done);

  // Sends Ctrl+V through an active keyboard RemoteDesktop grant. Returns false
  // until the grant is active; it never falls back to an unprivileged injector.
  bool send_ctrl_v();

 private:
  struct Impl;
  std::shared_ptr<Impl> impl_;
};

#endif  // FLUTTER_LINUX_PORTAL_H_
