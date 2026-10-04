#include <dlfcn.h>
#include <jni.h>

#include <cstdint>
#include <limits>

#include "copypaste_flutter_protected_pairing.h"

namespace {

constexpr size_t kMaximumArtifactBytes = 16 * 1024 * 1024;

void* BridgeModule() {
  static void* const module = dlopen(
      "libcopypaste_flutter_bridge.so", RTLD_NOW | RTLD_NOLOAD);
  return module;
}

template <typename Function>
Function Resolve(const char* name) {
  void* const module = BridgeModule();
  return module == nullptr ? nullptr
                           : reinterpret_cast<Function>(dlsym(module, name));
}

const char* UtfChars(JNIEnv* environment, jstring value) {
  return value == nullptr ? nullptr : environment->GetStringUTFChars(value, nullptr);
}

void ReleaseUtfChars(JNIEnv* environment, jstring value, const char* chars) {
  if (value != nullptr && chars != nullptr) {
    environment->ReleaseStringUTFChars(value, chars);
  }
}

jbyteArray CopyAndReleaseBuffer(
    JNIEnv* environment,
    copypaste_flutter_protected_pairing_buffer buffer) {
  const auto free_buffer = Resolve<decltype(&copypaste_flutter_free_protected_pairing_buffer)>(
      "copypaste_flutter_free_protected_pairing_buffer");
  if (buffer.bytes == nullptr || buffer.len == 0 || buffer.len > kMaximumArtifactBytes ||
      buffer.len > static_cast<size_t>(std::numeric_limits<jsize>::max()) ||
      free_buffer == nullptr) {
    if (buffer.bytes != nullptr && free_buffer != nullptr) free_buffer(buffer);
    return nullptr;
  }
  jbyteArray result = environment->NewByteArray(static_cast<jsize>(buffer.len));
  if (result != nullptr) {
    environment->SetByteArrayRegion(
        result, 0, static_cast<jsize>(buffer.len),
        reinterpret_cast<const jbyte*>(buffer.bytes));
  }
  free_buffer(buffer);
  return result;
}

}  // namespace

