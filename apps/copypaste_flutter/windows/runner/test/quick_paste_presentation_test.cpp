#include <gtest/gtest.h>
#include <windows.h>
#include <cstdint>
#include <algorithm>
#include <string>
#include <vector>
#include "../flutter_window.h"

namespace {
std::vector<std::string> calls;
std::string fail_at;
int position_count;
bool presentation_visible;
std::function<void(const std::string&)> during_presentation;
bool Succeeds(const std::string& operation) {
  calls.push_back(operation);
  const bool succeeds = fail_at != operation;
  const auto callback = during_presentation;
  if (callback) callback(operation);
  return succeeds;
}
BOOL WINAPI Cursor(LPPOINT point) {
  *point = POINT{100, 100};
  return Succeeds("cursor");
}
HMONITOR WINAPI Monitor(POINT, DWORD) {
  return Succeeds("monitor") ? reinterpret_cast<HMONITOR>(static_cast<uintptr_t>(1)) : nullptr;
}
BOOL WINAPI MonitorInfo(HMONITOR, LPMONITORINFO info) {
  info->rcWork = RECT{0, 0, 1920, 1080};
  return Succeeds("monitor-info");
}
BOOL WINAPI Position(HWND, HWND, int, int, int, int, UINT) {
  const bool showing = ++position_count != 1;
  if (showing) presentation_visible = true;
  return Succeeds(showing ? "show" : "move");
}
UINT WINAPI Dpi(HWND) { return Succeeds("dpi") ? 96 : 0; }
BOOL WINAPI Foreground(HWND) { return Succeeds("foreground"); }
BOOL WINAPI Hide(HWND, int mode) { EXPECT_EQ(mode, SW_HIDE); calls.push_back("hide"); presentation_visible = false; return TRUE; }
class QuickPastePresentation : public testing::Test {
 protected:
  void SetUp() override { calls.clear(); fail_at.clear(); position_count = 0; during_presentation = nullptr; }
  QuickPastePresentationApi api{Cursor, Monitor, MonitorInfo, Position, Dpi, Foreground, Hide};
  HWND window = reinterpret_cast<HWND>(static_cast<uintptr_t>(1));
};
TEST_F(QuickPastePresentation, SuccessTraversesNativeSequenceBeforeOpened) {
  EXPECT_TRUE(PresentQuickPasteAtCursor(window, api, [] { calls.push_back("opened"); }));
  EXPECT_EQ(calls, (std::vector<std::string>{"cursor", "monitor", "monitor-info", "move", "dpi", "show", "foreground", "opened"}));
}
TEST_F(QuickPastePresentation, EachFailureStopsLaterOperationsAndOpened) {
  const std::vector<std::string> sequence{"cursor", "monitor", "monitor-info", "move", "dpi", "show", "foreground"};
  for (size_t index = 0; index < sequence.size(); ++index) {
    calls.clear(); position_count = 0; fail_at = sequence[index];
    EXPECT_FALSE(PresentQuickPasteAtCursor(window, api, [] { calls.push_back("opened"); }));
    auto expected = std::vector<std::string>(sequence.begin(), sequence.begin() + index + 1);
    expected.push_back("hide");
    EXPECT_EQ(calls, expected);
  }
}
}  // namespace

