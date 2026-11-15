#import "ProtectedPairingBridge.h"

#import <dlfcn.h>
#import <mach-o/dyld.h>

#include <limits.h>
#include <string.h>

#include "copypaste_flutter_protected_pairing.h"

namespace {

constexpr size_t kMaximumArtifactBytes = 16 * 1024 * 1024;

void *BridgeModule() {
  static void *module = [] {
    const uint32_t count = _dyld_image_count();
    for (uint32_t index = 0; index < count; ++index) {
      const char *image = _dyld_get_image_name(index);
      if (image != nullptr && strstr(image, "libcopypaste_flutter_bridge.dylib") != nullptr) {
        return dlopen(image, RTLD_NOW | RTLD_NOLOAD);
      }
    }
    return static_cast<void *>(nullptr);
  }();
  return module;
}

template <typename Function>
Function Resolve(const char *name) {
  void *module = BridgeModule();
  return module == nullptr ? nullptr : reinterpret_cast<Function>(dlsym(module, name));
}

NSData *CopyAndReleaseBuffer(copypaste_flutter_protected_pairing_buffer buffer) {
  const auto freeBuffer = Resolve<decltype(&copypaste_flutter_free_protected_pairing_buffer)>(
      "copypaste_flutter_free_protected_pairing_buffer");
  if (buffer.bytes == nullptr || buffer.len == 0 || buffer.len > kMaximumArtifactBytes ||
      freeBuffer == nullptr) {
    if (buffer.bytes != nullptr && freeBuffer != nullptr) {
      freeBuffer(buffer);
    }
    return nil;
  }
  NSData *copy = [NSData dataWithBytes:buffer.bytes length:buffer.len];
  freeBuffer(buffer);
  return copy;
}

}  // namespace

uint64_t CPPairingBegin(NSString *ceremonyId, NSString *contextId) {
  const auto begin = Resolve<decltype(&copypaste_flutter_begin_protected_pairing_context)>(
      "copypaste_flutter_begin_protected_pairing_context");
  return begin == nullptr ? 0 : begin(ceremonyId.UTF8String, contextId.UTF8String);
}

BOOL CPPairingActive(NSString *contextId) {
  const auto active = Resolve<decltype(&copypaste_flutter_protected_pairing_context_active)>(
      "copypaste_flutter_protected_pairing_context_active");
  return active != nullptr && active(contextId.UTF8String);
}

BOOL CPPairingDetach(NSString *contextId) {
  const auto detach = Resolve<decltype(&copypaste_flutter_detach_protected_pairing_context)>(
      "copypaste_flutter_detach_protected_pairing_context");
  return detach != nullptr && detach(contextId.UTF8String);
}

BOOL CPPairingCancel(NSString *contextId) {
  const auto cancel = Resolve<decltype(&copypaste_flutter_cancel_protected_pairing_context)>(
      "copypaste_flutter_cancel_protected_pairing_context");
  return cancel != nullptr && cancel(contextId.UTF8String);
}

BOOL CPPairingStatus(NSString *contextId, uint64_t generation, uint32_t *state, uint64_t *expiresInMs) {
  if (state == nullptr || expiresInMs == nullptr) return NO;
  const auto status = Resolve<decltype(&copypaste_flutter_protected_pairing_status)>(
      "copypaste_flutter_protected_pairing_status");
  copypaste_flutter_protected_pairing_status_value output{};
  if (status == nullptr || !status(contextId.UTF8String, generation, &output)) return NO;
  *state = output.state;
  *expiresInMs = output.expires_in_ms;
  return YES;
}

NSData *CPPairingRevealQr(NSString *ceremonyId, NSString *contextId, uint64_t generation) {
  const auto reveal = Resolve<decltype(&copypaste_flutter_reveal_protected_pairing_qr_png)>(
      "copypaste_flutter_reveal_protected_pairing_qr_png");
  copypaste_flutter_protected_pairing_buffer output{};
  return reveal != nullptr && reveal(ceremonyId.UTF8String, contextId.UTF8String, generation, &output)
      ? CopyAndReleaseBuffer(output) : nil;
}

NSData *CPPairingRevealSas(NSString *contextId, uint64_t generation) {
  const auto reveal = Resolve<decltype(&copypaste_flutter_reveal_protected_pairing_sas)>(
      "copypaste_flutter_reveal_protected_pairing_sas");
  copypaste_flutter_protected_pairing_buffer output{};
  return reveal != nullptr && reveal(contextId.UTF8String, generation, &output)
      ? CopyAndReleaseBuffer(output) : nil;
}

BOOL CPPairingJoin(NSString *contextId, uint64_t generation, NSString *code, NSString *address) {
  const auto join = Resolve<decltype(&copypaste_flutter_protected_pairing_join)>(
      "copypaste_flutter_protected_pairing_join");
  return join != nullptr && join(contextId.UTF8String, generation, code.UTF8String, address.UTF8String);
}

BOOL CPPairingJoinURI(NSString *contextId, uint64_t generation, NSString *uri) {
  const auto join = Resolve<decltype(&copypaste_flutter_protected_pairing_join_uri)>(
      "copypaste_flutter_protected_pairing_join_uri");
  return join != nullptr && join(contextId.UTF8String, generation, uri.UTF8String);
}

BOOL CPPairingDecide(NSString *contextId, uint64_t generation, NSString *sas, BOOL accept) {
  const auto decide = Resolve<decltype(&copypaste_flutter_protected_pairing_decide)>(
      "copypaste_flutter_protected_pairing_decide");
  return decide != nullptr && decide(contextId.UTF8String, generation, sas.UTF8String, accept);
}
