# F.18 State machines (FSM)

EN 300 706 contains two different kinds of ordering rule.

**Rules for the broadcaster.** Most of the ordering rules tell the broadcaster how to transmit. Examples are:
- the packet order within a page (Annex B.2);
- the order of triplets (Annex A.4);
- the 20 millisecond gap after a header (Annex B.1);
- the limit of 100 milliseconds between packets of a page (Annex B.9).

Real transmissions and recordings do not always obey them. A receiver that depended on them would reject good data.

**Rules for the receiver.** A smaller set of rules binds the receiver:
- what starts and ends a page (clause 7.2.1);
- the erase bit;
- the page-completion timeout (decision E.19);
- the left-to-right processing of attributes in a row, with set-at and set-after timing (Table 26);
- the walk through Level 1.5 triplets (clause 12.3);
- at Levels 2.5 and 3.5, the walk through local enhancement data and objects (clauses 12.3 and 13), and the resolution of contending attributes (Annex C.2).

This area turns the receiver rules into explicit state machines: the six of REQ-FSM-002 to REQ-FSM-007, and the two of Levels 2.5 and 3.5 in REQ-FSM-011 and REQ-FSM-012. It requires every one of them to tolerate any breach of the broadcaster rules.

The tables below are the normative form of behaviour that areas SLC, ACQ, L1, L15, L25, OBJ and NAV describe in prose. If a table and a prose requirement disagree, that is a defect, to be corrected in both.

**REQ-FSM-001 — How the machines are implemented.**
Each state machine shall be implemented as an explicit enumeration of states and a single transition function. The transition function takes the current state and one event, and returns the next state together with the actions to perform. In Rust this is an `enum` of states and a `match` over the state and the event, with no wildcard arms, so that the compiler checks that every combination is handled. No state that affects a transition may be kept anywhere except in the machine's own state record. Every transition in the tables of this area shall exist in the code, and there shall be no transitions in the code that are not in the tables. Each machine shall be usable and testable on its own, without the rest of the decoder.
*Verification:* Inspection; the transition tests of REQ-FSM-009.

**REQ-FSM-002 — The line slicer machine.**
One run of this machine processes one sampled line (area SLC). It shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Start | Line received | Estimate black level (REQ-SLC-003) | Searching for run-in |
| Searching for run-in | Run-in pattern found within the search window (REQ-SLC-004) | Record its position | Measuring |
| Searching for run-in | Window exhausted | Count "no signal" | Rejected |
| Measuring | Levels, threshold and bit phase estimated | None | Gate check |
| Gate check | Amplitude and match quality at or above the minimums (REQ-SLC-005) | Hold the threshold for the rest of the line | Looking for framing code |
| Gate check | Below either minimum | Count "below gate" | Rejected |
| Looking for framing code | Exact framing code found within plus or minus 2 bit periods | None | Collecting bytes |
| Looking for framing code | Framing code with one wrong bit found, and single-bit tolerance enabled (decision E.24) | Mark "framing corrected" | Collecting bytes |
| Looking for framing code | Not found | Count "no framing code" | Rejected |
| Collecting bytes | 336 bits read at bit centres (REQ-SLC-007 and REQ-SLC-009) | Assemble 42 bytes | Checking address |
| Checking address | Both address bytes decode without uncorrectable error | Output packet with quality data (REQ-SLC-012) | Accepted |
| Checking address | Either address byte uncorrectable | Count "bad address". This check is what makes accepting a corrected framing code safe (decision E.24). | Rejected |

*Verification:* The transition tests of REQ-FSM-009, using synthetic lines from REQ-TST-060.

**REQ-FSM-003 — The magazine acquisition machine.**
There shall be one instance of this machine for each of the eight magazines (area ACQ). Header events shall be delivered as follows:
- **Serial mode:** every header is delivered to all eight instances, because in serial mode any header ends the page in progress (clause 7.2.1 and decision E.6).
- **Parallel mode:** a header is delivered only to the instance for its own magazine.