namespace {
HWND target_a = reinterpret_cast<HWND>(static_cast<uintptr_t>(10));
HWND target_b = reinterpret_cast<HWND>(static_cast<uintptr_t>(20));
HWND popup = reinterpret_cast<HWND>(static_cast<uintptr_t>(30));
HWND own_main = reinterpret_cast<HWND>(static_cast<uintptr_t>(40));
HWND foreground;
bool alive;
bool visible;
DWORD target_pid;
DWORD target_tid;
bool activation_result;
bool activation_changes_focus;
UINT inserted;
int input_calls;
int activation_calls;
int hide_calls;
std::function<void()> during_activation;
QuickPasteTargetSession* active_session;
HWND WINAPI CurrentForeground() { calls.push_back("get-foreground"); return foreground; }
HWND WINAPI Root(HWND window, UINT flags) { EXPECT_EQ(flags, GA_ROOT); return window; }
DWORD WINAPI Owner(HWND window, LPDWORD pid) {
  *pid = window == popup || window == own_main ? 1 : target_pid;
  return target_tid;
}
DWORD WINAPI OwnPid() { return 1; }
BOOL WINAPI Alive(HWND) { return alive; }
BOOL WINAPI Visible(HWND) { return visible; }
BOOL WINAPI TargetHide(HWND window, int mode) {
  EXPECT_EQ(window, popup); EXPECT_EQ(mode, SW_HIDE);
  ++hide_calls; visible = false;
  EXPECT_TRUE(active_session->internally_hiding());
  return TRUE;
}
BOOL WINAPI ActivateTarget(HWND window) {
  calls.push_back("activate"); ++activation_calls;
  if (during_activation) during_activation();
  if (activation_changes_focus) foreground = window;
  return activation_result;
}
UINT WINAPI Input(UINT count, LPINPUT records, int size) {
  calls.push_back("input"); ++input_calls;
  EXPECT_EQ(count, 4u); EXPECT_EQ(size, sizeof(INPUT));
  EXPECT_EQ(records[0].ki.wVk, VK_CONTROL);
  EXPECT_EQ(records[1].ki.wVk, 'V');
  EXPECT_EQ(records[2].ki.dwFlags, KEYEVENTF_KEYUP);
  EXPECT_EQ(records[3].ki.dwFlags, KEYEVENTF_KEYUP);
  return inserted;
}
class QuickPasteTarget : public testing::Test {
 protected:
  QuickPasteTargetApi api{CurrentForeground, Root, Owner, OwnPid, Alive, Visible, TargetHide, ActivateTarget, Input};
  QuickPasteTargetSession owner{api};
  void SetUp() override {
    calls.clear(); fail_at.clear(); position_count = 0; during_presentation = nullptr;
    foreground = popup; alive = true; visible = true;
    target_pid = 2; target_tid = 3; activation_result = true;
    activation_changes_focus = true; inserted = 4;
    input_calls = activation_calls = hide_calls = 0;
    during_activation = nullptr; active_session = &owner;
  }
};
TEST_F(QuickPasteTarget, DismissThenOwnOrNilOpeningCannotReuseExternalTarget) {
  for (HWND sample : {own_main, popup, static_cast<HWND>(nullptr)}) {
    owner.Begin(target_a, popup); owner.Invalidate();
    const auto id = owner.Begin(sample, popup);
    EXPECT_FALSE(owner.Paste(id, popup));
  }
  EXPECT_EQ(input_calls, 0); EXPECT_EQ(activation_calls, 0); EXPECT_EQ(hide_calls, 0);
}
TEST_F(QuickPasteTarget, ActivePopupRetainsExternalRootButGetsNewId) {
  const auto old_id = owner.Begin(target_a, popup);
  const auto id = owner.Begin(popup, popup);
  EXPECT_GT(id, old_id);
  EXPECT_FALSE(owner.Paste(old_id, popup));
  EXPECT_TRUE(owner.Paste(id, popup));
  EXPECT_EQ(foreground, target_a); EXPECT_EQ(input_calls, 1);
  EXPECT_FALSE(owner.Paste(id, popup));
}
TEST_F(QuickPasteTarget, DifferentExternalRootReplacesOldTarget) {
  owner.Begin(target_a, popup);
  const auto id = owner.Begin(target_b, popup);
  EXPECT_TRUE(owner.Paste(id, popup));
  EXPECT_EQ(foreground, target_b);
}
TEST_F(QuickPasteTarget, DeadRootAndOwnerReuseFailBeforeActivation) {
  auto id = owner.Begin(target_a, popup); alive = false;
  EXPECT_FALSE(owner.Paste(id, popup)); alive = true;
  id = owner.Begin(target_a, popup); ++target_pid;
  EXPECT_FALSE(owner.Paste(id, popup));
  id = owner.Begin(target_a, popup); ++target_tid;
  EXPECT_FALSE(owner.Paste(id, popup));
  id = owner.Begin(target_a, popup); target_tid = 0;
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_EQ(activation_calls, 0); EXPECT_EQ(input_calls, 0);
}
TEST_F(QuickPasteTarget, ActivationDenialAndWrongFinalForegroundSubmitNothing) {
  auto id = owner.Begin(target_a, popup); activation_result = false;
  EXPECT_FALSE(owner.Paste(id, popup));
  foreground = popup; activation_result = true; activation_changes_focus = false;
  id = owner.Begin(target_a, popup);
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_EQ(activation_calls, 2); EXPECT_EQ(input_calls, 0);
}
TEST_F(QuickPasteTarget, ReentrantReopenDuringActivationCannotSendOrCloseNewPopup) {
  const auto id = owner.Begin(target_a, popup);
  int64_t next_id = 0;
  during_activation = [&] { next_id = owner.Begin(target_b, popup); };
  EXPECT_FALSE(owner.Paste(id, popup));
  owner.Close(id, popup);
  EXPECT_EQ(owner.id(), next_id); EXPECT_EQ(hide_calls, 1); EXPECT_EQ(input_calls, 0);
}
TEST_F(QuickPasteTarget, CurrentForegroundSkipsActivationAndInputIsConsumedOnce) {
  foreground = target_a;
  const auto id = owner.Begin(target_a, popup);
  EXPECT_TRUE(owner.Paste(id, popup));
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_EQ(activation_calls, 0); EXPECT_EQ(input_calls, 1);
  EXPECT_EQ(calls.back(), "input");
  EXPECT_EQ(calls[calls.size() - 2], "get-foreground");
}
TEST_F(QuickPasteTarget, PartialAndZeroInputAreFalseAndNeverRetried) {
  for (UINT count : {0u, 2u}) {
    inserted = count;
    const auto id = owner.Begin(target_a, popup);
    EXPECT_FALSE(owner.Paste(id, popup));
    EXPECT_FALSE(owner.Paste(id, popup));
  }
  EXPECT_EQ(input_calls, 2);
}
TEST_F(QuickPasteTarget, SupersededNativePresentationStagesCannotHideOrInvalidateNewOwner) {
  const QuickPastePresentationApi presentation_api{Cursor, Monitor, MonitorInfo, Position, Dpi, Foreground, Hide};
  for (const std::string stage : {"show", "foreground"}) {
    for (const bool old_stage_succeeds : {true, false}) {
      calls.clear(); position_count = 0; fail_at = old_stage_succeeds ? "" : stage;
      const auto id = owner.Begin(target_a, popup);
      int64_t newer_id = 0;
      int opened = 0;
      int newer_opened = 0;
      during_presentation = [&](const std::string& operation) {
        if (operation != stage) return;
        during_presentation = nullptr;
        newer_id = owner.Begin(target_b, popup);
        fail_at.clear(); position_count = 0;
        EXPECT_TRUE(ShowQuickPastePresentationAtCursor(
            popup, owner, newer_id, presentation_api, [&] {
              ++newer_opened; calls.push_back("new-opened");
            }));
      };
      EXPECT_FALSE(ShowQuickPastePresentationAtCursor(
          popup, owner, id, presentation_api, [&] { ++opened; }));
      EXPECT_GT(newer_id, id);
      EXPECT_EQ(owner.id(), newer_id);
      EXPECT_EQ(opened, 0);
      EXPECT_EQ(newer_opened, 1);
      EXPECT_EQ(calls.back(), "new-opened");
      EXPECT_TRUE(presentation_visible);
      EXPECT_EQ(std::count(calls.begin(), calls.end(), "hide"), 0);
      // Prove cleanup preserved the newer target as well as its ID.
      during_presentation = nullptr;
      foreground = target_b;
      EXPECT_TRUE(owner.Paste(newer_id, popup));
    }
  }
}
TEST_F(QuickPasteTarget, SameOwnerPresentationFailureCleansPartialWindowOnce) {
  const QuickPastePresentationApi presentation_api{Cursor, Monitor, MonitorInfo, Position, Dpi, Foreground, Hide};
  for (const std::string stage : {"show", "foreground"}) {
    calls.clear(); position_count = 0; fail_at = stage;
    const auto id = owner.Begin(target_a, popup);
    int opened = 0;
    EXPECT_FALSE(ShowQuickPastePresentationAtCursor(
        popup, owner, id, presentation_api, [&] { ++opened; }));
    EXPECT_EQ(owner.id(), 0);
    EXPECT_EQ(opened, 0);
    EXPECT_EQ(std::count(calls.begin(), calls.end(), "hide"), 1);
    EXPECT_FALSE(presentation_visible);
    EXPECT_FALSE(owner.Paste(id, popup));
  }
}
}  // namespace
