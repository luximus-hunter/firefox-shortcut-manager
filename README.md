# Firefox Shortcut Manager

Create Windows Start menu shortcuts that open websites in a selected Firefox
profile, optionally in one of that profile's containers.

Existing shortcuts can be edited from the Shortcuts page, including their
website, Firefox profile or container, window mode, and Start menu name.

## Browser and app windows

Shortcuts can open in a regular Firefox browser tab or in a separate app
window. App-window shortcuts use Firefox's `-taskbar-tab` and `-new-window`
launch arguments, along with the selected profile and the container's numeric
`userContextId` (`0` when no container is selected). They open containers
directly through Firefox and do not require the external-container extension.

Firefox Taskbar Tabs must be enabled in the selected profile (enable it in
Firefox Labs or set `browser.taskbarTabs.enabled` to `true` in `about:config`).
When no existing registration matches the
shortcut's URL and container, the manager adds one to
`<profile>\taskbartabs\taskbartabs.json`; Firefox must be closed if a registry
change is needed. Existing matching app registrations are reused, and unrelated
registry entries and fields are preserved. Unsupported or malformed registry
data is left unchanged and reported as an error. To migrate an older managed
app shortcut that is not registered, edit and save it while its Firefox profile
is closed.

## Compact app-window toolbar

[userChrome.css](userChrome.css) provides an optional compact toolbar for
Windows app windows. It shows Firefox’s native app icon and the registered app name on the left,
with the native container icon and name beside it when a container is selected.
It keeps pinned extensions, the extensions menu, the
profile/account toolbar button, extension overflow, and native window controls.
The blank area between the app identity and buttons lets you drag or double-click
the window. Tab controls,
navigation/address controls, bookmarks, the sidebar, and the main menu are
hidden. Firefox notification bars and popup panels are left intact.

All rules require `:root[taskbartab]`, Firefox's native app-window marker;
ordinary browsing windows in the same profile keep their normal appearance.
This stylesheet does not change your saved toolbar layout. The profile/account
button must already be placed on the navigation toolbar using **Customize
Toolbar** in a regular Firefox window. CSS cannot create a missing button.

In the app, open **Firefox**, find the profile, and click **Install app toolbar**.
Close Firefox for that profile first. The app backs up `chrome/userChrome.css`
and `user.js` in `fsm-app-chrome-backup.json` inside the profile, installs the
bundled stylesheet, and enables Firefox's custom stylesheet preference.
**Disable and restore original** restores the original files (or removes them
if none existed), restores the previous stylesheet preference, and removes the
backup after a successful restore. Other preferences changed since installation
are preserved. If you edit installed files, disabling stops and keeps the backup
so your edits are not silently discarded. Reopen Firefox after either change.

After a stylesheet update, click **Update toolbar** to copy the latest bundled
rules and registered app names without replacing the original backup, then fully
restart Firefox. Use this after adding or renaming app shortcuts too. Updating
only stages CSS, so Firefox can remain open while copying; installation and
disabling still require the profile to be closed.

To install manually for each profile used by app shortcuts:

1. Open `about:support` in that profile and click **Open Folder** next to
   **Profile Folder**.
2. Create a `chrome` folder there if needed, then copy this repository's
   `userChrome.css` into it. If you already have a `chrome/userChrome.css`,
   back it up and append these rules instead of replacing your customizations.
3. In `about:config`, set
   `toolkit.legacyUserProfileCustomizations.stylesheets` to `true`.
4. Fully exit Firefox, then reopen an app shortcut.

To undo, remove the appended rules (or the copied file) and restart Firefox.
Firefox's internal toolbar selectors can change between releases. Other
userChrome customizations may conflict with these rules.

