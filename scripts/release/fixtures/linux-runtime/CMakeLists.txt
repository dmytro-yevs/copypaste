cmake_minimum_required(VERSION 3.13)
project(copypaste_runtime_fixture LANGUAGES C)

include("${COPYPASTE_RUNTIME_PATHS}")

add_library(engine SHARED engine.c)
set_target_properties(engine PROPERTIES OUTPUT_NAME flutter_linux_gtk)
add_library(plugin SHARED plugin.c)
set_target_properties(plugin PROPERTIES OUTPUT_NAME desktop_drop_plugin)
target_link_libraries(plugin PRIVATE engine)

add_executable(copypaste main.c)
target_link_libraries(copypaste PRIVATE engine)
target_compile_definitions(copypaste PRIVATE FIXTURE_VERSION="${FIXTURE_VERSION}")
set_target_properties(copypaste PROPERTIES INSTALL_RPATH "${COPYPASTE_RUNNER_INSTALL_RPATH}")

# Match Flutter: install the runner as a target, but copy shared plugin files.
install(TARGETS copypaste RUNTIME DESTINATION .)
install(FILES "$<TARGET_FILE:engine>" "$<TARGET_FILE:plugin>" DESTINATION lib)
