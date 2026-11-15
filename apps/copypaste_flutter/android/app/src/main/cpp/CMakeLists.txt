cmake_minimum_required(VERSION 3.22.1)
project(copypaste_pairing_host LANGUAGES CXX)

add_library(copypaste_pairing_host SHARED protected_pairing_jni.cpp)
target_include_directories(copypaste_pairing_host PRIVATE
    "${CMAKE_CURRENT_LIST_DIR}/../../../../../../../crates/copypaste-flutter-bridge/include")
find_library(log-lib log)
target_link_libraries(copypaste_pairing_host ${log-lib} dl)
