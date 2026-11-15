#ifndef COPYPASTE_LINUX_WINDOW_MATERIAL_H_
#define COPYPASTE_LINUX_WINDOW_MATERIAL_H_

#include <flutter_linux/flutter_linux.h>
#include <gtk/gtk.h>

// Gives an unrealized window an RGBA visual when the session can blur behind
// it: X11 with KWin's blur effect. GNOME and Wayland sessions have no blur
// an application can request, so their windows stay opaque. Returns whether
// the window can show the material.
bool prepare_linux_window_material(GtkWindow* window);

// Answers the window material channel of the engine shown in |window|; the
// channel lives as long as the window.
void register_linux_window_material(FlBinaryMessenger* messenger,
                                    GtkWindow* window, bool supported);

#endif  // COPYPASTE_LINUX_WINDOW_MATERIAL_H_
