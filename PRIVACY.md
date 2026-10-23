# Tideline Journal: privacy and security

This page covers the Windows app, the Android app and the optional sync server. It says what is protected, what leaves your devices, what the sync server can and can't see, and where the limits are. The last section has the technical details for anyone who wants to check.

*Last updated 23 October 2026.*

## In short

- **Your diary is encrypted on your own devices**, with a password only you know. Without the password (or your recovery key), the diary file is unreadable.
- **There is no Tideline account, cloud service, advertising, analytics or tracking.** Neither app contacts anyone unless you turn on a feature that needs it, and the only such feature that reaches beyond your own PC is sync.
- **Sync is optional and goes to a server you run yourself.** Everything is encrypted on your device before it is sent, so the server only holds scrambled data.
- **No one can recover a forgotten password for you**, including the author. A recovery key is the only way back in. That is what keeps the diary private.

## What is protected

| What | How |
| --- | --- |
| Entries, titles, moods, categories, templates, pictures, attached files | Encrypted inside the diary file. |
| The entry you are in the middle of writing | On the phone it is saved as a draft, encrypted with the diary's own key. On Windows it is kept only in memory while the diary is locked. |
| Automatic and manual backups of the diary | Encrypted exactly like the diary. |
| Everything sent to the sync server | Encrypted on your device first (see below). |
| The password on the phone, when you use fingerprint unlock | Encrypted with a key that lives in Android's secure key store and needs your fingerprint to use. |
| Private entries | Also hidden in the apps until you tap them, left out of text search, Insights, PDF exports (unless you ask) and the AI helpers. They are still synced and encrypted like every other entry. |

## What leaves your devices

| Feature | What is sent, and to whom |
| --- | --- |
| **Nothing at all, by default** | Neither app sends anything anywhere until you set up sync. |
| **Sync** (optional) | Encrypted records and pictures go to the server address **you** enter. See the next section for what that server can see. |
| **Dictation** (Android, optional) | When you use the microphone button, Android's own speech recognition runs. Depending on your phone's settings, that service may send the audio to its provider (usually Google) to turn it into text. Tideline Journal itself does not send audio anywhere. If this matters to you, don't use dictation, or choose an offline speech language in Android's settings. |
| **AI helpers** (Windows, optional) | They talk only to Ollama running on **your own PC** (`127.0.0.1`). Nothing goes over the internet, private entries are never included, and each result window shows exactly what was sent. |
| **PDF export** (Windows) | Writes a readable file wherever you choose. It is **not encrypted**. |
| **Copy details / QR code** | Show or copy the server address and sync code, so you can enter them on another device. The sync code is a secret: show it only when you need it. The QR code never contains your password. |
| **Reminders** (Android, optional) | A notification that only says "Time to write today?". It never shows what is in your diary. |

The Android app asks for these permissions: **Internet** (sync only), **Camera** (scanning the pairing QR code and taking photos), **Microphone** (dictation), **Notifications and exact alarms** (the daily reminder) and **Run at start-up** (to set the reminder again after a restart). It also uses the fingerprint sensor if you turn that on. Android's own cloud backup is switched off for the app.

## The sync server

You run the server on your own computer. The apps encrypt every record and picture with a key that comes from your diary password before sending it.

**What the server can see:** that a diary exists and its **name** (this is stored in plain text so you can tell diaries apart), how many records and pictures it holds and how big they are, when they were last changed (record identifiers also carry the time they were created), and the addresses and times of the devices that connect. Its console log shows the same kind of thing and never any diary text.

**What the server cannot see:** your entries, titles, moods, categories, pictures, attached files or your password. It does not have the keys.

**The sync code** is a secret that lets a device use your diary on the server. The server stores only a fingerprint (hash) of it. Anyone who has it can download the encrypted data and the wrapped keys, and can add to or overwrite what is on the server, but cannot read it without your password. Keep it private, and use *Change sync code* if you think it has been seen.

**Your password matters more with sync.** To let a new device join, the server holds your diary's keys wrapped by your password. Someone who got into the server could try guessing the password offline. The password stretching (Argon2id) makes each guess slow, but a short or common password can still be guessed. Use a long, unusual password.

**The connection is plain HTTP for now.** That is fine on your home network, because the data is encrypted anyway. But the sync code travels in the clear, so anyone able to watch the traffic could copy it. Keep sync on a network you trust, or use a private network such as Tailscale, and don't put the server on the open internet. HTTPS is on the to-do list for anyone who wants to sync away from home.

**Backups of the server:** the server makes its own daily backups of its database beside it. They hold only encrypted data and are safe to copy anywhere.

## Where the limits are

- **While the diary is unlocked it is readable on that device.** Lock it when you step away. Auto-lock and "hide when in the background" help.
- **A forgotten password cannot be recovered** unless you made a recovery key, and anyone who has both your recovery key and your diary file can open the diary. Keep the key somewhere safe, away from the device.
- **PDF exports are not encrypted.** Treat them as plain documents.
- **Files you open from an entry** are copied to the data folder as plain files and deleted when the diary locks or the app next starts.
- **Screenshots are allowed.** Neither app blocks them yet.
- **A few settings are stored unencrypted**, because they are needed before you unlock (for example where the data folder is, the accent colour and the window mode). They contain nothing from your diary.
- **File sizes and dates are visible** on disk. Anyone can see that a diary file exists, how big it is and when it changed.
- **The copy of the diary on your PC is only as safe as your PC.** Malware running as you could read it while it is unlocked, record your keystrokes or copy your files.
- **Pictures** taken or added on the phone are re-saved by the app, which removes the details cameras embed, such as location.

## Your choices

- **Don't use sync.** It is off until you set it up, and nothing is sent anywhere without it.
- **Turn sync off** at any time in *Settings → Sync*. Nothing is deleted from your devices or the server.
- **Remove diaries from the server** by deleting the server's database file (it holds every diary you have synced, so only do this if you want them all gone). A command to remove a single diary is planned.
- **Delete your diary** by deleting the diary file (and its backups) from the data folder, or by uninstalling the Android app.
- **Change your password** whenever you like in Settings. Your recovery key keeps working.

## Technical details

- **Diary encryption:** XChaCha20-Poly1305 (authenticated) under a random 256-bit data key. The data key is never stored as it is; it is wrapped by a key stretched from your password with **Argon2id** (64 MiB of memory, 3 passes, 1 lane, 16-byte random salt), and optionally by a key stretched from a **recovery key** (32 characters from a 32-symbol alphabet, 160 bits, shown once). Changing the password re-wraps the data key, so the recovery key keeps working.
- **Sync records:** each record (entry, category, template, the synced settings, a deletion marker) is sealed on its own with XChaCha20-Poly1305 under a key derived from the diary key (HKDF-SHA-256), bound to the diary and record it belongs to so it can't be swapped for another. Pictures and files are sealed the same way. The per-field clocks used to merge changes are inside the encrypted record.
- **Server:** an HTTP service backed by SQLite. It stores sealed records, their versions and sizes and times, sealed pictures, the diary's name, a SHA-256 hash of the sync code, and the wrapped keyring. It has no key to anything.
- **Android:** the diary and drafts live in the app's private storage (not included in Android backups). Fingerprint unlock stores the password encrypted with an AES key in the Android Keystore that requires user authentication.
- **Windows:** the data folder holds the encrypted diary, `config.json` (the few pre-unlock settings), `backups`, the sealed sync state.
- **No third-party services:** neither app includes analytics, advertising or crash-reporting code. The Android app uses the ZXing library for QR scanning, which works fully on the phone.

If you find a problem or something on this page that isn't true, please say so.
