#include <flutter/method_call.h>
#include <flutter/method_result_functions.h>
#include <gtest/gtest.h>
#include <windows.h>
#include <cstdint>
#include <memory>
#include <string>
#include <unordered_set>
#include <vector>
#include "hotkey_manager_windows_plugin.h"

namespace hotkey_manager_windows {
namespace {
using flutter::EncodableList;
using flutter::EncodableMap;
using flutter::EncodableValue;
using flutter::MethodCall;
using flutter::MethodResultFunctions;
struct NativeCall { HWND window; int id; };
std::vector<NativeCall> registrations;
std::vector<NativeCall> releases;
std::unordered_set<int> failed_releases;
bool reject_registration = false;
BOOL WINAPI RegisterFake(HWND window, int id, UINT, UINT) {
  registrations.push_back({window, id});
  return reject_registration ? FALSE : TRUE;
}
BOOL WINAPI UnregisterFake(HWND window, int id) {
  releases.push_back({window, id});
  return failed_releases.count(id) == 0 ? TRUE : FALSE;
}
DWORD WINAPI ErrorFake() { return ERROR_HOTKEY_ALREADY_REGISTERED; }
struct Reply { bool success = false; std::string error; };
Reply Invoke(HotkeyManagerWindowsPlugin& plugin, const char* method, EncodableValue args) {
  Reply reply;
  plugin.HandleMethodCall(
      MethodCall<EncodableValue>(method, std::make_unique<EncodableValue>(args)),
      std::make_unique<MethodResultFunctions<EncodableValue>>(
          [&reply](const EncodableValue* value) { reply.success = value && std::get<bool>(*value); },
          [&reply](const std::string& code, const std::string&, const EncodableValue*) { reply.error = code; }, nullptr));
  return reply;
}
EncodableValue Args(const std::string& identifier) {
  return EncodableValue(EncodableMap{
      {EncodableValue("identifier"), EncodableValue(identifier)},
      {EncodableValue("keyCode"), EncodableValue(117)},
      {EncodableValue("modifiers"), EncodableValue(EncodableList{EncodableValue("control")})}});
}
class HotKeyContracts : public testing::Test {
 protected:
  void SetUp() override {
    registrations.clear(); releases.clear(); failed_releases.clear(); reject_registration = false;
  }
  HotKeyApi api{RegisterFake, UnregisterFake, ErrorFake};
  HWND first = reinterpret_cast<HWND>(static_cast<uintptr_t>(1));
  HWND second = reinterpret_cast<HWND>(static_cast<uintptr_t>(2));
};
TEST_F(HotKeyContracts, FailedReservationIsNotTrackedAndMalformedArgumentsAreSafe) {
  HotkeyManagerWindowsPlugin plugin(api, first);
  reject_registration = true;
  EXPECT_EQ(Invoke(plugin, "register", Args("chosen")).error, "hotkey_registration_failed");
  EXPECT_TRUE(Invoke(plugin, "unregister", Args("chosen")).success);
  EXPECT_TRUE(releases.empty());
  EXPECT_EQ(Invoke(plugin, "register", EncodableValue()).error, "invalid_hotkey_arguments");
  EXPECT_EQ(Invoke(plugin, "unregister", EncodableValue("bad")).error, "invalid_hotkey_arguments");
  EXPECT_EQ(registrations.size(), 1u);
}
TEST_F(HotKeyContracts, FailedReleaseKeepsOriginalHandleAndBlocksReplacement) {
  HotkeyManagerWindowsPlugin plugin(api, first);
  ASSERT_TRUE(Invoke(plugin, "register", Args("chosen")).success);
  const int id = registrations[0].id;
  failed_releases.insert(id);
  EXPECT_EQ(Invoke(plugin, "unregister", Args("chosen")).error, "hotkey_unregistration_failed");
  EXPECT_EQ(Invoke(plugin, "register", Args("chosen")).error, "hotkey_unregistration_failed");
  EXPECT_EQ(registrations.size(), 1u);
  ASSERT_EQ(releases.size(), 2u);
  EXPECT_EQ(releases[0].id, id); EXPECT_EQ(releases[1].window, first);
  failed_releases.clear();
  EXPECT_TRUE(Invoke(plugin, "unregister", Args("chosen")).success);
  EXPECT_TRUE(Invoke(plugin, "unregister", Args("chosen")).success);
  EXPECT_EQ(releases.size(), 3u);
}
TEST_F(HotKeyContracts, OwnersRetainDistinctIdsAndDestructorReleasesOwnedWindow) {
  {
    HotkeyManagerWindowsPlugin one(api, first), two(api, second);
    ASSERT_TRUE(Invoke(one, "register", Args("shared")).success);
    ASSERT_TRUE(Invoke(two, "register", Args("shared")).success);
    ASSERT_EQ(registrations.size(), 2u);
    EXPECT_NE(registrations[0].id, registrations[1].id);
    EXPECT_TRUE(Invoke(one, "unregister", Args("shared")).success);
    EXPECT_EQ(releases[0].window, first);
  }
  ASSERT_EQ(releases.size(), 2u);
  EXPECT_EQ(releases[1].window, second); EXPECT_EQ(releases[1].id, registrations[1].id);
}
TEST_F(HotKeyContracts, BulkReleaseRetainsOnlyFailedHandlesForExplicitRetry) {
  HotkeyManagerWindowsPlugin plugin(api, first);
  ASSERT_TRUE(Invoke(plugin, "register", Args("one")).success);
  ASSERT_TRUE(Invoke(plugin, "register", Args("two")).success);
  const int failed = registrations[0].id;
  failed_releases.insert(failed);
  EXPECT_EQ(Invoke(plugin, "unregisterAll", EncodableValue()).error, "hotkey_unregistration_failed");
  ASSERT_EQ(releases.size(), 2u);
  failed_releases.clear();
  EXPECT_TRUE(Invoke(plugin, "unregisterAll", EncodableValue()).success);
  ASSERT_EQ(releases.size(), 3u); EXPECT_EQ(releases.back().id, failed);
}
}  // namespace
}  // namespace hotkey_manager_windows
