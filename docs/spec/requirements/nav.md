# F.10 Navigation and viewer functions (NAV)

**REQ-NAV-001 — Page number entry.**
The viewer shall be able to request a page by entering three digits. The first digit is the magazine, 1 to 8, and the other two are 0 to 9.
- While digits are being entered, the partial number shall be shown in the row 0 status area.
- Digits that cannot form a valid page number shall be rejected.
- Hexadecimal page numbers cannot be entered.
*Verification:* Test; demonstration.

**REQ-NAV-002 — Next and previous page.**
The viewer shall be able to step to the next or previous page number. Two behaviours shall be available, selected in the configuration:
- in numerical order;
- to the next or previous page actually present in the store.

Page 899 wraps to 100.
*Verification:* Test.

**REQ-NAV-003 — Links: the four coloured keys and Index.**
A page has at most four links, one for each coloured key, plus the Index key (decision E.36). The links come from either of two sources:
- **Pages from broadcast or T42 sources:** packet X/27/0. Links 1 to 4 are the red, green, yellow and cyan keys, and link 6 is the Index key.
- **Pages from TTI files and web services:** the link lines of the TTI Link Extension (Appendix E). The page encoder also turns these into packet X/27/0 (REQ-INP-019).

Pressing a coloured key requests that key's target. A null or absent link leaves the key without effect. A target with sub-code 3F7F requests any sub-page; any other sub-code requests that sub-page. When the page has no links at all, the coloured keys have no effect, and the Index key requests the initial page (REQ-SVC-004, Annex H.4). Link 5 of packet X/27/0 is used only for pre-fetching (REQ-NAV-004), never as a link the viewer can follow.
*Verification:* Test for each key, each source and each null case.

**REQ-NAV-011 — The receiver draws the link row.**
For pages whose links come from the TTI Link Extension, the receiver shall draw row 24 itself, from the link data. Any row 24 the page transmits shall be ignored. The row is divided into four equal prompts of 10 columns each, in the order red, green, yellow and cyan. Each prompt consists of:
- an alphanumeric colour attribute in the key's colour;
- then the link's label, of at most 9 characters.

A key with no link shows blank columns. A link to a page in another service shall begin its label with the receiver's mark for leaving the service: the right guillemet "»" from the G2 set. Its label then has at most 8 characters of the author's text.

The row shall be drawn in the receiver's fixed style:
- a black background;
- the key colour for text;
- no flash, conceal, double height or other attribute from the page.

Because the receiver draws the row, every link prompt is always shown whole (REQ-SEC-024). The engine draws it in one place, `neoteletext_formats::links::receiver_links`, for every port (decision E.108).
*Verification:* Test that page attempts to alter row 24 have no effect, and that the drawn row matches the link data exactly; tests of `receiver_links` for each kind of destination, a null link, a label's control codes and invalid links.
*Amended 23 October 2026 (decision E.108):* where the row is drawn, and its tests.

**REQ-NAV-012 — Following a link to another service.**
When a coloured key's link leads to a different service (REQ-WEB-015), and that origin has not been confirmed before in this session, the link shall not be followed at once. Instead:
- the receiver-owned status area shows that the receiver is asking (REQ-SEC-005);
- the receiver redraws row 24 to show the destination service's name as the descriptor gives it, and its origin, with the instruction to press the same key again to go.

Pressing the same key again follows the link and marks the origin as confirmed. Any other key cancels. Links within the same service, and the Index key, are followed at once.

This requirement applies to `neottx view` and to every port.
*Verification:* Test of confirmation, cancellation, and the already-confirmed case.

**REQ-NAV-004 — Pre-fetching linked pages.**
*Phase:* Phase 2 (whole requirement), because it depends on request mode (REQ-ACQ-006).
In request mode, when a page with Fastext links is displayed, the decoder should automatically place requests for all six linked pages, including link 5, in the spare acquisition slots. This lets the coloured keys respond at once, as the Fastext scheme intends (Annex H). On web services, pre-fetching shall fetch only from the service of the page being viewed (REQ-SEC-013).
*Verification:* Test.

