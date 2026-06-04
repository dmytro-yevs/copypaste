# Product Critique — Casual Cross-Device User Persona

**Persona:** Maya, 29. Graphic designer. MacBook Pro (primary work machine) + Samsung Galaxy S25.
Uses her phone and Mac interchangeably throughout the day — copying reference images on her phone,
pasting text from emails into Figma, grabbing links while commuting. Not technical. Has never opened
a terminal. Her threshold for abandoning an app: if she can't figure it out in 10 minutes, it's gone.

**Date of evaluation:** 2026-06-04  
**Branch reviewed:** v0.6.1-integration

---

## 1. First-Run Experience — From Zero to "It Actually Works"

Let me try to trace what Maya actually has to do to get her Mac and phone sharing a clipboard.

**On the Mac:**  
She downloads the app. It opens to a history view that says "Nothing copied yet." There is no welcome
screen, no "here's what you need to do first" screen, no tutorial overlay. She is staring at an empty
list with a sidebar containing tabs she doesn't recognise: History, Devices, Settings, Logs, About.
She clicks around.

Eventually she finds the Devices tab. A QR code is there — blurred out. There's a label that says
"Click to reveal." She clicks it. Now there's a QR code. Good. She also notices a section called
"LAN Discovery" below it, showing nothing ("No devices found"). There's a "Rescan" button. She clicks
it. Still nothing. She doesn't know why. (She doesn't know that the Android app isn't installed yet.)

**On the phone:**  
She finds the Android app. First launch shows an onboarding screen asking her to grant several
permissions. This part is actually okay — it lists them in plain language. She grants them. Then she's
in the app. Three tabs at the bottom: Clips, Devices, Settings.

She taps Devices. She sees her own device card. Below it is a button: "Scan QR." She taps it. A camera
opens. She points it at the Mac screen. Something happens. A card appears: "Pair & sync." She taps it.

Now here is where she might panic: **a dialog appears on her phone showing a 6-digit number**. She has
no idea what this is for. The heading says something like "Confirm pairing" and there are buttons
labelled "Match" and "Doesn't match." She doesn't know what these words mean as actions. Match *what*?
Doesn't match *what*?

Meanwhile, back on the Mac, a notification pops up: "A device wants to pair." The main window comes
to the foreground and switches to Devices, showing the same 6-digit number. If she happens to be
looking at the Mac at this moment, great. If she walked over to the phone to scan the QR and left the
Mac unattended, she might miss it entirely. The pairing modal has a **60-second timeout**. She has to
look at both screens simultaneously and confirm on both.

**Step count to first successful pairing (best case):**

1. Download and open Mac app.
2. Navigate to Devices tab.
3. Click the blurred QR to reveal it.
4. Download and open Android app.
5. Grant 4–5 permissions in onboarding (notifications, overlay, battery optimisation, possibly autostart).
6. Tap Devices tab on Android.
7. Tap "Scan QR."
8. Point camera at Mac, wait for scan.
9. Tap "Pair & sync" on the confirmation card.
10. While watching phone AND Mac simultaneously: confirm the 6-digit number matches on both devices.
11. Tap "Match" on phone. (What does "Match" mean as a button? Not obvious.)
12. Tap "Match" on Mac. (Or confirm — the Mac side says the same.)

