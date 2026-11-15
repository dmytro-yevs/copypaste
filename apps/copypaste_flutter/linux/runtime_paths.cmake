# Flutter copies plugin libraries with install(FILES), which does not rewrite
# their build RPATH. Link them with the portable sibling-library path up front.
set(CMAKE_INSTALL_RPATH "$ORIGIN")
set(CMAKE_BUILD_WITH_INSTALL_RPATH TRUE)
set(COPYPASTE_RUNNER_INSTALL_RPATH "$ORIGIN/lib")
