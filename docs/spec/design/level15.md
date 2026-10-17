# Part B — Walkthrough of Presentation Level 1.5 (EN 300 706)

## B.1 Why Level 1.5 exists

The Latin G0 set with national options gives each page 96 alphanumeric characters. Many languages need more than that. Level 1.5 "has evolved in the field due to the language requirements in certain countries" (EN 300 706 introduction) and was documented formally for the first time in EN 300 706.

A Level 1.5 decoder responds exactly as a Level 1.0 decoder does, except that it also reads packets X/26 attached to the page. Each X/26 packet carries instructions of the form "at row r, column c, display character k from character set S instead of what the Level 1 page has there". It is a character-overwrite mechanism and nothing more (clauses 10.1, 12 and 15.1, and Table 1).

The characters that can be written are:
- a character from the **G2 supplementary set**, which contains currency signs, fractions, typographic marks and additional letters;
- a **G0 character combined with a diacritical mark**, such as é, č, ő or ł;
- a plain **G0 character** without national option substitution;
- a small number of **G3 line-drawing characters**.

Clause 15.1 describes typical use: "a few characters from the G2 supplementary set of characters, plus a few G0 characters with diacritical marks."

## B.2 Packets used at Level 1.5

Clause 10.1 lists the Level 1.5 packets. Figure 17 and Table 1 confirm them.

- **X/26/0 to X/26/15.** The character overwrites. The same packet type also carries Programme Delivery Control data, which a Level 1.5 display decoder must skip.
  - Table 1 marks only designation codes 0 to 14 as Level 1.5 character overwrites.
  - Clause 10.1 notes that "some existing Level 1.5 decoders only respond to packets X/26 with designation codes in the range 0 to 14". The SAA5246A (Part D) stores only designation codes 0 to 14.
- **X/28/1 and M/29/1.** Character set designation "for compatibility with earlier specifications only". The meaning of the 7-bit character set codes in these packets is defined in EBU SPB 492 (1992), Figure 14. For example, code 6 is the first Latin set, 8 is block mosaics, 12 is Cyrillic and 16 is Greek. EN 300 706 Table 25 says equipment built for a single group of character sets "may ignore this data".
- **X/28/0 format 1 and M/29/0.** Table 1 marks their presentation data as Level 2.5 only. Table 25, however, ticks their default character set designation function at Level 1.5, 2.5 and 3.5. The designation they give is "also the default G0 and G2 sets to be addressed by packet 26 triplets as part of Level 1.5" (clause 15.2). Decision E.4 records how this contradiction is handled.
- **8/30 formats 1 and 2.** As at Level 1.0.

## B.3 Hamming 24/18 and the triplet

Packet X/26 is made up of:
- byte 6, a Hamming 8/4 **designation code** from 0 to 15;
- bytes 7 to 45, thirteen **triplets** of three bytes each. Triplet number n (1 to 13) occupies bytes 3n+4, 3n+5 and 3n+6.

Each triplet is protected by the Hamming 24/18 code (clause 8.3). Of the 24 bits, bits 1, 2, 4, 8, 16 and 24 are protection bits. The other 18 bits carry data.

**Bit layout.** Numbering bits in transmission order, and so least significant bit first in each byte, the layout is:

| Byte | Bit positions and meaning |
|---|---|
| First | 1 P1, 2 P2, 3 D1, 4 P3, 5 D2, 6 D3, 7 D4, 8 P4 |
| Second | 9 to 15 are D5 to D11, 16 is P5 |
| Third | 17 to 23 are D12 to D18, 24 is P6 |

**Encoding equations** (clause 8.3), all using odd parity:

- P1 = 1 ⊕ D1 ⊕ D2 ⊕ D4 ⊕ D5 ⊕ D7 ⊕ D9 ⊕ D11 ⊕ D12 ⊕ D14 ⊕ D16 ⊕ D18
- P2 = 1 ⊕ D1 ⊕ D3 ⊕ D4 ⊕ D6 ⊕ D7 ⊕ D10 ⊕ D11 ⊕ D13 ⊕ D14 ⊕ D17 ⊕ D18
- P3 = 1 ⊕ D2 ⊕ D3 ⊕ D4 ⊕ D8 ⊕ D9 ⊕ D10 ⊕ D11 ⊕ D15 ⊕ D16 ⊕ D17 ⊕ D18
- P4 = 1 ⊕ D5 ⊕ D6 ⊕ D7 ⊕ D8 ⊕ D9 ⊕ D10 ⊕ D11
- P5 = 1 ⊕ D12 ⊕ D13 ⊕ D14 ⊕ D15 ⊕ D16 ⊕ D17 ⊕ D18
- P6 = 1 ⊕ every other bit of the triplet

**Decoding** uses six odd-parity tests. Bit positions are numbered 1 to 24 in transmission order:

