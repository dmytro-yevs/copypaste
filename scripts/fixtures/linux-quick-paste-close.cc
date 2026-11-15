#include "linux_quick_paste_window.h"

#include <cassert>
#include <memory>

int main(int argc, char** argv) {
  if (!gtk_init_check(&argc, &argv)) return 1;
  auto* window = GTK_WINDOW(gtk_window_new(GTK_WINDOW_TOPLEVEL));
  int closes = 0;
  int destroys = 0;
  auto lifetime = std::make_shared<int>(1);
  std::weak_ptr<int> retained = lifetime;
  configure_linux_quick_paste_window(window, [&, lifetime] {
    ++closes;
    gtk_widget_hide(GTK_WIDGET(window));
  });
  lifetime.reset();
  assert(!retained.expired());
  g_signal_connect(window, "destroy", G_CALLBACK(+[](GtkWidget*, gpointer data) {
    ++*static_cast<int*>(data);
  }), &destroys);
  gtk_widget_show(GTK_WIDGET(window));

  // GTK emits delete-event for window-manager close, including Alt+F4.
  gboolean handled = FALSE;
  g_signal_emit_by_name(window, "delete-event", nullptr, &handled);
  assert(handled && closes == 1 && destroys == 0);
  assert(!gtk_widget_get_visible(GTK_WIDGET(window)));
  g_signal_emit_by_name(window, "delete-event", nullptr, &handled);
  assert(handled && closes == 1);

  gtk_widget_destroy(GTK_WIDGET(window));
  assert(destroys == 1 && retained.expired());

  auto* minimized = GTK_WINDOW(gtk_window_new(GTK_WINDOW_TOPLEVEL));
  configure_linux_quick_paste_window(minimized, [&] {
    ++closes;
    gtk_widget_hide(GTK_WIDGET(minimized));
  });
  gtk_widget_show(GTK_WIDGET(minimized));
  GdkEventWindowState iconified{};
  iconified.type = GDK_WINDOW_STATE;
  iconified.changed_mask = GDK_WINDOW_STATE_ICONIFIED;
  iconified.new_window_state = GDK_WINDOW_STATE_ICONIFIED;
  g_signal_emit_by_name(minimized, "window-state-event", &iconified, &handled);
  assert(handled && closes == 2);
  assert(!gtk_widget_get_visible(GTK_WIDGET(minimized)));
  g_signal_emit_by_name(minimized, "delete-event", nullptr, &handled);
  assert(closes == 2);
  gtk_widget_destroy(GTK_WIDGET(minimized));
}
