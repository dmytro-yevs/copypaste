#ifndef RUNNER_APPLICATION_INSTANCE_H_
#define RUNNER_APPLICATION_INSTANCE_H_

#include <windows.h>

#include <optional>
#include <string>

// Keeps launches in this Windows session attached to one main window.
class ApplicationInstance {
 public:
  ApplicationInstance();
  ~ApplicationInstance();
  ApplicationInstance(const ApplicationInstance&) = delete;
  ApplicationInstance& operator=(const ApplicationInstance&) = delete;

  bool valid() const { return mutex_ != nullptr; }
  bool primary() const { return primary_; }
  bool OpenExisting(const std::string& pairing_uri) const;
  static std::optional<std::string> ReadLaunch(const COPYDATASTRUCT* data);

 private:
  HANDLE mutex_ = nullptr;
  bool primary_ = false;
};

#endif  // RUNNER_APPLICATION_INSTANCE_H_
