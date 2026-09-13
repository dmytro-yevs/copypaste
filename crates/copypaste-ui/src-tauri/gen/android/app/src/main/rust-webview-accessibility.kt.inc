private val pairingBackdropAccessibility by lazy {
    PairingBackdropAccessibility(
        this,
        context.getString(R.string.pairing_dismiss_label),
    )
}

override fun getAccessibilityNodeProvider(): android.view.accessibility.AccessibilityNodeProvider? =
    pairingBackdropAccessibility.wrap(super.getAccessibilityNodeProvider())

override fun onCreateInputConnection(
    outAttrs: android.view.inputmethod.EditorInfo,
): android.view.inputmethod.InputConnection? {
    val connection = super.onCreateInputConnection(outAttrs)
    outAttrs.imeOptions = outAttrs.imeOptions or
        android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI or
        android.view.inputmethod.EditorInfo.IME_FLAG_NO_FULLSCREEN
    return connection
}
