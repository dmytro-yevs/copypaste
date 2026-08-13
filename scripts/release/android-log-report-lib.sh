#!/usr/bin/env bash
# Attribute Android crash and shrinker evidence to the CopyPaste process.

CRASH_PATTERNS='FATAL EXCEPTION|UnsatisfiedLinkError|Fatal signal|beginning of crash|did not include required runtime symbols'
R8_PATTERNS="ClassNotFoundException|NoSuchMethodException|NoSuchMethodError|NoSuchFieldException|Didn't find class|UnsatisfiedLinkError"

crash_report() { log_blocks "$1" "$CRASH_PATTERNS" "${2:-}"; }
r8_report()    { log_blocks "$1" "$R8_PATTERNS" "${2:-}"; }

log_blocks() {
    awk -v pat="$2" -v app="$PKG" -v expected_pid="$3" '
        function log_pid(line, fields, count, copy) {
            copy = line
            sub(/^[[:space:]]+/, "", copy)
            count = split(copy, fields, /[[:space:]]+/)
            if (count >= 5 && fields[1] ~ /^[0-9][0-9]-[0-9][0-9]$/ &&
                    fields[2] ~ /^[0-9][0-9]:[0-9][0-9]:/ &&
                    fields[3] ~ /^[0-9]+$/) return fields[3]
            return ""
        }
        function owns(line) {
            line = tolower(line)
            return line ~ app_re || line ~ short_re ||
                   index(line, "libcopypaste_ui_lib.so") > 0
        }
        BEGIN {
            app = tolower(app)
            short = length(app) > 15 ? substr(app, length(app) - 14) : app
            app_re = app
            short_re = short
            gsub(/\./, "[.]", app_re)
            gsub(/\./, "[.]", short_re)
            app_re = "(^|[^[:alnum:]_])" app_re "([^[:alnum:]_]|$)"
            short_re = "(^|[^[:alnum:]_])" short_re "([^[:alnum:]_]|$)"
        }
        { lines[NR] = $0; pids[NR] = log_pid($0) }
        END {
            total = NR
            for (start = 1; start <= total; start++) {
                if (lines[start] !~ pat) continue
                source_pid = pids[start]
                owned = owns(lines[start]) ||
                        (source_pid != "" && source_pid == expected_pid)
                stop = start + 14
                if (stop > total) stop = total
                if (!owned && source_pid != "" && expected_pid == "") {
                    for (line = start + 1; line <= stop; line++) {
                        if (pids[line] == source_pid && owns(lines[line])) {
                            owned = 1
                            break
                        }
                    }
                }
                if (!owned) continue
                print lines[start]
                if (source_pid == "") continue
                for (line = start + 1; line <= stop; line++) {
                    if (pids[line] == source_pid) print lines[line]
                }
            }
        }
    ' "$1"
}