That is **12 steps**, and step 10 is confusing because the concept of a shared confirmation code is
not explained anywhere. There is no tooltip, no sentence saying "Both devices should show the same
number — this confirms they're talking to each other, not to an imposter." Without that explanation,
a normal person will either tap "Match" blindly (which works, but feels unsafe) or tap "Doesn't match"
because they think the two numbers should be *different* (it's not intuitive that matching = good).

**Where would Maya give up?**

- The QR is blurred by default. She might not know to click it. Low patience.
- The 6-digit SAS dialog with "Match"/"Doesn't match" and no explanation. High confusion.
- The 60-second timeout. If she is slow or distracted, pairing silently fails. Then she has to start
  over from step 3.
- If she is not on the same Wi-Fi (she's at a coffee shop, her Mac is on the coffee shop Wi-Fi and
  her phone is on mobile data), QR pairing still works — but she has no indication of this. She might
  assume it only works on Wi-Fi and not bother.
- After pairing succeeds, there is no "You're connected! Clipboard will now sync." confirmation screen.
  The pairing dialog closes and she's back at the Devices tab looking at a card for her phone. She
  doesn't know if it worked.

**What actually happens after pairing:**  
Cloud/relay sync requires a separate setup: she must go to Settings → Sync and enter a Supabase URL,
anon key, relay URL, and a shared passphrase. She has never heard of Supabase. This form asks for a
"Project URL" and an "Anon key." These are developer concepts. She will not complete this step.

Without cloud/relay configuration, sync only works when both devices are on the same Wi-Fi network
(P2P). She doesn't know this. She will copy something on her phone while at a coffee shop, expect it
to appear on her Mac at home, and it won't. She will think the app is broken.

**Verdict on first-run:** A technical user who reads documentation might get through this in 10–15
minutes. Maya gets through it in 25 minutes if she's patient and lucky, or abandons around step 10.

---

## 2. Does It "Just Work" Day-to-Day?

Assuming Maya got through pairing and is on the same Wi-Fi:

**Latency:**  
When both devices are on the same LAN, P2P sync is 1–2 seconds. This is excellent. She copies a
phone number on her phone, and 2 seconds later it is available on her Mac. This is the one thing
that works beautifully.

**Do I have to open the app?**  
On the Mac: no. The daemon runs in the background, clipboard is captured automatically, and she can
use Cmd+Shift+V to open the popup from any app. This is great.

On Android: sort of. A persistent notification appears in the notification shade ("Active — N items
captured today"). It's necessary — it means the background service is alive. But to a normal person,
a permanent notification that says "Active" feels like clutter. She will try to dismiss it and be
confused when it keeps coming back.

**Reliability:**  
If she is not on the same Wi-Fi, nothing syncs (P2P only). She doesn't know this and will think it
is broken. Cloud sync (which would fix this) requires the Supabase setup she skipped.

If she goes home and is on a different Wi-Fi from her phone (common: Mac on home broadband, phone
on mobile data), sync stops working again.

**Images:**  
Images from her phone do sync over P2P (same Wi-Fi). This is genuinely impressive. However, the
Android app blocks screenshots (FLAG_SECURE is on the whole app) so she can't easily show someone
else what the app looks like, which is minor but annoying.

**Files:**  
Files larger than 8 MB are silently not synced. No notification. She copies a photo attachment from
her phone (14 MB), expects it on her Mac, it's not there. She thinks it's a bug. She is not wrong
to think this — there is no warning on the Android side that the file is too large to sync.

**Auto-paste:**  
On the Mac, she must press Cmd+Shift+V to get the popup, find the item, and click it. Or she can open
the full history window. There is no "the most recent synced item is automatically on your Mac
clipboard" behaviour — she has to actively retrieve it. This is a friction that rivals like
Universal Clipboard (Apple) do not have: if she were using iPhone + Mac with Universal Clipboard,
copying on iPhone literally means pasting on Mac with Cmd+V — no extra step. Here, there is always
an extra retrieval step.

(Technical note: the Supabase sync path has an "auto-apply newest text" feature on Android, but
cloud sync requires Supabase setup, which she skipped.)

**Day-to-day verdict:** When on the same Wi-Fi, it mostly works and the latency is good. Off Wi-Fi,
it is invisible and silent — no error, no explanation, just doesn't sync. This is the biggest
reliability gap for a casual user.

---

## 3. What Confuses or Annoys Me

**3.1 The Supabase form in Settings.**  
She opens Settings → Sync to try to make sync work away from home. She sees fields: "Supabase URL,"
"Supabase anon key," "Relay URL," "Sync passphrase." She has no idea what any of these mean. There is
no "Sign up" button, no "Get started with cloud sync," no plain-English explanation. This is a dead
end for any non-developer. She closes Settings and accepts that sync only works at home.

**3.2 The "Advanced" tab in Settings is empty.**  
She taps "Advanced" hoping to find something useful. The tab says: "Advanced daemon and storage limits
will appear here in a future release." A placeholder tab in a shipped app looks unfinished. It makes
the app feel like a beta.

**3.3 The Devices view shows her own phone as "3a7f1b2c."**  
Once paired, the history view shows items from her phone with a badge that says "3a7f1b2c" — a random
8-character hex code. She doesn't know this is her phone. She can see on the Devices tab that her
phone is called "Samsung Galaxy" — so why does the history say "3a7f1b2c"? It looks like a bug.

**3.4 Settings on Android requires a Save button; Mac does not.**  
She changes a setting on Android, taps the back button, and her change is lost because she forgot
to tap Save. On the Mac the same setting saves automatically. This is inconsistent and she will lose
changes at least once before learning the Android behavior.

**3.5 The persistent "Active" notification on Android.**  
It cannot be dismissed. To a normal user, a permanent notification that can't go away feels like the
app is doing something wrong. She will look in the notification settings and find that she can only
set it to "Silent" but not remove it entirely. She will feel the app is intrusive.

**3.6 Pairing confirmation dialog on Android does not explain anything.**  
"Match" and "Doesn't match" as button labels on a dialog showing a 6-digit number is confusing.
Signal says "The safety numbers have changed." WireGuard shows a QR. These are better than showing
a number with no context. The correct label is "Yes, the numbers match" or "Confirm" with a
one-sentence explanation above it.

**3.7 "Pairing ended — check the other device."**  
If she is too slow (the 60-second timeout elapses), the dialog closes and shows this message. She
doesn't know whether it succeeded or failed. Did she pair? Did she not pair? What does "check the
other device" mean? Which device? What should she be looking for?

**3.8 The "LAN Discovery" section shows "No devices found" and she doesn't know why.**  
On the Mac, the Devices tab shows "No devices found" in the LAN Discovery section even after pairing.
This is because LAN Discovery is for *unpaired* devices, and her phone is now paired. But to her, it
looks like the connection is broken.

**3.9 No "it worked" moment after sync.**  
She copies something on her phone. She opens her Mac and presses Cmd+Shift+V. The item is there. But
she had to actively check. There is no "Synced from Android" notification, no tray popup, no visible
signal that a sync just happened. If the item is not there, she doesn't know if sync is working or
not. She has no feedback loop.

**3.10 Six settings tabs with dozens of options she will never use.**  
Storage quota, sensitive auto-wipe TTL, image quality, preview lines (1–6), translucency, polling
interval (she doesn't even know what polling means), HKDF, p2p_identity... These all exist for a
reason but they are not relevant to her core use case. The settings UI presents them all equally.

---

## 4. What Is Missing (In Plain Language)

**Priority 1 — It should just work when I'm not on the same Wi-Fi.**  
The biggest thing missing for me is sync that doesn't require us to be on the same network. Universal
Clipboard just works. iMessage sync just works. This should too. I don't want to manage "cloud sync
setup." I want to tap "Connect devices" once and have it work everywhere. The way it is now, I think
it's broken 50% of the time (when I'm off the same Wi-Fi) and it's not broken, it's just... offline
and silent.

**Priority 2 — Tell me clearly that it worked.**  
When something I copied on my phone appears on my Mac, tell me. A small notification, a tray popup,
something. Right now sync is invisible. When it works, I feel nothing. When it doesn't work, I also
feel nothing. I have no idea what state the app is in.

**Priority 3 — Simpler pairing. Ideally zero manual steps.**  
Ideally: I open both apps, they find each other automatically on my Wi-Fi, and I tap "Connect" once.
No 6-digit codes, no blur/click to reveal, no 60-second timeouts. The way Apple's Universal Clipboard
works. I understand security might require some confirmation, but make it feel like adding an AirPod
to my phone, not configuring a VPN.

**Priority 4 — Cloud sync without needing a developer account.**  
I don't have a Supabase account and I don't know what one is. If cloud sync is a feature, there should
be a simple "Create an account" or "Sign in with Google" button. Or make it completely automatic with
your own hosted service. Don't show me a form asking for a project URL.

**Priority 5 — Tell me when something is too big to sync, and give me options.**  
I copied a 14 MB photo. It silently didn't sync. I want to know why and what I can do about it. Even
a small message: "This file is too large to sync. It is saved locally on your phone only."

**Priority 6 — An iPhone app.**  
I know this is Android + Mac, but most of my friends and colleagues have iPhones. If I recommend this
app to someone, 80% of them can't use it. This is the biggest adoption blocker.

**Priority 7 — A better "what is this app?" moment when I first open it.**  
The first screen is an empty list. There should be 3 sentences: "CopyPaste remembers everything you
copy. Copy on your phone, paste on your Mac. Press Cmd+Shift+V to open your clipboard anywhere."
That's it. Then a button: "Connect my phone." Guide me through it.

---

## 5. Would I Keep Using It or Uninstall?

Honest answer: **probably uninstall, but with regret.**

Here is why I would keep trying: when sync works (on the same Wi-Fi), it is genuinely faster and
better than anything else I have tried for Mac+Android. The 2-second latency, the fact that I can
see my Android clipboard history right in the Mac popup, the way I can pin things — this is actually
good. The core idea is excellent.

Here is why I would eventually uninstall: sync stops working the moment I leave my home network, and
nothing tells me it stopped. I would spend a week thinking the app is unreliable, not understanding
that I need to configure Supabase. No normal person will configure Supabase. The app punishes me for
not knowing something I should not have to know.

The secondary reason I would uninstall: pairing is annoying enough that I wouldn't want to re-do it
when I get a new phone or reinstall the app. The SAS confirmation dialog with its confusing labels
and 60-second timer is the kind of thing that makes me feel stupid, and apps that make me feel stupid
get deleted.

**If the app had a simple "Sign in with Google to sync everywhere" option, I would pay for this app.
The encryption story is great (even if I don't fully understand it), the Mac popup is great, and
cross-device clipboard is genuinely useful. The product is 70% of the way to something I would
enthusiastically recommend. The remaining 30% is entirely about friction.**

---

## 6. Top 10 Wishlist (Plain Language, Ranked)

1. **Sync everywhere, not just at home.** Make it work off my home Wi-Fi without making me sign up
   for developer infrastructure. A simple hosted cloud option (like "Sign in to CopyPaste Cloud")
   would fix this instantly.

2. **Tell me when something syncs.** A tray notification or a small badge: "3 items synced from
   your Android." Right now sync is invisible and I can't tell if it's working.

3. **One-tap pairing.** Make my devices find each other automatically when they're on the same
   Wi-Fi and just ask me to confirm once: "Connect to Maya's MacBook? [Yes / No]." No 6-digit codes,
   no blur/click, no timer.

4. **An iPhone app (or at least an iOS share extension).** I can't recommend this to anyone I know
   who has an iPhone. This is the single biggest thing holding back the product.

5. **A welcome screen on the Mac.** When I first open it, show me: (a) what the app does, (b) how
   to open the popup (Cmd+Shift+V), (c) a button to connect my phone. Don't drop me into an empty
   list.

6. **Auto-save in Android Settings.** When I change something, it should save automatically.
   The current "Save button required" model has caused me to lose settings changes multiple times.

7. **Tell me when a file is too big to sync.** "This item is saved locally only — too large to
   sync" in small text on the file row. Simple, informative, no action required.

8. **Show my phone's name in history, not a random code.** When I see a clipboard item that came
   from my phone, show "Samsung Galaxy" or "Maya's Phone," not "3a7f1b2c."

9. **A way to paste without formatting.** When I copy something from a website and paste it into
   an email, it brings all the formatting. A "Paste as plain text" option in the popup would
   be something I use every single day.

10. **Get rid of the empty "Advanced" settings tab.** It makes the app look unfinished. Either
    put real options there or remove the tab.

---

*Persona document written as a first-person product critique. All observations are grounded in the
feature inventories, UX review, and sync documentation for this branch. No code was modified.*
