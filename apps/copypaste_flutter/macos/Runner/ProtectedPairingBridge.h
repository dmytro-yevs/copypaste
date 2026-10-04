#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT uint64_t CPPairingBegin(NSString *ceremonyId, NSString *contextId);
FOUNDATION_EXPORT BOOL CPPairingActive(NSString *contextId);
FOUNDATION_EXPORT BOOL CPPairingDetach(NSString *contextId);
FOUNDATION_EXPORT BOOL CPPairingCancel(NSString *contextId);
FOUNDATION_EXPORT BOOL CPPairingStatus(
    NSString *contextId,
    uint64_t generation,
    uint32_t *state,
    uint64_t *expiresInMs);
FOUNDATION_EXPORT NSData * _Nullable CPPairingRevealQr(
    NSString *ceremonyId,
    NSString *contextId,
    uint64_t generation);
FOUNDATION_EXPORT NSData * _Nullable CPPairingRevealSas(
    NSString *contextId,
    uint64_t generation);
FOUNDATION_EXPORT BOOL CPPairingJoin(
    NSString *contextId,
    uint64_t generation,
    NSString *code,
    NSString *address);
FOUNDATION_EXPORT BOOL CPPairingJoinURI(
    NSString *contextId,
    uint64_t generation,
    NSString *uri);
FOUNDATION_EXPORT BOOL CPPairingDecide(
    NSString *contextId,
    uint64_t generation,
    NSString *sas,
    BOOL accept);

NS_ASSUME_NONNULL_END