The mode is taken from bit C11 of each header as it arrives. Packets 29, 30 and 31 never reach this machine; they go straight to the magazine and service handlers (REQ-PKT-008).

The states are:
- **Idle:** no page is being received in this magazine.
- **Receiving P:** page P is being received and stored.
- **Absorbing:** a page that is not to be stored is being transmitted, such as a hexadecimal page or, in request mode, a page nobody asked for. Its rows must not be attached to anything else.

| State | Event | Action | Next state |
|---|---|---|---|
| Idle | Valid header for a storable page P in this magazine | Start P. If C4 is set, erase P's stored rows 1 to 28 (REQ-ACQ-004). | Receiving P |
| Idle | Valid header for a page not to be stored, in this magazine | None | Absorbing |
| Idle | Row packet 1 to 28 of this magazine | Discard; count "row without header" | Idle |
| Idle | Any other header | None | Idle |
| Receiving P | Row packet 1 to 28 of this magazine | Store in P (REQ-ACQ-007 and REQ-ACQ-008); mark the row changed | Receiving P |
| Receiving P | Row packet 1 to 28 of another magazine, in serial mode | Discard; count (Annex B.3) | Receiving P |
| Receiving P | Valid header for a storable page Q in this magazine | End P and raise "page completed"; start Q, erasing it if C4 is set | Receiving Q |
| Receiving P | Valid header for a page not to be stored, in this magazine | End P and raise "page completed" | Absorbing |
| Receiving P | Header of another magazine, in serial mode | End P and raise "page completed" | Idle |
| Receiving P | Time-filling header (page FF) in this magazine, or, in serial mode, in any magazine | End P and raise "page completed" | Idle |
| Receiving P | Header of this magazine with a valid address but uncorrectable page or control bytes (decision E.7) | End P and raise "page completed" | Idle |
| Receiving P | No packet for P for the completion timeout, 100 milliseconds by default (decision E.19) | Raise "page completed" | Idle |
| Absorbing | Row packet 1 to 28 of this magazine | Discard silently; this is not an error | Absorbing |
| Absorbing | Any header that would end a page, following the same rules as in Receiving P | As for Receiving P, but with no "page completed" event | The same next state as for Receiving P |
| Absorbing | Completion timeout | None | Idle |

Row packets are stored whatever order they arrive in. The machine shall not require packets 26, 27 and 28 to come before the rows, nor designation codes to ascend, nor rows to ascend (REQ-FSM-008).
*Verification:* The transition tests of REQ-FSM-009.

**REQ-FSM-004 — The page lifecycle machine.**
There shall be one instance of this machine for each stored page and sub-page (areas ACQ and NAV). It is driven by the events of the acquisition machine and by time. It shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Empty | Reception started without C4 | Raise "page started" | Partial |
| Empty | Reception started with C4 | Raise "page started" | Partial |
| Partial | Row stored | Raise "page updated" for that row | Partial |
| Partial | Reception ended | Raise "page completed" | Complete |
| Complete | Reception started without C4 | Merge the new rows under decision E.8; raise "page started" | Partial |
| Complete | Reception started with C4 | Erase rows 1 to 28; raise "page erased" | Partial |
| Complete | Header with C8 received, or content changed | Raise "update indicator" (REQ-ACQ-010) | Unchanged |
| Complete | Not received for the configured "recently received" period | Raise "page stale" | Stale |
| Stale | Reception started, with or without C4 | As for Complete | Partial |
| Any | Evicted from the cache (REQ-ACQ-005) | Release storage | Empty |

*Verification:* The transition tests of REQ-FSM-009.