- Test A covers every position whose binary value has bit 0 set: 1, 3, 5, 7 and so on.
- Test B covers positions with bit 1 set.
- Test C covers positions with bit 2 set.
- Test D covers positions with bit 3 set.
- Test E covers positions with bit 4 set.
- Test F covers all 24 bits.

The results are interpreted in the same way as for Hamming 8/4:

| Tests A to E | Test F | Meaning | Action |
|---|---|---|---|
| All pass | Passes | No error | Accept the data |
| All pass | Fails | Error in P6 | Accept the data |
| Any fails | Passes | Double error | Reject the triplet |
| Any fails | Fails | Single error | Correct the bit and accept |

For a single error, the position of the wrong bit is the binary number formed from the failed tests: 16 times E, plus 8 times D, plus 4 times C, plus 2 times B, plus 1 times A. The standard does not say what to do with a rejected triplet, or with an error position greater than 24, which can only come from three or more errors. Decision E.9 covers both.

**The 18 data bits.** They are read as a number with D1 as the least significant bit and are divided into three fields (clause 12.3.1):

- **Address**: 6 bits, D1 to D6. A value from 0 to 39 is a **column address**. A value from 40 to 63 is a **row address**.
- **Mode**: 5 bits, D7 to D11. It says what the triplet does.
- **Data**: 7 bits, D12 to D18. Typically a character code.

## B.4 The active position

X/26 processing keeps an **active position**, a row and a column, that says where the next character goes (clause 12.3.2). It starts at row 0, column 0 at the beginning of each page's enhancement data.

**Row address triplets** (address 40 to 63) set the row:
- Address 40 means row 24.
- Addresses 41 to 63 mean rows 1 to 23.
- Row 0 can only be reached with the special mode "Address Display Row 0" (mode 00111) and address 63. That triplet sets both row and column to 0.

**Column address triplets** (address 0 to 39) set the column to the address value and leave the row unchanged. Then, depending on the mode, they place a character at that position.

**Which row triplets set the row.** The standard contradicts itself here:

- Clause 12.3.2 says the row is set only by the Full Row Colour, Set Active Position and Address Display Row 0 triplets. It adds that Level 2.5 and Level 3.5 decoders should not take the row from any other row triplet.
- Annex A.3, which is normative, describes what existing Level 1.5 decoders actually do: "The most recent value of the address bits in the range 40 to 63 will be treated by existing Level 1.5 decoders as setting the row co-ordinate of the Active Position, regardless of the mode value of the triplet."
- Annex E.3 repeats the Annex A.3 point.
- Annex A.5 even recommends that broadcasters send the Full Screen Colour triplet with address 41, which is row 1, precisely so that it does no harm on Level 1.5 decoders that behave this way.

Decision E.3 records how this is handled.

## B.5 The triplet modes a Level 1.5 decoder acts on

Tables 27, 28 and 29 of EN 300 706 define 32 row modes and 32 column modes. The level tick marks in those tables show which ones apply at Level 1.5.

**Row address group** (address 40 to 63):

| Mode | Function | Level 1.5 action |
|---|---|---|
| 00100 | Set Active Position | Sets the active row. At Level 1.5 only the row part is ticked. The column in the data field is a Level 2.5 feature. |
| 00111 | Address Display Row 0 | With address 63, sets the active position to row 0, column 0. Later column triplets should then address only columns 8 to 39. |
| 01000 to 01101 | Programme Delivery Control data | Not display data. Skipped for display. |
| 11111 | Termination Marker | With address 63, marks the end of the page's enhancement data. The broadcaster fills any unused triplets after it with copies of it. |
| All others | Level 2.5 and 3.5 functions, or reserved | Ignored for display. Under Annex A.3 behaviour they still set the row. |

**Column address group** (address 0 to 39):

| Mode | Function | Level 1.5 action |
|---|---|---|
| 00010 | Character from the G3 set, Level 1.5 form | Places the G3 character given by the data field. Annex E.3 notes that in practice only G3 positions 5/1 (horizontal line), 5/B (right arrow), 5/C (left arrow) and 5/D (up arrow) are used at Level 1.5. |
| 01111 | Character from the G2 set | Places the G2 character given by the data field. |
| 10000 | G0 character without diacritical mark | Places the G0 character given by the data field. As a special case, data value 2/A (binary 0101010) displays "@" instead of "*". |
| 10001 to 11111 | G0 character with diacritical mark | The low four bits of the mode select a diacritical mark from column 4 of the G2 set. That mark is combined with the G0 character given by the data field. |
| 00110 | Programme Delivery Control data | Skipped. The active position is not changed. |
| 01011 | Character from the G3 set, Level 2.5 form | Must be **ignored** by a Level 1.5 decoder (Table 29 note 3). |
| All others | Level 2.5 and 3.5 functions, or reserved | Ignored for display. |

