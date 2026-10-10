# Mutter writer identity bridge

The GNOME clipboard companion requires a small maintained Mutter export. Apply
the patch matching the installed Mutter major before building the native shim:

| GNOME Shell | Mutter pkg-config name | Patch |
| --- | --- | --- |
| 46 | `libmutter-14` | `mutter-46-writer-identity.patch` |
| 47 | `libmutter-15` | `mutter-47-writer-identity.patch` |

The immutable source commits currently used for verification are GNOME 46
`fe8d2be3f90f89f286c89b164c94a4f86552bc97` and GNOME 47
`d688d0823fdc044885a4bd5f51dff038c9c6e8fc`. Run `verify-patch.sh` in a Linux
builder with Mutter's normal build dependencies; it fetches and verifies the
commit, applies the patch, configures Meson, and builds the matching Mutter
shared library. The script does not install packages or modify the host.

The export accepts the actual `MetaSelectionSource` selected by Mutter. It
obtains the Wayland client's credentials from the source resource, then scans
Mutter's own Wayland windows for the exact source client. It returns an app ID
only if all matching windows provide the same non-empty ID. Mutter's
`xdg_toplevel.set_app_id` stores the Wayland app ID through
`meta_window_set_wm_class`; the exported `meta_window_get_wm_class` therefore
reads the compositor's stored Wayland app ID for that exact window. A
missing, conflicting, or non-Wayland source returns unknown. It never uses
focus, title, `/proc`, object layout offsets, or a PID-only app-ID guess.

The API is intentionally versioned with the Mutter patch. The native shim is
compiled against stock `libmutter-14` or `libmutter-15` headers and resolves
the bridge symbols from the running Mutter process at runtime. An unpatched
Mutter reports the explicit unavailable status and leaves the existing
companion running; qualification enables the matching patch before testing
writer identity.
