cmake_minimum_required(VERSION 3.13)
project(runner LANGUAGES CXX)

add_executable(${BINARY_NAME}
  "main.cc"
  "my_application.cc"
  "linux_host_channels.cc"
  "linux_gnome_shortcuts.cc"
  "linux_glibc_version.cc"
  "linux_packagekit.cc"
  "linux_portal.cc"
  "linux_restart_helper.cc"
  "linux_x11_quick_paste.cc"
  "${FLUTTER_MANAGED_DIR}/generated_plugin_registrant.cc"
)

apply_standard_settings(${BINARY_NAME})
target_compile_features(${BINARY_NAME} PRIVATE cxx_std_17)
target_compile_options(${BINARY_NAME} PRIVATE -Wextra -Werror)
add_definitions(-DAPPLICATION_ID="${APPLICATION_ID}")
add_definitions(-DCOPYPASTE_VERSION="${COPYPASTE_VERSION}")
target_link_libraries(${BINARY_NAME} PRIVATE flutter PkgConfig::GTK PkgConfig::X11)
target_include_directories(${BINARY_NAME} PRIVATE "${CMAKE_SOURCE_DIR}")