**REQ-FSM-005 — The viewer machine.**
There shall be one instance of this machine for each viewer (area NAV). Reveal, mix and zoom are part of the viewer state that the renderer reads (REQ-L1-020 and REQ-NAV-007). They do not change the machine's state. The machine shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Starting | Decoder started | Request the initial page (REQ-SVC-004) | Searching |
| Any state except Holding | First digit 1 to 8 | Show it in the status area | Entering 1 digit |
| Entering 1 digit | Digit 0 to 9 | Show it | Entering 2 digits |
| Entering 2 digits | Digit 0 to 9 | Request the entered page | Searching |
| Entering 1 or 2 digits | Digit that cannot make a valid page | Reject; keep the digits entered so far | Unchanged |
| Searching | Requested page present in the store | Show it; set the row 0 marker to white (decision E.10) | Displaying |
| Searching | Header received | Update the rolling header and clock (decision E.11) | Searching |
| Displaying | Page updated, or flash or reveal changed | Mark the affected rows changed (REQ-RND-019) | Displaying |
| Displaying | Page stale | Show "not recently received" in the status area | Displaying |
| Displaying | Hold key | Stop applying updates | Holding |
| Holding | Hold key | Apply updates received meanwhile | Displaying |
| Displaying or Searching | Coloured key with a link in the same service, Index, Home, next or previous page | Request the target page (REQ-NAV-002, REQ-NAV-003 and REQ-NAV-008) | Searching |
| Displaying | Coloured key whose link leads to an unconfirmed other service | Show the confirmation in the status area and row 24 (REQ-NAV-012) | Confirming link |
| Confirming link | The same key again | Mark the origin confirmed; request the target | Searching |
| Confirming link | Any other key | Restore row 24 | Displaying |
| Displaying | Next or previous sub-page | Show the neighbouring stored sub-page (REQ-NAV-005) | Displaying |
| Displaying or Searching | Back or Forward, with an entry in that direction | Move in the history and request that page (REQ-NAV-010) | Searching |
| Displaying or Searching | Back or Forward, with no entry in that direction | None | Unchanged |

When a request is made and the page is already in the store, the machine passes through Searching straight to Displaying, and the found page is shown at once.
*Verification:* The transition tests of REQ-FSM-009.

**REQ-FSM-006 — The row attribute machine: the per-cell step.**
The Level 1 decoding of each row (area L1) shall be implemented as a state record, updated one cell at a time from column 0 to column 39. The record holds:
- foreground colour;
- alphanumeric or mosaic mode;
- flash;
- conceal;
- box;
- size;
- contiguous or separated;
- hold;
- the held mosaic character and its form;
- which G0 set is in use.

It starts from the defaults of REQ-L1-002. For each cell, the machine shall perform these steps in this order:
1. **Set-at attributes.** If the byte is a set-at attribute, apply it. The set-at attributes are:
   - Steady;
   - Normal Size, which resets the held mosaic if the size changes;
   - Conceal;
   - Contiguous;
   - Separated;
   - Black Background;
   - New Background;
   - Hold Mosaics.
2. **Choose what the cell shows.**
   - An alphanumeric character: from the current G0 set, with national option substitution.
   - A mosaic character: from the G1 set, or the G0 character for blast-through codes.
   - An attribute: a space, or the held mosaic when hold is on and the row is in mosaic mode.
3. **Record the cell** with the current colours, flash, conceal, box and size (REQ-L1-001).
4. **Update the held mosaic.** If the byte was a mosaic character with bit 6 set, it becomes the held mosaic, stored with its contiguous or separated form.
5. **Set-after attributes.** If the byte is a set-after attribute, apply it. The set-after attributes are:
   - the alphanumeric and mosaic colours. A change of mode resets the held mosaic, and any colour ends conceal.
   - Flash;
   - End Box;
   - Start Box. Under decision E.17 it takes effect only when the next cell also holds Start Box, so this step needs to look one byte ahead;
   - Double Height, which resets the held mosaic if the size changes and marks the row as containing double height;
   - ESC;
   - Release Mosaics.

The handling of the row after a double-height row (REQ-L1-008) happens before this machine runs: that row is not processed from its own bytes at all.
*Reason:* The attributes have too many combinations to list as named states. The order of the five steps is exactly what Table 26 and decision E.16 define, and it is where attribute bugs are usually found.
*Verification:* The transition tests of REQ-FSM-009, and the attribute tests of REQ-TST-013.

