#include "linux_release_page.h"

#include <cassert>

int main() {
  assert(is_linux_release_page_url(
      "https://github.com/dmytro-yevs/copypaste/releases/latest"));
  assert(is_linux_release_page_url(
      "https://github.com:443/dmytro-yevs/copypaste/releases/tag/v1.0.24"));
  assert(!is_linux_release_page_url(nullptr));
  assert(!is_linux_release_page_url("file:///tmp/release"));
  assert(!is_linux_release_page_url(
      "http://github.com/dmytro-yevs/copypaste/releases/latest"));
  assert(!is_linux_release_page_url(
      "https://github.com:444/dmytro-yevs/copypaste/releases/latest"));
  assert(!is_linux_release_page_url(
      "https://user@github.com/dmytro-yevs/copypaste/releases/latest"));
  assert(!is_linux_release_page_url(
      "https://github.com.evil.invalid/dmytro-yevs/copypaste/releases/latest"));
  assert(!is_linux_release_page_url(
      "https://github.com/another/project/releases/latest"));
  assert(!is_linux_release_page_url(
      "https://github.com/dmytro-yevs/copypaste/releases/%2e%2e/issues"));
}
