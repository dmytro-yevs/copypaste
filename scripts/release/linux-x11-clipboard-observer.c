#define _POSIX_C_SOURCE 200809L
#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/XRes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

/* Fixture-only inspector: it never requests selection data or window titles. */
static unsigned long property_pid(Display *display, Window window, Atom pid_atom) {
  Atom type; int format; unsigned long count, after; unsigned char *data = NULL;
  if (XGetWindowProperty(display, window, pid_atom, 0, 1, False, XA_CARDINAL,
                         &type, &format, &count, &after, &data) != Success || !data)
    return 0;
  unsigned long value = count == 1 && format == 32 ? *(unsigned long *)data : 0;
  XFree(data); return value;
}

typedef struct {
  int instance_exact;
  int class_exact;
  int class_casefold;
} ClassMatch;

static ClassMatch fixture_class(Display *display, Window window) {
  XClassHint hint = {0};
  ClassMatch match = {0};
  if (XGetClassHint(display, window, &hint)) {
    match.instance_exact = hint.res_name && !strcmp(hint.res_name, "org.copypaste.QualificationSource");
    match.class_exact = hint.res_class && !strcmp(hint.res_class, "org.copypaste.QualificationSource");
    match.class_casefold = hint.res_class && !strcasecmp(hint.res_class, "org.copypaste.QualificationSource");
  }
  if (hint.res_name) XFree(hint.res_name);
  if (hint.res_class) XFree(hint.res_class);
  return match;
}

static int client_ids(Display *display, Window window, XID *client_xid, pid_t *pid) {
  XResClientIdSpec specs[] = {
    {.client = window, .mask = XRES_CLIENT_ID_XID_MASK},
    {.client = window, .mask = XRES_CLIENT_ID_PID_MASK},
  };
  XResClientIdValue *values = NULL; long count = 0;
  int found_xid = 0, found_pid = 0;
  if (XResQueryClientIds(display, 2, specs, &count, &values) != Success || !values)
    return 0;
  for (long i = 0; i < count; i++) {
    if (values[i].spec.mask == XRES_CLIENT_ID_XID_MASK) {
      *client_xid = values[i].spec.client;
      found_xid = *client_xid != None;
    } else if (values[i].spec.mask == XRES_CLIENT_ID_PID_MASK) {
      *pid = XResGetClientPid(&values[i]);
      found_pid = *pid > 0;
    }
  }
  XResClientIdsDestroy(count, values);
  return found_xid && found_pid;
}

static int cached_desktop_entry(void) {
  const char *data_home = getenv("XDG_DATA_HOME");
  char path[4096]; struct stat details;
  if (!data_home || snprintf(path, sizeof(path), "%s/applications/org.copypaste.QualificationSource.desktop", data_home) >= (int)sizeof(path))
    return 0;
  return lstat(path, &details) == 0 && S_ISREG(details.st_mode) && !S_ISLNK(details.st_mode);
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
  XID owner_client_xid = None;
  pid_t xres_pid = 0;
  int xres_pid_present = owner != None && client_ids(display, owner, &owner_client_xid, &xres_pid);
  int canonical_client_window_present = 0, canonical_client_pid_matches_owner = 0;
  int canonical_client_window_instance_matches_fixture = 0;
  int canonical_client_window_class_matches_fixture = 0;
  int canonical_client_window_class_casefold_matches_fixture = 0;
  if (owner != None) owner_pid = property_pid(display, owner, net_pid);
  if (XGetWindowProperty(display, root, net_clients, 0, 4096, False, XA_WINDOW,
                         &type, &format, &count, &after, &data) == Success && data) {
    ewmh_count = count;
    Window *windows = (Window *)data;
    for (unsigned long i = 0; i < count; i++) {
      if (xres_pid_present && property_pid(display, windows[i], net_pid) == (unsigned long)xres_pid) same_client_windows++;
      XID candidate_client_xid = None; pid_t candidate_pid = 0;
      if (xres_pid_present && windows[i] != owner && client_ids(display, windows[i], &candidate_client_xid, &candidate_pid)
          && candidate_client_xid == owner_client_xid) {
        canonical_client_window_present = 1;
        canonical_client_pid_matches_owner |= candidate_pid == xres_pid && property_pid(display, windows[i], net_pid) == (unsigned long)xres_pid;
        ClassMatch match = fixture_class(display, windows[i]);
        canonical_client_window_instance_matches_fixture |= match.instance_exact;
        canonical_client_window_class_matches_fixture |= match.class_exact;
        canonical_client_window_class_casefold_matches_fixture |= match.class_casefold;
      }
    }
    XFree(data);
  }
  Window root_return, parent_return, *children = NULL; unsigned int child_count = 0;
  unsigned int tree_children = XQueryTree(display, root, &root_return, &parent_return, &children, &child_count) ? child_count : 0;
  if (children) XFree(children);
  ClassMatch owner_match = owner != None ? fixture_class(display, owner) : (ClassMatch){0};
  printf("{\"selection_owner_present\":%s,\"selection_owner_pid_present\":%s,\"xres_client_pid_present\":%s,\"canonical_client_xid_present\":%s,\"owner_pid_matches_xres\":%s,\"owner_instance_matches_fixture\":%s,\"owner_class_matches_fixture\":%s,\"owner_class_casefold_matches_fixture\":%s,\"canonical_client_window_present\":%s,\"canonical_client_pid_matches_owner\":%s,\"canonical_client_window_instance_matches_fixture\":%s,\"canonical_client_window_class_matches_fixture\":%s,\"canonical_client_window_class_casefold_matches_fixture\":%s,\"cached_desktop_entry_available\":%s,\"ewmh_client_count\":%lu,\"root_child_count\":%u,\"same_xres_pid_window_count\":%lu}\n",
         owner == None ? "false" : "true", owner_pid ? "true" : "false", xres_pid_present ? "true" : "false",
         xres_pid_present ? "true" : "false", owner_pid && xres_pid_present && owner_pid == (unsigned long)xres_pid ? "true" : "false", owner_match.instance_exact ? "true" : "false", owner_match.class_exact ? "true" : "false", owner_match.class_casefold ? "true" : "false",
         canonical_client_window_present ? "true" : "false", canonical_client_pid_matches_owner ? "true" : "false", canonical_client_window_instance_matches_fixture ? "true" : "false", canonical_client_window_class_matches_fixture ? "true" : "false", canonical_client_window_class_casefold_matches_fixture ? "true" : "false", cached_desktop_entry() ? "true" : "false", ewmh_count, tree_children, same_client_windows);
  XCloseDisplay(display); return 0;
}
