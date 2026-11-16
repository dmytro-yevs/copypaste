#include "memory_relief_channel.h"

#include <windows.h>

#include <atomic>
#include <thread>

#include <flutter/standard_method_codec.h>

namespace {

std::atomic_bool g_relief_running{false};

void ReleaseFreedHeapMemory() {
  // Decommits free pages cached by every low-fragmentation heap in the process.
  HEAP_OPTIMIZE_RESOURCES_INFORMATION information{
      HEAP_OPTIMIZE_RESOURCES_CURRENT_VERSION, 0};
  ::HeapSetInformation(nullptr, HeapOptimizeResources, &information,
                       sizeof(information));
  ::HeapCompact(::GetProcessHeap(), 0);
}

}  // namespace

MemoryReliefChannel::MemoryReliefChannel(flutter::BinaryMessenger* messenger)
    : channel_(std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          messenger, "com.copypaste.app/memory",
          &flutter::StandardMethodCodec::GetInstance())) {
  channel_->SetMethodCallHandler([](const auto& call, auto result) {
    if (call.method_name() != "releaseFreedMemory") {
      result->NotImplemented();
      return;
    }
    if (!g_relief_running.exchange(true)) {
      std::thread([] {
        ReleaseFreedHeapMemory();
        g_relief_running = false;
      }).detach();
    }
    result->Success();
  });
}

MemoryReliefChannel::~MemoryReliefChannel() {
  channel_->SetMethodCallHandler(nullptr);
}
