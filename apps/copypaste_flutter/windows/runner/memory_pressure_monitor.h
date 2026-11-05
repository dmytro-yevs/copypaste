#ifndef RUNNER_MEMORY_PRESSURE_MONITOR_H_
#define RUNNER_MEMORY_PRESSURE_MONITOR_H_

#include <windows.h>

// Posts |message| to |window| when available physical memory becomes low and
// again when it recovers. Waits alternate between the two OS notifications, so
// a sustained low-memory state is reported once and needs no polling.
class MemoryPressureMonitor {
 public:
  MemoryPressureMonitor(HWND window, UINT message);
  ~MemoryPressureMonitor();
  MemoryPressureMonitor(const MemoryPressureMonitor&) = delete;
  MemoryPressureMonitor& operator=(const MemoryPressureMonitor&) = delete;

  // Handles a posted |message| on the window thread. Returns true when memory
  // became low.
  bool HandleSignal(WPARAM low);

 private:
  static void CALLBACK OnLow(void* context, BOOLEAN timed_out);
  static void CALLBACK OnRecovered(void* context, BOOLEAN timed_out);
  void Wait(bool low);
  void Cancel();

  const HWND window_;
  const UINT message_;
  HANDLE low_ = nullptr;
  HANDLE recovered_ = nullptr;
  HANDLE wait_ = nullptr;
};

#endif  // RUNNER_MEMORY_PRESSURE_MONITOR_H_