Data values below 0x20 are reserved for the character modes, but the column must still be set to the address value (Table 29).

**The sixteen diacritical marks.** Column 4 of the G2 set, positions 4/0 to 4/F, holds these marks in order:
1. none
2. grave
3. acute
4. circumflex
5. tilde
6. macron
7. breve
8. dot above
9. diaeresis (umlaut)
10. a low dot, at position 4/9
11. ring above
12. cedilla
13. a low line, at position 4/C
14. double acute
15. ogonek
16. caron

The identity of positions 4/9 and 4/C is read from the drawing in Table 37, because the standard gives no names. The standard does not say how the mark and the base letter are drawn together. That is a decoder option (Annex G.2), and Annex E.10 says composition should be limited to the Latin sets.

**No national option substitution.** G0 characters written through X/26 never have national option substitution applied (Table 25 and Table 35 note 2). At the thirteen national option positions they take the base glyphs shown in Table 35: #, ¤, @, [, \, ], ^, _, `, {, |, } and ~. The ESC attribute on the Level 1 page does not affect X/26 characters either (Table 26, code 1/B).

## B.6 The G2 and G3 sets

**The Latin G2 supplementary set** (Table 37) holds 95 characters plus space:

| Column | Contents |
|---|---|
| 2 | Inverted exclamation mark, cent, pound, dollar, yen, hash, section sign, currency sign, single and double quotation marks, guillemets, and four arrows |
| 3 | Degree, plus-minus, superscript 2 and 3, multiplication sign, micro sign, pilcrow, middle dot, division sign, closing quotation marks, guillemets, one-quarter, one-half, three-quarters and inverted question mark |
| 4 | The sixteen diacritical marks |
| 5 | Horizontal bar, superscript 1, registered, copyright, trade mark, musical note, European currency sign, per mille, alpha, and the eighths fractions |
| 6 and 7 | Additional letters and symbols: Ω, Æ, Đ, ª, Ħ, Ĳ, Ŀ, Ł, Ø, Œ, º, Þ, Ŧ, Ŋ and ŉ, with their lower-case partners, and ĸ, ð, ı, ß, and a solid block at 7/F |

Positions 5/9, 5/A, 5/B and 6/5 are not defined and should not be transmitted. Cyrillic, Greek and Arabic have their own G2 sets (Tables 41, 43 and 45). Hebrew pages use the Arabic G2 set.

**The G3 set** (Table 48) holds smoothed mosaic and line-drawing characters. Only the four positions listed under mode 00010 in B.5 matter in practice at Level 1.5.

## B.7 Other character sets

For completeness:

- EN 300 706 defines non-Latin G0 sets: three Cyrillic options (Tables 38 to 40), Greek (Table 42), Arabic (Table 44) and Hebrew (Table 46).
- Table 32 selects them in the non-Western groups. For example, group 0100 with C12 C13 C14 = 100 selects Cyrillic option 2 for Russian and Bulgarian.
- These sets have no national option positions.

## B.8 Ordering, termination and compatibility rules

- **Packet order.** Packets X/26 of a page are sent before its display rows, in ascending designation code order starting from 0 (Annex B.2 and Annex A.3).
- **Gaps.** A gap in the sequence of designation codes before the Termination Marker "may be interpreted as an error condition", and the decoder's response is not defined (Annex D.1.3).
- **Triplet order.** Triplets are in screen order, left to right and top to bottom. A decoder may ignore a triplet that addresses a position earlier than the one before it (Annex A.4).
- **One character per position.** There should be only one character-writing triplet per screen position that a Level 1.5 decoder could act on (Annex A.3).
- **Termination.** A Termination Marker ends the local enhancement data. After it may come object definitions, which a Level 1.5 decoder ignores (Annex A.3).
- **Row 0 during a search.** X/26 characters in row 0 must appear when the page is displayed in full. They need not appear in the rolling header while searching (Annex E.3).
- **Pages without X/26.** If a page has no X/26 packets, a Level 1.5 decoder shows the Level 1 page unchanged (Annex D.1.2).

## B.9 How an overwrite interacts with the Level 1 page

The standard defines the overwrite only as "overwriting the character defined for this location on the Level 1 page" (Table 1). It is silent on most interactions:

- whether a spacing attribute at the overwritten cell still takes effect;
- whether an overwritten character in a concealed area is concealed;
- how it behaves in mosaic mode;
- whether it changes the held mosaic;
- whether it can appear on the lower row of a double-height row.

The only guidance comes from the description of objects in clause 13.3, which says enhancement data takes the Level 1 attributes that apply at its position. Table 26 notes 3 and 6 add that "some existing Level 1.5 decoders will not display local X/26 enhancement data" on the lower row of a double-height row.

Decision E.5 records the interpretation this project adopts.
