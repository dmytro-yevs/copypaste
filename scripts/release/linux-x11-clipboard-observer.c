#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/XRes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Fixture-only inspector: it never requests selection data or window titles. */
static unsigned long property_pid(Display *display, Window window, Atom pid_atom) {
  Atom type; int format; unsigned long count, after; unsigned char *data = NULL;
  if (XGetWindowProperty(display, window, pid_atom, 0, 1, False, XA_CARDINAL,
                         &type, &format, &count, &after, &data) != Success || !data)
    return 0;
  unsigned long value = count == 1 && format == 32 ? *(unsigned long *)data : 0;
  XFree(data); return value;
}

int main(void) {
  Display *display = XOpenDisplay(NULL);
  if (!display) return 2;
  Window root = DefaultRootWindow(display);
  Atom clipboard = XInternAtom(display, "CLIPBOARD", False);
  Atom net_clients = XInternAtom(display, "_NET_CLIENT_LIST", False);
  Atom net_pid = XInternAtom(display, "_NET_WM_PID", False);
  Window owner = XGetSelectionOwner(display, clipboard);
  Atom type; int format; unsigned long count = 0, after; unsigned char *data = NULL;
  unsigned long ewmh_count = 0, same_client_windows = 0, owner_pid = 0;
  pid_t xres_pid = 0;
  XResClientIdSpec spec = {.client = owner, .mask = XRES_CLIENT_ID_PID_MASK};
  XResClientIdValue *values = NULL; long values_count = 0;
  int xres_pid_present = owner != None
      && XResQueryClientIds(display, 1, &spec, &values_count, &values) == Success
      && values_count == 1 && values != NULL;
  if (xres_pid_present) {
    xres_pid = XResGetClientPid(values);
    xres_pid_present = xres_pid > 0;
  }
  if (owner != None) owner_pid = property_pid(display, owner, net_pid);
  if (XGetWindowProperty(display, root, net_clients, 0, 4096, False, XA_WINDOW,
                         &type, &format, &count, &after, &data) == Success && data) {
    ewmh_count = count;
    Window *windows = (Window *)data;
    for (unsigned long i = 0; i < count; i++)
      if (xres_pid_present && property_pid(display, windows[i], net_pid) == (unsigned long)xres_pid) same_client_windows++;
    XFree(data);
  }
  Window root_return, parent_return, *children = NULL; unsigned int child_count = 0;
  unsigned int tree_children = XQueryTree(display, root, &root_return, &parent_return, &children, &child_count) ? child_count : 0;
  if (children) XFree(children);
  XClassHint hint = {0}; int class_matches = 0;
  if (owner != None && XGetClassHint(display, owner, &hint)) {
    class_matches = hint.res_name && hint.res_class && !strcmp(hint.res_name, "org.copypaste.QualificationSource") && !strcmp(hint.res_class, "org.copypaste.QualificationSource");
    if (hint.res_name) XFree(hint.res_name);
    if (hint.res_class) XFree(hint.res_class);
  }
  printf("{\"selection_owner_present\":%s,\"selection_owner_pid_present\":%s,\"xres_client_pid_present\":%s,\"owner_pid_matches_xres\":%s,\"owner_class_matches_fixture\":%s,\"ewmh_client_count\":%lu,\"root_child_count\":%u,\"same_xres_pid_window_count\":%lu}\n",
         owner == None ? "false" : "true", owner_pid ? "true" : "false", xres_pid_present ? "true" : "false",
         owner_pid && xres_pid_present && owner_pid == (unsigned long)xres_pid ? "true" : "false", class_matches ? "true" : "false", ewmh_count, tree_children, same_client_windows);
  if (values) XResClientIdsDestroy(values_count, values);
  XCloseDisplay(display); return 0;
}
