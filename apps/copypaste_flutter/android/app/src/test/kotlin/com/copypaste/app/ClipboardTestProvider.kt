package com.copypaste.app

import org.robolectric.RuntimeEnvironment

/** Bind FileProvider's cached path strategy to this test's application cache. */
internal fun initializeClipboardTestProvider() {
    val context = RuntimeEnvironment.getApplication()
    // Static getUriForFile can run before the manifest provider is created.
    // Creating it first invalidates the path strategy from a previous test.
    checkNotNull(
        context.contentResolver.acquireContentProviderClient("${context.packageName}.clipboard"),
    ) { "The clipboard FileProvider must be registered in the application manifest" }.close()
}
