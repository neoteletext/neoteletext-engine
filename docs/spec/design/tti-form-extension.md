# Appendix C — TTI Form Extension, version 3 (withdrawn)

**Withdrawn on 27 October 2026 (decision E.126).** The owner withdrew the TTI Form Extension, and the engine no longer implements any of it. This appendix is not normative and is kept only as history. A TTI page's form lines (`FD`, `OPTION`, `FS`, `FP`, `FT` and `FH`) are unknown commands, reported with a diagnostic and ignored as every unknown line is (REQ-INP-013), so a page that still carries them shows its rows alone. The requirements that cite this appendix (REQ-WEB-010, REQ-WEB-011, REQ-NAV-009, REQ-SEC-009) are withdrawn; a channel that wants a form draws it and takes the viewer's keys over the input protocol (area INT).

This appendix defines interactive forms on teletext pages. It is version 3 of the TTI Form System written for the jellytext project. Version 2.0 is kept in `research/jellytext-tti-form-system-spec-v2.md`, and the changes are explained in decision E.31. Where this appendix and version 2.0 differ, this appendix applies.

## C.1 Principles

1. **Visual first.** A form is drawn exactly as the viewer will see it, using ordinary TTI `OL` rows.
2. **Stated positions.** Each field definition states the row, column and width it occupies. The drawn placeholder characters are only decoration.
3. **Definitions next to rows.** A field definition follows the `OL` row it belongs to.
4. **Graceful degradation.** Decoders that do not understand the form commands ignore them and show a readable, non-interactive page.
5. **Ordinary web submission.** Forms submit by HTTP to an address on the page's own origin, and the viewer is then taken to a success or error page.

## C.2 Commands

All commands are TTI lines of the form `XX,arguments`, where XX is two letters. An unknown command is ignored.

| Command | Syntax | Meaning |
|---|---|---|
| Field definition | `FD,type,name,column,width,parameters…` | Defines a field on the row of the most recent `OL` line. `column` is 0 to 39. `width` is the number of columns the field occupies, and the field must fit within the row. Parameters depend on the type (C.3). |
| Option | `OPTION,value,display_text,selected` | One choice of the most recent SELECT or RADIO field. `selected` is `Y` or empty. |
| Form submission | `FS,method,address` | The method is `POST` or `GET`. The address is absolute, or relative to the page's own address. One per page. |
| Form pages | `FP,success_page,error_page` | Page numbers in `mpp` form, shown after submission (REQ-WEB-011). |
| Form token | `FT,token` | An opaque token issued by the server for this page and session, returned with the submission as `ttx_token`. |
| Header | `FH,…` | **Withdrawn.** Ignored with a diagnostic (decision E.31 item 2). |

A field definition applies to the row of the most recent `OL` line, even if other form commands come in between.

For a multi-row field (TEXTAREA), the field starts on that row and continues down the following rows at the same column and width.

Fields on the same page shall not overlap. A page with overlapping fields shall be displayed but not made interactive, and a diagnostic shall be given.

## C.3 Field types

| Type | Parameters after `column,width` | Notes |
|---|---|---|
| TEXT | `maxlen,required,pattern,default` | `pattern` is a simple pattern: `9` for a digit, `A` for a letter, `*` for any character. |
| EMAIL | `maxlen,required,default` | Must contain one "@" with text on either side. |
| TEL | `maxlen,required,pattern,default` | Digits, spaces and "+" only, unless a pattern says otherwise. |
| NUMBER | `min,max,step,required,default` | Whole numbers only. |
| DATE | `min_date,max_date,required,default` | Year-month-day. |
| TIME | `min_time,max_time,required,default` | Hours and minutes. |
| SELECT | `required` | Choices from the following `OPTION` lines, shown one at a time within the field. |
| RADIO | `required` | One choice from the following `OPTION` lines. `OPTION` display text is not drawn by the decoder, because the page row already shows the labels. `column` is the first choice's marker; each further choice's marker position follows as a `column` given in its `OPTION` line after `selected`. |
| CHECKBOX | `value,required,checked` | One marker cell. `checked` is `Y` or empty. |
| TEXTAREA | `rows,maxlen,required,default` | Occupies `rows` rows from the field's row down. |
| SUBMIT | `label,action` | The whole field area is the button. `action` is sent as `ttx_action`. |
| RESET | `label` | Restores all fields to their defaults. |

`required` is `Y` or `N`.

## C.4 Drawing and characters

- **Placeholders are decoration.** The author draws each field's placeholder in the `OL` row.
- **Recommended placeholder characters.** Because the parser never uses them, placeholders may be any characters. Authors should still use characters that look the same in every character set the service uses, such as the full stop for text fields and "(" "*" ")" for markers. They should not use the underscore or square brackets, which are national option positions (decision E.31 item 1).
- **How the decoder draws a field.** An interactive decoder draws the field's current value over the placeholder:
  - in the attributes in force at those cells;
  - highlighted for the field that has focus;
  - with a cursor where text is being entered.

  A value longer than the field scrolls within it.
- **What can be entered.** Characters shall be those the page's G0 set can display. Other characters are refused at entry (decision E.31 item 5).

## C.5 Validation

Before submission, the decoder shall check:
- that required fields are filled;
- lengths against `maxlen`;
- patterns;
- number ranges and steps;
- date and time ranges;
- the EMAIL and TEL rules.

The server remains the authority, and shall check everything again.

## C.6 Degradation and compatibility

- A decoder without form support shows the `OL` rows and ignores every other command in this appendix.
- A version 2.0 form, which has no column or width, shall be shown as an ordinary page and not made interactive, with a diagnostic that it needs converting.
- A conversion tool should convert version 2.0 forms to version 3 by finding the placeholders and writing their columns in. It should warn where a placeholder uses national option characters.
- **Broadcast.** When a form page is encoded for broadcast (REQ-INP-019), the form commands are not transmitted, because a broadcast television has no way to send anything back. A form page should therefore say in its own rows how a viewer without neoteletext can respond, for example by a telephone number or a web address. The page checker (REQ-CLI-006) shall warn when a form page has no such text. It detects this only approximately, and the warning can be marked as considered (REQ-CLI-006).

## C.7 Example

```
PN,40001
DE,Contact form
PS,8000
FT,5f2c9a17e0
OL,1,CONTACT US
OL,3,Name:  ...............................
FD,TEXT,name,7,31,50,Y,,
OL,4,Email: ...............................
FD,EMAIL,email,7,31,100,Y,
OL,6,Urgency: (*) Low  ( ) Medium  ( ) High
FD,RADIO,urgency,10,1,Y
OPTION,low,Low,Y
OPTION,medium,Medium,,19
OPTION,high,High,,31
OL,8,  < SEND MESSAGE >
FD,SUBMIT,send,2,16,Send message,send_contact
FS,POST,/contact
FP,40101,40102
```

On a decoder without form support, this page shows the labels and dotted placeholders. On neoteletext it is a working form. The session comes from the cookie, the token from `FT`, and the submission goes to `/contact` on the page's own origin.
