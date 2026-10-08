This update improves module access setup and build performance.

- Open SMS module access setup in a drawer and refresh permission state when the application resumes.
- Reuse build dependency caches across CI and release runs, and avoid the slow Windows Flutter SDK archive.

Optional modules are selected and installed from the first-party marketplace in Settings. OCR engines and models are not bundled with the application.

Cloud Sync is not part of this release.
