# F.23 Input: keys sent back to a channel (INT)

This area is the input system: a receiver sends the viewer's input to a channel over the channel's push WebSocket, as a terminal sends keys to its host, and the channel answers with pushed rows and records that place a cursor, acknowledge input and ask for input modes. The design and the normative text of REQ-INT-101 to REQ-INT-119 are `neoteletext-common/INPUT.md` (protocol version 2, adopted on 21 October 2026, decision E.100), which replaced `docs/spec/design/input.md` and decision E.90's key record. Requirements REQ-INT-130 to REQ-INT-139 of that document are the browser's. Everything is built in `neoteletext-input` (`no_std`, no allocation).

**REQ-INT-001 — Withdrawn (the key record).**
*Phase:* Phase 2 (the whole area).
Withdrawn on 21 October 2026 (decision E.100): replaced by the input stream of REQ-INT-101 to REQ-INT-104.

**REQ-INT-002 — Withdrawn (kinds and fields).**
Withdrawn on 21 October 2026 (decision E.100): replaced by REQ-INT-103 and REQ-INT-104.

**REQ-INT-003 — Withdrawn (the key tables).**
Withdrawn on 21 October 2026 (decision E.100): replaced by the functional codes of REQ-INT-105; the table of W3C key names stays, and the table of physical keys gave way to KEY's `base`.

**REQ-INT-004 — Withdrawn (the rate).**
Withdrawn on 21 October 2026 (decision E.100): replaced by REQ-INT-109, which counts input units rather than records.

**REQ-INT-005 — The cursor record.**
A cursor record shall be a record of the audio profile's frame (REQ-AUD-003) of type 4, with an 8-byte body: the page it is for (0x100 to 0x8FF), the last key record acted on, a row of 1 to 23 or 255 for no cursor, a column of 0 to 39, an echo width of 0 to 40 that does not run past column 39, and flags (1 echo, 2 masked, 4 block; no others). Echo shall need a width, masked shall need echo, and no cursor shall have column, width and flags 0. It shall travel as one fragment on packet 4/31, data channel 12 (REQ-AUD-007), and the audio receiver shall step over it.
*Amended 21 October 2026 (decision E.100):* the acknowledgement is the input count acted on (REQ-INT-110), no longer the last key record's sequence number.
*Verification:* Tests of the round trip through a packet, of every field's bounds, of a sound's record on the same channel, and of the audio receiver meeting a cursor record while a sound is in flight; the fuzz target `cursor_records`.

**REQ-INT-006 — The cursor in a TTI page.**
A TTI page may carry its cursor record as a line `CU,row,column,width,flags[,acked]`, decimal, for the page that holds it. The reader shall keep the last such line and the writer shall write it back; a line that cannot be read is reported and dropped.
*Verification:* Tests of reading, writing back and refusing the line.

**REQ-INT-007 — A phone as the keyboard.**
`neoteletext-qrkeyboard` shall be the screen's side of qrkeyboard's protocol 3: it shall compute the configuration's hash as qrkeyboard's canonical JSON does, put the code, the pairing key (no-PIN mode) and the hash only in the QR address's fragment, pair by salts and, with a PIN, SPAKE2 on P-256 bound to the code and the hash, seal frames with AES-256-GCM under a counter nonce taken once and in order, lock after five wrong PINs, accept only the keys of its layout, release a key held past 15 seconds, refuse a relay WebSocket on another host, and bound every frame to 1 024 bytes and every response body to 64 KB.
*Verification:* Fixtures computed by qrkeyboard's own `crypto.ts`: two configuration hashes, a SPAKE2 run and sealed frames; tests of replay, reordering and forgery; pairing by key and by PIN through a running relay.

**REQ-INT-008 — A key goes at once.**
A fetcher process shall send an input message the moment it arrives and pass on what the channel answers the moment it arrives, sleeping only until a request, a socket or a timer of its state machine wakes it (decision E.97). The viewer shall take what the fetcher passes on the moment it arrives, waiting for the terminal and its source together.
*Amended 21 October 2026:* the viewer too; it looked at its source only between 40 ms waits for the terminal.
*Verification:* A test types 20 keys through the fetcher to a local channel that answers each with a page, and holds the round trip's median to 2 ms; tests that the fetcher's pipe reader wakes the viewer for each frame, and that a wake ends the viewer's wait at once.

**REQ-INT-009 — The phone keyboard's wait ends at once.**
The qrkeyboard screen of `neoteletext-qrkeyboard` shall wait only on its relay socket, on a waker that another thread may hold, and on its own next deadline (the keepalive, a reconnection, a held key's release), never by sleeping a fixed interval and looking again (decision E.106). A wake shall end the wait at once, so that a port's "finish" or another socket's bytes are acted on the moment they come, and bytes from the relay shall be acted on the moment they arrive.
*Verification:* Tests against a relay played in the test: a waker ends a wait within 40 ms, the relay's message is reported within 40 ms of being sent, and a wait nothing ends lasts its limit and reports nothing; the pairing tests through a running relay, plain and over TLS.

**REQ-INT-101 — The input stream.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-102 — Encoding.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-103 — Events.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-104 — Keys.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-105 — Functional codes.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-106 — Negotiation.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-107 — Modes.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-108 — Held input.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-109 — The rate.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-110 — The cursor record and the input count.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-111 — The control record.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-112 — Immediacy.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-113 — A host goes to a page.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-114 — A page's own modes.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-115 — The mouse only on request.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.

**REQ-INT-116 — International keyboards.**
As `neoteletext-common/INPUT.md` section 12, which holds its text, its reason and its verification.
