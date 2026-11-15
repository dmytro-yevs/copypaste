#include "memory_pressure_monitor.h"

MemoryPressureMonitor::MemoryPressureMonitor(HWND window, UINT message)
    : window_(window), message_(message) {
  low_ = ::CreateMemoryResourceNotification(LowMemoryResourceNotification);
  recovered_ =
      ::CreateMemoryResourceNotification(HighMemoryResourceNotification);
  Wait(true);
}

MemoryPressureMonitor::~MemoryPressureMonitor() {
  Cancel();
  if (low_) ::CloseHandle(low_);
  if (recovered_) ::CloseHandle(recovered_);
}

bool MemoryPressureMonitor::HandleSignal(WPARAM low) {
  Wait(low == 0);
  return low != 0;
}

void CALLBACK MemoryPressureMonitor::OnLow(void* context, BOOLEAN) {
  const auto* monitor = static_cast<MemoryPressureMonitor*>(context);
  ::PostMessage(monitor->window_, monitor->message_, 1, 0);
}

void CALLBACK MemoryPressureMonitor::OnRecovered(void* context, BOOLEAN) {
  const auto* monitor = static_cast<MemoryPressureMonitor*>(context);
  ::PostMessage(monitor->window_, monitor->message_, 0, 0);
}

void MemoryPressureMonitor::Wait(bool low) {
  Cancel();
  const HANDLE notification = low ? low_ : recovered_;
  if (!notification) return;
  if (!::RegisterWaitForSingleObject(&wait_, notification,
                                     low ? &OnLow : &OnRecovered, this,
                                     INFINITE, WT_EXECUTEONLYONCE)) {
    wait_ = nullptr;
  }
}

void MemoryPressureMonitor::Cancel() {
  if (!wait_) return;
  // Blocks until a running callback returns, so it never outlives |this|.
  ::UnregisterWaitEx(wait_, INVALID_HANDLE_VALUE);
  wait_ = nullptr;
}
