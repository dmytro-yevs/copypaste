import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_es.dart';
import 'app_localizations_fr.dart';
import 'app_localizations_it.dart';
import 'app_localizations_ja.dart';
import 'app_localizations_ko.dart';
import 'app_localizations_pl.dart';
import 'app_localizations_pt.dart';
import 'app_localizations_ru.dart';
import 'app_localizations_tr.dart';
import 'app_localizations_uk.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('de'),
    Locale('en'),
    Locale('es'),
    Locale('fr'),
    Locale('it'),
    Locale('ja'),
    Locale('ko'),
    Locale('pl'),
    Locale('pt'),
    Locale('ru'),
    Locale('tr'),
    Locale('uk'),
    Locale('zh'),
  ];

  /// Button that dismisses a dialog or stops an action without applying it.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get commonCancel;

  /// Button or tooltip that closes a panel, drawer, or dialog.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get commonClose;

  /// Button that finishes a flow and closes it.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get commonDone;

  /// Button that repeats a failed action.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get commonRetry;

  /// Button that repeats a failed action.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get commonTryAgain;

  /// Verb. Copies a clipboard item or text to the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get commonCopy;

  /// Verb. Permanently removes an item.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get commonDelete;

  /// Verb. Saves changes.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get commonSave;

  /// Button that moves to the next step.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get commonContinue;

  /// Button that returns to the previous step or screen.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get commonBack;

  /// Button that opens the operating system settings page relevant to the current problem.
  ///
  /// In en, this message translates to:
  /// **'Open settings'**
  String get commonOpenSettings;

  /// Title shown while content is loading.
  ///
  /// In en, this message translates to:
  /// **'Loading'**
  String get commonLoading;

  /// Button that applies selected options.
  ///
  /// In en, this message translates to:
  /// **'Apply'**
  String get commonApply;

  /// Verb. Clears a field, a filter, or a list.
  ///
  /// In en, this message translates to:
  /// **'Clear'**
  String get commonClear;

  /// Verb. Keeps a clipboard item at the top of the history so it is never removed automatically.
  ///
  /// In en, this message translates to:
  /// **'Pin'**
  String get commonPin;

  /// Verb. Turns a feature on.
  ///
  /// In en, this message translates to:
  /// **'Enable'**
  String get commonEnable;

  /// Placeholder of an empty multi-choice field, whatever it lists (languages, content kinds, time windows).
  ///
  /// In en, this message translates to:
  /// **'Choose'**
  String get commonChoose;

  /// Settings row title for choosing the interface language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get settingsLanguageTitle;

  /// Settings row description for choosing the interface language. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Choose the language of the CopyPaste interface.'**
  String get settingsLanguageDescription;

  /// Language option that follows the operating system language.
  ///
  /// In en, this message translates to:
  /// **'System default'**
  String get settingsLanguageSystem;

  /// Space-separated search keywords that find the language setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'language locale translation interface'**
  String get settingsLanguageKeywords;

  /// Settings error message shown when turning automatic start at login on or off fails.
  ///
  /// In en, this message translates to:
  /// **'Start at login could not be updated. Try again.'**
  String get settingsRuntimeLaunchAtLoginUpdateFailed;

  /// Settings error message shown when the operating system's startup (login items) settings page cannot be opened.
  ///
  /// In en, this message translates to:
  /// **'System startup settings could not be opened.'**
  String get settingsRuntimeStartupSettingsOpenFailed;

  /// Settings error message shown when re-reading the current settings from the background service fails.
  ///
  /// In en, this message translates to:
  /// **'Settings could not be refreshed. Try again.'**
  String get settingsRuntimeRefreshFailed;

  /// Settings error message shown when the user adds an application that is already in the list of applications excluded from clipboard capture.
  ///
  /// In en, this message translates to:
  /// **'That app is already excluded.'**
  String get settingsRuntimeAppAlreadyExcluded;

  /// Generic settings error message shown when saving a setting or running a settings action fails.
  ///
  /// In en, this message translates to:
  /// **'The requested change could not be completed.'**
  String get settingsRuntimeChangeFailed;

  /// Settings error message shown when clipboard capture cannot resume because required permissions are still missing.
  ///
  /// In en, this message translates to:
  /// **'Clipboard capture permissions must be completed before capture can resume.'**
  String get settingsRuntimeCapturePermissionsIncomplete;

  /// Settings error message on Android shown when capture was paused but the background capture service did not confirm it stopped.
  ///
  /// In en, this message translates to:
  /// **'Clipboard capture is paused. The Android capture service did not confirm that it stopped.'**
  String get settingsRuntimeCaptureStopUnconfirmed;

  /// Settings error message shown when the user enables notifications for copied items but denies the system notification permission.
  ///
  /// In en, this message translates to:
  /// **'Notification permission is required to enable capture notifications.'**
  String get settingsRuntimeNotificationPermissionRequired;

  /// Settings warning shown when the saved settings file is unreadable and safe default values are used instead. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not read saved settings. Protective values are active; review and save your preferences to repair them.'**
  String get settingsRuntimeSavedSettingsUnreadable;

  /// Settings warning shown when some saved settings were unreadable and were replaced with safe default values. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste recovered unreadable settings with protective values. Review and save your preferences to repair them.'**
  String get settingsRuntimeSettingsRecovered;

  /// Settings error message shown when an application identifier added to the exclusion list is empty or too long.
  ///
  /// In en, this message translates to:
  /// **'App identifiers must be non-empty and 256 bytes or fewer.'**
  String get settingsRuntimeExcludedAppIdInvalid;

  /// Settings error message shown when the exclusion list already holds the maximum number of applications.
  ///
  /// In en, this message translates to:
  /// **'You can exclude up to 256 apps.'**
  String get settingsRuntimeExcludedAppsLimit;

  /// Settings error message shown when turning automatic pasting after selection on or off fails.
  ///
  /// In en, this message translates to:
  /// **'Auto-paste could not be updated.'**
  String get settingsRuntimeAutoPasteUpdateFailed;

  /// Settings error message shown when the chosen global keyboard shortcut cannot be registered with the system.
  ///
  /// In en, this message translates to:
  /// **'That shortcut could not be registered.'**
  String get settingsRuntimeShortcutRegisterFailed;

  /// Settings error message shown when the current global keyboard shortcut cannot be temporarily disabled before recording a new one.
  ///
  /// In en, this message translates to:
  /// **'The current shortcut could not be paused.'**
  String get settingsRuntimeShortcutPauseFailed;

  /// Settings error message shown when the previous global keyboard shortcut cannot be registered again after recording is cancelled.
  ///
  /// In en, this message translates to:
  /// **'The shortcut could not be restored.'**
  String get settingsRuntimeShortcutRestoreFailed;

  /// Error message shown when the Quick Paste window cannot be opened from its keyboard shortcut. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste could not be opened.'**
  String get settingsRuntimeQuickPasteOpenFailed;

  /// Settings error message shown when the system Accessibility permission prompt (needed for automatic pasting) cannot be shown.
  ///
  /// In en, this message translates to:
  /// **'Accessibility permission could not be requested.'**
  String get settingsRuntimeAccessibilityRequestFailed;

  /// Settings error message on Linux shown when the status of desktop integration (shortcuts, clipboard input) cannot be read.
  ///
  /// In en, this message translates to:
  /// **'Linux integration status could not be refreshed.'**
  String get settingsRuntimeLinuxIntegrationRefreshFailed;

  /// Settings error message on Linux shown when the user declines the system permission that lets CopyPaste paste into other apps.
  ///
  /// In en, this message translates to:
  /// **'Clipboard input permission was not granted.'**
  String get settingsRuntimeClipboardInputNotGranted;

  /// Settings error message on Linux shown when the system permission prompt for pasting into other apps cannot be shown.
  ///
  /// In en, this message translates to:
  /// **'Clipboard input permission could not be requested.'**
  String get settingsRuntimeClipboardInputRequestFailed;

  /// Settings error message on Linux shown when the setup of the desktop integration companion cannot be opened.
  ///
  /// In en, this message translates to:
  /// **'Desktop integration setup could not be opened.'**
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed;

  /// File type name shown in the system save dialog when exporting text history as a JSON file. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste text history'**
  String get settingsRuntimeTextHistoryFileType;

  /// File type name shown in the system file dialogs when creating or restoring an encrypted history backup. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste encrypted backup'**
  String get settingsRuntimeEncryptedBackupFileType;

  /// Noun. Text of the system notification shown after an image is copied, when its dimensions are unknown.
  ///
  /// In en, this message translates to:
  /// **'Image'**
  String get settingsRuntimeNotificationImage;

  /// Text of the system notification shown after an image is copied, with its width and height in pixels.
  ///
  /// In en, this message translates to:
  /// **'Image · {width} × {height}'**
  String settingsRuntimeNotificationImageSize(int width, int height);

  /// Noun. Text of the system notification shown after a file is copied, when its name is unknown.
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get settingsRuntimeNotificationFile;

  /// Update error message shown when the state of an update installation started earlier cannot be restored after relaunch. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not restore the update installation.'**
  String get updateRestoreInstallationFailed;

  /// Update error message shown when the installed Linux package type does not support automatic updates.
  ///
  /// In en, this message translates to:
  /// **'This Linux installation cannot update automatically.'**
  String get updateLinuxAutomaticUnsupported;

  /// Update status message shown when the newest release has no signed macOS package to install automatically.
  ///
  /// In en, this message translates to:
  /// **'This release has no signed macOS update package.'**
  String get updateMacosPackageMissing;

  /// Update error message shown when the version number of the installed app cannot be read. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'The installed CopyPaste version is invalid.'**
  String get updateInstalledVersionInvalid;

  /// Update error message shown when there is no network connection to GitHub, where releases are published. CopyPaste and GitHub are product names.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not reach GitHub to check for updates.'**
  String get updateCheckUnreachable;

  /// Update error message shown when checking for a new version takes too long.
  ///
  /// In en, this message translates to:
  /// **'The update check timed out.'**
  String get updateCheckTimedOut;

  /// Update error message shown when the device cannot run the update check. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not check for updates on this device.'**
  String get updateCheckDeviceFailed;

  /// Generic update error message shown when checking for a new version fails. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not check for updates.'**
  String get updateCheckFailed;

  /// Update error message shown when downloading the update package fails. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not download the update.'**
  String get updateDownloadFailed;

  /// Update error message shown when downloading the update package takes too long.
  ///
  /// In en, this message translates to:
  /// **'The update download timed out.'**
  String get updateDownloadTimedOut;

  /// Generic update error message shown when installing the update fails. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not install the update.'**
  String get updateInstallFailed;

  /// Update status message on Android shown after the system app installer opens.
  ///
  /// In en, this message translates to:
  /// **'Continue in the Android system installer.'**
  String get updateContinueInAndroidInstaller;

  /// Update status message on Linux shown after the system package manager opens to install the update.
  ///
  /// In en, this message translates to:
  /// **'Continue in your system package manager.'**
  String get updateContinueInPackageManager;

  /// Update status message shown on desktop while the update installer is launching.
  ///
  /// In en, this message translates to:
  /// **'The installer is starting.'**
  String get updateInstallerStarting;

  /// Update status message on Android asking the user to allow the app to install other apps in system settings. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste to install apps in Android settings, then continue.'**
  String get updatePermissionRequiredAndroid;

  /// Update status message on Linux asking the user to sign in to the system package manager to install the update.
  ///
  /// In en, this message translates to:
  /// **'Authenticate in your system package manager to install this update.'**
  String get updatePermissionRequiredLinux;

  /// Update status message asking the user to grant the system permission needed to install the update. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste to install this update, then continue.'**
  String get updatePermissionRequired;

  /// Update status message shown after the update is installed and the app must restart. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'The update is installed. Restart CopyPaste to use it.'**
  String get updateRestartRequired;

  /// Update status message shown while the app restarts after an update. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Restarting CopyPaste.'**
  String get updateRestarting;

  /// Update error message shown when the app fails to restart after an update. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not restart. Try again.'**
  String get updateRestartFailed;

  /// Update error message shown when the release page on GitHub cannot be opened in the browser. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not open the release page.'**
  String get updateReleasePageOpenFailed;

  /// Update error message shown when the downloaded file is not a valid app package. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'The downloaded update is not a valid CopyPaste package.'**
  String get updatePackageInvalid;

  /// Update error message shown when the code signature of the downloaded package is not valid.
  ///
  /// In en, this message translates to:
  /// **'The downloaded update has an invalid signature.'**
  String get updateSignatureInvalid;

  /// Update error message shown when the downloaded package would install an older or the same version.
  ///
  /// In en, this message translates to:
  /// **'The downloaded package is not newer than this version.'**
  String get updateDowngradeRefused;

  /// Update error message on macOS shown when the app can only be updated through the Homebrew package manager. CopyPaste and Homebrew are product names.
  ///
  /// In en, this message translates to:
  /// **'Install CopyPaste with Homebrew to update it here.'**
  String get updateHomebrewRequired;

  /// Update error message shown when the user or the system cancels the update installation.
  ///
  /// In en, this message translates to:
  /// **'The update installation was cancelled.'**
  String get updateInstallationCancelled;

  /// Update error message on Android shown next to the Check again action when device protection refuses to install the update. Play Protect and Auto Blocker are product names.
  ///
  /// In en, this message translates to:
  /// **'Device protection (Play Protect, Auto Blocker) blocked this update. Allow it there, then check again.'**
  String get updateInstallationBlocked;

  /// Update error message on Android shown when the update package conflicts with the installed app, for example a different signature.
  ///
  /// In en, this message translates to:
  /// **'The update conflicts with the installed app.'**
  String get updateInstallationConflict;

  /// Update error message shown when the update package does not support this device.
  ///
  /// In en, this message translates to:
  /// **'The update is incompatible with this device.'**
  String get updateInstallationIncompatible;

  /// Update error message shown when there is not enough free storage to install the update.
  ///
  /// In en, this message translates to:
  /// **'Free up storage to install the update.'**
  String get updateInstallationStorage;

  /// Update error message shown when the update installation stops before finishing.
  ///
  /// In en, this message translates to:
  /// **'The update installation was interrupted. Try again.'**
  String get updateInstallationInterrupted;

  /// Update error message shown when another update installation is already running.
  ///
  /// In en, this message translates to:
  /// **'An update installation is already in progress.'**
  String get updateInstallationBusy;

  /// Update error message shown when the information describing the update package is invalid.
  ///
  /// In en, this message translates to:
  /// **'The update package details are invalid.'**
  String get updatePackageDetailsInvalid;

  /// Update error message shown when the checksum of the downloaded package does not match the expected value.
  ///
  /// In en, this message translates to:
  /// **'The downloaded update failed its integrity check.'**
  String get updateIntegrityCheckFailed;

  /// Update error message shown when the operating system installer for the update cannot be launched. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not start the system installer.'**
  String get updateInstallerLaunchFailed;

  /// Update error message on macOS shown when the app runs from a temporary location and cannot replace itself. CopyPaste is the product name; Applications is the macOS folder name.
  ///
  /// In en, this message translates to:
  /// **'Move CopyPaste to the Applications folder and open it from there to update.'**
  String get updateInstallLocationTranslocated;

  /// Update error message on macOS shown when the app's folder is not writable, so the user must update manually from the release page. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste cannot replace itself in its current folder. Use the release page to update.'**
  String get updateInstallLocationUnwritable;

  /// Update error message on macOS shown when the system blocks replacing the app. App Management is the macOS privacy settings section name. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'macOS blocked replacing CopyPaste. Allow it in App Management settings or use the release page.'**
  String get updateReplacementBlocked;

  /// Update error message shown when the current platform installs updates without downloading a package.
  ///
  /// In en, this message translates to:
  /// **'This platform does not use a downloadable update package.'**
  String get updatePackageNotDownloadable;

  /// Update error message shown when the published size of the update package is missing or out of range.
  ///
  /// In en, this message translates to:
  /// **'The update package size is invalid.'**
  String get updatePackageSizeInvalid;

  /// Update error message shown when the downloaded package fails verification against the app's own update signature.
  ///
  /// In en, this message translates to:
  /// **'The downloaded update has an invalid updater signature.'**
  String get updateUpdaterSignatureInvalid;

  /// Update error message shown when the update package address is not on the trusted GitHub release host.
  ///
  /// In en, this message translates to:
  /// **'The update package URL is not trusted.'**
  String get updatePackageUrlUntrusted;

  /// Update error message shown when the update download redirects to a host other than GitHub. GitHub is a product name.
  ///
  /// In en, this message translates to:
  /// **'The update download left GitHub.'**
  String get updateDownloadLeftGitHub;

  /// Update error message shown when the update download is redirected too many times.
  ///
  /// In en, this message translates to:
  /// **'The update download used too many redirects.'**
  String get updateDownloadTooManyRedirects;

  /// Update error message shown when the update package exceeds the allowed size.
  ///
  /// In en, this message translates to:
  /// **'The update package is too large.'**
  String get updatePackageTooLarge;

  /// Update error message shown when the published size of the update signature file is missing or out of range.
  ///
  /// In en, this message translates to:
  /// **'The updater signature size is invalid.'**
  String get updateSignatureSizeInvalid;

  /// Update error message shown when the release information from GitHub cannot be parsed. GitHub is a product name.
  ///
  /// In en, this message translates to:
  /// **'GitHub returned invalid update metadata.'**
  String get updateMetadataInvalid;

  /// Update error message shown when the release information downloaded from GitHub exceeds the allowed size.
  ///
  /// In en, this message translates to:
  /// **'The update metadata is too large.'**
  String get updateMetadataTooLarge;

  /// Quick Paste toast title shown when an action on a clip (copy, pin, delete) fails.
  ///
  /// In en, this message translates to:
  /// **'Action failed'**
  String get quickPasteActionFailedTitle;

  /// Screen reader label of the logo button in the Quick Paste popup that opens the main CopyPaste window. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Open CopyPaste'**
  String get quickPasteOpenAppSemantics;

  /// Screen reader label of the Quick Paste header button that shows or hides the clip details panel beside the list.
  ///
  /// In en, this message translates to:
  /// **'Show or hide clip details'**
  String get quickPasteToggleClipDetails;

  /// Placeholder of the search field at the top of the Quick Paste popup.
  ///
  /// In en, this message translates to:
  /// **'Type to search…'**
  String get quickPasteSearchPlaceholder;

  /// Error title shown in the Quick Paste popup when the clipboard history cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'History is unavailable'**
  String get quickPasteHistoryUnavailable;

  /// Empty-state title in the Quick Paste popup when no clipboard items match the search.
  ///
  /// In en, this message translates to:
  /// **'No clips found'**
  String get quickPasteNoClipsFound;

  /// Button at the end of the Quick Paste list that loads older clipboard items.
  ///
  /// In en, this message translates to:
  /// **'Load more'**
  String get quickPasteLoadMore;

  /// Status shown next to a clip in Quick Paste when its file download failed. Adjective.
  ///
  /// In en, this message translates to:
  /// **'Failed'**
  String get quickPasteTransferFailed;

  /// Error shown in place of a hidden (secret) clip in Quick Paste when its content cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Clip unavailable'**
  String get quickPasteClipUnavailable;

  /// Name shown for a file clip in Quick Paste when the file has no name. Noun.
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get quickPasteFileFallbackName;

  /// Shown under a file clip in Quick Paste when the file location is unknown.
  ///
  /// In en, this message translates to:
  /// **'Path unavailable'**
  String get quickPastePathUnavailable;

  /// Menu item in the Quick Paste footer that opens the application settings.
  ///
  /// In en, this message translates to:
  /// **'Settings…'**
  String get quickPasteSettings;

  /// Menu item in the Quick Paste footer that shows information about the application.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get quickPasteAbout;

  /// Menu item in the Quick Paste footer that quits the application. Verb.
  ///
  /// In en, this message translates to:
  /// **'Quit'**
  String get quickPasteQuit;

  /// Menu item in the Quick Paste footer that asks to clear the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Clear history…'**
  String get quickPasteClearHistory;

  /// Title of the Quick Paste confirmation dialog that deletes all unpinned clipboard items.
  ///
  /// In en, this message translates to:
  /// **'Clear history?'**
  String get quickPasteClearHistoryTitle;

  /// Body of the Quick Paste dialog that confirms clearing the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Pinned clips will stay in your history.'**
  String get quickPasteClearHistoryMessage;

  /// Error shown when clearing the clipboard history from Quick Paste fails.
  ///
  /// In en, this message translates to:
  /// **'History could not be deleted.'**
  String get quickPasteClearHistoryFailed;

  /// Body of the About dialog opened from Quick Paste. Describes the product; operating system names stay unchanged.
  ///
  /// In en, this message translates to:
  /// **'Private clipboard history across macOS, Android, Windows, and Linux.'**
  String get quickPasteAboutDescription;

  /// Title of the transfers drawer, and tooltip and accessibility label of the icon-only header button that opens it. Covers file imports, downloads and outgoing transfers.
  ///
  /// In en, this message translates to:
  /// **'Transfers'**
  String get transfersTitle;

  /// Tooltip of the transfers header button while files transfer: number of active transfers, then bytes transferred out of total (already formatted sizes such as 2.1 MB).
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 active transfer} other{{count} active transfers}} · {written} / {total}'**
  String transfersActiveTooltip(int count, String written, String total);

  /// Subtitle of the downloads and transfers drawer: number of transfers in progress.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, other{{count} active}}'**
  String transfersActiveCount(int count);

  /// Button in the transfers drawer footer that returns to the most recent transfer records.
  ///
  /// In en, this message translates to:
  /// **'Latest'**
  String get transfersShowLatest;

  /// Button in the transfers drawer footer that loads older transfer records.
  ///
  /// In en, this message translates to:
  /// **'Earlier'**
  String get transfersShowEarlier;

  /// Button in the transfers drawer footer that removes every transfer record that is no longer active: completed, failed, cancelled and interrupted.
  ///
  /// In en, this message translates to:
  /// **'Clear history'**
  String get transfersClearCompletedHistory;

  /// Empty-state title of the transfers drawer.
  ///
  /// In en, this message translates to:
  /// **'No transfers'**
  String get transfersEmptyTitle;

  /// Empty-state message of the transfers drawer.
  ///
  /// In en, this message translates to:
  /// **'File imports, downloads and outgoing transfers appear here.'**
  String get transfersEmptyMessage;

  /// Transfer speed shown on an active transfer: a formatted size per second.
  ///
  /// In en, this message translates to:
  /// **'{size}/s'**
  String transfersSpeed(String size);

  /// Bytes transferred out of the total size, both already formatted.
  ///
  /// In en, this message translates to:
  /// **'{written} / {total}'**
  String transfersBytesProgress(String written, String total);

  /// Transfer progress as a whole-number percentage.
  ///
  /// In en, this message translates to:
  /// **'{percent}%'**
  String transfersProgressPercent(int percent);

  /// Status line of a file import: its phase (for example Adding) followed by the formatted file size.
  ///
  /// In en, this message translates to:
  /// **'{status} · {size}'**
  String transfersStatusSize(String status, String size);

  /// Status line of a download or outgoing transfer: its phase (for example Downloading) followed by bytes transferred out of the total.
  ///
  /// In en, this message translates to:
  /// **'{status} · {written} / {total}'**
  String transfersStatusProgress(String status, String written, String total);

  /// Tooltip of the button that removes one finished transfer record from the transfers list.
  ///
  /// In en, this message translates to:
  /// **'Remove from history'**
  String get transfersRemoveFromHistory;

  /// Tooltip of the button that stops an active file download.
  ///
  /// In en, this message translates to:
  /// **'Cancel download'**
  String get transfersCancelDownload;

  /// Button on a completed transfer that reveals the saved file in the file manager.
  ///
  /// In en, this message translates to:
  /// **'Show file'**
  String get transfersShowFile;

  /// Shown in the transfers list in place of the file name when the file belongs to a secret clip.
  ///
  /// In en, this message translates to:
  /// **'Hidden file'**
  String get transfersHiddenFile;

  /// Status of a file currently being sent to another device.
  ///
  /// In en, this message translates to:
  /// **'Sending'**
  String get transfersPhaseSending;

  /// Status of a file currently being downloaded from another device.
  ///
  /// In en, this message translates to:
  /// **'Downloading'**
  String get transfersPhaseDownloading;

  /// Status of a local file currently being added to the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Adding'**
  String get transfersPhaseAdding;

  /// Status of a local file that was added to the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Added'**
  String get transfersPhaseAdded;

  /// Status of a file that was sent to another device.
  ///
  /// In en, this message translates to:
  /// **'Sent'**
  String get transfersPhaseSent;

  /// Status of a file download that finished.
  ///
  /// In en, this message translates to:
  /// **'Completed'**
  String get transfersPhaseCompleted;

  /// Status of a file transfer that was cancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get transfersPhaseCancelled;

  /// Status of a file transfer that stopped unexpectedly, for example when a device disconnected.
  ///
  /// In en, this message translates to:
  /// **'Interrupted'**
  String get transfersPhaseInterrupted;

  /// Status of a file transfer that failed.
  ///
  /// In en, this message translates to:
  /// **'Failed'**
  String get transfersPhaseFailed;

  /// Error in the transfers drawer when live transfer progress stops arriving.
  ///
  /// In en, this message translates to:
  /// **'Transfer updates are unavailable. Reopen to refresh.'**
  String get transfersUpdatesUnavailable;

  /// Error in the transfers drawer when the latest state of a transfer cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Transfer updates could not be loaded.'**
  String get transfersUpdatesLoadFailed;

  /// Error when restarting a failed file download fails.
  ///
  /// In en, this message translates to:
  /// **'This file could not be retried.'**
  String get transfersRetryFailed;

  /// Error when removing finished transfer records fails.
  ///
  /// In en, this message translates to:
  /// **'Transfer history could not be cleared.'**
  String get transfersClearHistoryFailed;

  /// Error when removing one finished transfer record fails.
  ///
  /// In en, this message translates to:
  /// **'This history record could not be removed.'**
  String get transfersRemoveRecordFailed;

  /// Error when stopping an active file transfer fails.
  ///
  /// In en, this message translates to:
  /// **'The transfer could not be cancelled. It may still be running.'**
  String get transfersCancelFailed;

  /// Error when the downloaded file cannot be shown because it no longer exists.
  ///
  /// In en, this message translates to:
  /// **'The saved file is unavailable. It may have been moved or deleted.'**
  String get transfersSavedFileUnavailable;

  /// Tooltip and screen reader label of the notifications header button: how many notifications are unread, or only the name when none are.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Notifications} one{{count} unread notification} other{{count} unread notifications}}'**
  String notificationsTooltip(int count);

  /// Title of the notifications drawer.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get notificationsTitle;

  /// Button in the notifications drawer footer that removes notifications already read.
  ///
  /// In en, this message translates to:
  /// **'Clear read notifications'**
  String get notificationsClearRead;

  /// Empty-state title of the notifications drawer.
  ///
  /// In en, this message translates to:
  /// **'No notifications'**
  String get notificationsEmptyTitle;

  /// Empty-state message of the notifications drawer.
  ///
  /// In en, this message translates to:
  /// **'Device connection requests and their results appear here.'**
  String get notificationsEmptyMessage;

  /// Title of the alert in the notifications drawer explaining a problem with operating system notifications.
  ///
  /// In en, this message translates to:
  /// **'System notifications'**
  String get notificationsSystemAlertTitle;

  /// Button on a pending device connection request that opens it for confirmation. Verb.
  ///
  /// In en, this message translates to:
  /// **'Review'**
  String get notificationsReview;

  /// Notification title: another device asks to connect to this one.
  ///
  /// In en, this message translates to:
  /// **'Connection request'**
  String get notificationsConnectionRequest;

  /// Notification title: this device started a connection that the user still has to confirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm connection'**
  String get notificationsConfirmConnection;

  /// Notification title: a device connection was confirmed.
  ///
  /// In en, this message translates to:
  /// **'Device connected'**
  String get notificationsDeviceConnected;

  /// Notification title: a device connection request was rejected.
  ///
  /// In en, this message translates to:
  /// **'Connection rejected'**
  String get notificationsConnectionRejected;

  /// Notification title: a device connection request was cancelled.
  ///
  /// In en, this message translates to:
  /// **'Connection cancelled'**
  String get notificationsConnectionCancelled;

  /// Notification title: a device connection request timed out.
  ///
  /// In en, this message translates to:
  /// **'Connection request expired'**
  String get notificationsConnectionRequestExpired;

  /// Notification title: a device connection attempt failed.
  ///
  /// In en, this message translates to:
  /// **'Connection failed'**
  String get notificationsConnectionFailed;

  /// Name shown for a device asking to connect when it did not report its own name.
  ///
  /// In en, this message translates to:
  /// **'Nearby device'**
  String get notificationsNearbyDevice;

  /// Shown in the notifications drawer when the operating system denied notification permission.
  ///
  /// In en, this message translates to:
  /// **'Allow system notifications in your OS settings to receive requests in the background.'**
  String get notificationsPermissionDenied;

  /// Shown in the notifications drawer when operating system notifications cannot be used.
  ///
  /// In en, this message translates to:
  /// **'System notifications are unavailable. Connection requests remain here.'**
  String get notificationsSystemUnavailable;

  /// Clipboard capture status: capture cannot run on this device. Adjective.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get platformCaptureStateUnavailable;

  /// Clipboard capture status: the user paused capture. Adjective.
  ///
  /// In en, this message translates to:
  /// **'Paused'**
  String get platformCaptureStatePaused;

  /// Clipboard capture status: new clipboard items are being saved.
  ///
  /// In en, this message translates to:
  /// **'Capturing'**
  String get platformCaptureStateRunning;

  /// Clipboard capture status: the capture service is not running.
  ///
  /// In en, this message translates to:
  /// **'Not running'**
  String get platformCaptureStateStopped;

  /// Disabled tray and app menu item shown when clipboard capture cannot run.
  ///
  /// In en, this message translates to:
  /// **'Capture unavailable'**
  String get platformCaptureActionUnavailable;

  /// Tray and app menu item that resumes saving clipboard items. Verb.
  ///
  /// In en, this message translates to:
  /// **'Resume capture'**
  String get platformCaptureActionResume;

  /// Tray and app menu item that pauses saving clipboard items. Verb.
  ///
  /// In en, this message translates to:
  /// **'Pause capture'**
  String get platformCaptureActionPause;

  /// Tray and app menu item that restarts clipboard capture after its service stopped.
  ///
  /// In en, this message translates to:
  /// **'Resume capture (not running)'**
  String get platformCaptureActionResumeStopped;

  /// System tray menu item that opens the application settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get platformTraySettings;

  /// System tray menu item that quits the application. Verb.
  ///
  /// In en, this message translates to:
  /// **'Quit'**
  String get platformTrayQuit;

  /// Desktop setup error: the app cannot hide or manage its window. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Desktop window controls are unavailable. CopyPaste will stay visible.'**
  String get platformWindowControlsUnavailable;

  /// Desktop setup error: the system tray (menu bar) icon cannot be created. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'The system tray is unavailable. Keep CopyPaste open, then retry or quit.'**
  String get platformTrayUnavailable;

  /// Desktop error shown when quitting the application fails. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not quit.'**
  String get platformQuitFailed;

  /// Settings error when showing or hiding the app in the Dock, taskbar, or system tray fails.
  ///
  /// In en, this message translates to:
  /// **'App visibility could not be changed. Please try again.'**
  String get platformVisibilityChangeFailed;

  /// Operating system notification title shown after a new clipboard item is saved.
  ///
  /// In en, this message translates to:
  /// **'Clipboard saved'**
  String get platformCaptureNotificationTitle;

  /// Operating system notification body shown after a new clipboard item is saved, when no preview is available. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'A new item was added to CopyPaste.'**
  String get platformCaptureNotificationBody;

  /// Operating system notification title: another device asks to connect to this one.
  ///
  /// In en, this message translates to:
  /// **'Connection request'**
  String get platformPairingNotificationTitle;

  /// Operating system notification body for a device connection request. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'{peerName} wants to connect. Open CopyPaste to compare the code and confirm.'**
  String platformPairingNotificationBody(String peerName);

  /// Name of the Android notification category for device connection requests, shown in Android notification settings.
  ///
  /// In en, this message translates to:
  /// **'Device connection requests'**
  String get platformPairingNotificationChannelName;

  /// Description of the Android notification category for device connection requests, shown in Android notification settings.
  ///
  /// In en, this message translates to:
  /// **'Requests to connect nearby devices'**
  String get platformPairingNotificationChannelDescription;

  /// Linux notification action that opens the application when the notification is clicked. Verb.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get platformNotificationDefaultAction;

  /// Error when the system file picker fails while adding files to the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Files could not be selected.'**
  String get platformFilesSelectionFailed;

  /// Error when the user tries to add a folder or another non-file item to the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Only regular files can be imported.'**
  String get platformFilesOnlyRegular;

  /// Error when optional modules are not supported on this processor architecture.
  ///
  /// In en, this message translates to:
  /// **'Modules are unavailable on this device.'**
  String get platformModulesUnavailable;

  /// Android error when setting up SMS access for the SMS codes module fails.
  ///
  /// In en, this message translates to:
  /// **'SMS access could not be configured.'**
  String get platformSmsAccessConfigureFailed;

  /// Android error when the SMS access state reported by the system is invalid.
  ///
  /// In en, this message translates to:
  /// **'SMS access could not be verified.'**
  String get platformSmsAccessVerifyFailed;

  /// Update error on an operating system without application update support.
  ///
  /// In en, this message translates to:
  /// **'App updates are unavailable here.'**
  String get platformUpdatesUnavailableHere;

  /// Update error when the installed application version cannot be read.
  ///
  /// In en, this message translates to:
  /// **'The installed version is unavailable.'**
  String get platformUpdateVersionUnavailable;

  /// Update message when this copy of the application cannot update itself.
  ///
  /// In en, this message translates to:
  /// **'App updates are unavailable on this installation.'**
  String get platformUpdatesUnavailableInstallation;

  /// Update message when the Linux package type does not support automatic updates.
  ///
  /// In en, this message translates to:
  /// **'This Linux installation cannot update automatically.'**
  String get platformUpdateLinuxUnsupported;

  /// Update message when the installed Linux package is for a different processor architecture.
  ///
  /// In en, this message translates to:
  /// **'This Linux update does not match the running architecture.'**
  String get platformUpdateLinuxArchitectureMismatch;

  /// Update message when the macOS installation type does not support automatic updates.
  ///
  /// In en, this message translates to:
  /// **'This macOS installation cannot update automatically.'**
  String get platformUpdateMacosUnsupported;

  /// Update error when installing a downloaded update cannot start. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not start the update.'**
  String get platformUpdateStartFailed;

  /// Description of the global keyboard shortcut registered with the Linux desktop, shown in system shortcut settings. Quick Paste is the feature name.
  ///
  /// In en, this message translates to:
  /// **'Open Quick Paste'**
  String get platformGlobalShortcutDescription;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'General'**
  String get settingsSectionGeneral;

  /// Settings section name in navigation and as the section heading. Noun: how the application looks.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get settingsSectionAppearance;

  /// Settings row title for choosing the application theme. Noun.
  ///
  /// In en, this message translates to:
  /// **'Theme'**
  String get settingsThemeTitle;

  /// Description of the theme setting.
  ///
  /// In en, this message translates to:
  /// **'Light, dark, or matching the system.'**
  String get settingsThemeDescription;

  /// Space-separated search keywords for the theme setting.
  ///
  /// In en, this message translates to:
  /// **'dark mode light mode color scheme'**
  String get settingsThemeKeywords;

  /// Theme option that follows the operating system's light or dark appearance.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get settingsThemeSystem;

  /// Theme option: light appearance.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get settingsThemeLight;

  /// Theme option: dark appearance.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get settingsThemeDark;

  /// Settings row title for choosing the application's accent color.
  ///
  /// In en, this message translates to:
  /// **'Accent color'**
  String get settingsAccentTitle;

  /// Description of the accent color setting.
  ///
  /// In en, this message translates to:
  /// **'Highlights selections and the current place.'**
  String get settingsAccentDescription;

  /// Space-separated search keywords for the accent color setting.
  ///
  /// In en, this message translates to:
  /// **'highlight color tint'**
  String get settingsAccentKeywords;

  /// Accent option that uses the operating system's accent color.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get settingsAccentSystem;

  /// Accent option that uses the application's own default blue.
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get settingsAccentStandard;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Purple'**
  String get settingsAccentPurple;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Pink'**
  String get settingsAccentPink;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Red'**
  String get settingsAccentRed;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Orange'**
  String get settingsAccentOrange;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Yellow'**
  String get settingsAccentYellow;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Green'**
  String get settingsAccentGreen;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Teal'**
  String get settingsAccentTeal;

  /// Accent color option name; a color.
  ///
  /// In en, this message translates to:
  /// **'Graphite'**
  String get settingsAccentGraphite;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Clipboard'**
  String get settingsSectionClipboard;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get settingsSectionPrivacy;

  /// Name of the Quick Paste feature: its settings section in navigation and as the section heading, and its row in the Linux setup step. It is a feature name, kept in English in every language.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste'**
  String get settingsSectionQuickPaste;

  /// Settings section name in navigation and as the section heading. Noun, short for synchronization.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get settingsSectionSync;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get settingsSectionNotifications;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Data'**
  String get settingsSectionData;

  /// Settings section name in navigation and as the section heading. Noun: optional add-on features.
  ///
  /// In en, this message translates to:
  /// **'Modules'**
  String get settingsSectionModules;

  /// Settings section name in navigation and as the section heading. Rules that act on copied content automatically.
  ///
  /// In en, this message translates to:
  /// **'Content rules'**
  String get settingsSectionRules;

  /// Short summary of the Content rules settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Tag, skip, protect, or expire clips automatically.'**
  String get settingsSectionRulesSummary;

  /// Android settings section name for the clipboard keyboard, Quick Settings tiles and home-screen widget.
  ///
  /// In en, this message translates to:
  /// **'Quick access'**
  String get settingsSectionQuickAccess;

  /// Settings section name in navigation and as the section heading. Text conversions such as case changes or JSON formatting.
  ///
  /// In en, this message translates to:
  /// **'Text transforms'**
  String get settingsSectionTransforms;

  /// Heading of the Paste Queue settings group. Paste Queue is the product feature name.
  ///
  /// In en, this message translates to:
  /// **'Paste Queue'**
  String get pasteQueueSettingsTitle;

  /// Description of the Paste Queue settings group.
  ///
  /// In en, this message translates to:
  /// **'Collect several copies and paste them in order.'**
  String get pasteQueueSettingsSummary;

  /// Announcement after Paste next pasted one queue item.
  ///
  /// In en, this message translates to:
  /// **'Pasted {position} of {total}'**
  String pasteQueuePasted(int position, int total);

  /// Announcement after one queue item was put on the clipboard for the user to paste.
  ///
  /// In en, this message translates to:
  /// **'Item {position} of {total} is on the clipboard'**
  String pasteQueueCopied(int position, int total);

  /// Announcement after Paste all pasted several queue items at once.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Pasted 1 item} other{Pasted {count} items}}'**
  String pasteQueuePastedAll(int count);

  /// Announcement after several queue items were joined and put on the clipboard.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item is on the clipboard} other{{count} items are on the clipboard}}'**
  String pasteQueueCopiedAll(int count);

  /// Announcement when the last queue item was pasted.
  ///
  /// In en, this message translates to:
  /// **'Queue finished'**
  String get pasteQueueFinished;

  /// Announcement after items were added to the queue, with the number now waiting.
  ///
  /// In en, this message translates to:
  /// **'{total, plural, =1{Added, 1 in queue} other{Added, {total} in queue}}'**
  String pasteQueueAdded(int total);

  /// Feedback when Paste next found no pending item; nothing was pasted.
  ///
  /// In en, this message translates to:
  /// **'Queue empty, nothing pasted'**
  String get pasteQueueNothingToPaste;

  /// Why Paste all is unavailable: the queue mixes kinds of content.
  ///
  /// In en, this message translates to:
  /// **'Paste all works with text-only or files-only queues'**
  String get pasteQueueMixed;

  /// Why Paste all is unavailable for a queue of several files.
  ///
  /// In en, this message translates to:
  /// **'Paste files one at a time.'**
  String get pasteQueueSeveralFiles;

  /// Notice that queued items were skipped because their history item no longer exists.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item no longer available} other{{count} items no longer available}}'**
  String pasteQueueUnavailable(int count);

  /// Title of the notice that Paste next only puts items on the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Paste next copies only'**
  String get pasteQueueCopyOnlyTitle;

  /// macOS notice: without the Accessibility permission the item is only put on the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Item copied. Allow Accessibility to paste automatically.'**
  String get pasteQueueAccessibilityMessage;

  /// Notice that automatic pasting is not possible here, so the item is only on the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t paste automatically. Item copied instead.'**
  String get pasteQueuePasteUnsupported;

  /// Windows notice: the front app runs as administrator, so the item was not pasted and stays in the queue.
  ///
  /// In en, this message translates to:
  /// **'Can\'t paste into an admin app. Item copied instead.'**
  String get pasteQueueTargetElevated;

  /// Notice that no app could receive the paste, so the item stays in the queue.
  ///
  /// In en, this message translates to:
  /// **'No app to paste into. Item copied instead.'**
  String get pasteQueueNoTarget;

  /// Announcement and notification when the queue starts collecting copies.
  ///
  /// In en, this message translates to:
  /// **'Collecting copies'**
  String get pasteQueueCollectingStarted;

  /// Announcement and notification when the queue stops collecting copies.
  ///
  /// In en, this message translates to:
  /// **'Stopped collecting'**
  String get pasteQueueCollectingStopped;

  /// Generic error when a Paste Queue action could not be completed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t complete the Paste Queue action'**
  String get pasteQueueFailed;

  /// Queue header summary: the number of items still to paste.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 pending} other{{count} pending}}'**
  String pasteQueueSummary(int count);

  /// Queue header summary while collecting: the number of items still to paste.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 pending · Collecting} other{{count} pending · Collecting}}'**
  String pasteQueueSummaryCollecting(int count);

  /// Short header label while the queue collects copies.
  ///
  /// In en, this message translates to:
  /// **'Collecting'**
  String get pasteQueueCollectingLabel;

  /// Queue header summary when every item was pasted.
  ///
  /// In en, this message translates to:
  /// **'Finished'**
  String get pasteQueueFinishedSummary;

  /// Warning that the queue reached its size limit.
  ///
  /// In en, this message translates to:
  /// **'Queue full ({max})'**
  String pasteQueueFull(int max);

  /// Explains what happens while the queue is full.
  ///
  /// In en, this message translates to:
  /// **'New copies are no longer added.'**
  String get pasteQueueFullMessage;

  /// Empty state title of the queue drawer.
  ///
  /// In en, this message translates to:
  /// **'Paste Queue is empty'**
  String get pasteQueueEmptyTitle;

  /// Empty state message of the queue drawer.
  ///
  /// In en, this message translates to:
  /// **'Collect copies or add items from history.'**
  String get pasteQueueEmptyMessage;

  /// Empty state title while collecting and nothing was copied yet.
  ///
  /// In en, this message translates to:
  /// **'Copy items to add them'**
  String get pasteQueueCollectingEmptyTitle;

  /// Button that starts collecting copies into the queue.
  ///
  /// In en, this message translates to:
  /// **'Start collecting'**
  String get pasteQueueStartCollecting;

  /// Button that stops collecting copies; existing items stay.
  ///
  /// In en, this message translates to:
  /// **'Stop collecting'**
  String get pasteQueueStopCollecting;

  /// Error state title when the queue could not be read.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load the queue'**
  String get pasteQueueLoadFailed;

  /// Action and shortcut name: paste the next queue item into the front app.
  ///
  /// In en, this message translates to:
  /// **'Paste next'**
  String get pasteQueuePasteNext;

  /// Action that puts the next queue item on the clipboard without pasting.
  ///
  /// In en, this message translates to:
  /// **'Copy next'**
  String get pasteQueueCopyNext;

  /// Action and shortcut name: paste every pending item at once, joined by the separator.
  ///
  /// In en, this message translates to:
  /// **'Paste all'**
  String get pasteQueuePasteAll;

  /// Action that puts every pending item, joined, on the clipboard without pasting.
  ///
  /// In en, this message translates to:
  /// **'Copy all'**
  String get pasteQueueCopyAll;

  /// Action that reverses the order of the pending items.
  ///
  /// In en, this message translates to:
  /// **'Reverse order'**
  String get pasteQueueReverse;

  /// Action and shortcut name: return the last pasted item to the head of the queue.
  ///
  /// In en, this message translates to:
  /// **'Step back'**
  String get pasteQueueStepBack;

  /// Action and shortcut name: mark the next item skipped without pasting it.
  ///
  /// In en, this message translates to:
  /// **'Skip'**
  String get pasteQueueSkip;

  /// Action that removes pasted and skipped items from the queue.
  ///
  /// In en, this message translates to:
  /// **'Remove pasted'**
  String get pasteQueueRemovePasted;

  /// Notice after the queue was cleared, shown with Undo.
  ///
  /// In en, this message translates to:
  /// **'Queue cleared'**
  String get pasteQueueCleared;

  /// Button that restores the entries the last Clear removed.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get pasteQueueUndo;

  /// Choice when collecting restarts after the queue finished: remove pasted items first.
  ///
  /// In en, this message translates to:
  /// **'Start fresh'**
  String get pasteQueueStartFresh;

  /// Choice when collecting restarts after the queue finished: keep pasted items.
  ///
  /// In en, this message translates to:
  /// **'Append'**
  String get pasteQueueAppend;

  /// Dialog title when collecting starts again on a finished queue.
  ///
  /// In en, this message translates to:
  /// **'Start a new collection?'**
  String get pasteQueueRestartTitle;

  /// Dialog message explaining Start fresh and Append.
  ///
  /// In en, this message translates to:
  /// **'Start fresh removes the pasted items. Append keeps them.'**
  String get pasteQueueRestartMessage;

  /// One-line hint that the pending items are old.
  ///
  /// In en, this message translates to:
  /// **'Queue from {time}'**
  String pasteQueueStale(String time);

  /// Status word of a queue item that was pasted.
  ///
  /// In en, this message translates to:
  /// **'Pasted'**
  String get pasteQueueStatusPasted;

  /// Status word of a queue item that was skipped.
  ///
  /// In en, this message translates to:
  /// **'Skipped'**
  String get pasteQueueStatusSkipped;

  /// Status word of a queue item whose history item was deleted.
  ///
  /// In en, this message translates to:
  /// **'No longer available'**
  String get pasteQueueStatusUnavailable;

  /// Marker of a queue item whose text was edited in the queue.
  ///
  /// In en, this message translates to:
  /// **'Edited'**
  String get pasteQueueEdited;

  /// Placeholder shown instead of the content of a secret queue item.
  ///
  /// In en, this message translates to:
  /// **'Secret'**
  String get pasteQueueSecret;

  /// Shown for a queued image.
  ///
  /// In en, this message translates to:
  /// **'Image'**
  String get pasteQueueKindImage;

  /// Shown for queued files without a name.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get pasteQueueKindFiles;

  /// When a queued snippet was rendered.
  ///
  /// In en, this message translates to:
  /// **'Rendered at {time}'**
  String pasteQueueRenderedAt(String time);

  /// Screen-reader label of a queue row: its position and content.
  ///
  /// In en, this message translates to:
  /// **'Item {position}: {content}'**
  String pasteQueueRowLabel(String position, String content);

  /// Announcement after an item was moved.
  ///
  /// In en, this message translates to:
  /// **'Moved to position {position}'**
  String pasteQueueMoved(int position);

  /// Row action that moves an item one place earlier.
  ///
  /// In en, this message translates to:
  /// **'Move up'**
  String get pasteQueueMoveUp;

  /// Row action that moves an item one place later.
  ///
  /// In en, this message translates to:
  /// **'Move down'**
  String get pasteQueueMoveDown;

  /// Row action that moves an item to the start of the queue.
  ///
  /// In en, this message translates to:
  /// **'Move to top'**
  String get pasteQueueMoveToTop;

  /// Row action that moves an item to the end of the queue.
  ///
  /// In en, this message translates to:
  /// **'Move to bottom'**
  String get pasteQueueMoveToBottom;

  /// Tooltip of the drag handle that reorders a queue item.
  ///
  /// In en, this message translates to:
  /// **'Drag to reorder'**
  String get pasteQueueReorder;

  /// Tooltip of the button that opens a queue row's actions.
  ///
  /// In en, this message translates to:
  /// **'More actions'**
  String get pasteQueueMoreActions;

  /// Row action that pastes this item now, out of order.
  ///
  /// In en, this message translates to:
  /// **'Paste now'**
  String get pasteQueuePasteNow;

  /// Row action that edits a queued text item.
  ///
  /// In en, this message translates to:
  /// **'Edit text'**
  String get pasteQueueEdit;

  /// Row action that drops the queue-local edit and pastes the original text again.
  ///
  /// In en, this message translates to:
  /// **'Use original text'**
  String get pasteQueueUseOriginal;

  /// Row action that removes the item from the queue; history keeps it.
  ///
  /// In en, this message translates to:
  /// **'Remove from queue'**
  String get pasteQueueRemove;

  /// Title of the dialog that edits a queued text item.
  ///
  /// In en, this message translates to:
  /// **'Edit queued text'**
  String get pasteQueueEditTitle;

  /// Explains that a queue edit does not change history.
  ///
  /// In en, this message translates to:
  /// **'Edits apply only to this queue.'**
  String get pasteQueueEditMessage;

  /// Prompt title and button that leads to setting queue shortcuts.
  ///
  /// In en, this message translates to:
  /// **'Set shortcuts'**
  String get pasteQueueShortcutPromptTitle;

  /// Explains why to set queue shortcuts; they start unbound on Windows and Linux.
  ///
  /// In en, this message translates to:
  /// **'Collect and paste without opening CopyPaste.'**
  String get pasteQueueShortcutPromptMessage;

  /// Shortcut name: start or stop collecting copies.
  ///
  /// In en, this message translates to:
  /// **'Start or stop collecting'**
  String get pasteQueueShortcutToggleCollecting;

  /// Shortcut name: paste the next item without formatting.
  ///
  /// In en, this message translates to:
  /// **'Paste next as plain text'**
  String get pasteQueueShortcutPasteNextPlain;

  /// Settings search text for the queue shortcut rows; not shown in the rows themselves.
  ///
  /// In en, this message translates to:
  /// **'A global shortcut that works in any app.'**
  String get pasteQueueShortcutDescription;

  /// Shortcut refused because another CopyPaste shortcut uses it.
  ///
  /// In en, this message translates to:
  /// **'Another CopyPaste shortcut uses this combination. The previous one is kept.'**
  String get pasteQueueShortcutHeldByApp;

  /// Shortcut refused because another app holds it.
  ///
  /// In en, this message translates to:
  /// **'Another app uses this combination.'**
  String get pasteQueueShortcutInUse;

  /// Shortcut refused by the operating system.
  ///
  /// In en, this message translates to:
  /// **'The system refused this combination. The previous one is kept.'**
  String get pasteQueueShortcutRejected;

  /// Recorder refusal on Windows and Linux: Ctrl+Alt is AltGr on many keyboard layouts.
  ///
  /// In en, this message translates to:
  /// **'Ctrl+Alt types characters on many layouts. Choose another.'**
  String get pasteQueueShortcutAltGr;

  /// This desktop session offers no global shortcuts (for example some Wayland sessions).
  ///
  /// In en, this message translates to:
  /// **'Global shortcuts aren\'t available in this session.'**
  String get pasteQueueShortcutUnavailable;

  /// Generic failure when a queue shortcut could not be set.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t set this shortcut.'**
  String get pasteQueueShortcutFailed;

  /// Settings switch that turns the Paste Queue feature on or off.
  ///
  /// In en, this message translates to:
  /// **'Enable Paste Queue'**
  String get pasteQueueEnableTitle;

  /// Description of the Paste Queue switch.
  ///
  /// In en, this message translates to:
  /// **'When off, shortcuts and collecting stop.'**
  String get pasteQueueEnableDescription;

  /// Setting: what Paste next does on this device.
  ///
  /// In en, this message translates to:
  /// **'Paste next does'**
  String get pasteQueuePasteNextStyleTitle;

  /// Delivery style: paste into the front app.
  ///
  /// In en, this message translates to:
  /// **'Paste into the front app'**
  String get pasteQueuePasteIntoApp;

  /// Delivery style: only put the item on the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Put on the clipboard only'**
  String get pasteQueueClipboardOnly;

  /// Setting: where new items join the queue.
  ///
  /// In en, this message translates to:
  /// **'New items go to'**
  String get pasteQueueInsertTitle;

  /// Insertion position: at the end (first in, first out).
  ///
  /// In en, this message translates to:
  /// **'End (first in, first out)'**
  String get pasteQueueInsertEnd;

  /// Insertion position: at the start (last in, first out).
  ///
  /// In en, this message translates to:
  /// **'Start (last in, first out)'**
  String get pasteQueueInsertStart;

  /// Setting: what happens after the last item was pasted.
  ///
  /// In en, this message translates to:
  /// **'After the last item'**
  String get pasteQueueEndBehaviorTitle;

  /// End behavior: stop and finish the queue.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get pasteQueueEndStop;

  /// End behavior: start over from the first item.
  ///
  /// In en, this message translates to:
  /// **'Start over'**
  String get pasteQueueEndLoop;

  /// End behavior: keep pasting the last item.
  ///
  /// In en, this message translates to:
  /// **'Keep pasting the last item'**
  String get pasteQueueEndKeepLast;

  /// Setting: the separator placed between items by Paste all.
  ///
  /// In en, this message translates to:
  /// **'Paste all separator'**
  String get pasteQueueSeparatorTitle;

  /// Description of the separator setting.
  ///
  /// In en, this message translates to:
  /// **'Placed between items when pasting all at once.'**
  String get pasteQueueSeparatorDescription;

  /// Separator choice.
  ///
  /// In en, this message translates to:
  /// **'New line'**
  String get pasteQueueSeparatorNewLine;

  /// Separator choice.
  ///
  /// In en, this message translates to:
  /// **'Blank line'**
  String get pasteQueueSeparatorBlankLine;

  /// Separator choice: a tab character.
  ///
  /// In en, this message translates to:
  /// **'Tab'**
  String get pasteQueueSeparatorTab;

  /// Separator choice.
  ///
  /// In en, this message translates to:
  /// **'Space'**
  String get pasteQueueSeparatorSpace;

  /// Separator choice.
  ///
  /// In en, this message translates to:
  /// **'Comma'**
  String get pasteQueueSeparatorComma;

  /// Separator choice.
  ///
  /// In en, this message translates to:
  /// **'Comma and space'**
  String get pasteQueueSeparatorCommaSpace;

  /// Separator choice: nothing between items.
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get pasteQueueSeparatorNone;

  /// Separator choice: text the user enters.
  ///
  /// In en, this message translates to:
  /// **'Custom'**
  String get pasteQueueSeparatorCustom;

  /// Placeholder of the custom separator field; \n and \t are typed literally by the user.
  ///
  /// In en, this message translates to:
  /// **'Up to 32 characters; \\n and \\t allowed'**
  String get pasteQueueCustomSeparatorPlaceholder;

  /// Setting: always paste queue items without formatting.
  ///
  /// In en, this message translates to:
  /// **'Always paste as plain text'**
  String get pasteQueuePlainTextTitle;

  /// Setting: stop collecting after a period without copies.
  ///
  /// In en, this message translates to:
  /// **'Stop collecting after'**
  String get pasteQueueIdleStopTitle;

  /// Description of the idle stop setting.
  ///
  /// In en, this message translates to:
  /// **'Time without a new copy.'**
  String get pasteQueueIdleStopDescription;

  /// Idle stop choice in minutes.
  ///
  /// In en, this message translates to:
  /// **'{minutes} min'**
  String pasteQueueIdleMinutes(int minutes);

  /// Idle stop choice: never stop on its own.
  ///
  /// In en, this message translates to:
  /// **'Never'**
  String get pasteQueueIdleNever;

  /// Setting: the queue size limit.
  ///
  /// In en, this message translates to:
  /// **'Queue size limit'**
  String get pasteQueueMaxEntriesTitle;

  /// Description of the size limit setting.
  ///
  /// In en, this message translates to:
  /// **'When full, new copies aren\'t added.'**
  String get pasteQueueMaxEntriesDescription;

  /// Setting: restore the clipboard from before the queue started pasting.
  ///
  /// In en, this message translates to:
  /// **'Restore the previous clipboard when finished'**
  String get pasteQueueRestoreTitle;

  /// Search keywords for the Paste Queue settings, space separated.
  ///
  /// In en, this message translates to:
  /// **'paste queue collect sequence order shortcut hotkey separator stack'**
  String get pasteQueueSettingsKeywords;

  /// Settings section name in navigation and as the section heading, about the application.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get settingsSectionAbout;

  /// Description shown under the Modules settings section heading. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Optional features for CopyPaste.'**
  String get settingsModulesDescription;

  /// Description shown under the Quick Paste settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Open clipboard history from anywhere without switching windows.'**
  String get settingsQuickPasteDescription;

  /// Error title shown when settings could not be loaded.
  ///
  /// In en, this message translates to:
  /// **'Settings are unavailable'**
  String get settingsUnavailableTitle;

  /// Empty-state title when a settings search has no matches.
  ///
  /// In en, this message translates to:
  /// **'No settings found'**
  String get settingsNoResultsTitle;

  /// Empty-state message when a settings search has no matches.
  ///
  /// In en, this message translates to:
  /// **'Try a different search.'**
  String get settingsNoResultsMessage;

  /// Title of an error alert at the top of a settings section.
  ///
  /// In en, this message translates to:
  /// **'Settings need attention'**
  String get settingsAttentionTitle;

  /// Placeholder of the settings search field.
  ///
  /// In en, this message translates to:
  /// **'Search settings'**
  String get settingsSearchPlaceholder;

  /// Screen reader label of the back button that returns from a settings section to the list of sections.
  ///
  /// In en, this message translates to:
  /// **'Back to Settings'**
  String get settingsBackToSettings;

  /// Android settings row title: start the app's background services when the device boots.
  ///
  /// In en, this message translates to:
  /// **'Start on device boot'**
  String get settingsStartOnBootTitle;

  /// Desktop settings row title: open the app when the user signs in.
  ///
  /// In en, this message translates to:
  /// **'Start at login'**
  String get settingsStartAtLoginTitle;

  /// Search result description for the automatic startup setting. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Start CopyPaste automatically.'**
  String get settingsStartupSearchDescription;

  /// Space-separated search keywords that find the automatic startup setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'startup login boot launch automatically autostart'**
  String get settingsStartupKeywords;

  /// Windows startup setting description when startup is disabled by the user in Task Manager. Settings → Apps → Startup is the Windows settings path; use the Windows wording for your language.
  ///
  /// In en, this message translates to:
  /// **'Disabled in Task Manager. Enable CopyPaste in Settings → Apps → Startup.'**
  String get settingsStartupWindowsApproval;

  /// macOS startup setting description when the user must approve the login item. System Settings → General → Login Items is the macOS settings path; use the macOS wording for your language.
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste in System Settings → General → Login Items.'**
  String get settingsStartupMacApproval;

  /// Startup setting description when automatic startup cannot be configured.
  ///
  /// In en, this message translates to:
  /// **'Automatic startup is unavailable in this installation.'**
  String get settingsStartupUnavailable;

  /// Android startup setting description.
  ///
  /// In en, this message translates to:
  /// **'Resume background services after a restart.'**
  String get settingsStartupAndroidDescription;

  /// macOS settings row title: show the app icon in the Dock. Dock is the macOS feature name.
  ///
  /// In en, this message translates to:
  /// **'Show in Dock'**
  String get settingsShowInDock;

  /// Windows and Linux settings row title: show the app icon in the taskbar.
  ///
  /// In en, this message translates to:
  /// **'Show in taskbar'**
  String get settingsShowInTaskbar;

  /// Description of the Dock or taskbar visibility setting.
  ///
  /// In en, this message translates to:
  /// **'Show the app icon for switching windows.'**
  String get settingsTaskbarVisibilityDescription;

  /// Space-separated search keywords that find the Dock or taskbar visibility setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'dock taskbar application icon visibility'**
  String get settingsTaskbarVisibilityKeywords;

  /// macOS settings row title: show the app icon in the menu bar.
  ///
  /// In en, this message translates to:
  /// **'Show in menu bar'**
  String get settingsShowInMenuBar;

  /// Windows and Linux settings row title: show the app icon in the system tray.
  ///
  /// In en, this message translates to:
  /// **'Show in system tray'**
  String get settingsShowInSystemTray;

  /// Description of the menu bar or system tray visibility setting. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Show quick access to CopyPaste and capture controls.'**
  String get settingsTrayVisibilityDescription;

  /// Space-separated search keywords that find the menu bar or system tray visibility setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'tray menu bar menubar status icon visibility'**
  String get settingsTrayVisibilityKeywords;

  /// Desktop settings row title: closing the window quits the app while the background runtime keeps running.
  ///
  /// In en, this message translates to:
  /// **'Lightweight mode'**
  String get settingsLightweightModeTitle;

  /// Description of the lightweight mode setting. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Closing the window quits the app; capture and sync keep running. The tray, Quick Paste and shortcuts return when you open it.'**
  String get settingsLightweightModeDescription;

  /// Space-separated search keywords that find the lightweight mode setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'lightweight close quit background memory'**
  String get settingsLightweightModeKeywords;

  /// Search result description for the Modules section.
  ///
  /// In en, this message translates to:
  /// **'Install, update, and remove optional modules.'**
  String get settingsModulesSearchDescription;

  /// Space-separated search keywords that find the Modules setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'features commands extensions'**
  String get settingsModulesKeywords;

  /// Settings row title for setting up clipboard capture while the Android app is in the background.
  ///
  /// In en, this message translates to:
  /// **'Android background capture'**
  String get settingsAndroidCaptureTitle;

  /// Search result description for Android background capture. Full and Limited are capture mode names; Shizuku and ADB are tool names.
  ///
  /// In en, this message translates to:
  /// **'Full or Limited mode with Shizuku or ADB setup.'**
  String get settingsAndroidCaptureSearchDescription;

  /// Space-separated search keywords that find the Android background capture setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'background permissions setup'**
  String get settingsAndroidCaptureKeywords;

  /// Description of the Android background capture row. Full and Limited are capture mode names; Shizuku and ADB are tool names.
  ///
  /// In en, this message translates to:
  /// **'Full or Limited mode and one-time Shizuku or ADB setup.'**
  String get settingsAndroidCaptureDescription;

  /// Button that opens the Android background capture setup flow.
  ///
  /// In en, this message translates to:
  /// **'Open setup'**
  String get settingsOpenSetup;

  /// Settings row title for the switch that saves new screenshots to History.
  ///
  /// In en, this message translates to:
  /// **'Save screenshots'**
  String get settingsSaveScreenshotsTitle;

  /// Search result description for the screenshot saving setting. History is the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'Automatically save new Android screenshots to History.'**
  String get settingsSaveScreenshotsSearchDescription;

  /// Space-separated search keywords that find the screenshot saving setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'images photos capture permissions'**
  String get settingsSaveScreenshotsKeywords;

  /// Description of the screenshot saving switch. History is the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'Automatically add new screenshots to History.'**
  String get settingsSaveScreenshotsDescription;

  /// Settings row title asking for permissions needed to save screenshots.
  ///
  /// In en, this message translates to:
  /// **'Screenshot access'**
  String get settingsScreenshotAccessTitle;

  /// Description of the permission row needed to save screenshots.
  ///
  /// In en, this message translates to:
  /// **'Needed to save screenshots in the background.'**
  String get settingsScreenshotAccessDescription;

  /// Button that requests the photo and notification permissions for screenshot saving.
  ///
  /// In en, this message translates to:
  /// **'Allow access'**
  String get settingsAllowAccess;

  /// Error title when screenshot saving is enabled but its service is stopped.
  ///
  /// In en, this message translates to:
  /// **'Screenshot capture is not running'**
  String get settingsScreenshotCaptureStoppedTitle;

  /// Error message when screenshot saving is enabled but its service is stopped.
  ///
  /// In en, this message translates to:
  /// **'New screenshots are not being saved. Resume capture to start the service again.'**
  String get settingsScreenshotCaptureStoppedMessage;

  /// Button that restarts the stopped screenshot capture service.
  ///
  /// In en, this message translates to:
  /// **'Resume capture'**
  String get settingsResumeCapture;

  /// Settings row title for the switch that pauses or resumes clipboard capture.
  ///
  /// In en, this message translates to:
  /// **'Clipboard capture'**
  String get settingsClipboardCaptureTitle;

  /// Search result description for the clipboard capture switch.
  ///
  /// In en, this message translates to:
  /// **'Pause or resume clipboard capture.'**
  String get settingsClipboardCaptureSearchDescription;

  /// Clipboard capture row description when the capture state is unknown.
  ///
  /// In en, this message translates to:
  /// **'State unavailable'**
  String get settingsCaptureStateUnavailable;

  /// Alert title when clipboard capture is enabled but not running.
  ///
  /// In en, this message translates to:
  /// **'Capture is not running'**
  String get settingsCaptureStoppedTitle;

  /// Alert message when clipboard capture is enabled but not running. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is not currently receiving clipboard changes on this device.'**
  String get settingsCaptureStoppedMessage;

  /// Settings row title for how long clipboard history is kept. Noun.
  ///
  /// In en, this message translates to:
  /// **'Retention'**
  String get settingsRetentionTitle;

  /// Description of the history retention setting.
  ///
  /// In en, this message translates to:
  /// **'Automatically remove old unpinned clips.'**
  String get settingsRetentionDescription;

  /// Retention option that never removes history automatically.
  ///
  /// In en, this message translates to:
  /// **'Keep forever'**
  String get settingsRetentionForever;

  /// Settings row title for the maximum disk space used by history. Noun.
  ///
  /// In en, this message translates to:
  /// **'Storage quota'**
  String get settingsStorageQuotaTitle;

  /// Description of the storage quota setting.
  ///
  /// In en, this message translates to:
  /// **'Maximum local storage used by unpinned history.'**
  String get settingsStorageQuotaDescription;

  /// Space-separated search keywords that find the storage quota setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'disk space limit'**
  String get settingsStorageQuotaKeywords;

  /// Settings row title for the largest file that can be added or downloaded.
  ///
  /// In en, this message translates to:
  /// **'Maximum file size'**
  String get settingsMaxFileSizeTitle;

  /// Search result description for the maximum file size setting.
  ///
  /// In en, this message translates to:
  /// **'Limit for adding and downloading files.'**
  String get settingsMaxFileSizeSearchDescription;

  /// Space-separated search keywords that find the maximum file size setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'import download transfer size limit'**
  String get settingsMaxFileSizeKeywords;

  /// Description of the maximum file size setting. Download is the name of the History action that transfers a file.
  ///
  /// In en, this message translates to:
  /// **'Largest file you can add or download.'**
  String get settingsMaxFileSizeDescription;

  /// Settings row title for how captured files are stored.
  ///
  /// In en, this message translates to:
  /// **'File storage'**
  String get settingsFileStorageTitle;

  /// Description of the file storage setting: a link versus an encrypted copy.
  ///
  /// In en, this message translates to:
  /// **'A link works only while the original exists; a copy keeps the file.'**
  String get settingsFileStorageDescription;

  /// Space-separated search keywords that find the file storage setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'files copy link original keep store offline'**
  String get settingsFileStorageKeywords;

  /// File storage option: keep only a link to the original file.
  ///
  /// In en, this message translates to:
  /// **'Link to original'**
  String get settingsFileStorageLink;

  /// File storage option: keep an encrypted copy of the file.
  ///
  /// In en, this message translates to:
  /// **'Keep a copy'**
  String get settingsFileStorageCopy;

  /// Settings row title for the folder downloaded files are saved to.
  ///
  /// In en, this message translates to:
  /// **'Save files to'**
  String get settingsSaveFolderTitle;

  /// Save folder row description when no folder is chosen.
  ///
  /// In en, this message translates to:
  /// **'Ask every time'**
  String get settingsSaveFolderAsk;

  /// Button that clears the save folder so every download asks for a destination.
  ///
  /// In en, this message translates to:
  /// **'Ask every time'**
  String get settingsSaveFolderAskAction;

  /// Button that opens a folder picker for the save folder.
  ///
  /// In en, this message translates to:
  /// **'Choose folder'**
  String get settingsSaveFolderChooseAction;

  /// Search result description for the save folder setting.
  ///
  /// In en, this message translates to:
  /// **'Folder for downloaded files.'**
  String get settingsSaveFolderSearchDescription;

  /// Space-separated search keywords that find the save folder setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'download folder destination directory save location'**
  String get settingsSaveFolderKeywords;

  /// Settings row title for downloading files from other devices automatically.
  ///
  /// In en, this message translates to:
  /// **'Download received files'**
  String get settingsAutoDownloadTitle;

  /// Description of automatic downloads of files received from other devices.
  ///
  /// In en, this message translates to:
  /// **'Saves files from other devices to the folder while they are online.'**
  String get settingsAutoDownloadDescription;

  /// Automatic download row description while no save folder is chosen.
  ///
  /// In en, this message translates to:
  /// **'Choose a folder to save files to first.'**
  String get settingsAutoDownloadNeedsFolder;

  /// Space-separated search keywords that find the automatic download setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'automatic download receive sync files'**
  String get settingsAutoDownloadKeywords;

  /// Search result title for exporting, backing up, and restoring history.
  ///
  /// In en, this message translates to:
  /// **'History files'**
  String get settingsHistoryFilesTitle;

  /// Search result description for exporting, backing up, and restoring history.
  ///
  /// In en, this message translates to:
  /// **'Export history, create backups, or restore a backup.'**
  String get settingsHistoryFilesSearchDescription;

  /// Space-separated search keywords that find the history export and backup setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'text encrypted backup data'**
  String get settingsHistoryFilesKeywords;

  /// Settings row title for exporting text clips to a file.
  ///
  /// In en, this message translates to:
  /// **'Export text history'**
  String get settingsExportTextTitle;

  /// Description of the text history export row.
  ///
  /// In en, this message translates to:
  /// **'Save text clips as a JSON file.'**
  String get settingsExportTextDescription;

  /// Verb. Button that exports text history to a file.
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get settingsExportAction;

  /// Settings row title for creating an encrypted backup. Noun.
  ///
  /// In en, this message translates to:
  /// **'Encrypted backup'**
  String get settingsBackupTitle;

  /// Description of the encrypted backup row.
  ///
  /// In en, this message translates to:
  /// **'Save the complete local history for this device.'**
  String get settingsBackupDescription;

  /// Button that creates an encrypted backup file.
  ///
  /// In en, this message translates to:
  /// **'Back up'**
  String get settingsCreateBackupAction;

  /// Settings row title for restoring history from a backup.
  ///
  /// In en, this message translates to:
  /// **'Restore backup'**
  String get settingsRestoreTitle;

  /// Description of the restore backup row.
  ///
  /// In en, this message translates to:
  /// **'Replace local history with an encrypted backup.'**
  String get settingsRestoreDescription;

  /// Verb. Button and dialog action that restores history from a backup.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get settingsRestoreAction;

  /// Title of the dialog confirming a backup restore.
  ///
  /// In en, this message translates to:
  /// **'Replace local history?'**
  String get settingsRestoreDialogTitle;

  /// Message of the dialog confirming a backup restore.
  ///
  /// In en, this message translates to:
  /// **'The backup replaces this device\'s history. Settings are kept.'**
  String get settingsRestoreDialogMessage;

  /// Error shown in the restore dialog when restoring fails.
  ///
  /// In en, this message translates to:
  /// **'The backup could not be restored.'**
  String get settingsRestoreFailed;

  /// Settings row title for the master synchronization switch. Noun, short for synchronization.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get settingsSyncSwitchTitle;

  /// Description of the master synchronization switch.
  ///
  /// In en, this message translates to:
  /// **'Sync history with paired devices.'**
  String get settingsSyncSwitchDescription;

  /// Search result description for the master synchronization switch.
  ///
  /// In en, this message translates to:
  /// **'Paired-device synchronization.'**
  String get settingsSyncSearchDescription;

  /// Settings row title for automatically copying clips received from other devices.
  ///
  /// In en, this message translates to:
  /// **'Instant clipboard'**
  String get settingsInstantClipboardTitle;

  /// Search result description for the instant clipboard setting.
  ///
  /// In en, this message translates to:
  /// **'Automatically copy new clips from other devices.'**
  String get settingsInstantClipboardSearchDescription;

  /// Space-separated search keywords that find the instant clipboard setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'automatic copy paste sync received'**
  String get settingsInstantClipboardKeywords;

  /// Description of the instant clipboard setting.
  ///
  /// In en, this message translates to:
  /// **'Copy new clips from other devices to this clipboard.'**
  String get settingsInstantClipboardDescription;

  /// Settings row title for being discoverable on the local network. LAN means local area network.
  ///
  /// In en, this message translates to:
  /// **'LAN visibility'**
  String get settingsLanVisibilityTitle;

  /// Description of the LAN visibility setting.
  ///
  /// In en, this message translates to:
  /// **'Allow nearby devices to discover this device.'**
  String get settingsLanVisibilityDescription;

  /// Space-separated search keywords that find the LAN visibility setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'local network discovery'**
  String get settingsLanVisibilityKeywords;

  /// Title of the setting that holds the sync relay server address.
  ///
  /// In en, this message translates to:
  /// **'Relay server'**
  String get settingsRelayTitle;

  /// Explains the relay server setting: devices sync across networks through a self-hosted relay.
  ///
  /// In en, this message translates to:
  /// **'Sync paired devices across networks through your own relay. Leave empty to turn it off.'**
  String get settingsRelayDescription;

  /// Space-separated search keywords that find the relay setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'relay server remote internet network websocket'**
  String get settingsRelayKeywords;

  /// Example relay address shown in the empty field. Keep as is.
  ///
  /// In en, this message translates to:
  /// **'wss://relay.example.com'**
  String get settingsRelayPlaceholder;

  /// Status while the app connects to the relay.
  ///
  /// In en, this message translates to:
  /// **'Connecting to the relay…'**
  String get settingsRelayConnecting;

  /// Status when the app is connected to the relay.
  ///
  /// In en, this message translates to:
  /// **'Connected to the relay.'**
  String get settingsRelayConnected;

  /// Warning when the relay rejects the access token in the address.
  ///
  /// In en, this message translates to:
  /// **'The relay refused the access token.'**
  String get settingsRelayUnauthorized;

  /// Warning when the relay cannot be reached.
  ///
  /// In en, this message translates to:
  /// **'Can\'t reach the relay.'**
  String get settingsRelayUnreachable;

  /// Title of the switch that lets direct sync request a UPnP port mapping from the router.
  ///
  /// In en, this message translates to:
  /// **'Router port mapping'**
  String get settingsPortMappingTitle;

  /// Explains the router port mapping switch.
  ///
  /// In en, this message translates to:
  /// **'Let direct sync ask your router (UPnP) to open a port for a few minutes. Helps when both networks block direct connections.'**
  String get settingsPortMappingDescription;

  /// Space-separated search keywords that find the port mapping setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'router port mapping upnp nat forwarding direct'**
  String get settingsPortMappingKeywords;

  /// Settings row title for apps whose clipboard changes are not captured.
  ///
  /// In en, this message translates to:
  /// **'Excluded apps'**
  String get settingsExcludedAppsTitle;

  /// Search result description for excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Skip automatic capture from excluded apps.'**
  String get settingsExcludedAppsSearchDescription;

  /// Space-separated search keywords that find the excluded applications setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'privacy app application identifiers'**
  String get settingsExcludedAppsKeywords;

  /// Windows and Linux description of excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Skip automatic capture from these apps.'**
  String get settingsExcludedAppsDesktopDescription;

  /// Android description of excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Automatic capture pauses while any app is excluded.'**
  String get settingsExcludedAppsAndroidDescription;

  /// Description of excluded applications on platforms without support.
  ///
  /// In en, this message translates to:
  /// **'App exclusions are supported on macOS, Windows, Android, and Linux.'**
  String get settingsExcludedAppsUnsupportedDescription;

  /// Verb. Button that adds an application to the excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get settingsExcludedAppsAdd;

  /// Shown when the excluded applications list is empty.
  ///
  /// In en, this message translates to:
  /// **'No apps are excluded.'**
  String get settingsExcludedAppsEmpty;

  /// Tooltip of the button that removes an application from the excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Remove {app}'**
  String settingsExcludedAppsRemove(String app);

  /// Search field placeholder of the list for choosing an application.
  ///
  /// In en, this message translates to:
  /// **'Search apps'**
  String get applicationPickerSearch;

  /// Shown when no application matches the search in the application list.
  ///
  /// In en, this message translates to:
  /// **'No apps found'**
  String get applicationPickerEmpty;

  /// Settings row title for not capturing clipboard content that the source app marked as secret.
  ///
  /// In en, this message translates to:
  /// **'Skip secret clipboard'**
  String get settingsSkipSecretTitle;

  /// Description of the Skip secret clipboard setting. History is the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'Don\'t save copies that apps mark as secret.'**
  String get settingsSkipSecretDescription;

  /// Space-separated search keywords that find the confidential clipboard setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'secret password sensitive concealed spoiler'**
  String get settingsSkipSecretKeywords;

  /// Settings row title for not capturing clipboard content that the source app marked as temporary.
  ///
  /// In en, this message translates to:
  /// **'Skip temporary clipboard'**
  String get settingsSkipTransientTitle;

  /// Description of the temporary clipboard setting. History is the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'Don\'t save copies that apps mark as temporary.'**
  String get settingsSkipTransientDescription;

  /// Space-separated search keywords that find the temporary clipboard setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'transient temporary privacy'**
  String get settingsSkipTransientKeywords;

  /// Settings row title for preventing screenshots of the app.
  ///
  /// In en, this message translates to:
  /// **'Block screenshots'**
  String get settingsBlockScreenshotsTitle;

  /// Description of the block screenshots setting. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Prevent screenshots and screen recording of CopyPaste.'**
  String get settingsBlockScreenshotsDescription;

  /// Description of the block screenshots setting when the platform does not support it.
  ///
  /// In en, this message translates to:
  /// **'Screenshot blocking is unavailable on this platform.'**
  String get settingsBlockScreenshotsUnsupported;

  /// Space-separated search keywords that find the block screenshots setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'privacy screen capture protection pairing qr security code'**
  String get settingsBlockScreenshotsKeywords;

  /// Settings row title for showing a notification when a clip is captured.
  ///
  /// In en, this message translates to:
  /// **'Notification on copy'**
  String get settingsNotifyOnCopyTitle;

  /// Description of the notification on copy setting.
  ///
  /// In en, this message translates to:
  /// **'Show a notification after a background capture.'**
  String get settingsNotifyOnCopyDescription;

  /// Settings row title for including the clip content in copy notifications.
  ///
  /// In en, this message translates to:
  /// **'Show clipboard content'**
  String get settingsNotificationPreviewTitle;

  /// Description of the notification preview setting.
  ///
  /// In en, this message translates to:
  /// **'Include a clip preview in copy notifications.'**
  String get settingsNotificationPreviewDescription;

  /// Space-separated search keywords that find the notification preview setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'notification preview text image privacy'**
  String get settingsNotificationPreviewKeywords;

  /// Settings row title for playing a sound when a clip is captured.
  ///
  /// In en, this message translates to:
  /// **'Sound on copy'**
  String get settingsSoundOnCopyTitle;

  /// Description of the sound on copy setting.
  ///
  /// In en, this message translates to:
  /// **'Play platform feedback after a successful capture.'**
  String get settingsSoundOnCopyDescription;

  /// Settings row title showing the installed app version. Noun.
  ///
  /// In en, this message translates to:
  /// **'Version'**
  String get settingsVersionTitle;

  /// Shown instead of the version number when it is unknown.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get settingsVersionUnavailable;

  /// Settings row title for checking and installing app updates.
  ///
  /// In en, this message translates to:
  /// **'App updates'**
  String get settingsUpdatesTitle;

  /// Search result description for application updates. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Check for and install CopyPaste updates.'**
  String get settingsUpdatesSearchDescription;

  /// Space-separated search keywords that find the application updates setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'version github release application'**
  String get settingsUpdatesKeywords;

  /// Application updates row description while checking for updates.
  ///
  /// In en, this message translates to:
  /// **'Checking for updates.'**
  String get settingsUpdateChecking;

  /// Application updates row description when no update exists and the version is unknown.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is up to date.'**
  String get settingsUpdateUpToDate;

  /// Application updates row description when no update exists.
  ///
  /// In en, this message translates to:
  /// **'Version {version} is up to date.'**
  String settingsUpdateVersionUpToDate(String version);

  /// Toast message after a manual update check. Under the title 'No updates available' it is the installed version; under 'Update available' it is the version found. No trailing period.
  ///
  /// In en, this message translates to:
  /// **'Version {version}'**
  String settingsUpdateVersion(String version);

  /// Application updates row description when a newer version exists.
  ///
  /// In en, this message translates to:
  /// **'Version {available} is available. This device has version {current}.'**
  String settingsUpdateAvailable(String available, String current);

  /// Application updates row description while an update downloads, with download progress in percent.
  ///
  /// In en, this message translates to:
  /// **'Downloading version {version} · {percent}%.'**
  String settingsUpdateDownloading(String version, int percent);

  /// Application updates row description while installed modules are prepared for the update.
  ///
  /// In en, this message translates to:
  /// **'Preparing installed module updates.'**
  String get settingsUpdatePreparingModules;

  /// Application updates row description while an update installs.
  ///
  /// In en, this message translates to:
  /// **'Installing version {version}.'**
  String settingsUpdateInstalling(String version);

  /// Application updates row description when the update needs user action or failed.
  ///
  /// In en, this message translates to:
  /// **'App updates need attention.'**
  String get settingsUpdateNeedsAttention;

  /// Button that downloads and installs an available update.
  ///
  /// In en, this message translates to:
  /// **'Update now'**
  String get settingsUpdateNowAction;

  /// Button that opens the release web page for a manual update.
  ///
  /// In en, this message translates to:
  /// **'Open release'**
  String get settingsUpdateOpenReleaseAction;

  /// Disabled button label while checking for updates.
  ///
  /// In en, this message translates to:
  /// **'Checking'**
  String get settingsUpdateCheckingAction;

  /// Disabled button label while an update downloads.
  ///
  /// In en, this message translates to:
  /// **'Downloading'**
  String get settingsUpdateDownloadingAction;

  /// Disabled button label while an update installs.
  ///
  /// In en, this message translates to:
  /// **'Installing'**
  String get settingsUpdateInstallingAction;

  /// Button that restarts the app to finish an update. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Restart CopyPaste'**
  String get settingsUpdateRestartAction;

  /// Button that checks for app updates again.
  ///
  /// In en, this message translates to:
  /// **'Check again'**
  String get settingsUpdateCheckAgainAction;

  /// Button that checks for app updates when no check has run yet. After a check it reads settingsUpdateCheckAgainAction ("Check again").
  ///
  /// In en, this message translates to:
  /// **'Check now'**
  String get settingsUpdateCheckAction;

  /// Toast title after an update check finds no update.
  ///
  /// In en, this message translates to:
  /// **'No updates available'**
  String get settingsUpdateNoneTitle;

  /// Toast title after an update check finds a newer version.
  ///
  /// In en, this message translates to:
  /// **'Update available'**
  String get settingsUpdateAvailableTitle;

  /// Toast title when this installation cannot update itself.
  ///
  /// In en, this message translates to:
  /// **'Update unavailable'**
  String get settingsUpdateUnavailableTitle;

  /// Toast message when this installation cannot update itself.
  ///
  /// In en, this message translates to:
  /// **'This installation cannot update automatically.'**
  String get settingsUpdateUnavailableMessage;

  /// Toast title when checking for updates failed.
  ///
  /// In en, this message translates to:
  /// **'Update check failed'**
  String get settingsUpdateCheckFailedTitle;

  /// Linux alert title asking the user to finish Quick Paste setup. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Set up Quick Paste'**
  String get settingsQuickPasteSetupTitle;

  /// Linux alert message explaining Quick Paste setup. GNOME and KDE are desktop environment names.
  ///
  /// In en, this message translates to:
  /// **'Enable the signed GNOME or KDE companion and allow keyboard control to return to the previous app.'**
  String get settingsQuickPasteSetupMessage;

  /// Linux button that opens the desktop companion setup.
  ///
  /// In en, this message translates to:
  /// **'Desktop integration'**
  String get settingsQuickPasteDesktopIntegration;

  /// Linux button that requests permission to send keystrokes to other apps.
  ///
  /// In en, this message translates to:
  /// **'Allow keyboard control'**
  String get settingsQuickPasteAllowKeyboard;

  /// Linux button that checks the Quick Paste desktop integration state again.
  ///
  /// In en, this message translates to:
  /// **'Check again'**
  String get settingsQuickPasteCheckAgain;

  /// Alert title when the device does not support Quick Paste. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste is unavailable'**
  String get settingsQuickPasteUnavailableTitle;

  /// Alert message when the device does not support Quick Paste. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'This device does not provide the Quick Paste window.'**
  String get settingsQuickPasteUnavailableMessage;

  /// Settings row title for the global shortcut that opens Quick Paste. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Open Quick Paste'**
  String get settingsQuickPasteShortcutTitle;

  /// Search result description for the Quick Paste shortcut. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Configure the global Quick Paste shortcut.'**
  String get settingsQuickPasteShortcutSearchDescription;

  /// Space-separated search keywords that find the Quick Paste shortcut setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'keyboard hotkey'**
  String get settingsQuickPasteShortcutKeywords;

  /// Description of the Quick Paste shortcut row. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Works while CopyPaste runs in the background.'**
  String get settingsQuickPasteShortcutDescription;

  /// Description of the Quick Paste shortcut row showing the shortcut registered with the system.
  ///
  /// In en, this message translates to:
  /// **'Registered shortcut: {shortcut}'**
  String settingsQuickPasteRegisteredShortcut(String shortcut);

  /// Settings row title for pasting the selected clip into the previous app.
  ///
  /// In en, this message translates to:
  /// **'Paste automatically'**
  String get settingsAutoPasteTitle;

  /// Search result description for automatic paste.
  ///
  /// In en, this message translates to:
  /// **'Paste the selected clip into the previous app.'**
  String get settingsAutoPasteSearchDescription;

  /// Space-separated search keywords that find the automatic paste setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'accessibility automatic'**
  String get settingsAutoPasteKeywords;

  /// Description of the automatic paste setting.
  ///
  /// In en, this message translates to:
  /// **'Selecting a clip returns to the previous app and pastes it.'**
  String get settingsAutoPasteDescription;

  /// Settings row title for masking Quick Paste clips while a password field has focus.
  ///
  /// In en, this message translates to:
  /// **'Hide previews in password fields'**
  String get settingsMaskPasswordFieldsTitle;

  /// One-line description of the Hide previews in password fields setting.
  ///
  /// In en, this message translates to:
  /// **'Clips are masked while a password field has focus.'**
  String get settingsMaskPasswordFieldsDescription;

  /// Settings row title for showing the Paste Queue tab in Quick Paste.
  ///
  /// In en, this message translates to:
  /// **'Queue tab'**
  String get settingsQuickPasteQueueTabTitle;

  /// Description of the setting that shows the Paste Queue tab in Quick Paste.
  ///
  /// In en, this message translates to:
  /// **'Show Paste Queue in Quick Paste.'**
  String get settingsQuickPasteQueueTabDescription;

  /// macOS alert title when the Accessibility permission is needed for automatic paste. Accessibility is the macOS permission name.
  ///
  /// In en, this message translates to:
  /// **'Accessibility is required'**
  String get settingsAccessibilityRequiredTitle;

  /// macOS alert message when the Accessibility permission is missing.
  ///
  /// In en, this message translates to:
  /// **'Until permission is granted, selecting a clip copies it without pasting.'**
  String get settingsAccessibilityRequiredMessage;

  /// Error alert title in Quick Paste settings. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste needs attention'**
  String get settingsQuickPasteAttentionTitle;

  /// Button label while waiting for the user to press a new keyboard shortcut.
  ///
  /// In en, this message translates to:
  /// **'Press shortcut…'**
  String get settingsShortcutRecording;

  /// Label of a global shortcut recorder button when no shortcut is assigned.
  ///
  /// In en, this message translates to:
  /// **'Not set'**
  String get settingsShortcutNotSet;

  /// Tooltip of the button that restores the default Quick Paste shortcut.
  ///
  /// In en, this message translates to:
  /// **'Reset shortcut'**
  String get settingsShortcutReset;

  /// Retention option: keep history for this many days.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 day} other{{count} days}}'**
  String settingsRetentionDays(int count);

  /// Error title on the Devices screen when the device list cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Devices are unavailable'**
  String get devicesUnavailableTitle;

  /// Heading of the card listing unpaired devices discovered on the local network.
  ///
  /// In en, this message translates to:
  /// **'Nearby devices'**
  String get devicesNearbyTitle;

  /// Verb. Button that searches the local network for nearby devices again.
  ///
  /// In en, this message translates to:
  /// **'Scan'**
  String get devicesScan;

  /// Alert title on the Devices screen when the scanner worked but the scanned QR code is not a CopyPaste pairing code. The message under it is pairingInvalidQr.
  ///
  /// In en, this message translates to:
  /// **'Wrong QR code'**
  String get devicesScanInvalidTitle;

  /// Label of the scan button while the local network search is running.
  ///
  /// In en, this message translates to:
  /// **'Scanning…'**
  String get devicesScanning;

  /// Empty state when no unpaired devices were found on the local network.
  ///
  /// In en, this message translates to:
  /// **'No devices nearby'**
  String get devicesNoneNearby;

  /// Label for the device the app is running on, shown on its card and as a badge in its details.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get devicesThisDevice;

  /// Badge in device details for a paired device that this device trusts.
  ///
  /// In en, this message translates to:
  /// **'Trusted'**
  String get devicesTrusted;

  /// Verb. Button next to a nearby device that starts pairing with it.
  ///
  /// In en, this message translates to:
  /// **'Connect'**
  String get devicesConnect;

  /// Network round-trip time shown on a device card; {ping} is a value such as '12 ms'.
  ///
  /// In en, this message translates to:
  /// **'Ping: {ping}'**
  String devicesCardPing(String ping);

  /// Same as the ping on a device card, split over two lines for narrow cards; keep the line break. {ping} is a value such as '12 ms'.
  ///
  /// In en, this message translates to:
  /// **'Ping:\n{ping}'**
  String devicesCardPingStacked(String ping);

  /// Title of the dialog for changing the name of the current device.
  ///
  /// In en, this message translates to:
  /// **'Rename this device'**
  String get devicesRenameDialogTitle;

  /// Placeholder of the text field for the device name in the rename dialog.
  ///
  /// In en, this message translates to:
  /// **'Device name'**
  String get devicesNamePlaceholder;

  /// Error shown when renaming the current device fails.
  ///
  /// In en, this message translates to:
  /// **'The device name could not be saved.'**
  String get devicesRenameFailed;

  /// Error shown when unpairing or revoking a device fails.
  ///
  /// In en, this message translates to:
  /// **'The device action could not be completed.'**
  String get devicesActionFailedMessage;

  /// Tooltip and screen reader label of the button that closes the device details panel.
  ///
  /// In en, this message translates to:
  /// **'Close device details'**
  String get devicesCloseDetails;

  /// Row label in device details for the form factor, such as Laptop or Phone.
  ///
  /// In en, this message translates to:
  /// **'Device type'**
  String get devicesDetailType;

  /// Row label in device details for the operating system name and version.
  ///
  /// In en, this message translates to:
  /// **'Operating system'**
  String get devicesDetailOperatingSystem;

  /// Row label in device details for the hardware model.
  ///
  /// In en, this message translates to:
  /// **'Model'**
  String get devicesDetailModel;

  /// Row label in device details for the installed app version. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste version'**
  String get devicesDetailAppVersion;

  /// Row label in device details for the sync protocol version number.
  ///
  /// In en, this message translates to:
  /// **'Protocol version'**
  String get devicesDetailProtocolVersion;

  /// Row label in device details for the connection status of a paired device.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get devicesDetailStatus;

  /// Row label for the network round-trip time to a device, in device and sync details.
  ///
  /// In en, this message translates to:
  /// **'Ping'**
  String get devicesDetailPing;

  /// Row label in device details for the date and time when the ping was measured.
  ///
  /// In en, this message translates to:
  /// **'Measured'**
  String get devicesDetailMeasured;

  /// Row label in device details for the date and time the device was last seen on the network.
  ///
  /// In en, this message translates to:
  /// **'Last seen'**
  String get devicesDetailLastSeen;

  /// Row label for the local network address and port of a device, in device and sync details.
  ///
  /// In en, this message translates to:
  /// **'LAN endpoint'**
  String get devicesDetailLanEndpoint;

  /// Row label in details of the current device for its unique identifier.
  ///
  /// In en, this message translates to:
  /// **'Device ID'**
  String get devicesDetailDeviceId;

  /// Row label in details of a paired device for the identifier of its pairing.
  ///
  /// In en, this message translates to:
  /// **'Pairing ID'**
  String get devicesDetailPairingId;

  /// Row label in device details for how the device information was obtained (self-reported, observed or measured).
  ///
  /// In en, this message translates to:
  /// **'Profile provenance'**
  String get devicesDetailProfileProvenance;

  /// Row label in device details for how much the device information is trusted (local, authenticated or unverified).
  ///
  /// In en, this message translates to:
  /// **'Profile trust'**
  String get devicesDetailProfileTrust;

  /// Row label in device details for the date and time the device information was last updated.
  ///
  /// In en, this message translates to:
  /// **'Profile updated'**
  String get devicesDetailProfileUpdated;

  /// Value shown in a details table when a piece of device or pairing information is not available.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get devicesValueUnavailable;

  /// Button in the current device details that opens the rename dialog.
  ///
  /// In en, this message translates to:
  /// **'Rename device'**
  String get devicesRenameDevice;

  /// Button in paired device details that starts synchronization with that device immediately.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get devicesSyncNow;

  /// Heading of the sync section in device details.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get devicesSyncTitle;

  /// Muted note under the device sync heading; switching sync or a content type off never deletes stored clips.
  ///
  /// In en, this message translates to:
  /// **'Turning something off keeps clips already here.'**
  String get devicesSyncDescription;

  /// Switch in this device's details that turns synchronization on or off for this device.
  ///
  /// In en, this message translates to:
  /// **'Sync this device'**
  String get devicesSyncThisDevice;

  /// Switch in a paired device's details that turns synchronization with that device on or off.
  ///
  /// In en, this message translates to:
  /// **'Sync with this device'**
  String get devicesSyncPeer;

  /// Switch in device details that pauses automatic sync for this device, or for this paired device; manual sends still work.
  ///
  /// In en, this message translates to:
  /// **'Pause automatic sync'**
  String get devicesSyncPause;

  /// Switch in device details that syncs text, including rich text and HTML.
  ///
  /// In en, this message translates to:
  /// **'Text'**
  String get devicesSyncText;

  /// Switch in device details that syncs images.
  ///
  /// In en, this message translates to:
  /// **'Images'**
  String get devicesSyncImages;

  /// Switch in device details that syncs files.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get devicesSyncFiles;

  /// Note in a paired device's details shown before the first sync identified the device, when its sync settings cannot be set yet.
  ///
  /// In en, this message translates to:
  /// **'Sync settings for this device appear after its first sync.'**
  String get devicesSyncAfterFirstSync;

  /// Value in device details meaning the information comes from this device itself; used as a trust level and as a status.
  ///
  /// In en, this message translates to:
  /// **'Local'**
  String get devicesLocal;

  /// Trust level in device details: the information was verified cryptographically.
  ///
  /// In en, this message translates to:
  /// **'Authenticated'**
  String get devicesTrustAuthenticated;

  /// Trust level in device details: the information was not verified.
  ///
  /// In en, this message translates to:
  /// **'Unverified'**
  String get devicesTrustUnverified;

  /// Provenance value in device details: the device reported this information about itself.
  ///
  /// In en, this message translates to:
  /// **'Self-reported'**
  String get devicesProvenanceSelfReported;

  /// Provenance value in device details: this device observed the information on the network.
  ///
  /// In en, this message translates to:
  /// **'Observed'**
  String get devicesProvenanceObserved;

  /// Provenance value in device details: the information was measured directly.
  ///
  /// In en, this message translates to:
  /// **'Measured'**
  String get devicesProvenanceMeasured;

  /// Connection status of a paired device: reachable now.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get devicesConnectionOnline;

  /// Connection status of a paired device: confirmed unreachable.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get devicesConnectionOffline;

  /// Connection status of a paired device on its card: seen on the network but not yet verified.
  ///
  /// In en, this message translates to:
  /// **'Visible'**
  String get devicesConnectionVisible;

  /// Connection status of a paired device: it has not been seen on the network.
  ///
  /// In en, this message translates to:
  /// **'Not seen'**
  String get devicesConnectionNotSeen;

  /// Connection status of a paired device on its card: the status cannot be determined.
  ///
  /// In en, this message translates to:
  /// **'Unknown'**
  String get devicesConnectionUnknown;

  /// Ping value shown when the network round-trip time is not measured. 'ms' is the abbreviation for milliseconds.
  ///
  /// In en, this message translates to:
  /// **'— ms'**
  String get devicesPingUnavailable;

  /// Ping value shown when the network round-trip time is below one millisecond. 'ms' is the abbreviation for milliseconds.
  ///
  /// In en, this message translates to:
  /// **'<1 ms'**
  String get devicesPingUnderOneMillisecond;

  /// Ping value in milliseconds. 'ms' is the abbreviation for milliseconds.
  ///
  /// In en, this message translates to:
  /// **'{milliseconds} ms'**
  String devicesPingMilliseconds(int milliseconds);

  /// Device type: a desktop computer.
  ///
  /// In en, this message translates to:
  /// **'Desktop'**
  String get devicesClassDesktop;

  /// Device type: a laptop computer.
  ///
  /// In en, this message translates to:
  /// **'Laptop'**
  String get devicesClassLaptop;

  /// Device type: a mobile phone.
  ///
  /// In en, this message translates to:
  /// **'Phone'**
  String get devicesClassPhone;

  /// Device type: a tablet.
  ///
  /// In en, this message translates to:
  /// **'Tablet'**
  String get devicesClassTablet;

  /// Device type used when the form factor is unknown.
  ///
  /// In en, this message translates to:
  /// **'Device'**
  String get devicesClassUnknown;

  /// Shown instead of the operating system name when it is unknown.
  ///
  /// In en, this message translates to:
  /// **'Unknown OS'**
  String get devicesUnknownOs;

  /// Header button and tooltip on the Devices screen that shows a pairing code and QR code for another device.
  ///
  /// In en, this message translates to:
  /// **'Add device'**
  String get devicesAddDevice;

  /// Header button and tooltip on the Devices screen that opens the camera to scan a pairing QR code.
  ///
  /// In en, this message translates to:
  /// **'Scan QR'**
  String get devicesScanQr;

  /// Header button and tooltip on the Devices screen that opens manual entry of a pairing code.
  ///
  /// In en, this message translates to:
  /// **'Enter code'**
  String get devicesEnterCode;

  /// Alert title on the Devices screen when the QR code scanner cannot be used.
  ///
  /// In en, this message translates to:
  /// **'Scanner unavailable'**
  String get devicesScannerUnavailableTitle;

  /// Alert title on the Devices screen when a device action such as rename, sync or unpair fails.
  ///
  /// In en, this message translates to:
  /// **'Action failed'**
  String get devicesActionFailedTitle;

  /// Alert title on the Devices screen when the device list cannot be updated.
  ///
  /// In en, this message translates to:
  /// **'Device refresh failed'**
  String get devicesRefreshFailedTitle;

  /// Error shown when pairing with a discovered nearby device is not supported.
  ///
  /// In en, this message translates to:
  /// **'Nearby pairing is unavailable.'**
  String get devicesNearbyPairingUnavailable;

  /// Error shown when pairing with a nearby device is not supported. 'Add device' is the name of a button; translate it the same way.
  ///
  /// In en, this message translates to:
  /// **'Nearby pairing is unavailable. Use Add device to show a pairing code.'**
  String get devicesNearbyPairingUnavailableUseAddDevice;

  /// Generic title of the pairing panel when no specific pairing step applies.
  ///
  /// In en, this message translates to:
  /// **'Pairing'**
  String get pairingTitle;

  /// Title of the pairing panel that shows a code and QR code for another device to join.
  ///
  /// In en, this message translates to:
  /// **'Pair a device'**
  String get pairingPairDevice;

  /// Title of the pairing panel for scanning a QR code, and button that opens the camera scanner.
  ///
  /// In en, this message translates to:
  /// **'Scan QR code'**
  String get pairingScanQrCode;

  /// Title of the pairing panel for typing a code shown on another device.
  ///
  /// In en, this message translates to:
  /// **'Enter pairing code'**
  String get pairingEnterPairingCode;

  /// Title of the pairing panel when pairing with a device discovered on the local network.
  ///
  /// In en, this message translates to:
  /// **'Connect nearby device'**
  String get pairingConnectNearbyDevice;

  /// Loading message while a pairing request is sent to a nearby device.
  ///
  /// In en, this message translates to:
  /// **'Sending connection request…'**
  String get pairingSendingRequest;

  /// Loading message while a pairing code and QR code are created.
  ///
  /// In en, this message translates to:
  /// **'Creating a pairing invitation.'**
  String get pairingCreatingInvitation;

  /// Label and placeholder of the field for the code that pairs two devices, and row label for that code.
  ///
  /// In en, this message translates to:
  /// **'Pairing code'**
  String get pairingCode;

  /// Help text under the pairing code field: where to find the code. The panel title and the field label already ask for the pairing code.
  ///
  /// In en, this message translates to:
  /// **'Shown on the other device.'**
  String get pairingCodeHint;

  /// Label of the field for the network address of the device to pair with.
  ///
  /// In en, this message translates to:
  /// **'Device address'**
  String get pairingDeviceAddress;

  /// Help text under the device address field. Keep 'host:port' as is.
  ///
  /// In en, this message translates to:
  /// **'Use the host:port address shown on the other device.'**
  String get pairingDeviceAddressHint;

  /// Alert title when the entered pairing code or address is incomplete.
  ///
  /// In en, this message translates to:
  /// **'Check pairing details'**
  String get pairingCheckDetailsTitle;

  /// Error title when a pairing step fails.
  ///
  /// In en, this message translates to:
  /// **'Pairing action failed'**
  String get pairingActionFailed;

  /// Loading message while the system QR scanner is open or the scanned device is being connected.
  ///
  /// In en, this message translates to:
  /// **'Connecting device…'**
  String get pairingConnectingDevice;

  /// Message in the placeholder shown before the camera preview starts.
  ///
  /// In en, this message translates to:
  /// **'The camera preview appears here.'**
  String get pairingCameraPreviewPlaceholder;

  /// Validation error when the pairing code has fewer than 8 characters.
  ///
  /// In en, this message translates to:
  /// **'Enter the complete 8-character pairing code.'**
  String get pairingCodeIncomplete;

  /// Error when pairing has no device address and no relay is configured. Keep 'host:port' as is.
  ///
  /// In en, this message translates to:
  /// **'Enter the device host:port address, or set up a relay server in Settings.'**
  String get pairingAddressMissing;

  /// Pairing status after this device confirmed and the other device has not confirmed yet.
  ///
  /// In en, this message translates to:
  /// **'Waiting for the other device'**
  String get pairingWaitingForOtherDevice;

  /// Instructions next to a pairing code. 'Enter code' and 'Scan QR' are button names; translate them exactly as devicesEnterCode and devicesScanQr.
  ///
  /// In en, this message translates to:
  /// **'On the other device, choose Enter code or Scan QR.'**
  String get pairingInviteInstructions;

  /// Row label for the network address another device uses to pair.
  ///
  /// In en, this message translates to:
  /// **'Address'**
  String get pairingAddress;

  /// Heading of the code both devices show so the user can compare them before confirming.
  ///
  /// In en, this message translates to:
  /// **'Verification code'**
  String get pairingVerificationCode;

  /// Help text under the verification code.
  ///
  /// In en, this message translates to:
  /// **'Confirm only if this code matches on both devices.'**
  String get pairingVerificationCodeHint;

  /// Verb. Button that refuses the pairing when the verification codes do not match.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get pairingReject;

  /// Verb. Button that accepts the pairing after comparing verification codes.
  ///
  /// In en, this message translates to:
  /// **'Confirm'**
  String get pairingConfirm;

  /// Verb. Button in the protected pairing window that accepts the pairing after comparing verification codes.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get pairingAccept;

  /// Button that pairs with another device using the entered code and address.
  ///
  /// In en, this message translates to:
  /// **'Join device'**
  String get pairingJoinDevice;

  /// Button that starts the camera to scan a pairing QR code.
  ///
  /// In en, this message translates to:
  /// **'Start scanner'**
  String get pairingStartScanner;

  /// Pairing status title while the two devices establish a secure connection.
  ///
  /// In en, this message translates to:
  /// **'Verifying device'**
  String get pairingStateHandshaking;

  /// Pairing status title asking the user to confirm the pairing on both devices.
  ///
  /// In en, this message translates to:
  /// **'Confirm on both devices'**
  String get pairingStateAwaitingConfirmation;

  /// Pairing status title after pairing succeeded.
  ///
  /// In en, this message translates to:
  /// **'Device paired'**
  String get pairingStateConfirmed;

  /// Pairing status title after one device rejected the pairing.
  ///
  /// In en, this message translates to:
  /// **'Pairing rejected'**
  String get pairingStateRejected;

  /// Pairing status title after the pairing was cancelled.
  ///
  /// In en, this message translates to:
  /// **'Pairing cancelled'**
  String get pairingStateCancelled;

  /// Pairing status title after the pairing expired.
  ///
  /// In en, this message translates to:
  /// **'Pairing timed out'**
  String get pairingStateTimedOut;

  /// Pairing status title after the pairing failed.
  ///
  /// In en, this message translates to:
  /// **'Pairing failed'**
  String get pairingStateFailed;

  /// Pairing status description while the two devices establish a secure connection.
  ///
  /// In en, this message translates to:
  /// **'The devices are establishing a secure channel.'**
  String get pairingHandshakingDescription;

  /// Pairing status description after pairing succeeded, shown on both devices: the devices already in the space and the newly paired one reach each other without pairing again. 'Space' is the user's set of paired devices; use the same word as devicesSpaceTitle.
  ///
  /// In en, this message translates to:
  /// **'Other devices in your space connect automatically.'**
  String get pairingConfirmedDescription;

  /// Pairing status description after pairing was rejected, cancelled or failed.
  ///
  /// In en, this message translates to:
  /// **'No device was added.'**
  String get pairingNoDeviceAdded;

  /// Pairing status description after the pairing expired.
  ///
  /// In en, this message translates to:
  /// **'Start a new pairing attempt to continue.'**
  String get pairingTimedOutDescription;

  /// Pairing status description before any pairing attempt started.
  ///
  /// In en, this message translates to:
  /// **'Start a pairing attempt to connect a device.'**
  String get pairingIdleDescription;

  /// Error title when the screen-capture-protected pairing window cannot be used.
  ///
  /// In en, this message translates to:
  /// **'Protected pairing is unavailable'**
  String get pairingProtectedUnavailableTitle;

  /// Error message when the screen-capture-protected pairing window cannot be used.
  ///
  /// In en, this message translates to:
  /// **'This pairing session must open in a dedicated protected window.'**
  String get pairingProtectedUnavailableMessage;

  /// Button in the protected pairing window that stops the pairing.
  ///
  /// In en, this message translates to:
  /// **'Cancel pairing'**
  String get pairingCancel;

  /// Screen reader label of the button that closes the pairing panel on the Devices screen; closing also stops a pairing in progress.
  ///
  /// In en, this message translates to:
  /// **'Close pairing'**
  String get pairingClose;

  /// Heading in the protected pairing window above the field for typing a pairing code instead of scanning.
  ///
  /// In en, this message translates to:
  /// **'Or enter a pairing code'**
  String get pairingOrEnterCode;

  /// Button in the protected pairing window that pairs using the typed code.
  ///
  /// In en, this message translates to:
  /// **'Join securely'**
  String get pairingJoinSecurely;

  /// Description in the protected pairing window while waiting for the user to choose a pairing method.
  ///
  /// In en, this message translates to:
  /// **'Reveal a QR code, scan one, or enter a pairing code in this protected window.'**
  String get pairingProtectedWaitingDescription;

  /// Badge with the time left before the pairing code expires; 's' is the abbreviation for seconds.
  ///
  /// In en, this message translates to:
  /// **'Expires in {seconds}s'**
  String pairingExpiresInSeconds(int seconds);

  /// Error when the scanned QR code is not a pairing code. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Scan a CopyPaste pairing QR code.'**
  String get pairingInvalidQr;

  /// Error on Android when the Google code scanner cannot be used. Google is a brand name.
  ///
  /// In en, this message translates to:
  /// **'Google scanner is unavailable. Enter the pairing code instead.'**
  String get pairingGoogleScannerUnavailable;

  /// Error when the QR code scanner fails to open.
  ///
  /// In en, this message translates to:
  /// **'The scanner could not open. Enter the pairing code instead.'**
  String get pairingScannerCouldNotOpen;

  /// Error when the app cannot protect the pairing screen from screenshots and screen recording.
  ///
  /// In en, this message translates to:
  /// **'Secure pairing presentation is unavailable.'**
  String get pairingSecurePresentationUnavailable;

  /// Error when the user tries to confirm pairing before the verification code is shown.
  ///
  /// In en, this message translates to:
  /// **'Reveal and compare the verification code first.'**
  String get pairingVerificationCodeRequired;

  /// Synchronization status when the app cannot read the sync state.
  ///
  /// In en, this message translates to:
  /// **'Sync state unavailable'**
  String get syncStatusUnavailable;

  /// Synchronization status when sync is turned off.
  ///
  /// In en, this message translates to:
  /// **'Sync off'**
  String get syncStatusOff;

  /// Synchronization status while waiting for the next synchronization.
  ///
  /// In en, this message translates to:
  /// **'Waiting for sync'**
  String get syncStatusWaitingForSync;

  /// Synchronization status while synchronization is running.
  ///
  /// In en, this message translates to:
  /// **'Syncing'**
  String get syncStatusSyncing;

  /// Synchronization status when all clipboard history is synchronized.
  ///
  /// In en, this message translates to:
  /// **'Up to date'**
  String get syncStatusUpToDate;

  /// Synchronization status after synchronization stopped because of an error.
  ///
  /// In en, this message translates to:
  /// **'Sync interrupted'**
  String get syncStatusInterrupted;

  /// Synchronization status when some items could not be synchronized.
  ///
  /// In en, this message translates to:
  /// **'Partially synced'**
  String get syncStatusPartiallySynchronized;

  /// Synchronization status while a failed synchronization is retried automatically.
  ///
  /// In en, this message translates to:
  /// **'Retrying'**
  String get syncStatusRetrying;

  /// Synchronization status of a device when some items are queued for synchronization.
  ///
  /// In en, this message translates to:
  /// **'Waiting to sync'**
  String get syncStatusWaitingToSync;

  /// Synchronization status of a device whose network address is not known yet.
  ///
  /// In en, this message translates to:
  /// **'Waiting for device'**
  String get syncStatusWaitingForDevice;

  /// Synchronization status and empty state title when no devices are paired.
  ///
  /// In en, this message translates to:
  /// **'No paired devices'**
  String get syncStatusNoPairedDevices;

  /// Synchronization status when all paired devices are offline.
  ///
  /// In en, this message translates to:
  /// **'Devices offline'**
  String get syncStatusDevicesOffline;

  /// Synchronization status followed by a note that the sync activity log cannot be read; {status} is another synchronization status.
  ///
  /// In en, this message translates to:
  /// **'{status} · activity log unavailable'**
  String syncStatusWithActivityLogUnavailable(String status);

  /// Synchronization status when sync is turned off and some devices have problems.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Sync off · 1 issue} other{Sync off · {count} issues}}'**
  String syncStatusOffWithIssues(int count);

  /// Synchronization status followed by how many paired devices are offline; {status} is another synchronization status.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{{status} · 1 offline} other{{status} · {count} offline}}'**
  String syncStatusWithOfflineDevices(String status, int count);

  /// Synchronization status when some paired devices have problems that block synchronization.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 device needs attention} other{{count} devices need attention}}'**
  String syncStatusDevicesNeedAttention(int count);

  /// Synchronization status when all paired devices are offline and some have problems.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Devices offline · 1 issue} other{Devices offline · {count} issues}}'**
  String syncStatusDevicesOfflineWithIssues(int count);

  /// Connection status of a paired device while the connection is being checked.
  ///
  /// In en, this message translates to:
  /// **'Checking connection'**
  String get syncConnectionChecking;

  /// Connection status of a paired device when it cannot be determined.
  ///
  /// In en, this message translates to:
  /// **'Connection unknown'**
  String get syncConnectionUnknown;

  /// Synchronization activity log entry: a paired device went offline.
  ///
  /// In en, this message translates to:
  /// **'Device went offline.'**
  String get syncLogDeviceOffline;

  /// Synchronization activity log entry: a paired device is reachable again.
  ///
  /// In en, this message translates to:
  /// **'Connection restored.'**
  String get syncLogConnectionRestored;

  /// Synchronization activity log entry: a synchronization problem appeared.
  ///
  /// In en, this message translates to:
  /// **'Sync problem detected.'**
  String get syncLogProblemDetected;

  /// Synchronization activity log entry: a synchronization problem was resolved.
  ///
  /// In en, this message translates to:
  /// **'Sync problem resolved.'**
  String get syncLogProblemResolved;

  /// Synchronization activity log entry: synchronization failed.
  ///
  /// In en, this message translates to:
  /// **'Sync failed.'**
  String get syncLogFailed;

  /// Synchronization activity log entry: synchronization works again after a failure.
  ///
  /// In en, this message translates to:
  /// **'Sync restored.'**
  String get syncLogRecovered;

  /// Synchronization activity log entry with how many clipboard items (clips) were sent to and received from a device.
  ///
  /// In en, this message translates to:
  /// **'Sent {sent, plural, =1{1 clip} other{{sent} clips}} · Received {received, plural, =1{1 clip} other{{received} clips}}'**
  String syncLogClipsTransferred(int sent, int received);

  /// Screen reader label of the header button that opens synchronization details; {status} is the current synchronization status.
  ///
  /// In en, this message translates to:
  /// **'Sync: {status}'**
  String syncHeaderSemanticsLabel(String status);

  /// Title of the synchronization details panel.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get syncDetailsTitle;

  /// Tooltip and screen reader label of the button that closes the synchronization details panel.
  ///
  /// In en, this message translates to:
  /// **'Close sync details'**
  String get syncDetailsClose;

  /// Empty state message in synchronization details when no devices are paired.
  ///
  /// In en, this message translates to:
  /// **'Pair a device to sync clipboard history.'**
  String get syncNoPairedDevicesMessage;

  /// Alert title in synchronization details when the sync activity log cannot be read or saved.
  ///
  /// In en, this message translates to:
  /// **'Activity log unavailable'**
  String get syncActivityLogUnavailableTitle;

  /// Alert message in synchronization details when the sync activity log cannot be read or saved.
  ///
  /// In en, this message translates to:
  /// **'Sync activity could not be read or saved.'**
  String get syncActivityLogUnavailableMessage;

  /// Section heading in synchronization details above the list of paired devices.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get syncDetailsDevicesHeading;

  /// Section heading in synchronization details above the recent synchronization events.
  ///
  /// In en, this message translates to:
  /// **'Activity'**
  String get syncDetailsActivityHeading;

  /// Shown in synchronization details when the activity log is empty.
  ///
  /// In en, this message translates to:
  /// **'No recent sync problems'**
  String get syncDetailsNoRecentProblems;

  /// Name shown in the synchronization activity log when the device that caused an event is no longer known.
  ///
  /// In en, this message translates to:
  /// **'Paired device'**
  String get syncDetailsUnknownPeer;

  /// Row label in synchronization details for the connection status of a paired device.
  ///
  /// In en, this message translates to:
  /// **'Connection'**
  String get syncPeerConnection;

  /// Row label in synchronization details for when the last synchronization check started.
  ///
  /// In en, this message translates to:
  /// **'Last check started'**
  String get syncPeerLastCheckStarted;

  /// Row label in synchronization details for the number of clipboard items too large to synchronize.
  ///
  /// In en, this message translates to:
  /// **'Clips over the size limit'**
  String get syncPeerItemsOverSizeLimit;

  /// Shown next to an offline device's status; {time} is a relative or short time such as '5 min ago' or '14:20'.
  ///
  /// In en, this message translates to:
  /// **'Last seen {time}'**
  String syncPeerLastSeen(String time);

  /// Shown next to a device's synchronization status; {time} is a relative or short time such as '5 min ago' or '14:20'.
  ///
  /// In en, this message translates to:
  /// **'Last synced {time}'**
  String syncPeerLastSynced(String time);

  /// Row label in synchronization details for the date and time of the last successful synchronization.
  ///
  /// In en, this message translates to:
  /// **'Last successful sync'**
  String get syncPeerLastSuccessfulSync;

  /// Tooltip of the button that expands the connection details of a paired device in synchronization details.
  ///
  /// In en, this message translates to:
  /// **'Connection details'**
  String get syncPeerConnectionDetails;

  /// Screen reader label of the button that expands the connection details of a paired device; {device} is the device name.
  ///
  /// In en, this message translates to:
  /// **'Connection details for {device}'**
  String syncPeerConnectionDetailsFor(String device);

  /// Clip type label and History filter option for plain text clips. Noun.
  ///
  /// In en, this message translates to:
  /// **'Text'**
  String get historyKindText;

  /// Clip type label and History filter option for clips that contain a web link. Noun.
  ///
  /// In en, this message translates to:
  /// **'Link'**
  String get historyKindLink;

  /// Clip type label and History filter option for clips that contain an email address. Noun.
  ///
  /// In en, this message translates to:
  /// **'Email'**
  String get historyKindEmail;

  /// Clip type label and History filter option for clips that contain a color value (for example #FF0000). Noun.
  ///
  /// In en, this message translates to:
  /// **'Color'**
  String get historyKindColor;

  /// Clip type label and History filter option for clips that contain a phone number. Noun.
  ///
  /// In en, this message translates to:
  /// **'Phone'**
  String get historyKindPhone;

  /// Clip type label and History filter option for source code clips; also the code badge when the programming language is unknown. Noun.
  ///
  /// In en, this message translates to:
  /// **'Code'**
  String get historyKindCode;

  /// Clip type label and History filter option for clips that contain a file system path. Noun.
  ///
  /// In en, this message translates to:
  /// **'Path'**
  String get historyKindPath;

  /// Clip type label and History filter option for clips that look like passwords, tokens, or other secrets. Noun.
  ///
  /// In en, this message translates to:
  /// **'Credentials'**
  String get historyKindCredentials;

  /// Clip type label and History filter option for image clips. Noun.
  ///
  /// In en, this message translates to:
  /// **'Image'**
  String get historyKindImage;

  /// Clip type label and History filter option for copied files. Noun.
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get historyKindFile;

  /// Clip type label and History filter option for clips of any other content type. Noun.
  ///
  /// In en, this message translates to:
  /// **'Other'**
  String get historyKindOther;

  /// History sort option that lists the most recent clips first.
  ///
  /// In en, this message translates to:
  /// **'Newest first'**
  String get historySortNewest;

  /// History sort option that lists the oldest clips first.
  ///
  /// In en, this message translates to:
  /// **'Oldest first'**
  String get historySortOldest;

  /// History sort option, shown only while searching, that lists clips by search relevance.
  ///
  /// In en, this message translates to:
  /// **'Best match'**
  String get historySortRelevance;

  /// Error shown in History when live updates from the clipboard service stop.
  ///
  /// In en, this message translates to:
  /// **'History updates are temporarily unavailable.'**
  String get historyErrorUpdatesUnavailable;

  /// Error shown after deleting several selected clips when some deletions failed.
  ///
  /// In en, this message translates to:
  /// **'Some selected clips could not be deleted. Try again.'**
  String get historyErrorBulkDelete;

  /// Error shown after pinning or unpinning several selected clips when some changes failed.
  ///
  /// In en, this message translates to:
  /// **'Some selected pins could not be updated. Try again.'**
  String get historyErrorBulkPin;

  /// Error shown when the History list could not be reloaded after a change.
  ///
  /// In en, this message translates to:
  /// **'History could not be refreshed. Try again.'**
  String get historyErrorRefresh;

  /// Error shown when the next page of older clips fails to load while scrolling down.
  ///
  /// In en, this message translates to:
  /// **'More history could not be loaded.'**
  String get historyErrorLoadMore;

  /// Error shown when a previously scrolled page of clips fails to load again while scrolling back.
  ///
  /// In en, this message translates to:
  /// **'Earlier history could not be loaded.'**
  String get historyErrorLoadEarlier;

  /// Error shown when the details of the selected clip fail to load.
  ///
  /// In en, this message translates to:
  /// **'This clip could not be opened.'**
  String get historyErrorOpenClip;

  /// Error shown when copying a clip as plain text (without formatting) fails.
  ///
  /// In en, this message translates to:
  /// **'Plain text could not be copied.'**
  String get historyErrorCopyPlainText;

  /// Error shown when copying a clip back to the clipboard fails.
  ///
  /// In en, this message translates to:
  /// **'This clip could not be copied.'**
  String get historyErrorCopyClip;

  /// Error shown when copying text selected inside a clip fails.
  ///
  /// In en, this message translates to:
  /// **'This text could not be copied.'**
  String get historyErrorCopyText;

  /// Error shown when a file clip from another device could not be downloaded before copying it.
  ///
  /// In en, this message translates to:
  /// **'This file could not be downloaded. Check the source device and try again.'**
  String get historyErrorPrepareFile;

  /// Error shown when saving a file clip to this device fails.
  ///
  /// In en, this message translates to:
  /// **'This file could not be downloaded.'**
  String get historyErrorDownloadFile;

  /// Error shown when the system file picker for importing files into History fails.
  ///
  /// In en, this message translates to:
  /// **'Files could not be selected. Try again.'**
  String get historyErrorSelectFiles;

  /// Reason shown when a file chosen for import into History cannot be read.
  ///
  /// In en, this message translates to:
  /// **'The selected file could not be read. Check access and available storage.'**
  String get historyErrorReadSelectedFile;

  /// Error after importing files into History when some imports failed. {failed} is the failed count, {total} the number of chosen files, {name} the first failed file name, {reason} the first failure reason (a full sentence).
  ///
  /// In en, this message translates to:
  /// **'{total, plural, =1{{failed} of 1 file could not be imported. First: {name}. {reason}} other{{failed} of {total} files could not be imported. First: {name}. {reason}}}'**
  String historyErrorImportFiles(
    int total,
    int failed,
    String name,
    String reason,
  );

  /// Error shown when adding or removing the spoiler (hidden content) state of a clip fails.
  ///
  /// In en, this message translates to:
  /// **'The spoiler could not be updated.'**
  String get historyErrorSecrecyUpdate;

  /// Error shown when pinning or unpinning a clip fails.
  ///
  /// In en, this message translates to:
  /// **'The pin could not be updated.'**
  String get historyErrorPinUpdate;

  /// Error shown when the new order of pinned clips could not be saved after reordering.
  ///
  /// In en, this message translates to:
  /// **'The pinned order could not be saved. Try again.'**
  String get historyErrorPinnedOrderSave;

  /// Error shown when pinned clips were reordered successfully but the list could not be reloaded.
  ///
  /// In en, this message translates to:
  /// **'The order was saved, but history could not be refreshed.'**
  String get historyErrorPinnedOrderRefresh;

  /// Error shown when deleting a single clip fails.
  ///
  /// In en, this message translates to:
  /// **'This clip could not be deleted.'**
  String get historyErrorDeleteClip;

  /// Error shown when clearing the whole clipboard history fails.
  ///
  /// In en, this message translates to:
  /// **'History could not be deleted.'**
  String get historyErrorDeleteAll;

  /// Error shown in the delete confirmation dialog when deleting the selected clips fails.
  ///
  /// In en, this message translates to:
  /// **'The selected clips could not be deleted.'**
  String get historyErrorBulkDeleteFallback;

  /// Error shown when the optional text recognition (OCR) module is not installed or not enabled.
  ///
  /// In en, this message translates to:
  /// **'OCR is unavailable. Check the module in Settings and try again.'**
  String get historyOcrErrorUnavailable;

  /// Error shown when the text recognition (OCR) module fails to start.
  ///
  /// In en, this message translates to:
  /// **'OCR could not be started.'**
  String get historyOcrErrorStart;

  /// Error shown when text recognition (OCR) on an image clip fails.
  ///
  /// In en, this message translates to:
  /// **'Image text could not be recognized.'**
  String get historyOcrErrorRecognize;

  /// Loading message in the image text dialog while text recognition (OCR) runs.
  ///
  /// In en, this message translates to:
  /// **'Recognizing image text.'**
  String get historyOcrRecognizing;

  /// Error title in the image text dialog when text recognition (OCR) fails.
  ///
  /// In en, this message translates to:
  /// **'OCR failed'**
  String get historyOcrFailedTitle;

  /// Empty-state title in the image text dialog when no text was recognized in the image.
  ///
  /// In en, this message translates to:
  /// **'No text found'**
  String get historyOcrNoText;

  /// Title of the dialog that shows text recognized in an image clip.
  ///
  /// In en, this message translates to:
  /// **'Image text'**
  String get historyOcrTitle;

  /// Toast message when copying the recognized image text fails.
  ///
  /// In en, this message translates to:
  /// **'Text could not be copied. Try again.'**
  String get historyOcrCopyError;

  /// Error shown in the tags dialog when the list of tags fails to load.
  ///
  /// In en, this message translates to:
  /// **'Tags could not be loaded. Try again.'**
  String get historyTagsErrorLoad;

  /// Error shown in the tags dialog when creating, renaming, deleting, or assigning a tag fails. Only pinned clips can have tags.
  ///
  /// In en, this message translates to:
  /// **'The tag change could not be saved. Use a unique name of up to 64 characters and select pinned clips.'**
  String get historyTagsErrorSave;

  /// Noun. Title of the tags dialog, label of the clip details button that opens it, screen reader label of the tag icon in clip metadata, and tag filter label when the selected tag is unknown.
  ///
  /// In en, this message translates to:
  /// **'Tags'**
  String get historyTags;

  /// Error title in the tags dialog.
  ///
  /// In en, this message translates to:
  /// **'Could not update tags'**
  String get historyTagsUpdateErrorTitle;

  /// Empty-state title in the tags dialog when no tags exist.
  ///
  /// In en, this message translates to:
  /// **'No tags yet'**
  String get historyTagsEmpty;

  /// Verb. Menu item in the tags dialog that renames a tag.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get historyTagRename;

  /// Tooltip for the menu button with actions (rename, delete) for one tag in the tags dialog.
  ///
  /// In en, this message translates to:
  /// **'Tag actions'**
  String get historyTagActions;

  /// Placeholder of the text field in the tags dialog for a new or renamed tag.
  ///
  /// In en, this message translates to:
  /// **'Tag name'**
  String get historyTagNamePlaceholder;

  /// Screen reader label of the button that creates a tag from the entered name.
  ///
  /// In en, this message translates to:
  /// **'Create tag'**
  String get historyCreateTag;

  /// Screen reader label of the button that saves a renamed tag.
  ///
  /// In en, this message translates to:
  /// **'Save tag'**
  String get historySaveTag;

  /// Tag filter option in the History toolbar that removes the tag filter.
  ///
  /// In en, this message translates to:
  /// **'All tags'**
  String get historyTagsFilterAll;

  /// Tag filter label in the History toolbar when several tags are selected.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 tag} other{{count} tags}}'**
  String historyTagsFilterCount(int count);

  /// Tooltip of the tag filter in the History toolbar. {selection} is the current filter label, such as a tag name or 'All tags'.
  ///
  /// In en, this message translates to:
  /// **'Filter by tags: {selection}'**
  String historyTagsFilterTooltip(String selection);

  /// Item at the bottom of the tag filter menu that opens the tags dialog.
  ///
  /// In en, this message translates to:
  /// **'Manage tags'**
  String get historyManageTags;

  /// Screen reader label of the tag filter in the History toolbar. Only pinned clips can have tags.
  ///
  /// In en, this message translates to:
  /// **'Filter pinned clips by tags'**
  String get historyTagsFilterSemantics;

  /// Error shown when downloading a file clip from another device stops without a specific reason.
  ///
  /// In en, this message translates to:
  /// **'The file download was cancelled.'**
  String get historyDownloadErrorCancelled;

  /// Error shown when the background service that downloads file clips is not available. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'The download service is unavailable. Reopen CopyPaste and try again.'**
  String get historyDownloadErrorServiceUnavailable;

  /// Error shown when the background service that downloads file clips does not respond in time.
  ///
  /// In en, this message translates to:
  /// **'The download service did not connect. Try again.'**
  String get historyDownloadErrorServiceTimeout;

  /// Error shown when a file chosen for import cannot be opened.
  ///
  /// In en, this message translates to:
  /// **'The selected file could not be accessed.'**
  String get historyFileErrorAccess;

  /// Screen reader label of the mask that hides confidential clip content; activating it reveals the content.
  ///
  /// In en, this message translates to:
  /// **'Confidential content. Reveal spoiler'**
  String get historySpoilerRevealSemantics;

  /// Title of the error toast when a History action such as copy or download fails.
  ///
  /// In en, this message translates to:
  /// **'Action failed'**
  String get historyActionFailedTitle;

  /// Error title shown instead of the History list when it cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'History is unavailable'**
  String get historyUnavailableTitle;

  /// Empty-state title when History has no clips or no clips match the search and filters.
  ///
  /// In en, this message translates to:
  /// **'No clips found'**
  String get historyEmptyTitle;

  /// Empty-state message when History has no clips or no clips match the search and filters.
  ///
  /// In en, this message translates to:
  /// **'Captured clips and matching history appear here.'**
  String get historyEmptyMessage;

  /// Title of the error banner at the end of the History list.
  ///
  /// In en, this message translates to:
  /// **'History needs attention'**
  String get historyAttentionTitle;

  /// Button at the end of the History list that loads older clips.
  ///
  /// In en, this message translates to:
  /// **'Load more'**
  String get historyLoadMore;

  /// Section heading in the History list above the pinned clips. Refers to several clips.
  ///
  /// In en, this message translates to:
  /// **'Pinned'**
  String get historySectionPinned;

  /// Section heading in the History list above clips captured today.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get historySectionToday;

  /// Section heading in the History list above clips captured yesterday.
  ///
  /// In en, this message translates to:
  /// **'Yesterday'**
  String get historySectionYesterday;

  /// Warning title in History when some stored clips could not be decrypted and are hidden.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip was skipped} other{{count} clips were skipped}}'**
  String historySkippedRowsTitle(int count);

  /// Warning message in History when some stored clips could not be decrypted and are hidden.
  ///
  /// In en, this message translates to:
  /// **'Some clips could not be read and are not shown in this list.'**
  String get historySkippedRowsMessage;

  /// Placeholder of the History search field and tooltip of the button that opens it.
  ///
  /// In en, this message translates to:
  /// **'Search history'**
  String get historySearch;

  /// Accessibility label of the popover with suggestions under the History search field.
  ///
  /// In en, this message translates to:
  /// **'Search suggestions'**
  String get historySearchSuggestions;

  /// Category shown at the end of a suggestion row that repeats an earlier History search.
  ///
  /// In en, this message translates to:
  /// **'Recent'**
  String get historySearchSuggestionRecent;

  /// Category shown at the end of a suggestion row that filters History by a tag.
  ///
  /// In en, this message translates to:
  /// **'Tag'**
  String get historySearchSuggestionTag;

  /// Category shown at the end of a suggestion row that filters History by a source application.
  ///
  /// In en, this message translates to:
  /// **'App'**
  String get historySearchSuggestionApp;

  /// Category shown at the end of a suggestion row that filters History by a content type, such as Link or Image.
  ///
  /// In en, this message translates to:
  /// **'Type'**
  String get historySearchSuggestionKind;

  /// Category shown at the end of a suggestion row that filters History by the device a clip came from.
  ///
  /// In en, this message translates to:
  /// **'Device'**
  String get historySearchSuggestionDevice;

  /// Row in the History search suggestions that deletes the recent searches stored on this device.
  ///
  /// In en, this message translates to:
  /// **'Clear recent searches'**
  String get historySearchClearRecent;

  /// Tooltip of the History toolbar button that clears the search and all filters.
  ///
  /// In en, this message translates to:
  /// **'Reset filters'**
  String get historyResetFilters;

  /// Tooltip of the History toolbar button that adds files from this device to the clipboard history.
  ///
  /// In en, this message translates to:
  /// **'Import files'**
  String get historyImportFiles;

  /// Tooltip of the History toolbar button that starts selecting several clips at once.
  ///
  /// In en, this message translates to:
  /// **'Select clips'**
  String get historySelectClips;

  /// Screen reader label of the button that clears and closes the compact History search field.
  ///
  /// In en, this message translates to:
  /// **'Close search'**
  String get historyCloseSearch;

  /// History filter option that shows clips of every type.
  ///
  /// In en, this message translates to:
  /// **'All clips'**
  String get historyFilterAllClips;

  /// History filter option that shows clips from every synced device.
  ///
  /// In en, this message translates to:
  /// **'All devices'**
  String get historyFilterAllDevices;

  /// History filter option that shows clips copied from any application.
  ///
  /// In en, this message translates to:
  /// **'All apps'**
  String get historyFilterAllApps;

  /// Tooltip of the button on a History row that pins the clip.
  ///
  /// In en, this message translates to:
  /// **'Pin clip'**
  String get historyPinClip;

  /// Tooltip of the button on a History row that unpins the clip.
  ///
  /// In en, this message translates to:
  /// **'Unpin clip'**
  String get historyUnpinClip;

  /// Tooltip of the drag handle on a pinned History row that changes its position.
  ///
  /// In en, this message translates to:
  /// **'Reorder pinned clip'**
  String get historyReorderPinnedClip;

  /// Screen reader hint for the pinned clip drag handle. Alt, Up and Down are keyboard keys (Up and Down arrows).
  ///
  /// In en, this message translates to:
  /// **'Drag to move. Alt + Up or Down moves one position.'**
  String get historyReorderPinnedHint;

  /// Error shown in place of a hidden clip's content when it fails to load.
  ///
  /// In en, this message translates to:
  /// **'Clip unavailable'**
  String get historyClipUnavailable;

  /// Screen reader label of the clip type icon on a History row. {kind} is a clip type such as Text or Image.
  ///
  /// In en, this message translates to:
  /// **'{kind} clip type'**
  String historyClipKindSemantics(String kind);

  /// Empty-state title of the clip details panel when no clip is selected.
  ///
  /// In en, this message translates to:
  /// **'Select a clip'**
  String get historyInspectorEmptyTitle;

  /// Empty-state message of the clip details panel when no clip is selected.
  ///
  /// In en, this message translates to:
  /// **'Its full content and actions appear here.'**
  String get historyInspectorEmptyMessage;

  /// Text shown in clip details when the clip has no displayable content.
  ///
  /// In en, this message translates to:
  /// **'No preview is available.'**
  String get historyNoPreview;

  /// Tooltip of the button that closes the clip details panel.
  ///
  /// In en, this message translates to:
  /// **'Close clip details'**
  String get historyCloseClipDetails;

  /// Screen reader label of the menu next to the Copy button with other copy formats.
  ///
  /// In en, this message translates to:
  /// **'Copy options'**
  String get historyCopyOptions;

  /// Menu item that copies the clip as plain text without formatting.
  ///
  /// In en, this message translates to:
  /// **'Copy plain text'**
  String get historyCopyPlainText;

  /// Label of the clip details pin button when the clip is already pinned. Describes the current state of one clip; pressing it unpins the clip.
  ///
  /// In en, this message translates to:
  /// **'Pinned'**
  String get historyPinnedButton;

  /// Button in clip details that hides the clip content behind a spoiler mask.
  ///
  /// In en, this message translates to:
  /// **'Add spoiler'**
  String get historyAddSpoiler;

  /// Button in clip details that stops hiding the clip content behind a spoiler mask.
  ///
  /// In en, this message translates to:
  /// **'Remove spoiler'**
  String get historyRemoveSpoiler;

  /// Title of the error toast when changing the spoiler state of a clip fails.
  ///
  /// In en, this message translates to:
  /// **'Update failed'**
  String get historyUpdateFailedTitle;

  /// Tooltip of the clip details button that extracts text from an image clip (OCR).
  ///
  /// In en, this message translates to:
  /// **'Recognize image text'**
  String get historyRecognizeImageText;

  /// Title of the success toast after a clip or text is copied to the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Copied'**
  String get historyCopiedTitle;

  /// Title of the error toast when copying text fails.
  ///
  /// In en, this message translates to:
  /// **'Copy failed'**
  String get historyCopyFailedTitle;

  /// Toast message under the title "Copied" after a clip is copied as plain text; it continues the title.
  ///
  /// In en, this message translates to:
  /// **'As plain text.'**
  String get historyPlainTextCopied;

  /// Title of the success toast after a file clip is saved to this device.
  ///
  /// In en, this message translates to:
  /// **'Downloaded'**
  String get historyDownloadedTitle;

  /// Verb. Button, tooltip and menu item that opens the system share sheet for a clip.
  ///
  /// In en, this message translates to:
  /// **'Share'**
  String get historyShare;

  /// Toast message when a clip could not be prepared for or passed to the system share sheet.
  ///
  /// In en, this message translates to:
  /// **'This clip could not be shared.'**
  String get historyErrorShareClip;

  /// Toast message when the clips selected in History could not be prepared for or passed to the system share sheet.
  ///
  /// In en, this message translates to:
  /// **'The selected clips could not be shared.'**
  String get historyErrorShareClips;

  /// Tooltip of the selection toolbar button that shares the selected clips in one share sheet.
  ///
  /// In en, this message translates to:
  /// **'Share selected'**
  String get historyBulkShare;

  /// Toast title when a share of selected clips sent only their files because the system cannot share text with files.
  ///
  /// In en, this message translates to:
  /// **'Shared files only'**
  String get historyBulkShareFilesOnlyTitle;

  /// Toast message explaining that the selected text clips were not shared with the files.
  ///
  /// In en, this message translates to:
  /// **'Text clips were left out because this system cannot share text together with files. Share them separately.'**
  String get historyBulkShareFilesOnly;

  /// Verb. Button and tooltip in clip details that saves a file clip to this device.
  ///
  /// In en, this message translates to:
  /// **'Download'**
  String get historyDownload;

  /// Status of a file clip download in progress when the progress is unknown.
  ///
  /// In en, this message translates to:
  /// **'Downloading…'**
  String get historyDownloading;

  /// Status of a file clip download in progress. {percent} is a whole number from 0 to 100.
  ///
  /// In en, this message translates to:
  /// **'Downloading {percent}%'**
  String historyDownloadingPercent(int percent);

  /// Download button label showing download progress. {percent} is a whole number from 0 to 100.
  ///
  /// In en, this message translates to:
  /// **'{percent}%'**
  String historyDownloadProgressPercent(int percent);

  /// Download button label while a file clip download is being cancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelling…'**
  String get historyDownloadCancelling;

  /// Tooltip of the download button while a file clip download is being cancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelling download'**
  String get historyDownloadCancellingTooltip;

  /// Tooltip of the download button during a download. {percent} is a whole number from 0 to 100.
  ///
  /// In en, this message translates to:
  /// **'Downloading {percent}% · Click to cancel'**
  String historyDownloadingCancelTooltip(int percent);

  /// Tooltip of the download button after a download failed.
  ///
  /// In en, this message translates to:
  /// **'Download failed · Click to retry'**
  String get historyDownloadFailedRetryTooltip;

  /// Screen reader label of the download button during a download. {percent} is a whole number from 0 to 100.
  ///
  /// In en, this message translates to:
  /// **'Cancel download, {percent}% complete'**
  String historyCancelDownloadSemantics(int percent);

  /// Screen reader label of the download button after a download failed.
  ///
  /// In en, this message translates to:
  /// **'Retry download'**
  String get historyRetryDownload;

  /// Button in clip details that opens a downloaded file.
  ///
  /// In en, this message translates to:
  /// **'Show file'**
  String get historyShowFile;

  /// Clip details row label for the date and time the clip was copied.
  ///
  /// In en, this message translates to:
  /// **'Captured'**
  String get historyMetadataCaptured;

  /// Clip details row label for the application the clip was copied from.
  ///
  /// In en, this message translates to:
  /// **'Observed app'**
  String get historyMetadataObservedApp;

  /// Clip details row label for the device the clip was copied on.
  ///
  /// In en, this message translates to:
  /// **'Device'**
  String get historyMetadataDevice;

  /// Clip details row label for the technical content type (MIME type), such as text/plain.
  ///
  /// In en, this message translates to:
  /// **'Type'**
  String get historyMetadataType;

  /// Clip details row label for the original location of a file on the source device.
  ///
  /// In en, this message translates to:
  /// **'Path'**
  String get historyMetadataPath;

  /// Clip details row label for the file name.
  ///
  /// In en, this message translates to:
  /// **'File'**
  String get historyMetadataFile;

  /// Clip details row label for image width and height in pixels.
  ///
  /// In en, this message translates to:
  /// **'Resolution'**
  String get historyMetadataResolution;

  /// Clip details row label for the image or file size.
  ///
  /// In en, this message translates to:
  /// **'Size'**
  String get historyMetadataSize;

  /// Clip details row label for the number of files in the clip.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get historyMetadataFiles;

  /// Clip details row label for the clip's sync status between devices. Noun.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get historyMetadataSync;

  /// Clip details value of the Sync row when the clip exceeds the sync size limit.
  ///
  /// In en, this message translates to:
  /// **'Too large'**
  String get historyMetadataTooLargeToSync;

  /// Title of the confirmation dialog for deleting the selected clips.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Delete 1 selected clip?} other{Delete {count} selected clips?}}'**
  String historyBulkDeleteTitle(int count);

  /// Title of the confirmation dialog for deleting one clip.
  ///
  /// In en, this message translates to:
  /// **'Delete this clip?'**
  String get historyDeleteClipTitle;

  /// Tooltip of the selection toolbar button that opens the tags dialog for the selected clips. Only pinned clips can have tags.
  ///
  /// In en, this message translates to:
  /// **'Tag selected pinned clips'**
  String get historyBulkTagTooltip;

  /// Tooltip of the selection toolbar button that stops selecting clips.
  ///
  /// In en, this message translates to:
  /// **'Exit selection'**
  String get historyBulkExit;

  /// Tooltip of the selection toolbar button that pins the selected clips.
  ///
  /// In en, this message translates to:
  /// **'Pin selected'**
  String get historyBulkPin;

  /// Tooltip of the selection toolbar button that unpins the selected clips.
  ///
  /// In en, this message translates to:
  /// **'Unpin selected'**
  String get historyBulkUnpin;

  /// Tooltip of the selection toolbar button that deletes the selected clips.
  ///
  /// In en, this message translates to:
  /// **'Delete selected'**
  String get historyBulkDelete;

  /// Selection toolbar label with the number of selected clips.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, other{{count} selected}}'**
  String historyBulkSelectedCount(int count);

  /// Tooltip of the History toolbar button that filters clips by capture date.
  ///
  /// In en, this message translates to:
  /// **'Filter by date'**
  String get historyDateFilter;

  /// Date filter preset button that shows clips captured today.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get historyDatePresetToday;

  /// Date filter preset button that shows clips captured yesterday.
  ///
  /// In en, this message translates to:
  /// **'Yesterday'**
  String get historyDatePresetYesterday;

  /// Date filter preset button that shows clips captured in the last {days} days, including today.
  ///
  /// In en, this message translates to:
  /// **'{days, plural, other{Last {days} days}}'**
  String historyDatePresetLastDays(int days);

  /// Date filter preset button that removes the date filter.
  ///
  /// In en, this message translates to:
  /// **'All time'**
  String get historyDatePresetAllTime;

  /// Error message shown in History, Devices, and Settings when the app's background service fails to start; a Try again button follows. Use the same term as the Diagnostics log source "Background service". CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste couldn\'t start its background service.'**
  String get appRuntimeStartFailed;

  /// Message under the loading indicator in History, Devices, and Settings while the app waits for the operating system to unlock its encrypted history. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is waiting for protected history access.'**
  String get appRuntimeWaiting;

  /// Error title shown in place of the History screen when the clipboard history service is not running.
  ///
  /// In en, this message translates to:
  /// **'History is unavailable'**
  String get appHistoryUnavailableTitle;

  /// Error title shown in place of the Devices screen when the sync service is not running.
  ///
  /// In en, this message translates to:
  /// **'Devices are unavailable'**
  String get appDevicesUnavailableTitle;

  /// Error title shown in place of the Settings screen when the app service is not running.
  ///
  /// In en, this message translates to:
  /// **'Settings are unavailable'**
  String get appSettingsUnavailableTitle;

  /// Button that exits the application. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Quit CopyPaste'**
  String get appQuit;

  /// Error title shown when the desktop window could not be configured.
  ///
  /// In en, this message translates to:
  /// **'Window setup needs attention'**
  String get appWindowSetupTitle;

  /// Screen reader label of the app logo button that opens the History screen. History is the name of the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'Open History'**
  String get appOpenHistory;

  /// Navigation label and screen title of the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get appDestinationHistory;

  /// Navigation label and screen title of the paired devices screen.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get appDestinationDevices;

  /// Navigation label and screen title of the app settings screen.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get appDestinationSettings;

  /// Screen reader label of the dot badge that marks a filter button whose filter is applied.
  ///
  /// In en, this message translates to:
  /// **'Active filter'**
  String get appActiveFilter;

  /// Title of the error shown inside a confirmation dialog when the confirmed action fails.
  ///
  /// In en, this message translates to:
  /// **'Action failed'**
  String get appActionFailed;

  /// Title shown in the Shizuku setup card when the device's Android version is too old for Shizuku.
  ///
  /// In en, this message translates to:
  /// **'Android 11 or newer is required'**
  String get sharedAndroidVersionRequired;

  /// Hint shown when Shizuku is unsupported, pointing to the ADB tab of the same setup card. ADB is a product name.
  ///
  /// In en, this message translates to:
  /// **'Use the ADB tab on this device.'**
  String get sharedUseAdbTab;

  /// Title shown in the Shizuku setup card after the Android permission was granted once.
  ///
  /// In en, this message translates to:
  /// **'One-time access applied'**
  String get sharedOneTimeAccessApplied;

  /// Title of the Shizuku setup card when the Shizuku app is not installed. Shizuku is an Android app name.
  ///
  /// In en, this message translates to:
  /// **'Install Shizuku'**
  String get sharedInstallShizuku;

  /// Title of the Shizuku setup card when Shizuku is installed but not running. Shizuku is an Android app name.
  ///
  /// In en, this message translates to:
  /// **'Pair and start Shizuku'**
  String get sharedPairAndStartShizuku;

  /// Title of the Shizuku setup card asking to grant CopyPaste permission inside Shizuku; the button under it reads "Allow" (sharedAllow). CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste'**
  String get sharedAllowCopyPaste;

  /// Description shown after the one-time Android access was applied. Shizuku is an Android app name.
  ///
  /// In en, this message translates to:
  /// **'Shizuku is no longer needed.'**
  String get sharedShizukuNoLongerNeeded;

  /// Description of the Shizuku setup card when Shizuku is not installed. Shizuku is an Android app name.
  ///
  /// In en, this message translates to:
  /// **'Shizuku applies the one-time access.'**
  String get sharedGetShizukuDescription;

  /// Description telling the user to start Shizuku with Android's Wireless debugging and come back. Wireless debugging is an Android developer setting name.
  ///
  /// In en, this message translates to:
  /// **'Use Wireless debugging in Shizuku, then return.'**
  String get sharedShizukuWirelessDebugging;

  /// Description telling the user to allow CopyPaste in the Shizuku app one time.
  ///
  /// In en, this message translates to:
  /// **'Approve CopyPaste once in Shizuku.'**
  String get sharedApproveInShizuku;

  /// Button that opens the store page of the Shizuku app.
  ///
  /// In en, this message translates to:
  /// **'Get Shizuku'**
  String get sharedGetShizuku;

  /// Button that opens the Shizuku app.
  ///
  /// In en, this message translates to:
  /// **'Open Shizuku'**
  String get sharedOpenShizuku;

  /// Heading above ADB commands that the user runs on a computer connected to the phone.
  ///
  /// In en, this message translates to:
  /// **'Run on your computer'**
  String get sharedRunOnComputer;

  /// Tooltip of the button that copies all ADB setup commands.
  ///
  /// In en, this message translates to:
  /// **'Copy all commands'**
  String get sharedCopyAllCommands;

  /// Instruction above ADB commands. USB debugging is an Android developer setting name.
  ///
  /// In en, this message translates to:
  /// **'Enable USB debugging and connect this phone.'**
  String get sharedEnableUsbDebugging;

  /// Button that grants a permission: it asks Android for one, such as notifications, or allows CopyPaste in Shizuku under the card title "Allow CopyPaste".
  ///
  /// In en, this message translates to:
  /// **'Allow'**
  String get sharedAllow;

  /// Button on the Shizuku setup card titled "Apply capture access" (onboardingAndroidApplyAccess) or "Apply SMS access" (modulesSmsApplyAccess). Use the verb of those two titles, alone.
  ///
  /// In en, this message translates to:
  /// **'Apply'**
  String get sharedApply;

  /// Error shown when copying the ADB setup commands to the clipboard fails.
  ///
  /// In en, this message translates to:
  /// **'Setup commands could not be copied.'**
  String get sharedSetupCommandsCopyFailed;

  /// Error shown when the Shizuku app could not be opened.
  ///
  /// In en, this message translates to:
  /// **'Shizuku could not be opened.'**
  String get sharedShizukuOpenFailed;

  /// File size in bytes; {size} is the number. B is the abbreviation for bytes.
  ///
  /// In en, this message translates to:
  /// **'{size} B'**
  String sharedSizeBytes(String size);

  /// Window and header title of the first-run setup flow. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Set up CopyPaste'**
  String get onboardingTitle;

  /// Heading of the first setup step. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Welcome to CopyPaste'**
  String get onboardingWelcomeTitle;

  /// Short tagline under the welcome heading of the first setup step.
  ///
  /// In en, this message translates to:
  /// **'Your clipboard, ready when you are.'**
  String get onboardingWelcomeSubtitle;

  /// Heading of the final setup step. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is ready'**
  String get onboardingReadyTitle;

  /// Button on the final setup step that finishes setup and opens the app.
  ///
  /// In en, this message translates to:
  /// **'Get started'**
  String get onboardingGetStarted;

  /// Title of a non-blocking warning shown during setup when an optional step did not succeed.
  ///
  /// In en, this message translates to:
  /// **'Optional setup needs attention'**
  String get onboardingOptionalSetupTitle;

  /// Button that opens the operating system settings app (System Settings on macOS, Settings on Windows).
  ///
  /// In en, this message translates to:
  /// **'Open Settings'**
  String get onboardingOpenSystemSettings;

  /// Title of the error shown under a setup step when the step failed.
  ///
  /// In en, this message translates to:
  /// **'Setup needs attention'**
  String get onboardingSetupErrorTitle;

  /// Error shown when the setup flow could not read the operating system setup state; {platform} is the operating system name, such as macOS.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not read the {platform} setup state.'**
  String onboardingStateReadFailed(String platform);

  /// Error shown when the setup flow could not store which step the user reached.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not save setup progress.'**
  String get onboardingSaveProgressFailed;

  /// Title of the setup row that launches the app automatically when the user signs in to the computer.
  ///
  /// In en, this message translates to:
  /// **'Start at login'**
  String get onboardingStartAtLogin;

  /// Screen reader label of the Start at login switch. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Start CopyPaste at login'**
  String get onboardingStartAtLoginSemantics;

  /// Non-blocking warning shown when the app could not register itself to start at login.
  ///
  /// In en, this message translates to:
  /// **'Start at login could not be enabled. You can continue without it.'**
  String get onboardingStartAtLoginEnableFailed;

  /// Non-blocking warning shown when changing the Start at login setting failed.
  ///
  /// In en, this message translates to:
  /// **'Start at login could not be updated. You can continue without it.'**
  String get onboardingStartAtLoginUpdateFailed;

  /// Heading of the macOS setup step.
  ///
  /// In en, this message translates to:
  /// **'Set up your Mac'**
  String get onboardingMacosTitle;

  /// Title of the macOS setup row for the Accessibility permission (the macOS privacy setting name).
  ///
  /// In en, this message translates to:
  /// **'Accessibility'**
  String get onboardingAccessibility;

  /// Status shown when the macOS Accessibility permission is granted, so the app can paste automatically.
  ///
  /// In en, this message translates to:
  /// **'Auto-paste enabled'**
  String get onboardingAutoPasteEnabled;

  /// Description of the macOS Accessibility row when the permission is not granted; it is only needed for automatic pasting.
  ///
  /// In en, this message translates to:
  /// **'Optional for auto-paste'**
  String get onboardingAutoPasteOptional;

  /// Description of the Start at login row on macOS when the app runs from a location that cannot register a login item.
  ///
  /// In en, this message translates to:
  /// **'Available in the installed CopyPaste app.'**
  String get onboardingMacosStartAtLoginUnavailable;

  /// Warning title on macOS when the user must approve the app as a Login Item. Login Item is the macOS term.
  ///
  /// In en, this message translates to:
  /// **'Login Item approval is required'**
  String get onboardingLoginItemApprovalTitle;

  /// Warning message on macOS asking the user to approve the login item in System Settings (the macOS app name).
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste in System Settings, then return.'**
  String get onboardingLoginItemApprovalMessage;

  /// Error shown when the macOS setup step could not read permissions and startup state.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not read the macOS setup state.'**
  String get onboardingMacosStateReadFailed;

  /// Error shown when asking macOS for the Accessibility permission failed.
  ///
  /// In en, this message translates to:
  /// **'Accessibility permission could not be requested.'**
  String get onboardingMacosAccessibilityRequestFailed;

  /// Error shown when re-reading macOS permissions and startup state failed.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not refresh the macOS setup state.'**
  String get onboardingMacosStateRefreshFailed;

  /// Non-blocking warning shown when the app could not re-check the macOS Accessibility permission.
  ///
  /// In en, this message translates to:
  /// **'Accessibility status could not be refreshed.'**
  String get onboardingMacosAccessibilityRefreshFailed;

  /// Error shown when the macOS Login Items settings page could not be opened.
  ///
  /// In en, this message translates to:
  /// **'Login Items settings could not be opened.'**
  String get onboardingMacosLoginItemsOpenFailed;

  /// Non-blocking warning shown when macOS needs the user to approve the login item in System Settings.
  ///
  /// In en, this message translates to:
  /// **'Start at login requires approval in System Settings.'**
  String get onboardingMacosStartAtLoginApproval;

  /// Non-blocking warning shown when the app could not remove itself from the macOS login items.
  ///
  /// In en, this message translates to:
  /// **'Start at login could not be disabled.'**
  String get onboardingMacosStartAtLoginDisableFailed;

  /// Heading of the Windows setup step.
  ///
  /// In en, this message translates to:
  /// **'Set up your PC'**
  String get onboardingWindowsTitle;

  /// Description of the Start at login row on Windows when this installation cannot register for startup.
  ///
  /// In en, this message translates to:
  /// **'Unavailable in this installation. You can continue without it.'**
  String get onboardingWindowsStartAtLoginUnavailable;

  /// Warning title on Windows when the app's startup entry is turned off in Task Manager (the Windows app name).
  ///
  /// In en, this message translates to:
  /// **'Disabled in Task Manager'**
  String get onboardingTaskManagerDisabledTitle;

  /// Warning message on Windows asking the user to turn on the app in the Windows startup apps list.
  ///
  /// In en, this message translates to:
  /// **'Enable CopyPaste in Windows startup apps, then return.'**
  String get onboardingTaskManagerDisabledMessage;

  /// Non-blocking warning on Windows when the startup entry is turned off in Task Manager.
  ///
  /// In en, this message translates to:
  /// **'Start at login is disabled in Task Manager. You can enable it in Windows Settings later.'**
  String get onboardingWindowsTaskManagerDisabled;

  /// Non-blocking warning on Windows when the startup state could not be read.
  ///
  /// In en, this message translates to:
  /// **'Start at login is unavailable. You can continue without it.'**
  String get onboardingWindowsStartAtLoginUnavailableNotice;

  /// Error shown when the Windows setup step could not read its saved state.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not read Windows setup state.'**
  String get onboardingWindowsStateReadFailed;

  /// Error shown when the Windows startup apps settings page could not be opened.
  ///
  /// In en, this message translates to:
  /// **'Windows startup settings could not be opened.'**
  String get onboardingWindowsStartupSettingsFailed;

  /// Non-blocking warning on Windows when Start at login did not change; Settings is the app's own settings screen.
  ///
  /// In en, this message translates to:
  /// **'Start at login could not be updated. You can change it later in Settings.'**
  String get onboardingWindowsStartAtLoginChangeLater;

  /// Text shown while the Linux setup step detects the desktop session.
  ///
  /// In en, this message translates to:
  /// **'Checking your Linux desktop session.'**
  String get onboardingLinuxChecking;

  /// Heading of the Linux setup step.
  ///
  /// In en, this message translates to:
  /// **'Set up your Linux desktop'**
  String get onboardingLinuxTitle;

  /// Explanation under the Linux setup heading. Clipboard attribution means knowing which app copied an item. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste needs verified desktop integration for clipboard attribution and Quick Paste.'**
  String get onboardingLinuxSubtitle;

  /// Title of the Linux setup row showing the detected display server session.
  ///
  /// In en, this message translates to:
  /// **'Desktop session'**
  String get onboardingLinuxSession;

  /// Status shown when the Linux desktop runs on X11 (a display server name).
  ///
  /// In en, this message translates to:
  /// **'X11 session detected.'**
  String get onboardingLinuxX11Detected;

  /// Status shown when the Linux desktop runs on Wayland (a display server name).
  ///
  /// In en, this message translates to:
  /// **'Wayland session detected.'**
  String get onboardingLinuxWaylandDetected;

  /// Status shown when the Linux desktop session type is not supported.
  ///
  /// In en, this message translates to:
  /// **'No supported X11 or Wayland desktop session was detected.'**
  String get onboardingLinuxSessionUnsupported;

  /// Title of the Linux setup row for the desktop extension that integrates with GNOME or KDE (desktop environment names).
  ///
  /// In en, this message translates to:
  /// **'GNOME or KDE companion'**
  String get onboardingLinuxCompanion;

  /// Status under the Linux setup row titled "GNOME or KDE companion" when the desktop extension is active.
  ///
  /// In en, this message translates to:
  /// **'Ready.'**
  String get onboardingLinuxCompanionReady;

  /// Status shown when clipboard capture is paused because excluded-app rules need the Linux desktop extension.
  ///
  /// In en, this message translates to:
  /// **'Clipboard capture is paused by source app exclusions. Enable the signed companion to resume it.'**
  String get onboardingLinuxCompanionCapturePaused;

  /// Instruction shown when the Linux desktop extension is not installed or enabled.
  ///
  /// In en, this message translates to:
  /// **'Install and enable the signed companion session package, then return here.'**
  String get onboardingLinuxCompanionInstall;

  /// Button that opens the installation guide for the Linux desktop extension.
  ///
  /// In en, this message translates to:
  /// **'Open setup'**
  String get onboardingLinuxOpenSetup;

  /// Title of the Linux setup row for the permission that lets the app send keystrokes.
  ///
  /// In en, this message translates to:
  /// **'Keyboard control'**
  String get onboardingLinuxKeyboardControl;

  /// Status under the Linux setup row titled "Keyboard control" when the permission is granted; it continues the title, so do not repeat it. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Allowed for Quick Paste.'**
  String get onboardingLinuxKeyboardAllowed;

  /// Status under the Linux setup row titled "Keyboard control" when the desktop session cannot grant it; it continues the title, so do not repeat it.
  ///
  /// In en, this message translates to:
  /// **'Unavailable in this desktop session.'**
  String get onboardingLinuxKeyboardUnsupported;

  /// Button that asks the Linux desktop for the keyboard control permission.
  ///
  /// In en, this message translates to:
  /// **'Allow keyboard control'**
  String get onboardingLinuxAllowKeyboard;

  /// Status under the Linux setup row titled "Quick Paste" when the feature works.
  ///
  /// In en, this message translates to:
  /// **'Ready.'**
  String get onboardingLinuxQuickPasteReady;

  /// Status under the Linux setup row titled "Quick Paste" when the desktop integration is missing: the feature becomes ready once it is active. It continues the title, so do not repeat it.
  ///
  /// In en, this message translates to:
  /// **'Ready after the required desktop integration is active.'**
  String get onboardingLinuxQuickPastePending;

  /// Title of the Linux setup row that registers the app to open copypaste:// links used for pairing invitations.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste links'**
  String get onboardingLinuxLinks;

  /// Status under the Linux setup row titled "CopyPaste links" when copypaste:// links are registered: the links open in this app. A statement about the links, not an instruction.
  ///
  /// In en, this message translates to:
  /// **'Open in this app.'**
  String get onboardingLinuxLinksRegistered;

  /// Description shown when copypaste:// links are not registered with the Linux desktop.
  ///
  /// In en, this message translates to:
  /// **'Register CopyPaste links for pairing invitations.'**
  String get onboardingLinuxLinksUnregistered;

  /// Button that registers copypaste:// links with the Linux desktop.
  ///
  /// In en, this message translates to:
  /// **'Register links'**
  String get onboardingLinuxRegisterLinks;

  /// Title of the Linux setup row about hiding the app window from screenshots.
  ///
  /// In en, this message translates to:
  /// **'Screenshot blocking'**
  String get onboardingLinuxScreenshotBlocking;

  /// Status under the Linux setup row titled "Screenshot blocking": Linux cannot hide the app window from screenshots, while stored data stays encrypted. The first sentence continues the title, so do not repeat it.
  ///
  /// In en, this message translates to:
  /// **'Unavailable on Linux. Stored data remains encrypted.'**
  String get onboardingLinuxScreenshotBlockingUnavailable;

  /// Button that re-checks the Linux desktop integration status.
  ///
  /// In en, this message translates to:
  /// **'Check again'**
  String get onboardingLinuxCheckAgain;

  /// Error shown when the Linux setup step could not read the desktop integration status.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not read Linux integration status.'**
  String get onboardingLinuxStatusReadFailed;

  /// Error shown when the installation guide for the Linux desktop extension could not be opened.
  ///
  /// In en, this message translates to:
  /// **'Desktop integration setup could not be opened.'**
  String get onboardingLinuxSetupOpenFailed;

  /// Error shown when the user or system declined the Linux keyboard control permission.
  ///
  /// In en, this message translates to:
  /// **'Keyboard control permission was not granted.'**
  String get onboardingLinuxKeyboardDenied;

  /// Error shown on Linux when changing the Start at login setting failed.
  ///
  /// In en, this message translates to:
  /// **'Start at login could not be updated.'**
  String get onboardingLinuxStartAtLoginFailed;

  /// Error shown when copypaste:// links could not be registered with the Linux desktop.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste links could not be registered.'**
  String get onboardingLinuxLinksFailed;

  /// Error shown when re-checking the Linux desktop integration failed.
  ///
  /// In en, this message translates to:
  /// **'Linux integration status could not be refreshed.'**
  String get onboardingLinuxStatusRefreshFailed;

  /// Error shown when setup cannot continue because the X11 session gives no clipboard access.
  ///
  /// In en, this message translates to:
  /// **'Clipboard access is unavailable in this X11 session.'**
  String get onboardingLinuxX11ClipboardUnavailable;

  /// Error shown when setup cannot continue because the Linux desktop session type is unsupported.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste requires an X11 or Wayland desktop session.'**
  String get onboardingLinuxSessionRequired;

  /// Name of the Android capture mode that records every copy in the background; used as heading and option title.
  ///
  /// In en, this message translates to:
  /// **'Full capture'**
  String get onboardingAndroidFullCapture;

  /// Heading of the Android setup step when background clipboard capture is not running yet.
  ///
  /// In en, this message translates to:
  /// **'Background capture'**
  String get onboardingAndroidBackgroundCapture;

  /// Description of the Full capture option on Android.
  ///
  /// In en, this message translates to:
  /// **'Set up once, then verify a real copy.'**
  String get onboardingAndroidFullCaptureDescription;

  /// Name of the Android capture mode that only saves shared items or copies made before returning to the app.
  ///
  /// In en, this message translates to:
  /// **'Limited capture'**
  String get onboardingAndroidLimitedCapture;

  /// Description of the Limited capture option on Android. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Share to CopyPaste or return after copying.'**
  String get onboardingAndroidLimitedCaptureDescription;

  /// Title of the Android setup row for the notification permission needed while capture runs.
  ///
  /// In en, this message translates to:
  /// **'Capture notification'**
  String get onboardingAndroidNotification;

  /// Description of the Android capture notification row.
  ///
  /// In en, this message translates to:
  /// **'Required while capture runs'**
  String get onboardingAndroidNotificationDescription;

  /// Title of the Android setup row for the battery optimization exemption.
  ///
  /// In en, this message translates to:
  /// **'Background activity'**
  String get onboardingAndroidBackgroundActivity;

  /// Description of the Android battery optimization row.
  ///
  /// In en, this message translates to:
  /// **'Recommended to reduce capture loss'**
  String get onboardingAndroidBackgroundActivityDescription;

  /// Button that opens the Android battery optimization settings.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get onboardingAndroidOpen;

  /// Title of the Shizuku setup card that grants Android background clipboard access; the button under it reads "Apply" (sharedApply).
  ///
  /// In en, this message translates to:
  /// **'Apply capture access'**
  String get onboardingAndroidApplyAccess;

  /// Description of the Shizuku setup card titled "Apply capture access": the access is applied once and the Shizuku app is not needed after that. Shizuku is an Android app name.
  ///
  /// In en, this message translates to:
  /// **'One-time. Shizuku is not needed afterwards.'**
  String get onboardingAndroidApplyAccessDescription;

  /// Hint shown when Limited capture is selected. Full capture is the capture mode name.
  ///
  /// In en, this message translates to:
  /// **'You can set up Full capture later.'**
  String get onboardingAndroidFullCaptureLater;

  /// Title shown after a test copy proved that Android background capture works.
  ///
  /// In en, this message translates to:
  /// **'Background capture is working'**
  String get onboardingAndroidCaptureWorking;

  /// Title of the Android step that asks the user to copy something to test capture.
  ///
  /// In en, this message translates to:
  /// **'Verify a real copy'**
  String get onboardingAndroidVerifyCopy;

  /// Description shown after the test copy arrived. History is the clipboard history screen name.
  ///
  /// In en, this message translates to:
  /// **'A new copy reached your local History.'**
  String get onboardingAndroidCopyReached;

  /// Instruction for the Android capture test.
  ///
  /// In en, this message translates to:
  /// **'Copy in another app, then return here.'**
  String get onboardingAndroidCopyInstruction;

  /// Button that starts Android background clipboard capture.
  ///
  /// In en, this message translates to:
  /// **'Start capture'**
  String get onboardingAndroidStartCapture;

  /// Error shown when the Android setup step could not read the capture setup state.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not read Android setup state.'**
  String get onboardingAndroidStateReadFailed;

  /// Error shown when re-reading the Android capture setup state failed.
  ///
  /// In en, this message translates to:
  /// **'Android capture state could not be refreshed.'**
  String get onboardingAndroidStateRefreshFailed;

  /// Error shown when the app cannot watch Android permission changes during setup.
  ///
  /// In en, this message translates to:
  /// **'Android setup monitoring is unavailable.'**
  String get onboardingAndroidMonitoringUnavailable;

  /// Error shown when Android clipboard capture could not be turned on.
  ///
  /// In en, this message translates to:
  /// **'Clipboard capture could not be enabled.'**
  String get onboardingAndroidIntakeFailed;

  /// Error shown when switching to Limited capture could not stop background capture.
  ///
  /// In en, this message translates to:
  /// **'Background capture could not be stopped.'**
  String get onboardingAndroidStopCaptureFailed;

  /// Error shown when the Android battery optimization settings could not be opened.
  ///
  /// In en, this message translates to:
  /// **'Battery settings could not be opened.'**
  String get onboardingAndroidBatterySettingsFailed;

  /// Error shown when Shizuku permits the app but granting the Android capture permissions failed.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is allowed in Shizuku, but capture access could not be applied.'**
  String get onboardingAndroidGrantsFailed;

  /// Error shown when the app must first be allowed in the Shizuku app.
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste in Shizuku to apply capture access.'**
  String get onboardingAndroidAllowInShizuku;

  /// Error shown when capture is started before the background clipboard permission was granted.
  ///
  /// In en, this message translates to:
  /// **'Grant background clipboard access first.'**
  String get onboardingAndroidGrantAccessFirst;

  /// Error shown when capture is started before the notification permission was granted.
  ///
  /// In en, this message translates to:
  /// **'Allow notifications before starting background capture.'**
  String get onboardingAndroidAllowNotificationsFirst;

  /// Error shown when Android background capture failed to start.
  ///
  /// In en, this message translates to:
  /// **'Background capture could not be started.'**
  String get onboardingAndroidStartCaptureFailed;

  /// Error shown when the Android capture setup could not be reopened from Settings.
  ///
  /// In en, this message translates to:
  /// **'Android capture setup could not be opened.'**
  String get onboardingAndroidSetupOpenFailed;

  /// Error shown when the chosen Android capture mode or setup method could not be stored.
  ///
  /// In en, this message translates to:
  /// **'Android setup choice could not be saved.'**
  String get onboardingAndroidChoiceSaveFailed;

  /// Tab that lists optional modules available to install.
  ///
  /// In en, this message translates to:
  /// **'Marketplace'**
  String get modulesMarketplaceTab;

  /// Tab that lists optional modules already installed.
  ///
  /// In en, this message translates to:
  /// **'Installed'**
  String get modulesInstalledTab;

  /// Verb. Button that reloads the module marketplace.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get modulesRefresh;

  /// Button in Modules that installs a module package file the user chooses.
  ///
  /// In en, this message translates to:
  /// **'Install from file'**
  String get modulesInstallFromFile;

  /// File type name for module package files in the system file picker and its error message.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste module'**
  String get modulesPackageFileType;

  /// Placeholder of the search field on the Modules settings page.
  ///
  /// In en, this message translates to:
  /// **'Search modules'**
  String get modulesSearchPlaceholder;

  /// Error title shown when installing, enabling, configuring, or removing a module failed.
  ///
  /// In en, this message translates to:
  /// **'Module operation failed'**
  String get modulesOperationFailed;

  /// Error title shown when the installed modules could not be read.
  ///
  /// In en, this message translates to:
  /// **'Modules are unavailable'**
  String get modulesUnavailableTitle;

  /// Empty-state title of the Installed modules tab.
  ///
  /// In en, this message translates to:
  /// **'No modules installed'**
  String get modulesNoneInstalled;

  /// Empty-state title when no module matches the search.
  ///
  /// In en, this message translates to:
  /// **'No matching modules'**
  String get modulesNoMatches;

  /// Empty-state message of the Installed modules tab.
  ///
  /// In en, this message translates to:
  /// **'Choose a module from the marketplace.'**
  String get modulesChooseFromMarketplace;

  /// Empty-state message when no module matches the search.
  ///
  /// In en, this message translates to:
  /// **'Try a different search.'**
  String get modulesTryDifferentSearch;

  /// Error title shown when the module marketplace could not be loaded.
  ///
  /// In en, this message translates to:
  /// **'Marketplace is unavailable'**
  String get modulesMarketplaceUnavailableTitle;

  /// Empty-state title of the module marketplace.
  ///
  /// In en, this message translates to:
  /// **'No modules available'**
  String get modulesNoneAvailable;

  /// Empty-state message of the module marketplace.
  ///
  /// In en, this message translates to:
  /// **'Modules will appear here when they are published.'**
  String get modulesNoneAvailableMessage;

  /// Screen reader label of the switch that turns a module on or off; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'Enable {module}'**
  String modulesEnableModule(String module);

  /// Verb. Button that downloads and installs a module.
  ///
  /// In en, this message translates to:
  /// **'Install'**
  String get modulesInstall;

  /// Tooltip of the button that updates a module; {version} is the new version number.
  ///
  /// In en, this message translates to:
  /// **'Update to {version}'**
  String modulesUpdateTo(String version);

  /// Tooltip of the button that uninstalls a module.
  ///
  /// In en, this message translates to:
  /// **'Remove module'**
  String get modulesRemoveModule;

  /// Tooltip of the button that opens a module's settings, and the settings panel title when the module name is unknown.
  ///
  /// In en, this message translates to:
  /// **'Module settings'**
  String get modulesModuleSettings;

  /// Title of a module's settings panel; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'{module} settings'**
  String modulesSettingsTitle(String module);

  /// Progress text shown while a module downloads a language model.
  ///
  /// In en, this message translates to:
  /// **'Downloading language model…'**
  String get modulesDownloadingModel;

  /// Verb. Button in a module command dialog that runs the command.
  ///
  /// In en, this message translates to:
  /// **'Run'**
  String get modulesRun;

  /// Title of the dialog that confirms uninstalling a module; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'Remove {module}?'**
  String modulesRemoveTitle(String module);

  /// Message of the dialog that confirms uninstalling a module.
  ///
  /// In en, this message translates to:
  /// **'The module and its local settings will be removed.'**
  String get modulesRemoveMessage;

  /// Verb. Confirmation button that uninstalls a module.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get modulesRemove;

  /// Error shown when uninstalling a module failed.
  ///
  /// In en, this message translates to:
  /// **'The module could not be removed.'**
  String get modulesRemoveFailed;

  /// Tooltip and screen reader label of the button that closes a module's settings panel.
  ///
  /// In en, this message translates to:
  /// **'Close module settings'**
  String get modulesCloseSettings;

  /// Empty-state title in a module's settings panel after the module was uninstalled.
  ///
  /// In en, this message translates to:
  /// **'Module removed'**
  String get modulesRemoved;

  /// Empty-state title in a module's settings panel when the module has no options.
  ///
  /// In en, this message translates to:
  /// **'No additional settings'**
  String get modulesNoSettingsTitle;

  /// Button in a module's settings that opens the Android SMS permission setup.
  ///
  /// In en, this message translates to:
  /// **'Set up SMS access'**
  String get modulesSetUpSmsAccess;

  /// Button that restarts the app to finish a module change. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Restart CopyPaste'**
  String get modulesRestartApp;

  /// Title of the panel that sets up the Android permission to read incoming SMS messages.
  ///
  /// In en, this message translates to:
  /// **'SMS access'**
  String get modulesSmsAccessTitle;

  /// Tooltip and screen reader label of the button that closes the SMS access panel.
  ///
  /// In en, this message translates to:
  /// **'Close SMS access'**
  String get modulesCloseSmsAccess;

  /// Text shown when the SMS permission is granted. SMS Codes is the module name.
  ///
  /// In en, this message translates to:
  /// **'Access is ready. Enable SMS Codes to start copying new codes.'**
  String get modulesSmsReady;

  /// Text shown when the SMS permission is not granted yet; codes are one-time verification codes.
  ///
  /// In en, this message translates to:
  /// **'Allow access to copy codes from new SMS messages.'**
  String get modulesSmsAllowAccess;

  /// Title of the setup row for the notification permission needed while the SMS module runs.
  ///
  /// In en, this message translates to:
  /// **'SMS notification'**
  String get modulesSmsNotification;

  /// Description of the SMS notification row. SMS Codes is the module name.
  ///
  /// In en, this message translates to:
  /// **'Required while SMS Codes runs'**
  String get modulesSmsNotificationDescription;

  /// Title of the setup row for the separate Android app that alone holds SMS access. Product name; do not translate.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste SMS'**
  String get modulesSmsApp;

  /// Description of the CopyPaste SMS setup row: the main app has no SMS access, only this separate app reads SMS.
  ///
  /// In en, this message translates to:
  /// **'Separate app that alone reads SMS'**
  String get modulesSmsAppDescription;

  /// Title of the Shizuku setup card that grants the Android SMS permission; the button under it reads "Apply" (sharedApply).
  ///
  /// In en, this message translates to:
  /// **'Apply SMS access'**
  String get modulesSmsApplyAccess;

  /// Description of the step that grants the Android SMS permission once.
  ///
  /// In en, this message translates to:
  /// **'Needed only once.'**
  String get modulesSmsApplyAccessDescription;

  /// Error title shown in the SMS access panel.
  ///
  /// In en, this message translates to:
  /// **'SMS setup failed'**
  String get modulesSmsSetupFailed;

  /// Error toast title shown in a module form when choosing a file failed; shown alone when no reason is known.
  ///
  /// In en, this message translates to:
  /// **'File selection failed'**
  String get modulesFileSelectionFailed;

  /// Button in a module form that opens a file picker.
  ///
  /// In en, this message translates to:
  /// **'Choose file'**
  String get modulesChooseFile;

  /// Status under module settings when the selected language model is downloaded; {model} is the model name.
  ///
  /// In en, this message translates to:
  /// **'{model} model is ready'**
  String modulesModelReady(String model);

  /// Status under module settings when the selected language model still needs downloading; {model} is the model name and {size} is its size in mebibytes.
  ///
  /// In en, this message translates to:
  /// **'{model} model · {size} MiB download'**
  String modulesModelDownload(String model, String size);

  /// Module card line when the module's supported operating systems are unknown.
  ///
  /// In en, this message translates to:
  /// **'Platforms: Unknown'**
  String get modulesPlatformsUnknown;

  /// Module card line listing supported operating systems; {platforms} is a list such as macOS · Windows.
  ///
  /// In en, this message translates to:
  /// **'Platforms: {platforms}'**
  String modulesPlatforms(String platforms);

  /// Error shown when the downloaded module marketplace list fails its signature check.
  ///
  /// In en, this message translates to:
  /// **'The module catalog signature is invalid.'**
  String get modulesCatalogSignatureInvalid;

  /// Error shown when the module marketplace cannot be reached.
  ///
  /// In en, this message translates to:
  /// **'Could not connect to the module marketplace.'**
  String get modulesMarketplaceConnectFailed;

  /// Error shown when the module marketplace request timed out.
  ///
  /// In en, this message translates to:
  /// **'The module marketplace did not respond.'**
  String get modulesMarketplaceTimeout;

  /// Error shown when installing a module that does not support this device.
  ///
  /// In en, this message translates to:
  /// **'This module is not compatible with this device.'**
  String get modulesIncompatible;

  /// Error shown when a module package reports an invalid size.
  ///
  /// In en, this message translates to:
  /// **'The module package size is invalid.'**
  String get modulesPackageSizeInvalid;

  /// Error shown when the network connection dropped during a module download.
  ///
  /// In en, this message translates to:
  /// **'The module download was interrupted.'**
  String get modulesDownloadInterrupted;

  /// Error shown when a module download timed out.
  ///
  /// In en, this message translates to:
  /// **'The module download did not respond.'**
  String get modulesDownloadTimeout;

  /// Error shown when a module download address is not an allowed GitHub address.
  ///
  /// In en, this message translates to:
  /// **'The module package URL is not trusted.'**
  String get modulesPackageUrlUntrusted;

  /// Error shown when a module download redirected away from GitHub (a service name).
  ///
  /// In en, this message translates to:
  /// **'The module download left GitHub.'**
  String get modulesDownloadLeftGithub;

  /// Error shown when a module download was redirected too many times.
  ///
  /// In en, this message translates to:
  /// **'The module download used too many redirects.'**
  String get modulesDownloadTooManyRedirects;

  /// Error shown when a module download exceeds the allowed size.
  ///
  /// In en, this message translates to:
  /// **'The module download is too large.'**
  String get modulesDownloadTooLarge;

  /// Error shown when a downloaded module package does not match its expected checksum.
  ///
  /// In en, this message translates to:
  /// **'The module package failed its integrity check.'**
  String get modulesPackageIntegrityFailed;

  /// Error shown when a module file is not found on the server.
  ///
  /// In en, this message translates to:
  /// **'This module download is currently unavailable.'**
  String get modulesDownloadUnavailable;

  /// Error shown when the module marketplace server returned an error.
  ///
  /// In en, this message translates to:
  /// **'The module marketplace is unavailable. Try again.'**
  String get modulesMarketplaceUnavailableRetry;

  /// Error shown when the module marketplace list cannot be read.
  ///
  /// In en, this message translates to:
  /// **'The module catalog is invalid.'**
  String get modulesCatalogInvalid;

  /// Fallback operating system name used in a module requirement such as 'this system 14 or newer' when the system is unknown.
  ///
  /// In en, this message translates to:
  /// **'this system'**
  String get modulesThisSystem;

  /// Module card line with the minimum operating system; {system} is a system name such as macOS and {version} is a version number.
  ///
  /// In en, this message translates to:
  /// **'{system} {version} or newer'**
  String modulesSystemRequirement(String system, String version);

  /// Module card note when the module has no package for this device.
  ///
  /// In en, this message translates to:
  /// **'Not available for this device.'**
  String get modulesNotAvailableForDevice;

  /// Module card note when the app's own version is unknown, so compatibility cannot be checked.
  ///
  /// In en, this message translates to:
  /// **'The installed CopyPaste version could not be verified.'**
  String get modulesAppVersionUnverified;

  /// Module card note when the module does not support this app version; {version} is the app version number.
  ///
  /// In en, this message translates to:
  /// **'This module does not support CopyPaste {version}.'**
  String modulesAppVersionUnsupported(String version);

  /// Module card note naming a missing requirement; {requirement} is text such as 'macOS 14 or newer'.
  ///
  /// In en, this message translates to:
  /// **'Requires {requirement}.'**
  String modulesRequires(String requirement);

  /// Error shown when enabling an SMS module before the SMS permission is granted.
  ///
  /// In en, this message translates to:
  /// **'Set up SMS access before enabling this module.'**
  String get modulesSmsSetupRequired;

  /// Error shown when SMS monitoring failed and the module could not be turned back off.
  ///
  /// In en, this message translates to:
  /// **'SMS monitoring could not be started. The module is still enabled; disable it again to retry.'**
  String get modulesSmsMonitoringStillEnabled;

  /// Error shown when SMS monitoring failed and the module was turned off again.
  ///
  /// In en, this message translates to:
  /// **'SMS monitoring could not be started. The module was disabled.'**
  String get modulesSmsMonitoringDisabled;

  /// Error shown when SMS monitoring failed and the module's on/off state is unknown.
  ///
  /// In en, this message translates to:
  /// **'SMS monitoring could not be started. The module enabled state could not be confirmed.'**
  String get modulesSmsMonitoringUnconfirmed;

  /// Error shown when watching for new SMS messages could not start.
  ///
  /// In en, this message translates to:
  /// **'SMS monitoring could not be started.'**
  String get modulesSmsMonitoringFailed;

  /// Error shown when SMS setup is opened on a platform other than Android.
  ///
  /// In en, this message translates to:
  /// **'SMS access is available only on Android.'**
  String get modulesSmsAndroidOnly;

  /// Error shown when an enabled SMS module lost its permission and must be set up again.
  ///
  /// In en, this message translates to:
  /// **'SMS monitoring needs setup.'**
  String get modulesSmsMonitoringNeedsSetup;

  /// Notice shown when the app could not check whether an enabled SMS module is being monitored.
  ///
  /// In en, this message translates to:
  /// **'SMS monitoring could not be checked.'**
  String get modulesSmsMonitoringUnavailable;

  /// Error shown when a module action failed for an unknown reason.
  ///
  /// In en, this message translates to:
  /// **'The module operation could not be completed.'**
  String get modulesOperationIncomplete;

  /// Error shown when an installed module declares a system event this app does not support.
  ///
  /// In en, this message translates to:
  /// **'The module uses an unsupported event.'**
  String get modulesUnsupportedEvent;

  /// Error shown when a module command returns data this app cannot display.
  ///
  /// In en, this message translates to:
  /// **'The module returned an unsupported result.'**
  String get modulesUnsupportedResult;

  /// Error shown when a module declares a form field type this app does not support.
  ///
  /// In en, this message translates to:
  /// **'The module uses an unsupported field.'**
  String get modulesUnsupportedField;

  /// Error shown when the module service returned malformed data.
  ///
  /// In en, this message translates to:
  /// **'The module response is invalid.'**
  String get modulesResponseInvalid;

  /// Message under the error toast title "File selection failed" when a module form asks for a file where no file picker is offered; it continues the title.
  ///
  /// In en, this message translates to:
  /// **'Unavailable here.'**
  String get modulesFileSelectionUnavailable;

  /// Error shown when the chosen file has the wrong type or size; {field} is the module's field name.
  ///
  /// In en, this message translates to:
  /// **'{field} is not an accepted file.'**
  String modulesFileNotAccepted(String field);

  /// Error shown when the SMS permission state could not be read.
  ///
  /// In en, this message translates to:
  /// **'SMS access could not be verified.'**
  String get modulesSmsVerifyFailed;

  /// Error shown when the notification permission needed by the SMS module was declined. SMS Codes is the module name.
  ///
  /// In en, this message translates to:
  /// **'Allow notifications to keep SMS Codes active in the background.'**
  String get modulesSmsAllowNotifications;

  /// Error shown when granting the SMS permission did not succeed.
  ///
  /// In en, this message translates to:
  /// **'SMS access was not granted.'**
  String get modulesSmsNotGranted;

  /// Error shown when an SMS setup action failed for an unknown reason.
  ///
  /// In en, this message translates to:
  /// **'SMS access could not be configured.'**
  String get modulesSmsConfigureFailed;

  /// Primary button on the Linux setup step when clipboard capture works but Quick Paste is not set up yet; continuing keeps basic clipboard history only.
  ///
  /// In en, this message translates to:
  /// **'Continue with basic capture'**
  String get onboardingLinuxContinueBasic;

  /// Description of the Linux desktop extension row when clipboard capture already works without it. Quick Paste is a feature name; source app exclusions are rules that ignore copies from chosen apps.
  ///
  /// In en, this message translates to:
  /// **'Optional. Needed for Quick Paste and source app exclusions.'**
  String get onboardingLinuxCompanionOptional;

  /// Description of the Linux keyboard control row while the permission still needs consent. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Optional. Needed for Quick Paste to return to the previous app.'**
  String get onboardingLinuxKeyboardOptional;

  /// Error shown when Linux setup cannot continue because the Wayland session gives no clipboard access. Wayland, GNOME and KDE are product names.
  ///
  /// In en, this message translates to:
  /// **'Clipboard access is unavailable in this Wayland session. Enable the signed GNOME or KDE companion, then check again.'**
  String get onboardingLinuxWaylandClipboardUnavailable;

  /// Tooltip and accessible name of the icon button on a History row that opens a menu with more clip actions.
  ///
  /// In en, this message translates to:
  /// **'More actions'**
  String get historyMoreActions;

  /// Menu item on a History row that opens the editor for the clip's tags.
  ///
  /// In en, this message translates to:
  /// **'Tags…'**
  String get historyTagsAction;

  /// Reason shown in the operating system's identity prompt for unlocking secret items for the current surface (Quick Paste, the Android keyboard panel or History). The macos form is a lowercase clause that macOS inserts into its own sentence "<App> is trying to <reason>." in the system language; the other form is a standalone phrase.
  ///
  /// In en, this message translates to:
  /// **'{platform, select, macos{unlock secret items} other{Unlock secret items}}'**
  String deviceAuthReasonUnlockSecrets(String platform);

  /// Reason shown in the operating system's identity prompt for copying a secret item, for example from the tray Recent menu. The macos form is a lowercase clause that macOS inserts into its own sentence "<App> is trying to <reason>." in the system language; the other form is a standalone phrase.
  ///
  /// In en, this message translates to:
  /// **'{platform, select, macos{copy a secret item} other{Copy a secret item}}'**
  String deviceAuthReasonCopySecret(String platform);

  /// State word shown beside a switch that is turned on. Adjective; keep it as short as possible.
  ///
  /// In en, this message translates to:
  /// **'On'**
  String get commonOn;

  /// State word shown beside a switch that is turned off. Adjective; keep it as short as possible.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get commonOff;

  /// Settings error message, shown with a retry action, when Quick Paste fails to start for a reason other than its shortcut being refused. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste could not start, so its shortcut does not work.'**
  String get settingsQuickPasteStartFailed;

  /// Warning in the Quick Paste shortcut row when the system refuses the global shortcut because another application already uses it.
  ///
  /// In en, this message translates to:
  /// **'{shortcut} is used by another app. Choose another shortcut.'**
  String settingsQuickPasteShortcutInUse(String shortcut);

  /// Empty state title on the Devices screen when no other device has been paired yet.
  ///
  /// In en, this message translates to:
  /// **'No other devices paired yet'**
  String get devicesEmptyTitle;

  /// Empty state message on the Devices screen explaining what pairing another device gives and requires.
  ///
  /// In en, this message translates to:
  /// **'Pair a device on the same network to sync history.'**
  String get devicesEmptyMessage;

  /// Pairing status title when the pairing ended unfinished without this device rejecting it; the other device may have rejected it or stopped pairing, so do not say who ended it.
  ///
  /// In en, this message translates to:
  /// **'Pairing not completed'**
  String get pairingStateNotCompleted;

  /// Short name of a step in the pairing progress indicator: this device shows a pairing code and QR code and waits for the other device. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Invite'**
  String get pairingStepInvite;

  /// Short name of a step in the pairing progress indicator: this device scans the QR code shown on the other device. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Scan'**
  String get pairingStepScan;

  /// Short name of a step in the pairing progress indicator: the user types the pairing code shown on the other device. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Code'**
  String get pairingStepCode;

  /// Short name of a step in the pairing progress indicator: a connection request is sent to or received from a nearby device. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Request'**
  String get pairingStepRequest;

  /// Short name of a step in the pairing progress indicator: the two devices establish a secure connection. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Connect'**
  String get pairingStepConnect;

  /// Short name of a step in the pairing progress indicator: the verification code is compared and both devices confirm. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Confirm'**
  String get pairingStepConfirm;

  /// Short name of a step in the pairing progress indicator: the final step, reached when the device has been paired. Keep it to one or two short words.
  ///
  /// In en, this message translates to:
  /// **'Paired'**
  String get pairingStepPaired;

  /// Label for the other device in the pairing confirmation status when its name is not known.
  ///
  /// In en, this message translates to:
  /// **'Other device'**
  String get pairingOtherDevice;

  /// Pairing confirmation status of this device before the user has confirmed or rejected.
  ///
  /// In en, this message translates to:
  /// **'Not confirmed yet'**
  String get pairingDecisionNotConfirmed;

  /// Pairing confirmation status of this device while its confirmation or rejection is being sent.
  ///
  /// In en, this message translates to:
  /// **'Sending your decision…'**
  String get pairingDecisionSending;

  /// Pairing confirmation status of a device that confirmed the pairing.
  ///
  /// In en, this message translates to:
  /// **'Confirmed'**
  String get pairingDecisionConfirmed;

  /// Pairing confirmation status of a device that rejected the pairing.
  ///
  /// In en, this message translates to:
  /// **'Rejected'**
  String get pairingDecisionRejected;

  /// Pairing confirmation status of the other device after this device confirmed and the other has not answered yet.
  ///
  /// In en, this message translates to:
  /// **'Waiting for confirmation'**
  String get pairingDecisionWaiting;

  /// Pairing confirmation status of the other device when this device has not been told its answer.
  ///
  /// In en, this message translates to:
  /// **'Answer not known'**
  String get pairingDecisionUnknown;

  /// Number of paired devices shown after the synchronization status in the application header, such as: Up to date · 3 devices.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 device} other{{count} devices}}'**
  String syncHeaderDeviceCount(int count);

  /// Quiet note at the end of a History row's metadata line when the clip has more text lines than the row can show. {count} is the total number of lines in the clip and is always greater than 4.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 line} other{{count} lines}}'**
  String historyClipLineCount(int count);

  /// Screen reader label of the application header region that holds the History, Devices and Settings destinations.
  ///
  /// In en, this message translates to:
  /// **'Main navigation'**
  String get appNavigationLabel;

  /// Position of the current step of a multi-step operation, shown beside its name.
  ///
  /// In en, this message translates to:
  /// **'Step {step} of {count}'**
  String sharedStepCounter(int step, int count);

  /// Screen reader description of the current step of a multi-step operation; {name} is the name of the current step.
  ///
  /// In en, this message translates to:
  /// **'Step {step} of {count}: {name}'**
  String sharedStepSemantics(int step, int count, String name);

  /// Noun. Short name of the first phase of a module install or update, shown in the progress strip on the module card. Keep it as short as possible.
  ///
  /// In en, this message translates to:
  /// **'Download'**
  String get modulesPhaseDownload;

  /// Noun. Short name of the phase of a module install or update in which the downloaded package is verified and installed, shown in the progress strip on the module card. Keep it as short as possible.
  ///
  /// In en, this message translates to:
  /// **'Install'**
  String get modulesPhaseInstall;

  /// Screen reader name of the progress strip on a module card while the module is installed for the first time; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'Installing {module}'**
  String modulesInstallingModule(String module);

  /// Screen reader name of the progress strip on a module card while an installed module is updated to a newer version; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'Updating {module}'**
  String modulesUpdatingModule(String module);

  /// Screen reader name of the button that closes a transient message before it leaves by itself.
  ///
  /// In en, this message translates to:
  /// **'Dismiss'**
  String get commonDismiss;

  /// Title of the confirmation shown before a shorter retention period deletes history.
  ///
  /// In en, this message translates to:
  /// **'Remove older history?'**
  String get settingsRetentionConfirmTitle;

  /// Body of the confirmation shown before a shorter retention period deletes history. limit is the chosen period, for example '7 days'.
  ///
  /// In en, this message translates to:
  /// **'New limit: {limit}. Older unpinned clips are deleted permanently.'**
  String settingsRetentionConfirmMessage(String limit);

  /// Title of the confirmation shown before a smaller storage quota deletes history.
  ///
  /// In en, this message translates to:
  /// **'Reduce the storage quota?'**
  String get settingsStorageQuotaConfirmTitle;

  /// Body of the confirmation shown before a smaller storage quota deletes history. size is the chosen quota, for example '5 GiB'.
  ///
  /// In en, this message translates to:
  /// **'New quota: {size}. The oldest unpinned clips over it are deleted permanently.'**
  String settingsStorageQuotaConfirmMessage(String size);

  /// Button that confirms a stricter history limit which deletes history.
  ///
  /// In en, this message translates to:
  /// **'Apply limit'**
  String get settingsLimitConfirmAction;

  /// Title of the confirmation shown before the last visible entry point of the application (Dock or taskbar icon, menu bar or tray icon) is turned off.
  ///
  /// In en, this message translates to:
  /// **'Hide CopyPaste completely?'**
  String get settingsHideEverywhereTitle;

  /// Explains that with both visibility settings off the application has no visible icon and how to open it again. Used as a dialog body and as a notice.
  ///
  /// In en, this message translates to:
  /// **'With both off, there\'s no icon. To open it, start CopyPaste again.'**
  String get settingsHideEverywhereMessage;

  /// Same as settingsHideEverywhereMessage, when the Quick Paste shortcut can open the application. shortcut is a key combination.
  ///
  /// In en, this message translates to:
  /// **'With both off, there\'s no icon. To open it, press {shortcut} or start CopyPaste again.'**
  String settingsHideEverywhereShortcutMessage(String shortcut);

  /// Verb. Button that confirms hiding the application's last visible icon.
  ///
  /// In en, this message translates to:
  /// **'Hide'**
  String get settingsHideEverywhereAction;

  /// Title of the confirmation shown after the text history was exported.
  ///
  /// In en, this message translates to:
  /// **'Text history exported'**
  String get settingsExportDoneTitle;

  /// Title of the confirmation shown after an encrypted backup was created.
  ///
  /// In en, this message translates to:
  /// **'Backup created'**
  String get settingsBackupDoneTitle;

  /// Title of the confirmation shown after history was restored from a backup.
  ///
  /// In en, this message translates to:
  /// **'History restored'**
  String get settingsRestoreDoneTitle;

  /// Message of the confirmation shown after the text history was exported: how many text clips the file holds.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 text clip} other{{count} text clips}}'**
  String settingsExportedTextClips(int count);

  /// Short addition to the export confirmation: how many text clips could not be read and were left out of the file.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 could not be read} other{{count} could not be read}}'**
  String settingsExportUnreadableClips(int count);

  /// Joins the outcome of an action with a detail about it, for example '214 text clips · 3 could not be read'.
  ///
  /// In en, this message translates to:
  /// **'{outcome} · {detail}'**
  String settingsOutcomeDetail(String outcome, String detail);

  /// A size in kibibytes (binary units of 1024); size is an already formatted number.
  ///
  /// In en, this message translates to:
  /// **'{size} KiB'**
  String sharedSizeKibibytes(String size);

  /// A size in mebibytes (binary units of 1024); size is an already formatted number.
  ///
  /// In en, this message translates to:
  /// **'{size} MiB'**
  String sharedSizeMebibytes(String size);

  /// A size in gibibytes (binary units of 1024); size is an already formatted number.
  ///
  /// In en, this message translates to:
  /// **'{size} GiB'**
  String sharedSizeGibibytes(String size);

  /// Short storage usage in the application header, such as '2.1 of 5 GiB'. used is the used amount, without its unit when it shares the quota's unit; quota is the storage quota with its unit.
  ///
  /// In en, this message translates to:
  /// **'{used} of {quota}'**
  String storageUsageShort(String used, String quota);

  /// Storage used by unpinned history against the storage quota, such as '2.1 GiB used of 5 GiB'.
  ///
  /// In en, this message translates to:
  /// **'{used} used of {quota}'**
  String storageUsageOfQuota(String used, String quota);

  /// Storage used by unpinned history when no storage quota applies, such as '2.1 GiB used'.
  ///
  /// In en, this message translates to:
  /// **'{used} used'**
  String storageUsageNoQuota(String used);

  /// Status words shown when unpinned history uses more storage than the quota allows.
  ///
  /// In en, this message translates to:
  /// **'Over quota'**
  String get storageUsageOverQuota;

  /// Capture date of the oldest unpinned clip, shown after the storage usage.
  ///
  /// In en, this message translates to:
  /// **'Oldest clip {date}'**
  String storageUsageOldestClip(String date);

  /// Joins a storage usage summary with a detail about it, such as '2.1 GiB used of 5 GiB · Oldest clip 14 Aug 2026'.
  ///
  /// In en, this message translates to:
  /// **'{usage} · {detail}'**
  String storageUsageDetail(String usage, String detail);

  /// Screen reader label of the header button that shows storage usage and opens the storage settings; usage is the usage summary.
  ///
  /// In en, this message translates to:
  /// **'Storage: {usage}'**
  String storageHeaderSemanticsLabel(String usage);

  /// Body of the confirmation shown before a storage quota smaller than the current usage removes the oldest unpinned clips. size is the chosen quota; used is the storage unpinned history uses now.
  ///
  /// In en, this message translates to:
  /// **'New quota: {size}. {used} is in use, so the oldest unpinned clips are deleted permanently.'**
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used);

  /// Number of clips in a History group, shown after the group name and date, such as: Today · Wednesday, November 11 · 14 clips.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip} other{{count} clips}}'**
  String historySectionClipCount(int count);

  /// Tray and menu-bar menu item that shows the main window.
  ///
  /// In en, this message translates to:
  /// **'Open CopyPaste'**
  String get trayOpenApp;

  /// Tray and menu-bar menu item that opens the Quick Paste window. Quick Paste is a product name.
  ///
  /// In en, this message translates to:
  /// **'Open Quick Paste'**
  String get trayOpenQuickPaste;

  /// Same as trayOpenQuickPaste, followed by its global shortcut.
  ///
  /// In en, this message translates to:
  /// **'Open Quick Paste ({shortcut})'**
  String trayOpenQuickPasteWithShortcut(String shortcut);

  /// Tray and menu-bar submenu that lists the newest clips.
  ///
  /// In en, this message translates to:
  /// **'Recent'**
  String get trayRecent;

  /// Shown in the Recent submenu while clips load.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get trayRecentLoading;

  /// Shown in the Recent submenu when clips could not be read.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load clips'**
  String get trayRecentFailed;

  /// Shown in the Recent submenu when History is empty.
  ///
  /// In en, this message translates to:
  /// **'No clips yet'**
  String get trayRecentEmpty;

  /// Shown in the Recent submenu when previews are set to Hidden: only the number of clips.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 recent clip hidden} other{{count} recent clips hidden}}'**
  String trayRecentHidden(int count);

  /// Replaces the content of a secret clip in the Recent submenu, followed by its source and time.
  ///
  /// In en, this message translates to:
  /// **'Secret'**
  String get trayRecentSecret;

  /// Tray and menu-bar menu item that copies the newest clip received from another device.
  ///
  /// In en, this message translates to:
  /// **'Copy latest from other device'**
  String get trayCopyLatestFromOtherDevice;

  /// Tray and menu-bar menu item that stores the current clipboard in History now, even while capture is paused.
  ///
  /// In en, this message translates to:
  /// **'Capture now'**
  String get trayCaptureNow;

  /// Disabled tray row while capture is paused for a while. time is a clock time.
  ///
  /// In en, this message translates to:
  /// **'Paused until {time}'**
  String trayPausedUntil(String time);

  /// Tray and menu-bar submenu with pause durations.
  ///
  /// In en, this message translates to:
  /// **'Pause for…'**
  String get trayPauseFor;

  /// Pause duration in the Pause for submenu.
  ///
  /// In en, this message translates to:
  /// **'15 minutes'**
  String get trayPauseFor15Minutes;

  /// Pause duration in the Pause for submenu.
  ///
  /// In en, this message translates to:
  /// **'1 hour'**
  String get trayPauseForOneHour;

  /// Pause option in the Pause for submenu: no automatic resume.
  ///
  /// In en, this message translates to:
  /// **'Until I resume'**
  String get trayPauseUntilResumed;

  /// Tray row with the number of items in the Paste Queue. Paste Queue is a product name.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Paste Queue: 1 item} other{Paste Queue: {count} items}}'**
  String trayPasteQueue(int count);

  /// Tray row while the Paste Queue collects copies, with its number of items.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Collecting: 1 item} other{Collecting: {count} items}}'**
  String trayPasteQueueCollecting(int count);

  /// Tray submenu listing captures that content rules did not save, with their number.
  ///
  /// In en, this message translates to:
  /// **'Recently skipped ({count})'**
  String trayRecentlySkipped(int count);

  /// Item in the Recently skipped submenu that opens the content rules settings.
  ///
  /// In en, this message translates to:
  /// **'Open content rules'**
  String get trayOpenContentRules;

  /// Heading of the settings for the menu-bar (macOS) or tray (Windows, Linux) menu.
  ///
  /// In en, this message translates to:
  /// **'Menu bar and tray'**
  String get trayRecentSettingsTitle;

  /// Description under trayRecentSettingsTitle.
  ///
  /// In en, this message translates to:
  /// **'What the menu bar or tray menu lists under Recent.'**
  String get trayRecentSettingsDescription;

  /// Setting title: how many clips the Recent submenu lists.
  ///
  /// In en, this message translates to:
  /// **'Recent clips'**
  String get trayRecentCountTitle;

  /// Description of trayRecentCountTitle.
  ///
  /// In en, this message translates to:
  /// **'How many of the newest clips the menu lists.'**
  String get trayRecentCountDescription;

  /// Setting title: how much of each clip the Recent submenu shows.
  ///
  /// In en, this message translates to:
  /// **'Recent clip previews'**
  String get trayRecentPreviewTitle;

  /// Description of trayRecentPreviewTitle: menu text leaves the encrypted store.
  ///
  /// In en, this message translates to:
  /// **'How much clip text the menu shows.'**
  String get trayRecentPreviewDescription;

  /// Option of trayRecentPreviewTitle: the beginning of the clip text.
  ///
  /// In en, this message translates to:
  /// **'Full preview'**
  String get trayRecentPreviewFull;

  /// Option of trayRecentPreviewTitle: clip type, source app and time, no content.
  ///
  /// In en, this message translates to:
  /// **'Type and source only'**
  String get trayRecentPreviewTitles;

  /// Option of trayRecentPreviewTitle: only the number of clips.
  ///
  /// In en, this message translates to:
  /// **'Hidden'**
  String get trayRecentPreviewHidden;

  /// Space-separated search keywords for the menu-bar and tray settings.
  ///
  /// In en, this message translates to:
  /// **'tray menu bar recent preview privacy count'**
  String get trayRecentSettingsKeywords;

  /// Title of the transform palette dialog.
  ///
  /// In en, this message translates to:
  /// **'Transform text'**
  String get transformsPaletteTitle;

  /// Placeholder of the filter field in the transform palette.
  ///
  /// In en, this message translates to:
  /// **'Filter transforms'**
  String get transformsFilterPlaceholder;

  /// Shown in the palette list when the filter matches nothing.
  ///
  /// In en, this message translates to:
  /// **'No transforms match'**
  String get transformsNoMatches;

  /// Palette list group with filtered results.
  ///
  /// In en, this message translates to:
  /// **'Results'**
  String get transformsGroupResults;

  /// Palette list group with transforms suggested for the content.
  ///
  /// In en, this message translates to:
  /// **'Suggested'**
  String get transformsGroupSuggested;

  /// Palette list group with recently used transforms.
  ///
  /// In en, this message translates to:
  /// **'Recent'**
  String get transformsGroupRecent;

  /// Palette list group with favorite transforms.
  ///
  /// In en, this message translates to:
  /// **'Favorites'**
  String get transformsGroupFavorites;

  /// Palette list group with saved pipelines (chains of transforms).
  ///
  /// In en, this message translates to:
  /// **'Pipelines'**
  String get transformsGroupPipelines;

  /// Transform category name.
  ///
  /// In en, this message translates to:
  /// **'JSON, YAML, CSV and XML'**
  String get transformsCategoryStructure;

  /// Transform category name.
  ///
  /// In en, this message translates to:
  /// **'Encode and decode'**
  String get transformsCategoryEncode;

  /// Transform category name: letter case changes.
  ///
  /// In en, this message translates to:
  /// **'Case'**
  String get transformsCategoryCase;

  /// Transform category name.
  ///
  /// In en, this message translates to:
  /// **'Hashes'**
  String get transformsCategoryHash;

  /// Transform category name.
  ///
  /// In en, this message translates to:
  /// **'Lines and whitespace'**
  String get transformsCategoryLines;

  /// Transform category name.
  ///
  /// In en, this message translates to:
  /// **'Clean up'**
  String get transformsCategoryClean;

  /// Transform category name: URL cleanup.
  ///
  /// In en, this message translates to:
  /// **'Links'**
  String get transformsCategoryUrl;

  /// Transform category name.
  ///
  /// In en, this message translates to:
  /// **'Rich text and Markdown'**
  String get transformsCategoryMarkup;

  /// Transform category name: tokens, timestamps, find and replace.
  ///
  /// In en, this message translates to:
  /// **'Developer'**
  String get transformsCategoryDev;

  /// Transform category name for transforms of an unknown category.
  ///
  /// In en, this message translates to:
  /// **'Other'**
  String get transformsCategoryOther;

  /// Label of one step in a transform chain: its position and transform name.
  ///
  /// In en, this message translates to:
  /// **'{step} · {name}'**
  String transformsStepLabel(int step, String name);

  /// Label of the highlighted transform that is not yet a step of the chain.
  ///
  /// In en, this message translates to:
  /// **'Then: {name}'**
  String transformsThenLabel(String name);

  /// Switch label that enables or disables one step of a chain.
  ///
  /// In en, this message translates to:
  /// **'Run this step'**
  String get transformsStepEnabled;

  /// Button that moves a step or favorite one place earlier.
  ///
  /// In en, this message translates to:
  /// **'Move up'**
  String get transformsMoveEarlier;

  /// Button that moves a step or favorite one place later.
  ///
  /// In en, this message translates to:
  /// **'Move down'**
  String get transformsMoveLater;

  /// Button that removes one step of a chain.
  ///
  /// In en, this message translates to:
  /// **'Remove step'**
  String get transformsRemoveStep;

  /// Button that saves the current chain as a named pipeline.
  ///
  /// In en, this message translates to:
  /// **'Save as pipeline'**
  String get transformsSavePipeline;

  /// Title of the dialog that names a new pipeline.
  ///
  /// In en, this message translates to:
  /// **'Save pipeline'**
  String get transformsSavePipelineTitle;

  /// Error when saving a pipeline fails.
  ///
  /// In en, this message translates to:
  /// **'The pipeline could not be saved.'**
  String get transformsSavePipelineFailed;

  /// Label of the pipeline name field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get transformsPipelineNameLabel;

  /// Label of the pipeline description field.
  ///
  /// In en, this message translates to:
  /// **'Description (optional)'**
  String get transformsPipelineDescriptionLabel;

  /// Heading of the step list in the pipeline editor.
  ///
  /// In en, this message translates to:
  /// **'Steps'**
  String get transformsPipelineStepsLabel;

  /// Shown in the pipeline editor when it has no steps.
  ///
  /// In en, this message translates to:
  /// **'Add at least one step.'**
  String get transformsPipelineNoSteps;

  /// Validation message when the pipeline name is empty.
  ///
  /// In en, this message translates to:
  /// **'Enter a name.'**
  String get transformsPipelineNameRequired;

  /// Validation message when the pipeline name is too long.
  ///
  /// In en, this message translates to:
  /// **'Use at most 80 characters.'**
  String get transformsPipelineNameTooLong;

  /// Validation message when the pipeline name is already used.
  ///
  /// In en, this message translates to:
  /// **'Another pipeline already has this name.'**
  String get transformsPipelineNameTaken;

  /// Toast title after saving a pipeline.
  ///
  /// In en, this message translates to:
  /// **'Pipeline saved'**
  String get transformsPipelineSavedTitle;

  /// Toast message after saving a pipeline.
  ///
  /// In en, this message translates to:
  /// **'{name} is now under Pipelines.'**
  String transformsPipelineSavedMessage(String name);

  /// Number of steps of a pipeline.
  ///
  /// In en, this message translates to:
  /// **'Steps: {count}'**
  String transformsPipelineSteps(int count);

  /// Name of a duplicated pipeline.
  ///
  /// In en, this message translates to:
  /// **'{name} ({index})'**
  String transformsPipelineCopyName(String name, int index);

  /// Shown where a favorite or default names a pipeline that no longer exists.
  ///
  /// In en, this message translates to:
  /// **'Deleted pipeline'**
  String get transformsRemovedPipeline;

  /// Shown for a pipeline or step made by a newer app version; it cannot run here.
  ///
  /// In en, this message translates to:
  /// **'Needs a newer CopyPaste version'**
  String get transformsNeedsNewerVersion;

  /// Error when the transform catalogue or settings cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Transforms could not be loaded.'**
  String get transformsLoadFailed;

  /// Title shown when the palette opens on an image, file or color item.
  ///
  /// In en, this message translates to:
  /// **'Transforms work on text'**
  String get transformsNotTextTitle;

  /// Explanation shown when the palette opens on a non-text item.
  ///
  /// In en, this message translates to:
  /// **'This clip is an image, a file or a color. Pick a text clip or copy some text.'**
  String get transformsNotTextMessage;

  /// Title shown when there is no source text for the palette.
  ///
  /// In en, this message translates to:
  /// **'Nothing to transform'**
  String get transformsNothingTitle;

  /// Hint shown when there is no source text for the palette.
  ///
  /// In en, this message translates to:
  /// **'Copy some text or pick a clip from history.'**
  String get transformsNothingMessage;

  /// Shown in the preview before any transform is highlighted.
  ///
  /// In en, this message translates to:
  /// **'Pick a transform'**
  String get transformsPickTransform;

  /// Keyboard hint in the empty preview on desktop.
  ///
  /// In en, this message translates to:
  /// **'Tab adds it as a step. Enter applies the result.'**
  String get transformsPickTransformHint;

  /// Status line while the preview is computed.
  ///
  /// In en, this message translates to:
  /// **'Updating preview…'**
  String get transformsPreviewing;

  /// Counts of the shown output, announced to screen readers when the preview updates.
  ///
  /// In en, this message translates to:
  /// **'Lines: {lines} · Words: {words} · Characters: {chars}'**
  String transformsCounts(int lines, int words, int chars);

  /// Preview status when the output equals the input.
  ///
  /// In en, this message translates to:
  /// **'No changes'**
  String get transformsNoChanges;

  /// Preview status when only a prefix of a long text is shown.
  ///
  /// In en, this message translates to:
  /// **'Showing the first 64 KiB of {size}'**
  String transformsTruncated(String size);

  /// Preview status when the chain contains a random transform such as shuffle.
  ///
  /// In en, this message translates to:
  /// **'Random'**
  String get transformsRandom;

  /// Preview of a step after a failed step.
  ///
  /// In en, this message translates to:
  /// **'Not run'**
  String get transformsNotRun;

  /// Shown instead of a sensitive preview until it is revealed.
  ///
  /// In en, this message translates to:
  /// **'The result is sensitive and hidden.'**
  String get transformsSensitiveHidden;

  /// Button that shows a hidden sensitive preview.
  ///
  /// In en, this message translates to:
  /// **'Reveal'**
  String get transformsReveal;

  /// Preview tab with the transform output.
  ///
  /// In en, this message translates to:
  /// **'Output'**
  String get transformsPaneOutput;

  /// Preview tab with the transform input.
  ///
  /// In en, this message translates to:
  /// **'Input'**
  String get transformsPaneInput;

  /// Preview tab that highlights what the transform changed.
  ///
  /// In en, this message translates to:
  /// **'Changes'**
  String get transformsPaneChanges;

  /// Title of an error about the source text.
  ///
  /// In en, this message translates to:
  /// **'Can\'t transform this text'**
  String get transformsCannotTransform;

  /// Preview error naming the failed step of a chain.
  ///
  /// In en, this message translates to:
  /// **'Step {step} · {name}: {message}'**
  String transformsStepFailed(int step, String name, String message);

  /// Button offered when decoded data is binary: switch the step to hex output.
  ///
  /// In en, this message translates to:
  /// **'Show as hex'**
  String get transformsUseHexOutput;

  /// Palette action that pastes the result into the previous app.
  ///
  /// In en, this message translates to:
  /// **'Paste'**
  String get transformsActionPaste;

  /// Palette action that stores the result as a new history item.
  ///
  /// In en, this message translates to:
  /// **'Save as new clip'**
  String get transformsActionSave;

  /// Toast message after copying a transform result.
  ///
  /// In en, this message translates to:
  /// **'The result is also in history.'**
  String get transformsCopiedMessage;

  /// Toast title after saving a transform result as a new item.
  ///
  /// In en, this message translates to:
  /// **'Saved'**
  String get transformsSavedTitle;

  /// Toast message after saving a transform result as a new item.
  ///
  /// In en, this message translates to:
  /// **'The result is a new clip in history.'**
  String get transformsSavedMessage;

  /// Toast title when a transformed paste cannot reach the target app.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t paste'**
  String get transformsPasteFailedTitle;

  /// Toast message when a transformed paste cannot reach the target app.
  ///
  /// In en, this message translates to:
  /// **'The result is on the clipboard. Paste it manually.'**
  String get transformsPasteFailedMessage;

  /// A transform error with its position in the input.
  ///
  /// In en, this message translates to:
  /// **'{message} at line {line}, column {column}'**
  String transformsErrorAt(String message, int line, int column);

  /// Transform error: decoded bytes are not text.
  ///
  /// In en, this message translates to:
  /// **'Decoded data is binary, not text'**
  String get transformsErrorBinary;

  /// Transform error: input or output is over the size limit.
  ///
  /// In en, this message translates to:
  /// **'Too large to transform'**
  String get transformsErrorTooLarge;

  /// Transform error: a parameter value is refused.
  ///
  /// In en, this message translates to:
  /// **'A setting of this step is not valid'**
  String get transformsErrorInvalidParam;

  /// Transform error: the computation was cancelled.
  ///
  /// In en, this message translates to:
  /// **'Cancelled'**
  String get transformsErrorCancelled;

  /// Transform error: a pattern compiles to something too large to run.
  ///
  /// In en, this message translates to:
  /// **'Pattern too expensive'**
  String get transformsErrorTooExpensive;

  /// Transform error: an empty or invalid search pattern.
  ///
  /// In en, this message translates to:
  /// **'The pattern is not valid'**
  String get transformsErrorPattern;

  /// Transform error: a timestamp needs its unit.
  ///
  /// In en, this message translates to:
  /// **'Choose whether the number is in seconds or milliseconds'**
  String get transformsErrorTimestampUnit;

  /// Transform error: the step did not finish within its time limit. Shown after the step name, like the other transform errors.
  ///
  /// In en, this message translates to:
  /// **'Took too long'**
  String get transformsErrorTimeLimit;

  /// Transform error: XML that references external entities.
  ///
  /// In en, this message translates to:
  /// **'External entities are not allowed'**
  String get transformsErrorExternalEntity;

  /// Transform error: the JWT is encrypted.
  ///
  /// In en, this message translates to:
  /// **'Encrypted tokens can\'t be decoded'**
  String get transformsErrorJwtEncrypted;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid JSON'**
  String get transformsErrorJson;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid YAML'**
  String get transformsErrorYaml;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid XML'**
  String get transformsErrorXml;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid CSV'**
  String get transformsErrorCsv;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid Base64'**
  String get transformsErrorBase64;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid hex'**
  String get transformsErrorHex;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Invalid escape sequence'**
  String get transformsErrorEscape;

  /// Transform error in URL decoding.
  ///
  /// In en, this message translates to:
  /// **'Malformed percent-encoding'**
  String get transformsErrorUrl;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Not a valid JWT'**
  String get transformsErrorJwt;

  /// Transform error.
  ///
  /// In en, this message translates to:
  /// **'Not a recognized date or timestamp'**
  String get transformsErrorDate;

  /// Transform error with an unknown code.
  ///
  /// In en, this message translates to:
  /// **'This step can\'t transform its input'**
  String get transformsErrorGeneric;

  /// Transform warning: a signed URL was not modified.
  ///
  /// In en, this message translates to:
  /// **'Signed link left unchanged'**
  String get transformsWarningSignedLink;

  /// Transform warning: the decoded token has expired.
  ///
  /// In en, this message translates to:
  /// **'Expired'**
  String get transformsWarningExpired;

  /// Transform warning: the decoded token is not valid yet.
  ///
  /// In en, this message translates to:
  /// **'Not yet valid'**
  String get transformsWarningNotYetValid;

  /// Transform warning: a decoded token's signature is not checked.
  ///
  /// In en, this message translates to:
  /// **'Signature not verified'**
  String get transformsWarningSignatureNotVerified;

  /// Transform warning: the input has duplicate keys.
  ///
  /// In en, this message translates to:
  /// **'Duplicate keys'**
  String get transformsWarningDuplicateKeys;

  /// Transform warning: trailing commas or comments were accepted.
  ///
  /// In en, this message translates to:
  /// **'Accepted non-standard JSON'**
  String get transformsWarningLenientJson;

  /// Transform warning.
  ///
  /// In en, this message translates to:
  /// **'The JSON was encoded twice'**
  String get transformsWarningDoubleEncoded;

  /// Transform warning.
  ///
  /// In en, this message translates to:
  /// **'Unknown escape sequences kept as written'**
  String get transformsWarningEscapes;

  /// Transform warning: a date without a zone was read as local time.
  ///
  /// In en, this message translates to:
  /// **'Local time zone assumed'**
  String get transformsWarningLocalZone;

  /// Transform warning: an extraction found nothing.
  ///
  /// In en, this message translates to:
  /// **'No matches'**
  String get transformsWarningNoMatches;

  /// Transform warning with an unknown code.
  ///
  /// In en, this message translates to:
  /// **'Check the result'**
  String get transformsWarningGeneric;

  /// Settings row title: put the previous clipboard back after a transformed paste.
  ///
  /// In en, this message translates to:
  /// **'Restore clipboard after paste'**
  String get transformsRestoreTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Bring back the previous clipboard after pasting.'**
  String get transformsRestoreDescription;

  /// Settings row title: a pipeline applied when pasting from Quick Paste.
  ///
  /// In en, this message translates to:
  /// **'Default paste transform'**
  String get transformsDefaultTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Applied to text you paste from Quick Paste.'**
  String get transformsDefaultDescription;

  /// Option meaning no default paste transform.
  ///
  /// In en, this message translates to:
  /// **'None'**
  String get transformsNoDefault;

  /// Settings row title: default paste transforms for specific apps.
  ///
  /// In en, this message translates to:
  /// **'Defaults per app'**
  String get transformsAppDefaultsTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Use a different default in specific apps.'**
  String get transformsAppDefaultsDescription;

  /// Marks a per-app default made for another platform.
  ///
  /// In en, this message translates to:
  /// **'only on {platform}'**
  String transformsOtherPlatform(String platform);

  /// Button that chooses an application to add a per-app default for.
  ///
  /// In en, this message translates to:
  /// **'Add app'**
  String get transformsAddApp;

  /// Button that removes an entry from a list.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get transformsRemove;

  /// Settings row title: favorite transforms and pipelines.
  ///
  /// In en, this message translates to:
  /// **'Favorites'**
  String get transformsFavoritesTitle;

  /// Description of the Favorites row in Text transforms settings: where favorite transforms appear in the Transform text dialog.
  ///
  /// In en, this message translates to:
  /// **'Shown near the top when you transform text.'**
  String get transformsFavoritesDescription;

  /// Shortcut of a favorite in the palette.
  ///
  /// In en, this message translates to:
  /// **'{shortcut} in Transform text'**
  String transformsFavoriteShortcut(String shortcut);

  /// Placeholder of the selector that adds a favorite.
  ///
  /// In en, this message translates to:
  /// **'Add a favorite'**
  String get transformsAddFavorite;

  /// Settings row title: saved chains of transforms.
  ///
  /// In en, this message translates to:
  /// **'Pipelines'**
  String get transformsPipelinesTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Saved chains of transforms.'**
  String get transformsPipelinesDescription;

  /// Button and drawer title for creating a pipeline.
  ///
  /// In en, this message translates to:
  /// **'New pipeline'**
  String get transformsNewPipeline;

  /// Button and drawer title for editing a pipeline.
  ///
  /// In en, this message translates to:
  /// **'Edit pipeline'**
  String get transformsEditPipeline;

  /// Button that copies a pipeline.
  ///
  /// In en, this message translates to:
  /// **'Duplicate'**
  String get transformsDuplicatePipeline;

  /// Button that deletes a pipeline.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get transformsDeletePipeline;

  /// Title of the dialog confirming a pipeline deletion.
  ///
  /// In en, this message translates to:
  /// **'Delete {name}?'**
  String transformsDeletePipelineTitle(String name);

  /// Explanation in the delete pipeline dialog.
  ///
  /// In en, this message translates to:
  /// **'Favorites and paste defaults using it are cleared.'**
  String get transformsDeletePipelineMessage;

  /// Error when deleting a pipeline fails.
  ///
  /// In en, this message translates to:
  /// **'The pipeline could not be deleted.'**
  String get transformsDeletePipelineFailed;

  /// Placeholder of the selector that adds a step to a pipeline.
  ///
  /// In en, this message translates to:
  /// **'Add a step'**
  String get transformsAddStep;

  /// Settings row title: transforms hidden from the palette.
  ///
  /// In en, this message translates to:
  /// **'Hidden transforms'**
  String get transformsHiddenTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Not listed or suggested.'**
  String get transformsHiddenDescription;

  /// Placeholder of the selector that hides a transform.
  ///
  /// In en, this message translates to:
  /// **'Hide a transform'**
  String get transformsHideTransform;

  /// Button that shows a hidden transform again.
  ///
  /// In en, this message translates to:
  /// **'Show'**
  String get transformsShow;

  /// Settings row title: suggested transforms in the palette.
  ///
  /// In en, this message translates to:
  /// **'Suggestions'**
  String get transformsSuggestionsTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Suggest transforms that fit the text.'**
  String get transformsSuggestionsDescription;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'JSON'**
  String get transformsDetectorJson;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'JSON Web Tokens'**
  String get transformsDetectorJwt;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'Base64'**
  String get transformsDetectorBase64;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'Links with tracking parameters'**
  String get transformsDetectorTrackingUrl;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'Timestamps'**
  String get transformsDetectorTimestamp;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'HTML'**
  String get transformsDetectorHtml;

  /// Content detector name.
  ///
  /// In en, this message translates to:
  /// **'Markdown'**
  String get transformsDetectorMarkdown;

  /// Settings row title: URL parameters removed by the Remove tracking parameters transform.
  ///
  /// In en, this message translates to:
  /// **'Tracking parameters'**
  String get transformsTrackingTitle;

  /// Settings row description.
  ///
  /// In en, this message translates to:
  /// **'Link parameters to also remove or always keep.'**
  String get transformsTrackingDescription;

  /// Label of the custom list of tracking parameters to remove.
  ///
  /// In en, this message translates to:
  /// **'Also remove'**
  String get transformsTrackingRemoveLabel;

  /// Label of the list of parameters that are never removed.
  ///
  /// In en, this message translates to:
  /// **'Always keep'**
  String get transformsTrackingAllowLabel;

  /// Placeholder of the tracking parameter field.
  ///
  /// In en, this message translates to:
  /// **'name or name@domain, then Enter'**
  String get transformsTrackingPlaceholder;

  /// Error title in Android Settings > Quick access when the keyboard, tile and widget status cannot be read.
  ///
  /// In en, this message translates to:
  /// **'Quick access status unavailable'**
  String get quickAccessUnavailableTitle;

  /// Notice title in Android Settings > Quick access when a system screen could not be opened or an option could not be saved.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t complete that'**
  String get quickAccessActionFailedTitle;

  /// Notice message under quickAccessActionFailedTitle.
  ///
  /// In en, this message translates to:
  /// **'Android didn\'t open the requested screen or save the change. Try again.'**
  String get quickAccessActionFailedMessage;

  /// Setting row title: the CopyPaste clips keyboard panel (an Android input method that inserts saved clips).
  ///
  /// In en, this message translates to:
  /// **'Clips keyboard'**
  String get quickAccessKeyboardTitle;

  /// Setting row description for the clips keyboard. Explains turning it on and the system warning, and states that the panel never records keystrokes.
  ///
  /// In en, this message translates to:
  /// **'Turn on CopyPaste clips in keyboard settings.'**
  String get quickAccessKeyboardDescription;

  /// Status under the clips keyboard row: the keyboard is turned on in system settings.
  ///
  /// In en, this message translates to:
  /// **'Turned on'**
  String get quickAccessKeyboardOn;

  /// Status under the clips keyboard row: the keyboard is turned off in system settings.
  ///
  /// In en, this message translates to:
  /// **'Turned off'**
  String get quickAccessKeyboardOff;

  /// Button that opens the Android system screen where keyboards are turned on.
  ///
  /// In en, this message translates to:
  /// **'Open keyboard settings'**
  String get quickAccessOpenKeyboardSettings;

  /// Settings search result description for the clips keyboard row.
  ///
  /// In en, this message translates to:
  /// **'Turn on the CopyPaste clips keyboard.'**
  String get quickAccessKeyboardSearchDescription;

  /// Space-separated search keywords for the clips keyboard row; translate and add local synonyms.
  ///
  /// In en, this message translates to:
  /// **'keyboard input method ime panel enable'**
  String get quickAccessKeyboardKeywords;

  /// Setting row title: make the clips keyboard the current keyboard.
  ///
  /// In en, this message translates to:
  /// **'Switch to the clips keyboard'**
  String get quickAccessSwitchKeyboardTitle;

  /// Button that shows the Android keyboard picker.
  ///
  /// In en, this message translates to:
  /// **'Choose keyboard'**
  String get quickAccessSwitchKeyboardAction;

  /// Status: the clips keyboard is the current keyboard.
  ///
  /// In en, this message translates to:
  /// **'Current keyboard'**
  String get quickAccessKeyboardCurrent;

  /// Setting row title: how copies made in other apps are saved on this device.
  ///
  /// In en, this message translates to:
  /// **'Capture mode'**
  String get quickAccessCaptureTierTitle;

  /// Settings search result description for the capture mode row.
  ///
  /// In en, this message translates to:
  /// **'See how copies from other apps are saved.'**
  String get quickAccessCaptureTierSearchDescription;

  /// Space-separated search keywords for the capture mode row; translate and add local synonyms.
  ///
  /// In en, this message translates to:
  /// **'capture tier full panel manual shizuku collect'**
  String get quickAccessCaptureTierKeywords;

  /// Capture mode status: background capture granted through Shizuku or ADB saves every copy.
  ///
  /// In en, this message translates to:
  /// **'Full: every copy is saved in the background'**
  String get quickAccessTierFull;

  /// Capture mode status: the clips keyboard is the current keyboard and saves each copy.
  ///
  /// In en, this message translates to:
  /// **'Clips keyboard: saves every copy while selected'**
  String get quickAccessTierPanelCollecting;

  /// Capture mode status: the clips keyboard is turned on and saves the clipboard when it opens.
  ///
  /// In en, this message translates to:
  /// **'Clips keyboard: saves the clipboard when opened'**
  String get quickAccessTierPanelCatchUp;

  /// Capture mode status: copies are saved only by an explicit action.
  ///
  /// In en, this message translates to:
  /// **'Manual: save from the app, a tile, the widget, or Share'**
  String get quickAccessTierManual;

  /// Capture mode status: the user paused clipboard capture.
  ///
  /// In en, this message translates to:
  /// **'Capture paused'**
  String get quickAccessTierPaused;

  /// Switch title: after inserting a clip, the panel switches back to the keyboard used before.
  ///
  /// In en, this message translates to:
  /// **'Return to previous keyboard after inserting'**
  String get quickAccessAutoReturnTitle;

  /// Switch description for returning to the previous keyboard after inserting.
  ///
  /// In en, this message translates to:
  /// **'Switch back after inserting one clip.'**
  String get quickAccessAutoReturnDescription;

  /// Space-separated search keywords for the return-to-previous-keyboard switch; translate and add local synonyms.
  ///
  /// In en, this message translates to:
  /// **'switch back previous keyboard auto return insert'**
  String get quickAccessAutoReturnKeywords;

  /// Switch title: the clips panel opens with its search field focused.
  ///
  /// In en, this message translates to:
  /// **'Focus search when the clips keyboard opens'**
  String get quickAccessFocusSearchTitle;

  /// Setting row title for the Android Quick Settings tiles; also the search result label.
  ///
  /// In en, this message translates to:
  /// **'Quick Settings tiles'**
  String get quickAccessTilesTitle;

  /// Settings search result description for the Quick Settings tile rows.
  ///
  /// In en, this message translates to:
  /// **'Add the Save clipboard and Clipboard capture tiles.'**
  String get quickAccessTilesSearchDescription;

  /// Space-separated search keywords for the Quick Settings tile rows; translate and add local synonyms.
  ///
  /// In en, this message translates to:
  /// **'quick settings tile shade save pause'**
  String get quickAccessTilesKeywords;

  /// How to add a CopyPaste tile by hand, shown when Android cannot or will not show the add-tile prompt.
  ///
  /// In en, this message translates to:
  /// **'Swipe down twice, tap Edit, and add a CopyPaste tile.'**
  String get quickAccessTileInstructions;

  /// Setting row title for the Save clipboard Quick Settings tile.
  ///
  /// In en, this message translates to:
  /// **'Save clipboard tile'**
  String get quickAccessSaveTileTitle;

  /// Setting row description for the Save clipboard tile.
  ///
  /// In en, this message translates to:
  /// **'Saves what\'s on the clipboard from Quick Settings.'**
  String get quickAccessSaveTileDescription;

  /// Setting row title for the Clipboard capture Quick Settings tile.
  ///
  /// In en, this message translates to:
  /// **'Clipboard capture tile'**
  String get quickAccessCaptureTileTitle;

  /// Setting row description for the Clipboard capture tile.
  ///
  /// In en, this message translates to:
  /// **'Pauses and resumes capture from Quick Settings.'**
  String get quickAccessCaptureTileDescription;

  /// Button that asks Android to add the tile to Quick Settings.
  ///
  /// In en, this message translates to:
  /// **'Add tile'**
  String get quickAccessAddTile;

  /// Status: the tile is in Quick Settings.
  ///
  /// In en, this message translates to:
  /// **'Added to Quick Settings'**
  String get quickAccessTileAdded;

  /// Setting row title for the Recent clips home-screen widget.
  ///
  /// In en, this message translates to:
  /// **'Recent clips widget'**
  String get quickAccessWidgetTitle;

  /// Setting row description for the Recent clips widget.
  ///
  /// In en, this message translates to:
  /// **'Recent clips on your home screen, one tap to copy.'**
  String get quickAccessWidgetDescription;

  /// How to add the widget by hand, shown when the launcher cannot pin widgets.
  ///
  /// In en, this message translates to:
  /// **'Hold the home screen, tap Widgets, and add Recent clips.'**
  String get quickAccessWidgetInstructions;

  /// Status: how many Recent clips widgets are on the home screen.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{Not added} =1{1 widget added} other{{count} widgets added}}'**
  String quickAccessWidgetCount(int count);

  /// Button that asks the launcher to add the Recent clips widget.
  ///
  /// In en, this message translates to:
  /// **'Add widget'**
  String get quickAccessAddWidget;

  /// Space-separated search keywords for the widget row; translate and add local synonyms.
  ///
  /// In en, this message translates to:
  /// **'home screen widget launcher recent'**
  String get quickAccessWidgetKeywords;

  /// Notice in the clip editor for clips over 256 KB.
  ///
  /// In en, this message translates to:
  /// **'Too large to edit. This clip opens read-only.'**
  String get snippetsClipTooLarge;

  /// Confirms the in-app owner check on devices without device authentication.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get snippetsConcealedConfirmAction;

  /// In-app owner check shown when the system cannot verify the device owner.
  ///
  /// In en, this message translates to:
  /// **'This device can\'t verify you. Continue only on your own device.'**
  String get snippetsConcealedConfirmMessage;

  /// Title of the in-app owner check.
  ///
  /// In en, this message translates to:
  /// **'Confirm it\'s you'**
  String get snippetsConcealedConfirmTitle;

  /// Unsaved-changes dialog action that closes without saving.
  ///
  /// In en, this message translates to:
  /// **'Discard'**
  String get snippetsDiscard;

  /// Title of the clip editor.
  ///
  /// In en, this message translates to:
  /// **'Edit clip'**
  String get snippetsEditClip;

  /// Notice when editing text that has rich formatting.
  ///
  /// In en, this message translates to:
  /// **'Formatting will be removed when you save.'**
  String get snippetsFormattingNotice;

  /// Title of the editor for new text without a source clip.
  ///
  /// In en, this message translates to:
  /// **'New text'**
  String get snippetsNewText;

  /// Clip editor switch that deletes the original clip after saving.
  ///
  /// In en, this message translates to:
  /// **'Replace original'**
  String get snippetsReplaceOriginal;

  /// Hint under the Replace original switch.
  ///
  /// In en, this message translates to:
  /// **'Deletes the original clip after saving.'**
  String get snippetsReplaceOriginalHint;

  /// Error title in editors.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save'**
  String get snippetsSaveFailed;

  /// Message of the unsaved changes dialog.
  ///
  /// In en, this message translates to:
  /// **'Save your changes before closing?'**
  String get snippetsUnsavedMessage;

  /// Title of the unsaved changes dialog.
  ///
  /// In en, this message translates to:
  /// **'Unsaved changes'**
  String get snippetsUnsavedTitle;

  /// Quick Paste tab that shows the paste queue.
  ///
  /// In en, this message translates to:
  /// **'Queue'**
  String get quickPasteTabQueue;

  /// Notice in Quick Paste while the paste target is a password field, so every preview is masked.
  ///
  /// In en, this message translates to:
  /// **'Password field: previews are hidden'**
  String get quickPasteMaskedMode;

  /// Masked row label in Quick Paste; the content is hidden and only its kind is named.
  ///
  /// In en, this message translates to:
  /// **'Hidden: {kind}'**
  String quickPasteMaskedItem(String kind);

  /// Button label in Quick Paste that verifies the device owner to show and paste secret items.
  ///
  /// In en, this message translates to:
  /// **'Unlock secret items'**
  String get quickPasteUnlockSecrets;

  /// Chip in Quick Paste naming the transform pipeline that Enter pastes text through.
  ///
  /// In en, this message translates to:
  /// **'Pasting with: {name}'**
  String quickPastePastingWith(String name);

  /// Hint next to the default transform chip naming the shortcut that pastes the original instead.
  ///
  /// In en, this message translates to:
  /// **'{shortcut} pastes the original'**
  String quickPastePasteOriginalHint(String shortcut);

  /// Toast title when the default paste transform failed on an item.
  ///
  /// In en, this message translates to:
  /// **'Default transform skipped'**
  String get quickPasteDefaultSkipped;

  /// Toast message when the default paste transform failed and the original item was used.
  ///
  /// In en, this message translates to:
  /// **'The original was used instead.'**
  String get quickPasteDefaultSkippedMessage;

  /// Toast message when Quick Paste put content on the clipboard but could not paste it.
  ///
  /// In en, this message translates to:
  /// **'Paste it manually.'**
  String get quickPastePasteManually;

  /// Error title when the content rules cannot be read.
  ///
  /// In en, this message translates to:
  /// **'Content rules could not be loaded.'**
  String get rulesLoadFailed;

  /// Button and editor title that creates a content rule.
  ///
  /// In en, this message translates to:
  /// **'New rule'**
  String get rulesNewRule;

  /// Title of the rule editor for an existing rule.
  ///
  /// In en, this message translates to:
  /// **'Edit rule'**
  String get rulesEditRule;

  /// Shown for a rule without a name.
  ///
  /// In en, this message translates to:
  /// **'Untitled rule'**
  String get rulesUntitled;

  /// Name of the shipped rule that saves detected passwords and secrets as secret. Shown wherever the rule is named, unless the user renamed it.
  ///
  /// In en, this message translates to:
  /// **'Detected credentials'**
  String get rulesBuiltinCredentials;

  /// Name of the shipped rule that saves detected payment card numbers as secret. Shown wherever the rule is named, unless the user renamed it.
  ///
  /// In en, this message translates to:
  /// **'Payment card numbers'**
  String get rulesBuiltinPaymentCards;

  /// Name of the shipped rule, off by default, that deletes detected passwords and secrets after a short time. Shown wherever the rule is named, unless the user renamed it.
  ///
  /// In en, this message translates to:
  /// **'Delete detected credentials'**
  String get rulesBuiltinCredentialsExpiry;

  /// Heading of the device Privacy settings that guard capture before any content rule.
  ///
  /// In en, this message translates to:
  /// **'Privacy defaults'**
  String get rulesPrivacyDefaultsTitle;

  /// Description of the Privacy defaults row. The built-in rules are listed below it.
  ///
  /// In en, this message translates to:
  /// **'Privacy settings apply before any rule.'**
  String get rulesPrivacyDefaultsDescription;

  /// Search keywords for the Privacy defaults row of Content rules.
  ///
  /// In en, this message translates to:
  /// **'privacy concealed temporary excluded apps built-in credentials passwords payment cards'**
  String get rulesPrivacyDefaultsKeywords;

  /// Button that opens the Privacy settings section.
  ///
  /// In en, this message translates to:
  /// **'Open Privacy'**
  String get rulesOpenPrivacy;

  /// Heading of the list of rules the user created.
  ///
  /// In en, this message translates to:
  /// **'My rules'**
  String get rulesMyRulesTitle;

  /// Description of the user rules list.
  ///
  /// In en, this message translates to:
  /// **'Run in order on new copies.'**
  String get rulesMyRulesDescription;

  /// Search keywords for the user rules list.
  ///
  /// In en, this message translates to:
  /// **'content rules automation tag skip secret expire delete pattern regex'**
  String get rulesMyRulesKeywords;

  /// Empty state title when the user has no rules.
  ///
  /// In en, this message translates to:
  /// **'No rules yet'**
  String get rulesEmptyTitle;

  /// Heading of the rules deleted within 30 days.
  ///
  /// In en, this message translates to:
  /// **'Recently deleted'**
  String get rulesDeletedTitle;

  /// Description of the recently deleted rules.
  ///
  /// In en, this message translates to:
  /// **'Restore within 30 days.'**
  String get rulesDeletedDescription;

  /// When a recently deleted rule was deleted.
  ///
  /// In en, this message translates to:
  /// **'Deleted {time}'**
  String rulesDeletedAt(String time);

  /// Button that restores a deleted rule.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get rulesRestore;

  /// Toast title after a rule was deleted.
  ///
  /// In en, this message translates to:
  /// **'Rule deleted'**
  String get rulesDeletedToast;

  /// Toast message after a rule was deleted.
  ///
  /// In en, this message translates to:
  /// **'You can restore it for 30 days.'**
  String get rulesDeletedToastMessage;

  /// Toast action that undoes deleting a rule.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get rulesUndo;

  /// Notice that a concurrent edit of a rule from another device lost and can be restored.
  ///
  /// In en, this message translates to:
  /// **'A concurrent edit of “{rule}” from {device} was replaced by a newer one.'**
  String rulesConflictMessage(String rule, String device);

  /// Used in rulesConflictMessage when the device name is unknown.
  ///
  /// In en, this message translates to:
  /// **'another device'**
  String get rulesAnotherDevice;

  /// Button that restores the version of a rule that lost a concurrent edit.
  ///
  /// In en, this message translates to:
  /// **'Use that version'**
  String get rulesRestoreConflict;

  /// Title of the list of copies content rules did not save.
  ///
  /// In en, this message translates to:
  /// **'Recently skipped'**
  String get rulesSkipLogTitle;

  /// Description of the skip log row.
  ///
  /// In en, this message translates to:
  /// **'Copies your rules didn\'t save on this device.'**
  String get rulesSkipLogDescription;

  /// Search keywords for the skip log row.
  ///
  /// In en, this message translates to:
  /// **'skipped ignored not saved log'**
  String get rulesSkipLogKeywords;

  /// Button that shows the skip log.
  ///
  /// In en, this message translates to:
  /// **'Show'**
  String get rulesSkipLogShow;

  /// Skip log text when no copy was skipped.
  ///
  /// In en, this message translates to:
  /// **'No copies were skipped.'**
  String get rulesSkipLogEmpty;

  /// Skip log entry when the source app is not known.
  ///
  /// In en, this message translates to:
  /// **'Unknown app'**
  String get rulesUnknownApp;

  /// Condition part of a rule summary when the rule has no conditions.
  ///
  /// In en, this message translates to:
  /// **'Any copy'**
  String get rulesMatchesEverything;

  /// One-line rule summary: its conditions, an arrow, its actions.
  ///
  /// In en, this message translates to:
  /// **'{conditions} → {actions}'**
  String rulesSummary(String conditions, String actions);

  /// A rule summary for a rule that stops later rules.
  ///
  /// In en, this message translates to:
  /// **'{summary}, then stop'**
  String rulesSummaryStop(String summary);

  /// Separator between conditions in a rule summary, by match mode (any or all). Keep the spaces.
  ///
  /// In en, this message translates to:
  /// **'{mode, select, any{ or } other{ and }}'**
  String rulesConditionJoin(String mode);

  /// One condition in a rule summary, such as "Source app is Terminal".
  ///
  /// In en, this message translates to:
  /// **'{attribute} {operator} {value}'**
  String rulesConditionSummary(String attribute, String operator, String value);

  /// One action with its value in a rule summary, such as "Delete after 15 minutes".
  ///
  /// In en, this message translates to:
  /// **'{action} {value}'**
  String rulesActionSummary(String action, String value);

  /// The devices a rule runs on.
  ///
  /// In en, this message translates to:
  /// **'Runs on {devices}'**
  String rulesRunsOn(String devices);

  /// Badge on a rule another device added or changed in the last 7 days.
  ///
  /// In en, this message translates to:
  /// **'New from {device}'**
  String rulesNewFromDevice(String device);

  /// Badge on a rule written by a newer CopyPaste.
  ///
  /// In en, this message translates to:
  /// **'Needs update'**
  String get rulesNeedsUpdateBadge;

  /// Shown for a rule this version cannot run.
  ///
  /// In en, this message translates to:
  /// **'Update CopyPaste to run this rule.'**
  String get rulesUpdateRequired;

  /// Badge on a rule that cannot run on this device.
  ///
  /// In en, this message translates to:
  /// **'Error'**
  String get rulesErrorBadge;

  /// Explains the error badge. problem is the reason, a full sentence.
  ///
  /// In en, this message translates to:
  /// **'{problem} Until fixed, matches are saved as secret.'**
  String rulesErrorHint(String problem);

  /// Drag handle of a rule row.
  ///
  /// In en, this message translates to:
  /// **'Drag to reorder'**
  String get rulesReorder;

  /// Menu button of a rule row.
  ///
  /// In en, this message translates to:
  /// **'More actions'**
  String get rulesMoreActions;

  /// Menu item that opens the rule editor.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get rulesEdit;

  /// Menu item and screen reader action that moves a rule in the list.
  ///
  /// In en, this message translates to:
  /// **'{direction, select, up{Move up} down{Move down} top{Move to top} other{Move to bottom}}'**
  String rulesMove(String direction);

  /// Menu item that previews a rule against history and applies it to existing items.
  ///
  /// In en, this message translates to:
  /// **'Apply to existing clips…'**
  String get rulesApplyToItems;

  /// Menu item that resets a built-in rule to its shipped definition.
  ///
  /// In en, this message translates to:
  /// **'Reset to default'**
  String get rulesResetBuiltin;

  /// Label of the rule name field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get rulesNameLabel;

  /// Placeholder of the rule name field.
  ///
  /// In en, this message translates to:
  /// **'For example, Work links'**
  String get rulesNamePlaceholder;

  /// Editor section with the rule conditions.
  ///
  /// In en, this message translates to:
  /// **'When'**
  String get rulesWhenTitle;

  /// Choice of how conditions combine.
  ///
  /// In en, this message translates to:
  /// **'{mode, select, any{Any condition matches} other{All conditions match}}'**
  String rulesMatchMode(String mode);

  /// Editor label of a rule without conditions, which applies to every copy.
  ///
  /// In en, this message translates to:
  /// **'No conditions: this rule applies to every copy.'**
  String get rulesNoConditions;

  /// Button that adds a condition.
  ///
  /// In en, this message translates to:
  /// **'Add condition'**
  String get rulesAddCondition;

  /// Button that removes a condition.
  ///
  /// In en, this message translates to:
  /// **'Remove condition'**
  String get rulesRemoveCondition;

  /// Name of a condition attribute.
  ///
  /// In en, this message translates to:
  /// **'{type, select, content_text{Text} content_type{Content type} detected_kind{Detected kind} source_app{Source app} size{Size} privacy_marker{Privacy marker} capture_origin{Origin} other{Unknown condition}}'**
  String rulesConditionType(String type);

  /// Operator of a text condition.
  ///
  /// In en, this message translates to:
  /// **'{op, select, contains{contains} equals{is} starts_with{starts with} ends_with{ends with} matches_regex{matches the pattern} other{matches}}'**
  String rulesTextOperator(String op);

  /// Negated operator of a text condition.
  ///
  /// In en, this message translates to:
  /// **'{op, select, contains{does not contain} equals{is not} starts_with{does not start with} ends_with{does not end with} matches_regex{does not match the pattern} other{does not match}}'**
  String rulesTextOperatorNegated(String op);

  /// Operator of a choice condition: is or is not one of the chosen values.
  ///
  /// In en, this message translates to:
  /// **'{negate, select, true{is not} other{is}}'**
  String rulesListOperator(String negate);

  /// Operator of a size condition.
  ///
  /// In en, this message translates to:
  /// **'{op, select, less_than{is less than} other{is more than}}'**
  String rulesSizeOperator(String op);

  /// Negated operator of a size condition.
  ///
  /// In en, this message translates to:
  /// **'{op, select, less_than{is not less than} other{is not more than}}'**
  String rulesSizeOperatorNegated(String op);

  /// Unit of a size condition.
  ///
  /// In en, this message translates to:
  /// **'{unit, select, lines{lines} bytes{bytes} other{characters}}'**
  String rulesSizeUnit(String unit);

  /// A size with its unit in a rule summary.
  ///
  /// In en, this message translates to:
  /// **'{count} {unit, select, lines{lines} bytes{bytes} other{characters}}'**
  String rulesSizeValue(int count, String unit);

  /// A content type a condition can match.
  ///
  /// In en, this message translates to:
  /// **'{kind, select, plain_text{Plain text} rich_text{Rich text} image{Image} files{Files} color{Color} link{Link} other{Other}}'**
  String rulesContentKind(String kind);

  /// A kind of content the built-in detector finds.
  ///
  /// In en, this message translates to:
  /// **'{kind, select, credential{Password or secret} url{Web address} email{Email address} phone{Phone number} code{Code} color{Color value} json{JSON} file_path{File path} number{Number} one_time_code{One-time code} payment_card{Payment card number} other{Other}}'**
  String rulesDetectedKind(String kind);

  /// A privacy marker the copying app or system sets.
  ///
  /// In en, this message translates to:
  /// **'{marker, select, concealed{Concealed} transient{Temporary} auto_generated{Auto-generated} excluded_from_history{Excluded from history} other{Other}}'**
  String rulesPrivacyMarker(String marker);

  /// Where a copy came from.
  ///
  /// In en, this message translates to:
  /// **'{origin, select, local_copy{Copied on this device} received{Received from another device} created_in_app{Created in CopyPaste} other{Other}}'**
  String rulesCaptureOrigin(String origin);

  /// Placeholder of the text value of a condition.
  ///
  /// In en, this message translates to:
  /// **'Text to look for'**
  String get rulesValuePlaceholder;

  /// Placeholder of a pattern (regular expression) value. Keep the example pattern unchanged.
  ///
  /// In en, this message translates to:
  /// **'Pattern, for example ^ticket-[0-9]+'**
  String get rulesPatternPlaceholder;

  /// Hint under a pattern value.
  ///
  /// In en, this message translates to:
  /// **'Look-ahead, look-behind and backreferences aren\'t supported.'**
  String get rulesPatternHint;

  /// Switch that makes a text condition case-sensitive.
  ///
  /// In en, this message translates to:
  /// **'Match case'**
  String get rulesCaseSensitive;

  /// Shown on a condition from a newer version, which this version keeps unchanged.
  ///
  /// In en, this message translates to:
  /// **'This condition comes from a newer version of CopyPaste.'**
  String get rulesUnknownConditionNotice;

  /// Inline notice on a source app condition: Android cannot evaluate it outside the CopyPaste keyboard.
  ///
  /// In en, this message translates to:
  /// **'On Android, only copies made with the clips keyboard match.'**
  String get rulesSourceAppNotice;

  /// Inline notice on a privacy marker condition.
  ///
  /// In en, this message translates to:
  /// **'Not every system reports every marker.'**
  String get rulesPrivacyMarkerNotice;

  /// Editor section with the rule actions.
  ///
  /// In en, this message translates to:
  /// **'Then'**
  String get rulesThenTitle;

  /// Shown when a rule has no actions yet.
  ///
  /// In en, this message translates to:
  /// **'Add at least one action.'**
  String get rulesNoActions;

  /// Button that adds an action.
  ///
  /// In en, this message translates to:
  /// **'Add action'**
  String get rulesAddAction;

  /// Button that removes an action.
  ///
  /// In en, this message translates to:
  /// **'Remove action'**
  String get rulesRemoveAction;

  /// Name of a rule action.
  ///
  /// In en, this message translates to:
  /// **'{type, select, dont_save{Don\'t save} mark_secret{Mark as secret} delete_after{Delete after} add_tags{Add tags} transform{Transform} other{Unknown action}}'**
  String rulesActionType(String type);

  /// Why an action cannot be added to the rule.
  ///
  /// In en, this message translates to:
  /// **'{reason, select, dont_save_alone{“Don\'t save” can\'t be combined with other actions.} present{This rule already has this action.} other{A rule can have up to 8 actions.}}'**
  String rulesActionBlock(String reason);

  /// Shown on an action from a newer version.
  ///
  /// In en, this message translates to:
  /// **'This action comes from a newer version of CopyPaste.'**
  String get rulesUnknownActionNotice;

  /// Shown under the Don't save action.
  ///
  /// In en, this message translates to:
  /// **'Matching copies are never stored, indexed or synced.'**
  String get rulesDontSaveWarning;

  /// A duration in minutes.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 minute} other{{count} minutes}}'**
  String rulesMinutes(int count);

  /// A duration in hours.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 hour} other{{count} hours}}'**
  String rulesHours(int count);

  /// A duration in days.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 day} other{{count} days}}'**
  String rulesDays(int count);

  /// Option of the Delete after action.
  ///
  /// In en, this message translates to:
  /// **'Also clear the clipboard if it still holds this copy.'**
  String get rulesClearClipboardAtExpiry;

  /// Shown in the Add tags action when no tags exist.
  ///
  /// In en, this message translates to:
  /// **'Create tags in History to use them here.'**
  String get rulesNoTags;

  /// Shown in the Transform action when no transform is available.
  ///
  /// In en, this message translates to:
  /// **'No transforms are available.'**
  String get rulesNoTransforms;

  /// Option of the Transform action.
  ///
  /// In en, this message translates to:
  /// **'Also replace the clipboard with the result'**
  String get rulesReplaceClipboard;

  /// Editor section with rule options.
  ///
  /// In en, this message translates to:
  /// **'Options'**
  String get rulesOptionsTitle;

  /// Switch that turns a rule on.
  ///
  /// In en, this message translates to:
  /// **'Enabled'**
  String get rulesEnabledSwitch;

  /// Switch that stops later rules when this rule matches.
  ///
  /// In en, this message translates to:
  /// **'Stop processing later rules when this one matches'**
  String get rulesStopProcessing;

  /// Label of the device scope field.
  ///
  /// In en, this message translates to:
  /// **'Runs on'**
  String get rulesRunsOnLabel;

  /// Device scope choice: all devices.
  ///
  /// In en, this message translates to:
  /// **'All devices'**
  String get rulesAllDevices;

  /// Device scope choice: chosen devices.
  ///
  /// In en, this message translates to:
  /// **'Chosen devices'**
  String get rulesChosenDevices;

  /// Editor section that tests the rule on a sample.
  ///
  /// In en, this message translates to:
  /// **'Test'**
  String get rulesTestTitle;

  /// Placeholder of the test sample field.
  ///
  /// In en, this message translates to:
  /// **'Paste or type sample text'**
  String get rulesSamplePlaceholder;

  /// Switch that tests the rule together with every other rule.
  ///
  /// In en, this message translates to:
  /// **'Test together with all rules'**
  String get rulesTestFullSet;

  /// Button that runs the rule test.
  ///
  /// In en, this message translates to:
  /// **'Run test'**
  String get rulesRunTest;

  /// Shown when the rule test fails.
  ///
  /// In en, this message translates to:
  /// **'The test could not run.'**
  String get rulesTestFailed;

  /// Button that previews the rule against recent history.
  ///
  /// In en, this message translates to:
  /// **'Preview against history'**
  String get rulesPreviewHistory;

  /// Alert title when saving a rule fails.
  ///
  /// In en, this message translates to:
  /// **'The rule could not be saved.'**
  String get rulesSaveFailed;

  /// Editor notice for a rule a newer version wrote.
  ///
  /// In en, this message translates to:
  /// **'Made by a newer CopyPaste. Update to edit or run it.'**
  String get rulesReadOnlyNotice;

  /// A problem the runtime found in a rule, by error code.
  ///
  /// In en, this message translates to:
  /// **'{code, select, invalid_definition{The rule could not be read.} unsupported_schema{This rule needs a newer version of CopyPaste.} unknown_condition_type{This condition needs a newer version of CopyPaste.} unknown_action_type{This action needs a newer version of CopyPaste.} too_many_conditions{A rule can have up to 20 conditions.} too_many_actions{A rule can have up to 8 actions.} too_many_rules{Only the first 200 rules run.} no_actions{Add at least one action.} conflicting_actions{“Don\'t save” can\'t be combined with other actions.} duplicate_action{Each action can be used only once.} empty_value{Enter a value.} empty_list{Choose at least one value.} too_many_values{Choose up to 64 values.} invalid_value{This value is not valid.} pattern_too_long{Patterns can be up to 2,000 characters.} unsupported_look_around{Look-ahead and look-behind are not supported. Use two conditions with “All conditions match” instead.} unsupported_backreference{Backreferences are not supported.} pattern_too_large{This pattern is too complex. Simplify its repetitions.} invalid_pattern{This pattern is not valid.} invalid_duration{Choose a duration from 1 minute to 90 days.} invalid_tags{Choose at least one tag.} invalid_transform{Choose a transform.} other{This rule has a problem.}}'**
  String rulesProblem(String code);

  /// One line of a rule trace: a rule or condition and its result.
  ///
  /// In en, this message translates to:
  /// **'{label}: {result}'**
  String rulesTraceLine(String label, String result);

  /// Result of a rule or condition in a trace.
  ///
  /// In en, this message translates to:
  /// **'{result, select, matched{matched} not_matched{not matched} unknown{unknown} skipped{not run} other{error}}'**
  String rulesTraceResult(String result);

  /// A condition in a trace, by its number.
  ///
  /// In en, this message translates to:
  /// **'Condition {number}'**
  String rulesTraceCondition(int number);

  /// Trace when no rule was evaluated.
  ///
  /// In en, this message translates to:
  /// **'No rule was evaluated.'**
  String get rulesTraceNoRules;

  /// Trace note under a rule that stopped later rules.
  ///
  /// In en, this message translates to:
  /// **'Later rules did not run.'**
  String get rulesTraceStopped;

  /// Trace note under a condition whose attribute is unknown for the copy.
  ///
  /// In en, this message translates to:
  /// **'Unknown for this copy, so it doesn\'t match.'**
  String get rulesTraceUnknownReason;

  /// Trace note under a rule that could not run on this device.
  ///
  /// In en, this message translates to:
  /// **'This rule couldn\'t run, so matches are saved as secret.'**
  String get rulesTraceRuleError;

  /// Heading of the combined result in a trace.
  ///
  /// In en, this message translates to:
  /// **'Result'**
  String get rulesTraceOutcomeTitle;

  /// Trace result when no rule changes the copy.
  ///
  /// In en, this message translates to:
  /// **'No rule changes this copy.'**
  String get rulesTraceNoEffect;

  /// Trace result: the copy is not saved.
  ///
  /// In en, this message translates to:
  /// **'Not saved'**
  String get rulesTraceSkipped;

  /// Trace result: the copy is saved as secret.
  ///
  /// In en, this message translates to:
  /// **'Saved as secret'**
  String get rulesTraceSecret;

  /// Trace result: the copy is deleted after a duration.
  ///
  /// In en, this message translates to:
  /// **'Deleted after {duration}'**
  String rulesTraceExpires(String duration);

  /// Trace result: the copy gets tags.
  ///
  /// In en, this message translates to:
  /// **'Tagged {tags}'**
  String rulesTraceTags(String tags);

  /// Trace result: the copy is transformed.
  ///
  /// In en, this message translates to:
  /// **'Transformed with {names}'**
  String rulesTraceTransforms(String names);

  /// Heading of the sample text after the rules' transforms.
  ///
  /// In en, this message translates to:
  /// **'Transformed text'**
  String get rulesTraceOutput;

  /// Title of the dialog that previews and applies a rule to existing items.
  ///
  /// In en, this message translates to:
  /// **'Apply to existing clips'**
  String get rulesPreviewTitle;

  /// Shown while the preview checks recent clips.
  ///
  /// In en, this message translates to:
  /// **'Checking the latest {count} clips…'**
  String rulesPreviewRunning(int count);

  /// Preview result when no recent clip matches.
  ///
  /// In en, this message translates to:
  /// **'No recent clip matches this rule.'**
  String get rulesPreviewNone;

  /// Number of recent clips the rule matches.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip matches} other{{count} clips match}}'**
  String rulesPreviewMatched(int count);

  /// Preview effect: clips that applying would delete.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 would be deleted} other{{count} would be deleted}}'**
  String rulesPreviewDeleted(int count);

  /// Preview effect: clips that applying would mark secret.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 would be marked secret} other{{count} would be marked secret}}'**
  String rulesPreviewSecret(int count);

  /// Preview effect: clips that applying would give a deletion time.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 would get a deletion time} other{{count} would get a deletion time}}'**
  String rulesPreviewExpiring(int count);

  /// Preview effect: clips that applying would tag.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 would be tagged} other{{count} would be tagged}}'**
  String rulesPreviewTagged(int count);

  /// Preview note: older clips past the limit were not checked.
  ///
  /// In en, this message translates to:
  /// **'Only the latest {count} clips were checked.'**
  String rulesPreviewTruncated(int count);

  /// Preview list note: more matching clips than the list shows.
  ///
  /// In en, this message translates to:
  /// **'and {count} more'**
  String rulesPreviewMore(int count);

  /// Preview note about when changes happen.
  ///
  /// In en, this message translates to:
  /// **'Nothing changes until you apply.'**
  String get rulesPreviewNote;

  /// A secret clip in the preview list; its content is never shown.
  ///
  /// In en, this message translates to:
  /// **'Secret clip'**
  String get rulesSecretItem;

  /// Shown after a preview was stopped.
  ///
  /// In en, this message translates to:
  /// **'The preview was stopped. Nothing changed.'**
  String get rulesPreviewCancelled;

  /// Shown when the preview cannot run.
  ///
  /// In en, this message translates to:
  /// **'The preview could not run.'**
  String get rulesPreviewFailed;

  /// Confirmation title before applying a rule that deletes clips.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Delete 1 clip?} other{Delete {count} clips?}}'**
  String rulesApplyDeleteTitle(int count);

  /// Explains that applying a Don't save rule deletes matching clips everywhere.
  ///
  /// In en, this message translates to:
  /// **'Matching clips are deleted on all your devices. This can\'t be undone.'**
  String get rulesApplyDeleteMessage;

  /// Confirmation button that deletes clips and applies the rule.
  ///
  /// In en, this message translates to:
  /// **'Delete and apply'**
  String get rulesApplyDeleteConfirm;

  /// Progress while a rule is applied to existing items.
  ///
  /// In en, this message translates to:
  /// **'Applying: {done} of {total}'**
  String rulesApplyProgress(int done, int total);

  /// Apply report: clips changed.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip changed} other{{count} clips changed}}'**
  String rulesApplyChanged(int count);

  /// Apply report: clips deleted.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip deleted} other{{count} clips deleted}}'**
  String rulesApplyDeleted(int count);

  /// Apply report: clips that could not be changed.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip could not be changed} other{{count} clips could not be changed}}'**
  String rulesApplyFailed(int count);

  /// Shown after applying was stopped.
  ///
  /// In en, this message translates to:
  /// **'Stopped. Changes made so far stay.'**
  String get rulesApplyCancelled;

  /// History clip action that opens the text editor; saving creates a new clip.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get historyActionEdit;

  /// History action that adds the clip, or the selected clips, to the paste queue. Paste Queue is the feature name.
  ///
  /// In en, this message translates to:
  /// **'Add to Paste Queue'**
  String get historyActionAddToQueue;

  /// History clip action that opens the dialog for sending the clip to paired devices now.
  ///
  /// In en, this message translates to:
  /// **'Send to…'**
  String get historyActionSendTo;

  /// Title of the dialog that sends a clip to chosen paired devices.
  ///
  /// In en, this message translates to:
  /// **'Send to device'**
  String get historySendDialogTitle;

  /// Note at the top of the send dialog.
  ///
  /// In en, this message translates to:
  /// **'Sends now, even while automatic sync is paused.'**
  String get historySendDescription;

  /// Shown in the send dialog when no paired device can receive clips yet.
  ///
  /// In en, this message translates to:
  /// **'No paired device has synced yet.'**
  String get historySendNoDevices;

  /// Confirm button of the send dialog.
  ///
  /// In en, this message translates to:
  /// **'Send'**
  String get historySendAction;

  /// Toast title after clips were sent to devices.
  ///
  /// In en, this message translates to:
  /// **'Sent'**
  String get historySentTitle;

  /// Toast title when a manual send failed.
  ///
  /// In en, this message translates to:
  /// **'Not sent'**
  String get historySendFailedTitle;

  /// Error when the device sync settings refuse the clip's kind.
  ///
  /// In en, this message translates to:
  /// **'Sync settings for {device} don\'t allow this clip.'**
  String historySendBlocked(String device);

  /// Error when a paired device could not be reached or refused the send.
  ///
  /// In en, this message translates to:
  /// **'{device} couldn\'t take the clip. Make sure it\'s online and try again.'**
  String historySendFailed(String device);

  /// Error when paired devices could not be listed or the send could not start.
  ///
  /// In en, this message translates to:
  /// **'Sending isn\'t available right now.'**
  String get historySendUnavailable;

  /// History clip action that opens the text transform palette.
  ///
  /// In en, this message translates to:
  /// **'Transform…'**
  String get historyActionTransform;

  /// History clip action that explains which content rules made the clip stored, secret or expiring.
  ///
  /// In en, this message translates to:
  /// **'Why is this here?'**
  String get historyActionExplain;

  /// History clip action that cancels the automatic deletion of an expiring clip.
  ///
  /// In en, this message translates to:
  /// **'Keep'**
  String get historyActionKeep;

  /// Button and tooltip that opens the editor for a new clip the user types.
  ///
  /// In en, this message translates to:
  /// **'New clip'**
  String get historyNewClip;

  /// Toast title after an edited clip, a new clip or a snippet was saved.
  ///
  /// In en, this message translates to:
  /// **'Saved'**
  String get historySavedTitle;

  /// Toast message after saving an edited clip, which creates a new clip.
  ///
  /// In en, this message translates to:
  /// **'The edited text is a new clip at the top of History.'**
  String get historyEditSaved;

  /// Toast message after clips were added to the paste queue; several clips keep the order in which they were selected.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip joined the queue.} other{{count} clips joined the queue.}}'**
  String historyQueueAddedInOrder(int count);

  /// Error toast when clips could not be added to the paste queue. Paste Queue is the feature name.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t add to Paste Queue. Check that it is turned on.'**
  String get historyQueueAddFailed;

  /// Error toast when an expiring clip could not be kept.
  ///
  /// In en, this message translates to:
  /// **'This clip could not be kept.'**
  String get historyKeepFailed;

  /// Error shown when the content-rule explanation of a clip could not be loaded.
  ///
  /// In en, this message translates to:
  /// **'The explanation could not be loaded.'**
  String get historyExplainFailed;

  /// Countdown of an expiring clip with less than one minute left.
  ///
  /// In en, this message translates to:
  /// **'Deletes in less than a minute'**
  String get historyDeletesSoon;

  /// Countdown of an expiring clip, in minutes.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Deletes in 1 min} other{Deletes in {count} min}}'**
  String historyDeletesInMinutes(int count);

  /// Countdown of an expiring clip, in hours.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Deletes in 1 h} other{Deletes in {count} h}}'**
  String historyDeletesInHours(int count);

  /// Countdown of an expiring clip, in days.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Deletes in 1 day} other{Deletes in {count} days}}'**
  String historyDeletesInDays(int count);

  /// Marker next to the source app of a clip the user typed in CopyPaste.
  ///
  /// In en, this message translates to:
  /// **'Manual'**
  String get historyProvenanceManual;

  /// Marker next to the source app of a clip saved from the clip editor.
  ///
  /// In en, this message translates to:
  /// **'Edited'**
  String get historyProvenanceEdited;

  /// Marker next to the source app of a clip created by a text transform.
  ///
  /// In en, this message translates to:
  /// **'Transformed'**
  String get historyProvenanceTransformed;

  /// Marker next to the source app for a clip written by an installed module.
  ///
  /// In en, this message translates to:
  /// **'Module'**
  String get historyProvenanceModule;

  /// A tag that a content rule assigned automatically.
  ///
  /// In en, this message translates to:
  /// **'{name} (auto)'**
  String historyRuleTag(String name);

  /// Clip details label for when an expiring clip is deleted.
  ///
  /// In en, this message translates to:
  /// **'Deletion'**
  String get historyMetadataExpiry;

  /// Clip details label for tags assigned by content rules.
  ///
  /// In en, this message translates to:
  /// **'Automatic tags'**
  String get historyMetadataRuleTags;

  /// Title of the dialog confirming what a module may access before it is enabled; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'Enable {module}?'**
  String modulesEnableTitle(String module);

  /// Lead-in above the list of what a module may access.
  ///
  /// In en, this message translates to:
  /// **'It will be able to:'**
  String get modulesEnableAccess;

  /// One line naming a host service a module asks for; {capability} is its identifier.
  ///
  /// In en, this message translates to:
  /// **'{capability, select, history_read{Read your clipboard history} clipboard_write{Copy to your clipboard} library_write{Save snippets and pin clips} snippets{Read your snippets} synced_records{Sync its data across your devices} local_state{Keep its data on this device} prompts{Ask you to confirm actions} endpoints{Accept connections from local apps} clipboard_read{Read the text on your clipboard} other{Use a CopyPaste service}}'**
  String modulesCapability(String capability);

  /// Line naming the module that asks a question; {module} is the module name.
  ///
  /// In en, this message translates to:
  /// **'From {module}'**
  String modulesPromptFrom(String module);

  /// Shown instead of a module name that is no longer known.
  ///
  /// In en, this message translates to:
  /// **'Unknown module'**
  String get modulesUnknownModule;

  /// Error when an answer to a module question could not be sent.
  ///
  /// In en, this message translates to:
  /// **'Your answer was not sent.'**
  String get modulesPromptAnswerFailed;

  /// System notification text for a module question that arrives while the app is hidden.
  ///
  /// In en, this message translates to:
  /// **'Open CopyPaste to answer.'**
  String get modulesPromptNotificationBody;

  /// Shown instead of a device name that is no longer known.
  ///
  /// In en, this message translates to:
  /// **'Unknown device'**
  String get modulesUnknownDevice;

  /// Placeholder of a device picker in module settings.
  ///
  /// In en, this message translates to:
  /// **'Choose devices'**
  String get modulesChooseDevices;

  /// Button that adds an application to a module setting.
  ///
  /// In en, this message translates to:
  /// **'Add app'**
  String get modulesAddApp;

  /// Tooltip of the button that removes an application from a module setting; {app} is the application name.
  ///
  /// In en, this message translates to:
  /// **'Remove {app}'**
  String modulesRemoveApp(String app);

  /// Shown when a module application list is empty.
  ///
  /// In en, this message translates to:
  /// **'No apps chosen'**
  String get modulesNoApps;

  /// Settings section that lists runtime logs.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get settingsSectionDiagnostics;

  /// Summary of the Diagnostics settings section.
  ///
  /// In en, this message translates to:
  /// **'Runtime logs for troubleshooting. Clipboard content is never logged.'**
  String get settingsSectionDiagnosticsSummary;

  /// Placeholder of the Diagnostics log search field.
  ///
  /// In en, this message translates to:
  /// **'Search logs'**
  String get diagnosticsSearchPlaceholder;

  /// Diagnostics level filter choice that lists every level.
  ///
  /// In en, this message translates to:
  /// **'All levels'**
  String get diagnosticsAllLevels;

  /// Diagnostics process filter choice that lists the app and background service.
  ///
  /// In en, this message translates to:
  /// **'All processes'**
  String get diagnosticsAllProcesses;

  /// Log level label.
  ///
  /// In en, this message translates to:
  /// **'Error'**
  String get diagnosticsLevelError;

  /// Log level label.
  ///
  /// In en, this message translates to:
  /// **'Warning'**
  String get diagnosticsLevelWarning;

  /// Log level label.
  ///
  /// In en, this message translates to:
  /// **'Info'**
  String get diagnosticsLevelInfo;

  /// Log level label.
  ///
  /// In en, this message translates to:
  /// **'Debug'**
  String get diagnosticsLevelDebug;

  /// Log level label.
  ///
  /// In en, this message translates to:
  /// **'Trace'**
  String get diagnosticsLevelTrace;

  /// Log source: the CopyPaste application process.
  ///
  /// In en, this message translates to:
  /// **'App'**
  String get diagnosticsProcessApp;

  /// Log source: the desktop background service that captures and syncs the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Background service'**
  String get diagnosticsProcessDaemon;

  /// Tooltip of the button that reloads the Diagnostics log.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get diagnosticsRefresh;

  /// Button that saves the Diagnostics log to a file.
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get diagnosticsExport;

  /// File type label in the Diagnostics export save dialog.
  ///
  /// In en, this message translates to:
  /// **'Text file'**
  String get diagnosticsExportFileType;

  /// Toast title after the Diagnostics log is exported.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics exported'**
  String get diagnosticsExported;

  /// Toast title when the Diagnostics export fails.
  ///
  /// In en, this message translates to:
  /// **'Export failed'**
  String get diagnosticsExportFailed;

  /// Toast message when the Diagnostics export fails.
  ///
  /// In en, this message translates to:
  /// **'The log file could not be saved.'**
  String get diagnosticsExportFailedMessage;

  /// Error title when the Diagnostics log cannot be read.
  ///
  /// In en, this message translates to:
  /// **'Logs could not be loaded'**
  String get diagnosticsLoadFailed;

  /// Shown when no log entry matches the Diagnostics filters.
  ///
  /// In en, this message translates to:
  /// **'No log entries'**
  String get diagnosticsEmpty;

  /// Title of the Diagnostics setting that chooses how much the runtime log records.
  ///
  /// In en, this message translates to:
  /// **'Log level'**
  String get diagnosticsLogLevelTitle;

  /// One-line description of the Diagnostics log level setting.
  ///
  /// In en, this message translates to:
  /// **'How much detail is recorded on this device.'**
  String get diagnosticsLogLevelDescription;

  /// Heading above the choice of how Android background capture detects copies.
  ///
  /// In en, this message translates to:
  /// **'Detection method'**
  String get onboardingAndroidCaptureMethod;

  /// Background capture method that reacts to the Android system log.
  ///
  /// In en, this message translates to:
  /// **'System log (recommended)'**
  String get onboardingAndroidLogTrigger;

  /// Description of the system log capture method.
  ///
  /// In en, this message translates to:
  /// **'Instant, no extra battery use. Needs Shizuku or ADB access once.'**
  String get onboardingAndroidLogTriggerDescription;

  /// Background capture method that checks the clipboard on a timer.
  ///
  /// In en, this message translates to:
  /// **'Polling'**
  String get onboardingAndroidPolling;

  /// Description of the timed polling capture method and its cost.
  ///
  /// In en, this message translates to:
  /// **'Checks every second while the screen is on. Needs only overlay access; uses more battery.'**
  String get onboardingAndroidPollingDescription;

  /// Android 'Display over other apps' permission row title.
  ///
  /// In en, this message translates to:
  /// **'Display over other apps'**
  String get onboardingAndroidOverlay;

  /// Explains why the overlay permission is needed for polling.
  ///
  /// In en, this message translates to:
  /// **'Lets an invisible 1-pixel window read the clipboard.'**
  String get onboardingAndroidOverlayDescription;

  /// Error when Android overlay permission settings cannot be opened.
  ///
  /// In en, this message translates to:
  /// **'Overlay settings could not be opened.'**
  String get onboardingAndroidOverlaySettingsFailed;

  /// Setting that keeps sync running while the Android app is closed.
  ///
  /// In en, this message translates to:
  /// **'Background sync'**
  String get settingsBackgroundSyncTitle;

  /// Description of the background sync setting.
  ///
  /// In en, this message translates to:
  /// **'Keep syncing while the app is closed. Shows a notification.'**
  String get settingsBackgroundSyncDescription;

  /// Space-separated search keywords that find the background sync setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'keep alive persistent service notification battery'**
  String get settingsBackgroundSyncKeywords;

  /// Shown when background sync is enabled but its service is not running.
  ///
  /// In en, this message translates to:
  /// **'Background sync is not running'**
  String get settingsBackgroundSyncStoppedTitle;

  /// Explains how to restart background sync.
  ///
  /// In en, this message translates to:
  /// **'Allow notifications so Android can keep sync running.'**
  String get settingsBackgroundSyncStoppedMessage;

  /// Button that restarts background sync.
  ///
  /// In en, this message translates to:
  /// **'Resume sync'**
  String get settingsBackgroundSyncResume;

  /// Home-screen shortcut that saves and syncs the current clipboard.
  ///
  /// In en, this message translates to:
  /// **'Send clipboard shortcut'**
  String get quickAccessSendShortcutTitle;

  /// Description of the Send clipboard home-screen shortcut.
  ///
  /// In en, this message translates to:
  /// **'Saves the current clipboard and sends it to your devices.'**
  String get quickAccessSendShortcutDescription;

  /// Home-screen shortcut that opens transforms for the current clipboard.
  ///
  /// In en, this message translates to:
  /// **'Transform clipboard shortcut'**
  String get quickAccessTransformShortcutTitle;

  /// Description of the Transform clipboard home-screen shortcut.
  ///
  /// In en, this message translates to:
  /// **'Opens transforms for the current clipboard.'**
  String get quickAccessTransformShortcutDescription;

  /// Button that asks the launcher to pin a shortcut.
  ///
  /// In en, this message translates to:
  /// **'Add to home screen'**
  String get quickAccessAddToHomeScreen;

  /// Space-separated search keywords that find the home-screen shortcuts. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'shortcut home screen launcher icon'**
  String get quickAccessShortcutKeywords;

  /// Number of files in one multi-file clip or transfer.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 file} other{{count} files}}'**
  String fileSetCount(int count);

  /// Button that reveals the folder a multi-file clip was saved into.
  ///
  /// In en, this message translates to:
  /// **'Show folder'**
  String get fileSetShowFolder;

  /// Verb. Button on an interrupted download that continues it from the bytes already received.
  ///
  /// In en, this message translates to:
  /// **'Resume'**
  String get transfersResume;

  /// General setting title: a login launch keeps the window closed.
  ///
  /// In en, this message translates to:
  /// **'Start in background'**
  String get settingsStartInBackgroundTitle;

  /// One-line description of Start in background. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'When CopyPaste starts at login, keep the window closed.'**
  String get settingsStartInBackgroundDescription;

  /// General setting title: desktop windows show the system's translucent, blurred material.
  ///
  /// In en, this message translates to:
  /// **'Glass effect'**
  String get settingsWindowGlassTitle;

  /// One-line description of Glass effect. The system setting is macOS Reduce transparency or Windows Transparency effects.
  ///
  /// In en, this message translates to:
  /// **'Blur the desktop behind the window. Turns off when the system reduces transparency.'**
  String get settingsWindowGlassDescription;

  /// Clipboard setting title.
  ///
  /// In en, this message translates to:
  /// **'Restore clipboard on startup'**
  String get settingsRestoreClipboardTitle;

  /// One-line description of Restore clipboard on startup.
  ///
  /// In en, this message translates to:
  /// **'Put the latest clip back on an empty clipboard at start. Secret clips are skipped.'**
  String get settingsRestoreClipboardDescription;

  /// Storage row title.
  ///
  /// In en, this message translates to:
  /// **'Storage usage'**
  String get settingsStorageUsageTitle;

  /// One-line description of Storage usage. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Space CopyPaste uses on this device.'**
  String get settingsStorageUsageDescription;

  /// Error when the storage breakdown fails to load.
  ///
  /// In en, this message translates to:
  /// **'Storage usage could not be read.'**
  String get settingsStorageUsageUnavailable;

  /// Storage breakdown label: the clipboard history database.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get settingsStorageHistory;

  /// Storage breakdown label: files kept in history.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get settingsStorageFiles;

  /// Storage breakdown label: previews and temporary files.
  ///
  /// In en, this message translates to:
  /// **'Caches'**
  String get settingsStorageCaches;

  /// Storage breakdown label: backups made before data upgrades.
  ///
  /// In en, this message translates to:
  /// **'Upgrade backups'**
  String get settingsStorageUpgradeBackups;

  /// Storage breakdown label for everything else.
  ///
  /// In en, this message translates to:
  /// **'Logs, modules and settings'**
  String get settingsStorageOther;

  /// Storage breakdown total label.
  ///
  /// In en, this message translates to:
  /// **'Total'**
  String get settingsStorageTotal;

  /// Storage row title.
  ///
  /// In en, this message translates to:
  /// **'Clear cache'**
  String get settingsClearCacheTitle;

  /// One-line description of Clear cache.
  ///
  /// In en, this message translates to:
  /// **'Remove previews and temporary files. History stays.'**
  String get settingsClearCacheDescription;

  /// Button that clears the cache.
  ///
  /// In en, this message translates to:
  /// **'Clear'**
  String get settingsClearCacheAction;

  /// Toast title after clearing the cache.
  ///
  /// In en, this message translates to:
  /// **'Cache cleared'**
  String get settingsClearCacheDoneTitle;

  /// Toast message after clearing the cache.
  ///
  /// In en, this message translates to:
  /// **'Previews are rebuilt when they are needed.'**
  String get settingsClearCacheDoneMessage;

  /// Error when clearing the cache fails.
  ///
  /// In en, this message translates to:
  /// **'The cache could not be cleared.'**
  String get settingsClearCacheFailed;

  /// Storage row title.
  ///
  /// In en, this message translates to:
  /// **'Data folder'**
  String get settingsDataFolderTitle;

  /// One-line description of Data folder.
  ///
  /// In en, this message translates to:
  /// **'Show where history, backups and modules are kept.'**
  String get settingsDataFolderDescription;

  /// Storage row title.
  ///
  /// In en, this message translates to:
  /// **'Logs folder'**
  String get settingsLogsFolderTitle;

  /// Button that opens a folder in the file manager.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get settingsOpenFolderAction;

  /// Error when a folder cannot be shown.
  ///
  /// In en, this message translates to:
  /// **'The folder could not be opened.'**
  String get settingsOpenFolderFailed;

  /// Storage switch title.
  ///
  /// In en, this message translates to:
  /// **'Confirm before deleting'**
  String get settingsConfirmDeleteTitle;

  /// One-line description of Confirm before deleting.
  ///
  /// In en, this message translates to:
  /// **'Ask before deleting one clip. Deleting several always asks.'**
  String get settingsConfirmDeleteDescription;

  /// Storage row title.
  ///
  /// In en, this message translates to:
  /// **'Upgrade backups'**
  String get settingsUpgradeBackupsTitle;

  /// One-line description of Upgrade backups.
  ///
  /// In en, this message translates to:
  /// **'Encrypted copies made before data upgrades. The latest 5 are kept.'**
  String get settingsUpgradeBackupsDescription;

  /// Shown when no upgrade backup exists.
  ///
  /// In en, this message translates to:
  /// **'No upgrade backups yet.'**
  String get settingsUpgradeBackupsEmpty;

  /// One upgrade backup: when it was made and its size.
  ///
  /// In en, this message translates to:
  /// **'{date} · {size}'**
  String settingsUpgradeBackupEntry(String date, String size);

  /// Button that restores one upgrade backup.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get settingsUpgradeBackupRestoreAction;

  /// Confirmation title before restoring an upgrade backup.
  ///
  /// In en, this message translates to:
  /// **'Restore this backup?'**
  String get settingsUpgradeBackupRestoreTitle;

  /// Confirmation message before restoring an upgrade backup.
  ///
  /// In en, this message translates to:
  /// **'History is replaced with the copy made on {date}. Clips added since then are removed.'**
  String settingsUpgradeBackupRestoreMessage(String date);

  /// Updates switch title.
  ///
  /// In en, this message translates to:
  /// **'Download updates in background'**
  String get settingsDownloadUpdatesTitle;

  /// One-line description of Download updates in background.
  ///
  /// In en, this message translates to:
  /// **'Fetch and verify a found update so installing is instant. Waits on metered networks and in low-power mode.'**
  String get settingsDownloadUpdatesDescription;

  /// Update row description after a background download. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste {available} is downloaded and ready to install.'**
  String settingsUpdateDownloadedReady(String available);

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Your space'**
  String get devicesSpaceTitle;

  /// Count of devices in the space, this device included.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Only this device} other{{count} devices}}'**
  String devicesSpaceSummary(int count);

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Offline devices get the new key when they come online.'**
  String get devicesSpaceKeyPending;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Needs new key'**
  String get devicesSpaceWaitingForKey;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Not in a space'**
  String get devicesSpaceRemovedTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'This device was removed or left. Its history stays here.'**
  String get devicesSpaceRemovedMessage;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Join a space'**
  String get devicesSpaceJoin;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Remove from space'**
  String get devicesRemoveFromSpace;

  /// Title of the dialog that removes a device from the space.
  ///
  /// In en, this message translates to:
  /// **'Remove {device}?'**
  String devicesRemoveDialogTitle(String device);

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'It is removed on all your devices and can\'t read anything synced from now on.'**
  String get devicesRemoveDialogMessage;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'New sync passphrase (optional)'**
  String get devicesRemoveNewPassphrase;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'The removed device knows the current passphrase. Without a new one, add devices only by invitation.'**
  String get devicesRemoveNewPassphraseHint;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Sign other devices out of the cloud account'**
  String get devicesRemoveSignOutOthers;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Use at least 12 characters.'**
  String get devicesPassphraseTooShort;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Leave space'**
  String get devicesLeaveSpace;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Leave this space?'**
  String get devicesLeaveDialogTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Your other devices remove this one. Its history stays here.'**
  String get devicesLeaveDialogMessage;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Leave'**
  String get devicesLeave;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Your devices'**
  String get onboardingSpaceTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Sync your clipboard between your own devices. Everything stays encrypted.'**
  String get onboardingSpaceSubtitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Create a space'**
  String get onboardingSpaceCreateTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'This is my first device with CopyPaste.'**
  String get onboardingSpaceCreateDescription;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Join a space'**
  String get onboardingSpaceJoinTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'I already use CopyPaste on another device.'**
  String get onboardingSpaceJoinDescription;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Later'**
  String get onboardingSpaceLater;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Add your other devices now or later in Devices.'**
  String get onboardingReadySpaceCreated;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Next, confirm the code shown on your other device.'**
  String get onboardingReadyJoin;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Add device'**
  String get onboardingAddDeviceNow;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Join now'**
  String get onboardingJoinNow;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Welcome'**
  String get onboardingBadgeWelcome;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Setup'**
  String get onboardingBadgeSetup;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get onboardingBadgeDevices;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Ready'**
  String get onboardingBadgeReady;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Join your space'**
  String get onboardingPairJoinTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'On your other device, open CopyPaste and join with this code or QR code.'**
  String get onboardingPairAddDeviceSubtitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Later'**
  String get onboardingPairSkip;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Pairing didn\'t finish'**
  String get onboardingPairInterruptedTitle;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'Try again, or continue and pair later in Devices.'**
  String get onboardingPairInterruptedMessage;

  /// Ready-step message after pairing in the welcome tour. The paired device's name is shown in the card below it.
  ///
  /// In en, this message translates to:
  /// **'Your clipboard now syncs between your devices.'**
  String get onboardingReadyPaired;

  /// Device space UI text.
  ///
  /// In en, this message translates to:
  /// **'You can pair your devices anytime in Devices.'**
  String get onboardingReadyPairLater;

  /// Settings section with global keyboard shortcuts.
  ///
  /// In en, this message translates to:
  /// **'Shortcuts'**
  String get settingsSectionShortcuts;

  /// Summary of the Shortcuts settings section.
  ///
  /// In en, this message translates to:
  /// **'Global keyboard shortcuts. Switch off any you do not use.'**
  String get settingsSectionShortcutsSummary;

  /// Accessibility label of the switch that turns one global shortcut on or off.
  ///
  /// In en, this message translates to:
  /// **'Switch off this shortcut'**
  String get settingsShortcutEnabledLabel;

  /// Settings row that replays the first-run welcome tour.
  ///
  /// In en, this message translates to:
  /// **'Welcome tour'**
  String get settingsWelcomeTourTitle;

  /// Description of the welcome tour settings row.
  ///
  /// In en, this message translates to:
  /// **'Walk through the first-run setup again.'**
  String get settingsWelcomeTourDescription;

  /// Button that starts the welcome tour again.
  ///
  /// In en, this message translates to:
  /// **'Show again'**
  String get settingsWelcomeTourAction;

  /// Module details drawer:
  ///
  /// In en, this message translates to:
  /// **'Details'**
  String get modulesDetails;

  /// Module details drawer: Features
  ///
  /// In en, this message translates to:
  /// **'What it adds'**
  String get modulesDetailsFeatures;

  /// Module details drawer: Search
  ///
  /// In en, this message translates to:
  /// **'Finds clips by meaning with a local model.'**
  String get modulesDetailsSearch;

  /// Module details drawer: Permissions
  ///
  /// In en, this message translates to:
  /// **'Permissions'**
  String get modulesDetailsPermissions;

  /// Module details drawer: PermissionsPending
  ///
  /// In en, this message translates to:
  /// **'Listed after install. You confirm them when you enable the module.'**
  String get modulesDetailsPermissionsPending;

  /// Module details drawer: NoPermissions
  ///
  /// In en, this message translates to:
  /// **'Needs no permissions.'**
  String get modulesDetailsNoPermissions;

  /// Module details drawer: Information
  ///
  /// In en, this message translates to:
  /// **'Information'**
  String get modulesDetailsInformation;

  /// Module details drawer: Version
  ///
  /// In en, this message translates to:
  /// **'Version'**
  String get modulesDetailsVersion;

  /// Module details drawer: Size
  ///
  /// In en, this message translates to:
  /// **'Size'**
  String get modulesDetailsSize;

  /// Module details drawer: Requirements
  ///
  /// In en, this message translates to:
  /// **'Requirements'**
  String get modulesDetailsRequirements;

  /// Module details drawer: Status
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get modulesDetailsStatus;

  /// Generic explanation for a failure with no more specific reason. Shown under a title that says what failed.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong. Try again.'**
  String get errorUnexpected;

  /// Failure reason: the CopyPaste background service is not running.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste isn\'t running in the background. Restart the app.'**
  String get errorRuntimeUnavailable;

  /// Failure reason: the CopyPaste background service could not start.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste couldn\'t start its background service. Restart the app.'**
  String get errorRuntimeStartFailed;

  /// Failure reason: the CopyPaste background service is still starting.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is still starting. Try again in a moment.'**
  String get errorRuntimeStarting;

  /// Failure reason: the CopyPaste background service did not answer in time.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste didn\'t respond in time. Try again.'**
  String get errorRuntimeTimeout;

  /// Failure reason: the app and its background service are different versions, usually right after an update.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste was updated. Restart the app to finish.'**
  String get errorRuntimeMismatch;

  /// Failure reason: another paired device could not be reached.
  ///
  /// In en, this message translates to:
  /// **'The other device couldn\'t be reached. Check that it\'s online.'**
  String get errorPeerUnreachable;

  /// Failure reason: another device runs a CopyPaste version that cannot talk to this one.
  ///
  /// In en, this message translates to:
  /// **'The other device runs an incompatible CopyPaste version. Update both devices.'**
  String get errorPeerVersion;

  /// Failure reason: too many attempts in a short time.
  ///
  /// In en, this message translates to:
  /// **'Too many attempts. Wait a moment and try again.'**
  String get errorRateLimited;

  /// Failure reason when syncing did not finish with some paired devices. {devices} is a comma-separated list of device names.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t sync with {devices}.'**
  String devicesSyncFailedWith(String devices);

  /// Title of the error state shown in place of the transfers list when it cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load transfers'**
  String get transfersUnavailableTitle;

  /// Title of the error state shown in place of the tag list in the tags dialog when tags cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t load tags'**
  String get historyTagsLoadFailedTitle;

  /// Title of the settings row that deletes all local CopyPaste data.
  ///
  /// In en, this message translates to:
  /// **'Delete all data'**
  String get settingsEraseAllDataTitle;

  /// Description of the settings row that deletes all local data.
  ///
  /// In en, this message translates to:
  /// **'Removes history, settings and the encryption key, then closes CopyPaste.'**
  String get settingsEraseAllDataDescription;

  /// Button that starts deleting all local data.
  ///
  /// In en, this message translates to:
  /// **'Delete all'**
  String get settingsEraseAllDataAction;

  /// Title of the dialog confirming deletion of all local data.
  ///
  /// In en, this message translates to:
  /// **'Delete all CopyPaste data?'**
  String get settingsEraseAllDataConfirmTitle;

  /// Message of the dialog confirming deletion of all local data.
  ///
  /// In en, this message translates to:
  /// **'History, files, settings and the encryption key on this device will be deleted and CopyPaste will close. Paired devices and cloud copies stay. This cannot be undone.'**
  String get settingsEraseAllDataConfirmMessage;

  /// Error shown when deleting all local data fails.
  ///
  /// In en, this message translates to:
  /// **'Some CopyPaste data could not be deleted.'**
  String get settingsEraseAllDataFailed;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>[
    'de',
    'en',
    'es',
    'fr',
    'it',
    'ja',
    'ko',
    'pl',
    'pt',
    'ru',
    'tr',
    'uk',
    'zh',
  ].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return AppLocalizationsDe();
    case 'en':
      return AppLocalizationsEn();
    case 'es':
      return AppLocalizationsEs();
    case 'fr':
      return AppLocalizationsFr();
    case 'it':
      return AppLocalizationsIt();
    case 'ja':
      return AppLocalizationsJa();
    case 'ko':
      return AppLocalizationsKo();
    case 'pl':
      return AppLocalizationsPl();
    case 'pt':
      return AppLocalizationsPt();
    case 'ru':
      return AppLocalizationsRu();
    case 'tr':
      return AppLocalizationsTr();
    case 'uk':
      return AppLocalizationsUk();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