**REQ-FSM-007 — The Level 1.5 triplet walk.**
One run of this machine applies a page's enhancement packets to its cell grid (area L15). It shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Start | Walk begins | Active position is row 0, column 0 (REQ-L15-003) | Walking |
| Walking | Next stored X/26 packet, in designation-code order. Missing codes are skipped. X/26/15 is skipped when the profile says so (decision E.2). | Take its triplets in order | Walking |
| Walking | Triplet that failed its Hamming 24/18 check | Ignore (decision E.9) | Walking |
| Walking | Row triplet with address 63 and mode 11111 (Termination Marker) | Stop | Terminated |
| Walking | Row triplet with address 63 and mode 00111 | Active position becomes row 0, column 0 | Walking |
| Walking | Any other row triplet that sets the row under decision E.3 | Row becomes 24 for address 40, or the address minus 40 otherwise; column becomes 0 | Walking |
| Walking | Other row triplet | None | Walking |
| Walking | Column triplet with mode 00100, 00101, 01010 or 00110 | None; the active position is unchanged (REQ-L15-005) | Walking |
| Walking | Column triplet with a character mode (00010, 01111, or 10000 to 11111) | Column becomes the address. If the data is 0x20 or above, write the character under decision E.5 (REQ-L15-006 and REQ-L15-008). | Walking |
| Walking | Any other column triplet | Column becomes the address | Walking |
| Walking | No more packets | None | Terminated |

*Verification:* The transition tests of REQ-FSM-009, and the Level 1.5 tests of REQ-TST-014.

**REQ-FSM-008 — Tolerance of the broadcaster's ordering rules.**
None of the machines shall depend on the ordering rules that EN 300 706 places on the broadcaster. In particular:
- packets 26, 27 and 28 may arrive after, or among, the rows of their page;
- designation codes may arrive in any order, or with gaps;
- rows may arrive in any order;
- the gap after a header may be shorter than 20 milliseconds;
- triplets may address positions out of screen order (REQ-L15-009 and REQ-L25-005);
- magazines may be interleaved in any pattern that parallel mode allows;
- the Magazine Organization Table and object pages may arrive before or after the pages that use them, and in any order among themselves (Annex B.7 asks for an order the receiver does not rely on).

A page received in any such order shall give the same stored page and the same cell grid as the same packets in standard order, once all of them have arrived (REQ-ACQ-022). The only exceptions are cases where the standard itself makes order significant: which header starts and ends a page, which of two overwrites of the same cell comes later (decision E.5 (h)), and which of two contending attributes comes later in transmission (Annex C.2 rule 3).
*Verification:* The reordering tests of REQ-FSM-009.

**REQ-FSM-009 — Tests for every transition.**
The test suite shall contain at least one test for every row of every table in this area, and for each of the five steps of REQ-FSM-006. Each test shall check both the next state and the actions performed.

In addition, reordering tests shall take pages from the encoder and from the real-broadcast corpus (REQ-TST-022) and feed them to the decoder in each non-standard order listed in REQ-FSM-008. The resulting pages and cell grids shall be checked to be identical to those from standard order.
*Verification:* Test; the traceability table of REQ-DOC-003 shall link each table row to its test.

**REQ-FSM-010 — Transition trace.**
Each machine shall be able to report every transition it makes, as a trace entry containing:
- the machine and instance, for example "magazine 3";
- the time;
- the state before;
- the event;
- the actions;
- the state after.

Tracing shall be switched on and off in the configuration. When it is off, it shall cost nothing beyond a single test per transition. The command-line program's verbosity option (REQ-CLI-002) shall be able to print the trace.
*Reason:* A trace of transitions is the most effective way to find out why a real recording decodes wrongly.
*Verification:* Test that a known input produces the expected trace.

**REQ-FSM-011 — The Level 2.5 and 3.5 enhancement walk.**
One run of this machine walks a page's local enhancement data, the objects it invokes and the default objects, and produces the character and attribute events that REQ-FSM-012 resolves into the grid (areas L25 and OBJ). The Level 1.5 walk of REQ-FSM-007 is unchanged and is used at Level 1.5 only.

