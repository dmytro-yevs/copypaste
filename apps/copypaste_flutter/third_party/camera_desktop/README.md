# Camera desktop compatibility patch

This is camera_desktop 2.0.0 from pub.dev, archive SHA-256
`280ce2b335f3a6d2bd4345c9906697d86de40065d52cd6090df28a8b52b32304`.
The original license is preserved in LICENSE.

The Linux callback definitions omit two unused parameter names so the plugin
compiles with the application's existing warnings-as-errors policy. The callback
signatures and implementation behavior are unchanged. Other platform sources
are copied unchanged. The build metadata suffix identifies this local patch.
