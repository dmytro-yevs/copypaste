#include "linux_quick_paste_window.h"

#include <cassert>

int main() {
  const auto menu = linux_quick_paste_frame(
      {120, 30, 448, 800}, {0, 0, 1920, 1080}, false);
  assert(menu.x == 120 && menu.y == 30);
  assert(menu.width == 448 && menu.height == 800);

  const auto inspector = linux_quick_paste_frame(
      {1500, 600, 448, 800}, {0, 0, 1920, 1080}, true);
  assert(inspector.width == 816 && inspector.height == 800);
  assert(inspector.x == 1104 && inspector.y == 280);

  const auto small = linux_quick_paste_frame(
      {200, 100, 448, 800}, {40, 20, 640, 480}, true);
  assert(small.width == 640 && small.height == 480);
  assert(small.x == 40 && small.y == 20);

  const auto left_monitor = linux_quick_paste_frame(
      {-2000, -100, 448, 800}, {-1920, -40, 1920, 1040}, false);
  assert(left_monitor.x == -1920 && left_monitor.y == -40);
  assert(left_monitor.width == 448 && left_monitor.height == 800);

  // GTK monitor coordinates already account for compositor scaling. A narrow
  // logical work area must not be multiplied again by a physical pixel scale.
  const auto scaled_monitor = linux_quick_paste_frame(
      {300, 50, 448, 800}, {0, 0, 800, 600}, true);
  assert(scaled_monitor.width == 800 && scaled_monitor.height == 600);
  assert(scaled_monitor.x == 0 && scaled_monitor.y == 0);
}
