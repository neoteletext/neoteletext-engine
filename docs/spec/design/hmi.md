# Human-machine interfaces across machines: source-level design

*Note of 18 October 2026:* the windowed browser was removed from this repository and is maintained in the separate neoteletext-browser repository (decision E.76). What this design says of the browser, its tuning screen and its address bar applies there; the rest stays the engine's design.

A proposal, not a requirement. It describes how the code should take input from the very different controls of the machines neoteletext targets (REQ-GEN-015), and what that means for each crate and port. Nothing here changes REQUIREMENTS.md until it is recorded as decisions in Part E. Adding any of the third-party crates named in section 5 is a library choice and needs the owner's approval first.

Related documents:

- `docs/web-native-concept.md` of the neoteletext-browser repository section 7: the desktop keyboard mapping for the web browser concept;
- REQUIREMENTS.md area NAV: REQ-NAV-001 to REQ-NAV-012, especially REQ-NAV-009 (forms from a keyboard or a remote) and REQ-NAV-010 (Back and Forward);
- `ARCHITECTURE.md` layers L6 (Interaction) and L8 (Platform interface);
- `docs/spec/design/porting.md`.

## 1. What the code does today

| Piece | Where | What it does |
|---|---|---|
| `viewer::Key` | `crates/neoteletext-core/src/viewer.rs:20` | The page functions: digits, the four colours, Index, Home, next and previous page and sub-page, Reveal, Hold, Mix, Zoom, Back, Forward. |
| `forms::FormKey` | `crates/neoteletext-core/src/forms/editing.rs:13` | A second enum for forms, mixing keyboard keys (`Tab`, `Char`, `Backspace`) with remote keys (`RemoteDigit`, the colours). |
| `forms::MULTITAP_US` | `crates/neoteletext-core/src/forms.rs:33` | Telephone-style letter entry from digits, for remotes (REQ-NAV-009). |
| `port::Keys` | `crates/neoteletext-core/src/port.rs` | The port's key trait: `poll() -> Option<Key>`. Nothing implements it yet. |
| `map_key`, `map_form_key` | `crates/neoteletext-cli/src/view.rs` | `neottx`'s crossterm bindings. |
| The modal loop | `crates/neoteletext-cli/src/view.rs`, the event loop | `neottx` decides which consumer gets a key, in this order: Ctrl+C quits; a pending form submission takes Enter or F4 as yes; a focused form gets form keys; `q` or Esc quits; Tab focuses the form; otherwise the viewer. |

Five problems follow for anyone writing a second port.

1. **The routing lives in the command-line program.** Every port must rewrite the modal loop, including the confirmation before a form is sent (REQ-SEC-009), which is security behaviour. A port that gets the order wrong can let page-number digits reach a form, or let any key confirm a submission.
2. **Two key types for one keypad.** `Key` and `FormKey` overlap (digits, colours), and the port must know which mode the core is in to choose between them.
3. **No press, repeat or release.** `Keys::poll` returns a bare key. REQ-NAV-010's default for remotes, *hold Index for Back*, cannot be expressed, and no code implements it. Momentary reveal (REQ-NAV-007) depends on auto-repeat arriving as repeated presses, which is true of IR remotes and terminals but not of every device.
4. **Bindings are code.** The physical-key tables are `match` arms in the command-line program, so a remote control, a 4 by 4 keypad or a game controller each need new code rather than a new table.
5. **The core cannot adapt its prompts.** It does not know whether the port has digit keys, an OK key, a keyboard or a pointer, so the status area and row 24 cannot say "press OK" or "press the same key again" as appropriate.

## 2. The proposed structure

Input passes through four stages. Only the first is per machine.

```
 device driver         binding table          gestures               router
 (port, L9)            (data, L6)             (core, L6)             (core, L6)
 raw event   ──────►   Control + Phase  ──►   Function + Press   ──► Viewer / Form / Address /
 evdev, HID,           per device class       Short / Long / Repeat   Confirm / Catch consumer
 crossterm, DOM…       const, replaceable     uses port's Clock       ──► Action to the port
```

