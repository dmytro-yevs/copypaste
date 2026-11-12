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

  /// Error message shown when the Settings screen cannot load its values.
  ///
  /// In en, this message translates to:
  /// **'Settings could not be loaded.'**
  String get settingsRuntimeLoadFailed;

  /// Settings error message shown when the user adds an application that is already in the list of applications excluded from clipboard capture.
  ///
  /// In en, this message translates to:
  /// **'That application is already excluded.'**
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
  /// **'Application identifiers must be non-empty and 256 bytes or fewer.'**
  String get settingsRuntimeExcludedAppIdInvalid;

  /// Settings error message shown when the exclusion list already holds the maximum number of applications.
  ///
  /// In en, this message translates to:
  /// **'You can exclude up to 256 applications.'**
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

  /// Update error message on Android shown when the system refuses to install the update.
  ///
  /// In en, this message translates to:
  /// **'Android blocked the update installation.'**
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

  /// Placeholder of the search field at the top of the Quick Paste popup.
  ///
  /// In en, this message translates to:
  /// **'Type to search…'**
  String get quickPasteSearchPlaceholder;

  /// Message shown in the Quick Paste popup while the clipboard history is loading.
  ///
  /// In en, this message translates to:
  /// **'Loading clipboard history.'**
  String get quickPasteLoadingHistory;

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
  /// **'Preferences…'**
  String get quickPastePreferences;

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

  /// Title of the downloads and transfers drawer, and tooltip of the header button that opens it.
  ///
  /// In en, this message translates to:
  /// **'Downloads and transfers'**
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

  /// Button in the transfers drawer footer that removes finished transfer records.
  ///
  /// In en, this message translates to:
  /// **'Clear completed history'**
  String get transfersClearCompletedHistory;

  /// Error title in the transfers drawer when transfer records cannot be shown. Noun.
  ///
  /// In en, this message translates to:
  /// **'Transfers'**
  String get transfersErrorTitle;

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

  /// Error in the transfers drawer when the transfer list cannot be loaded.
  ///
  /// In en, this message translates to:
  /// **'Transfers could not be loaded. Try again.'**
  String get transfersLoadFailed;

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

  /// Tooltip of the notifications header button: the number of unread notifications.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, other{Notifications · {count} unread}}'**
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

  /// System tray menu item that opens the main application window. Verb.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get platformTrayOpen;

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
  /// **'CopyPaste could not quit. Please try again.'**
  String get platformQuitFailed;

  /// Settings error when showing or hiding the app in the Dock, taskbar, or system tray fails.
  ///
  /// In en, this message translates to:
  /// **'Application visibility could not be changed. Please try again.'**
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
  /// **'Application updates are unavailable here.'**
  String get platformUpdatesUnavailableHere;

  /// Update error when the installed application version cannot be read.
  ///
  /// In en, this message translates to:
  /// **'The installed version is unavailable.'**
  String get platformUpdateVersionUnavailable;

  /// Update message when this copy of the application cannot update itself.
  ///
  /// In en, this message translates to:
  /// **'Application updates are unavailable on this installation.'**
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

  /// Short summary of the General settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Language and application visibility.'**
  String get settingsSectionGeneralSummary;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Clipboard'**
  String get settingsSectionClipboard;

  /// Short summary of the Clipboard settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Capture and history limits.'**
  String get settingsSectionClipboardSummary;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get settingsSectionPrivacy;

  /// Short summary of the Privacy settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Application exclusions and screen protection.'**
  String get settingsSectionPrivacySummary;

  /// Short summary of the Quick Paste settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Shortcut and automatic paste behavior.'**
  String get settingsSectionQuickPasteSummary;

  /// Settings section name in navigation and as the section heading. Noun, short for synchronization.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get settingsSectionSync;

  /// Short summary of the Sync settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Synchronization and nearby-device discovery.'**
  String get settingsSectionSyncSummary;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get settingsSectionNotifications;

  /// Short summary of the Notifications settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Capture notifications and sounds.'**
  String get settingsSectionNotificationsSummary;

  /// Settings section name in navigation and as the section heading. Noun.
  ///
  /// In en, this message translates to:
  /// **'Data'**
  String get settingsSectionData;

  /// Short summary of the Data settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Export, backup, and restore.'**
  String get settingsSectionDataSummary;

  /// Settings section name in navigation and as the section heading. Noun: optional add-on features.
  ///
  /// In en, this message translates to:
  /// **'Modules'**
  String get settingsSectionModules;

  /// Short summary of the Modules settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Install and manage optional modules.'**
  String get settingsSectionModulesSummary;

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

  /// Short summary of the Quick access settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Keyboard, tiles, and home-screen widget.'**
  String get settingsSectionQuickAccessSummary;

  /// Settings section name in navigation and as the section heading. Noun: saved reusable texts.
  ///
  /// In en, this message translates to:
  /// **'Snippets'**
  String get settingsSectionSnippets;

  /// Short summary of the Snippets settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Reusable text and templates.'**
  String get settingsSectionSnippetsSummary;

  /// Settings section name in navigation and as the section heading. Text conversions such as case changes or JSON formatting.
  ///
  /// In en, this message translates to:
  /// **'Text transforms'**
  String get settingsSectionTransforms;

  /// Short summary of the Text transforms settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Favorites, pipelines, and default paste transforms.'**
  String get settingsSectionTransformsSummary;

  /// Settings section name in navigation and as the section heading. External AI assistants that can read clipboard history.
  ///
  /// In en, this message translates to:
  /// **'AI tools'**
  String get settingsSectionAiTools;

  /// Short summary of the AI tools settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Connect AI assistants and control what they can see.'**
  String get settingsSectionAiToolsSummary;

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

  /// Settings section name in navigation and as the section heading, about the application.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get settingsSectionAbout;

  /// Short summary of the About settings section, used when searching settings.
  ///
  /// In en, this message translates to:
  /// **'Version and application updates.'**
  String get settingsSectionAboutSummary;

  /// Description shown under the General settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Language, automatic startup, and application visibility.'**
  String get settingsGeneralDescription;

  /// Description shown under the Modules settings section heading. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Optional features for CopyPaste.'**
  String get settingsModulesDescription;

  /// Description shown under the Clipboard settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Capture clipboard changes and manage history limits.'**
  String get settingsClipboardDescription;

  /// Description shown under the Data settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Export, back up, and restore clipboard history.'**
  String get settingsDataDescription;

  /// Description shown under the Sync settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Control synchronization with paired devices.'**
  String get settingsSyncDescription;

  /// Description shown under the Privacy settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Control which apps are captured and protect clipboard content.'**
  String get settingsPrivacyDescription;

  /// Description shown under the Notifications settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Choose notifications and sounds for captured clips.'**
  String get settingsNotificationsDescription;

  /// Description shown under the About settings section heading. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste version and application updates.'**
  String get settingsAboutDescription;

  /// Description shown under the Quick Paste settings section heading.
  ///
  /// In en, this message translates to:
  /// **'Open clipboard history from anywhere without switching windows.'**
  String get settingsQuickPasteDescription;

  /// Message shown while the settings screen is loading.
  ///
  /// In en, this message translates to:
  /// **'Loading settings.'**
  String get settingsLoadingMessage;

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
  /// **'Resume enabled background services after restarting the device.'**
  String get settingsStartupAndroidDescription;

  /// Desktop startup setting description. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Open CopyPaste automatically when you sign in.'**
  String get settingsStartupDesktopDescription;

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
  /// **'Show the application icon for switching windows.'**
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

  /// Error alert title when the Dock, taskbar, menu bar, or tray visibility could not be changed.
  ///
  /// In en, this message translates to:
  /// **'Visibility could not be changed'**
  String get settingsVisibilityErrorTitle;

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
  /// **'Choose Full or Limited mode and manage the one-time Shizuku or ADB setup.'**
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
  /// **'Allow access to all photos and notifications to save new screenshots in the background.'**
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
  /// **'Automatically remove old unpinned clipboard items.'**
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
  /// **'Limit for adding and downloading files. File contents transfer only when you choose Download.'**
  String get settingsMaxFileSizeDescription;

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
  /// **'Save text clips in a portable file.'**
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
  /// **'Create backup'**
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
  /// **'The selected encrypted backup will replace the clipboard history on this device. Current settings and device identity are kept.'**
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
  /// **'Master switch for paired-device synchronization.'**
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
  /// **'Automatically copy newer clips from other devices to this clipboard.'**
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

  /// Settings row title for apps whose clipboard changes are not captured.
  ///
  /// In en, this message translates to:
  /// **'Excluded applications'**
  String get settingsExcludedAppsTitle;

  /// Search result description for excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Skip automatic capture from excluded applications.'**
  String get settingsExcludedAppsSearchDescription;

  /// Space-separated search keywords that find the excluded applications setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'privacy app identifiers'**
  String get settingsExcludedAppsKeywords;

  /// macOS description of excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Skip automatic capture during activity from these apps. Background copies may bypass exclusions.'**
  String get settingsExcludedAppsMacDescription;

  /// Windows and Linux description of excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Skip automatic capture from identified clipboard owners. Capture is paused when exclusions are set and the source cannot be identified.'**
  String get settingsExcludedAppsDesktopDescription;

  /// Android description of excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Android skips automatic capture while exclusions are set because it cannot identify source apps.'**
  String get settingsExcludedAppsAndroidDescription;

  /// Description of excluded applications on platforms without support.
  ///
  /// In en, this message translates to:
  /// **'Application exclusions are supported on macOS, Windows, Android, and Linux.'**
  String get settingsExcludedAppsUnsupportedDescription;

  /// Verb. Button that adds an application to the excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get settingsExcludedAppsAdd;

  /// Shown when the excluded applications list is empty.
  ///
  /// In en, this message translates to:
  /// **'No applications are excluded.'**
  String get settingsExcludedAppsEmpty;

  /// Tooltip of the button that removes an application from the excluded applications.
  ///
  /// In en, this message translates to:
  /// **'Remove {app}'**
  String settingsExcludedAppsRemove(String app);

  /// Title of the dialog that adds an excluded application.
  ///
  /// In en, this message translates to:
  /// **'Exclude application'**
  String get settingsExcludeDialogTitle;

  /// Verb. Confirm button of the dialog that adds an excluded application.
  ///
  /// In en, this message translates to:
  /// **'Exclude'**
  String get settingsExcludeDialogAction;

  /// Placeholder of the field for the application identifier, such as a bundle ID or executable name.
  ///
  /// In en, this message translates to:
  /// **'Application identifier'**
  String get settingsExcludeDialogPlaceholder;

  /// Error shown in the exclude application dialog when adding fails.
  ///
  /// In en, this message translates to:
  /// **'The application could not be excluded.'**
  String get settingsExcludeFailed;

  /// Settings row title for not capturing clipboard content that the source app marked as confidential.
  ///
  /// In en, this message translates to:
  /// **'Skip confidential clipboard'**
  String get settingsSkipSecretTitle;

  /// Description of the confidential clipboard setting. History is the clipboard history screen.
  ///
  /// In en, this message translates to:
  /// **'Keep producer-marked secrets out of History.'**
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
  /// **'Keep producer-marked temporary copies out of History.'**
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

  /// Description of the version row. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Installed CopyPaste version.'**
  String get settingsVersionDescription;

  /// Shown instead of the version number when it is unknown.
  ///
  /// In en, this message translates to:
  /// **'Unavailable'**
  String get settingsVersionUnavailable;

  /// Settings row title for checking and installing app updates.
  ///
  /// In en, this message translates to:
  /// **'Application updates'**
  String get settingsUpdatesTitle;

  /// Search result description for application updates. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Check for and install CopyPaste updates.'**
  String get settingsUpdatesSearchDescription;

  /// Space-separated search keywords that find the application updates setting. Translate each word and keep the English words too.
  ///
  /// In en, this message translates to:
  /// **'version github release'**
  String get settingsUpdatesKeywords;

  /// Application updates row description before any check. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Check for a newer CopyPaste version.'**
  String get settingsUpdateIdle;

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
  /// **'Application updates need attention.'**
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

  /// Toast title after an update check finds no update.
  ///
  /// In en, this message translates to:
  /// **'No updates available'**
  String get settingsUpdateNoneTitle;

  /// Toast message after an update check finds no update and the version is unknown. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is the latest version.'**
  String get settingsUpdateLatest;

  /// Toast message after an update check finds no update. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste {version} is the latest version.'**
  String settingsUpdateVersionLatest(String version);

  /// Toast title after an update check finds a newer version.
  ///
  /// In en, this message translates to:
  /// **'Update available'**
  String get settingsUpdateAvailableTitle;

  /// Toast message when a newer version exists and its number is unknown. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'A newer CopyPaste version is ready to install.'**
  String get settingsUpdateReady;

  /// Toast message when a newer version exists. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste {version} is ready to install.'**
  String settingsUpdateVersionReady(String version);

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

  /// Toast message when checking for updates failed. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not check for updates.'**
  String get settingsUpdateCheckFailedMessage;

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

  /// Message shown while Quick Paste settings load. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Loading Quick Paste settings.'**
  String get settingsQuickPasteLoading;

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
  /// **'The shortcut works while CopyPaste is running in the background.'**
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

  /// Loading message on the Devices screen while the device list is read.
  ///
  /// In en, this message translates to:
  /// **'Loading devices…'**
  String get devicesLoading;

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

  /// Title of the confirmation dialog for revoking a paired device; {device} is the device name.
  ///
  /// In en, this message translates to:
  /// **'Revoke {device}?'**
  String devicesRevokeDialogTitle(String device);

  /// Title of the confirmation dialog for unpairing a device; {device} is the device name.
  ///
  /// In en, this message translates to:
  /// **'Unpair {device}?'**
  String devicesUnpairDialogTitle(String device);

  /// Explanation in the confirmation dialog for revoking a paired device.
  ///
  /// In en, this message translates to:
  /// **'This removes the device from your group and permanently blocks this pairing. Other devices apply the removal when they reconnect.'**
  String get devicesRevokeDialogMessage;

  /// Explanation in the confirmation dialog for unpairing a device.
  ///
  /// In en, this message translates to:
  /// **'This removes the device from your group. Other devices apply the removal when they reconnect. Adding it again requires a new confirmation.'**
  String get devicesUnpairDialogMessage;

  /// Verb. Confirm button that removes a paired device and permanently blocks its pairing.
  ///
  /// In en, this message translates to:
  /// **'Revoke'**
  String get devicesRevoke;

  /// Verb. Button that removes a paired device from the group; it can be paired again later.
  ///
  /// In en, this message translates to:
  /// **'Unpair'**
  String get devicesUnpair;

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

  /// Destructive button in paired device details that removes the device and permanently blocks its pairing.
  ///
  /// In en, this message translates to:
  /// **'Revoke pairing'**
  String get devicesRevokePairing;

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

  /// Status row value in paired device details: the device is reachable now.
  ///
  /// In en, this message translates to:
  /// **'Available'**
  String get devicesPeerAvailable;

  /// Status row value in paired device details: seen on the network but not yet verified.
  ///
  /// In en, this message translates to:
  /// **'Visible now'**
  String get devicesPeerVisibleNow;

  /// Status row value in paired device details: the status cannot be determined.
  ///
  /// In en, this message translates to:
  /// **'Status unknown'**
  String get devicesPeerStatusUnknown;

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

  /// Empty state title in the pairing panel before a pairing method is chosen.
  ///
  /// In en, this message translates to:
  /// **'Choose a pairing action'**
  String get pairingEmptyTitle;

  /// Empty state message in the pairing panel before a pairing method is chosen.
  ///
  /// In en, this message translates to:
  /// **'Pairing controls appear here.'**
  String get pairingEmptyMessage;

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

  /// Help text under the pairing code field.
  ///
  /// In en, this message translates to:
  /// **'Enter the 8-character code shown on the other device.'**
  String get pairingCodeHint;

  /// Label of the field for the network address of the device to pair with.
  ///
  /// In en, this message translates to:
  /// **'Device address'**
  String get pairingDeviceAddress;

  /// Help text under the device address field. Keep 'host:port' as is.
  ///
  /// In en, this message translates to:
  /// **'Use the host:port address reported by the other device.'**
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

  /// Title of the placeholder shown before the camera preview starts.
  ///
  /// In en, this message translates to:
  /// **'QR scanner'**
  String get pairingQrScannerTitle;

  /// Message in the placeholder shown before the camera preview starts.
  ///
  /// In en, this message translates to:
  /// **'The camera preview appears here.'**
  String get pairingCameraPreviewPlaceholder;

  /// Alert title when the QR code scanner reports a problem.
  ///
  /// In en, this message translates to:
  /// **'Scanner needs attention'**
  String get pairingScannerNeedsAttention;

  /// Validation error when the pairing code has fewer than 8 characters.
  ///
  /// In en, this message translates to:
  /// **'Enter the complete 8-character pairing code.'**
  String get pairingCodeIncomplete;

  /// Validation error when the device address is empty. Keep 'host:port' as is.
  ///
  /// In en, this message translates to:
  /// **'Enter the device host:port address.'**
  String get pairingAddressMissing;

  /// Pairing status after this device confirmed and the other device has not confirmed yet.
  ///
  /// In en, this message translates to:
  /// **'Waiting for the other device'**
  String get pairingWaitingForOtherDevice;

  /// Instructions next to a pairing code. 'Add device' and 'Enter code' are button names; translate them the same way as those buttons.
  ///
  /// In en, this message translates to:
  /// **'On the other device, choose Add device → Enter code, or scan the QR code.'**
  String get pairingInviteInstructions;

  /// Heading of the table with the pairing code and address.
  ///
  /// In en, this message translates to:
  /// **'Pairing details'**
  String get pairingDetailsTitle;

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

  /// Fallback error message when pairing cannot start.
  ///
  /// In en, this message translates to:
  /// **'The pairing action could not be started.'**
  String get pairingActionNotStarted;

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

  /// Pairing status description asking the user to compare codes and confirm.
  ///
  /// In en, this message translates to:
  /// **'Compare the protected verification code, then confirm on both devices.'**
  String get pairingAwaitingConfirmationDescription;

  /// Pairing status description after pairing succeeded.
  ///
  /// In en, this message translates to:
  /// **'The device is connected. Your group membership will sync automatically.'**
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
  /// **'Synchronization interrupted'**
  String get syncStatusInterrupted;

  /// Synchronization status when some items could not be synchronized.
  ///
  /// In en, this message translates to:
  /// **'Partially synchronized'**
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
  /// **'Synchronization problem detected.'**
  String get syncLogProblemDetected;

  /// Synchronization activity log entry: a synchronization problem was resolved.
  ///
  /// In en, this message translates to:
  /// **'Synchronization problem resolved.'**
  String get syncLogProblemResolved;

  /// Synchronization activity log entry: synchronization failed.
  ///
  /// In en, this message translates to:
  /// **'Synchronization failed.'**
  String get syncLogFailed;

  /// Synchronization activity log entry: synchronization works again after a failure.
  ///
  /// In en, this message translates to:
  /// **'Synchronization restored.'**
  String get syncLogRecovered;

  /// Synchronization activity log entry with how many clipboard items (clips) were sent to and received from a device.
  ///
  /// In en, this message translates to:
  /// **'Sent {sent, plural, =1{1 clip} other{{sent} clips}} · Received {received, plural, =1{1 clip} other{{received} clips}}'**
  String syncLogClipsTransferred(int sent, int received);

  /// Screen reader label of the header button that opens synchronization details; {status} is the current synchronization status.
  ///
  /// In en, this message translates to:
  /// **'Synchronization: {status}'**
  String syncHeaderSemanticsLabel(String status);

  /// Title of the synchronization details panel.
  ///
  /// In en, this message translates to:
  /// **'Synchronization'**
  String get syncDetailsTitle;

  /// Tooltip and screen reader label of the button that closes the synchronization details panel.
  ///
  /// In en, this message translates to:
  /// **'Close synchronization details'**
  String get syncDetailsClose;

  /// Empty state message in synchronization details when no devices are paired.
  ///
  /// In en, this message translates to:
  /// **'Pair a device to synchronize clipboard history.'**
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
  /// **'No recent synchronization problems'**
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
  /// **'Items over the size limit'**
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

  /// Error message shown when the History list fails to load.
  ///
  /// In en, this message translates to:
  /// **'History could not be loaded. Try again.'**
  String get historyErrorLoad;

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
  /// **'The clip secrecy could not be updated.'**
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
  /// **'OCR could not be started. Try again.'**
  String get historyOcrErrorStart;

  /// Error shown when text recognition (OCR) on an image clip fails.
  ///
  /// In en, this message translates to:
  /// **'Image text could not be recognized. Try again.'**
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

  /// Toast message after the recognized image text is copied to the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Recognized text copied.'**
  String get historyOcrTextCopied;

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

  /// Loading message while the History list loads.
  ///
  /// In en, this message translates to:
  /// **'Loading clipboard history.'**
  String get historyLoadingMessage;

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
  /// **'{count, plural, =1{1 clipboard row was skipped} other{{count} clipboard rows were skipped}}'**
  String historySkippedRowsTitle(int count);

  /// Warning message in History when some stored clips could not be decrypted and are hidden.
  ///
  /// In en, this message translates to:
  /// **'Some retained clips could not be read and are not shown in this list.'**
  String get historySkippedRowsMessage;

  /// Placeholder of the History search field and tooltip of the button that opens it.
  ///
  /// In en, this message translates to:
  /// **'Search history'**
  String get historySearch;

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

  /// Tooltip of the clip details button that opens the tags dialog for the clip.
  ///
  /// In en, this message translates to:
  /// **'Manage clip tags'**
  String get historyManageClipTags;

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

  /// Toast message after a clip is copied as plain text.
  ///
  /// In en, this message translates to:
  /// **'Plain text copied.'**
  String get historyPlainTextCopied;

  /// Toast message after a clip is copied to the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Clip copied.'**
  String get historyClipCopied;

  /// Title of the success toast after a file clip is saved to this device.
  ///
  /// In en, this message translates to:
  /// **'Downloaded'**
  String get historyDownloadedTitle;

  /// Toast message after a file clip is saved to this device.
  ///
  /// In en, this message translates to:
  /// **'File saved.'**
  String get historyFileSaved;

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

  /// Error title in clip details when a file clip download failed.
  ///
  /// In en, this message translates to:
  /// **'Download failed'**
  String get historyDownloadFailedTitle;

  /// Button in clip details that opens a downloaded file.
  ///
  /// In en, this message translates to:
  /// **'Show file'**
  String get historyShowFile;

  /// Clip details row label shown instead of all metadata while a confidential clip is hidden.
  ///
  /// In en, this message translates to:
  /// **'Privacy'**
  String get historyMetadataPrivacy;

  /// Clip details value of the Privacy row while a confidential clip is hidden. Adjective.
  ///
  /// In en, this message translates to:
  /// **'Confidential'**
  String get historyMetadataConfidential;

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

  /// Clip details row label for the clip type, such as Text or Image.
  ///
  /// In en, this message translates to:
  /// **'Clip type'**
  String get historyMetadataClipType;

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
  /// **'Too large to sync'**
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

  /// Error message shown in History, Devices, and Settings when the app's background runtime fails to start. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste could not start its runtime.'**
  String get appRuntimeStartFailed;

  /// Loading message shown while the app window prepares its first screen. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Preparing CopyPaste.'**
  String get appPreparing;

  /// Message shown in History, Devices, and Settings while the app waits for the operating system to unlock its encrypted history. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste is waiting for protected history access. The available parts of the app remain usable.'**
  String get appRuntimeWaiting;

  /// Error title shown in place of the History screen when the clipboard history service is not running.
  ///
  /// In en, this message translates to:
  /// **'History runtime is unavailable'**
  String get appHistoryUnavailableTitle;

  /// Error message shown in place of the History screen when the clipboard history service is not running.
  ///
  /// In en, this message translates to:
  /// **'Start the application runtime before opening history.'**
  String get appHistoryUnavailableMessage;

  /// Error title shown in place of the Devices screen when the sync service is not running.
  ///
  /// In en, this message translates to:
  /// **'Devices runtime is unavailable'**
  String get appDevicesUnavailableTitle;

  /// Error message shown in place of the Devices screen when the sync service is not running.
  ///
  /// In en, this message translates to:
  /// **'Start the application runtime before opening devices.'**
  String get appDevicesUnavailableMessage;

  /// Error title shown in place of the Settings screen when the app service is not running.
  ///
  /// In en, this message translates to:
  /// **'Settings runtime is unavailable'**
  String get appSettingsUnavailableTitle;

  /// Error message shown in place of the Settings screen when the app service is not running.
  ///
  /// In en, this message translates to:
  /// **'Start the application runtime before opening settings.'**
  String get appSettingsUnavailableMessage;

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

  /// Screen reader label of the currently selected navigation item; {destination} is the screen name, such as History.
  ///
  /// In en, this message translates to:
  /// **'{destination}, selected'**
  String appDestinationSelected(String destination);

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

  /// Title and button of the Shizuku setup card asking to grant CopyPaste permission inside Shizuku. CopyPaste is the product name.
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
  /// **'Get Shizuku to apply one-time access.'**
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

  /// Button that asks Android for a permission, such as notifications.
  ///
  /// In en, this message translates to:
  /// **'Allow'**
  String get sharedAllow;

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

  /// Uppercase label above each setup step showing progress, such as STEP 2 OF 3 · MACOS. {platform} is the uppercased operating system name. Keep the uppercase style if the language has it.
  ///
  /// In en, this message translates to:
  /// **'STEP {step} OF {count} · {platform}'**
  String onboardingStepProgress(int step, int count, String platform);

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
  /// **'CopyPaste could not save onboarding progress.'**
  String get onboardingSaveProgressFailed;

  /// Title of the setup row that launches the app automatically when the user signs in to the computer.
  ///
  /// In en, this message translates to:
  /// **'Start at login'**
  String get onboardingStartAtLogin;

  /// Description of the Start at login row on macOS and Windows. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Open CopyPaste when you sign in'**
  String get onboardingStartAtLoginDescription;

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

  /// Status shown when the Linux desktop extension is active. Clipboard v2 is the name of the integration protocol.
  ///
  /// In en, this message translates to:
  /// **'The authenticated Clipboard v2 bridge is ready.'**
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

  /// Status shown when the keyboard control permission is granted. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Keyboard control is allowed for Quick Paste.'**
  String get onboardingLinuxKeyboardAllowed;

  /// Status shown when the Linux desktop session cannot grant keyboard control.
  ///
  /// In en, this message translates to:
  /// **'Keyboard control is unavailable in this desktop session.'**
  String get onboardingLinuxKeyboardUnsupported;

  /// Button that asks the Linux desktop for the keyboard control permission.
  ///
  /// In en, this message translates to:
  /// **'Allow keyboard control'**
  String get onboardingLinuxAllowKeyboard;

  /// Status of the Quick Paste row on Linux when the feature works. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste is ready.'**
  String get onboardingLinuxQuickPasteReady;

  /// Status of the Quick Paste row on Linux when the desktop integration is missing. Quick Paste is a feature name.
  ///
  /// In en, this message translates to:
  /// **'Quick Paste will be ready after the required desktop integration is active.'**
  String get onboardingLinuxQuickPastePending;

  /// Description of the Start at login row on Linux. CopyPaste is the product name.
  ///
  /// In en, this message translates to:
  /// **'Open CopyPaste when you sign in.'**
  String get onboardingLinuxStartAtLoginDescription;

  /// Title of the Linux setup row that registers the app to open copypaste:// links used for pairing invitations.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste links'**
  String get onboardingLinuxLinks;

  /// Status shown when copypaste:// links are registered with the Linux desktop.
  ///
  /// In en, this message translates to:
  /// **'CopyPaste links open in this app.'**
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

  /// Explanation that Linux cannot hide the app window from screenshots, while stored data stays encrypted.
  ///
  /// In en, this message translates to:
  /// **'Screenshot blocking is unavailable on Linux. Protected stored data remains encrypted.'**
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

  /// Title and button that grant Android background clipboard access through Shizuku.
  ///
  /// In en, this message translates to:
  /// **'Apply capture access'**
  String get onboardingAndroidApplyAccess;

  /// Description of the step that grants Android background clipboard access once.
  ///
  /// In en, this message translates to:
  /// **'Apply one-time capture access.'**
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
  /// **'Clipboard intake could not be enabled.'**
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
  /// **'CopyPaste is allowed in Shizuku, but Android capture grants could not be applied.'**
  String get onboardingAndroidGrantsFailed;

  /// Error shown when the app must first be allowed in the Shizuku app.
  ///
  /// In en, this message translates to:
  /// **'Allow CopyPaste in Shizuku to apply capture grants.'**
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

  /// Loading message shown while installed modules are read.
  ///
  /// In en, this message translates to:
  /// **'Loading modules.'**
  String get modulesLoading;

  /// Error title shown when the installed modules could not be read.
  ///
  /// In en, this message translates to:
  /// **'Modules are unavailable'**
  String get modulesUnavailableTitle;

  /// Error message shown when the installed modules could not be read.
  ///
  /// In en, this message translates to:
  /// **'Try loading modules again.'**
  String get modulesUnavailableMessage;

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

  /// Loading message shown while the module marketplace is downloaded.
  ///
  /// In en, this message translates to:
  /// **'Loading marketplace.'**
  String get modulesMarketplaceLoading;

  /// Error title shown when the module marketplace could not be loaded.
  ///
  /// In en, this message translates to:
  /// **'Marketplace is unavailable'**
  String get modulesMarketplaceUnavailableTitle;

  /// Fallback error message shown when the module marketplace could not be loaded.
  ///
  /// In en, this message translates to:
  /// **'Try loading the marketplace again.'**
  String get modulesMarketplaceUnavailableMessage;

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

  /// Empty-state message in a module's settings panel when the module has no options.
  ///
  /// In en, this message translates to:
  /// **'This module does not need extra configuration.'**
  String get modulesNoSettingsMessage;

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

  /// Loading message shown while the SMS permission state is read.
  ///
  /// In en, this message translates to:
  /// **'Checking SMS access.'**
  String get modulesSmsChecking;

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

  /// Title and button that grant the Android SMS permission through Shizuku.
  ///
  /// In en, this message translates to:
  /// **'Apply SMS access'**
  String get modulesSmsApplyAccess;

  /// Description of the step that grants the Android SMS permission once.
  ///
  /// In en, this message translates to:
  /// **'Apply one-time SMS access.'**
  String get modulesSmsApplyAccessDescription;

  /// Error title shown in the SMS access panel.
  ///
  /// In en, this message translates to:
  /// **'SMS setup failed'**
  String get modulesSmsSetupFailed;

  /// Error title shown in a module form when choosing a file failed.
  ///
  /// In en, this message translates to:
  /// **'File selection failed'**
  String get modulesFileSelectionFailed;

  /// Placeholder of the multi-select field for module languages.
  ///
  /// In en, this message translates to:
  /// **'Choose languages'**
  String get modulesChooseLanguages;

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

  /// Error shown when loading the module marketplace failed for an unknown reason.
  ///
  /// In en, this message translates to:
  /// **'The module marketplace could not be loaded.'**
  String get modulesMarketplaceLoadFailed;

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

  /// Error shown when a module form asks for a file but this device has no file picker.
  ///
  /// In en, this message translates to:
  /// **'File selection is unavailable.'**
  String get modulesFileSelectionUnavailable;

  /// Error shown when choosing a file for a module failed.
  ///
  /// In en, this message translates to:
  /// **'The file could not be selected.'**
  String get modulesFileSelectFailed;

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

  /// Reason shown in the operating system's identity prompt for using a snippet the user marked as concealed. The macos form is a lowercase clause that macOS inserts into its own sentence "<App> is trying to <reason>." in the system language; the other form is a standalone phrase.
  ///
  /// In en, this message translates to:
  /// **'{platform, select, macos{use a concealed snippet} other{Use a concealed snippet}}'**
  String deviceAuthReasonUseConcealedSnippet(String platform);

  /// Reason shown in the operating system's identity prompt for copying a secret item, for example from the tray Recent menu. The macos form is a lowercase clause that macOS inserts into its own sentence "<App> is trying to <reason>." in the system language; the other form is a standalone phrase.
  ///
  /// In en, this message translates to:
  /// **'{platform, select, macos{copy a secret item} other{Copy a secret item}}'**
  String deviceAuthReasonCopySecret(String platform);

  /// Status shown for an identity-check setting when the system has no device authentication (for example Linux).
  ///
  /// In en, this message translates to:
  /// **'Not available on this system'**
  String get deviceAuthUnavailable;

  /// Status shown for an identity-check setting when device authentication exists but no password, PIN or biometrics is set up.
  ///
  /// In en, this message translates to:
  /// **'Set up a device password, PIN or biometrics first'**
  String get deviceAuthNotConfigured;

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
  /// **'Pair another device to keep clipboard history in sync between them. Both devices must be on the same network, and every pairing is confirmed on both devices.'**
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

  /// Compact progress of a multi-step operation shown when the step names do not fit, such as '1 / 2 · Download'; {name} is the name of the current step.
  ///
  /// In en, this message translates to:
  /// **'{step} / {count} · {name}'**
  String sharedStepPosition(int step, int count, String name);

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
  /// **'New limit: {limit}. Older unpinned items are removed from this device now and cannot be recovered. Pinned items are kept.'**
  String settingsRetentionConfirmMessage(String limit);

  /// Title of the confirmation shown before a smaller storage quota deletes history.
  ///
  /// In en, this message translates to:
  /// **'Reduce the storage quota?'**
  String get settingsStorageQuotaConfirmTitle;

  /// Body of the confirmation shown before a smaller storage quota deletes history. size is the chosen quota, for example '5 GiB'.
  ///
  /// In en, this message translates to:
  /// **'New quota: {size}. If unpinned history uses more, the oldest items are removed now and cannot be recovered. Pinned items are kept.'**
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
  /// **'With both turned off, CopyPaste has no visible icon. To open it again, start CopyPaste again.'**
  String get settingsHideEverywhereMessage;

  /// Same as settingsHideEverywhereMessage, when the Quick Paste shortcut can open the application. shortcut is a key combination.
  ///
  /// In en, this message translates to:
  /// **'With both turned off, CopyPaste has no visible icon. To open it again, press {shortcut} or start CopyPaste again.'**
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
  /// **'{count, plural, =1{Exported 1 text clip} other{Exported {count} text clips}}'**
  String settingsExportedTextClips(int count);

  /// Short addition to the export confirmation: how many text clips could not be read and were left out of the file.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 could not be read} other{{count} could not be read}}'**
  String settingsExportUnreadableClips(int count);

  /// Joins the outcome of an action with a detail about it, for example 'Exported 214 text clips · 3 could not be read'.
  ///
  /// In en, this message translates to:
  /// **'{outcome} · {detail}'**
  String settingsOutcomeDetail(String outcome, String detail);

  /// Message of the confirmation shown after an encrypted backup was created. size is the file size, for example '12.4 MiB'.
  ///
  /// In en, this message translates to:
  /// **'Encrypted backup saved · {size}'**
  String settingsBackupDoneMessage(String size);

  /// Message of the confirmation shown after history was restored from an encrypted backup.
  ///
  /// In en, this message translates to:
  /// **'The clipboard history on this device now matches the backup.'**
  String get settingsRestoreDoneMessage;

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
  /// **'New quota: {size}. Unpinned history uses {used}, so its oldest clips are removed now until it fits. They cannot be recovered. Pinned items are kept.'**
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used);

  /// Number of clips in a History group, shown after the group name and date, such as: Today · Wednesday, November 11 · 14 clips.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 clip} other{{count} clips}}'**
  String historySectionClipCount(int count);
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