**REQ-NAV-005 — Sub-pages.**
The viewer shall be able to:
- show whichever sub-page of the current page arrived most recently (rotating display, the default);
- step forward and backward through the stored sub-pages;
- enter a sub-page number directly.

The status area shall show the current sub-page number and the number of sub-pages stored.
*Verification:* Test; demonstration.

**REQ-NAV-006 — Hold.**
The viewer shall be able to hold the displayed page, so that it is not updated by new receptions until hold is released.
*Verification:* Test.

**REQ-NAV-007 — Reveal, mix and zoom.**
*Phase:* Phase 1 for latched reveal, mix and reveal clearing; Phase 2 for momentary reveal and page zoom.
The viewer shall be able to toggle reveal, toggle mix mode, and cycle page zoom through three states: top half, bottom half and off (REQ-L1-020).

Reveal shall work in either of two configurable styles:
- **Latched**, which is the default.
- **Momentary**, with a timeout of 160 milliseconds after the last request, like the SAA5050.

Reveal shall be cleared when a different page is selected.
*Verification:* Test.

**REQ-NAV-008 — Home.**
A Home function shall request the initial page (REQ-SVC-004).
*Verification:* Test.

**REQ-NAV-009 — Entering values into forms.**
On a page with an interactive form (REQ-WEB-010 and Appendix C), the viewer shall be able to move between fields, edit them and submit. Both input devices shall be supported.

**With a keyboard:**
- the Tab key and Shift-Tab move between fields;
- typing enters characters;
- the arrow keys move within a field and step through SELECT and RADIO choices;
- the space bar toggles a checkbox;
- the Enter key presses the focused button.

**With a remote control only** (digits, the four colour keys and a few others):

| Key | Action |
|---|---|
| Red | Previous field |
| Green | Next field |
| Yellow | Change the focused field. It steps SELECT and RADIO choices, toggles a checkbox, and switches TEXT entry between letters and digits. |
| Cyan | Presses the focused button, or submits the form from any field |
| Index | Leaves the form without submitting |

In letter mode, TEXT entry uses the digit keys with repeated presses, as on a telephone keypad ("2" gives A, B, C and so on). In digit mode, the digit keys enter digits directly.

While a form has focus, the digit keys shall not change the page. The status area shall show which mode is active.
*Verification:* Test of each key in both input methods; demonstration.


**REQ-NAV-010 — Back and forward history.**
The viewer shall keep a history of the pages it has displayed, like a browser. Each entry records:
- the service;
- the page number;
- the sub-page;
- whether the page came from a form response.

**Moving through history:**
- **Back** returns to the previous page. **Forward** undoes a Back.
- Selecting a new page by any means clears the forward history.
- A form submission's response page is recorded in the history. Back from it returns to the form, with the values already entered (REQ-WEB-011).

**Keys:** On a keyboard, Back and Forward are the Backspace key and Alt with the left and right arrows. On a remote control, which has no standard back key, they are two configurable keys. The default is to hold the Index key for Back.

**Other rules:**
- The history holds a configurable number of entries, 32 by default, in caller-provided memory (REQ-GEN-003).
- Returning to a page shows the stored copy at once, and checks it is current as REQ-WEB-007 describes.
- History is never saved between runs unless the port enables it, and it can be wiped, like the cookie store (REQ-SEC-013).

*Verification:* Test of back, forward, clearing and the form case.

**REQ-NAV-013 — Links to addresses, page numbers and runs of text.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-NAV-014 — The tuning screen.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.

**REQ-NAV-015 — Page catching and showing links.** Moved out of the engine on 16 October 2026 (decision E.76), and withdrawn on 18 October 2026: it concerned only the browser, which was removed from this repository and is maintained in the separate neoteletext-browser repository.
