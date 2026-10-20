# The input profile (area INT)

A receiver connected to a channel's WebSocket may send the viewer's keys back, one small record a key, as a terminal sends each key to its host. The channel decides what a key means and answers by pushing the rows that changed, with an optional cursor record. Nothing executes in the receiver, and every page stays a plain teletext page. The design was made for T42 Browser (its `docs/interaction.md`, decision 44 there) and is the engine's so that every receiver, a microcontroller's included, sends the same bytes.

## 1. Where the keys go

A channel takes keys only when its descriptor says `"input": true`, which is allowed only with push over a WebSocket (REQ-WEB-002). A key record is one binary WebSocket message from the receiver; the connection is the channel's own push socket, so nothing new is opened and no address is taken from a page (REQ-WEB-005). A receiver sends keys only while the viewer has a page of that channel open, and says so on screen; which keys a receiver keeps for itself is the receiver's choice, as a terminal emulator's is.

## 2. The key record

Fourteen bytes, and at most 32 bytes of text after them for committed text (REQ-INT-001): version 1, kind, a sequence number, the receiver's clock in milliseconds, and three 16-bit fields whose meaning the kind gives.

| Kind | Meaning | a | b | c |
|---|---|---|---|---|
| 1 | Key down | key code | modifiers | physical key |
| 2 | Key up | key code | modifiers | physical key |
| 3 | Text committed by an input method or a phone (the text follows) | 0 | 0 | 0 |
| 4 | The viewer opened a page, or none | page, 0x100 to 0x8FF, or 0 | 0 | 0 |
| 5 | A click on a cell of the open page | column 0 to 39 | row 0 to 24 | 0 |
| 6 | The keys left the page (0) or came to it (1) | 0 or 1 | 0 | 0 |
| 7 | The connection was made again: the last sequence number sent | that number | 0 | 0 |

Modifiers are 1 Shift, 2 Control and 4 Alt; 16 marks a repeat of a held key. 8 is left for Meta, which a receiver keeps for its own shortcuts and never sends.

A **key code** is what the key means: a printable character is its Unicode scalar value in the Basic Multilingual Plane, and a named key is 0xF000 plus its place in the W3C `KeyboardEvent.key` table of `neoteletext_input::names`. A **physical key** is where the key is, its place in the W3C `KeyboardEvent.code` table, or 0 when unknown, so a game can ask for the key at W's position on any layout. Both tables only ever grow at the end (REQ-INT-003).

The clock is the receiver's own, from any start: only differences mean anything, and it is never the time of day. A sender sends at most 120 records a second and never drops a key-up (REQ-INT-004).

## 3. The cursor record

Optional, as a cursor is (REQ-INT-005). A channel that is typed into sends it to place the cursor, to ask for local echo, and to say which key record it has acted on, which lets a receiver measure the round trip; a game sends none, or one with no cursor for the acknowledgement alone.

| Field | Bytes | Meaning |
|---|---|---|
| `page` | 2 | The page it is for; ignored for any other |
| `acked` | 2 | The last key record acted on, or 0 |
| `row` | 1 | 1 to 23, or 255 for no cursor |
| `column` | 1 | 0 to 39 |
| `width` | 1 | How far local echo may run from the column, 0 to 40 cells |
| `flags` | 1 | 1 echo; 2 masked (echo `*`); 4 a block cursor rather than a bar |

**Carriage.** The record is type 4 of the audio profile's record frame (REQ-AUD-003: version, type, length, body, CRC-32), sixteen bytes in all, and travels as one fragment on the audio profile's independent data lines: packet 4/31, data channel 12, a user-defined service of EN 300 708 clause 6.7.3.2 (REQ-AUD-007). The design first proposed data channel 13; EN 300 708 Table 16 gives channel 13 to Datavideo (clause 6.6), so it cannot be taken (decision E.91). Because the record is always one fragment, a receiver reads it from the packet without reassembly, and the audio receiver steps over it, so its record id can never meet a sound's.

**In a TTI page** the record is a line, `CU,row,column,width,flags[,acked]`, for the page that holds it (REQ-INT-006). There is no cursor in the teletext standards, and X/26's reserved modes are not this profile's to take.

## 4. Local echo

With `echo` set, a receiver may draw a printable key at the cursor at once, provisionally, in a style no page can draw, and remove it when a cursor record's `acked` reaches that key's sequence number or after two seconds; it never runs past `width` cells from the cursor's column, and `masked` echoes `*`. A receiver without a display of its own ignores echo.

## 5. Page selection

A channel that takes input is a videotex host, not a teletext set: the receiver holds no pages of its own to choose from, so every key goes to the channel, digits included. A channel is asked to follow the Prestel convention, `*` page `#` for a page, `#` for the next, `*#` for the one before and `*00` to show the page again, and may also take three plain digits as a page number where it has no other use for them. The receiver interprets neither.

## 6. A phone as the keyboard: qrkeyboard

qrkeyboard (`qrkeyboard.com`, the owner's own relay) makes a phone the keyboard of a screen: the screen shows a QR, the phone that scans it pairs with the screen through the relay, and its keys travel end to end encrypted. `neoteletext-qrkeyboard` is the screen's side of its protocol 3, for desktop programs (decision E.93):

- **The instance.** The screen asks the relay for an instance (`POST /i`) with its configuration: its name, the phone's controls (a preset, or the terminal layout of a text box, the direction pad and the colour keys), whether a PIN is used, and how long a dropped phone keeps the screen. The relay answers with a join code, an owner token and the address of the screen's WebSocket, which must be on the relay's own host.
- **The QR** holds the relay's phone page with the code, the pairing key in no-PIN mode, and the configuration's hash, all in the fragment, which the phone never sends. The hash is SHA-256 of the configuration in qrkeyboard's canonical JSON, cut to 128 bits, so a relay that altered the configuration is caught by the phone.
- **Pairing.** Each end sends a fresh 16-byte salt; with a PIN, a SPAKE2 share on P-256 (RFC 9382) and a confirmation, whose transcript binds the join code and the configuration's hash. HKDF-SHA-256 then gives one AES-256-GCM key each way, and frames carry a 64-bit counter nonce that the receiver takes once and only in order. Five wrong PINs lock the screen.
- **Keys.** Messages are JSON in sealed frames of at most 1 024 bytes; the screen takes a key of its layout down and up, text, and the text box's special keys, acknowledges each, and releases a key held more than 15 seconds. Keepalives go every 25 seconds.
- **The QR as mosaics.** `qr::Matrix::mosaic_rows` writes a code as teletext mosaic rows, one sixel a module, dark on white with its quiet zone, so a channel can show its own QR on any receiver.

The cryptography was checked against qrkeyboard's own code: the configuration hashes, a SPAKE2 run with fixed scalars and sealed frames were computed by its `crypto.ts` under Node and are the crate's test fixtures. A screen and a phone written here pair through a running relay in the crate's `relay` tests, given `QRKEYBOARD_RELAY`.