The machine's state record holds the current source, which is the local enhancement data, an object or a default object, and a stack of walk records, one for each invoking source. A walk record holds:
- the active position, relative to the source's origin;
- the origin;
- any pending Origin Modifier;
- the source's attribute plane and display priority.

The stack is at most three records deep, because of the nesting rules of REQ-OBJ-008, so no recursion is needed (REQ-GEN-016). The machine shall follow this table:

| State | Event | Action | Next state |
|---|---|---|---|
| Start | Walk begins and the page has a packet X/26 with a display triplet | The source is the local enhancement data. Active position row 0, column 0; no Origin Modifier (REQ-L25-003). | Walking |
| Start | Walk begins and the page has no packet X/26 with a display triplet | None | Placing default objects |
| Walking | Next triplet of the source: for the local enhancement data, from the next stored X/26 in designation-code order, skipping missing codes; for an object, the next triplet of its body (REQ-OBJ-006) | Take it | Walking |
| Walking | Triplet that failed its Hamming 24/18 check | Skip it (decision E.46) | Walking |
| Walking | Termination Marker (address 63, mode 11111), or the end of the source's data | End the source | Returning |
| Walking | Object Definition triplet | End the source (decision E.46) | Returning |
| Walking | Full Row Colour, Set Active Position, or Address Display Row 0 with address 63 | Move the active position (REQ-L25-003). For Full Row Colour and Address Display Row 0, emit a row colour event (REQ-L25-033), unless the source is an Adaptive or Passive object or lies in a side panel (REQ-OBJ-013) | Walking |
| Walking | Full Screen Colour | Emit a screen colour event (REQ-L25-032), unless REQ-OBJ-013 forbids it | Walking |
| Walking | Origin Modifier | Record the offsets (REQ-OBJ-002) | Modifier pending |
| Modifier pending | Object Invocation | Handle it as an invocation from Walking, with the recorded offsets | As for an invocation from Walking |
| Modifier pending | Any other triplet | Discard the offsets (decision E.46), then handle the triplet as in Walking | As for that triplet in Walking |
| Walking | Object Invocation that REQ-OBJ-008 permits, within the limits of REQ-OBJ-020, whose definition is found and accepted (REQ-OBJ-003 to REQ-OBJ-006) | Push the walk record. The new source is the object: origin as REQ-OBJ-002 gives; active position row 0, column 0; the attribute plane of its type (REQ-OBJ-009 to REQ-OBJ-011) | Walking |
| Walking | Any other Object Invocation | Skip it and count it | Walking |
| Walking | Any other row triplet: a reserved mode, DRCS Mode (decision E.95), or a Programme Delivery Control mode | None (REQ-L25-052) | Walking |
| Walking | Column triplet of a reserved mode, or of mode 00110 | None; the active position is unchanged (REQ-L25-004) | Walking |
| Walking | Column triplet of mode 01101, DRCS Character Invocation, or 01110, Font Style | Column becomes the address; nothing is emitted (decisions E.95 and E.96) | Walking |
| Walking | Column triplet of a character mode (REQ-L25-010) | Column becomes the address. If the data is 2/0 or above, emit a character event at the origin plus the active position, with the source's priority, unless REQ-OBJ-014 or REQ-L25-050 discards it | Walking |
| Walking | Column triplet of an attribute mode: 00000, 00011, 00111, 01000 or 01100 | Column becomes the address. Emit an attribute event at the origin plus the active position, unless REQ-OBJ-014 or REQ-L25-050 discards it, or the level ignores the mode (REQ-L25-001) | Walking |
| Returning | The ended source is an object | Pop the walk record: restore the invoker's active position and origin; cancel the Origin Modifier (REQ-OBJ-002) | Walking |
| Returning | The ended source is the local enhancement data | None | Finished |
| Placing default objects | Next default object of REQ-OBJ-040, in order, that is found and accepted | The source is the default object, with origin and active position row 0, column 0 | Walking |
| Returning | The ended source is a default object | Active position row 0, column 0 | Placing default objects |
| Placing default objects | No more default objects | None | Finished |

