#ifndef COPYPASTE_FLUTTER_PROTECTED_PAIRING_H
#define COPYPASTE_FLUTTER_PROTECTED_PAIRING_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Both arguments are native-only, NUL-terminated UTF-8 opaque identifiers.
// These functions never accept or return invitation, QR, SAS, address, peer,
// or clipboard content and never allocate caller-owned buffers.
bool copypaste_flutter_attach_protected_pairing_context(
    const char *ceremony_id,
    const char *context_id);

uint64_t copypaste_flutter_begin_protected_pairing_context(
    const char *ceremony_id,
    const char *context_id);

bool copypaste_flutter_protected_pairing_context_active(
    const char *context_id);

bool copypaste_flutter_detach_protected_pairing_context(
    const char *context_id);

bool copypaste_flutter_cancel_protected_pairing_context(
    const char *context_id);

typedef struct {
  uint8_t *bytes;
  size_t len;
} copypaste_flutter_protected_pairing_buffer;
typedef struct {
  uint32_t state;
  uint64_t expires_in_ms;
} copypaste_flutter_protected_pairing_status_value;

bool copypaste_flutter_reveal_protected_pairing_qr_png(
    const char *ceremony_id,
    const char *context_id,
    uint64_t generation,
    copypaste_flutter_protected_pairing_buffer *output);

bool copypaste_flutter_reveal_protected_pairing_sas(
    const char *context_id,
    uint64_t generation,
    copypaste_flutter_protected_pairing_buffer *output);

void copypaste_flutter_free_protected_pairing_buffer(
    copypaste_flutter_protected_pairing_buffer buffer);
bool copypaste_flutter_protected_pairing_join(const char *context_id, uint64_t generation, const char *code, const char *addr);
bool copypaste_flutter_protected_pairing_join_uri(const char *context_id, uint64_t generation, const char *uri);
bool copypaste_flutter_protected_pairing_status(const char *context_id, uint64_t generation, copypaste_flutter_protected_pairing_status_value *output);
bool copypaste_flutter_protected_pairing_decide(const char *context_id, uint64_t generation, const char *sas, bool accept);

#ifdef __cplusplus
}
#endif

#endif
