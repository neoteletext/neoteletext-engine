# F.23 Input: keys sent back to a channel (INT)

This area is the input profile (decisions E.90 and E.91): a receiver sends the viewer's keys to a channel over the channel's WebSocket, as a terminal sends keys to its host, and the channel answers with pushed rows and an optional cursor record. The design is `docs/spec/design/input.md`. The records are built in `neoteletext-input` (`no_std`, no allocation).

**REQ-INT-001 — The key record.**
*Phase:* Phase 2 (the whole area).
A key record shall be 14 bytes, then, for kind 3 only, 1 to 32 bytes of UTF-8 text: the version (1), the kind (1 to 7), a sequence number counting the connection's records from 1 and wrapping, the sender's clock in milliseconds from any start and wrapping, and three 16-bit fields `a`, `b` and `c`, all big-endian. A field the kind does not use shall be 0. A record shall be refused whole on a wrong length, an unknown version or kind, or any field outside its range.
*Verification:* Tests of every kind's round trip, of the 46-byte bound and of each refusal; the fuzz target `key_records`.

**REQ-INT-002 — Kinds and fields.**
Kinds 1 and 2 shall carry a key code in `a`, modifiers in `b` (1 Shift, 2 Control, 4 Alt, 16 a repeat, which a key-up never has; no others) and a physical key in `c`. A key code shall be a printable Unicode scalar value of the Basic Multilingual Plane outside 0xF000 to 0xF0FF, or a named key of REQ-INT-003. Kind 3 shall carry text with no control character. Kind 4 shall carry a page from 0x100 to 0x8FF, or 0 for none; kind 5 a column of 0 to 39 and a row of 0 to 24; kind 6, 0 or 1; kind 7, the last sequence number sent on the connection before.
*Verification:* Tests of every field's bounds.

**REQ-INT-003 — The key tables.**
The named keys shall be the W3C `KeyboardEvent.key` values in one numbered table, a key code being 0xF000 plus a name's place, and the physical keys the W3C `KeyboardEvent.code` values in another, 0 meaning unknown. A name shall only ever be added at the end of its table.
*Verification:* A test pins the places of names in each table; every name round-trips.

**REQ-INT-004 — The rate.**
A sender shall send at most 120 records in any second, and shall never drop a key-up to keep to it.
*Verification:* A test of the 121st record in a second and of a key-up beyond it, across the clock's wrap.

**REQ-INT-005 — The cursor record.**
A cursor record shall be a record of the audio profile's frame (REQ-AUD-003) of type 4, with an 8-byte body: the page it is for (0x100 to 0x8FF), the last key record acted on, a row of 1 to 23 or 255 for no cursor, a column of 0 to 39, an echo width of 0 to 40 that does not run past column 39, and flags (1 echo, 2 masked, 4 block; no others). Echo shall need a width, masked shall need echo, and no cursor shall have column, width and flags 0. It shall travel as one fragment on packet 4/31, data channel 12 (REQ-AUD-007), and the audio receiver shall step over it.
*Verification:* Tests of the round trip through a packet, of every field's bounds, of a sound's record on the same channel, and of the audio receiver meeting a cursor record while a sound is in flight; the fuzz target `cursor_records`.

**REQ-INT-006 — The cursor in a TTI page.**
A TTI page may carry its cursor record as a line `CU,row,column,width,flags[,acked]`, decimal, for the page that holds it. The reader shall keep the last such line and the writer shall write it back; a line that cannot be read is reported and dropped.
*Verification:* Tests of reading, writing back and refusing the line.

**REQ-INT-007 — A phone as the keyboard.**
`neoteletext-qrkeyboard` shall be the screen's side of qrkeyboard's protocol 3: it shall compute the configuration's hash as qrkeyboard's canonical JSON does, put the code, the pairing key (no-PIN mode) and the hash only in the QR address's fragment, pair by salts and, with a PIN, SPAKE2 on P-256 bound to the code and the hash, seal frames with AES-256-GCM under a counter nonce taken once and in order, lock after five wrong PINs, accept only the keys of its layout, release a key held past 15 seconds, refuse a relay WebSocket on another host, and bound every frame to 1 024 bytes and every response body to 64 KB.
*Verification:* Fixtures computed by qrkeyboard's own `crypto.ts`: two configuration hashes, a SPAKE2 run and sealed frames; tests of replay, reordering and forgery; pairing by key and by PIN through a running relay.

**REQ-INT-008 — A key goes at once.**
A fetcher process shall send a key record the moment it arrives and pass on what the channel answers the moment it arrives, sleeping only until a request, a socket or a timer of its state machine wakes it (decision E.97). The viewer shall take what the fetcher passes on the moment it arrives, waiting for the terminal and its source together.
*Amended 21 October 2026:* the viewer too; it looked at its source only between 40 ms waits for the terminal.
*Verification:* A test types 20 keys through the fetcher to a local channel that answers each with a page, and holds the round trip's median to 2 ms; tests that the fetcher's pipe reader wakes the viewer for each frame, and that a wake ends the viewer's wait at once.
