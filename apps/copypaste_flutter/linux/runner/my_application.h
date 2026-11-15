#ifndef FLUTTER_MY_APPLICATION_H_
#define FLUTTER_MY_APPLICATION_H_

#include <gtk/gtk.h>

G_DECLARE_FINAL_TYPE(MyApplication, my_application, MY, APPLICATION, GtkApplication)

// `quick_paste` is a separately scoped, short-lived process. The root
// application remains a single GApplication instance for its desktop session.
MyApplication* my_application_new(gboolean quick_paste);

#endif  // FLUTTER_MY_APPLICATION_H_
