# Appendix E — TTI Link Extension (normative)

This appendix defines how a TTI page declares its links (decisions E.36 and E.64). It was amended on 12 October 2026 to allow addresses as targets, and to add the `LK` and `LT` lines.

## E.1 The link command

```
LN,key,target,label
```

- **`key`** is `R`, `G`, `Y` or `C` for the red, green, yellow and cyan keys, or `I` for Index. Each key may appear at most once on a page or sub-page.
- **`target`** has one of these forms:
  - `mpp` or `mpp:ssss`: a page, and optionally a sub-page, in the same service, in hexadecimal;
  - `name:mpp` or `name:mpp:ssss`: a page in another service named in the service descriptor (REQ-WEB-002). Names are letters, digits and hyphens;
  - an address: a channel's folder, or a page file in a channel's folder, such as `https://weather.example/300.t42` (decisions E.64 and E.70).

  A target containing `://` is an address; any other target is a page or a named service. An address shall use `http` or `https`, shall be printable ASCII with no spaces or commas, and shall be at most 256 bytes. A target that breaks these rules makes the link invalid. An address that is a page file in the page's own channel folder is a page of the current channel; any other address leads to another channel (REQ-WEB-026).
- **`label`** is the prompt text. It is at most 9 characters for a link in the same service and 8 for a link to another service, using characters the page's G0 set can show. It is ignored for `I`.

`LN` lines apply to the page or sub-page of the most recent `PN` line.

### E.1.1 Page-number links

```
LK,mpp,target
```

Links the page number `mpp`, which the page shows, to `target`, in any of the forms above. While the page is shown, typing that number, or choosing the number printed on the page, follows the link. A page has at most eight `LK` lines; a ninth is ignored with a diagnostic. Two `LK` lines for the same number, or an invalid number or target, are reported by the page checker as errors (REQ-CLI-006).

### E.1.2 Text links

```
LT,row,column,length,target
```

Links a run of `length` characters starting at `column` on `row` to `target`. `row` is 1 to 23 and the run must lie within the 40 columns; anything else makes the line invalid. A page has at most eight `LT` lines; a ninth is ignored with a diagnostic. The page draws the text itself; a receiver may show it as a link, for example when the viewer asks to see a page's links.

`LK` and `LT` lines apply to the page or sub-page of the most recent `PN` line, and are kept by the TTI writer.

## E.2 Rules

1. **At most four.** A page has at most four coloured links. A fifth coloured `LN` line, or a repeated key, makes the page's links invalid. The page is still displayed, with no links, and a diagnostic is given.
2. **Unknown services.** A target naming a service not in the descriptor makes that link null.
3. **Older decoders.** Older TTI readers ignore `LN` lines. A page that wants links on such readers may also carry an ordinary `FL` line. When both are present, `LN` wins in neoteletext.
4. **Encoding for broadcast.** When the page encoder turns the page into packets (REQ-INP-019):
   - it writes packet X/27/0 from the links to the same service, with links to other services and to addresses written as null;
   - it writes a row 24 (packet X/24) drawn as REQ-NAV-011 describes, with the "display row 24" flag set. The one difference is that a link to another service is written in white rather than its key's colour. The "»" mark still shows, but the prompt does not look like a key that works: a broadcast television cannot follow it.

   A Fastext television therefore shows the same prompts and follows the same-service links.
5. **Reachable without Fastext.** Televisions made before Fastext (before about 1987) can only follow a link by having the viewer type its page number. Every link to a page in the same service should therefore have its three-digit page number visible somewhere in the page's own rows, for example "Sport 140". The page checker (REQ-CLI-006) shall warn when one is not.
6. **Honest about what cannot work.** A link to another service, or a destination outside teletext, cannot be followed on a broadcast-only television. Authors should give such destinations in a form a viewer can act on without the link: the channel or service name, a short address, or a QR code drawn in mosaic graphics. The page checker shall warn when a page's only route to another service is its link.
7. **Linked numbers are shown.** A page-number link changes what typing that number does on that page only, so the number should appear in the page's text, where the viewer can see it. The page checker shall warn when an `LK` line links a number the page does not show.
8. **No packet form.** Addresses, `LK` lines and `LT` lines cannot be carried in packets. The page encoder writes a coloured link to an address as null, with its prompt in white (rule 4), and leaves `LK` and `LT` lines out.

## E.3 Example

```
PN,10000
OL,1,NEWS INDEX
OL,3,Headlines        ...
LN,R,101,Headlines
LN,G,140,Sport
LN,Y,weather:400,Weather
LN,C,190,TV guide
LN,I,100,
```

Row 24 then shows:
- red "Headlines";
- green "Sport";
- yellow "»Weather", because it leaves this service;
- cyan "TV guide".

On a broadcast Fastext television, the yellow prompt appears in white and its key does nothing, because that link leaves the service. The page checker would also warn unless the page's text gives the page numbers 101, 140 and 190, and some other way to reach the weather service.
