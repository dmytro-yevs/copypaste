#include <stdio.h>

extern int copypaste_fixture_engine(void);

int main(void) {
  puts(FIXTURE_VERSION);
  return copypaste_fixture_engine();
}
