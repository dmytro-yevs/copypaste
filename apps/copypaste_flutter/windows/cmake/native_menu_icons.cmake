# cnativeapi 0.3.0 hardcodes 32px menu bitmaps, which doubles native row height.
# Patch only the build copy; keep the locked package cache untouched.
get_target_property(_nativeapi_sources nativeapi SOURCES)
foreach(_source IN LISTS _nativeapi_sources)
  if(_source MATCHES "/platform/windows/menu_windows\\.cpp$")
    file(READ "${_source}" _menu_source)
    set(_old "const int iconSize = 32;")
    string(FIND "${_menu_source}" "${_old}" _match)
    if(_match EQUAL -1)
      message(FATAL_ERROR "Review cnativeapi native menu bitmap size patch")
    endif()
    set(_clear "memset(pixels, 0, iconSize * iconSize * sizeof(DWORD));")
    string(FIND "${_menu_source}" "${_clear}" _clear_match)
    if(_clear_match EQUAL -1)
      message(FATAL_ERROR "Review cnativeapi native menu bitmap clear-size patch")
    endif()
    string(REPLACE "${_old}"
      "const int systemIconSize = GetSystemMetrics(SM_CXSMICON);
    const int iconSize = systemIconSize > 0 && systemIconSize <= 256 ? systemIconSize : 32;"
      _menu_source "${_menu_source}")
    string(REPLACE "${_clear}"
      "memset(pixels, 0, static_cast<size_t>(iconSize) * static_cast<size_t>(iconSize) * sizeof(DWORD));"
      _menu_source "${_menu_source}")
    set(_patched "${CMAKE_CURRENT_BINARY_DIR}/menu_windows_sized.cpp")
    file(WRITE "${_patched}" "${_menu_source}")
    list(REMOVE_ITEM _nativeapi_sources "${_source}")
    list(APPEND _nativeapi_sources "${_patched}")
    get_filename_component(_source_dir "${_source}" DIRECTORY)
    target_include_directories(nativeapi PRIVATE "${_source_dir}")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${_source}")
    set_property(TARGET nativeapi PROPERTY SOURCES "${_nativeapi_sources}")
    break()
  endif()
endforeach()
