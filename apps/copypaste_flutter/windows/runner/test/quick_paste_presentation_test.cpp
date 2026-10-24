#include <gtest/gtest.h>
#include <windows.h>
#include <cstdint>
#include <algorithm>
#include <string>
#include <limits>
#include <set>
#include <vector>
#include "../flutter_window.h"

namespace {
std::vector<std::string> calls;
std::string fail_at;
int position_count;
bool presentation_visible;
RECT presentation_frame;
UINT presentation_flags;
UINT presentation_dpi = 96;
RECT presentation_work_area{0, 0, 1920, 1080};
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
  info->rcWork = presentation_work_area;
  return Succeeds("monitor-info");
}
BOOL WINAPI Position(HWND, HWND, int x, int y, int width, int height, UINT flags) {
  const bool showing = ++position_count != 1;
  if (showing) {
    presentation_visible = (flags & SWP_SHOWWINDOW) != 0;
    presentation_flags = flags;
    presentation_frame = RECT{x, y, x + width, y + height};
  }
  return Succeeds(showing ? "show" : "move");
}
UINT WINAPI Dpi(HWND) { return Succeeds("dpi") ? presentation_dpi : 0; }
BOOL WINAPI Foreground(HWND) { return Succeeds("foreground"); }
BOOL WINAPI Hide(HWND, int mode) { EXPECT_EQ(mode, SW_HIDE); calls.push_back("hide"); presentation_visible = false; return TRUE; }
class QuickPastePresentation : public testing::Test {
 protected:
  void SetUp() override {
    calls.clear(); fail_at.clear(); position_count = 0; during_presentation = nullptr;
    presentation_dpi = 96; presentation_work_area = RECT{0, 0, 1920, 1080};
  }
  QuickPastePresentationApi api{Cursor, Monitor, MonitorInfo, Position, Dpi, Foreground, Hide};
  HWND window = reinterpret_cast<HWND>(static_cast<uintptr_t>(1));
};
TEST_F(QuickPastePresentation, SuccessTraversesNativeSequenceBeforeOpened) {
  EXPECT_TRUE(PresentQuickPaste(window, api, [] { calls.push_back("opened"); }));
  EXPECT_EQ(calls, (std::vector<std::string>{"cursor", "monitor", "monitor-info", "move", "dpi", "show", "foreground", "opened"}));
}
TEST_F(QuickPastePresentation, EachFailureStopsLaterOperationsAndOpened) {
  const std::vector<std::string> sequence{"cursor", "monitor", "monitor-info", "move", "dpi", "show", "foreground"};
  for (size_t index = 0; index < sequence.size(); ++index) {
    calls.clear(); position_count = 0; fail_at = sequence[index];
    EXPECT_FALSE(PresentQuickPaste(window, api, [] { calls.push_back("opened"); }));
    auto expected = std::vector<std::string>(sequence.begin(), sequence.begin() + index + 1);
    expected.push_back("hide");
    EXPECT_EQ(calls, expected);
  }
}
TEST_F(QuickPastePresentation, TrayUsesIconInsteadOfCursorAndRetainsFocusSequence) {
  fail_at = "cursor";
  const RECT anchor{1800, 1056, 1824, 1080};
  EXPECT_TRUE(PresentQuickPaste(window, api, [] { calls.push_back("opened"); },
                               [] { return true; }, {}, false, anchor));
  EXPECT_EQ(calls, (std::vector<std::string>{"monitor", "monitor-info", "move", "dpi", "show", "foreground", "opened"}));
  EXPECT_EQ(presentation_frame.right, anchor.right);
  EXPECT_EQ(presentation_frame.bottom, anchor.top - 8);
  EXPECT_EQ(presentation_frame.bottom - presentation_frame.top, 600);
}
TEST_F(QuickPastePresentation, PreparingTheMediaFrameDoesNotShowOrActivateTheWindow) {
  const RECT anchor{1800, 1056, 1824, 1080};
  EXPECT_TRUE(PresentQuickPaste(window, api, [] { calls.push_back("opened"); },
                               [] { return true; }, {}, false, anchor, false));
  EXPECT_EQ(calls, (std::vector<std::string>{"monitor", "monitor-info", "move", "dpi", "show", "opened"}));
  EXPECT_FALSE(presentation_visible);
  EXPECT_EQ(presentation_flags & SWP_SHOWWINDOW, 0u);
  EXPECT_NE(presentation_flags & SWP_NOACTIVATE, 0u);
}
TEST_F(QuickPastePresentation, CursorGapUsesLogicalPixelsOnScaledDisplays) {
  presentation_work_area = RECT{0, 0, 3840, 2160};
  for (UINT dpi : {96u, 144u, 192u}) {
    presentation_dpi = dpi;
    position_count = 0;
    EXPECT_TRUE(PresentQuickPaste(window, api, [] {}));
    EXPECT_EQ(presentation_frame.left, 100);
    EXPECT_EQ(presentation_frame.top, 100 + ::MulDiv(8, dpi, USER_DEFAULT_SCREEN_DPI));
    EXPECT_EQ(presentation_frame.right - presentation_frame.left,
              ::MulDiv(448, dpi, USER_DEFAULT_SCREEN_DPI));
  }
}
TEST(QuickPasteTrayGeometry, TopTrayOpensBelowAndLeftEdgeClamps) {
  const auto frame = QuickPasteTrayFrame(RECT{4, 0, 28, 24}, RECT{0, 24, 1920, 1080}, 448, 600, 8);
  EXPECT_EQ(frame.left, 0);
  EXPECT_EQ(frame.top, 32);
  EXPECT_EQ(frame.bottom - frame.top, 600);
}
TEST(QuickPasteTrayGeometry, ShortSecondaryMonitorConstrainsExpandedInspector) {
  const auto frame = QuickPasteTrayFrame(RECT{-80, 476, -56, 500}, RECT{-900, 0, 0, 476}, 816, 600, 8);
  EXPECT_EQ(frame.left, -872);
  EXPECT_EQ(frame.right, -56);
  EXPECT_EQ(frame.top, 0);
  EXPECT_EQ(frame.bottom, 468);
}
TEST(QuickPasteTrayGeometry, ScaledDimensionsRemainBoundedAboveTheTaskbar) {
  const auto frame = QuickPasteTrayFrame(RECT{3600, 2112, 3648, 2160}, RECT{0, 0, 3840, 2112}, 896, 1200, 16);
  EXPECT_EQ(frame.right, 3648);
  EXPECT_EQ(frame.bottom, 2096);
  EXPECT_EQ(frame.top, 896);
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
UINT recovery_inserted;
std::set<int> held_keys;
std::set<int> simulated_modifiers;
std::vector<INPUT> submitted_inputs;
std::vector<std::vector<INPUT>> input_batches;
int accepted_v_down;
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
  EXPECT_EQ(size, sizeof(INPUT));
  const bool paste_batch = std::any_of(records, records + count, [](const auto& input) {
    return input.ki.wVk == 'V';
  });
  if (paste_batch) {
    submitted_inputs.assign(records, records + count);
    simulated_modifiers = held_keys;
  }
  input_batches.emplace_back(records, records + count);
  const UINT accepted = std::min(paste_batch ? inserted : recovery_inserted, count);
  for (UINT index = 0; index < accepted; ++index) {
    const auto& input = records[index];
    const int key = input.ki.wVk == VK_CONTROL ? VK_LCONTROL : input.ki.wVk;
    const bool up = (input.ki.dwFlags & KEYEVENTF_KEYUP) != 0;
    if (key == 'V') {
      if (!up) ++accepted_v_down;
      continue;
    }
    if (up) simulated_modifiers.erase(key);
    else simulated_modifiers.insert(key);
  }
  return accepted;
}
SHORT WINAPI KeyState(int key) { return held_keys.count(key) ? static_cast<SHORT>(0x8000) : 0; }
BOOL WINAPI RevealPosition(HWND window, HWND after, int x, int y, int width, int height, UINT flags) {
  EXPECT_EQ(window, popup);
  EXPECT_EQ(after, HWND_TOPMOST);
  const bool result = Position(window, after, x, y, width, height, flags);
  if (!result) ::SetLastError(ERROR_INVALID_WINDOW_HANDLE);
  return result;
}
BOOL WINAPI RevealActivation(HWND window) {
  EXPECT_EQ(window, popup);
  const bool result = Foreground(window);
  if (!result) ::SetLastError(ERROR_ACCESS_DENIED);
  return result;
}
class QuickPasteReveal : public testing::Test {
 protected:
  QuickPastePresentationApi api{Cursor, Monitor, MonitorInfo, RevealPosition,
                               Dpi, RevealActivation, Hide, CurrentForeground};
  void SetUp() override {
    calls.clear(); fail_at.clear(); position_count = 1;
    during_presentation = nullptr;
    foreground = target_a;
    presentation_visible = false;
  }
};
TEST_F(QuickPasteReveal, SavedTargetAllowsShowAndActivation) {
  EXPECT_TRUE(RevealQuickPaste(popup, target_a, api));
  EXPECT_EQ(calls, (std::vector<std::string>{"get-foreground", "show", "foreground"}));
  EXPECT_TRUE(presentation_visible);
  EXPECT_EQ(presentation_flags, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_FRAMECHANGED);
}
TEST_F(QuickPasteReveal, CurrentPopupSelfActivationAllowsShowAndActivation) {
  foreground = popup;
  EXPECT_TRUE(RevealQuickPaste(popup, target_a, api));
  EXPECT_EQ(calls, (std::vector<std::string>{"get-foreground", "show", "foreground"}));
  EXPECT_TRUE(presentation_visible);
}
TEST_F(QuickPasteReveal, ForeignMainAndMissingForegroundNeverShowOrActivate) {
  for (HWND other : {target_b, own_main, static_cast<HWND>(nullptr)}) {
    calls.clear(); foreground = other;
    EXPECT_FALSE(RevealQuickPaste(popup, target_a, api));
    EXPECT_EQ(calls, (std::vector<std::string>{"get-foreground"}));
    EXPECT_FALSE(presentation_visible);
  }
}
TEST_F(QuickPasteReveal, PositionFailureStopsActivationAndPreservesError) {
  fail_at = "show";
  EXPECT_FALSE(RevealQuickPaste(popup, target_a, api));
  EXPECT_EQ(::GetLastError(), ERROR_INVALID_WINDOW_HANDLE);
  EXPECT_EQ(calls, (std::vector<std::string>{"get-foreground", "show"}));
}
TEST_F(QuickPasteReveal, ActivationFailurePreservesErrorForCallerCleanup) {
  fail_at = "foreground";
  EXPECT_FALSE(RevealQuickPaste(popup, target_a, api));
  EXPECT_EQ(::GetLastError(), ERROR_ACCESS_DENIED);
  EXPECT_EQ(calls, (std::vector<std::string>{"get-foreground", "show", "foreground"}));
}
class QuickPasteTarget : public testing::Test {
 protected:
  QuickPasteTargetApi api{CurrentForeground, Root, Owner, OwnPid, Alive, Visible, TargetHide, ActivateTarget, Input, KeyState};
  QuickPasteTargetSession owner{api};
  void SetUp() override {
    calls.clear(); fail_at.clear(); position_count = 0; during_presentation = nullptr;
    presentation_dpi = 96; presentation_work_area = RECT{0, 0, 1920, 1080};
    foreground = popup; alive = true; visible = true;
    target_pid = 2; target_tid = 3; activation_result = true;
    activation_changes_focus = true;
    inserted = recovery_inserted = std::numeric_limits<UINT>::max();
    held_keys.clear(); submitted_inputs.clear(); simulated_modifiers.clear();
    input_batches.clear(); accepted_v_down = 0;
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
TEST_F(QuickPasteTarget, ActiveReopenHidesOldMediaWithoutLosingTheCapturedTarget) {
  const auto old_id = owner.Begin(target_a, popup);
  const auto id = owner.Begin(popup, popup);
  EXPECT_GT(id, old_id);
  EXPECT_FALSE(owner.HideForPreparation(old_id, popup));
  EXPECT_EQ(hide_calls, 0);
  EXPECT_TRUE(owner.HideForPreparation(id, popup));
  EXPECT_EQ(owner.id(), id);
  EXPECT_FALSE(visible);
  EXPECT_EQ(hide_calls, 1);
  EXPECT_EQ(input_calls, 0);
  EXPECT_FALSE(owner.Paste(old_id, popup));
  EXPECT_TRUE(owner.Paste(id, popup));
  EXPECT_EQ(foreground, target_a);
  EXPECT_EQ(input_calls, 1);
}
TEST_F(QuickPasteTarget, DelayedFailureRetirementCannotDestroyReopenedOrReplacementEngine) {
  const uint64_t generation = 7;
  const auto old_id = owner.Begin(target_a, popup);
  owner.Close(old_id, popup);
  EXPECT_TRUE(CanRetireQuickPaste(generation, generation, owner.id(), false));
  const auto reopened = owner.Begin(target_b, popup);
  EXPECT_GT(reopened, old_id);
  EXPECT_FALSE(CanRetireQuickPaste(generation, generation, owner.id(), false));
  owner.Close(reopened, popup);
  EXPECT_FALSE(CanRetireQuickPaste(generation, generation + 1, owner.id(), false));
  EXPECT_FALSE(CanRetireQuickPaste(generation, generation, owner.id(), true));
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
  ASSERT_EQ(submitted_inputs.size(), 4u);
  EXPECT_EQ(submitted_inputs[0].ki.wVk, VK_CONTROL);
  EXPECT_EQ(submitted_inputs[1].ki.wVk, 'V');
  EXPECT_EQ(submitted_inputs[2].ki.dwFlags, KEYEVENTF_KEYUP);
  EXPECT_EQ(submitted_inputs[3].ki.dwFlags, KEYEVENTF_KEYUP);
}
TEST_F(QuickPasteTarget, HeldActivationModifiersCannotChangePasteChordAndAreRestored) {
  const std::vector<std::set<int>> combinations{
      {VK_LMENU}, {VK_RMENU}, {VK_LSHIFT}, {VK_RSHIFT},
      {VK_LMENU, VK_LSHIFT}, {VK_RMENU, VK_RSHIFT},
      {VK_LCONTROL}, {VK_RCONTROL}, {VK_LCONTROL, VK_RCONTROL},
      {VK_LWIN}, {VK_RWIN}, {VK_RCONTROL, VK_RMENU, VK_RWIN},
      {VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN}};
  for (const auto& initial : combinations) {
    held_keys = initial;
    foreground = popup;
    const auto id = owner.Begin(target_a, popup);
    EXPECT_TRUE(owner.Paste(id, popup));
    EXPECT_FALSE(owner.Paste(id, popup));
    auto state = initial;
    int paste_down = 0;
    for (const auto& input : submitted_inputs) {
      EXPECT_EQ(input.type, INPUT_KEYBOARD);
      const int key = input.ki.wVk == VK_CONTROL ? VK_LCONTROL : input.ki.wVk;
      const bool up = (input.ki.dwFlags & KEYEVENTF_KEYUP) != 0;
      if (key == 'V' && !up) {
        ++paste_down;
        EXPECT_TRUE(state.count(VK_LCONTROL) || state.count(VK_RCONTROL));
        for (const int other : {VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN}) {
          EXPECT_EQ(state.count(other), 0u);
        }
      }
      if (key == VK_LMENU || key == VK_RMENU || key == VK_LWIN || key == VK_RWIN) {
        EXPECT_TRUE(state.count(VK_LCONTROL) || state.count(VK_RCONTROL));
      }
      if (key == VK_RMENU || key == VK_LWIN || key == VK_RWIN) {
        EXPECT_NE(input.ki.dwFlags & KEYEVENTF_EXTENDEDKEY, 0u);
      }
      if (up) state.erase(key);
      else state.insert(key);
    }
    EXPECT_EQ(paste_down, 1);
    EXPECT_EQ(state, initial);
    EXPECT_EQ(simulated_modifiers, initial);
    if (initial.count(VK_LCONTROL) || initial.count(VK_RCONTROL)) {
      EXPECT_EQ(std::count_if(submitted_inputs.begin(), submitted_inputs.end(), [](const auto& input) {
        return input.ki.wVk == VK_CONTROL || input.ki.wVk == VK_LCONTROL || input.ki.wVk == VK_RCONTROL;
      }), 0);
    }
  }
  EXPECT_EQ(input_calls, combinations.size());
}
TEST_F(QuickPasteTarget, PartialAndZeroInputAreFalseAndNeverRetried) {
  for (UINT count : {0u, 2u}) {
    inserted = count;
    const auto id = owner.Begin(target_a, popup);
    EXPECT_FALSE(owner.Paste(id, popup));
    EXPECT_FALSE(owner.Paste(id, popup));
    EXPECT_EQ(simulated_modifiers, held_keys);
  }
  EXPECT_EQ(input_calls, 3); // Two paste attempts; one modifier-only recovery.
  EXPECT_EQ(accepted_v_down, 1);
}
TEST_F(QuickPasteTarget, PartialHeldModifierBatchNeverReportsSuccessOrRetriesPaste) {
  for (const auto& initial : std::vector<std::set<int>>{
           {VK_RMENU, VK_LSHIFT, VK_RWIN},
           {VK_RCONTROL, VK_RMENU, VK_LSHIFT, VK_RWIN}}) {
    for (UINT count : {0u, 1u, 3u, 7u}) {
      held_keys = initial;
      inserted = count;
      foreground = popup;
      input_batches.clear(); accepted_v_down = 0;
      const int before_calls = input_calls;
      const auto id = owner.Begin(target_a, popup);
      EXPECT_FALSE(owner.Paste(id, popup));
      EXPECT_FALSE(owner.Paste(id, popup));
      EXPECT_EQ(simulated_modifiers, initial);
      EXPECT_EQ(owner.id(), 0);
      ASSERT_FALSE(input_batches.empty());
      EXPECT_EQ(input_batches.size(), count == 0 ? 1u : 2u);
      EXPECT_EQ(input_calls - before_calls, input_batches.size());
      EXPECT_LE(accepted_v_down, 1);
      if (input_batches.size() == 2) {
        for (const auto& input : input_batches.back()) {
          EXPECT_NE(input.ki.wVk, 'V');
          if (initial.count(VK_RCONTROL)) {
            EXPECT_NE(input.ki.wVk, VK_CONTROL);
            EXPECT_NE(input.ki.wVk, VK_RCONTROL);
          }
        }
      }
    }
  }
}
TEST_F(QuickPasteTarget, PrefixAfterPasteRecoversOnlyMissingModifiers) {
  held_keys = {VK_RMENU, VK_LSHIFT, VK_RWIN};
  inserted = 8;
  const auto id = owner.Begin(target_a, popup);
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_EQ(accepted_v_down, 1);
  EXPECT_EQ(simulated_modifiers, held_keys);
  ASSERT_EQ(input_batches.size(), 2u);
  ASSERT_EQ(input_batches.back().size(), 2u);
  EXPECT_EQ(input_batches.back()[0].ki.wVk, VK_RWIN);
  EXPECT_EQ(input_batches.back()[0].ki.dwFlags, KEYEVENTF_EXTENDEDKEY);
  EXPECT_EQ(input_batches.back()[1].ki.wVk, VK_CONTROL);
  EXPECT_EQ(input_batches.back()[1].ki.dwFlags, KEYEVENTF_KEYUP);
}
TEST_F(QuickPasteTarget, RejectedModifierRecoveryStopsWithoutAnotherAttempt) {
  held_keys = {VK_RMENU, VK_LSHIFT, VK_RWIN};
  inserted = 3;
  recovery_inserted = 0;
  const auto id = owner.Begin(target_a, popup);
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_FALSE(owner.Paste(id, popup));
  EXPECT_EQ(owner.id(), 0);
  EXPECT_EQ(input_calls, 2);
  EXPECT_EQ(accepted_v_down, 0);
  ASSERT_EQ(input_batches.size(), 2u);
  for (const auto& input : input_batches.back()) EXPECT_NE(input.ki.wVk, 'V');
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
        EXPECT_TRUE(ShowQuickPastePresentation(
            popup, owner, newer_id, presentation_api, [&] {
              ++newer_opened; calls.push_back("new-opened");
            }));
      };
      EXPECT_FALSE(ShowQuickPastePresentation(
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
    EXPECT_FALSE(ShowQuickPastePresentation(
        popup, owner, id, presentation_api, [&] { ++opened; }));
    EXPECT_EQ(owner.id(), 0);
    EXPECT_EQ(opened, 0);
    EXPECT_EQ(std::count(calls.begin(), calls.end(), "hide"), 1);
    EXPECT_FALSE(presentation_visible);
    EXPECT_FALSE(owner.Paste(id, popup));
  }
}
TEST_F(QuickPasteTarget, DelayedOpenedFailureOnlyClosesCurrentLiveGenerationAndSession) {
  const uint64_t generation = 4;
  const auto old_id = owner.Begin(target_a, popup);
  const auto current_id = owner.Begin(target_b, popup);
  const auto fail = [&](uint64_t requested_generation, int64_t requested_id, bool retiring) {
    if (CanFailQuickPasteOpen(requested_generation, generation, requested_id, owner.id(), retiring)) {
      owner.Close(requested_id, popup);
    }
  };
  calls.clear();
  fail(generation, old_id, false);
  fail(generation - 1, current_id, false);
  fail(generation, current_id, true);
  fail(generation, 0, false);
  EXPECT_EQ(owner.id(), current_id);
  EXPECT_TRUE(calls.empty());
  EXPECT_EQ(hide_calls, 0);
  fail(generation, current_id, false);
  EXPECT_EQ(owner.id(), 0);
  EXPECT_EQ(hide_calls, 1);
  fail(generation, current_id, false);
  EXPECT_EQ(hide_calls, 1);
}
}  // namespace
