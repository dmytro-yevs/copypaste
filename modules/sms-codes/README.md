# SMS Codes

Optional first-party Android module. It recognizes login, verification, PIN,
and transaction codes locally in new incoming SMS messages. It preserves
leading zeroes and letter case, removes grouping spaces or hyphens, and skips
ambiguous messages, promotions, delivery codes, dates, phone numbers and amounts.
The SMS body stays in the invocation only. Only the extracted code enters the
system clipboard, encrypted History, and the existing configured sync source.

Install the signed target-specific `.cpmodule` in Settings > Modules. New event
modules start disabled. Open **Set up SMS access**, allow notifications, and
choose **Shizuku** or **ADB** in the shared Android access setup. Apply the
grants, then enable the module. Access updates while the setup dialog is open.
The module requires `READ_SMS` and `RECEIVE_SMS`. On systems exposing `READ_OTP_SMS`, setup
also grants that app-op to receive protected OTP messages immediately. If the
installer has not allowlisted the hard-restricted SMS permissions, grant
verification fails and the module stays disabled; reinstall the APK through an
installer that permits SMS access, such as `adb install -r`, before setup.
Shizuku is used only for setup, not ongoing SMS reads.

The Android host observes new inbox rows in a foreground service and receives
SMS and boot broadcasts. Its private watermark contains row IDs and activation
times, never message bodies. It does not import old inbox messages. Private
mode blocks provider reads and code publication. Clipboard application
exclusions apply to clipboard sources; SMS is a separate known OS source.
Disable or remove the module to stop event admission and the service. Android
force-stop, revoked permissions, or OS restrictions can prevent reception until
the application is reopened. This module does not make CopyPaste the default
SMS application and does not send or delete SMS messages.

## Build

```sh
cargo test --manifest-path modules/sms-codes/Cargo.toml --locked
python3 scripts/modules/build-android-module.py \
  --module-dir modules/sms-codes --library-name libcopypaste_module_sms_codes.so \
  --architecture aarch64 --ndk "$ANDROID_HOME/ndk/29.0.13846066"
```

The **Build SMS Codes module** workflow builds all shipped Android ABIs and
signs separate packages with the existing release identity. It uploads build
artifacts only. Marketplace publication follows native qualification of the
exact signed package and the host APK. Compilation and host tests do not prove
physical-device SMS reception, clipboard writing, reboot recovery, protected
OTP access, or cross-device delivery.

Android sources for protected OTP access:
[SmsManager](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android16-qpr2-release/telephony/java/android/telephony/SmsManager.java),
[SmsProvider](https://android.googlesource.com/platform/packages/providers/TelephonyProvider/+/refs/heads/android16-qpr2-release/src/com/android/providers/telephony/SmsProvider.java).
