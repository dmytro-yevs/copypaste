// Protocol v1 stays installed between application updates. Changing this
// executable deliberately requires a new protocol version and one new grant.
#include <CoreFoundation/CoreFoundation.h>
#include <Security/Security.h>
#include <pwd.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static const char *service = "com.copypaste.daemon";
static const char *account = "device-secret-key";

static bool authorized_parent(void) {
    pid_t parent = getppid();
    SecCodeRef self = NULL, caller = NULL;
    CFDictionaryRef own_info = NULL, caller_info = NULL;
    CFNumberRef pid = CFNumberCreate(NULL, kCFNumberIntType, &parent);
    const void *keys[] = {kSecGuestAttributePid};
    const void *values[] = {pid};
    CFDictionaryRef attributes = CFDictionaryCreate(NULL, keys, values, 1,
        &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    SecCSFlags flags = kSecCSStrictValidate | kSecCSNoNetworkAccess;
    bool allowed = parent > 1 &&
        SecCodeCopySelf(0, &self) == errSecSuccess &&
        SecCodeCopyGuestWithAttributes(NULL, attributes, 0, &caller) == errSecSuccess &&
        SecCodeCheckValidity(self, flags, NULL) == errSecSuccess &&
        SecCodeCheckValidity(caller, flags, NULL) == errSecSuccess &&
        SecCodeCopySigningInformation(self, kSecCSSigningInformation, &own_info) == errSecSuccess &&
        SecCodeCopySigningInformation(caller, kSecCSSigningInformation, &caller_info) == errSecSuccess;
    if (allowed) {
        CFTypeRef identifier = CFDictionaryGetValue(caller_info, kSecCodeInfoIdentifier);
        CFArrayRef ours = CFDictionaryGetValue(own_info, kSecCodeInfoCertificates);
        CFArrayRef theirs = CFDictionaryGetValue(caller_info, kSecCodeInfoCertificates);
        allowed = identifier && CFEqual(identifier, CFSTR("com.copypaste.copypaste-daemon")) &&
            ours && theirs && CFArrayGetCount(ours) > 0 && CFArrayGetCount(theirs) > 0;
        if (allowed) {
            CFDataRef a = SecCertificateCopyData((SecCertificateRef)CFArrayGetValueAtIndex(ours, 0));
            CFDataRef b = SecCertificateCopyData((SecCertificateRef)CFArrayGetValueAtIndex(theirs, 0));
            allowed = a && b && CFEqual(a, b) && getppid() == parent;
            if (a) CFRelease(a);
            if (b) CFRelease(b);
        }
    }
    if (own_info) CFRelease(own_info);
    if (caller_info) CFRelease(caller_info);
    if (self) CFRelease(self);
    if (caller) CFRelease(caller);
    CFRelease(attributes);
    CFRelease(pid);
    return allowed;
}

static int transfer(bool create) {
    struct stat output;
    if (fstat(STDOUT_FILENO, &output) || !S_ISFIFO(output.st_mode) || !authorized_parent()) return 5;
    SecKeychainRef keychain = NULL;
#ifdef COPYPASTE_KEYCHAIN_TESTING
    // Fixture builds can only use an explicitly supplied disposable keychain.
    const char *path = getenv("COPYPASTE_KEYCHAIN_TEST_PATH");
    if (!path || SecKeychainOpen(path, &keychain) != errSecSuccess) return 2;
    SecKeychainSetUserInteractionAllowed(false);
#endif
    OSStatus status;
    int result = 2;
    if (create) {
        unsigned char secret[32];
        size_t count = 0;
        while (count < sizeof(secret)) {
            ssize_t n = read(STDIN_FILENO, secret + count, sizeof(secret) - count);
            if (n <= 0) break;
            count += (size_t)n;
        }
        unsigned char extra;
        if (count == sizeof(secret) && read(STDIN_FILENO, &extra, 1) == 0) {
            // Add never replaces a key that appeared after the absence check.
            status = SecKeychainAddGenericPassword(keychain, (UInt32)strlen(service), service,
                (UInt32)strlen(account), account, sizeof(secret), secret, NULL);
            result = status == errSecSuccess ? 0 : 2;
        }
        memset_s(secret, sizeof(secret), 0, sizeof(secret));
    } else {
        UInt32 length = 0;
        void *secret = NULL;
        status = SecKeychainFindGenericPassword(keychain, (UInt32)strlen(service), service,
            (UInt32)strlen(account), account, &length, &secret, NULL);
        if (status == errSecItemNotFound) result = 3;
        if (status == errSecSuccess) {
            result = length == 32 ? 5 : 4;
            if (length == 32 && authorized_parent()) {
                size_t count = 0;
                while (count < length) {
                    ssize_t n = write(STDOUT_FILENO, (unsigned char *)secret + count, length - count);
                    if (n <= 0) break;
                    count += (size_t)n;
                }
                result = count == length ? 0 : 2;
            }
            if (secret) {
                memset_s(secret, length, 0, length);
                SecKeychainItemFreeContent(NULL, secret);
            }
        }
    }
    if (keychain) CFRelease(keychain);
    return result;
}

int main(int argc, char **argv) {
    // Bound an OS-mediated prompt even if the daemon exits during the call.
    alarm(8);
    if (argc != 2) return 2;
    if (!strcmp(argv[1], "support-directory")) {
#ifdef COPYPASTE_KEYCHAIN_TESTING
        const char *fixture = getenv("COPYPASTE_KEYCHAIN_TEST_SUPPORT_DIR");
        if (fixture) {
            if (fixture[0] != '/') return 2;
            printf("%s\n", fixture);
            return 0;
        }
#endif
        // HOME is deliberately temporary inside Homebrew's install sandbox.
        struct passwd *user = getpwuid(getuid());
        if (!user || !user->pw_dir || user->pw_dir[0] != '/') return 2;
        printf("%s/Library/Application Support/com.copypaste.CopyPaste\n", user->pw_dir);
        return 0;
    }
    if (!strcmp(argv[1], "load")) return transfer(false);
    if (!strcmp(argv[1], "create")) return transfer(true);
    return 2;
}
