# CopyPaste 1.0.1

This update improves clipboard capture, search, and desktop behavior.

- Preserve word boundaries when searching text containing URLs or punctuation.
- Improve clipboard source tracking, exclusion handling, and private-mode transitions.
- Capture supported local files through native macOS file URLs and preserve their contents during storage retries.
- Improve Android capture ownership across background and foreground lifecycle changes.
- Correct desktop shortcut registration, popup result handling, and Quick Paste target ownership.
- Allow macOS shutdown without waiting for a Flutter termination reply.
- Correct local and remote device labels and improve Settings validation messages.

Application exclusions on macOS use observed app activity; background copies can bypass them. Empty files and multiple-file clipboard selections remain unsupported.

Cloud Sync is not part of CopyPaste 1.0.1.
