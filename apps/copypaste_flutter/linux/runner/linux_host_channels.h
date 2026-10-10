#ifndef FLUTTER_LINUX_HOST_CHANNELS_H_
#define FLUTTER_LINUX_HOST_CHANNELS_H_

#include <flutter_linux/flutter_linux.h>

void register_linux_host_channels(FlBinaryMessenger* messenger,
                                  GtkApplication* application);

void deliver_linux_pairing_uri(const gchar* uri);

#endif  // FLUTTER_LINUX_HOST_CHANNELS_H_