### 2.1 Controls: what the port reports (L8)

```rust
// neoteletext-core/src/port.rs: replaces `Keys::poll -> Option<Key>`.

/// One physical control, already named by the port's driver. A port
/// reports what was pressed, never what it means.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    Digit(u8),
    /// The four coloured keys in their Fastext order. The fourth is cyan on
    /// screen and usually blue on remotes.
    Colour(Colour),
    Up, Down, Left, Right,
    Ok,
    Back,
    Home, End, PageUp, PageDown, Insert, Delete,
    /// Function keys beyond F1 to F4, which the binding table names.
    Function(u8),
    /// A dedicated remote-control key: Text, Index, Reveal, Hold, Mix, Size…
    Remote(RemoteKey),
    /// A printable character from a keyboard or an on-screen keyboard.
    Char(char),
    Backspace, Tab, BackTab, Escape, Enter,
    /// A pointer or touch on a cell of the grid (row, column).
    Cell(u8, u8),
    /// One step of a rotary encoder, +1 or -1.
    Detent(i8),
    /// A port-defined button number, for GPIO buttons and game controllers.
    Button(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase { Press, Repeat, Release }

/// Where the event came from. Pasted text and synthetic events can never
/// confirm anything (section 3.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin { Physical, Paste, Synthetic }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InputEvent {
    pub control: Control,
    pub phase: Phase,
    pub origin: Origin,
    pub at: Instant,
}

pub trait Input {
    /// The next input event, if any.
    fn poll(&mut self) -> Option<InputEvent>;
    /// What this port's controls can do (section 2.4).
    fn capabilities(&self) -> Capabilities;
}
```

`Control` is deliberately physical. `Ok`, `Enter` and a game controller's A button stay distinct here; the binding table decides they all mean Confirm.

### 2.2 Functions: what the core understands (L6)

One enum replaces `viewer::Key` and `forms::FormKey`:

```rust
// neoteletext-core/src/hmi.rs (new, layer L6 "input", per ARCHITECTURE.md).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Function {
    // Page functions: today's viewer::Key.
    Digit(u8), Red, Green, Yellow, Cyan, Index, Home,
    NextPage, PreviousPage, NextSubPage, PreviousSubPage,
    Reveal, Hold, Mix, Zoom, Back, Forward,
    // Added for the browser concept and for small devices.
    SubPageSeparator, Confirm, Cancel, Address, Refresh,
    NextLink, PreviousLink, FollowLink, Menu, Quit,
    // Text and field movement, today's FormKey.
    Char(char), DeleteBack, NextField, PreviousField,
    CursorLeft, CursorRight, ChoiceUp, ChoiceDown, Toggle,
}
```

The router (section 2.5) passes `Function`s to the viewer and the form model, which keep their logic. `viewer::Viewer::key` and `forms::FormModel::key` take `Function` instead of their own enums. `FormKey::RemoteDigit` disappears: the router knows whether the field is in letter mode and turns `Digit` into multi-tap letters itself.

### 2.3 Binding tables: data, not code

```rust
/// One binding: a control, the gesture on it, the function, and the modes
/// in which it applies.
pub struct Binding {
    pub control: Control,
    pub gesture: Gesture,        // Short, Long, Repeat, Any
    pub function: Function,
    pub modes: ModeSet,          // Browse | Entry | Form | Address | Confirm | Catch
}

pub struct BindingTable(pub &'static [Binding]);
```