android_log_report_self_test() { # <temporary directory>
    local t="$1"
    group "self-test: crash detection"

    printf 'I ActivityManager: Start proc 1234:com.copypaste.app/u0a123\nI copypaste: hello\n' > "$t/clean.log"
    [[ -z "$(crash_report "$t/clean.log")" ]] \
        && ok "an ordinary log naming the app is not a crash" \
        || bad "an ordinary log naming the app is not a crash" "$(crash_report "$t/clean.log")"

    printf '%s\n' \
        '08-10 04:27:24.300  4242  4242 E AndroidRuntime: FATAL EXCEPTION: main' \
        '08-10 04:27:24.301  4242  4242 E AndroidRuntime: Process: com.copypaste.app, PID: 4242' \
        '08-10 04:27:24.302  4242  4242 E AndroidRuntime: java.lang.RuntimeException' \
        > "$t/ours.log"
    [[ -n "$(crash_report "$t/ours.log")" ]] \
        && ok "a FATAL EXCEPTION naming our process is reported" \
        || bad "a FATAL EXCEPTION naming our process is reported"

    printf '%s\n' \
        '08-10 04:27:24.300    99    99 E AndroidRuntime: FATAL EXCEPTION: main' \
        '08-10 04:27:24.301    99    99 E AndroidRuntime: Process: com.android.settings, PID: 99' \
        > "$t/theirs.log"
    [[ -z "$(crash_report "$t/theirs.log")" ]] \
        && ok "another package's crash is not ours" \
        || bad "another package's crash is not ours" "$(crash_report "$t/theirs.log")"

    printf '08-10 04:27:24.300  4242  4242 F libc: Fatal signal 11 (SIGSEGV) in tid 4242 (m.copypaste.app)\n' > "$t/libc.log"
    [[ -n "$(crash_report "$t/libc.log")" ]] \
        && ok "libc's truncated process name still matches" \
        || bad "libc's truncated process name still matches"

    printf 'E AndroidRuntime: java.lang.UnsatisfiedLinkError: couldnt find "libcopypaste_ui_lib.so"\n' > "$t/link.log"
    [[ -n "$(crash_report "$t/link.log")" ]] \
        && ok "a missing libcopypaste_ui_lib.so is reported" \
        || bad "a missing libcopypaste_ui_lib.so is reported"

    printf '08-10 04:27:24.300  4242  4242 E AndroidRuntime: java.lang.UnsatisfiedLinkError: dlopen failed: library "libcapture_bridge.so" not found\n' > "$t/own-link.log"
    [[ -n "$(crash_report "$t/own-link.log" 4242)" ]] \
        && ok "an app-owned UnsatisfiedLinkError is reported as a crash" \
        || bad "an app-owned UnsatisfiedLinkError is reported as a crash"

    printf '%s\n' \
        '08-10 04:27:24.376  6074  6123 W NativeCrashHandlerImpl: java.lang.UnsatisfiedLinkError: dlopen failed: library "libnative_crash_handler_jni.so" not found' \
        '08-10 04:27:24.376  6074  6123 W NativeCrashHandlerImpl: at java.lang.Runtime.loadLibrary0(Runtime.java:1077)' \
        '08-10 04:27:24.376  6074  6123 W NativeCrashHandlerImpl: at com.google.android.libraries.performance.primes.metrics.crash.NativeCrashHandlerImpl.b(PG:3)' \
        '08-10 04:27:24.383   482   591 I input_focus: [Focus entering cf2e353 com.copypaste.app/com.copypaste.app.MainActivity (server)]' \
        > "$t/interleaved-foreign.log"
    [[ -z "$(crash_report "$t/interleaved-foreign.log" 3832)" ]] \
        && ok "an interleaved app focus event does not own another process's crash" \
        || bad "an interleaved app focus event does not own another process's crash" \
               "$(crash_report "$t/interleaved-foreign.log" 3832)"

    printf '%s\n' \
        '08-10 04:27:24.376   482   591 W NativeCrashHandlerImpl: java.lang.UnsatisfiedLinkError: dlopen failed: library "libnative_crash_handler_jni.so" not found' \
        '08-10 04:27:24.383   482   591 I input_focus: [Focus entering cf2e353 com.copypaste.app/com.copypaste.app.MainActivity (server)]' \
        > "$t/same-pid-foreign-crash.log"
    [[ -z "$(crash_report "$t/same-pid-foreign-crash.log" 5357)" ]] \
        && ok "a same-PID system_server focus does not own a foreign crash" \
        || bad "a same-PID system_server focus does not own a foreign crash" \
               "$(crash_report "$t/same-pid-foreign-crash.log" 5357)"

    printf '%s\n' \
        '08-10 04:27:24.376   482   591 W UnsafeUtil: java.lang.NoSuchMethodException: sun.misc.Unsafe.compareAndSwapObject' \
        '08-10 04:27:24.383   482   591 I input_focus: [Focus entering cf2e353 com.copypaste.app/com.copypaste.app.MainActivity (server)]' \
        > "$t/same-pid-foreign-r8.log"
    [[ -z "$(r8_report "$t/same-pid-foreign-r8.log" 5357)" ]] \
        && ok "a same-PID system_server focus does not own a foreign R8 error" \
        || bad "a same-PID system_server focus does not own a foreign R8 error" \
               "$(r8_report "$t/same-pid-foreign-r8.log" 5357)"

    group "self-test: a stripped symbol"

    printf '08-10 04:27:24.300  4242  4242 E AndroidRuntime: java.lang.ClassNotFoundException: Didn'"'"'t find class "com.copypaste.app.CapturePlugin" on path: DexPathList\n' > "$t/r8.log"
    [[ -n "$(r8_report "$t/r8.log" 4242)" ]] \
        && ok "a missing CapturePlugin class is reported as R8's work" \
        || bad "a missing CapturePlugin class is reported as R8's work"

    [[ -n "$(r8_report "$t/own-link.log" 4242)" ]] \
        && ok "an app-owned UnsatisfiedLinkError is reported as an R8 error" \
        || bad "an app-owned UnsatisfiedLinkError is reported as an R8 error"

    [[ -z "$(r8_report "$t/interleaved-foreign.log" 3832)" ]] \
        && ok "an interleaved app focus event does not own another process's R8 error" \
        || bad "an interleaved app focus event does not own another process's R8 error" \
               "$(r8_report "$t/interleaved-foreign.log" 3832)"

    printf 'W ClassNotFoundException: com.google.android.gms.SomeOptionalThing\n' > "$t/r8-theirs.log"
    [[ -z "$(r8_report "$t/r8-theirs.log")" ]] \
        && ok "another package's missing class is not our stripped symbol" \
        || bad "another package's missing class is not our stripped symbol" "$(r8_report "$t/r8-theirs.log")"

    printf 'I copypaste: everything resolved\n' > "$t/r8-clean.log"
    [[ -z "$(r8_report "$t/r8-clean.log")" ]] \
        && ok "an ordinary log holds no stripped symbol" \
        || bad "an ordinary log holds no stripped symbol"
}