*Amended 20 October 2026 (decision E.95):* DRCS is not decoded, and the parts of this requirement that concerned it are removed.
*Amended 20 October 2026 (decision E.96):* the Level 3.5 font styles are not drawn, and the parts of this requirement that concerned them are removed.

*Verification:* The transition tests of REQ-FSM-009, and the tests of REQ-TST-015 and REQ-TST-017.

**REQ-FSM-012 — Row resolution at Levels 2.5 and 3.5: the per-cell step.**
At Levels 2.5 and 3.5, each row shall be resolved from its Level 1 bytes and from the events of REQ-FSM-011 that fall on it. This extends the row attribute machine of REQ-FSM-006 (decision E.47).

The state record of REQ-FSM-006 is kept for the underlying plane, which is the Level 1 page modified by the local enhancement data and Active objects. It gains these fields:
- the colours as Colour Map indices;
- underline or separated mosaics, set by a non-spacing attribute;
- invert;
- the flash mode, rate and phase;
- conceal, with whether it was set by a non-spacing attribute;
- window;
- the horizontal part of the size;
- the current G0 and G2 designation of the enhancement data.

Each Adaptive object on the row has an overlay record, holding the attribute types it has changed. Each Passive object has its own record, starting from the defaults of REQ-OBJ-011.

The row starts from the start-of-row defaults, re-mapped as REQ-L25-022 describes. For each cell, from column 0 to column 39, and then separately for the cells of the right panel and of the left panel, which start from the defaults of REQ-L25-041 and have no Level 1 bytes, the machine shall perform these steps in this order:
1. **Set-after attributes from the previous cell.** Apply the set-after attributes recorded by step 7 of the previous cell, as REQ-FSM-006 step 5 does.
2. **Set-at Level 1 attributes.** If the Level 1 byte is a set-at attribute, apply it, as REQ-FSM-006 step 1 does.
3. **Non-spacing attributes of the underlying plane.** Apply each attribute event of the local enhancement data and of Active objects placed at this cell, in the order of the walk. These win against step 2 (REQ-L25-020).
4. **Adaptive overlays.** For each Adaptive object whose range on this row, from its first to its rightmost addressed cell, includes this cell: start its overlay, at its first cell, from the underlying record; apply its attribute events at this cell; and, for each attribute type it has changed, use its value in place of the underlying one (REQ-OBJ-010). Outside its range the overlay has no effect.
5. **Passive records.** Apply the attribute events of each Passive object placed at this cell to that object's own record (REQ-OBJ-011).
6. **Choose what the cell shows.** Take the character of the highest display priority at this cell (REQ-OBJ-012): Passive, then Adaptive, then local enhancement data, then Active, then the Level 1 character chosen as REQ-FSM-006 step 2 does. Take the attributes from the Passive object's record when a Passive object supplied the character. Otherwise take them from the Adaptive overlay when the cell is in an Adaptive object's range, whether the character is the object's or the Level 1 page's; where two ranges overlap, the later invoked object's overlay applies. Otherwise take them from the underlying record. Apply the size rules of REQ-L25-025.
7. **Record the cell and finish.** Record the cell (REQ-L25-006). Update the held mosaic from Level 1 characters only, as REQ-FSM-006 step 4 does (decision E.50). Record the set-after attributes of the Level 1 byte for step 1 of the next cell.

The row after a row with spacing double height or double size is handled as REQ-L25-026 describes before this machine runs: its Level 1 bytes are not used, and only enhancement events are resolved on it.
*Reason:* Annex C.2 orders contending attributes by where they take effect, then where they are placed, then transmission order. Steps 1 to 3 apply exactly that order to the underlying plane. Steps 4 to 6 add the planes of clause 13.2, which reproduce the worked example of clause 13.9.
*Amended 20 October 2026 (decision E.96):* the Level 3.5 font styles are not drawn, and the parts of this requirement that concerned them are removed.
*Verification:* The transition tests of REQ-FSM-009, and the worked examples of REQ-TST-016.
