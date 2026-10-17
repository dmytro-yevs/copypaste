#ifndef RUNNER_STARTUP_CHANNEL_H_
#define RUNNER_STARTUP_CHANNEL_H_

#include <flutter/binary_messenger.h>
#include <flutter/method_channel.h>
#include <memory>

class StartupChannel {
 public:
  explicit StartupChannel(flutter::BinaryMessenger* messenger);
  ~StartupChannel();

 private:
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif
