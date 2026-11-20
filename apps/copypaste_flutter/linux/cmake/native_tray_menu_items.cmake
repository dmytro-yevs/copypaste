# cnativeapi 0.3.0 keeps a raw pointer to every tray menu item it last exported
# over D-Bus. The menu frees an item when the application removes it, while the
# desktop's menu host may still ask about the layout it read before; the stale
# pointer then crashes the process. Patch only the build copy; keep the locked
# package cache untouched.
function(_replace_exactly_once text_variable old new)
  set(_text "${${text_variable}}")
  string(LENGTH "${_text}" _length)
  string(REPLACE "${old}" "" _without "${_text}")
  string(LENGTH "${_without}" _length_without)
  string(LENGTH "${old}" _old_length)
  math(EXPR _removed "${_length} - ${_length_without}")
  if(NOT _removed EQUAL _old_length)
    message(FATAL_ERROR "Review cnativeapi tray menu item lifetime patch: ${old}")
  endif()
  string(REPLACE "${old}" "${new}" _text "${_text}")
  set(${text_variable} "${_text}" PARENT_SCOPE)
endfunction()

if(NOT TARGET cnativeapi)
  message(FATAL_ERROR "Review cnativeapi tray menu item lifetime patch: no cnativeapi target")
endif()
get_target_property(_cnativeapi_sources cnativeapi SOURCES)
set(_tray_source_path "")
foreach(_source IN LISTS _cnativeapi_sources)
  if(_source MATCHES "/platform/linux/tray_icon_linux\\.cpp$")
    set(_tray_source_path "${_source}")
    break()
  endif()
endforeach()
if(NOT _tray_source_path)
  message(FATAL_ERROR "Review cnativeapi tray menu item lifetime patch: no tray icon source")
endif()

file(READ "${_tray_source_path}" _tray_source)
# The layout is rebuilt from the live menu: stop tracking the previous items.
_replace_exactly_once(_tray_source
  "      dbusmenu_items_.clear();"
  "      ForgetDbusMenuItems();")
# The tracked addresses live in the map, so release them before it goes away.
_replace_exactly_once(_tray_source
  "  void Cleanup() {"
  "  void Cleanup() {
    ForgetDbusMenuItems();")
# GTK clears a tracked pointer when its item is finalized. Every reader of the
# map already treats a null item as one that no longer exists.
_replace_exactly_once(_tray_source
  "    dbusmenu_items_[id] = item;
    return id;
  }"
  "    TrackDbusMenuItem(id, item);
    return id;
  }

  void TrackDbusMenuItem(int id, GtkWidget* item) {
    GtkWidget*& tracked = dbusmenu_items_[id];
    if (tracked == item) {
      return;
    }
    if (tracked) {
      g_object_remove_weak_pointer(G_OBJECT(tracked), reinterpret_cast<gpointer*>(&tracked));
    }
    tracked = item;
    if (tracked) {
      g_object_add_weak_pointer(G_OBJECT(tracked), reinterpret_cast<gpointer*>(&tracked));
    }
  }

  void ForgetDbusMenuItems() {
    for (auto& entry : dbusmenu_items_) {
      if (entry.second) {
        g_object_remove_weak_pointer(G_OBJECT(entry.second),
                                     reinterpret_cast<gpointer*>(&entry.second));
      }
    }
    dbusmenu_items_.clear();
  }")

set(_patched "${CMAKE_CURRENT_BINARY_DIR}/tray_icon_linux_tracked_items.cpp")
file(WRITE "${_patched}" "${_tray_source}")
list(REMOVE_ITEM _cnativeapi_sources "${_tray_source_path}")
list(APPEND _cnativeapi_sources "${_patched}")
# The source includes its neighbours by relative path.
get_filename_component(_tray_source_dir "${_tray_source_path}" DIRECTORY)
target_include_directories(cnativeapi PRIVATE "${_tray_source_dir}")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_tray_source_path}")
set_property(TARGET cnativeapi PROPERTY SOURCES "${_cnativeapi_sources}")
