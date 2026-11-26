# Hersir Cloud on the desktop and the web, and Hersir on your other computers (GA-317)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Cloud: desktop, web and other computers (GA-317)**
(node `379:2`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=379-2

These are the notes for the builders: card 5 (Settings → Hersir Cloud), card 9 (Remote on screen) and card 6 (the account
pages, in `Yotech-AI/hersir-cloud`). Nothing in the app changes in this card. It follows the plan in
`docs/research/hersir-cloud-personal.md` (GA-67): sections 0.1, 3.4, 3.6, 3.11 to 3.14, 5.1, 5.4 and 5.8, and Jefsev's
answers of 20 November (A1 to A3, 2FA with an authenticator app only). It builds on the islands (GA-284 to GA-288:
`design/islands/README.md`, `design/islands-settings/README.md`): their window, glass, tokens and components.

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`. Every frame and
sheet is there in dark and in light; the light one is the dark one with the **GizAi colour** collection set to **Light**
on the frame and `Islands/Window background` set to Wallpaper=Light, so every colour follows its variable.

| # | Frame | Dark | Light | Built in |
|---|---|---|---|---|
| 1 | Settings → Hersir Cloud, signed in, on the host PC (window: the new tab, the three sections, the host's connected line in the sidebar) | `382:34` | `389:1895` | cards 5 and 9 |
| 4 | The first screen with *Restore from Hersir Cloud* (GA-288's onboarding window) | `389:4592` | `389:4681` | card 5 |
| A | Sheet: Settings → Hersir Cloud, every state (A1 to A9) | `384:416` | `389:2445` | card 5 |
| B | Sheet: *Use from other devices* on a host, every state and its dialogs (B1 to B9) | `386:576` | `389:2909` | card 9 |
| C | Sheet: on a device (the MacBook): the hosts' rail, the offline page and its states, the band, *Add a computer*, the repository note, host-only places (C1 to C7) | `387:659` | `389:3186` | card 9 |
| W | Sheet: the web pages at `cloud.hersir.dev` (W1 to W5) | `389:1307` | `389:4247` | card 6 (card 2 starts the account page) |
| | Hersir Cloud components (GA-317) | `379:3` | | |

On the sheets each state is drawn as the part that changes, on its island (the Settings tab's scroll area, a page
island, or a window crop on the glass where the rail matters), with a caption that names the state, the component and
the commands it shows. Sheet A's and B's panels are the Settings island's scroll area at 1100 wide (`.page.settings-page`,
padding `20 28`), as frame 1 shows it in its window.

## New Figma parts

On the sheet **Hersir Cloud components (GA-317)** (`379:3`):

- **Lucide icons** as components, 16 px, 1.75 stroke at 24, `text-2` (as GA-284's and GA-288's): `monitor-off` `379:10`
  (a host that is off, everywhere), `monitor-smartphone` `379:16` (the repository note, a remote workspace's foot),
  `loader-circle` `379:19`, `cloud` `379:22` (the tab), `smartphone` `379:26`, `laptop` `379:30`, `copy` `379:34`, `mail`
  `379:38`, `bell` `379:42`, `shield-check` `379:46`, `trash-2` `379:53`, `unplug` `379:61` (remote use off, Disconnect),
  `link-2-off` `379:67` (a removed device), `lock` `379:71` (Change this on PC), `cloud-upload` `379:76`.
- **`Cloud/Radio`** (`381:7`, Checked=On `381:2`, Off `381:5`): the native radio of `.radios` (accent fill and
  `on-accent` dot when on; `raised` with `line-2` when off). The app keeps `<input type="radio">` with `accent-color`.
- **`Hosts/Tile`** (`381:96`): one host in the rail, State = Connected, Connected selected, This computer, This computer
  selected, Connecting, Offline, Offline selected, Remote off, Old version, Removed. HostRail.tsx draws it.
- **`Hosts/Add a computer`** (`381:97`): the rail's `+`.

Everything else is the file's own: `App/Button` (Primary, Secondary, Ghost; Medium and Small), `App/Button danger`,
`App/Input`, `App/Checkbox`, `App/Badge` (Needs), `App/Nav item`, `Islands/Window background`, `Islands/Island tab`,
`Islands/Scrollbar`, the App/* text styles and the GizAi variables. Dialogs are drawn as BackupSettings' `ask` (`float`,
`line-2` border, `radius-xl`, padding `20 22 16`, `shadow-pop`; the heading `t-section`, the text `t-small` in `text-2`,
the actions on the right). No new token.

## Decisions in this design

1. **Settings → Hersir Cloud is a seventh tab** (`#/settings/cloud`), after GitHub and Bitbucket, as an Island tab with the
   `cloud` icon. Its sections are `FormSection`s with `Field`s, as every Settings tab: **Hersir Cloud** (account,
   storage, recovery code), **Cloud Backup**, **Use from other devices**; signed out, **Restore from Hersir Cloud**
   follows Hersir Cloud. Its controls save at once (`cloudSaveSettings`, `remoteSet`), as the local Backups section
   does; Save settings in the top bar stays for the other tabs.
2. **Connected is `success`, not teal.** 5.8 sketched the rail's dot and the host's connected line in `live`. The design
   system keeps teal for one meaning only, an agent at work now (`design/README.md`, `CLAUDE.md`), so a connected device
   or host gets a 6 px `success` dot, always with its words ("connected now", "MacBook is connected"). If Jefsev wants
   teal there after all, only the dot's variable changes.
3. **Gold only where you are needed**: *Save your recovery code* (its key icon and the **Needs you** badge), the rail's
   Inbox count (`needs` on `needs-soft` over `island`, as the closed sidebar's toolbar), *PC runs an older Hersir*
   ("Update Hersir"), and the band of a host that went offline. A lapsed plan and an unreachable Hersir Cloud are
   `warning`; failures and a full quota `danger`.
4. **The rail sits on the background**, left of the sidebar island, 44 wide (`size-topbar`), 6 from the window's edge
   and from the sidebar island (`island-gap`), as the top bar does above the page. A tile is 40 wide: a 32 × 32 mark
   (radius 6, a 1px `text-2` border; dashed `line-2` when the host can't be used) with the host's initials in `t-label`
   (or `monitor` for this computer), and its name under it in `t-caption`, cut with an ellipsis at 40 px (the tooltip has
   it whole). Selected: the tile on `canvas-hover`, its name `text`. On the background faint text is `text-2`, never
   `text-3` (GA-284's contrast rule), so a host that is off is told apart by its dashed mark, its corner icon and no
   badge or dot, not by a fainter grey. The corner icon sits on an `island` disc, 12 px: `loader-circle`, `monitor-off`,
   `unplug`, `circle-arrow-up`, `link-2-off`. `+` at the bottom opens *Add a computer*.
5. **With one host and no projects of its own** (a Mac used only for PC) the rail is hidden (C1a), as 3.11 says; it shows
   once a second host is paired. The sidebar's foot then says which computer you work on: `monitor-smartphone` and
   **PC · remote** in `t-small`, `text-2`, with the host's state in place of *remote* (C1).
6. **A host's projects while it is off** stay in its sidebar from the directory, their dots at 45 % and names in `text-3`
   (inside the island, where `text-3` keeps AA), each with `monitor-off` (14 px, `text-3`) at the right (A2). A click
   opens the offline page (C2), never data.
7. **The band** of a host that went offline is a strip at the top of the page island: `needs-soft` over `island`, a 1px
   `needs` border, radius 6, `monitor-off` in `needs`, the sentence in `text`. The page under it stays drawn at 50 %
   opacity and every write is disabled; a write that was on its way ends in the `error-banner` (C4).
8. **The QR code is on white in both themes** (5.8), 200 px of code in a 216 px white square with radius 6, so phones
   read it; the HP- code under it in `t-code`.
9. **The recovery code can't be skipped** (card 5's acceptance): after the first sign-in Cloud Backup shows *Save your
   recovery code* in place of its fields until the last group is typed; the code is never shown again. *Make a new
   recovery code* asks first (A4) with the same warning, then shows the new code in the Recovery code field the same way.
10. **Remove…** asks without "Is it lost or stolen?": that question only means something once *Remove and change the key*
    exists (question 18, later). It then gets that second button.
11. **The repository note** (A1) is a hint under *Folder*, not an error: `monitor-smartphone`, the host lines in `t-small`
    `text`, the rest in `text-2`, and **Open EUQR on PC** (Secondary, small). Create project stays enabled.
12. **The web pages use Hersir's tokens**, not Flux's defaults: the page `canvas-from`, the card `island`, inputs `raised`
    with `line-2`, the primary button `accent` with `on-accent`, Atkinson Hyperlegible Next and Mono, `radius-l` for the
    card and `radius` for controls. Dark and light follow the browser (`prefers-color-scheme`); the kit's Appearance
    page goes.

## Screen by screen: the component, its commands, its states

### Settings → Hersir Cloud (card 5: `src/components/CloudSettings.tsx`, `SettingsPage.tsx`, `router.ts`)

| State | Frame | What it shows | Commands and fields (5.4) |
|---|---|---|---|
| Signed out | A1 | the plan line, **Sign in** (primary, `log-in`), **Create an account** (secondary, `external-link`), hint "Signing in opens your browser."; *Restore from Hersir Cloud* under it (A9) | `cloudStatus()` `signedIn: false`; `cloudSignIn()`; Create an account opens `cloud.hersir.dev/register` |
| Signing in | A2 | `loader-circle`, the waiting line, **Cancel** (ghost) | `signingIn: true`; `cloudCancelSignIn()` |
| The recovery code and its warning | A3 | Account; in Cloud Backup: *Save your recovery code* with **Needs you**, decision 2's warning, the code in `t-code` on `raised`, **Copy**, **Save as a file…**, the last group's input and **Turn on Cloud Backup** | `key: "missing"`; `cloudMakeKey()` → `{ recoveryCode }`; `cloudConfirmRecovery(lastGroup)` |
| Signed in | frame 1 | Account (email, plan and renewal, **Account page**, **Sign out**), Storage (meter, `1.4 of 10 GB`), Recovery code (made on, **Make a new recovery code**); Back up (Daily, Only when I ask, Also when I quit Hersir completely), Last backup and **Back up now**, Backups (the `backup-list` rows with **Restore**) | `plan`, `recoveryMadeAt`, `backup.{on, every, onQuit, last, next}`; `cloudSaveSettings`, `cloudBackUpNow`, `cloudListBackups`, `cloudStartRestore`, `cloudOpenAccount`, `cloudSignOut`; event `cloud-changed` |
| A backup running | A5 | **Back up now** disabled, `loader-circle` and the step | `backup.running { step, done, total }` |
| A failure | A6 | under Last backup a `gh-line failed` (`circle-x` in `danger`) | `backup.lastError { at, message }` |
| The quota full | A7 | the meter full in `danger` with its words, the refusal, **Back up now** disabled, **Account page** | `plan.usedBytes`, `quotaBytes`; 413 `quota` |
| A lapsed plan | A8 | Account says the plan ended, **Renew on the account page**; a `warning` line; Backups still list with **Restore** | `plan.status: "lapsed"`, `readOnlyUntil`; `past_due` changes nothing on screen |
| Make a new recovery code | A4 | the ask, then the new code in the Recovery code field with its last group and **Done** | `cloudNewRecovery()` → `{ recoveryCode }` |
| Restore from Hersir Cloud | A9 | three numbered steps (the current one's number on `accent`, a done one a `success` check, a later one `line-2` with its title in `text-3`): Sign in, Your recovery code, Pick a backup; then GA-218's steps | `cloudSignIn()`, `cloudEnterRecovery(code)` (`key: "needed"`), `cloudListBackups()`, `cloudStartRestore(name)`, then `restoreStatus` and `restartToRestore` as BackupSettings does |

The meter is a 240 × 6 track on `raised` with a `line-2` border; what is used is `text-2`, `danger` when full. Sizes and
the code are mono (`t-code`). **Restore** on a row opens BackupSettings' ask, as for a folder.

### The first screen (card 5: `src/pages/OnboardingPage.tsx`, frame 4)

A strip at the island's foot, under the step's actions, on step 1 only: `raised` with a `line` border, radius 6, the line
in `t-small` `text-2` and **Restore from Hersir Cloud** (Secondary, small, `archive-restore`). It opens A9's three steps
inside the onboarding island in place of step 1; after the restore Hersir restarts, so the rest of the onboarding is
skipped (the restored data has its own team).

### Use from other devices, on a host (card 9: `src/components/RemoteSettings.tsx`, `PairDialog.tsx`)

| State | Frame | What it shows | Commands (5.8) |
|---|---|---|---|
| Off | B1 | the switch off, *Keep PC awake* disabled | `remoteStatus().on: false`; `remoteSet({ on })` |
| On with no devices | B2 | the empty line, **Add a device** | `devices: []` |
| Devices connected and not | B3, frame 1 | each device: `laptop` or `smartphone`, name, kind, a `success` dot and "connected now" or when it was last seen, its scope; **Last actions** (ghost, `history`), **Remove…** | `RemoteDevice { connected, lastSeenAt, scope }` |
| Can't be turned on | B4 | the switch disabled with the reason as its hint: signed out, no plan; signed in but Hersir Cloud unreachable: a `warning` line | `RemoteStatus.problem` (`signed_out`, `plan`, `offline`) |
| The host's sidebar | B5, frame 1 | at the bottom of the sidebar island, under a `line`: the dot, the names, **Disconnect** (ghost, `unplug`) | `remoteDisconnectAll()` |
| Add a device | B6 | the ask, 520 wide: the QR code, the HP- code, the line, how long it works, **Cancel** | `remotePairStart()` → `Pairing { qr, code, expiresAt }`; `remotePairCancel(id)` |
| A device joining | B7 | 3.13 step 3's question, *What it may do* (Everything above, Board and chat only), **Cancel**, **Add** | `Pairing.joined`; `remotePairAnswer(id, add, scope)` |
| Last actions | B8 | the device's writes, newest first: time (`t-small`, `text-2`), the action in words, the card or chat in `t-id` | `remoteDeviceLog(id)` → `RemoteAction[]` |
| Remove… | B9 | the ask with **Remove** (`App/Button danger`) | `remoteRemoveDevice(id)` |

The last-action words for `remote_log`'s commands: `start_run` Ran, `stop_run` Stopped, `continue_run` and
`continue_after_run_for_me` Continued, `approve_pending_action(s)` Approved a call, `discard_pending_action` Discarded a
call, `add_comment` Commented on, `move_task` Moved to *column*, `create_task` Added, `update_task` Changed,
`send_chat` Wrote to the Chief in; any other command shows its own name in `t-id`.

### On a device (card 9: `HostRail.tsx`, `HostOffline.tsx`, `AddComputer.tsx`, `App.tsx`, `Sidebar.tsx`)

| State | Frame | What it shows | Commands (5.8) |
|---|---|---|---|
| One host | C1a | no rail; the sidebar's foot **PC · remote** | `hosts()` with one host and no projects of this computer |
| Two hosts | C1b | this computer's tile (`monitor`), PC selected with the dot and its Inbox count, `+` | `Host.state` `this`, `connected`; `inbox`; `hostSelect(id)` |
| A host connecting | C1c | the PC tile's corner `loader-circle` | `connecting` |
| Offline | C1d | dashed mark, `monitor-off`; its projects grayed with the icon; tooltip "PC is offline since 09:12" | `offline`, `offlineSince` |
| Remote off | C1e | `unplug` | `remote_off` |
| An old version | C1f | `circle-arrow-up` | `old_version` |
| Removed | C1g | `link-2-off` | `removed` |
| The offline page | C2 | in the page island: `monitor-off` 20 and **PC is offline** (`t-drawer-title`), 3.11's text in `t-prose` `text-2` (max 620), the projects as chips (`monitor-off` 12 and the key in `t-id`, on `raised`; the clicked one on `selected`), **Check again**, *Tell me when PC is back* | `Host.projects`, `hostCheckAgain(id)`, `hostNotifyWhenBack(id, on)`; event `hosts-changed` |
| The page's other states | C3 | the same layout: Connecting, Remote use is off, an older Hersir (its icon in `needs`), removed, the plan ended | `Host.state`, `Host.message`, `RemoteStatus.problem` |
| The band | C4 | the band, the page grayed, writes disabled, the error banner for a write on its way | `onHostsChanged`; the write's refusal |
| Add a computer | C5 | the account's computers not paired yet with **Pair** (disabled while that computer is offline), then the code to type; the phone line | `hosts()` (from `GET /devices`), `hostPair(code)` |
| The repository note | C6 | under *Folder* in the project form: one host, that host offline, two hosts | `repoElsewhere(repoUrl)` → `RepoElsewhere[]`; **Open EUQR on PC** → `hostSelect(hostId)` and the project |
| Host-only places | C7 | the Editor and Terminal pages: `monitor` 20 in `text-3` and the line, centred in the page island; Settings tabs and the agent form read-only, with `lock` and **Change this on PC** at the top and the fields disabled | 3.12's host-only list; the refusals of `save_project` and `save_settings` |

### The web pages at `cloud.hersir.dev` (card 6, card 2 for the first account page; sheet W)

The Livewire starter kit's views, with Hersir's tokens from `design/tokens.json` in place of Flux's colours: the page
`canvas-from` (no glass and no glow on the web), cards `island` with `radius-l`, inputs `raised` with a `line-2` border and
`radius`, the primary button `accent` with `on-accent` text, a secondary button `line-2`, `danger` for errors and Delete
account, `success` for a done line, text `text`, `text-2`, `text-3`; type `--font-sans` Atkinson Hyperlegible Next (the
headings `t-drawer-title` 20/600, labels `t-label`, body `t-body`, hints `t-caption`) and `--font-mono` Atkinson
Hyperlegible Mono for codes. The logo: the `cloud` icon and **Hersir Cloud** in `t-section` (the longboat H can take its
place once the brand has a Cloud mark). Dark and light follow `prefers-color-scheme`.

The view names below are those of `laravel new --livewire` in November 2026 as the research names them; card 2 checks them
against its first commit and keeps these changes on whatever the kit calls them.

| Page | Frame | Starter-kit view it changes | What changes |
|---|---|---|---|
| Create an account (`/register`) | W1 | `resources/views/livewire/auth/register.blade.php` in `components/layouts/auth/simple.blade.php` | the lead line; the password hint (`Password::defaults()`: 12 or more, checked against leaks); the logo; the tokens |
| Check your email (`/verify-email`) | W2 | `livewire/auth/verify-email.blade.php` | the texts; the status line after a new link |
| Set up two-factor authentication | W3 | `livewire/settings/two-factor.blade.php` and its recovery-codes part, moved into the auth layout as a page of its own | required: `RequireTwoFactor` sends every user here until `two_factor_confirmed_at`; QR on white, the key, the code, the 8 recovery codes with Copy and Download, **Confirm and continue** |
| Your account (`/account`) | W4 | `components/settings/layout.blade.php`, `livewire/settings/profile.blade.php`, `password.blade.php`, `two-factor.blade.php`, `delete-user-form.blade.php` | one column on one island (the kit's side menu and Appearance go); new sections Plan (Cashier: **Manage billing**, or **Subscribe**), Storage, **Your computers and devices** (each with **Remove**: `DELETE /devices/{id}`), Withdraw from the contract |
| Signed in (the desktop's sign-in) | W5 | new, in the auth layout | the done page after `/oauth/authorize` |

## Every text, word for word

New in this design. Sample data (names, emails, dates, sizes, codes, cards) is not text to build; the captions on the
sheets are notes for the builders.

**Settings → Hersir Cloud**

- Tab: **Hersir Cloud**
- Section **Hersir Cloud**: **Encrypted backups of your data, kept off your computer. Restore them on a new one.** (signed
  in it adds **Hersir can't read what it stores.**)
- Signed out: field **Personal plan**: **€5 a month or €48 a year, VAT included. Hersir can't read what it stores.**; field
  **Account**: **Sign in**, **Create an account**, hint **Signing in opens your browser.**
- Signing in: **Waiting for you to sign in in your browser…**, **Cancel**, hint **Sign in with your password and your
  authenticator's code. Didn't a tab open? Cancel and sign in again.**
- Signed in: **Account**: *email* **· Personal, renews** *9 Nov 2027*, **Account page**, **Sign out**; **Storage**: *1.4*
  **of 10 GB**; **Recovery code**: **Made on** *9 Nov 2026*, **Make a new recovery code**, hint **Hersir never keeps it on
  a computer. A new code replaces the old one, which stops working.**
- Section **Cloud Backup**: **A copy of this computer's data in Hersir Cloud, encrypted before it leaves. Each backup is
  kept 7 days, then one a day up to 90 days.**; **Back up**: **Daily**, **Only when I ask**, **Also when I quit Hersir
  completely**; **Last backup**: *Today 09:02* **·** *38 MB* **sent · next** *tomorrow 09:02*, **Back up now**;
  **Backups**: rows *when* · **Daily** / **On quit** / **By hand** / **Before a restore** · *the computer, or* **this
  computer** · *size* · **Restore**; hint **Restoring saves today's data first and finishes when Hersir restarts.**
- **Save your recovery code**, badge **Needs you**, then decision 2's warning word for word with one sentence after it:
  **Hersir encrypts your data on this computer before it leaves. Only your computers and this recovery code can open it.
  If you lose this code and every computer signed in to Hersir Cloud, your cloud backups can't be opened by anyone,
  Hersir included. Hersir won't show it again.** **Copy**, **Save as a file…**, field **Type its last group to show you
  have it**, **Turn on Cloud Backup**; a wrong group: **That isn't the code's last group. Look at the code above again.**
  Copy's result: **Copied**.
- Running: **Copying your data…**, **Encrypting…**, **Uploading:** *112* **of** *240 MB***…**, **Finishing…**
- Failure: *Today 09:02***: the backup failed.** *the message* (for an unreachable server: **Hersir Cloud couldn't be
  reached.**) **Hersir tries again at the next backup, or back up now.**
- Quota full: **Hersir Cloud is full: 10.0 of 10 GB.**, hint **Backups stop until there is room. Older backups go after
  90 days, or delete some on the account page.**
- Lapsed: **· Personal ended on** *9 Dec 2026*, **Renew on the account page**, **Your plan ended on 9 Dec 2026: backups
  can be restored until 7 Feb 2027.**, hint **Hersir makes no new cloud backups until you renew. Your data on this
  computer is untouched.**
- Make a new recovery code: ask **Make a new recovery code?** with decision 2's warning and **Your current code stops
  working once you type the new one's last group.**, **Cancel**, **Make a new code**; then **Save your new recovery code.
  It replaces your old one once you type its last group.**, hint **Type its last group to show you have it.**, **Done**
  (GA-370: the new code is stored only at Done, so until then the old one keeps working)
- Section **Restore from Hersir Cloud**: **Your data from a backup in Hersir Cloud. It replaces what is on this computer,
  which Hersir saves first.**; steps **Sign in** (**Opens your browser: your password, then your authenticator's code.**;
  done: **Signed in as** *email*), **Your recovery code** (placeholder `HC-____-____-____-____-____-____`, **Check**;
  done: **Accepted.**; wrong: **That recovery code doesn't open this account's backups.**), **Pick a backup** (before:
  **Your backups show once the code is right.**; then the rows and **Hersir restarts to finish, as a restore from a
  folder does.**); then GA-218's texts (**Downloading:** *640* **of** *1,200 MB***…**, **Checking the backup…**, the
  ready line, **Restart now**, **Cancel restore**).
- First screen: **Moving from another computer? Your data comes back from your backup in Hersir Cloud.** and **Restore
  from Hersir Cloud**

**Use from other devices** (on a host; PC is the host's name)

- Section **Use from other devices**: **See and work on PC's projects from your MacBook's Hersir and your phone. PC does
  the work: its agents, folders and runs stay here.**
- **Use PC's projects from my other computers and my phone**, hint **They send PC's own commands through Hersir Cloud,
  end to end encrypted. PC must stay on.**
- **Keep PC awake while this is on**, hint **On by default on a computer without a battery. Hersir can't wake a computer
  that sleeps.**
- **Your devices**: *name* **· desktop** / **· phone · connected now** or *when* **· Everything** / **· Board and chat
  only**, **Last actions**, **Remove…**; none: **No devices yet. Add your MacBook or your phone: each one joins with a
  code PC shows.**; **Add a device**
- Can't be on: **Sign in to Hersir Cloud first: Hersir Cloud above.**; **Hersir Cloud's plan ended on 9 Dec 2026: use PC
  on PC itself, or renew.**; **Hersir Cloud can't be reached, so your devices can't reach PC now. Hersir keeps trying.**
- The host's sidebar: **MacBook is connected** (two: **MacBook and iPhone are connected**; more: **3 devices are
  connected**), **Disconnect**
- Add a device: **Add a device to PC**, **Open Hersir on your other computer and choose Add a computer, or scan this with
  your phone's camera. It works for 5 minutes.**, **Or type this code**, **Works until** *14:07***.**, **Cancel**; expired:
  **This code has expired.** and **Show a new code**
- Joining (3.13 step 3): **Add iPhone (Safari) to PC?** **It will see PC's projects and can run agents, approve the
  Chief's calls and add cards and comments. It can't use the terminal, the editor or PC's folders, or change what agents
  may run.** **What it may do**: **Everything above**, **Board and chat only: no Run, Stop, Continue or approvals**;
  **Cancel**, **Add**
- Last actions: **MacBook's last actions**, **What MacBook did on PC in the last 30 days, newest first.**, **Close**; none:
  **Nothing yet.**
- Remove: **Remove MacBook?** **It is disconnected and signed out of Hersir Cloud at once, and can no longer use PC.**
  **Cancel**, **Remove**

**On a device**

- The sidebar's foot: **PC · remote**, **PC · connecting…**, **PC · offline since 09:12**, **PC · remote use is off**,
  **PC · update Hersir on PC**, **PC · removed this device**
- Tile tooltips: **PC · 3 need you** (or **PC**), **Connecting to PC…**, **PC is offline since 09:12**, **Remote use is
  off on PC**, **Update Hersir on PC to use it from here**, **This device was removed from PC**, `+` **Add a computer**
- The offline page (3.11, word for word): **PC is offline** / **Since today 09:12. EUQR, GA and its other projects are on
  PC, and Hersir keeps no copies of them, so they open again when PC is on, awake and running Hersir.** / the projects /
  **Check again**, **Tell me when PC is back**
- Its other states (3.11): **Connecting to PC…** (with **Hersir is reaching PC through Hersir Cloud. After 10 seconds
  without an answer, PC shows as offline.**); **Remote use is off on PC** / **Turn it on in Hersir on PC: Settings →
  Hersir Cloud → Use from other devices**; **PC runs an older Hersir** / **Update Hersir on PC to use it from here**;
  **This device was removed from PC** / **Pair it again from PC's Settings → Hersir Cloud**, with **Add a computer**; the
  plan: **Hersir Cloud's plan ended** / **Hersir Cloud's plan ended on 9 Dec 2026: use PC on PC itself, or renew.** with
  **Renew on the account page**
- The band (3.11): **PC went offline at 14:02. What you see may be out of date; changes can't be sent.**; a write on its
  way: **PC went offline before it answered. Check on PC whether it was done.**; a write button's title while offline:
  **Disabled while PC is offline**
- Add a computer: **Add a computer**, **Your computers on Hersir Cloud that this Mac doesn't use yet. Pair one with the
  code it shows.**, *name* **· online · not paired** / **· offline since** *yesterday* **· not paired**, **Pair**, **To add
  your phone, open app.hersir.dev on it.**, **Close**; then **Pair with PC**, **Type the code PC shows in Settings →
  Hersir Cloud → Use from other devices → Add a device.**, **Back**, **Pair**; refusals (5.8): **That code has expired:
  show a new one on PC.**
- The repository note (3.11, word for word): **This repository is already the project EUQR on PC. Adding it here makes a
  separate project on this Mac, with its own cards, agents and runs; nothing is shared with EUQR on PC. To work on EUQR,
  open PC in the rail.** **Open EUQR on PC**; that host off: **This repository is already the project EUQR on PC (offline
  since 09:12). Adding it here…** (the rest the same); several hosts, one line per host, then **Adding it here makes a
  separate project on this Mac, with its own cards, agents and runs; nothing is shared with them. To work on one, open its
  computer in the rail.** ("this Mac" is this computer's kind: this PC, this computer on Linux.)
- Host-only: **The Editor works on PC itself.**; **The terminal works on PC itself.**; **Change this on PC**; refusal
  (3.12): **The project's folder and setup commands can only be changed on PC.**

**The web pages**

- Create an account: **Create an account**, **Hersir Cloud keeps encrypted backups of your data. Two-factor
  authentication is required.**, **Name**, **Email address**, **Password** (**At least 12 characters.**), **Confirm
  password**, **Create account**, **Already have an account? Log in**
- Check your email: **Check your email**, **We sent a link to** *email***. Open it to confirm your address; then set up
  two-factor authentication.**, **Send the link again**, **Log out**, **Sent again: a new link is on its way.**
- Set up two-factor authentication: **Set up two-factor authentication**, **Required for every Hersir Cloud account. Scan
  the code with an authenticator app, then type the 6-digit code it shows.**, **Or type this key:** *key*, **Code from
  your app**, **Save these recovery codes. Each one signs you in once if you lose your phone.**, **Copy**, **Download**,
  **Confirm and continue**
- Your account: **Plan** (**Personal: Cloud Backup and Hersir on your other devices.**; **Personal ·** *€48 a year,
  VAT included* **· renews** *date*; **Manage billing**; no plan: **Subscribe**), **Storage** (**What your backups use, each
  piece once.**), **Your computers and devices** (**Each has its own sign-in. Remove one you no longer use, or lost: it is
  signed out at once.**; rows *name* · *platform* · **Hersir** *version* · **online** / **seen** *when* · **added** *date*;
  **Remove**; **This browser**), **Two-factor authentication** (**Required for every account.**; **On, with an
  authenticator app.** *6* **of 8 recovery codes left.**; **Show recovery codes**, **Make new recovery codes**),
  **Password** (**Change password**), **Withdraw from the contract** (**In your first 14 days, as a consumer: your money
  back and the plan ends.**; **Withdraw**), **Delete account** (**Your password and a code from your app. Your cloud
  copies go after 7 days; nothing on your computers changes.**; **Delete account**), **Log out**
- Done page: **Signed in**, **Signed in. You can close this tab and go back to Hersir.**

## Open points for Jefsev (none blocks building)

1. **Teal for connected** (decision 2 above): the design uses `success`; 5.8 sketched `live`.
2. **Question 21** (keep a host awake) and **question 23** (*What it may do*) are drawn with the research's
   recommendations (on without a battery; Everything by default). If Jefsev answers otherwise, B2's checkbox and B7's
   radios change, not the layout.
3. **Remove and change the key** (question 18) is left out of B9 until it exists.

## Not in this card

- The phone layouts and the PWA's own screens (card 10, `design/hersir-phone.md`, builds on this).
- Building any screen (cards 5, 6 and 9), and the server's views beyond what sheet W draws.
- The emails (verification, a new computer, a removal, the lapse), which card 6 writes in the same words' style.
