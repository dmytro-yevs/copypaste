#ifndef RUNNER_MEMORY_RELIEF_CHANNEL_H_
#define RUNNER_MEMORY_RELIEF_CHANNEL_H_

#include <flutter/binary_messenger.h>
#include <flutter/method_channel.h>
#include <memory>

// Returns heap memory the process has already freed to the OS, off the
// window thread. Concurrent requests coalesce into one pass.
class MemoryReliefChannel {
 public:
  explicit MemoryReliefChannel(flutter::BinaryMessenger* messenger);
  ~MemoryReliefChannel();

 private:
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif  // RUNNER_MEMORY_RELIEF_CHANNEL_H_