extern "C" JNIEXPORT jlong JNICALL
Java_com_copypaste_app_NativeProtectedPairing_begin(
    JNIEnv* environment, jclass, jstring ceremony_id, jstring context_id) {
  const auto function = Resolve<decltype(&copypaste_flutter_begin_protected_pairing_context)>(
      "copypaste_flutter_begin_protected_pairing_context");
  const char* const ceremony = UtfChars(environment, ceremony_id);
  const char* const context = UtfChars(environment, context_id);
  const uint64_t generation = function != nullptr && ceremony != nullptr && context != nullptr
      ? function(ceremony, context) : 0;
  ReleaseUtfChars(environment, ceremony_id, ceremony);
  ReleaseUtfChars(environment, context_id, context);
  return static_cast<jlong>(generation);
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_copypaste_app_NativeProtectedPairing_active(
    JNIEnv* environment, jclass, jstring context_id) {
  const auto function = Resolve<decltype(&copypaste_flutter_protected_pairing_context_active)>(
      "copypaste_flutter_protected_pairing_context_active");
  const char* const context = UtfChars(environment, context_id);
  const bool active = function != nullptr && context != nullptr && function(context);
  ReleaseUtfChars(environment, context_id, context);
  return active;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_copypaste_app_NativeProtectedPairing_detach(
    JNIEnv* environment, jclass, jstring context_id) {
  const auto function = Resolve<decltype(&copypaste_flutter_detach_protected_pairing_context)>(
      "copypaste_flutter_detach_protected_pairing_context");
  const char* const context = UtfChars(environment, context_id);
  const bool detached = function != nullptr && context != nullptr && function(context);
  ReleaseUtfChars(environment, context_id, context);
  return detached;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_copypaste_app_NativeProtectedPairing_cancel(
    JNIEnv* environment, jclass, jstring context_id) {
  const auto function = Resolve<decltype(&copypaste_flutter_cancel_protected_pairing_context)>(
      "copypaste_flutter_cancel_protected_pairing_context");
  const char* const context = UtfChars(environment, context_id);
  const bool cancelled = function != nullptr && context != nullptr && function(context);
  ReleaseUtfChars(environment, context_id, context);
  return cancelled;
}

extern "C" JNIEXPORT jlongArray JNICALL
Java_com_copypaste_app_NativeProtectedPairing_status(
    JNIEnv* environment, jclass, jstring context_id, jlong generation) {
  const auto function = Resolve<decltype(&copypaste_flutter_protected_pairing_status)>(
      "copypaste_flutter_protected_pairing_status");
  const char* const context = UtfChars(environment, context_id);
  copypaste_flutter_protected_pairing_status_value status{};
  const bool success = function != nullptr && context != nullptr &&
      function(context, static_cast<uint64_t>(generation), &status);
  ReleaseUtfChars(environment, context_id, context);
  if (!success || status.expires_in_ms > static_cast<uint64_t>(std::numeric_limits<jlong>::max())) return nullptr;
  const jlong values[] = {static_cast<jlong>(status.state), static_cast<jlong>(status.expires_in_ms)};
  jlongArray result = environment->NewLongArray(2);
  if (result != nullptr) environment->SetLongArrayRegion(result, 0, 2, values);
  return result;
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_com_copypaste_app_NativeProtectedPairing_revealQr(
    JNIEnv* environment, jclass, jstring ceremony_id, jstring context_id, jlong generation) {
  const auto function = Resolve<decltype(&copypaste_flutter_reveal_protected_pairing_qr_png)>(
      "copypaste_flutter_reveal_protected_pairing_qr_png");
  const char* const ceremony = UtfChars(environment, ceremony_id);
  const char* const context = UtfChars(environment, context_id);
  copypaste_flutter_protected_pairing_buffer buffer{};
  const bool success = function != nullptr && ceremony != nullptr && context != nullptr &&
      function(ceremony, context, static_cast<uint64_t>(generation), &buffer);
  ReleaseUtfChars(environment, ceremony_id, ceremony);
  ReleaseUtfChars(environment, context_id, context);
  return success ? CopyAndReleaseBuffer(environment, buffer) : nullptr;
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_com_copypaste_app_NativeProtectedPairing_revealSas(
    JNIEnv* environment, jclass, jstring context_id, jlong generation) {
  const auto function = Resolve<decltype(&copypaste_flutter_reveal_protected_pairing_sas)>(
      "copypaste_flutter_reveal_protected_pairing_sas");
  const char* const context = UtfChars(environment, context_id);
  copypaste_flutter_protected_pairing_buffer buffer{};
  const bool success = function != nullptr && context != nullptr &&
      function(context, static_cast<uint64_t>(generation), &buffer);
  ReleaseUtfChars(environment, context_id, context);
  return success ? CopyAndReleaseBuffer(environment, buffer) : nullptr;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_copypaste_app_NativeProtectedPairing_join(
    JNIEnv* environment, jclass, jstring context_id, jlong generation, jstring code, jstring address) {
  const auto function = Resolve<decltype(&copypaste_flutter_protected_pairing_join)>(
      "copypaste_flutter_protected_pairing_join");
  const char* const context = UtfChars(environment, context_id);
  const char* const code_chars = UtfChars(environment, code);
  const char* const address_chars = UtfChars(environment, address);
  const bool joined = function != nullptr && context != nullptr && code_chars != nullptr && address_chars != nullptr &&
      function(context, static_cast<uint64_t>(generation), code_chars, address_chars);
  ReleaseUtfChars(environment, context_id, context);
  ReleaseUtfChars(environment, code, code_chars);
  ReleaseUtfChars(environment, address, address_chars);
  return joined;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_copypaste_app_NativeProtectedPairing_joinUri(
    JNIEnv* environment, jclass, jstring context_id, jlong generation, jstring uri) {
  const auto function = Resolve<decltype(&copypaste_flutter_protected_pairing_join_uri)>(
      "copypaste_flutter_protected_pairing_join_uri");
  const char* const context = UtfChars(environment, context_id);
  const char* const uri_chars = UtfChars(environment, uri);
  const bool joined = function != nullptr && context != nullptr && uri_chars != nullptr &&
      function(context, static_cast<uint64_t>(generation), uri_chars);
  ReleaseUtfChars(environment, context_id, context);
  ReleaseUtfChars(environment, uri, uri_chars);
  return joined;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_copypaste_app_NativeProtectedPairing_decide(
    JNIEnv* environment, jclass, jstring context_id, jlong generation, jstring sas, jboolean accept) {
  const auto function = Resolve<decltype(&copypaste_flutter_protected_pairing_decide)>(
      "copypaste_flutter_protected_pairing_decide");
  const char* const context = UtfChars(environment, context_id);
  const char* const sas_chars = UtfChars(environment, sas);
  const bool decided = function != nullptr && context != nullptr && sas_chars != nullptr &&
      function(context, static_cast<uint64_t>(generation), sas_chars, accept == JNI_TRUE);
  ReleaseUtfChars(environment, context_id, context);
  ReleaseUtfChars(environment, sas, sas_chars);
  return decided;
}