- **Default tables are `const` data in the core**, one per device class: keyboard, IR remote, 4 by 4 keypad, three buttons, rotary encoder, game controller, touch. A microcontroller port keeps its table in flash with no allocation.
- **A port can supply its own table**, or start from a default and override entries. Ports for a particular remote can learn its codes into a table.
- **Each default table is tested for completeness.** Every essential function (section 4) must be reachable in every mode, from that class alone. This is a unit test per table, not a review checklist.
- Like glyphs, tables are replaceable data (REQ-RND-020's principle applied to input).

### 2.4 Capabilities: what the core may ask for

```rust
pub struct Capabilities {
    pub digits: bool,          // can type page numbers directly
    pub colours: bool,         // has the four coloured keys
    pub ok: bool,              // has a distinct confirm key
    pub release: bool,         // reports Phase::Release, so long presses work
    pub text: TextEntry,       // None | Multitap | Keyboard | OnScreen
    pub pointer: bool,         // reports Control::Cell
    pub outside_grid: bool,    // can show text outside the 40 by 25 grid
}
```

The core uses it for three things:

- **Prompts.** The status area and the tuning screen say "OK to go" or "press the key again" depending on `ok` (REQ-NAV-012 already uses "press the same key again").
- **Fallbacks.** Without `release`, Back cannot be a long press of Index, so the remote table binds it to a key instead. Without `digits`, the core offers a page dial (section 4.3).
- **Refusals.** Without `text`, address entry and text fields that need letters are refused with a message, rather than offered and unusable.

### 2.5 The router: modes owned by the core

```rust
pub enum Mode { Browse, Entry, Form, Address, Confirm(ConfirmKind), Catch }

pub struct Router { mode: Mode, gestures: Gestures, /* … */ }

impl Router {
    pub fn input(&mut self, event: InputEvent, table: &BindingTable,
                 viewer: &mut Viewer, form: Option<&mut FormModel>,
                 context: &KeyContext<'_>) -> Routed;
}

pub enum Routed { Viewer(Action), Form(FormAction), Address(AddressAction), Quit, None }
```

The order `neottx` now hard-codes in `view.rs` moves here, with one test per mode:

1. **Confirm.** A pending confirmation takes only Confirm (or the same coloured key, REQ-NAV-012) as yes; every other function cancels. Events whose origin is `Paste` or `Synthetic` always cancel.
2. **Form.** A focused form takes text and field functions; the colour keys keep their REQ-NAV-009 meanings; Cancel leaves the form.
3. **Address.** Address entry takes text; Confirm goes; Cancel restores the location.
4. **Catch.** When page catching is on, the arrows become NextLink and PreviousLink.
5. **Entry and Browse.** Everything else goes to the viewer.

`Quit` is returned to the port, which owns the process.

### 2.6 Gestures: long presses in the core

```rust
pub struct Gestures { held: Option<(Control, Instant)>, /* … */ }
/// Microseconds after which a held control is a long press.
pub const LONG_PRESS_US: u64 = 600_000; // to be measured on real remotes
```

- **With release events,** `Gestures` turns Press and Release into Short or Long using the port's `Clock`.
- **Without them,** every Press is Short and the binding table never asks for Long.

This implements REQ-NAV-010's default "hold Index for Back" on remotes that report release. The value of `LONG_PRESS_US` is a placeholder, to be measured, and is defined once next to `MULTITAP_US`.

## 3. Machines

Each subsection gives the input sources, how the driver names controls, what the machine lacks, and where the code lives.

### 3.1 Terminals: `neottx` on macOS, Linux and Windows

**Library.** crossterm, already a dependency of `neoteletext-cli`.

**Driver.** `crates/neoteletext-cli/src/hmi.rs` (new) turns `crossterm::event::Event` into `InputEvent`:

```rust
fn control(key: KeyEvent) -> Option<Control> {
    Some(match key.code {
        KeyCode::Char(c @ '0'..='9') => Control::Digit(c as u8 - b'0'),
        KeyCode::F(n @ 1..=4) => Control::Colour(Colour::from_fastext(n - 1)),
        KeyCode::F(n) => Control::Function(n),
        KeyCode::Up => Control::Up,
        KeyCode::Home => Control::Home,
        KeyCode::PageUp => Control::PageUp,
        KeyCode::Insert => Control::Insert,
        KeyCode::Delete => Control::Delete,
        KeyCode::Char(c) => Control::Char(c),
        // …
        _ => return None,
    })
}

fn phase(kind: KeyEventKind) -> Phase {
    match kind { KeyEventKind::Press => Phase::Press,
                 KeyEventKind::Repeat => Phase::Repeat,
                 KeyEventKind::Release => Phase::Release }
}
```

**Capabilities and limits.**

- **Release events** arrive only when the terminal supports the kitty keyboard protocol and the program pushes `KeyboardEnhancementFlags::REPORT_EVENT_TYPES`. Otherwise `release` is false. The driver probes with `crossterm::terminal::supports_keyboard_enhancement()`, and pops the flags in `RawGuard::drop`.
- **The keypad.** It is distinguishable from the main keys only with the same protocol (`KeyEventState::KEYPAD`). The bindings in `docs/web-native-concept.md` of the neoteletext-browser repository section 7 never need to distinguish them.
- **Pasted text.** `EnableBracketedPaste` makes a paste arrive as `Event::Paste(String)`. The driver emits its characters as `Char` events with `Origin::Paste`, so a pasted `r r` or a newline can never follow a link or confirm a submission (section 2.5 rule 1). Today the viewer does not enable bracketed paste, so a paste arrives as ordinary key presses.
- **Mouse.** `EnableMouseCapture` reports `MouseEvent { column, row, .. }` in terminal cells. With text output, one terminal cell is one teletext cell. With iTerm2 images, the driver divides by the cell size in pixels from `terminal::text_area()`. The viewer does not capture the mouse today.
- **macOS.**
  - Option sends Alt only if the iTerm2 profile sets "Left Option key: Esc+". Otherwise Alt+Left, today's Back, may never arrive. Backspace and the keypad bindings cover it.
  - F1 to F4 control brightness and media unless Fn is held or the system setting is changed. The letters `r`, `g`, `y`, `c` remain.
- **Windows.** crossterm reads the console API, so the same driver works. There are no inline images, so the output is text.
- **The sandbox.** The terminal is already open before Landlock closes the file system, so input needs no new rule.

### 3.2 A desktop window

A future port, not in the workspace. **Library:** winit, which needs approval.

- **Driver:** `WindowEvent::KeyboardInput` gives `physical_key: PhysicalKey::Code(KeyCode::F1 | KeyCode::Numpad7 | …)`, `state: ElementState`, `repeat: bool` and `logical_key`. Physical codes map to `Control`; the logical key and IME commits give `Char`.
- **Capabilities:** release events, keypad distinction, pointer, scroll wheel (`Detent`), and window chrome outside the grid for the location bar (`outside_grid: true`).
- **Clipboard paste** goes through the same `Origin::Paste` rule.

### 3.3 The web browser: WebAssembly

The browser receiver of `docs/web-native-concept.md` of the neoteletext-browser repository section 13. **Libraries:** `web-sys` and `wasm-bindgen`, which need approval.

- **Keys.** `KeyboardEvent.code()` gives physical names (`"F1"`, `"Numpad7"`, `"ArrowUp"`), `key()` gives text, and `repeat()` gives auto-repeat. `keyup` gives release.
- **Keys the browser keeps.** Ctrl+L, Ctrl+W, Ctrl+T and F11 cannot be taken reliably, and F5 and F1 should not be. The browser table binds Address and Refresh to other keys, for example `/` and `R`, and leaves the browser's own keys alone.
- **Pointer and touch.** `PointerEvent` coordinates are divided by the canvas cell size to give `Control::Cell`.
- **Game controllers.** The Gamepad API's standard mapping puts A, B, X and Y at buttons 0 to 3 and the D-pad at 12 to 15. Controllers are polled each animation frame and compared with the previous state to get press and release.
- **Screen readers.** A hidden DOM text layer, from the same Unicode text `neottx text` produces, with `aria-live` on row 0 and row 24 (`docs/web-native-concept.md` of the neoteletext-browser repository section 11).

### 3.4 Linux television boxes: Raspberry Pi, set-top boxes

**Driver.** evdev, reading `/dev/input/event*` directly or through the `evdev` crate, which needs approval.

- **Phases.** Each `input_event` has a value of 0 for release, 1 for press and 2 for auto-repeat, so `release` is true.
- **IR remotes.** The kernel's rc-core decodes them, and `ir-keytable` loads a keymap that turns scancodes into key codes. The port only sees evdev key codes:

  | Key code | Control |
  |---|---|
  | `KEY_RED`, `KEY_GREEN`, `KEY_YELLOW`, `KEY_BLUE` | `Colour(Red…Cyan)` (blue is cyan) |
  | `KEY_0`…`KEY_9`, `KEY_NUMERIC_0`…`KEY_NUMERIC_9` | `Digit` |
  | `KEY_OK`, `KEY_SELECT` | `Ok` |
  | `KEY_BACK`, `KEY_EXIT` | `Back`, `Escape` |
  | `KEY_CHANNELUP`, `KEY_CHANNELDOWN` | `PageUp`, `PageDown` (next and previous page in the remote table) |
  | `KEY_TEXT`, `KEY_INFO`, `KEY_SUBTITLE` | `Remote(Text…)` |

- **HDMI-CEC.** The television's own remote reaches the box over HDMI. The kernel's CEC framework, for adapters that pass remote keys through, delivers them as evdev keys too. The CEC "User Control" codes list the coloured keys as F1 (Blue), F2 (Red), F3 (Green), F4 (Yellow). A CEC driver that maps F1 to red by number swaps every colour. The keymap on the device must be checked against the remote.
- **The sandbox.** `neottx` closes the file system with Landlock after start-up. The port opens its `/dev/input` devices first, as `neottx` measures the terminal before closing it, and the seccomp filter must allow `read` and `ioctl(EVIOCGRAB)` on them.

### 3.5 Android television

Through the C interface `neoteletext-ffi` (Phase 2) and the port's own Kotlin or Java code.

- **Keys.** `KeyEvent.KEYCODE_PROG_RED`, `_GREEN`, `_YELLOW`, `_BLUE`; `KEYCODE_TV_TELETEXT`; `KEYCODE_DPAD_*` and `KEYCODE_DPAD_CENTER`; `KEYCODE_CHANNEL_UP` and `_DOWN`; `KEYCODE_BACK`; digits.
- **Phases.** `ACTION_DOWN` and `ACTION_UP`, with `getRepeatCount()` for repeats, so `release` is true.
- **The C interface** carries `InputEvent` as a plain struct of three integers and a time. The C side never sees `Function` or the router.

### 3.6 Microcontrollers: Cortex-M, RISC-V, ESP32

No allocation, no operating system. The driver usually runs in an interrupt or an async task and fills a fixed-size single-producer queue. `Input::poll` drains it.

| Hardware | How it is read | Controls | Notes |
|---|---|---|---|
| Push buttons | `embedded-hal` `InputPin`, or Embassy `ExtiInput::wait_for_any_edge`, debounced in the port | `Button(n)` | 3 buttons are enough with the three-button table (section 4.3). |
| 4 by 4 matrix keypad | Row and column scanning on GPIO | `Digit`, `Colour` for A to D, `*`, `#` | A to D are the four colours; `*` is Index (held: Back); `#` is OK. Every function of a remote fits. |
| IR receiver | ESP32 RMT peripheral, RP2040 PIO, or timer input capture; NEC, RC-5 or RC-6 decoding (for example the `infrared` crate, which needs approval) | Through a learned table: `Digit`, `Colour`, `Ok`, `Remote(..)` | NEC and RC-5 send repeat frames while held but no release, so release is inferred when repeats stop. A *learn* mode stores the codes of whatever remote is at hand, in flash. |
| Rotary encoder with push | ESP32 PCNT, STM32 timer encoder mode, or GPIO interrupts | `Detent(±1)`, `Ok` | Turning steps pages; pressing opens the dial (section 4.3). |
| Capacitive touch pads | ESP32 touch peripheral | `Button(n)` | Thresholds are calibrated at start-up. |
| USB keyboard | USB host on ESP32-S3 or RP2040; HID boot protocol | Keyboard usage IDs: F1 to F4 are `0x3A` to `0x3D`, keypad 1 to 9 and 0 are `0x59` to `0x62`, keypad `.` is `0x63`, Insert to Up are `0x49` to `0x52` | A boot report lists the keys down. Comparing it with the previous report gives press and release. |
| Bluetooth keyboard or controller | ESP32 Bluetooth HID host | As USB, or `Button(n)` | Pairing needs some other input first, such as the buttons. |
| Touch screen | XPT2046 (resistive, SPI) or FT6206 (capacitive, I²C) | `Cell(row, column)` | Needs scaling from panel to grid coordinates, and calibration for resistive panels. |

Constraints that shape the code:

- **The binding table is `const` data in flash.** A learned IR table is a fixed-size array in a flash sector. Neither allocates.
- **The queue holds `InputEvent`, which is `Copy`.** Event loss on overflow is counted, not hidden.
- **Small screens are an output problem that becomes an input problem.** A 320 by 240 panel cannot show 480 by 500 pixels, or even the 480 by 250 of the 12 by 10 cell mode (REQ-RND-003). Zoom (REQ-NAV-007) and a panning function become essential controls on such a port, and the binding table must reach them.

### 3.7 Game controllers

They are found on desktops (gilrs, which needs approval), in browsers (Gamepad API) and on ESP32 (Bluetooth HID).

- **Xbox face buttons are coloured like Fastext:** A green, B red, X blue, Y yellow. The game-controller table binds them by colour, with X as cyan.
- **Nintendo controllers** swap A and B and X and Y and are not coloured. They are bound by position, so the table follows the standard mapping's positions, not the labels.
- **The D-pad** pages and steps sub-pages. The bumpers are Back and Forward, and the menu button opens the dial for page numbers.

### 3.8 Accessibility

- **Switch access.** One or two switches are `Button(0)` and `Button(1)`. A scanning table steps a highlight through the functions of the current mode, and the switch selects. It is one more binding table plus a scanning mode in the router.
- **Screen readers.** These read the text layer of the web and desktop ports; the core supplies the Unicode text of each row and the link labels.

### 3.9 Headless

`neottx list`, `show`, `text`, `render` and `convert` take no input. Their command-line arguments are their interface, and they do not create an `Input` at all.

## 4. Mapping functions to machines

### 4.1 Essential functions

A port that cannot reach these cannot be used as a receiver:

- choose a page (digits or the dial);
- next and previous page;
- the four colours;
- Confirm and Cancel (or the same key again, REQ-NAV-012);
- leave a form.

Reveal, Hold, Mix, Zoom, sub-pages, Back, Forward and Home are standard; the address bar, Refresh and page catching are the browser extension.

### 4.2 The default tables

| Function | Keyboard | IR or CEC remote | 4 by 4 keypad | Game controller | Rotary encoder |
|---|---|---|---|---|---|
| Digits | `0`–`9` | `0`–`9` | `0`–`9` | Dial | Dial |
| Red, Green, Yellow, Cyan | F1–F4, `r` `g` `y` `c` | Coloured keys | A, B, C, D | B, A, Y, X | Dial menu |
| Index | End, `i` | Index or Text | `*` | Menu | Dial menu |
| Next and previous page | Up and Down, `+` and `-` | Channel up and down, D-pad | — | D-pad up and down | Turn |
| Next and previous sub-page | Right and Left | D-pad right and left | — | D-pad right and left | — |
| Confirm | Enter, F4 for submissions | OK | `#` | A | Press |
| Cancel | Delete, Escape | Back or Exit | `*` outside Browse | B outside Browse | Long press |
| Back and Forward | Page Up and Page Down, Backspace | Index held; Back | `*` held | Bumpers | — |
| Reveal, Hold, Mix, Zoom | `?`, Insert, `m`, `z` | Dedicated keys where present | Dial menu | Dial menu | Dial menu |
| Address | Ctrl+L, F6 | — (presets instead) | — | — | — |

One control may mean different things in different modes; each binding's mode set (section 2.3) resolves it. On the keypad, `*` is Index while browsing and Cancel in every other mode, and held it is Back. On a game controller, B is red while browsing and Cancel in every other mode. While a link to another service waits for confirmation, the same coloured key still confirms it (REQ-NAV-012), so B confirms a red link there rather than cancelling.

### 4.3 Devices without digits: the dial

A device without digit keys still has to choose a page. The router's dial mode draws a three-digit spinner in the receiver-owned status area (row 0 columns 0 to 7, REQ-SEC-005):

- Up and Down (or a turn of the encoder) change the current digit.
- Right moves to the next digit, and Confirm goes.

The same mode carries a short menu for the functions without their own control: Reveal, Hold, Mix, Zoom and the colours. Only the core draws it, so pages cannot imitate it. The three-button table (Up, Down, Select) and the encoder table both use it.

### 4.4 Text without a keyboard

Text is needed for form fields and web addresses. Each `TextEntry` value gives one method:

- `Multitap`: the existing letter entry from digits (`MULTITAP_US`).
- `OnScreen`: an on-screen keyboard drawn by the core in rows 1 to 23, chosen with the arrows and Confirm. The page is hidden while it is shown, so it cannot be imitated.
- `None`: forms needing letters, and address entry, are refused with a message. Channel presets replace typed addresses (`docs/web-native-concept.md` of the neoteletext-browser repository section 9).

## 5. Third-party crates this would need

None is added by this document. Each is a library choice for the owner.

| Crate | For | Where |
|---|---|---|
| crossterm | Terminals | Already used by `neoteletext-cli` |
| winit | Desktop windows | A future desktop port |
| web-sys, wasm-bindgen | The browser | A future WebAssembly port |
| evdev | Linux input devices | `neoteletext-cli` on Linux, or a television-box port |
| gilrs | Game controllers on desktops | A future desktop port |
| infrared | IR decoding on microcontrollers | Example microcontroller ports |
| embedded-hal, embassy | Buttons, encoders and keypads on microcontrollers | Example microcontroller ports |

The core itself gains no dependency. `hmi.rs` is plain `no_std` Rust under the existing rules: no allocation, no panics, no floating point.

## 6. Changes to the source, in order

1. **Add `neoteletext-core/src/hmi.rs`** with `Function`, `Binding`, `BindingTable`, `Capabilities`, `Gestures`, `Router` and the default tables. Update `ARCHITECTURE.md`'s L6 section and its dependency table (REQ-ARCH-011).
2. **Replace `port::Keys` with `port::Input`** (`Control`, `Phase`, `Origin`, `InputEvent`), and update `docs/spec/design/porting.md`.
3. **Change `Viewer::key` and `FormModel::key` to take `Function`.** Delete `viewer::Key` and `forms::FormKey` in the same change, as the project's rules ask, rather than keeping shims.
4. **Move the modal loop out of `neoteletext-cli/src/view.rs` into `Router`.** `neottx` keeps only drawing, its source, and quitting.
5. **Implement `Gestures`**, with REQ-NAV-010's "hold Index for Back" as the first user.
6. **In `neottx`:**
   - enable bracketed paste and mark pasted events;
   - enable the kitty keyboard protocol when the terminal supports it;
   - optionally enable the mouse.
7. **Write the tests:**
   - one test per router mode and per confirmation rule;
   - a test that `Origin::Paste` never confirms;
   - long-press timing with a fake `Clock`;
   - completeness of every default table;
   - a test that the dial and the on-screen keyboard are drawn only in receiver-owned areas.

   Each test must fail on today's code where it describes a gap, for example the paste test and the long-press test.
8. **Write an example port per device class** that has no host to test it, starting with an evdev remote on oldvga. These exercise the tables against real devices.

## 7. Decisions needed

1. **Does the core own input routing**, including the confirmations of REQ-SEC-009 and REQ-NAV-012, as section 2.5 proposes? This moves security behaviour from every port into one tested place.
2. **One `Function` enum** in place of `viewer::Key` and `forms::FormKey`.
3. **Press, repeat and release in the port interface**, and the value of `LONG_PRESS_US`, measured on real remotes.
4. **Binding tables as replaceable data**, with a completeness test per device class.
5. **The dial and the on-screen keyboard** as core-drawn, receiver-owned screens.
6. **Pasted and synthetic input** never confirming, recorded as a SEC requirement.
7. **Which example ports to build**, and the crates each needs (section 5).
