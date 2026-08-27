private val pairingBackdropAccessibility by lazy {
    PairingBackdropAccessibility(
        this,
        context.getString(R.string.pairing_dismiss_label),
    )
}

override fun getAccessibilityNodeProvider(): android.view.accessibility.AccessibilityNodeProvider? =
    pairingBackdropAccessibility.wrap(super.getAccessibilityNodeProvider())
