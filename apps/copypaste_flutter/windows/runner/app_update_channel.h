#ifndef RUNNER_APP_UPDATE_CHANNEL_H_
#define RUNNER_APP_UPDATE_CHANNEL_H_

#include <flutter/binary_messenger.h>
#include <flutter/encodable_value.h>
#include <flutter/method_channel.h>
#include <windows.h>

#include <memory>

class AppUpdateChannel {
 public:
  AppUpdateChannel(flutter::BinaryMessenger* messenger, HWND window);
  ~AppUpdateChannel();

 private:
  HWND window_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif  // RUNNER_APP_UPDATE_CHANNEL_H_
