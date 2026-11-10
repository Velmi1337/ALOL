# Google Sans

Unmodified Android variable font from the official Google Sans v14.000 release:
https://github.com/googlefonts/googlesans/releases/tag/v14.000

Archive member: `build/GoogleSans/android/variable/GoogleSans[GRAD,opsz,wght].ttf`.
Stored here as `GoogleSans.ttf` without changing its contents or internal names.

SHA256: `7dd96f5c236defe6e611530762c03e1e2c22e74a83dba92d89269b0b129a699c`.

License: SIL Open Font License 1.1; see [OFL.md](OFL.md). Copyright and the full
license are also included in the root NOTICE, which ships with packages and is
embedded in `alol --license`. The font's license is separate from ALOL's code.

The UI writes the embedded font to its own data directory and registers it with
Fontconfig for this process only. No system fonts are replaced.

For AZRTC's file-based game UI, ALOL also supplies missing `segoeui.ttf` and
`seguisb.ttf` files inside its Wine prefix as aliases of this unmodified font.
Existing font files and links, including case variants, are preserved. These
aliases are Google Sans, not Microsoft Segoe UI; their internal names and bytes
remain unchanged. Copyright, license and an explanation are written alongside
the prefix's ALOL support files in `C:/alol/fonts/`.
