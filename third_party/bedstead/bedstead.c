/* -*- c-file-style: "bsd"; indent-tabs-mode: t; -*- */
/*
 * Many of the character bitmaps below formed the typeface embodied in
 * the SAA5050 series of character-generator chips originally made and
 * sold by British company Mullard in the early 1980s.  Copyright in the
 * typeface will still be owned by Mullard's corporate successors, but
 * under section 55 of the Copyright Designs and Patents Act 1988 that
 * copyright is no longer infringed by the production or use of
 * articles specifically designed or adapted for producing material in
 * that typeface.
 *
 * The rest of the glyphs, and all of the code in this file, were
 * written by Ben Harris <bjh21@bjh21.me.uk>, Simon Tatham
 * <anakin@pobox.com>, Marnanel Thurman <marnanel@thurman.org.uk> and
 * Neil Williamson <p298@tiddles.org> between 2009 and 2025.
 *
 * To the extent possible under law, Ben Harris, Simon Tatham, Marnanel
 * Thurman and Neil Williamson have dedicated all copyright and related
 * and neighboring rights to this software and the embodied typeface to
 * the public domain worldwide.  This software and typeface are
 * distributed without any warranty.
 *
 * You should have received a copy of the CC0 Public Domain Dedication
 * along with this software. If not, see
 * <http://creativecommons.org/publicdomain/zero/1.0/>.
 */
/*
 * This is a program to construct an outline font from a bitmap.  It's
 * based on the character-rounding algorithm of the Mullard SAA5050
 * series of Teletext character generators, and thus works best on
 * character shapes in the same style of those of the SAA5050.  This
 * file includes all of the glyphs from the SAA5050, SAA5051, SAA5052,
 * SAA5053, SAA5054, SAA5055, SAA5056, and SAA5057.  The output is a
 * Spline Font Database file suitable for feeding to Fontforge.
 *
 * The character-smoothing algorithm of the SAA5050 and friends is
 * a fairly simple means of expanding a 5x9 pixel character to 10x18
 * pixels for use on an interlaced display.  All it does is to detect
 * 2x2 clumps of pixels containing a diagonal line and add a couple of
 * subpixels to it, like this:
 *
 * . #  -> . . # # -> . . # # or # . -> # # . . -> # # . .
 * # .     . . # #    . # # #    . #    # # . .    # # # .
 *         # # . .    # # # .           . . # #    . # # #
 *         # # . .    # # . .           . . # #    . . # #
 *
 * This is applied to every occurrence of these patterns, even when
 * they overlap, and the result is that thin diagonal lines are
 * smoothed out while other features mostly remain the same.
 *
 * One way of extending this towards continuity would be to repeatedly
 * double the resolution and add more pixels to diagonals each time,
 * but this ends up with the diagonals being much too heavy.  Instead,
 * in places where the SAA5050 would add pixels, this program adds a
 * largeish triangle to each unfilled pixel, and removes a small
 * triangle from each filled one, something like this:
 *
 * . #  -> . . # # -> . . / # or # . -> # # . . -> # \ . .
 * # .     . . # #    . / # /    . #    # # . .    \ # \ .
 *         # # . .    / # / .           . . # #    . \ # \
 *         # # . .    # / . .           . . # #    . . \ #
 *
 * The position of the lines is such that the diagonal stroke is the
 * same width as a horizontal or vertical stroke (assuming square
 * pixels).  There are a few additional complications, in that the
 * trimming of filled pixels can leave odd gaps where a diagonal stem
 * joins another one, so the code detects this and doesn't trim in
 * these cases:
 *
 * . # # -> . . # # # # -> . . / # # # -> . . / # # #
 * # . .    . . # # # #    . / # / # #    . / # # # #
 *          # # . . . .    / # / . . .    / # / . . .
 *          # # . . . .    # / . . . .    # / . . . .
 *
 * That is the interesting part of the program, and is in the dochar()
 * function.  Most of the rest is just dull geometry to join all the
 * bits together into a sensible outline.  Much of the code is wildly
 * inefficient -- O(n^2) algorithms aren't much of a problem when you
 * have at most a few thousand points to deal with.
 *
 * A rather nice feature of the outlines produced by this program is
 * that when rasterised at precisely 10 or 20 pixels high, they
 * produce the input and output respectively of the character-rounding
 * process.  While there are obious additional smoothings that could
 * be applied, doing so would probably lose this nice property.
 *
 * The glyph bitmaps included below include all the ones from the various
 * members of the SAA5050 family that I know about.  They are as shown
 * in the datasheet, and the English ones have been checked against a
 * real SAA5050.  Occasionally, different languages have different
 * glyphs for the same character -- these are represented as
 * alternates, with the default being the glyph that looks best.
 *
 * There are some extra glyphs included as well, some derived from
 * standard ones and some made from whole cloth.  They are built on
 * the same 5x9 matrix as the originals, and processed in the same way.
 */

#include <assert.h>
#include <ctype.h>
#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* This cast to "int" assumes we don't have any stupidly large arrays. */
#define lenof(x) ((int)(sizeof(x) / sizeof((x)[0])))

/* Font identity information. */
#define FAMILY_NAME "Bedstead"
#define VERSION "3.261"
#define VENDOR_ID "BJHa" /* For the achVendID field in the 'OS/2' table. */
#define FOUNDRY "bjh21" /* For the XLFD/BDF FOUNDRY property. */

/* Metrics expressed in pixels. */
#define XSIZE 6
#define YSIZE 10
#define ASCENT 8
#define DESCENT 2
#define XHEIGHT 5
#define ALEFHEIGHT 6
#define CAPHEIGHT (ASCENT-1)

/*
 * Design parameters.  These can vary between fonts in the Bedstead family.
 *
 * To faithfully represent the output of an SAA5050, we need to know a
 * little about TV pixel aspect ratios:
 *
 * http://www.lurkertech.com/lg/video-systems/#sqnonsq
 *
 * The SAA5050 can be used to drive both 480i ("NTSC") and 576i
 * ("PAL") displays.  In 480i, industry convention is that you get
 * square pixels with a pixel clock of 12+3/11 MHz.  In 576i, you need
 * 14.75 MHz.  The SAA5050 takes a nominal 6MHz clock and uses both
 * edges to output subpixels (duty cycle between 0.4 and 0.6).  This
 * means that the nominal pixel aspect ratios are 12+3/11:12 for 480i
 * and 14.75:12 for 576i.  These correspond to pixel widths of 102.3
 * and 122.9 respectively.
 *
 * 102.3 is close enough to 100 that we may as well just use square
 * pixels, which will work better on modern displays.  Similarly,
 * 122.9 is close to 125 and keeping the widths as simple fractions
 * makes the fonts easier to work with.
 *
 * Double-height mode requires fonts half the usual width, hence the
 * existence of ultra condensed and extra condensed forms.  The other
 * two widths just fill in the gap with the obvious fractions of the
 * normal width.
 */

static struct width {
	char const *option;
	char const *suffix;
	double xpix;
	int ttfwidth;
} const widths[] = {
	{ "--normal",          "",                 100.0, 5 },
	{ "--ultra-condensed", " Ultra Condensed",  50.0, 1 },
	{ "--extra-condensed", " Extra Condensed",  62.5, 2 },
	{ "--condensed",       " Condensed",        75.0, 3 },
	{ "--semi-condensed",  " Semi Condensed",   87.5, 4 },
	{ "--extended",        " Extended",        125.0, 7 },
};

static struct width const *width = &widths[0];

/* Size of output pixels in font design units (usually 1000/em) */
#define XPIX (width->xpix)
#define YPIX 100

/* Internally, we work in pixels 100 design units square */
#define XPIX_S 100
#define YPIX_S 100

#define XSCALE ((double)XPIX_S / (double)XPIX)
#define YSCALE ((double)YPIX_S / (double)YPIX)

/* Position of diagonal lines within pixels */
/* 29 is approximately 100 * (1-1/sqrt(2)) */
#define XQTR_S 29
#define YQTR_S 29

static struct weight {
	char const *option;
	char const *suffix;
	int weight; /* Expressed in internal units */
	int ttfweight;
} const weights[] = {
	{ "--medium", "",                      0, 500 },
	{ "--light",  " Light",	2 * XQTR_S - 100, 300 },
	{ "--bold",   " Bold",               +50, 700 },
};

static struct weight const *weight = &weights[0];

/*
 * Slightly evil macro for overloading based on argument count.
 * See https://jadlevesque.github.io/PPMP-Iceberg/explanations#overloading-macros-based-on-argument-count
 */
#define GET_MACRO(_1, _2, _3, x, ...) x

/* U(N) sets the code point and name of a glyph not in AGLFN */
#define U(N) 0x ## N, 0x ## N >= 0x10000 ? "u" #N : "uni" #N
/* UALIAS2 is for passing U(N) as the first two arguments. */
#define UALIAS2(alias_unicode_name, canonical)				\
	{ { .alias_of = canonical }, alias_unicode_name, IS_ALIAS }
#define UALIAS3(alias_unicode, alias_name, canonical)			\
	{ { .alias_of = canonical }, alias_unicode, alias_name, IS_ALIAS }
#define UALIAS(...) GET_MACRO(__VA_ARGS__, UALIAS3, UALIAS2)(__VA_ARGS__)
#define ALIAS(alias, canonical) UALIAS(NU, alias, canonical)

static struct glyph {
	union {
		/*
		 * Top row (and left column) don't appear in ROM.  But
		 * we allow an extra byte so that string constants can
		 * write a terminating null.  Without that, GCC 15
		 * generates a warning that the string might be
		 * unterminated.
		 */
	        unsigned char data[YSIZE - 1 + 1];
		char const *alias_of;
		int subr_idx;
	};
	uint_least32_t unicode; /* Code point and maybe variation selector. */
#define U_UVMASK 0x001FFFFF /* Code point of base character. */
#define U_HASVS  0x00200000 /* Is there a variation selector? */
#define U_VSMASK 0x0F000000 /* Bottom 4 bits of VS code point. */
#define U_VSSHIFT 24
#define VS(uvs) ((((unsigned long)uvs << U_VSSHIFT) & U_VSMASK) | U_HASVS)
#define GET_UV(u) (u & U_UVMASK)
#define GET_UVS(u) ((u & U_VSMASK) >> U_VSSHIFT | 0xfe00)
#define NU 0xFFFFFFFF /* No Unicode code point. Must be > all valid values. */
	char const *name;
	uint_least16_t flags;
#define SEP  0x01 /* Separated graphics */
#define MOS6 0x02 /* 6-cell mosaic graphics character */
#define MOS4 0x04 /* 4-cell mosaic graphics character */
#define MOS8 0x08 /* 8-cell mosaic graphics character */
#define SEP6 (SEP | MOS6)
#define SEP4 (SEP | MOS4)
#define JOIN_L 0x10 /* Copy left column leftward to join to next character. */
#define JOIN_U 0x20 /* Copy top row upwards ditto. */
#define JOIN_R 0x40 /* Pretend column to right is identical to right column. */
#define JOIN_D 0x80 /* Similarly downwards. */
#define JOIN_H (JOIN_L | JOIN_R)
#define JOIN_V (JOIN_U | JOIN_D)
#define JOIN (JOIN_H | JOIN_V)
#define IS_ALIAS 0x100 /* Set iff .alias_of is valid (and .data is not). */
#define IS_SUBR  0x200 /* Set iff .subr_idx is valid.(and .data is not). */
#define COMPAT   0x400 /* Mark as a compatibility alias in complement PDF. */
	uint_least16_t left_sidebearing; /* Set when outline generated. */
} glyphs[] = {
 /*
  * The first batch of glyphs comes from the code tables at the end of
  * the Mullard SAA5050 series datasheet, dated July 1982.
  */
 /* US ASCII (SAA5055) character set */
 {"\00\00\00\00\00\00\00\00\00", 0x0020, "space" },
 {"\04\04\04\04\04\00\04\00\00", 0x0021, "exclam" },
 {"\12\12\12\00\00\00\00\00\00", 0x0022, "quotedbl" },
 {"\12\12\37\12\37\12\12\00\00", 0x0023, "numbersign" },
 {"\16\25\24\16\05\25\16\00\00", 0x0024, "dollar" },
 {"\30\31\02\04\10\23\03\00\00", 0x0025, "percent" },
 {"\10\24\24\10\25\22\15\00\00", 0x0026, "ampersand" },
 {"\04\04\10\00\00\00\00\00\00", 0x2019, "quoteright" },
 {"\02\04\10\10\10\04\02\00\00", 0x0028, "parenleft" },
 {"\10\04\02\02\02\04\10\00\00", 0x0029, "parenright" },
 {"\04\25\16\04\16\25\04\00\00", 0x002a, "asterisk" },
 {"\00\04\04\37\04\04\00\00\00", 0x002b, "plus" },
 {"\00\00\00\00\00\04\04\10\00", 0x002c, "comma"},
 {"\00\00\00\16\00\00\00\00\00", 0x002d, "hyphen" },
 {"\00\00\00\00\00\00\04\00\00", 0x002e, "period"},
 {"\00\01\02\04\10\20\00\00\00", 0x002f, "slash" },
 {"\04\12\21\21\21\12\04\00\00", 0x0030, "zero" },
 {"\04\14\04\04\04\04\16\00\00", 0x0031, "one" },
 {"\16\21\01\06\10\20\37\00\00", 0x0032, "two" },
 {"\37\01\02\06\01\21\16\00\00", 0x0033, "three" },
 {"\02\06\12\22\37\02\02\00\00", 0x0034, "four" },
 {"\37\20\36\01\01\21\16\00\00", 0x0035, "five" },
 {"\06\10\20\36\21\21\16\00\00", 0x0036, "six" },
 {"\37\01\02\04\10\10\10\00\00", 0x0037, "seven" },
 {"\16\21\21\16\21\21\16\00\00", 0x0038, "eight" },
 {"\16\21\21\17\01\02\14\00\00", 0x0039, "nine" },
 {"\00\00\04\00\00\00\04\00\00", 0x003a, "colon"},
 {"\00\00\04\00\00\04\04\10\00", 0x003b, "semicolon"},
 {"\02\04\10\20\10\04\02\00\00", 0x003c, "less" },
 {"\00\00\37\00\37\00\00\00\00", 0x003d, "equal" },
 {"\10\04\02\01\02\04\10\00\00", 0x003e, "greater" },
 {"\16\21\02\04\04\00\04\00\00", 0x003f, "question" },
 {"\16\21\27\25\27\20\16\00\00", 0x0040, "at" },
 {"\04\12\21\21\37\21\21\00\00", 0x0041, "A" },
 {"\36\21\21\36\21\21\36\00\00", 0x0042, "B" },
 {"\16\21\20\20\20\21\16\00\00", 0x0043, "C" },
 {"\36\21\21\21\21\21\36\00\00", 0x0044, "D" },
 {"\37\20\20\36\20\20\37\00\00", 0x0045, "E" },
 {"\37\20\20\36\20\20\20\00\00", 0x0046, "F" },
 {"\16\21\20\20\23\21\17\00\00", 0x0047, "G" },
 {"\21\21\21\37\21\21\21\00\00", 0x0048, "H" },
 {"\16\04\04\04\04\04\16\00\00", 0x0049, "I" },
 {"\01\01\01\01\01\21\16\00\00", 0x004a, "J" },
 {"\21\22\24\30\24\22\21\00\00", 0x004b, "K" },
 {"\20\20\20\20\20\20\37\00\00", 0x004c, "L" },
 {"\21\33\25\25\21\21\21\00\00", 0x004d, "M" },
 {"\21\21\31\25\23\21\21\00\00", 0x004e, "N" },
 {"\16\21\21\21\21\21\16\00\00", 0x004f, "O" },
 {"\36\21\21\36\20\20\20\00\00", 0x0050, "P" },
 {"\16\21\21\21\25\22\15\00\00", 0x0051, "Q" },
 {"\36\21\21\36\24\22\21\00\00", 0x0052, "R" },
 {"\16\21\20\16\01\21\16\00\00", 0x0053, "S" },
 {"\37\04\04\04\04\04\04\00\00", 0x0054, "T" },
 {"\21\21\21\21\21\21\16\00\00", 0x0055, "U" },
 {"\21\21\21\12\12\04\04\00\00", 0x0056, "V" },
 {"\21\21\21\25\25\25\12\00\00", 0x0057, "W" },
 {"\21\21\12\04\12\21\21\00\00", 0x0058, "X" },
 {"\21\21\12\04\04\04\04\00\00", 0x0059, "Y" },
 {"\37\01\02\04\10\20\37\00\00", 0x005a, "Z" },
 {"\17\10\10\10\10\10\17\00\00", 0x005b, "bracketleft" },
 {"\00\20\10\04\02\01\00\00\00", 0x005c, "backslash" },
 {"\36\02\02\02\02\02\36\00\00", 0x005d, "bracketright" },
 {"\04\12\21\00\00\00\00\00\00", 0x005e, "asciicircum" },
 {"\00\00\00\00\00\00\37\00\00", 0x005f, "underscore" },
 {"\04\04\02\00\00\00\00\00\00", 0x201b, "quotereversed" },
 {"\00\00\16\01\17\21\17\00\00", 0x0061, "a" },
 {"\20\20\36\21\21\21\36\00\00", 0x0062, "b" },
 {"\00\00\17\20\20\20\17\00\00", 0x0063, "c" },
 {"\01\01\17\21\21\21\17\00\00", 0x0064, "d" },
 {"\00\00\16\21\37\20\16\00\00", 0x0065, "e" },
 {"\02\04\04\16\04\04\04\00\00", 0x0066, "f" },
 {"\00\00\17\21\21\21\17\01\16", 0x0067, "g" },
 {"\20\20\36\21\21\21\21\00\00", 0x0068, "h" },
 {"\04\00\14\04\04\04\16\00\00", 0x0069, "i" },
 {"\04\00\04\04\04\04\04\04\10", 0x006a, "j" },
 {"\10\10\11\12\14\12\11\00\00", 0x006b, "k" },
 {"\14\04\04\04\04\04\16\00\00", 0x006c, "l" },
 {"\00\00\32\25\25\25\25\00\00", 0x006d, "m" },
 {"\00\00\36\21\21\21\21\00\00", 0x006e, "n" },
 {"\00\00\16\21\21\21\16\00\00", 0x006f, "o" },
 {"\00\00\36\21\21\21\36\20\20", 0x0070, "p" },
 {"\00\00\17\21\21\21\17\01\01", 0x0071, "q" },
 {"\00\00\13\14\10\10\10\00\00", 0x0072, "r" },
 {"\00\00\17\20\16\01\36\00\00", 0x0073, "s" },
 {"\04\04\16\04\04\04\02\00\00", 0x0074, "t" },
 {"\00\00\21\21\21\21\17\00\00", 0x0075, "u" },
 {"\00\00\21\21\12\12\04\00\00", 0x0076, "v" },
 {"\00\00\21\21\25\25\12\00\00", 0x0077, "w" },
 {"\00\00\21\12\04\12\21\00\00", 0x0078, "x" },
 {"\00\00\21\21\21\21\17\01\16", 0x0079, "y" },
 {"\00\00\37\02\04\10\37\00\00", 0x007a, "z" },
 {"\03\04\04\10\04\04\03\00\00", 0x007b, "braceleft" },
 {"\04\04\04\00\04\04\04\00\00", 0x00a6, "brokenbar" },
 {"\30\04\04\02\04\04\30\00\00", 0x007d, "braceright" },
 {"\10\25\02\00\00\00\00\00\00", 0x007e, "asciitilde" },
 {"\37\37\37\37\37\37\37\00\00", 0x25a0, "filledbox" },

 /* Extra characters found in the English (SAA5050) character set */
 {"\06\11\10\34\10\10\37\00\00", 0x00a3, "sterling" },
 {"\04\04\04\00\00\00\00\00\00", 0x0027, "quotesingle" },
 {"\00\04\10\37\10\04\00\00\00", 0x2190, "arrowleft" },
 {"\20\20\20\20\26\01\02\04\07", 0x00bd, "onehalf" },
 {"\00\04\02\37\02\04\00\00\00", 0x2192, "arrowright" },
 {"\00\04\16\25\04\04\00\00\00", 0x2191, "arrowup" },
 {"\00\00\00\37\00\00\00\00\00", 0x2014, "emdash" },
 {"\10\10\10\10\11\03\05\07\01", 0x00bc, "onequarter" },
 {"\12\12\12\12\12\12\12\00\00", 0x2016, "dblverticalbar" },
 {"\30\04\30\04\31\03\05\07\01", 0x00be, "threequarters" },
 {"\00\04\00\37\00\04\00\00\00", 0x00f7, "divide" },

 /* Extra characters found in the German (SAA5051) character set */
 {"\00\00\00\00\00\10\10\20\00", 0xf1d0, "comma.left" },
 {"\00\00\00\00\00\14\14\00\00", 0xf1d1, "period.large" },
 {"\00\00\00\10\00\00\10\00\00", 0xf1d2, "colon.leftsmall" },
 {"\00\00\10\00\00\10\10\20\00", 0xf1d3, "semicolon.left" },
 {"\16\21\01\02\04\00\04\00\00", 0xf1da, "question.open" },
 {"\16\21\20\16\21\16\01\21\16", 0x00a7, "section" },
 {"\36\11\11\11\11\11\36\00\00", 0xf1db, "D.serif" },
 {"\02\02\02\02\02\22\14\00\00", 0xf1dc, "J.narrow" },
 {"\10\10\10\10\10\10\17\00\00", 0xf1d5, "L.narrow" },
 {"\12\00\16\21\37\21\21\00\00", 0x00c4, "Adieresis" },
 {"\12\00\16\21\21\21\16\00\00", 0x00d6, "Odieresis" },
 {"\12\00\21\21\21\21\16\00\00", 0x00dc, "Udieresis" },
 {"\06\11\06\00\00\00\00\00\00", 0x00b0, "degree" },
 {"\04\00\14\04\04\04\04\04\10", 0xf1dd, "j.serif" },
 {"\00\04\16\04\04\04\02\00\00", 0xf1de, "t.small" },
 {"\12\00\16\01\17\21\17\00\00", 0x00e4, "adieresis" },
 {"\00\12\00\16\21\21\16\00\00", 0x00f6, "odieresis" },
 {"\00\12\00\21\21\21\17\00\00", 0x00fc, "udieresis" },
 {"\14\22\22\26\21\21\26\20\20", 0x00df, "germandbls" },

 /* Extra characters found in the Swedish (SAA5052) character set */
 {"\00\00\21\16\12\16\21\00\00", 0x00a4, "currency" },
 {"\02\04\37\20\36\20\37\00\00", 0x00c9, "Eacute" },
 {"\16\11\11\11\11\11\16\00\00", 0xf1d4, "D.narrow" },
 {"\04\00\16\21\37\21\21\00\00", 0x00c5, "Aring" },
 {"\02\04\16\21\37\20\16\00\00", 0x00e9, "eacute" },
 {"\04\00\16\01\17\21\17\00\00", 0x00e5, "aring" },

 /* Extra characters found in the Italian (SAA5053) character set */
 {"\00\00\17\20\20\20\17\02\04", 0x00e7, "ccedilla" },
 {"\10\04\21\21\21\21\17\00\00", 0x00f9, "ugrave" },
 {"\10\04\16\01\17\21\17\00\00", 0x00e0, "agrave" },
 {"\10\04\00\16\21\21\16\00\00", 0x00f2, "ograve" },
 {"\10\04\16\21\37\20\16\00\00", 0x00e8, "egrave" },
 {"\10\04\00\14\04\04\16\00\00", 0x00ec, "igrave" },

 /* Extra characters found in the Belgian (SAA5054) character set */
 {"\12\00\14\04\04\04\16\00\00", 0x00ef, "idieresis" },
 {"\12\00\16\21\37\20\16\00\00", 0x00eb, "edieresis" },
 {"\04\12\16\21\37\20\16\00\00", 0x00ea, "ecircumflex" },
 /* 5/13 from real SAA5054; datasheet has "\04\02\21\21\21\21\17\00\00". */
 {"\04\02\21\21\21\21\16\00\00", 0xf1d6, "ugrave.roundjoined" },
 {"\04\12\00\14\04\04\16\00\00", 0x00ee, "icircumflex" },
 {"\04\12\16\01\17\21\17\00\00", 0x00e2, "acircumflex" },
 {"\04\12\16\21\21\21\16\00\00", 0xf1d7, "ocircumflex.large" },
 {"\04\12\00\21\21\21\17\00\00", 0x00fb, "ucircumflex" },
 {"\00\00\17\20\20\20\17\02\06", 0xf1d8, "ccedilla.square" },

 /* Extra characters found in the Hebrew (SAA5056) character set */
 {"\00\21\11\25\22\21\21\00\00", U(05D0) }, /* alef */
 {"\00\16\02\02\02\02\37\00\00", U(05D1) }, /* bet */
 {"\00\03\01\01\03\05\11\00\00", U(05D2) }, /* gimel */
 {"\00\37\02\02\02\02\02\00\00", U(05D3) }, /* dalet */
 {"\00\37\01\01\21\21\21\00\00", U(05D4) }, /* he */
 {"\00\14\04\04\04\04\04\00\00", U(05D5) }, /* vav */
 {"\00\16\04\10\04\04\04\00\00", U(05D6) }, /* zayin */
 {"\00\37\21\21\21\21\21\00\00", U(05D7) }, /* het */
 {"\00\21\23\25\21\21\37\00\00", U(05D8) }, /* tet */
 {"\00\14\04\00\00\00\00\00\00", U(05D9) }, /* yod */
 {"\00\37\01\01\01\01\01\01\00", U(05DA) }, /* kaffinal */
 {"\00\37\01\01\01\01\37\00\00", U(05DB) }, /* kaf */
 {"\20\37\01\01\01\02\14\00\00", U(05DC) }, /* lamed */
 {"\00\37\21\21\21\21\37\00\00", U(05DD) }, /* memfinal */
 {"\00\26\11\21\21\21\27\00\00", U(05DE) }, /* mem */
 {"\00\14\04\04\04\04\04\04\04", U(05DF) }, /* nunfinal */
 {"\00\06\02\02\02\02\16\00\00", U(05E0) }, /* nun */
 {"\00\37\11\21\21\21\16\00\00", U(05E1) }, /* samekh */
 {"\00\11\11\11\11\12\34\00\00", U(05E2) }, /* ayin */
 {"\00\37\11\15\01\01\01\01\00", U(05E3) }, /* pefinal */
 {"\00\37\11\15\01\01\37\00\00", U(05E4) }, /* pe */
 {"\00\31\12\14\10\10\10\10\00", U(05E5) }, /* tsadifinal */
 {"\00\21\21\12\04\02\37\00\00", U(05E6) }, /* tsadi */
 {"\00\37\01\11\11\12\10\10\00", U(05E7) }, /* qof */
 {"\00\37\01\01\01\01\01\00\00", U(05E8) }, /* resh */
 {"\00\25\25\25\31\21\36\00\00", U(05E9) }, /* shin */
 {"\00\17\11\11\11\11\31\00\00", U(05EA) }, /* tav */
 {"\00\00\25\25\16\00\00\00\00", 0xf1d9, "oldsheqel" },

 /* Extra characters found in the Cyrillic (SAA5057) character set */
 {"\00\00\21\21\35\25\35\00\00", U(044B) }, /* yeru */
 {"\22\25\25\35\25\25\22\00\00", U(042E) }, /* Iu */
 {"\16\21\21\21\37\21\21\00\00", U(0410) }, /* A */
 {"\37\20\20\37\21\21\37\00\00", U(0411) }, /* Be */
 {"\22\22\22\22\22\22\37\01\00", U(0426) }, /* Tse */
 {"\06\12\12\12\12\12\37\21\00", U(0414) }, /* De */
 {"\37\20\20\36\20\20\37\00\00", U(0415) }, /* Ie */
 {"\04\37\25\25\25\37\04\00\00", U(0424) }, /* Ef */
 {"\37\20\20\20\20\20\20\00\00", U(0413) }, /* Ghe */
 {"\21\21\12\04\12\21\21\00\00", U(0425) }, /* Ha */
 {"\21\21\23\25\31\21\21\00\00", U(0418) }, /* I */
 {"\25\21\23\25\31\21\21\00\00", U(0419) }, /* Ishort */
 {"\21\22\24\30\24\22\21\00\00", U(041A) }, /* Ka */
 {"\07\11\11\11\11\11\31\00\00", U(041B) }, /* El */
 {"\21\33\25\25\21\21\21\00\00", U(041C) }, /* Em */
 {"\21\21\21\37\21\21\21\00\00", U(041D) }, /* En */
 {"\16\21\21\21\21\21\16\00\00", U(041E) }, /* O */
 {"\37\21\21\21\21\21\21\00\00", U(041F) }, /* Pe */
 {"\17\21\21\17\05\11\21\00\00", U(042F) }, /* Ya */
 {"\36\21\21\36\20\20\20\00\00", U(0420) }, /* Er */
 {"\16\21\20\20\20\21\16\00\00", U(0421) }, /* Es */
 {"\37\04\04\04\04\04\04\00\00", U(0422) }, /* Te */
 {"\21\21\21\37\01\01\37\00\00", U(0423) }, /* U */
 {"\25\25\25\16\25\25\25\00\00", U(0416) }, /* Zhe */
 {"\36\21\21\36\21\21\36\00\00", U(0412) }, /* Ve */
 {"\20\20\20\37\21\21\37\00\00", U(042C) }, /* Soft */
 {"\30\10\10\17\11\11\17\00\00", U(042A) }, /* Hard */
 {"\16\21\01\06\01\21\16\00\00", U(0417) }, /* Ze */
 {"\25\25\25\25\25\25\37\00\00", U(0428) }, /* Sha */
 {"\14\22\01\07\01\22\14\00\00", U(042D) }, /* E */
 {"\25\25\25\25\25\25\37\01\00", U(0429) }, /* Shcha */
 {"\21\21\21\37\01\01\01\00\00", U(0427) }, /* Che */
 {"\21\21\21\35\25\25\35\00\00", U(042B) }, /* Yeru */
 {"\00\00\22\25\35\25\22\00\00", U(044E) }, /* yu */
 {"\00\00\16\01\17\21\17\00\00", U(0430) }, /* a */
 {"\16\20\36\21\21\21\36\00\00", U(0431) }, /* be */
 {"\00\00\22\22\22\22\37\01\00", U(0446) }, /* tse */
 {"\00\00\06\12\12\12\37\21\00", U(0434) }, /* de */
 {"\00\00\16\21\37\20\16\00\00", U(0435) }, /* ie */
 {"\00\04\16\25\25\25\16\04\00", U(0444) }, /* ef */
 {"\00\00\37\20\20\20\20\00\00", U(0433) }, /* ghe */
 {"\00\00\21\12\04\12\21\00\00", U(0445) }, /* ha */
 {"\00\00\21\23\25\31\21\00\00", U(0438) }, /* i */
 {"\00\04\21\23\25\31\21\00\00", U(0439) }, /* ishort */
 {"\00\00\21\22\34\22\21\00\00", U(043A) }, /* ka */
 {"\00\00\07\11\11\11\31\00\00", U(043B) }, /* el */
 {"\00\00\21\33\25\21\21\00\00", U(043C) }, /* em */
 {"\00\00\21\21\37\21\21\00\00", U(043D) }, /* en */
 {"\00\00\16\21\21\21\16\00\00", U(043E) }, /* o */
 {"\00\00\37\21\21\21\21\00\00", U(043F) }, /* pe */
 {"\00\00\17\21\17\05\31\00\00", U(044F) }, /* ya */
 {"\00\00\36\21\21\21\36\20\20", U(0440) }, /* er */
 {"\00\00\16\21\20\21\16\00\00", U(0441) }, /* es */
 {"\00\00\37\04\04\04\04\00\00", U(0442) }, /* te */
 {"\00\00\21\21\21\21\17\01\16", U(0443) }, /* u */
 {"\00\00\25\25\16\25\25\00\00", U(0436) }, /* zhe */
 {"\00\00\36\21\36\21\36\00\00", U(0432) }, /* ve */
 {"\00\00\20\20\36\21\36\00\00", U(044C) }, /* soft */
 {"\00\00\30\10\16\11\16\00\00", U(044A) }, /* hard */
 {"\00\00\16\21\06\21\16\00\00", U(0437) }, /* ze */
 {"\00\00\25\25\25\25\37\00\00", U(0448) }, /* sha */
 {"\00\00\14\22\06\22\14\00\00", U(044D) }, /* e */
 {"\00\00\25\25\25\25\37\01\00", U(0449) }, /* shcha */
 {"\00\00\21\21\21\17\01\00\00", U(0447) }, /* che */

 /* Mosaic graphics common to all versions */
 {{0x00}, U(2003), /* em space */ MOS6 },
 {{0x15}, 0x258c, "lfblock",      MOS6 },
 {{0x2a}, 0x2590, "rtblock",      MOS6 },
 {{0x3f}, 0x2588, "block",        MOS6 },
#define M(u) { {0x ## u * 21 / 20 + 1}, U(1FB ## u), MOS6 }
 /* */  M(00), M(01), M(02), M(03), M(04), M(05), M(06),
 M(07), M(08), M(09), M(0A), M(0B), M(0C), M(0D), M(0E),
 M(0F), M(10), M(11), M(12), M(13),        M(14), M(15),
 M(16), M(17), M(18), M(19), M(1A), M(1B), M(1C), M(1D),
 M(1E), M(1F), M(20), M(21), M(22), M(23), M(24), M(25),
 M(26), M(27),        M(28), M(29), M(2A), M(2B), M(2C),
 M(2D), M(2E), M(2F), M(30), M(31), M(32), M(33), M(34),
 M(35), M(36), M(37), M(38), M(39), M(3A), M(3B),
#undef M
#define M(u) { {0x ## u - 0x50}, U(1CE ## u), SEP6 }
 /*  */ M(51), M(52), M(53), M(54), M(55), M(56), M(57),
 M(58), M(59), M(5A), M(5B), M(5C), M(5D), M(5E), M(5F),
 M(60), M(61), M(62), M(63), M(64), M(65), M(66), M(67),
 M(68), M(69), M(6A), M(6B), M(6C), M(6D), M(6E), M(6F),
 M(70), M(71), M(72), M(73), M(74), M(75), M(76), M(77),
 M(78), M(79), M(7A), M(7B), M(7C), M(7D), M(7E), M(7F),
 M(80), M(81), M(82), M(83), M(84), M(85), M(86), M(87),
 M(88), M(89), M(8A), M(8B), M(8C), M(8D), M(8E), M(8F),
#undef M

 /*
  * The other batch of glyphs were specially designed for this font.
  * These are kept sorted by Unicode code point.  Glyphs accessed
  * using OpenType features are next to their originals, even when
  * they also have Private Use Area mappings.
  */

 /* Basic Latin */
 ALIAS("quotesingle.curly", "quoteright"),
 {"\00\00\04\12\21\12\04\00\00", 0xf130, "zero.onum" },
 ALIAS("zero.zero", "zero_uniFE00"), /* slashed zero */
 {"\16\21\23\25\31\21\16\00\00", 0x0030 + VS(0xfe00), "zero_uniFE00" },
 {"\00\00\04\14\04\04\16\00\00", 0xf131, "one.onum" },
 {"\00\00\36\01\16\20\37\00\00", 0xf132, "two.onum" },
 {"\00\00\37\01\02\06\01\21\16", 0xf133, "three.onum" },
 {"\00\00\02\06\12\22\37\02\02", 0xf134, "four.onum" },
 {"\00\00\37\20\36\01\01\21\16", 0xf135, "five.onum" },
 {"\00\00\37\01\02\04\10\10\10", 0xf137, "seven.onum" },
 {"\00\00\16\21\21\17\01\02\14", 0xf139, "nine.onum" },
 ALIAS("A.c2sc", "uni1D00"),
 ALIAS("B.c2sc", "uni0299"),
 ALIAS("C.c2sc", "uni1D04"),
 ALIAS("D.c2sc", "uni1D05"),
 ALIAS("D.c2sc.serif", "uni1D05.serif"),
 ALIAS("D.c2sc.narrow", "uni1D05.narrow"),
 ALIAS("E.c2sc", "uni1D07"),
 ALIAS("F.c2sc", "uniA730"),
 ALIAS("G.c2sc", "uni0262"),
 ALIAS("H.c2sc", "uni029C"),
 ALIAS("I.c2sc", "uni026A"),
 ALIAS("J.c2sc", "uni1D0A"),
 ALIAS("J.c2sc.narrow", "uni1D0A.narrow"),
 ALIAS("K.c2sc", "uni1D0B"),
 ALIAS("L.c2sc", "uni029F"),
 ALIAS("L.c2sc.narrow", "uni029F.narrow"),
 ALIAS("M.c2sc", "uni1D0D"),
 ALIAS("N.c2sc", "uni0274"),
 ALIAS("O.c2sc", "uni1D0F"),
 ALIAS("P.c2sc", "uni1D18"),
 ALIAS("Q.c2sc", "uniA7AF"),
 ALIAS("R.c2sc", "uni0280"),
 ALIAS("S.c2sc", "uniA731"),
 ALIAS("T.c2sc", "uni1D1B"),
 ALIAS("U.c2sc", "uni1D1C"),
 ALIAS("V.c2sc", "uni1D20"),
 ALIAS("W.c2sc", "uni1D21"),
 {"\00\00\21\12\04\12\21\00\00", 0xf1c0, "X.c2sc" },
 ALIAS("Y.c2sc", "uni028F"),
 ALIAS("Z.c2sc", "uni1D22"),
 {"\10\04\02\00\00\00\00\00\00", 0x0060, "grave" },
 ALIAS("grave.curly", "quotereversed"),
 ALIAS("a.sc", "A.c2sc"),
 ALIAS("b.sc", "B.c2sc"),
 ALIAS("c.sc", "C.c2sc"),
 ALIAS("d.sc", "D.c2sc"),
 ALIAS("d.sc.serif", "D.c2sc.serif"),
 ALIAS("d.sc.narrow", "D.c2sc.narrow"),
 ALIAS("e.sc", "E.c2sc"),
 ALIAS("f.sc", "F.c2sc"),
 ALIAS("g.sc", "G.c2sc"),
 ALIAS("h.sc", "H.c2sc"),
 ALIAS("i.sc", "I.c2sc"),
 ALIAS("j.sc", "J.c2sc"),
 ALIAS("j.sc.narrow", "J.c2sc.narrow"),
 ALIAS("k.sc", "K.c2sc"),
 ALIAS("l.sc", "L.c2sc"),
 ALIAS("l.sc.narrow", "L.c2sc.narrow"),
 ALIAS("m.sc", "M.c2sc"),
 ALIAS("n.sc", "N.c2sc"),
 ALIAS("o.sc", "O.c2sc"),
 ALIAS("p.sc", "P.c2sc"),
 ALIAS("q.sc", "Q.c2sc"),
 ALIAS("r.sc", "R.c2sc"),
 ALIAS("s.sc", "S.c2sc"),
 ALIAS("t.sc", "T.c2sc"),
 ALIAS("u.sc", "U.c2sc"),
 ALIAS("v.sc", "V.c2sc"),
 ALIAS("w.sc", "W.c2sc"),
 ALIAS("x.sc", "X.c2sc"),
 ALIAS("y.sc", "Y.c2sc"),
 ALIAS("z.sc", "Z.c2sc"),
 {"\04\04\04\04\04\04\04\00\00", 0x007c, "bar" },
 ALIAS("bar.broken", "brokenbar"),

 /* Latin-1 supplement */
 {"\00\00\00\00\00\00\00\00\00", U(00A0) }, /* non-breaking space */
 {"\00\00\04\00\04\04\04\04\04", 0x00a1, "exclamdown" },
 {"\00\04\17\24\24\24\17\04\00", 0x00a2, "cent" },
 {"\21\12\37\04\37\04\04\00\00", 0x00a5, "yen" },
 {"\12\00\00\00\00\00\00\00\00", 0x00a8, "dieresis" },
 {"\16\21\25\33\31\33\25\21\16", 0x00a9, "copyright" },
 {"\16\01\17\21\17\00\37\00\00", 0x00aa, "ordfeminine" },
 {"\00\00\00\11\22\11\00\00\00", 0x00ab, "guillemotleft" },
 {"\00\00\37\01\01\00\00\00\00", 0x00ac, "logicalnot" },
 {"\00\00\00\16\00\00\00\00\00", U(00AD) }, /* soft hyphen */
 {"\16\21\35\33\33\35\33\21\16", 0x00ae, "registered" },
 {"\16\00\00\00\00\00\00\00\00", 0x00af, "macron" },
 {"\04\04\37\04\04\00\37\00\00", 0x00b1, "plusminus" },
 {"\14\02\04\10\16\00\00\00\00", 0x00b2, "twosuperior" },
 {"\14\02\14\02\14\00\00\00\00", 0x00b3, "threesuperior" },
 {"\02\04\10\00\00\00\00\00\00", 0x00b4, "acute" },
 {"\00\00\22\22\22\22\35\20\20", 0x00b5, "mu" },
 {"\15\25\25\15\05\05\05\00\00", 0x00b6, "paragraph" },
 {"\00\00\00\04\00\00\00\00\00", 0x00b7, "periodcentered" },
 {"\00\00\00\00\00\00\04\02\04", 0x00b8, "cedilla" },
 {"\00\00\00\00\00\00\04\02\06", 0xf200, "cedilla.square" },
 {"\04\14\04\04\16\00\00\00\00", 0x00b9, "onesuperior" },
 {"\16\21\21\21\16\00\37\00\00", 0x00ba, "ordmasculine" },
 {"\00\00\00\22\11\22\00\00\00", 0x00bb, "guillemotright" },
 {"\00\00\04\00\04\04\10\21\16", 0x00bf, "questiondown" },
 {"\00\00\04\00\04\10\20\21\16", 0xf211, "questiondown.open" },
 {"\10\04\16\21\37\21\21\00\00", 0x00c0, "Agrave" },
 {"\02\04\16\21\37\21\21\00\00", 0x00c1, "Aacute" },
 {"\04\12\16\21\37\21\21\00\00", 0x00c2, "Acircumflex" },
 {"\05\12\16\21\37\21\21\00\00", 0x00c3, "Atilde" },
 {"\17\24\24\26\34\24\27\00\00", 0x00c6, "AE" },
 ALIAS("AE.c2sc", "uni1D01"),
 {"\16\21\20\20\20\21\16\04\10", 0x00c7, "Ccedilla" },
 {"\16\21\20\20\20\21\16\04\14", 0xf201, "Ccedilla.square" },
 {"\00\00\16\21\20\21\16\04\10", 0xf190, "Ccedilla.c2sc" },
 {"\00\00\16\21\20\21\16\04\14", 0xf202, "Ccedilla.c2sc.square" },
 {"\10\04\37\20\36\20\37\00\00", 0x00c8, "Egrave" },
 {"\04\12\37\20\36\20\37\00\00", 0x00ca, "Ecircumflex" },
 {"\12\00\37\20\36\20\37\00\00", 0x00cb, "Edieresis" },
 {"\10\04\00\16\04\04\16\00\00", 0x00cc, "Igrave" },
 {"\02\04\00\16\04\04\16\00\00", 0x00cd, "Iacute" },
 {"\04\12\00\16\04\04\16\00\00", 0x00ce, "Icircumflex" },
 ALIAS("icircumflex.sc", "Icircumflex"),
 {"\12\00\16\04\04\04\16\00\00", 0x00cf, "Idieresis" },
 {"\16\11\11\35\11\11\16\00\00", 0x00d0, "Eth" },
 {"\36\11\11\35\11\11\36\00\00", 0xf221, "Eth.serif" },
 ALIAS("Eth.c2sc", "uni1D06"),
 ALIAS("Eth.c2sc.serif", "uni1D06.serif"),
 {"\05\12\00\31\25\23\21\00\00", 0x00d1, "Ntilde" },
 {"\10\04\16\21\21\21\16\00\00", 0x00d2, "Ograve" },
 {"\02\04\16\21\21\21\16\00\00", 0x00d3, "Oacute" },
 {"\04\12\16\21\21\21\16\00\00", 0x00d4, "Ocircumflex" },
 {"\05\12\16\21\21\21\16\00\00", 0x00d5, "Otilde" },
 {"\00\21\12\04\12\21\00\00\00", 0x00d7, "multiply" },
 {"\15\22\25\25\25\11\26\00\00", 0x00d8, "Oslash" },
 ALIAS("Oslash.c2sc", "oslash"),
 {"\10\04\21\21\21\21\16\00\00", 0x00d9, "Ugrave" },
 {"\02\04\21\21\21\21\16\00\00", 0x00da, "Uacute" },
 {"\04\12\00\21\21\21\16\00\00", 0x00db, "Ucircumflex" },
 {"\02\04\21\12\04\04\04\00\00", 0x00dd, "Yacute" },
 {"\20\36\21\21\36\20\20\00\00", 0x00de, "Thorn" },
 {"\00\00\20\36\21\36\20\00\00", 0xf1c2, "Thorn.c2sc" },
 ALIAS("germandbls.sc", "uni1E9E.c2sc"),
 ALIAS("agrave.sc", "Agrave"),
 {"\02\04\16\01\17\21\17\00\00", 0x00e1, "aacute" },
 ALIAS("aacute.sc", "Aacute"),
 ALIAS("acircumflex.sc", "Acircumflex"),
 {"\05\12\16\01\17\21\17\00\00", 0x00e3, "atilde" },
 ALIAS("atilde.sc", "Atilde"),
 ALIAS("adieresis.sc", "Adieresis"),
 ALIAS("aring.sc", "Aring"),
 {"\00\00\12\05\17\24\16\00\00", 0x00e6, "ae" },
 ALIAS("ae.sc", "AE.c2sc"),
 ALIAS("ccedilla.sc", "Ccedilla.c2sc"),
 ALIAS("ccedilla.sc.square", "Ccedilla.c2sc.square"),
 ALIAS("egrave.sc", "Egrave"),
 ALIAS("eacute.sc", "Eacute"),
 ALIAS("ecircumflex.sc", "Ecircumflex"),
 ALIAS("edieresis.sc", "Edieresis"),
 ALIAS("igrave.sc", "Igrave"),
 {"\02\04\00\14\04\04\16\00\00", 0x00ed, "iacute" },
 ALIAS("iacute.sc", "Iacute"),
 ALIAS("idieresis.sc", "Idieresis"),
 {"\32\04\12\01\17\21\16\00\00", 0x00f0, "eth" },
 ALIAS("eth.sc", "Eth.c2sc"),
 ALIAS("eth.sc.serif", "Eth.c2sc.serif"),
 {"\05\12\36\21\21\21\21\00\00", 0x00f1, "ntilde" },
 ALIAS("ntilde.sc", "Ntilde"),
 ALIAS("ograve.large", "Ograve"),
 ALIAS("ograve.sc", "Ograve"),
 {"\02\04\00\16\21\21\16\00\00", 0x00f3, "oacute" },
 ALIAS("oacute.large", "Oacute"),
 ALIAS("oacute.sc", "Oacute"),
 {"\04\12\00\16\21\21\16\00\00", 0x00f4, "ocircumflex" },
 ALIAS("ocircumflex.sc", "Ocircumflex"),
 {"\05\12\00\16\21\21\16\00\00", 0x00f5, "otilde" },
 ALIAS("otilde.large", "Otilde"),
 ALIAS("otilde.sc", "Otilde"),
 ALIAS("odieresis.large", "Odieresis"),
 ALIAS("odieresis.sc", "Odieresis"),
 {"\00\00\15\22\25\11\26\00\00", 0x00f8, "oslash" },
 ALIAS("ugrave.sc", "Ugrave"),
 {"\02\04\21\21\21\21\17\00\00", 0x00fa, "uacute" },
 ALIAS("uacute.sc", "Uacute"),
 ALIAS("ucircumflex.sc", "Ucircumflex"),
 ALIAS("udieresis.sc", "Udieresis"),
 {"\02\04\21\21\21\21\17\01\16", 0x00fd, "yacute" },
 ALIAS("yacute.sc", "Yacute"),
 {"\20\20\36\21\21\21\36\20\20", 0x00fe, "thorn" },
 ALIAS("thorn.sc", "Thorn.c2sc"),
 {"\12\00\21\21\21\21\17\01\16", 0x00ff, "ydieresis" },
 ALIAS("ydieresis.sc", "Ydieresis"),

 /* Latin extended-A */
 {"\16\00\16\21\37\21\21\00\00", 0x0100, "Amacron" },
 {"\16\00\16\01\17\21\17\00\00", 0x0101, "amacron" },
 ALIAS("amacron.sc", "Amacron"),
 {"\04\12\21\21\37\21\21\02\01", 0x0104, "Aogonek" },
 {"\00\00\04\12\21\37\21\02\01", 0xf191, "Aogonek.c2sc" },
 {"\00\00\16\01\17\21\17\02\01", 0x0105, "aogonek" },
 ALIAS("aogonek.sc", "Aogonek.c2sc"),
 {"\02\04\16\21\20\21\16\00\00", 0x0106, "Cacute" },
 {"\02\04\17\20\20\20\17\00\00", 0x0107, "cacute" },
 ALIAS("cacute.sc", "Cacute"),
 {"\04\12\16\21\20\21\16\00\00", 0x0108, "Ccircumflex" },
 {"\04\12\17\20\20\20\17\00\00", 0x0109, "ccircumflex" },
 ALIAS("ccircumflex.sc", "Ccircumflex"),
 {"\04\00\16\21\20\21\16\00\00", 0x010a, "Cdotaccent" },
 {"\04\00\17\20\20\20\17\00\00", 0x010b, "cdotaccent" },
 ALIAS("cdotaccent.sc", "Cdotaccent"),
 {"\12\04\16\21\20\21\16\00\00", 0x010c, "Ccaron" },
 {"\12\04\17\20\20\20\17\00\00", 0x010d, "ccaron" },
 ALIAS("ccaron.sc", "Ccaron"),
 {"\12\04\36\21\21\21\36\00\00", 0x010e, "Dcaron" },
 {"\12\04\36\11\11\11\36\00\00", 0xf223, "Dcaron.serif" },
 {"\12\04\16\11\11\11\16\00\00", 0xf233, "Dcaron.narrow" },
 {"\05\05\14\24\24\24\14\00\00", 0x010f, "dcaron" },
 ALIAS("dcaron.sc", "Dcaron"),
 ALIAS("dcaron.sc.serif", "Dcaron.serif"),
 ALIAS("dcaron.sc.narrow", "Dcaron.narrow"),
 UALIAS(0x0110, "Dcroat", "Eth"),
 ALIAS("Dcroat.serif", "Eth.serif"),
 UALIAS(0xf192, "Dcroat.c2sc", "Eth.c2sc"),
 ALIAS("Dcroat.c2sc.serif", "Eth.c2sc.serif"),
 {"\02\07\02\16\22\22\16\00\00", 0x0111, "dcroat" },
 ALIAS("dcroat.sc", "Dcroat.c2sc"),
 ALIAS("dcroat.sc.serif", "Dcroat.c2sc.serif"),
 {"\16\00\37\20\36\20\37\00\00", 0x0112, "Emacron" },
 {"\16\00\16\21\37\20\16\00\00", 0x0113, "emacron" },
 ALIAS("emacron.sc", "Emacron"),
 {"\04\00\37\20\36\20\37\00\00", 0x0116, "Edotaccent" },
 {"\04\00\16\21\37\20\16\00\00", 0x0117, "edotaccent" },
 ALIAS("edotaccent.sc", "Edotaccent"),
 {"\37\20\20\36\20\20\37\02\01", 0x0118, "Eogonek" },
 {"\00\00\37\20\36\20\37\02\01", 0xf193, "Eogonek.c2sc" },
 {"\00\00\16\21\37\20\16\02\01", 0x0119, "eogonek" },
 ALIAS("eogonek.sc", "Eogonek.c2sc"),
 {"\12\04\37\20\36\20\37\00\00", 0x011a, "Ecaron" },
 {"\12\04\16\21\37\20\16\00\00", 0x011b, "ecaron" },
 ALIAS("ecaron.sc", "Ecaron"),
 {"\04\12\17\20\23\21\17\00\00", 0x011c, "Gcircumflex" },
 {"\04\12\17\21\21\21\17\01\16", 0x011d, "gcircumflex" },
 ALIAS("gcircumflex.sc", "Gcircumflex"),
 {"\04\00\17\20\23\21\17\00\00", 0x0120, "Gdotaccent" },
 {"\04\00\17\21\21\21\17\01\16", 0x0121, "gdotaccent" },
 ALIAS("gdotaccent.sc", "Gdotaccent"),
 {"\16\21\20\20\23\21\17\04\10", U(0122) }, /* Gcommaaccent */
 {"\00\00\17\20\23\21\17\04\10", 0xf1cf, "uni0122.c2sc" },
 {"\02\04\17\21\21\21\17\01\16", U(0123) }, /* gcommaaccent */
 ALIAS("uni0123.sc", "uni0122.c2sc"),
 {"\21\37\21\37\21\21\21\00\00", 0x0126, "Hbar" },
 {"\00\00\21\37\21\37\21\00\00", 0xf194, "Hbar.c2sc" },
 {"\10\34\10\16\11\11\11\00\00", 0x0127, "hbar" },
 ALIAS("hbar.sc", "Hbar.c2sc"),
 {"\05\12\00\16\04\04\16\00\00", 0x0128, "Itilde" },
 {"\05\12\00\14\04\04\16\00\00", 0x0129, "itilde" },
 ALIAS("itilde.sc", "Itilde"),
 {"\16\00\16\04\04\04\16\00\00", 0x012a, "Imacron" },
 {"\16\00\14\04\04\04\16\00\00", 0x012b, "imacron" },
 ALIAS("imacron.sc", "Imacron"),
 {"\16\04\04\04\04\04\16\04\02", 0x012e, "Iogonek" },
 {"\00\00\16\04\04\04\16\04\02", 0xf195, "Iogonek.c2sc" },
 {"\04\00\14\04\04\04\16\04\02", 0x012f, "iogonek" },
 ALIAS("iogonek.sc", "Iogonek.c2sc"),
 {"\04\00\16\04\04\04\16\00\00", 0x0130, "Idotaccent" },
 {"\00\00\14\04\04\04\16\00\00", 0x0131, "dotlessi" },
 ALIAS("dotlessi.sc", "I.c2sc"),
 {"\21\21\21\21\21\25\22\00\00", 0x0132, "IJ" },
 {"\00\00\21\21\21\25\22\00\00", 0xf196, "IJ.c2sc" },
 {"\11\00\31\11\11\11\35\01\02", 0x0133, "ij" },
 {"\11\00\33\11\11\11\35\01\02", 0xf21d, "ij.serif" },
 ALIAS("ij.sc", "IJ.c2sc"),
 {"\02\05\00\02\02\22\14\00\00", 0x0134, "Jcircumflex" },
 {"\04\12\00\04\04\04\04\04\10", 0x0135, "jcircumflex" },
 {"\04\12\00\14\04\04\04\04\10", 0xf21e, "jcircumflex.serif" },
 ALIAS("jcircumflex.sc", "Jcircumflex"),
 {"\21\21\22\34\22\21\25\04\10", U(0136) }, /* Kcommaaccent */
 {"\00\00\21\22\34\22\21\04\10", 0xf180, "uni0136.c2sc" },
 {"\20\20\21\22\34\22\21\04\10", U(0137) }, /* kcommaaccent */
 ALIAS("uni0137.sc", "uni0136.c2sc"),
 {"\00\00\21\22\34\22\21\00\00", 0x0138, "kgreenlandic" },
 {"\20\20\20\20\20\20\37\04\10", U(013B) }, /* Lcommaaccent */
 {"\10\10\10\10\10\10\17\02\04", 0xf213, "uni013B.narrow" },
 {"\00\00\20\20\20\20\37\04\10", 0xf181, "uni013B.c2sc" },
 {"\00\00\10\10\10\10\17\02\04", 0xf214, "uni013B.c2sc.narrow" },
 {"\14\04\04\04\04\04\16\04\10", U(013C) }, /* lcommaaccent */
 ALIAS("uni013C.sc", "uni013B.c2sc"),
 ALIAS("uni013C.sc.narrow", "uni013B.c2sc.narrow"),
 {"\22\22\24\20\20\20\37\00\00", 0x013d, "Lcaron" },
 {"\11\11\12\10\10\10\17\00\00", 0xf215, "Lcaron.narrow" },
 {"\00\00\22\22\24\20\37\00\00", 0xf197, "Lcaron.c2sc" },
 {"\00\00\11\11\12\10\17\00\00", 0xf216, "Lcaron.c2sc.narrow" },
 {"\31\11\12\10\10\10\34\00\00", 0x013e, "lcaron" },
 ALIAS("lcaron.sc", "Lcaron.c2sc"),
 ALIAS("lcaron.sc.narrow", "Lcaron.c2sc.narrow"),
 {"\20\20\20\22\20\20\37\00\00", 0x013f, "Ldot" },
 {"\20\20\20\22\20\20\36\00\00", 0xf217, "Ldot.narrow" },
 {"\00\00\20\22\20\20\37\00\00", 0xf198, "Ldot.c2sc" },
 {"\00\00\20\22\20\20\36\00\00", 0xf218, "Ldot.c2sc.narrow" },
 {"\30\10\10\11\10\10\34\00\00", 0x0140, "ldot" },
 ALIAS("ldot.sc", "Ldot.c2sc"),
 ALIAS("ldot.sc.narrow", "Ldot.c2sc.narrow"),
 {"\10\10\14\30\10\10\17\00\00", 0x0141, "Lslash" },
 UALIAS(0xf199, "Lslash.c2sc", "uni1D0C"),
 {"\14\04\06\14\04\04\16\00\00", 0x0142, "lslash" },
 ALIAS("lslash.sc", "Lslash.c2sc"),
 {"\02\04\21\31\25\23\21\00\00", 0x0143, "Nacute" },
 {"\02\04\36\21\21\21\21\00\00", 0x0144, "nacute" },
 ALIAS("nacute.sc", "Nacute"),
 {"\21\21\31\25\23\21\25\04\10", U(0145) }, /* Ncommaaccent */
 {"\00\00\31\25\23\21\25\04\10", 0xf182, "uni0145.c2sc" },
 {"\00\00\36\21\21\21\25\04\10", U(0146) }, /* ncommaaccent */
 ALIAS("uni0146.sc", "uni0145.c2sc"),
 {"\12\04\21\31\25\23\21\00\00", 0x0147, "Ncaron" },
 {"\12\04\36\21\21\21\21\00\00", 0x0148, "ncaron" },
 ALIAS("ncaron.sc", "Ncaron"),
 {"\20\20\26\05\05\05\05\00\00", 0x0149, "napostrophe" },
 {"\21\21\31\25\23\21\21\01\16", 0x014a, "Eng" },
 {"\00\00\21\31\25\23\21\01\16", 0xf19a, "Eng.c2sc" },
 {"\00\00\36\21\21\21\21\01\16", 0x014b, "eng" },
 ALIAS("eng.sc", "Eng.c2sc"),
 {"\16\00\16\21\21\21\16\00\00", 0x014c, "Omacron" },
 {"\00\16\00\16\21\21\16\00\00", 0x014d, "omacron" },
 ALIAS("omacron.large", "Omacron"),
 ALIAS("omacron.sc", "Omacron"),
 {"\17\24\24\26\24\24\17\00\00", 0x0152, "OE" },
 ALIAS("OE.c2sc", "uni0276"),
 {"\00\00\12\25\27\24\13\00\00", 0x0153, "oe" },
 ALIAS("oe.sc", "OE.c2sc"),
 {"\02\04\36\21\36\22\21\00\00", 0x0154, "Racute" },
 {"\01\02\13\14\10\10\10\00\00", 0x0155, "racute" },
 ALIAS("racute.sc", "Racute"),
 {"\36\21\21\36\22\21\25\04\10", U(0156) }, /* Rcommaaccent */
 {"\00\00\36\21\36\21\25\04\10", 0xf183, "uni0156.c2sc" },
 {"\00\00\13\14\10\10\12\02\04", U(0157) }, /* rcommaaccent */
 ALIAS("uni0157.sc", "uni0156.c2sc"),
 {"\12\04\36\21\36\22\21\00\00", 0x0158, "Rcaron" },
 {"\05\02\13\14\10\10\10\00\00", 0x0159, "rcaron" },
 ALIAS("rcaron.sc", "Rcaron"),
 {"\02\04\17\20\16\01\36\00\00", 0x015a, "Sacute" },
 {"\02\04\16\20\16\01\16\00\00", 0x015b, "sacute" },
 ALIAS("sacute.sc", "Sacute"),
 {"\16\21\20\16\01\21\16\04\10", 0x015e, "Scedilla" },
 {"\16\21\20\16\01\21\16\04\14", 0xf203, "Scedilla.square" },
 ALIAS("Scedilla.c2sc", "scedilla"),
 ALIAS("Scedilla.c2sc.square", "scedilla.square"),
 {"\00\00\17\20\16\01\36\04\10", 0x015f, "scedilla" },
 {"\00\00\17\20\16\01\36\04\14", 0xf204, "scedilla.square" },
 {"\12\04\17\20\16\01\36\00\00", 0x0160, "Scaron" },
 {"\12\04\16\20\16\01\16\00\00", 0x0161, "scaron" },
 ALIAS("scaron.sc", "Scaron"),
 {"\37\04\04\04\04\04\04\02\04", U(0162) }, /* Tcedilla */
 {"\37\04\04\04\04\04\04\02\06", 0xf205, "uni0162.square" },
 {"\00\00\37\04\04\04\04\02\04", 0xf19b, "uni0162.c2sc" },
 {"\00\00\37\04\04\04\04\02\06", 0xf206, "uni0162.c2sc.square" },
 {"\04\04\16\04\04\05\02\02\04", U(0163) }, /* tcedilla */
 {"\04\04\16\04\04\05\02\02\06", 0xf207, "uni0163.square" },
 {"\00\04\16\04\04\05\02\02\04", 0xf240, "uni0163.small" },
 {"\00\04\16\04\04\05\02\02\06", 0xf208, "uni0163.small.square" },
 ALIAS("uni0163.square.small", "uni0163.small.square"),
 ALIAS("uni0163.sc", "uni0162.c2sc"),
 ALIAS("uni0163.sc.square", "uni0162.c2sc.square"),
 {"\12\04\37\04\04\04\04\00\00", 0x0164, "Tcaron" },
 {"\11\11\34\10\10\10\04\00\00", 0x0165, "tcaron" },
 {"\01\11\34\10\10\10\04\00\00", 0xf241, "tcaron.small" },
 ALIAS("tcaron.sc", "Tcaron"),
 {"\37\04\04\16\04\04\04\00\00", 0x0166, "Tbar" },
 {"\00\00\37\04\16\04\04\00\00", 0xf1cd, "Tbar.c2sc" },
 {"\04\16\04\16\04\04\02\00\00", 0x0167, "tbar" },
 {"\00\04\16\04\16\04\02\00\00", 0xf242, "tbar.small" },
 ALIAS("tbar.sc", "Tbar.c2sc"),
 {"\16\00\21\21\21\21\16\00\00", 0x016a, "Umacron" },
 {"\00\16\00\21\21\21\17\00\00", 0x016b, "umacron" },
 ALIAS("umacron.sc", "Umacron"),
 {"\04\00\21\21\21\21\16\00\00", 0x016e, "Uring" },
 {"\04\00\21\21\21\21\17\00\00", 0x016f, "uring" },
 {"\21\21\21\21\21\21\16\04\02", 0x0172, "Uogonek" },
 {"\00\00\21\21\21\21\16\04\02", 0xf19c, "Uogonek.c2sc" },
 {"\00\00\21\21\21\21\17\02\01", 0x0173, "uogonek" },
 ALIAS("uogonek.sc", "Uogonek.c2sc"),
 {"\04\12\00\25\25\25\12\00\00", 0x0174, "Wcircumflex" },
 {"\04\12\00\21\25\25\12\00\00", 0x0175, "wcircumflex" },
 ALIAS("wcircumflex.sc", "Wcircumflex"),
 {"\04\12\00\21\12\04\04\00\00", 0x0176, "Ycircumflex" },
 {"\04\12\00\21\21\21\17\01\16", 0x0177, "ycircumflex" },
 ALIAS("ycircumflex.sc", "Ycircumflex"),
 {"\12\00\21\12\04\04\04\00\00", 0x0178, "Ydieresis" },
 {"\02\04\37\01\16\20\37\00\00", 0x0179, "Zacute" },
 {"\02\04\37\02\04\10\37\00\00", 0x017a, "zacute" },
 ALIAS("zacute.sc", "Zacute"),
 {"\04\00\37\01\16\20\37\00\00", 0x017b, "Zdotaccent" },
 {"\04\00\37\02\04\10\37\00\00", 0x017c, "zdotaccent" },
 ALIAS("zdotaccent.sc", "Zdotaccent"),
 {"\12\04\37\01\16\20\37\00\00", 0x017d, "Zcaron" },
 {"\12\04\37\02\04\10\37\00\00", 0x017e, "zcaron" },
 ALIAS("zcaron.sc", "Zcaron"),
 {"\02\04\04\04\04\04\04\00\00", 0x017f, "longs" },
 ALIAS("longs.sc", "S.c2sc"),

 /* Latin extended-B */
 {"\10\10\16\11\35\11\16\00\00", U(0180) }, /* b with stroke */
 ALIAS("uni0180.sc", "uni0243.c2sc"),
 {"\36\20\20\36\21\21\36\00\00", U(0182) }, /* B with topbar */
 {"\00\00\36\20\36\21\36\00\00", 0xf1ce, "uni0182.c2sc" },
 {"\37\20\36\21\21\21\36\00\00", U(0183) }, /* b with topbar */
 ALIAS("uni0183.sc", "uni0182.c2sc"),
 {"\16\21\01\01\01\21\16\00\00", U(0186) }, /* open O */
 {"\00\00\16\21\01\21\16\00\00", 0xf187, "uni0186.c2sc" },
 {"\03\02\17\20\20\20\17\00\00", U(0188) }, /* Hooktop C */
 UALIAS(U(0189), "Eth"), /* African D */
 ALIAS("uni0189.serif", "Eth.serif"),
 ALIAS("uni0189.c2sc", "Eth.c2sc"),
 ALIAS("uni0189.c2sc.serif", "Eth.c2sc.serif"),
 {"\37\01\01\17\01\01\37\00\00", U(018E) }, /* reversed E */
 ALIAS("uni018E.c2sc", "uni2C7B"),
 {"\16\21\01\37\21\21\16\00\00", U(018F) }, /* Schwa */
 ALIAS("uni018F.c2sc", "uni0259"),
 {"\16\21\20\14\20\21\16\00\00", U(0190) }, /* Open E */
 {"\00\00\16\21\14\21\16\00\00", 0xf188, "uni0190.c2sc" },
 {"\02\04\04\16\04\04\04\04\10", 0x0192, "florin" },
 {"\21\21\21\12\12\04\12\12\04", U(0194) }, /* Latin Gamma */
 {"\00\00\21\21\12\04\12\12\04", 0xf189, "uni0194.c2sc" },
 {"\16\04\04\04\04\04\02\00\00", U(0196) }, /* Latin Iota */
 {"\00\00\16\04\04\04\02\00\00", 0xf18a, "uni0196.c2sc" },
 {"\16\04\04\16\04\04\16\00\00", U(0197) }, /* I with stroke */
 ALIAS("uni0197.c2sc", "uni1D7B"),
 {"\06\10\11\12\14\12\11\00\00", U(0199) }, /* Hooktop K */
 {"\30\04\16\04\12\12\21\00\00", U(019B) }, /* Barred lambda */
 {"\11\11\15\13\11\11\11\10\20", U(019D) }, /* N with left hook */
 {"\00\00\11\15\13\11\11\10\20", 0xf18b, "uni019D.c2sc" },
 {"\00\00\36\21\21\21\21\01\01", U(019E) }, /* N, right leg */
 {"\14\20\36\21\21\21\36\20\20", U(01A5) }, /* Hooktop P */
 {"\16\21\01\16\20\21\16\00\00", U(01A7) }, /* Tone 2 (reversed S) */
 ALIAS("uni01A7.c2sc", "uni01A8"),
 {"\00\00\36\01\16\20\17\00\00", U(01A8) }, /* tone 2 (reversed s) */
 {"\04\04\16\04\04\04\02\14\00", U(01AB) }, /* Left-hook T */
 {"\00\04\16\04\04\04\02\14\00", 0xf243, "uni01AB.small" },
 {"\02\04\16\04\04\04\02\00\00", U(01AD) }, /* Hooktop T */
 {"\33\12\21\21\21\21\16\00\00", U(01B1) }, /* Latin Upsilon */
 ALIAS("uni01B1.c2sc", "uni028A"),
 {"\22\21\21\21\21\22\14\00\00", U(01B2) }, /* V with hook */
 ALIAS("uni01B2.c2sc", "uni028B"),
 {"\37\02\04\16\01\21\16\00\00", U(01B7) }, /* Ezh */
 ALIAS("uni01B7.c2sc", "uni1D23"),
 {"\16\21\05\02\15\20\37\00\00", U(01BB) }, /* Barred two */
 {"\04\04\04\04\04\04\04\04\04", U(01C0) }, /* Pipe */
 {"\12\12\12\12\12\12\12\12\12", U(01C1) }, /* Double pipe */
 {"\04\04\04\37\04\37\04\04\04", U(01C2) }, /* Double-barred pipe */
 {"\04\04\04\04\04\00\04\00\00", U(01C3) }, /* Exclamation point */
 {"\12\04\16\21\37\21\21\00\00", U(01CD) }, /* Acaron */
 {"\12\04\16\01\17\21\17\00\00", U(01CE) }, /* acaron */
 ALIAS("uni01CE.sc", "uni01CD"),
 {"\12\04\00\16\04\04\16\00\00", U(01CF) }, /* Icaron */
 {"\12\04\00\14\04\04\16\00\00", U(01D0) }, /* icaron */
 ALIAS("uni01D0.sc", "uni01CF"),
 {"\12\04\16\21\21\21\16\00\00", U(01D1) }, /* Ocaron */
 {"\12\04\00\16\21\21\16\00\00", U(01D2) }, /* ocaron */
 ALIAS("uni01D2.sc", "uni01D1"),
 {"\12\04\21\21\21\21\16\00\00", U(01D3) }, /* Ucaron */
 {"\12\04\21\21\21\21\17\00\00", U(01D4) }, /* ucaron */
 ALIAS("uni01D4.sc", "uni01D3"),
 {"\16\00\12\00\21\21\16\00\00", U(01D5) }, /* Udieresismacron */
 {"\16\00\12\00\21\21\17\00\00", U(01D6) }, /* udieresismacron */
 ALIAS("uni01D6.sc", "uni01D5"),
 {"\02\04\21\00\21\21\16\00\00", U(01D7) }, /* Udieresisacute */
 {"\02\04\21\00\21\21\17\00\00", U(01D8) }, /* udieresisacute */
 ALIAS("uni01D8.sc", "uni01D7"),
 {"\12\04\21\00\21\21\16\00\00", U(01D9) }, /* Udieresiscaron */
 {"\12\04\21\00\21\21\17\00\00", U(01DA) }, /* Udieresiscaron */
 ALIAS("uni01DA.sc", "uni01D9"),
 {"\10\04\21\00\21\21\16\00\00", U(01DB) }, /* Udieresisgrave */
 {"\10\04\21\00\21\21\17\00\00", U(01DC) }, /* Udieresisgrave */
 ALIAS("uni01DC.sc", "uni01DB"),
 {"\00\00\16\01\37\21\16\00\00", U(01DD) }, /* turned e */
 ALIAS("uni01DD.sc", "uni018E.c2sc"),
 {"\16\00\17\24\26\34\27\00\00", U(01E2) }, /* AEmacron */
 {"\16\00\12\05\17\24\16\00\00", U(01E3) }, /* aemacron */
 ALIAS("uni01E3.sc", "uni01E2"),
 {"\12\04\00\04\04\04\04\04\10", U(01F0) }, /* J wedge */
 {"\12\04\00\14\04\04\04\04\10", 0xf21f, "uni01F0.serif" },
 {"\10\04\21\31\25\23\21\00\00", U(01F8) }, /* Ngrave */
 {"\10\04\36\21\21\21\21\00\00", U(01F9) }, /* ngrave */
 ALIAS("uni01F9.sc", "uni01F8"),
 {"\16\21\20\16\01\21\16\04\10", U(0218) }, /* Scommaaccent */
 ALIAS("uni0218.c2sc", "uni0219"),
 {"\00\00\17\20\16\01\36\04\10", U(0219) }, /* scommaaccent */
 {"\37\04\04\04\04\04\00\04\10", U(021A) }, /* Tcommaaccent */
 {"\00\00\37\04\04\04\00\04\10", 0xf184, "uni021A.c2sc" },
 {"\04\04\16\04\04\02\00\04\10", U(021B) }, /* tcommaaccent */
 {"\00\04\16\04\04\02\00\04\10", 0xf244, "uni021B.small" },
 ALIAS("uni021B.sc", "uni021A.c2sc"),
 {"\37\20\20\36\20\20\37\04\10", U(0228) }, /* Ecedilla */
 {"\37\20\20\36\20\20\37\04\14", 0xf209, "uni0228.square" },
 {"\00\00\37\20\36\20\37\04\10", 0xf185, "uni0228.c2sc" },
 {"\00\00\37\20\36\20\37\04\14", 0xf20a, "uni0228.c2sc.square" },
 {"\00\00\16\21\37\20\16\04\10", U(0229) }, /* ecedilla */
 {"\00\00\16\21\37\20\16\04\14", 0xf20b, "uni0229.square" },
 ALIAS("uni0229.sc", "uni0228.c2sc"),
 ALIAS("uni0229.sc.square", "uni0228.c2sc.square"),
 {"\16\00\21\12\04\04\04\00\00", U(0232) }, /* Ymacron */
 {"\16\00\21\21\21\21\17\01\16", U(0233) }, /* ymacron */
 ALIAS("uni0233.sc", "uni0232"),
 {"\00\00\04\04\04\04\04\04\10", U(0237) }, /* dotlessj */
 {"\00\00\14\04\04\04\04\04\10", 0xf22d, "uni0237.serif" },
 {"\00\00\17\20\16\01\36\04\03", U(023F) }, /* s with swash tail */
 {"\00\00\37\02\04\10\30\04\03", U(0240) }, /* z with swash tail */
 ALIAS("uni0240.sc", "uni2C7E.c2sc"),
 {"\16\11\16\11\35\11\16\00\00", U(0243) }, /* B with stroke */
 ALIAS("uni0243.c2sc", "uni1D03"),
 {"\04\04\12\12\21\21\21\00\00", U(0245) }, /* turned V */
 ALIAS("uni0245.c2sc", "uni028C"),

 /* IPA extensions */
 {"\00\00\36\21\36\20\16\00\00", U(0250) }, /* Turned a */
 ALIAS("uni0250.sc", "uni2C6F.c2sc"),
 {"\00\00\17\21\21\23\15\00\00", U(0251) }, /* Script a */
 {"\00\00\26\31\21\21\36\00\00", U(0252) }, /* Turned script a */
 {"\14\20\36\21\21\21\36\00\00", U(0253) }, /* Hooktop a */
 {"\00\00\36\01\01\01\36\00\00", U(0254) }, /* Open o */
 ALIAS("uni0254.sc", "uni0186.c2sc"),
 {"\00\00\17\20\22\25\16\04\00", U(0255) }, /* Curly-tail c */
 {"\02\02\16\22\22\22\16\02\01", U(0256) }, /* Right-tail d */
 ALIAS("uni0256.sc", "uni0189.c2sc"),
 ALIAS("uni0256.sc.serif", "uni0189.c2sc.serif"),
 {"\01\02\16\22\22\22\16\00\00", U(0257) }, /* Hooktop d */
 {"\00\00\16\21\37\01\16\00\00", U(0258) }, /* Reversed e */
 {"\00\00\16\01\37\21\16\00\00", U(0259) }, /* Schwa */
 {"\00\00\30\04\35\26\10\00\00", U(025A) }, /* Right-hook schwa */
 {"\00\00\17\20\16\20\17\00\00", U(025B) }, /* epsilon */
 ALIAS("uni025B.sc", "uni0190.c2sc"),
 {"\00\00\36\01\16\01\36\00\00", U(025C) }, /* Reversed epsilon */
 ALIAS("uni025C.sc", "uniA7AB.c2sc"),
 {"\00\00\30\04\31\06\30\00\00", U(025D) }, /* Right-hook rev epsilon */
 {"\00\00\16\21\26\21\16\00\00", U(025E) }, /* Closed reversed epsilon */
 {"\00\00\04\04\04\16\04\04\10", U(025F) }, /* Barred dotless j */
 {"\01\02\16\22\22\22\16\02\14", U(0260) }, /* Hooktop g */
 {"\00\00\17\21\21\21\17\01\16", U(0261) }, /* Opentail g */
 {"\00\00\17\20\23\21\17\00\00", U(0262) }, /* Small capital G */
 {"\00\00\21\12\12\04\12\12\04", U(0263) }, /* gamma */
 ALIAS("uni0263.sc", "uni0194.c2sc"),
 {"\00\00\33\04\12\12\04\00\00", U(0264) }, /* ram's horns */
 {"\00\00\21\21\21\21\17\01\01", U(0265) }, /* Turned h */
 {"\14\20\36\21\21\21\21\00\00", U(0266) }, /* Hooktop h */
 ALIAS("uni0266.sc", "uniA7AA.c2sc"),
 {"\14\20\36\21\21\21\21\01\06", U(0267) }, /* Hooktop heng */
 {"\04\00\14\04\16\04\16\00\00", U(0268) }, /* Barred i */
 ALIAS("uni0268.sc", "uni0197.c2sc"),
 {"\00\00\14\04\04\04\02\00\00", U(0269) }, /* iota */
 ALIAS("uni0269.sc", "uni0196.c2sc"),
 {"\00\00\16\04\04\04\16\00\00", U(026A) }, /* Small capital I */
 ALIAS("uni026A.sc", "uniA792.c2sc"),
 {"\14\04\15\26\04\04\16\00\00", U(026B) }, /* l with tilde */
 {"\14\04\14\25\16\04\16\00\00", U(026C) }, /* Belted L */
 ALIAS("uni026C.sc", "uniA7AD.c2sc"),
 {"\14\04\04\04\04\04\04\04\02", U(026D) }, /* Right-tail L */
 {"\30\10\17\11\12\11\35\01\06", U(026E) }, /* l-ezh ligature */
 {"\00\00\25\25\25\25\13\00\00", U(026F) }, /* Turned m */
 {"\00\00\25\25\25\25\13\01\01", U(0270) }, /* Turned m, right tail */
 {"\00\00\32\25\25\25\25\01\02", U(0271) }, /* Left-tail m (at right) */
 {"\00\00\16\11\11\11\11\10\20", U(0272) }, /* Left-tail n (at left) */
 ALIAS("uni0272.sc", "uni019D.c2sc"),
 {"\00\00\34\22\22\22\22\02\01", U(0273) }, /* Right-tail n */
 {"\00\00\21\31\25\23\21\00\00", U(0274) }, /* Small capital N */
 {"\00\00\16\21\37\21\16\00\00", U(0275) }, /* Barred o */
 {"\00\00\17\24\27\24\17\00\00", U(0276) }, /* Small capital O-E ligature */
 {"\00\00\16\21\25\25\12\00\00", U(0277) }, /* Closed omega */
 {"\04\04\16\25\25\25\16\04\04", U(0278) }, /* phi */
 {"\00\00\02\02\02\06\32\00\00", U(0279) }, /* Turned r */
 {"\02\02\02\02\02\06\32\00\00", U(027A) }, /* Turned long-leg r */
 {"\00\00\02\02\02\06\32\02\01", U(027B) }, /* Turned r, right tail */
 {"\00\00\13\14\10\10\10\10\10", U(027C) }, /* Long-leg r */
 {"\00\00\13\14\10\10\10\10\04", U(027D) }, /* Right-tail r */
 {"\00\00\06\11\10\10\10\00\00", U(027E) }, /* Fish-hook r */
 {"\00\00\36\21\36\22\21\00\00", U(0280) }, /* Small capital R */
 {"\00\00\21\22\36\21\36\00\00", U(0281) }, /* Inverted small capital R */
 {"\00\00\17\20\16\01\36\20\10", U(0282) }, /* Right-tail s (at left) */
 {"\02\04\04\04\04\04\04\04\10", U(0283) }, /* esh */
 {"\02\04\04\04\04\16\04\04\10", U(0284) }, /* Hooktop barred dotless j */
 {"\02\04\04\04\04\16\24\24\10", U(0286) }, /* Curly-tail esh */
 {"\00\00\10\04\04\04\16\04\04", U(0287) }, /* Turned t */
 {"\00\00\10\04\04\04\16\04\00", 0xf245, "uni0287.small" },
 ALIAS("uni0287.sc", "uniA7B1.c2sc"),
 {"\04\04\16\04\04\04\04\04\02", U(0288) }, /* Right-tail t */
 {"\00\04\16\04\04\04\04\04\02", 0xf246, "uni0288.small" },
 {"\00\00\12\37\12\12\06\00\00", U(0289) }, /* Barred u */
 {"\00\00\33\12\21\21\16\00\00", U(028A) }, /* upsilon */
 {"\00\00\22\21\21\22\14\00\00", U(028B) }, /* Cursive v (or v with hook) */
 {"\00\00\04\12\12\21\21\00\00", U(028C) }, /* Turned v */
 {"\00\00\12\25\25\21\21\00\00", U(028D) }, /* Turned w */
 {"\16\20\34\22\21\21\21\00\00", U(028E) }, /* Turned y */
 {"\00\00\21\12\04\04\04\00\00", U(028F) }, /* Small capital Y */
 {"\00\00\36\04\10\20\36\02\01", U(0290) }, /* Right-tail z */
 {"\00\00\37\02\04\10\36\05\02", U(0291) }, /* Curly-tail z */
 {"\00\00\37\02\04\16\01\21\16", U(0292) }, /* ezh; Tailed z */
 ALIAS( "uni0292.sc", "uni01B7.c2sc"),
 {"\00\00\37\02\04\02\11\25\16", U(0293) }, /* Curly-tail ezh */
 {"\16\21\01\06\04\04\04\00\00", U(0294) }, /* Glottal stop */
 {"\16\21\20\14\04\04\04\00\00", U(0295) }, /* Reversed glottal stop */
 {"\04\04\04\06\01\21\16\00\00", U(0296) }, /* Inverted glottal stop */
 {"\16\21\20\20\20\20\20\21\16", U(0297) }, /* Stretched c */
 {"\16\21\21\25\21\21\16\00\00", U(0298) }, /* Bull's eye */
 {"\00\00\36\21\36\21\36\00\00", U(0299) }, /* Small capital B */
 {"\00\00\16\21\15\21\16\00\00", U(029A) }, /* Closed epsilon */
 {"\01\02\16\20\26\22\16\00\00", U(029B) }, /* Hooktop small capital G */
 {"\00\00\21\21\37\21\21\00\00", U(029C) }, /* Small capital H */
 {"\04\00\04\04\04\16\24\24\10", U(029D) }, /* Curly-tail j */
 {"\04\00\14\04\04\16\24\24\10", 0xf22e, "uni029D.serif" },
 ALIAS("uni029D.sc", "uniA7B2.c2sc"),
 {"\00\00\22\12\06\12\22\02\02", U(029E) }, /* Turned k */
 ALIAS("uni029E.sc", "uniA7B0.c2sc"),
 {"\00\00\20\20\20\20\37\00\00", U(029F) }, /* Small capital L */
 {"\00\00\10\10\10\10\17\00\00", 0xf219, "uni029F.narrow" },
 {"\06\01\17\21\21\21\17\01\01", U(02A0) }, /* Hooktop q */
 {"\16\21\01\06\04\16\04\00\00", U(02A1) }, /* Barred glottal stop */
 {"\16\21\20\14\04\16\04\00\00", U(02A2) }, /* Barred reversed glottal stop */
 {"\04\04\17\25\26\26\17\00\00", U(02A3) }, /* dz ligature */
 {"\04\04\17\25\26\25\15\01\06", U(02A4) }, /* dezh ligature */
 {"\04\04\17\25\26\26\17\05\02", U(02A5) }, /* dz-curl ligature */
 {"\20\20\37\24\22\21\16\00\00", U(02A6) }, /* ts ligature */
 {"\00\20\37\24\22\21\16\00\00", 0xf247, "uni02A6.small" },
 {"\11\12\36\12\12\12\06\02\04", U(02A7) }, /* tesh ligature */
 {"\01\12\36\12\12\12\06\02\04", 0xf248, "uni02A7.small" },
 {"\20\30\23\24\24\24\16\05\02", U(02A8) }, /* tc-curl ligature */
 {"\10\20\20\36\25\25\25\01\06", U(02A9) }, /* feng ligature */
 {"\30\10\13\14\12\11\36\00\00", U(02AA) }, /* ls ligature */
 {"\30\10\17\11\12\14\37\00\00", U(02AB) }, /* lz ligature */
 {"\00\00\25\12\00\25\12\00\00", U(02AC) }, /* bilabial percussive */
 {"\00\00\16\12\00\16\12\00\00", U(02AD) }, /* bidental percussive */

 /* Spacing modifier letters */
 {"\20\20\30\24\24\00\00\00\00", U(02B0) }, /* Superscript h */
 {"\10\00\10\10\10\20\00\00\00", U(02B2) }, /* Superscript j */
 UALIAS(U(02B9), "minute"),
 UALIAS(U(02BA), "second"),
 UALIAS(U(02BB), "quoteleft"),
 UALIAS(U(02BC), "quoteright"),
 UALIAS(U(02BD), "quotereversed"),
 {"\04\12\00\00\00\00\00\00\00", 0x02c6, "circumflex" },
 {"\12\04\00\00\00\00\00\00\00", 0x02c7, "caron" },
 {"\04\04\04\00\00\00\00\00\00", U(02C8) }, /* Vertical stroke (superior) */
 {"\16\00\00\00\00\00\00\00\00", U(02C9) }, /* Modifier letter macron */
 {"\02\04\00\00\00\00\00\00\00", U(02CA) }, /* Modifier letter acute accent */
 {"\10\04\00\00\00\00\00\00\00", U(02CB) }, /* Modifier letter grave accent */
 {"\00\00\00\00\00\00\04\04\04", U(02CC) }, /* Vertical stroke (inferior) */
 {"\00\00\00\00\00\00\00\00\16", U(02CD) }, /* Modifier letter low macron */
 {"\00\00\00\00\00\00\00\04\02", U(02CE) }, /* Modifier letter low grave */
 {"\00\00\00\00\00\00\00\04\10", U(02CF) }, /* Modifier letter low acute */
 {"\00\00\12\04\00\04\12\00\00", U(02D0) }, /* Length mark */
 {"\00\00\12\04\00\00\00\00\00", U(02D1) }, /* Half-length mark */
 {"\21\16\00\00\00\00\00\00\00", 0x02d8, "breve" },
 {"\04\00\00\00\00\00\00\00\00", 0x02d9, "dotaccent" },
 {"\04\12\04\00\00\00\00\00\00", 0x02da, "ring" },
 {"\00\00\00\00\00\00\04\10\04", 0x02db, "ogonek" },
 {"\05\12\00\00\00\00\00\00\00", 0x02dc, "tilde" },
 {"\11\22\00\00\00\00\00\00\00", 0x02dd, "hungarumlaut" },
 {"\24\10\24\24\10\00\00\00\00", U(02E0) }, /* Superscript gamma */
 {"\30\10\10\10\34\00\00\00\00", U(02E1) }, /* Superscript l */
 {"\00\00\14\10\30\00\00\00\00", U(02E2) }, /* Superscript s */
 {"\00\00\24\10\24\00\00\00\00", U(02E3) }, /* Superscript x */
 {"\10\24\20\10\10\00\00\00\00", U(02E4) }, /* Superscript reversed glottal stop */
 {"\37\01\01\01\01\01\01\00\00", U(02E5) }, /* Extra-high tone letter */
 {"\01\37\01\01\01\01\01\00\00", U(02E6) }, /* High tone letter */
 {"\01\01\01\37\01\01\01\00\00", U(02E7) }, /* Mid tone letter */
 {"\01\01\01\01\01\37\01\00\00", U(02E8) }, /* Low tone letter */
 {"\01\01\01\01\01\01\37\00\00", U(02E9) }, /* Extra-low tone letter */

 /* Greek and Coptic */
 {"\10\10\20\20\00\00\00\00\00", U(0374) }, /* Greek numeral sign */
 {"\01\01\01\01\01\21\16\00\00", U(037F) }, /* Yot */
 {"\04\00\00\00\00\00\00\00\00", 0x0384, "tonos" },
 {"\04\25\00\00\00\00\00\00\00", 0x0385, "dieresistonos" },
 {"\22\25\05\05\07\05\05\00\00", 0x0386, "Alphatonos" },
 {"\27\24\04\06\04\04\07\00\00", 0x0388, "Epsilontonos" },
 {"\25\25\05\07\05\05\05\00\00", 0x0389, "Etatonos" },
 {"\27\22\02\02\02\02\07\00\00", 0x038a, "Iotatonos" },
 {"\22\25\05\05\05\05\02\00\00", 0x038c, "Omicrontonos" },
 {"\25\25\05\02\02\02\02\00\00", 0x038e, "Upsilontonos" },
 {"\22\25\05\05\05\02\07\00\00", 0x038f, "Omegatonos" },
 {"\04\25\00\14\04\04\02\00\00", 0x0390, "iotadieresistonos" },
 {"\04\12\21\21\37\21\21\00\00", 0x0391, "Alpha" },
 {"\36\21\21\36\21\21\36\00\00", 0x0392, "Beta" },
 {"\37\20\20\20\20\20\20\00\00", 0x0393, "Gamma" },
 {"\04\04\12\12\21\21\37\00\00", U(0394) }, /* Delta */
 {"\37\20\20\36\20\20\37\00\00", 0x0395, "Epsilon" },
 {"\37\01\02\04\10\20\37\00\00", 0x0396, "Zeta" },
 {"\21\21\21\37\21\21\21\00\00", 0x0397, "Eta" },
 {"\16\21\21\37\21\21\16\00\00", 0x0398, "Theta" },
 {"\16\04\04\04\04\04\16\00\00", 0x0399, "Iota" },
 {"\21\22\24\30\24\22\21\00\00", 0x039a, "Kappa" },
 {"\04\04\12\12\21\21\21\00\00", 0x039b, "Lambda" },
 {"\21\33\25\25\21\21\21\00\00", 0x039c, "Mu" },
 {"\21\21\31\25\23\21\21\00\00", 0x039d, "Nu" },
 {"\37\00\00\16\00\00\37\00\00", 0x039e, "Xi" },
 {"\16\21\21\21\21\21\16\00\00", 0x039f, "Omicron" },
 {"\37\21\21\21\21\21\21\00\00", 0x03a0, "Pi" },
 {"\36\21\21\36\20\20\20\00\00", 0x03a1, "Rho" },
 {"\37\20\10\04\10\20\37\00\00", 0x03a3, "Sigma" },
 {"\37\04\04\04\04\04\04\00\00", 0x03a4, "Tau" },
 {"\21\21\12\04\04\04\04\00\00", 0x03a5, "Upsilon" },
 {"\04\16\25\25\25\16\04\00\00", 0x03a6, "Phi" },
 {"\21\21\12\04\12\21\21\00\00", 0x03a7, "Chi" },
 {"\25\25\25\25\25\16\04\00\00", 0x03a8, "Psi" },
 {"\16\21\21\21\21\12\33\00\00", U(03A9) }, /* Omega */
 {"\12\00\16\04\04\04\16\00\00", 0x03aa, "Iotadieresis" },
 {"\12\00\21\12\04\04\04\00\00", 0x03ab, "Upsilondieresis" },
 {"\04\00\15\22\22\22\15\00\00", 0x03ac, "alphatonos" },
 {"\04\00\17\20\16\20\17\00\00", 0x03ad, "epsilontonos" },
 {"\04\00\36\21\21\21\21\01\01", 0x03ae, "etatonos" },
 {"\04\00\14\04\04\04\02\00\00", 0x03af, "iotatonos" },
 {"\04\25\00\22\21\21\16\00\00", 0x03b0, "upsilondieresistonos" },
 {"\00\00\15\22\22\22\15\00\00", 0x03b1, "alpha" },
 {"\16\21\21\36\21\21\36\20\20", 0x03b2, "beta" },
 {"\00\00\21\21\12\12\04\04\04", 0x03b3, "gamma" },
 {"\17\20\16\21\21\21\16\00\00", 0x03b4, "delta" },
 {"\00\00\17\20\16\20\17\00\00", 0x03b5, "epsilon" },
 {"\36\02\04\10\20\20\14\02\04", 0x03b6, "zeta" },
 {"\00\00\36\21\21\21\21\01\01", 0x03b7, "eta" },
 {"\14\22\22\36\22\22\14\00\00", 0x03b8, "theta" },
 {"\00\00\14\04\04\04\02\00\00", 0x03b9, "iota" },
 {"\00\00\11\12\14\12\11\00\00", 0x03ba, "kappa" },
 {"\30\04\04\12\12\21\21\00\00", 0x03bb, "lambda" },
 {"\00\00\22\22\22\22\35\20\20", U(03BC) }, /* mu */
 {"\00\00\21\21\11\12\04\00\00", 0x03bd, "nu" },
 {"\16\20\20\16\20\20\16\01\02", 0x03be, "xi" },
 {"\00\00\16\21\21\21\16\00\00", 0x03bf, "omicron" },
 {"\00\00\37\12\12\12\11\00\00", 0x03c0, "pi" },
 {"\00\00\16\21\21\21\36\20\20", 0x03c1, "rho" },
 {"\00\00\17\20\20\20\16\01\02", 0x03c2, "sigma1" },
 {"\00\00\17\22\21\21\16\00\00", 0x03c3, "sigma" },
 {"\00\00\37\04\04\04\02\00\00", 0x03c4, "tau" },
 {"\00\00\22\21\21\21\16\00\00", 0x03c5, "upsilon" },
 {"\00\00\26\25\25\25\16\04\04", 0x03c6, "phi" },
 {"\00\00\21\11\12\04\12\22\21", 0x03c7, "chi" },
 {"\00\00\25\25\25\25\16\04\04", 0x03c8, "psi" },
 {"\00\00\12\21\25\25\12\00\00", 0x03c9, "omega" },
 {"\12\00\14\04\04\04\02\00\00", 0x03ca, "iotadieresis" },
 {"\12\00\22\21\21\21\16\00\00", 0x03cb, "upsilondieresis" },
 {"\04\00\16\21\21\21\16\00\00", 0x03cc, "omicrontonos" },
 {"\04\00\22\21\21\21\16\00\00", 0x03cd, "upsilontonos" },
 {"\04\00\12\21\25\25\12\00\00", 0x03ce, "omegatonos" },
 {"\04\04\16\25\25\25\16\04\04", 0x03d5, "phi1" },
 {"\04\00\04\04\04\04\04\04\10", U(03F3) }, /* yot */

 /* Cyrillic */
 {"\10\04\37\20\36\20\37\00\00", U(0400) }, /* Ie grave */
 {"\12\00\37\20\36\20\37\00\00", U(0401) }, /* Yo */
 {"\36\10\10\16\11\11\11\01\02", U(0402) }, /* Dje */
 {"\02\04\37\20\20\20\20\00\00", U(0403) }, /* Gje */
 {"\06\11\20\34\20\11\06\00\00", U(0404) }, /* Ye Ukrainian */
 {"\16\21\20\16\01\21\16\00\00", U(0405) }, /* Dze */
 {"\16\04\04\04\04\04\16\00\00", U(0406) }, /* dotted I */
 {"\12\00\16\04\04\04\16\00\00", U(0407) }, /* Yi */
 {"\01\01\01\01\01\21\16\00\00", U(0408) }, /* Je */
 {"\14\24\24\27\25\25\27\00\00", U(0409) }, /* Lje */
 {"\24\24\24\37\25\25\27\00\00", U(040A) }, /* Nje */
 {"\36\10\10\16\11\11\11\00\00", U(040B) }, /* Tshe */
 {"\02\04\21\22\34\22\21\00\00", U(040C) }, /* Kje */
 {"\10\04\21\23\25\31\21\00\00", U(040D) }, /* I grave */
 {"\25\21\21\37\01\01\37\00\00", U(040E) }, /* short U */
 {"\21\21\21\21\21\21\37\04\00", U(040F) }, /* Dzhe */
 {"\10\04\16\21\37\20\16\00\00", U(0450) }, /* ie grave */
 {"\12\00\16\21\37\20\16\00\00", U(0451) }, /* yo */
 {"\10\36\10\16\11\11\11\01\02", U(0452) }, /* dje */
 {"\02\04\00\37\20\20\20\00\00", U(0453) }, /* gje */
 {"\00\00\14\22\30\22\14\00\00", U(0454) }, /* ye Ukrainian */
 {"\00\00\17\20\16\01\36\00\00", U(0455) }, /* dze */
 {"\04\00\14\04\04\04\16\00\00", U(0456) }, /* dotted i */
 {"\12\00\14\04\04\04\16\00\00", U(0457) }, /* yi */
 {"\04\00\04\04\04\04\04\04\10", U(0458) }, /* je */
 {"\00\00\14\24\26\25\26\00\00", U(0459) }, /* lje */
 {"\00\00\24\24\36\25\26\00\00", U(045A) }, /* nje */
 {"\10\36\10\16\11\11\11\00\00", U(045B) }, /* tshe */
 {"\02\04\20\22\34\22\21\00\00", U(045C) }, /* kje */
 {"\10\04\00\23\25\31\21\00\00", U(045D) }, /* i grave */
 {"\00\04\21\21\21\21\17\01\16", U(045E) }, /* short u */
 {"\00\00\21\21\21\21\37\04\00", U(045F) }, /* dzhe */
 {"\01\37\20\20\20\20\20\00\00", U(0490) }, /* Ghe with upturn */
 {"\00\01\37\20\20\20\20\00\00", U(0491) }, /* ghe with upturn */
 {"\17\10\10\36\10\10\10\00\00", U(0492) }, /* Ghe with stroke */
 {"\00\00\17\10\36\10\10\00\00", U(0493) }, /* ghe with stroke */

#ifdef ARMENIAN
 /* Armenian */
 {"\21\21\21\21\25\22\15\00\00", U(0531) }, /* Ayb */
 {"\16\21\21\20\37\20\20\00\00", U(0532) }, /* Ben */
 {"\14\22\22\22\17\02\02\00\00", U(0533) }, /* Gim */
 {"\14\22\22\02\03\02\02\00\00", U(0534) }, /* Da */
 {"\20\20\37\20\21\21\16\00\00", U(0535) }, /* Ech */
 {"\16\21\21\01\01\22\37\00\00", U(0536) }, /* Za */
 {"\20\20\20\37\20\20\36\00\00", U(0537) }, /* Eh */
 {"\16\21\21\20\20\20\37\00\00", U(0538) }, /* Et */
 {"\16\21\21\23\25\25\22\00\00", U(0539) }, /* To */
 {"\02\02\17\22\22\22\14\00\00", U(053A) }, /* Zhe */
 {"\20\20\36\21\21\21\20\00\00", U(053B) }, /* Ini */
 {"\20\20\20\20\20\20\37\00\00", U(053C) }, /* Liwn */
 {"\20\20\35\25\25\25\22\00\00", U(053D) }, /* Xeh */
 {"\37\16\21\21\21\21\16\00\00", U(053E) }, /* Ca */
 {"\20\21\21\17\01\01\01\00\00", U(053F) }, /* Ken */
 {"\02\01\03\14\20\14\03\00\00", U(0540) }, /* Ho */
 {"\16\21\21\01\15\22\15\00\00", U(0541) }, /* Ja */
 {"\14\22\22\02\02\02\03\00\00", U(0542) }, /* Ghad */
 {"\01\32\04\12\12\21\37\00\00", U(0543) }, /* Cheh */
 {"\23\22\22\22\22\22\14\00\00", U(0544) }, /* Men */
 {"\16\21\01\36\01\21\16\00\00", U(0545) }, /* Yi */
 {"\30\10\10\10\11\11\06\00\00", U(0546) }, /* Now */
 {"\30\07\10\20\21\21\16\00\00", U(0547) }, /* Sha */
 {"\16\21\21\21\21\21\21\00\00", U(0548) }, /* Vo */
 {"\16\21\21\01\02\34\03\00\00", U(0549) }, /* Cha */
 {"\16\25\25\05\05\01\01\00\00", U(054A) }, /* Peh */
 {"\16\21\21\31\05\26\37\00\00", U(054B) }, /* Jheh */
 {"\14\22\22\23\22\22\22\00\00", U(054C) }, /* Ra */
 {"\21\21\21\21\21\21\16\00\00", U(054D) }, /* Seh */
 {"\02\22\22\22\16\02\03\00\00", U(054E) }, /* Vew */
 {"\16\21\20\16\01\21\16\00\00", U(054F) }, /* Tiwn */
 {"\16\21\21\20\20\20\20\00\00", U(0550) }, /* Reh */
 {"\16\21\16\21\01\21\16\00\00", U(0551) }, /* Co */
 {"\20\20\37\20\20\20\20\00\00", U(0552) }, /* Yiwn */
 {"\04\16\25\25\25\16\04\00\00", U(0553) }, /* Piwr */
 {"\06\11\11\16\10\37\10\00\00", U(0554) }, /* Keh */
 {"\16\21\21\21\21\21\16\00\00", U(0555) }, /* Oh */
 {"\14\24\16\05\25\25\16\00\00", U(0556) }, /* Feh */
 {"\00\00\25\25\25\25\13\00\00", U(0561) }, /* ayb */
 {"\00\00\36\21\21\20\37\20\20", U(0562) }, /* ben */
 {"\00\00\16\22\22\22\17\02\02", U(0563) }, /* gim */
 {"\00\00\34\22\22\22\23\02\02", U(0564) }, /* da */
 {"\20\20\37\20\21\21\17\00\00", U(0565) }, /* ech */
 {"\00\00\16\22\22\22\16\02\03", U(0566) }, /* za */
 {"\20\20\37\20\20\20\36\00\00", U(0567) }, /* eh */
 {"\00\00\36\21\21\21\21\20\37", U(0568) }, /* et */
 {"\00\00\36\21\27\31\26\20\20", U(0569) }, /* to */
 {"\02\02\17\22\22\22\14\00\00", U(056A) }, /* zhe */
 {"\20\20\36\21\21\21\21\20\20", U(056B) }, /* ini */
 {"\00\00\10\10\10\10\10\10\16", U(056C) }, /* liwn */
 {"\20\20\35\25\25\25\23\20\20", U(056D) }, /* xeh */
 {"\04\02\17\22\22\22\14\00\00", U(056E) }, /* ca */
 {"\20\20\21\21\21\21\17\01\01", U(056F) }, /* ken */
 {"\20\20\36\21\21\21\21\00\00", U(0570) }, /* ho */
 {"\04\02\04\12\21\23\15\00\00", U(0571) }, /* ja */
 {"\00\00\34\22\22\22\22\02\03", U(0572) }, /* ghad */
 {"\07\10\36\11\11\11\07\00\00", U(0573) }, /* cheh */
 {"\03\02\22\22\22\22\16\00\00", U(0574) }, /* men */
 {"\00\00\04\04\04\04\04\04\10", U(0575) }, /* yi */
 {"\30\10\11\11\11\11\07\00\00", U(0576) }, /* now */
 {"\00\00\16\21\01\02\14\20\37", U(0577) }, /* sha */
 {"\00\00\36\21\21\21\21\00\00", U(0578) }, /* vo */
 {"\00\00\04\10\04\02\14\20\37", U(0579) }, /* cha */
 {"\00\00\25\25\25\25\13\01\01", U(057A) }, /* peh */
 {"\00\00\16\21\21\12\14\20\37", U(057B) }, /* jheh */
 {"\00\00\36\21\21\22\23\00\00", U(057C) }, /* ra */
 {"\00\00\21\21\21\21\17\00\00", U(057D) }, /* seh */
 {"\02\02\22\22\22\22\16\02\03", U(057E) }, /* vew */
 {"\00\00\26\25\25\25\15\00\00", U(057F) }, /* tiwn */
 {"\00\00\36\21\21\21\21\20\20", U(0580) }, /* reh */
 {"\00\00\17\21\21\21\17\01\16", U(0581) }, /* co */
 {"\00\00\10\10\10\10\16\00\00", U(0582) }, /* yiwn */
 {"\04\04\26\25\25\25\15\04\04", U(0583) }, /* piwr */
 {"\00\00\16\11\11\16\10\37\10", U(0584) }, /* keh */
 {"\00\00\16\21\21\21\16\00\00", U(0585) }, /* oh */
 {"\14\24\16\05\25\25\16\04\04", U(0586) }, /* feh */
 {"\20\20\24\24\24\24\17\00\00", U(0587) }, /* ech_yiwn */
 {"\14\22\22\07\02\07\02\00\00", U(058F) }, /* armdram */
#endif

 /* Phonetic extensions */
 {"\00\00\16\21\37\21\21\00\00", U(1D00) }, /* small cap A */
 {"\00\00\17\24\26\34\27\00\00", U(1D01) }, /* small cap AE */
 {"\00\00\16\05\36\24\12\00\00", U(1D02) }, /* turned ae */
 {"\00\00\16\11\36\11\16\00\00", U(1D03) }, /* small cap barred B */
 {"\00\00\16\21\20\21\16\00\00", U(1D04) }, /* small cap C */
 {"\00\00\36\21\21\21\36\00\00", U(1D05) }, /* small cap D */
 {"\00\00\36\11\11\11\36\00\00", 0xf220, "uni1D05.serif" },
 {"\00\00\16\11\11\11\16\00\00", 0xf230, "uni1D05.narrow" },
 {"\00\00\16\11\35\11\16\00\00", U(1D06) }, /* small cap Eth */
 {"\00\00\36\11\35\11\36\00\00", 0xf222, "uni1D06.serif" },
 {"\00\00\37\20\36\20\37\00\00", U(1D07) }, /* small cap E */
 {"\00\00\16\04\04\04\06\00\04", U(1D09) }, /* turned i */
 {"\00\00\01\01\01\21\16\00\00", U(1D0A) }, /* small cap J */
 {"\00\00\02\02\02\22\14\00\00", 0xf21c, "uni1D0A.narrow" },
 {"\00\00\21\22\34\22\21\00\00", U(1D0B) }, /* small cap K */
 {"\00\00\10\14\30\10\17\00\00", U(1D0C) }, /* small cap Lslash */
 {"\00\00\21\33\25\21\21\00\00", U(1D0D) }, /* small cap M */
 {"\00\00\21\23\25\31\21\00\00", U(1D0E) }, /* small cap reversed N */
 {"\00\00\16\21\21\21\16\00\00", U(1D0F) }, /* small cap O */
 {"\00\00\32\05\35\25\12\00\00", U(1D14) }, /* turned oe */
 {"\00\00\36\21\36\20\20\00\00", U(1D18) }, /* small cap P */
 {"\00\00\17\21\17\11\21\00\00", U(1D19) }, /* small cap reversed R */
 {"\00\00\21\11\17\21\17\00\00", U(1D1A) }, /* small cap turned R */
 {"\00\00\37\04\04\04\04\00\00", U(1D1B) }, /* small cap T */
 {"\00\00\21\21\21\21\16\00\00", U(1D1C) }, /* small cap U */
 {"\00\00\37\01\01\01\36\00\00", U(1D1D) }, /* sideways u */
 {"\00\00\37\01\36\01\36\00\00", U(1D1F) }, /* sideways m */
 {"\00\00\21\21\12\12\04\00\00", U(1D20) }, /* small cap V */
 {"\00\00\21\25\25\25\12\00\00", U(1D21) }, /* small cap W */
 {"\00\00\37\01\16\20\37\00\00", U(1D22) }, /* small cap Z */
 {"\00\00\37\01\06\21\16\00\00", U(1D23) }, /* small cap Ezh */
 {"\30\04\34\24\10\00\00\00\00", U(1D4A) }, /* Superscript schwa */
 {"\00\00\00\00\04\00\14\04\16", U(1D62) }, /* iinferior */
 {"\00\00\00\00\00\00\12\14\10", U(1D63) }, /* rinferior */
 {"\00\00\00\00\00\00\12\12\06", U(1D64) }, /* uinferior */
 {"\00\00\00\00\00\00\12\12\04", U(1D65) }, /* vinferior */
 {"\00\00\16\04\16\04\16\00\00", U(1D7B) }, /* small cap I with stroke */

 /* Phonetic extensions supplement */
 {"\10\24\34\24\10\00\00\00\00", U(1DBF) }, /* Superscript theta */

 /* Latin extended additional */
 {"\04\00\36\21\36\21\36\00\00", U(1E02) }, /* Bdotaccent */
 {"\24\20\36\21\21\21\36\00\00", U(1E03) }, /* bdotaccent */
 ALIAS("uni1E03.sc", "uni1E02"),
 {"\36\21\21\36\21\21\36\00\04", U(1E04) }, /* Bdotbelow */
 {"\00\00\36\21\36\21\36\00\04", 0xf19d, "uni1E04.c2sc"},
 {"\20\20\36\21\21\21\36\00\04", U(1E05) }, /* bdotbelow */
 ALIAS("uni1E05.sc", "uni1E04.c2sc"),
 {"\36\21\21\36\21\21\36\00\16", U(1E06) }, /* Bmacronbelow */
 {"\00\00\36\21\36\21\36\00\16", 0xf19e, "uni1E06.c2sc"},
 {"\20\20\36\21\21\21\36\00\16", U(1E07) }, /* bmacronbelow */
 ALIAS("uni1E06.sc", "uni1E06.c2sc"),
 {"\02\04\16\21\20\21\16\04\10", U(1E08) }, /* Ccedillaacute */
 {"\02\04\16\21\20\21\16\04\14", 0xf20c, "uni1E08.square" },
 {"\02\04\17\20\20\20\17\04\10", U(1E09) }, /* ccedillaacute */
 {"\02\04\17\20\20\20\17\04\14", 0xf20d, "uni1E09.square" },
 ALIAS("uni1E09.sc", "uni1E08"),
 ALIAS("uni1E09.sc.square", "uni1E08.square"),
 {"\04\00\36\21\21\21\36\00\00", U(1E0A) }, /* Ddotaccent */
 {"\04\00\36\11\11\11\36\00\00", 0xf224, "uni1E0A.serif" },
 {"\04\00\36\11\11\11\36\00\00", 0xf234, "uni1E0A.narrow" },
 {"\05\01\17\21\21\21\17\00\00", U(1E0B) }, /* ddotaccent */
 ALIAS("uni1E0B.sc", "uni1E0A"),
 ALIAS("uni1E0B.sc.serif", "uni1E0A.serif"),
 ALIAS("uni1E0B.sc.narrow", "uni1E0A.narrow"),
 {"\36\21\21\21\21\21\36\00\04", U(1E0C) }, /* Ddotbelow */
 {"\36\11\11\11\11\11\36\00\04", 0xf225, "uni1E0C.serif" },
 {"\16\11\11\11\11\11\16\00\04", 0xf235, "uni1E0C.narrow" },
 {"\00\00\36\21\21\21\36\00\04", 0xf19f, "uni1E0C.c2sc" },
 {"\00\00\36\11\11\11\36\00\04", 0xf226, "uni1E0C.c2sc.serif" },
 {"\00\00\16\11\11\11\16\00\04", 0xf236, "uni1E0C.c2sc.narrow" },
 {"\01\01\17\21\21\21\17\00\04", U(1E0D) }, /* ddotbelow */
 ALIAS("uni1E0D.sc", "uni1E0C.c2sc"),
 ALIAS("uni1E0D.sc.serif", "uni1E0C.c2sc.serif"),
 ALIAS("uni1E0D.sc.narrow", "uni1E0C.c2sc.narrow"),
 {"\36\21\21\21\21\21\36\00\16", U(1E0E) }, /* Dmacronbelow */
 {"\36\11\11\11\11\11\36\00\16", 0xf227, "uni1E0E.serif" },
 {"\16\11\11\11\11\11\16\00\16", 0xf237, "uni1E0E.narrow" },
 {"\00\00\36\21\21\21\36\00\16", 0xf1a0, "uni1E0E.c2sc" },
 {"\00\00\36\11\11\11\36\00\16", 0xf228, "uni1E0E.c2sc.serif" },
 {"\00\00\16\11\11\11\16\00\16", 0xf238, "uni1E0E.c2sc.narrow" },
 {"\01\01\17\21\21\21\17\00\16", U(1E0F) }, /* dmacronbelow */
 ALIAS("uni1E0F.sc", "uni1E0E.c2sc"),
 ALIAS("uni1E0F.sc.serif", "uni1E0E.c2sc.serif"),
 ALIAS("uni1E0F.sc.narrow", "uni1E0E.c2sc.narrow"),
 {"\36\21\21\21\21\21\36\04\10", U(1E10) }, /* Dcommaaccent */
 {"\36\11\11\11\11\11\36\04\10", 0xf229, "uni1E10.serif" },
 {"\16\11\11\11\11\11\16\04\10", 0xf239, "uni1E10.narrow" },
 {"\00\00\36\21\21\21\36\04\10", 0xf1a1, "uni1E10.c2sc" },
 {"\00\00\36\11\11\11\36\04\10", 0xf22a, "uni1E10.c2sc.serif" },
 {"\00\00\16\11\11\11\16\04\10", 0xf23a, "uni1E10.c2sc.narrow" },
 {"\01\01\17\21\21\21\17\04\10", U(1E11) }, /* dcommaaccent */
 ALIAS("uni1E11.sc", "uni1E10.c2sc"),
 ALIAS("uni1E11.sc.serif", "uni1E10.c2sc.serif"),
 ALIAS("uni1E11.sc.narrow", "uni1E10.c2sc.narrow"),
 {"\36\21\21\21\21\21\36\04\12", U(1E12) }, /* Dcircumflexbelow */
 {"\36\11\11\11\11\11\36\04\12", 0xf22b, "uni1E12.serif" },
 {"\16\11\11\11\11\11\16\04\12", 0xf23b, "uni1E12.narrow" },
 {"\00\00\36\21\21\21\36\04\12", 0xf1a2, "uni1E12.c2sc" },
 {"\00\00\36\11\11\11\36\04\12", 0xf22c, "uni1E12.c2sc.serif" },
 {"\00\00\16\11\11\11\16\04\12", 0xf23c, "uni1E12.c2sc.narrow" },
 {"\01\01\17\21\21\21\17\04\12", U(1E13) }, /* dcircumflexbelow */
 ALIAS("uni1E13.sc", "uni1E12.c2sc"),
 ALIAS("uni1E13.sc.serif", "uni1E12.c2sc.serif"),
 ALIAS("uni1E13.sc.narrow", "uni1E12.c2sc.narrow"),
 {"\37\20\20\36\20\20\37\04\12", U(1E18) }, /* Ecircumflexbelow */
 {"\00\00\37\20\36\20\37\04\12", 0xf1a3, "uni1E18.c2sc" },
 {"\00\00\16\21\37\20\16\04\12", U(1E19) }, /* ecircumflexbelow */
 ALIAS("uni1E19.sc", "uni1E18.c2sc"),
 {"\04\00\37\20\36\20\20\00\00", U(1E1E) }, /* Fdotaccent */
 {"\04\00\02\04\16\04\04\00\00", U(1E1F) }, /* fdotaccent */
 ALIAS("uni1E1F.sc", "uni1E1E"),
 {"\16\00\17\20\23\21\17\00\00", U(1E20) }, /* Gmacron */
 {"\16\00\17\21\21\21\17\01\16", U(1E21) }, /* gmacron */
 ALIAS("uni1E21.sc", "uni1E20"),
 {"\25\21\21\37\21\21\21\00\00", U(1E22) }, /* Hdotaccent */
 {"\04\00\21\21\37\21\21\00\00", 0xf1a4, "uni1E22.c2sc" },
 {"\24\20\36\21\21\21\21\00\00", U(1E23) }, /* hdotaccent */
 ALIAS("uni1E23.sc", "uni1E22.c2sc"),
 {"\21\21\21\37\21\21\21\00\04", U(1E24) }, /* Hdotbelow */
 {"\00\00\21\21\37\21\21\00\04", 0xf1a5, "uni1E24.c2sc" },
 {"\20\20\36\21\21\21\21\00\04", U(1E25) }, /* hdotbelow */
 ALIAS("uni1E25.sc", "uni1E24.c2sc"),
 {"\12\00\21\21\37\21\21\00\00", U(1E26) }, /* Hdieresis */
 {"\25\20\36\21\21\21\21\00\00", U(1E27) }, /* hdieresis */
 ALIAS("uni1E27.sc", "uni1E26"),
 {"\21\21\21\37\21\21\21\10\20", U(1E28) }, /* Hcedilla */
 {"\21\21\21\37\21\21\21\10\30", 0xf20e, "uni1E28.square" },
 {"\00\00\21\21\37\21\21\10\20", 0xf1a6, "uni1E28.c2sc" },
 {"\00\00\21\21\37\21\21\10\30", 0xf20f, "uni1E28.c2sc.square" },
 {"\20\20\36\21\21\21\21\10\20", U(1E29) }, /* hcedilla */
 {"\20\20\36\21\21\21\21\10\30", 0xf210, "uni1E29.square" },
 ALIAS("uni1E29.sc", "uni1E28"),
 ALIAS("uni1E29.sc.square", "uni1E28.square"),
 {"\21\22\24\30\24\22\21\00\04", U(1E32) }, /* Kdotbelow */
 {"\10\10\11\12\14\12\11\00\04", U(1E33) }, /* kdotbelow */
 {"\21\22\24\30\24\22\21\00\16", U(1E34) }, /* Kmacronbelow */
 {"\00\00\21\22\34\22\21\00\16", 0xf1a7, "uni1E34.c2sc" },
 {"\10\10\11\12\14\12\11\00\16", U(1E35) }, /* kmacronbelow */
 ALIAS("uni1E35.sc", "uni1E34.c2sc"),
 {"\20\20\20\20\20\20\37\00\04", U(1E36) }, /* Ldotbelow */
 {"\00\00\20\20\20\20\37\00\04", 0xf1a8, "uni1E36.c2sc" },
 {"\14\04\04\04\04\04\16\00\04", U(1E37) }, /* ldotbelow */
 ALIAS("uni1E37.sc", "uni1E36.c2sc"),
 {"\20\20\20\20\20\20\37\00\16", U(1E3A) }, /* Lmacronbelow */
 {"\00\00\20\20\20\20\37\00\16", 0xf1a9, "uni1E3A.c2sc" },
 {"\14\04\04\04\04\04\16\00\16", U(1E3B) }, /* lmacronbelow */
 ALIAS("uni1E3B.sc", "uni1E3A.c2sc"),
 {"\20\20\20\20\20\20\37\04\12", U(1E3C) }, /* Lcircumflexbelow */
 {"\00\00\20\20\20\20\37\04\12", 0xf1aa, "uni1E3C.c2sc" },
 {"\14\04\04\04\04\16\00\04\12", U(1E3D) }, /* lcircumflexbelow */
 ALIAS("uni1E3D.sc", "uni1E3C.c2sc"),
 {"\02\04\21\33\25\21\21\00\00", U(1E3E) }, /* Macute */
 {"\02\04\00\32\25\25\25\00\00", U(1E3F) }, /* macute */
 ALIAS("uni1E3F.sc", "uni1E3E"),
 {"\04\21\33\25\21\21\21\00\00", U(1E40) }, /* Mdotaccent */
 {"\04\00\21\33\25\21\21\00\00", 0xf1ab, "uni1E40.c2sc" },
 {"\04\00\32\25\25\25\25\00\00", U(1E41) }, /* mdotaccent */
 ALIAS("uni1E41.sc", "uni1E40.c2sc"),
 {"\21\33\25\25\21\21\21\00\04", U(1E42) }, /* Mdotbelow */
 {"\00\00\21\33\25\21\21\00\04", 0xf1ac, "uni1E42.c2sc" },
 {"\00\00\32\25\25\25\25\00\04", U(1E43) }, /* mdotbelow */
 ALIAS("uni1E43.sc", "uni1E42.c2sc"),
 {"\25\21\31\25\23\21\21\00\00", U(1E44) }, /* Ndotaccent */
 {"\04\00\21\31\25\23\21\00\00", 0xf1ad, "uni1E44.c2sc" },
 {"\04\00\36\21\21\21\21\00\00", U(1E45) }, /* ndotaccent */
 ALIAS("uni1E45.sc", "uni1E44.c2sc"),
 {"\21\21\31\25\23\21\21\00\04", U(1E46) }, /* Ndotbelow */
 {"\00\00\21\31\25\23\21\00\04", 0xf1ae, "uni1E46.c2sc" },
 {"\00\00\36\21\21\21\21\00\04", U(1E47) }, /* ndotbelow */
 ALIAS("uni1E47.sc", "uni1E46.c2sc"),
 {"\21\21\31\25\23\21\21\00\16", U(1E48) }, /* Nmacronbelow */
 {"\00\00\21\31\25\23\21\00\16", 0xf1af, "uni1E48.c2sc" },
 {"\00\00\36\21\21\21\21\00\16", U(1E49) }, /* nmacronbelow */
 ALIAS("uni1E49.sc", "uni1E48.c2sc"),
 {"\02\04\36\21\36\20\20\00\00", U(1E54) }, /* Pacute */
 {"\02\04\36\21\21\21\36\20\20", U(1E55) }, /* pacute */
 ALIAS("uni1E55.sc", "uni1E54"),
 {"\04\00\36\21\36\20\20\00\00", U(1E56) }, /* Pdotaccent */
 {"\04\00\36\21\21\21\36\20\20", U(1E57) }, /* pdotaccent */
 ALIAS("uni1E57.sc", "uni1E56"),
 {"\04\00\36\21\36\22\21\00\00", U(1E58) }, /* Rdotaccent */
 {"\04\00\13\14\10\10\10\00\00", U(1E59) }, /* rdotaccent */
 ALIAS("uni1E59.sc", "uni1E58"),
 {"\36\21\21\36\24\22\21\00\04", U(1E5A) }, /* Rdotbelow */
 {"\00\00\36\21\36\22\21\00\04", 0xf1b0, "uni1E5A.c2sc" },
 {"\00\00\13\14\10\10\10\00\04", U(1E5B) }, /* rdotbelow */
 ALIAS("uni1E5B.sc", "uni1E5A.c2sc"),
 {"\16\00\36\21\36\22\21\00\04", U(1E5C) }, /* Rdotbelowmacron */
 {"\16\00\13\14\10\10\10\00\04", U(1E5D) }, /* rdotbelowmacron */
 ALIAS("uni1E5D.sc", "uni1E5C"),
 {"\36\21\21\36\24\22\21\00\16", U(1E5E) }, /* Rmacronbelow */
 {"\00\00\36\21\36\22\21\00\16", 0xf1b1, "uni1E5E.c2sc" },
 {"\00\00\13\14\10\10\10\00\16", U(1E5F) }, /* rmacronbelow */
 ALIAS("uni1E5F.sc", "uni1E5E.c2sc"),
 {"\16\21\20\16\01\21\16\00\04", U(1E62) }, /* Sdotbelow */
 ALIAS("uni1E62.c2sc", "uni1E63"),
 {"\00\00\17\20\16\01\36\00\04", U(1E63) }, /* sdotbelow */
 {"\04\00\37\04\04\04\04\00\00", U(1E6A) }, /* Tdotaccent */
 {"\04\00\04\16\04\04\02\00\00", U(1E6B) }, /* tdotaccent */
 ALIAS("uni1E6B.sc", "uni1E6A"),
 {"\37\04\04\04\04\04\04\00\04", U(1E6C) }, /* Tdotbelow */
 {"\00\00\37\04\04\04\04\00\04", 0xf1b2, "uni1E6C.c2sc" },
 {"\04\04\16\04\04\04\02\00\04", U(1E6D) }, /* tdotbelow */
 {"\00\04\16\04\04\04\02\00\04", 0xf249, "uni1E6D.small" },
 ALIAS("uni1E6D.sc", "uni1E6C.c2sc"),
 {"\37\04\04\04\04\04\04\00\16", U(1E6E) }, /* Tmacronbelow */
 {"\00\00\37\04\04\04\04\00\16", 0xf1b3, "uni1E6E.c2sc" },
 {"\04\04\16\04\04\04\02\00\16", U(1E6F) }, /* tmacronbelow */
 {"\00\04\16\04\04\04\02\00\16", 0xf24a, "uni1E6F.small" },
 ALIAS("uni1E6F.sc", "uni1E6E.c2sc"),
 {"\37\04\04\04\04\04\00\04\12", U(1E70) }, /* Tcircumflexbelow */
 {"\00\00\37\04\04\04\00\04\12", 0xf1b4, "uni1E70.c2sc" },
 {"\04\04\16\04\04\02\00\04\12", U(1E71) }, /* tcircumflexbelow */
 {"\00\04\16\04\04\02\00\04\12", 0xf24b, "uni1E71.small" },
 ALIAS("uni1E71.sc", "uni1E70.c2sc"),
 {"\21\21\21\21\21\21\16\00\12", U(1E72) }, /* Udieresisbelow */
 {"\00\00\21\21\21\21\16\00\12", 0xf1b5, "uni1E72.c2sc" },
 {"\00\00\21\21\21\21\17\00\12", U(1E73) }, /* udieresisbelow */
 ALIAS("uni1E73.sc", "uni1E72.c2sc"),
 {"\21\21\21\21\21\21\16\04\12", U(1E76) }, /* Ucircumflexbelow */
 {"\00\00\21\21\21\21\16\04\12", 0xf1b6, "uni1E76.c2sc" },
 {"\00\00\21\21\21\21\17\04\12", U(1E77) }, /* ucircumflexbelow */
 ALIAS("uni1E77.sc", "uni1E76.c2sc"),
 {"\12\00\16\00\21\21\16\00\00", U(1E7A) }, /* Umacrondieresis */
 {"\12\00\16\00\21\21\17\00\00", U(1E7B) }, /* umacrondieresis */
 ALIAS("uni1E7B.sc", "uni1E7A"),
 {"\21\21\21\12\12\04\04\00\04", U(1E7E) }, /* Vdotbelow */
 ALIAS("uni1E7E.c2sc", "uni1E7F"),
 {"\00\00\21\21\12\12\04\00\04", U(1E7F) }, /* vdotbelow */
 {"\10\04\21\25\25\25\12\00\00", 0x1e80, "Wgrave" },
 {"\10\04\21\21\25\25\12\00\00", 0x1e81, "wgrave" },
 ALIAS("wgrave.sc", "Wgrave"),
 {"\02\04\21\25\25\25\12\00\00", 0x1e82, "Wacute" },
 {"\02\04\21\21\25\25\12\00\00", 0x1e83, "wacute" },
 ALIAS("wacute.sc", "Wacute"),
 {"\12\00\21\25\25\25\12\00\00", 0x1e84, "Wdieresis", },
 {"\12\00\21\21\25\25\12\00\00", 0x1e85, "wdieresis", },
 ALIAS("wdieresis.sc", "Wdieresis"),
 {"\21\21\21\25\25\25\12\00\04", U(1E88) }, /* Wdotbelow */
 {"\00\00\21\25\25\25\12\00\04", 0xf18c, "uni1E88.c2sc" },
 {"\00\00\21\21\25\25\12\00\04", U(1E89) }, /* wdotbelow */
 ALIAS("uni1E89.sc", "uni1E88.c2sc"),
 {"\25\21\12\04\12\21\21\00\00", U(1E8A) }, /* Xdotaccent */
 ALIAS("uni1E8A.c2sc", "uni1E8B"),
 {"\04\00\21\12\04\12\21\00\00", U(1E8B) }, /* xdotaccent */
 {"\12\00\21\21\16\21\21\00\00", U(1E8C) }, /* Xdieresis */
 {"\12\00\21\12\04\12\21\00\00", U(1E8D) }, /* xdieresis */
 ALIAS("uni1E8D.sc", "uni1E8C"),
 {"\25\21\12\04\04\04\04\00\00", U(1E8E) }, /* Ydotaccent */
 {"\04\00\21\12\04\04\04\00\00", 0xf1b7, "uni1E8E.c2sc" },
 {"\04\00\21\21\21\21\17\01\16", U(1E8F) }, /* ydotaccent */
 ALIAS("uni1E8F.sc", "uni1E8E.c2sc"),
 {"\04\12\37\01\16\20\37\00\00", U(1E90) }, /* Zcircumflex */
 {"\04\12\37\02\04\10\37\00\00", U(1E91) }, /* zcircumflex */
 ALIAS("uni1E91.sc", "uni1E90"),
 {"\37\01\02\04\10\20\37\00\04", U(1E92) }, /* Zdotbelow */
 {"\00\00\37\01\16\20\37\00\04", 0xf1b8, "uni1E92.c2sc" },
 {"\00\00\37\02\04\10\37\00\04", U(1E93) }, /* zdotbelow */
 ALIAS("uni1E93.sc", "uni1E92.c2sc"),
 {"\37\01\02\04\10\20\37\00\16", U(1E94) }, /* Zmacronbelow */
 {"\00\00\37\01\16\20\37\00\16", 0xf1b9, "uni1E94.c2sc" },
 {"\00\00\37\02\04\10\37\00\16", U(1E95) }, /* zmacronbelow */
 ALIAS("uni1E95.sc", "uni1E94.c2sc"),
 {"\20\20\36\21\21\21\21\00\16", U(1E96) }, /* hmacronbelow */
 {"\25\04\16\04\04\04\02\00\00", U(1E97) }, /* tdieresis */
 {"\21\04\16\04\04\04\02\00\00", 0xf24c, "uni1E97.small" },
 {"\16\21\22\22\21\21\26\00\00", U(1E9E) }, /* Germandbls */
 {"\00\00\16\21\22\21\26\00\00", 0xf1c1, "uni1E9E.c2sc" },
 {"\04\12\21\21\37\21\21\00\04", U(1EA0) }, /* Adotbelow */
 {"\00\00\16\21\37\21\21\00\04", 0xf1ba, "uni1EA0.c2sc" },
 {"\00\00\16\01\17\21\17\00\04", U(1EA1) }, /* adotbelow */
 ALIAS("uni1EA1.sc", "uni1EA0.c2sc"),
 {"\37\20\20\36\20\20\37\00\04", U(1EB8) }, /* Edotbelow */
 {"\00\00\37\20\36\20\37\00\04", 0xf1bb, "uni1EB8.c2sc" },
 {"\00\00\16\21\37\20\16\00\04", U(1EB9) }, /* edotbelow */
 ALIAS("uni1EB9.sc", "uni1EB8.c2sc"),
 {"\16\04\04\04\04\04\16\00\04", U(1ECA) }, /* Idotbelow */
 {"\00\00\16\04\04\04\16\00\04", 0xf1bc, "uni1ECA.c2sc" },
 {"\04\00\14\04\04\04\16\00\04", U(1ECB) }, /* idotbelow */
 ALIAS("uni1ECB.sc", "uni1ECA.c2sc"),
 {"\16\21\21\21\21\21\16\00\04", U(1ECC) }, /* Odotbelow */
 ALIAS("uni1ECC.c2sc", "uni1ECD"),
 {"\00\00\16\21\21\21\16\00\04", U(1ECD) }, /* odotbelow */
 {"\21\21\21\21\21\21\16\00\04", U(1EE4) }, /* Udotbelow */
 {"\00\00\21\21\21\21\16\00\04", 0xf1bd, "uni1EE4.c2sc" },
 {"\00\00\21\21\21\21\17\00\04", U(1EE5) }, /* udotbelow */
 ALIAS("uni1EE5.sc", "uni1EE4.c2sc"),
 {"\10\04\21\12\04\04\04\00\00", 0x1ef2, "Ygrave" },
 {"\10\04\21\21\21\21\17\01\16", 0x1ef3, "ygrave" },
 ALIAS("ygrave.sc", "Ygrave"),
 {"\21\21\12\04\04\04\04\00\04", U(1EF4) }, /* Ydotbelow */
 {"\00\00\21\12\04\04\04\00\04", 0xf1be, "uni1EF4.c2sc" },
 {"\00\00\21\21\17\01\16\00\04", U(1EF5) }, /* ydotbelow */
 ALIAS("uni1EF5.sc", "uni1EF4.c2sc"),

 /* General punctuation */
 {{0x00}, U(2001), MOS6 }, /* em quad */
 {"\00\00\00\16\00\00\00\00\00", U(2010) }, /* hyphen */
 {"\00\00\00\16\00\00\00\00\00", U(2011) }, /* non-breaking hyphen */
 {"\00\00\00\36\00\00\00\00\00", 0x2013, "endash" },
 {"\00\00\00\00\00\00\37\00\37", 0x2017, "underscoredbl" },
 {"\02\04\04\00\00\00\00\00\00", 0x2018, "quoteleft" },
 {"\00\00\00\00\00\04\04\10\00", 0x201a, "quotesinglbase" },
 {"\11\22\22\00\00\00\00\00\00", 0x201c, "quotedblleft" },
 {"\11\11\22\00\00\00\00\00\00", 0x201d, "quotedblright" },
 {"\00\00\00\00\00\11\11\22\00", 0x201e, "quotedblbase" },
 {"\22\22\11\00\00\00\00\00\00", U(201F) }, /* quotedblreversed */
 {"\04\04\37\04\04\04\04\04\04", 0x2020, "dagger" },
 {"\04\04\37\04\04\04\37\04\04", 0x2021, "daggerdbl" },
 {"\00\00\16\16\16\00\00\00\00", 0x2022, "bullet" },
 {"\00\00\00\00\00\00\25\00\00", 0x2026, "ellipsis" },
 {"\21\22\04\10\20\05\05\00\00", 0x2030, "perthousand" },
 {"\10\10\20\20\00\00\00\00\00", 0x2032, "minute" },
 {"\11\11\22\22\00\00\00\00\00", 0x2033, "second" },
 {"\02\02\01\01\00\00\00\00\00", U(2035) }, /* reversed prime */
 {"\22\22\11\11\00\00\00\00\00", U(2036) }, /* reversed double prime */
 {"\00\00\00\02\04\02\00\00\00", 0x2039, "guilsinglleft" },
 {"\00\00\00\10\04\10\00\00\00", 0x203a, "guilsinglright" },
 {"\25\21\12\04\25\04\12\21\25", U(203B) }, /* referencemark */
 {"\12\12\12\12\12\00\12\00\00", 0x203c, "exclamdbl" },
 {"\16\25\05\06\04\00\04\00\00", U(203D) }, /* interrobang */
 {"\37\00\00\00\00\00\00\00\00", U(203E) }, /* overline */
 {"\00\00\00\00\00\00\00\21\16", U(203F) }, /* Bottom tie bar */
 {"\16\21\00\00\00\00\00\00\00", U(2040) }, /* character tie */
 {"\01\01\02\02\04\10\10\20\20", 0x2044, "fraction" },
 {"\12\25\05\11\12\00\12\00\00", U(2047) }, /* questiondbl */
 {"\11\25\05\11\11\00\11\00\00", U(2048) }, /* question exclam */
 {"\22\25\21\22\22\00\22\00\00", U(2049) }, /* exclam question */
 {"\26\25\25\26\24\24\24\00\00", U(204B) }, /* reversed pilcrow */
 {"\00\00\04\00\00\04\04\02\00", U(204F) }, /* reversed semicolon */
 {"\21\01\02\04\10\20\21\00\00", U(2052) }, /* commercial minus */
 {"\00\00\10\25\02\00\00\00\00", U(2053) }, /* swing dash */
 {"\00\00\00\00\00\00\00\16\21", U(2054) }, /* inverted undertie */
 {"\00\00\01\00\20\00\01\00\00", U(2056) }, /* three dots */
 {"\00\00\04\00\21\00\04\00\00", U(2058) }, /* four dots */
 {"\00\00\21\00\04\00\21\00\00", U(2059) }, /* five dots */
 {"\04\00\00\00\00\00\04\00\00", U(205A) }, /* two dots */
 {"\04\00\00\00\21\00\00\00\04", U(205B) }, /* large four dots */
 {"\00\00\25\04\37\04\25\00\00", U(205C) }, /* dotted cross */
 {"\04\00\00\04\00\00\04\00\00", U(205D) }, /* tricolon */
 {"\04\00\04\00\04\00\04\00\00", U(205E) }, /* vertical four dots */

 /* Subscripts and superscripts */
 {"\04\12\12\12\04\00\00\00\00", U(2070) }, /* zerosuperior */
 {"\04\00\14\04\16\00\00\00\00", U(2071) }, /* isuperior */
 {"\02\06\12\16\02\00\00\00\00", U(2074) }, /* foursuperior */
 {"\16\10\14\02\14\00\00\00\00", U(2075) }, /* fivesuperior */
 {"\04\10\14\12\04\00\00\00\00", U(2076) }, /* sixsuperior */
 {"\16\02\04\10\10\00\00\00\00", U(2077) }, /* sevensuperior */
 {"\04\12\04\12\04\00\00\00\00", U(2078) }, /* eightsuperior */
 {"\04\12\06\02\04\00\00\00\00", U(2079) }, /* ninesuperior */
 {"\00\04\16\04\00\00\00\00\00", U(207A) }, /* plussuperior */
 {"\00\00\16\00\00\00\00\00\00", U(207B) }, /* minussuperior */
 {"\00\16\00\16\00\00\00\00\00", U(207C) }, /* equalssuperior */
 {"\02\04\04\04\02\00\00\00\00", U(207D) }, /* parenleftsuperior */
 {"\10\04\04\04\10\00\00\00\00", U(207E) }, /* parenrightsuperior */
 {"\00\00\14\12\12\00\00\00\00", U(207F) }, /* nsuperior */
 {"\00\00\00\00\04\12\12\12\04", U(2080) }, /* zeroinferior */
 {"\00\00\00\00\04\14\04\04\16", U(2081) }, /* oneinferior */
 {"\00\00\00\00\14\02\04\10\16", U(2082) }, /* twoinferior */
 {"\00\00\00\00\14\02\14\02\14", U(2083) }, /* threeinferior */
 {"\00\00\00\00\02\06\12\16\02", U(2084) }, /* fourinferior */
 {"\00\00\00\00\16\10\14\02\14", U(2085) }, /* fiveinferior */
 {"\00\00\00\00\04\10\14\12\04", U(2086) }, /* sixinferior */
 {"\00\00\00\00\16\02\04\10\10", U(2087) }, /* seveninferior */
 {"\00\00\00\00\04\12\04\12\04", U(2088) }, /* eightinferior */
 {"\00\00\00\00\04\12\06\02\04", U(2089) }, /* nineinferior */
 {"\00\00\00\00\00\04\16\04\00", U(208A) }, /* plusinferior */
 {"\00\00\00\00\00\00\16\00\00", U(208B) }, /* minusinferior */
 {"\00\00\00\00\00\16\00\16\00", U(208C) }, /* equalsinferior */
 {"\00\00\00\00\02\04\04\04\02", U(208D) }, /* parenleftinferior */
 {"\00\00\00\00\10\04\04\04\10", U(208E) }, /* parenrightinferior */
 {"\00\00\00\00\00\00\06\12\06", U(2090) }, /* ainferior */
 {"\00\00\00\00\04\12\16\10\06", U(2091) }, /* einferior */
 {"\00\00\00\00\00\00\06\11\06", U(2092) }, /* oinferior */
 {"\00\00\00\00\00\00\12\04\12", U(2093) }, /* xinferior */
 {"\00\00\00\00\14\02\16\12\04", U(2094) }, /* schwainferior */
 {"\00\00\00\00\10\10\14\12\12", U(2095) }, /* hinferior */
 {"\00\00\00\00\10\10\12\14\12", U(2096) }, /* kinferior */
 {"\00\00\00\00\14\04\04\04\16", U(2097) }, /* linferior */
 {"\00\00\00\00\00\00\32\25\25", U(2098) }, /* minferior */
 {"\00\00\00\00\00\00\14\12\12", U(2099) }, /* ninferior */
 {"\00\00\00\00\00\14\12\14\10", U(209A) }, /* pinferior */
 {"\00\00\00\00\06\10\04\02\14", U(209B) }, /* sinferior */
 {"\00\00\00\00\04\16\04\04\02", U(209C) }, /* tinferior */

 /* Currency symbols */
 {"\16\20\27\24\16\04\07\00\00", U(20A0) }, /* euro-currency symbol */
 {"\17\10\36\10\34\10\10\00\00", 0x20a3, "franc" },
 {"\06\11\34\10\34\10\37\00\00", 0x20a4, "lira" },
 {"\30\24\30\22\27\22\21\00\00", 0x20a7, "peseta" },
 {"\31\25\25\21\25\25\26\00\00", U(20AA) }, /* newsheqel */
 {"\06\11\34\10\34\11\06\00\00", 0x20ac, "Euro" },

 /* Letterlike symbols */
 {"\16\25\24\24\24\25\16\00\00", U(2102) }, /* double-struck C */
 {"\35\25\25\27\25\25\35\00\00", U(210D) }, /* double-struck H */
 {"\31\25\23\31\25\23\21\00\00", U(2115) }, /* double-struck N */
 {"\16\21\35\33\33\35\31\21\16", U(2117) }, /* phonographic */
 {"\36\25\25\26\24\24\34\00\00", U(2119) }, /* double-struck P */
 {"\16\25\25\25\25\22\15\00\00", U(211A) }, /* double-struck Q */
 {"\36\25\25\26\25\25\35\00\00", U(211D) }, /* double-struck R */
 {"\37\05\11\22\24\24\37\00\00", U(2124) }, /* double-struck Z */
 {"\16\21\21\21\21\12\33\00\00", 0x2126, "Omega" },
 {"\33\12\21\21\21\21\16\00\00", U(2127) }, /* mho sign */
 {"\00\00\10\04\04\04\06\00\00", U(2129) }, /* turned iota */
 {"\01\01\01\17\01\01\37\00\00", U(2132) }, /* turned F */
 {"\26\11\25\02\05\05\02\00\00", U(214B) }, /* turned ampersand */
 {"\00\00\01\01\17\01\37\00\00", U(214E) }, /* small cap turned F */

 /* Number forms */
 {"\20\20\20\20\27\01\02\04\04", U(2150) }, /* oneseventh */
 {"\20\20\20\20\22\05\03\01\02", U(2151) }, /* oneninth */
 {"\10\10\10\10\00\22\25\25\22", U(2152) }, /* onetenth */
 {"\20\20\20\20\26\01\06\01\06", 0x2153, "onethird" },
 {"\20\10\10\20\32\01\02\01\02", 0x2154, "twothirds" },
 {"\20\20\20\20\27\04\06\01\06", U(2155) }, /* onefifth */
 {"\20\10\10\20\33\02\03\01\02", U(2156) }, /* twofifths */
 {"\20\10\20\10\23\02\03\01\02", U(2157) }, /* threefifths */
 {"\04\14\24\34\07\02\03\01\02", U(2158) }, /* fourfifths */
 {"\20\20\20\20\22\04\06\05\02", U(2159) }, /* onesixth */
 {"\30\20\30\10\22\04\06\05\02", U(215A) }, /* fivesixths */
 {"\20\20\20\20\22\05\02\05\02", 0x215b, "oneeighth" },
 {"\20\10\20\10\22\05\02\05\02", 0x215c, "threeeighths" },
 {"\30\20\30\10\22\05\02\05\02", 0x215d, "fiveeighths" },
 {"\34\04\10\20\22\05\02\05\02", 0x215e, "seveneighths" },
 {"\10\24\24\10\02\01\02\01\02", U(2189) }, /* zerothirds */
 {"\37\01\02\14\20\21\16\00\00", U(218A) }, /* turned 2 */
 {"\16\21\20\14\10\20\37\00\00", U(218B) }, /* turned 3 */

 /* Arrows */
 {"\00\04\04\25\16\04\00\00\00", 0x2193, "arrowdown" },
 {"\00\12\21\37\21\12\00\00\00", 0x2194, "arrowboth" },
 {"\04\16\25\04\25\16\04\00\00", 0x2195, "arrowupdn" },
 {"\00\36\30\24\22\01\00\00\00", U(2196) }, /* arrowupleft */
 {"\00\17\03\05\11\20\00\00\00", U(2197) }, /* arrowupright */
 {"\00\20\11\05\03\17\00\00\00", U(2198) }, /* arrowdownright */
 {"\00\01\22\24\30\36\00\00\00", U(2199) }, /* arrowdownleft */
 {"\00\05\11\37\11\05\00\00\00", U(21A4) }, /* arrowbarleft */
 {"\00\04\16\25\04\37\00\00\00", U(21A5) }, /* arrowbarup */
 {"\00\24\22\37\22\24\00\00\00", U(21A6) }, /* arrowbarright */
 {"\00\37\04\25\16\04\00\00\00", U(21A7) }, /* arrowbardown */
 {"\04\16\25\04\25\16\37\00\00", 0x21a8, "arrowupdnbse" },
 {"\02\01\05\11\36\10\04\00\00", U(21A9) }, /* arrowleft w/hook */
 {"\10\20\24\22\17\02\04\00\00", U(21AA) }, /* arrowright w/hook */
 {"\00\34\04\25\16\04\00\00\00", U(21B4) }, /* arrow right then down */
 {"\01\05\11\37\10\04\00\00\00", 0x21b5, "carriagereturn" },
 {"\24\30\37\30\25\03\37\03\05", U(21B9) }, /* arrowtabsleftright */
 {"\00\04\10\37\00\00\00\00\00", U(21BC) }, /* harpoonleftbarbup */
 {"\00\00\00\37\10\04\00\00\00", U(21BD) }, /* harpoonleftbarbdown */
 {"\00\04\06\05\04\04\00\00\00", U(21BE) }, /* harpoonupbarbright */
 {"\00\04\14\24\04\04\00\00\00", U(21BF) }, /* harpoonupbarbleft */
 {"\00\04\02\37\00\00\00\00\00", U(21C0) }, /* harpoonrightbarbup */
 {"\00\00\00\37\02\04\00\00\00", U(21C1) }, /* harpoonrightbarbdown */
 {"\00\04\04\05\06\04\00\00\00", U(21C2) }, /* harpoondownbarbright */
 {"\00\04\04\24\14\04\00\00\00", U(21C3) }, /* harpoondownbarbleft */
 {"\04\10\37\00\37\02\04\00\00", U(21CB) }, /* harpoonsleftright */
 {"\04\02\37\00\37\10\04\00\00", U(21CC) }, /* harpoonsrightleft */
 {"\00\06\13\20\13\06\00\00\00", 0x21d0, "arrowdblleft" },
 {"\04\12\21\33\12\12\12\00\00", 0x21d1, "arrowdblup" },
 {"\00\14\32\01\32\14\00\00\00", 0x21d2, "arrowdblright" },
 {"\12\12\12\33\21\12\04\00\00", 0x21d3, "arrowdbldown" },
 {"\00\24\30\37\30\24\00\00\00", U(21E4) }, /* arrowtableft */
 {"\00\05\03\37\03\05\00\00\00", U(21E5) }, /* arrowtabright */
 {"\00\06\13\21\13\06\00\00\00", U(21E6) }, /* white left arrow */
 {"\04\12\21\33\12\12\16\00\00", U(21E7) }, /* white up arrow */
 {"\00\14\32\21\32\14\00\00\00", U(21E8) }, /* white right arrow */
 {"\16\12\12\33\21\12\04\00\00", U(21E9) }, /* white down arrow */
 {"\04\12\21\33\16\12\16\00\00", U(21EA) }, /* caps lock arrow */

 /* Mathematical operators */
 {"\21\21\37\21\12\12\04\00\00", 0x2200, "universal" },
 {"\16\21\01\17\21\21\16\00\00", 0x2202, "partialdiff" },
 {"\16\21\20\36\21\21\16\00\00", 0xf100, "partialdiff.rtlm" },
 {"\37\01\01\37\01\01\37\00\00", 0x2203, "existential" },
 {"\37\20\20\37\20\20\37\00\00", 0xf101, "existential.rtlm" },
 {"\00\15\22\25\11\26\00\00\00", 0x2205, "emptyset" },
 {"\04\04\12\12\21\21\37\00\00", 0x2206, "Delta" },
 {"\37\21\21\12\12\04\04\00\00", 0x2207, "gradient" },
 {"\17\20\20\37\20\20\17\00\00", 0x2208, "element" },
 {"\00\17\20\37\20\17\00\00\00", U(220A) }, /* small element of */
 {"\36\01\01\37\01\01\36\00\00", 0x220b, "suchthat" },
 {"\00\36\01\37\01\36\00\00\00", U(220D) }, /* small has element */
 {"\37\21\21\21\21\21\21\21\21", 0x220f, "product" },
 {"\21\21\21\21\21\21\21\21\37", U(2210) }, /* n-ary coproduct */
 {"\37\20\10\04\02\04\10\20\37", 0x2211, "summation" },
 {"\37\01\02\04\10\04\02\01\37", 0xf102, "summation.rtlm" },
 {"\00\00\00\37\00\00\00\00\00", 0x2212, "minus" },
 {"\37\00\04\04\37\04\04\00\00", U(2213) }, /* minusplus */
 {"\04\00\04\04\37\04\04\00\00", U(2214) }, /* dot plus */
 {"\00\01\02\04\10\20\00\00\00", U(2215) }, /* division slash */
 {"\00\20\10\04\02\01\00\00\00", U(2216) }, /* set minus */
 ALIAS("uni2216.rtlm", "uni2215"),
 {"\04\25\16\04\16\25\04\00\00", 0x2217, "asteriskmath" },
 {"\00\00\04\12\12\04\00\00\00", U(2218) }, /* ring operator */
 {"\00\00\04\16\04\00\00\00\00", U(2219) }, /* bulletoperator */
 {"\07\04\04\04\24\14\04\00\00", 0x221a, "radical" },
 {"\34\04\04\04\05\06\04\00\00", 0xf103, "radical.rtlm" },
 {"\23\12\22\12\22\06\02\00\00", U(221B) }, /* cube root */
 {"\22\21\22\21\22\30\20\00\00", 0xf104, "uni221B.rtlm" },
 {"\15\25\35\05\01\03\01\00\00", U(221C) }, /* fourth root */
 {"\23\25\27\21\20\30\20\00\00", 0xf105, "uni221C.rtlm" },
 {"\00\12\25\24\25\12\00\00\00", 0x221d, "proportional" },
 {"\00\12\25\05\25\12\00\00\00", 0xf106, "proportional.rtlm" },
 {"\00\12\25\25\25\12\00\00\00", 0x221e, "infinity" },
 {"\00\20\20\20\20\37\00\00\00", 0x221f, "orthogonal" },
 {"\04\04\04\04\04\04\04\00\00", U(2223) }, /* divides */
 {"\12\12\12\12\12\12\12\00\00", U(2225) }, /* parallel */
 {"\00\00\04\12\21\00\00\00\00", 0x2227, "logicaland" },
 {"\00\00\21\12\04\00\00\00\00", 0x2228, "logicalor" },
 {"\00\16\21\21\21\21\00\00\00", 0x2229, "intersection" },
 {"\00\21\21\21\21\16\00\00\00", 0x222a, "union" },
 {"\02\05\04\04\04\04\04\24\10", 0x222b, "integral" },
 {"\10\24\04\04\04\04\04\05\02", 0xf107, "integral.rtlm" },
 {"\02\05\04\16\25\16\04\24\10", U(222E) }, /* contour integral */
 {"\10\24\04\16\25\16\04\05\02", 0xf108, "uni222E.rtlm" },
 {"\00\04\00\00\00\21\00\00\00", 0x2234, "therefore" },
 {"\00\21\00\00\00\04\00\00\00", U(2235) }, /* because */
 {"\00\04\00\00\00\04\00\00\00", U(2236) }, /* ratio */
 {"\00\04\00\37\00\00\00\00\00", U(2238) }, /* dot minus */
 {"\00\21\00\37\00\21\00\00\00", U(223A) }, /* geometric proportion */
 {"\00\00\10\25\02\00\00\00\00", 0x223c, "similar" },
 {"\00\00\02\25\10\00\00\00\00", U(223D) }, /* reversed tilde */
 {"\00\02\11\25\22\10\00\00\00", U(223E) }, /* inverted lazy S */
 {"\00\10\22\25\11\02\00\00\00", 0xf109, "uni223E.rtlm" },
 {"\00\10\24\25\05\02\00\00\00", U(223F) }, /* sinewave */
 {"\00\02\05\25\24\10\00\00\00", 0xf10a, "uni223F.rtlm" },
 {"\00\37\00\10\25\02\00\00\00", U(2242) }, /* minus tilde */
 {"\00\37\00\02\25\10\00\00\00", 0xf10b, "uni2242.rtlm" },
 {"\00\10\25\02\00\37\00\00\00", U(2243) }, /* asymptotically equal */
 {"\10\25\02\00\37\00\37\00\00", 0x2245, "congruent" },
 ALIAS("congruent.rtlm", "uni224C"),
 {"\00\10\25\02\10\25\02\00\00", 0x2248, "approxequal" },
 {"\00\02\25\10\02\25\10\00\00", 0xf10c, "approxequal.rtlm" },
 {"\02\25\10\00\37\00\37\00\00", U(224C) }, /* all equal to */
 ALIAS("uni224C.rtlm", "congruent"),
 {"\00\21\16\00\16\21\00\00\00", U(224D) }, /* equivalent to */
 {"\00\04\33\00\33\04\00\00\00", U(224E) }, /* geom equiv to */
 {"\00\04\33\00\37\00\00\00\00", U(224F) }, /* difference between */
 {"\04\00\37\00\37\00\00\00\00", U(2250) }, /* approaches limit */
 {"\04\00\37\00\37\00\04\00\00", U(2251) }, /* geometrically equal */
 {"\20\00\37\00\37\00\01\00\00", U(2252) }, /* approx equal or image */
 {"\01\00\37\00\37\00\20\00\00", U(2253) }, /* image or approx equal */
 {"\16\21\00\37\00\37\00\00\00", U(2258) }, /* corresponds to */
 {"\32\25\25\00\37\00\37\00\00", U(225E) }, /* measured by */
 {"\00\02\37\04\37\10\00\00\00", 0x2260, "notequal" },
 {"\00\10\37\04\37\02\00\00\00", 0xf10d, "notequal.rtlm" },
 {"\00\37\00\37\00\37\00\00\00", 0x2261, "equivalence" },
 {"\02\37\04\37\04\37\10\00\00", U(2262) }, /* not identical */
 {"\10\37\04\37\04\37\02\00\00", 0xf10e, "uni2262.rtlm" },
 {"\03\14\20\14\03\30\07\00\00", 0x2264, "lessequal" },
 {"\30\06\01\06\30\03\34\00\00", 0x2265, "greaterequal" },
 {"\12\04\12\12\12\04\12\00\00", U(226C) }, /* between */
 {"\00\17\20\20\20\17\00\00\00", 0x2282, "propersubset" },
 {"\00\36\01\01\01\36\00\00\00", 0x2283, "propersuperset" },
 {"\17\20\20\20\17\00\37\00\00", 0x2286, "reflexsubset" },
 {"\36\01\01\01\36\00\37\00\00", 0x2287, "reflexsuperset" },
 {"\00\21\21\25\21\16\00\00\00", U(228D) }, /* union with dot */
 {"\00\37\20\20\20\37\00\00\00", U(228F) }, /* square image of */
 {"\00\37\01\01\01\37\00\00\00", U(2290) }, /* square original of */
 {"\37\20\20\20\37\00\37\00\00", U(2291) }, /* square image or equal */
 {"\37\01\01\01\37\00\37\00\00", U(2292) }, /* square original or equal */
 {"\00\37\21\21\21\21\00\00\00", U(2293) }, /* square cap */
 {"\00\21\21\21\21\37\00\00\00", U(2294) }, /* square cup */
 {"\16\25\25\37\25\25\16\00\00", 0x2295, "circleplus" },
 {"\16\21\21\37\21\21\16\00\00", U(2296) }, /* circled minus (ESC) */
 {"\16\21\33\25\33\21\16\00\00", 0x2297, "circlemultiply" },
 {"\16\21\23\25\31\21\16\00\00", U(2298) }, /* circled slash */
 {"\16\21\21\25\21\21\16\00\00", U(2299) }, /* circled dot (SI) */
 {"\16\21\37\21\37\21\16\00\00", U(229C) }, /* circled equals */
 {"\00\37\25\37\25\37\00\00\00", U(229E) }, /* squared plus */
 {"\00\37\21\37\21\37\00\00\00", U(229F) }, /* squared minus (DLE) */
 {"\00\37\21\25\21\37\00\00\00", U(22A1) }, /* squared dot */
 {"\00\20\20\37\20\20\00\00\00", U(22A2) }, /* right tack */
 {"\00\01\01\37\01\01\00\00\00", U(22A3) }, /* left tack */
 {"\00\37\04\04\04\04\00\00\00", U(22A4) }, /* down tack */
 {"\00\04\04\04\04\37\00\00\00", 0x22a5, "perpendicular" },
 {"\00\10\10\16\10\10\00\00\00", U(22A6) }, /* assertion */
 {"\00\10\16\10\16\10\00\00\00", U(22A7) }, /* models */
 {"\00\02\16\02\16\02\00\00\00", 0xf10f, "uni22A7.rtlm" },
 {"\00\20\37\20\37\20\00\00\00", U(22A8) }, /* true */
 {"\00\24\24\27\24\24\00\00\00", U(22A9) }, /* forces */
 {"\00\24\27\24\27\24\00\00\00", U(22AB) }, /* dbl v dbl rt tstile */
 {"\00\00\02\35\02\00\00\00\00", U(22B8) }, /* multimap */
 ALIAS("uni22B8.rtlm", "uni27DC"),
 {"\00\21\12\04\00\37\00\00\00", U(22BB) }, /* xor */
 {"\00\37\00\04\12\21\00\00\00", U(22BC) }, /* nand */
 {"\00\37\00\21\12\04\00\00\00", U(22BD) }, /* nor */
 {"\04\04\04\12\12\12\21\21\21", U(22C0) }, /* n-ary logical and */
 {"\21\21\21\12\12\12\04\04\04", U(22C1) }, /* n-ary logical or */
 {"\16\21\21\21\21\21\21\21\21", U(22C2) }, /* n-ary intersection */
 {"\21\21\21\21\21\21\21\21\16", U(22C3) }, /* n-ary union */
 {"\00\04\12\21\12\04\00\00\00", U(22C4) }, /* diamond operator */
 {"\00\00\00\04\00\00\00\00\00", 0x22c5, "dotmath" },
 {"\04\25\16\04\12\21\00\00\00", U(22C6) }, /* star operator */
 {"\00\00\33\25\33\00\00\00\00", U(22C8) }, /* bowtie */
 {"\00\20\10\04\12\21\00\00\00", U(22CB) }, /* left semidirect prod */
 {"\00\01\02\04\12\21\00\00\00", U(22CC) }, /* right semidirect prod */
 {"\00\02\25\10\00\37\00\00\00", U(22CD) }, /* reversed tilde equals */
 {"\04\04\16\25\25\25\25\00\00", U(22D4) }, /* pitchfork */
 {"\07\30\03\14\20\14\03\00\00", U(22DC) }, /* equal or less than */
 {"\34\03\30\06\01\06\30\00\00", U(22DD) }, /* equal or greater than */
 {"\00\04\00\04\00\04\00\00\00", U(22EE) }, /* vertical ellipsis */
 {"\00\00\00\25\00\00\00\00\00", U(22EF) }, /* midline ellipsis */
 {"\00\01\00\04\00\20\00\00\00", U(22F0) }, /* /-diagonal ellipsis */
 {"\00\20\00\04\00\01\00\00\00", U(22F1) }, /* \-diagonal ellipsis */
 {"\37\00\17\20\37\20\17\00\00", U(22F7) }, /* small element overbar */
 {"\17\20\20\37\20\20\17\00\37", U(22F8) }, /* element of underbar */
 {"\36\01\01\37\01\01\36\00\37", 0xf110, "uni22F8.rtlm" },
 {"\37\00\36\01\37\01\36\00\00", U(22FE) }, /* small contains o'bar */

 /* Miscellaneous technical */
 {"\04\12\21\21\21\21\37\00\00", 0x2302, "house" },
 {"\16\10\10\10\10\10\10\00\00", U(2308) }, /* left ceiling */
 {"\16\02\02\02\02\02\02\00\00", U(2309) }, /* right ceiling */
 {"\10\10\10\10\10\10\16\00\00", U(230A) }, /* left floor */
 {"\02\02\02\02\02\02\16\00\00", U(230B) }, /* right floor */
 {"\00\00\37\20\20\00\00\00\00", 0x2310, "revlogicalnot" },
 {"\00\00\20\20\37\00\00\00\00", U(2319) }, /* turned logical not */
 {"\37\20\20\20\20\00\00\00\00", U(231C) }, /* top left corner */
 {"\37\01\01\01\01\00\00\00\00", U(231D) }, /* top right corner */
 {"\00\00\20\20\20\20\37\00\00", U(231E) }, /* bottom left corner */
 {"\00\00\01\01\01\01\37\00\00", U(231F) }, /* bottom right corner */
 {"\00\00\02\05\04\04\04\04\04", 0x2320, "integraltp", JOIN_D },
 {"\00\00\10\24\04\04\04\04\04", 0xf111, "integraltp.rtlm", JOIN_D},
 {"\04\04\04\04\04\24\10\00\00", 0x2321, "integralbt", JOIN_U },
 {"\04\04\04\04\04\05\02\00\00", 0xf112, "integralbt.rtlm", JOIN_U },
 {"\02\02\04\10\04\02\02\00\00", 0x2329, "angleleft" },
 {"\10\10\04\02\04\10\10\00\00", 0x232a, "angleright" },
 {"\00\37\04\04\04\37\00\00\00", U(2336) }, /* APL I-beam */
 {"\16\12\12\12\12\12\16\00\00", U(2337) }, /* APL squish quad */
 {"\37\21\37\21\37\21\37\00\00", U(2338) }, /* APL quad equal */
 {"\37\25\21\37\21\25\37\00\00", U(2339) }, /* APL quad divide */
 {"\37\25\33\21\33\25\37\00\00", U(233A) }, /* APL quad diamond */
 {"\37\21\25\33\25\21\37\00\00", U(233B) }, /* APL quad jot */
 {"\04\16\25\25\25\16\04\00\00", U(233D) }, /* APL circle stile */
 {"\16\21\25\33\25\21\16\00\00", U(233E) }, /* APL circle jot */
 {"\00\01\12\04\12\20\00\00\00", U(233F) }, /* APL slash bar */
 {"\00\20\12\04\12\01\00\00\00", U(2340) }, /* APL backslash bar */
 {"\37\21\23\25\31\21\37\00\00", U(2341) }, /* APL quad slash */
 {"\37\21\31\25\23\21\37\00\00", U(2342) }, /* APL quad backslash */
 {"\00\26\11\25\22\15\00\00\00", U(2349) }, /* APL circle backslash */
 {"\04\04\04\04\37\00\37\00\00", U(234A) }, /* APL down tack u'bar */
 {"\04\04\12\12\25\25\37\04\04", U(234B) }, /* APL delta stile */
 {"\04\16\25\25\16\04\37\00\00", U(234E) }, /* APL down tack jot */
 {"\37\00\37\04\04\04\04\00\00", U(2351) }, /* APL up tack overbar */
 {"\04\04\37\25\25\12\12\04\04", U(2352) }, /* APL del stile */
 {"\37\04\16\25\25\16\04\00\00", U(2355) }, /* APL up tack jot */
 {"\04\04\12\12\21\21\37\00\37", U(2359) }, /* APL delta underbar */
 {"\00\16\25\33\25\21\00\00\00", U(235D) }, /* APL up shoe jot */
 {"\37\25\25\21\21\21\37\00\00", U(235E) }, /* APL quote quad */
 {"\16\25\37\25\33\21\16\00\00", U(235F) }, /* APL circle star */
 {"\37\21\25\21\25\21\37\00\00", U(2360) }, /* APL quad colon */
 {"\12\00\37\04\04\04\04\00\00", U(2361) }, /* APL up tack dieresis */
 {"\12\00\37\21\12\12\04\00\00", U(2362) }, /* APL del dieresis */
 {"\12\00\25\16\04\12\21\00\00", U(2363) }, /* APL star dieresis */
 {"\12\00\04\12\12\04\00\00\00", U(2364) }, /* APL jot dieresis */
 {"\12\00\16\21\21\16\00\00\00", U(2365) }, /* APL circle dieresis */
 {"\12\00\10\25\02\00\00\00\00", U(2368) }, /* APL tilde dieresis */
 {"\00\00\00\37\00\00\04\04\10", U(236A) }, /* APL comma bar */
 {"\37\21\21\16\33\04\04\00\00", U(236B) }, /* APL del tilde */
 {"\04\12\21\37\21\12\04\00\00", U(236C) }, /* APL zilde */
 {"\10\25\02\00\21\12\04\00\00", U(2371) }, /* APL nor */
 {"\10\25\02\00\04\12\21\00\00", U(2372) }, /* APL nand */
 {"\00\00\14\04\04\04\02\00\00", U(2373) }, /* APL iota */
 {"\00\00\16\21\21\21\36\20\20", U(2374) }, /* APL rho */
 {"\00\00\12\21\25\25\12\00\00", U(2375) }, /* APL omega */
 {"\00\00\15\22\22\22\15\00\37", U(2376) }, /* APL alpha underbar */
 {"\17\20\37\20\17\00\37\00\00", U(2377) }, /* APL epsilon underbar */
 {"\00\00\14\04\04\04\02\00\37", U(2378) }, /* APL iota underbar */
 {"\00\00\12\21\25\25\12\00\37", U(2379) }, /* APL omega underbar */
 {"\00\00\15\22\22\22\15\00\00", U(237A) }, /* APL alpha */
 {"\00\00\05\02\25\10\00\00\00", U(237B) }, /* crossed tick (NAK) */
 {"\00\00\00\00\00\33\16\00\00", U(237D) }, /* nbsp symbol */
 {"\00\16\21\37\12\33\00\00\00", U(237E) }, /* bell symbol (BEL) */
 {"\04\04\04\12\04\04\04\00\00", U(237F) }, /* vert line w/dot (EOM) */
 {"\16\21\21\33\25\21\16\00\00", U(2389) }, /* circled notched bar */
 {"\16\21\37\33\25\21\16\00\00", U(238A) }, /* circled down triangle */
 {"\00\16\12\12\12\33\00\00\00", U(238D) }, /* monostable (SYN) */
 {"\00\07\12\12\12\34\00\00\00", U(238E) }, /* hysteresis */
 {"\00\00\37\00\25\00\00\00\00", U(2393) }, /* directcurrent */
 {"\37\21\21\21\21\21\37\00\00", U(2395) }, /* APL quad */
 {"\02\04\10\10\10\10\10\10\10", U(239B), JOIN_D }, /* long parenleft top */
 {"\10\10\10\10\10\10\10\10\10", U(239C), JOIN_V }, /* long parenleft middle */
 {"\10\10\10\10\10\04\02\00\00", U(239D), JOIN_U }, /* long parenleft bottom */
 {"\10\04\02\02\02\02\02\02\02", U(239E), JOIN_D }, /* long parenright top */
 {"\02\02\02\02\02\02\02\02\02", U(239F), JOIN_V }, /* long parenright middle */
 {"\02\02\02\02\02\04\10\00\00", U(23A0), JOIN_U }, /* long parenright bottom */
 {"\17\10\10\10\10\10\10\10\10", U(23A1), JOIN_D }, /* long bracketleft top */
 {"\10\10\10\10\10\10\10\10\10", U(23A2), JOIN_V }, /* long bracketleft mid */
 {"\10\10\10\10\10\10\17\00\00", U(23A3), JOIN_U }, /* long bracketleft bot */
 {"\36\02\02\02\02\02\02\02\02", U(23A4), JOIN_D }, /* long bracketright top */
 {"\02\02\02\02\02\02\02\02\02", U(23A5), JOIN_V }, /* long bracketright mid */
 {"\02\02\02\02\02\02\36\00\00", U(23A6), JOIN_U }, /* long bracketright bot */
 {"\03\04\04\04\04\04\04\04\04", U(23A7), JOIN_D }, /* long braceleft top */
 {"\04\04\04\10\04\04\04\04\04", U(23A8), JOIN_V }, /* long braceleft middle */
 {"\04\04\04\04\04\04\03\00\00", U(23A9), JOIN_U }, /* long braceleft bottom */
 {"\04\04\04\04\04\04\04\04\04", U(23AA), JOIN_V }, /* long brace extension */
 {"\30\04\04\04\04\04\04\04\04", U(23AB), JOIN_D }, /* long braceright top */
 {"\04\04\04\02\04\04\04\04\04", U(23AC), JOIN_V }, /* long braceright middle */
 {"\04\04\04\04\04\04\30\00\00", U(23AD), JOIN_U }, /* long braceright bottom */
 {"\04\04\04\04\04\04\04\04\04", U(23AE), JOIN_V }, /* integral extension */
 {"\03\04\04\04\04\04\04\04\30", U(23B0) }, /* two-level brace / */
 {"\30\04\04\04\04\04\04\04\03", U(23B1) }, /* two-level brace \ */
 {"\37\20\10\10\04\04\02\02\01", U(23B2), JOIN_D }, /* summation top */
 {"\01\02\02\04\04\10\10\20\37", U(23B3), JOIN_U }, /* summation bottom */
 {"\37\21\21\00\00\00\00\00\00", U(23B4) }, /* brackettop */
 {"\00\00\00\00\21\21\37\00\00", U(23B5) }, /* bracketbottom */
 {"\21\21\37\00\37\21\21\00\00", U(23B6) }, /* bracketbottom + brackettop */
 {"\37\00\00\00\00\00\00\00\00", U(23BA), JOIN_H }, /* horizontal scan 1 */
 {"\00\00\37\00\00\00\00\00\00", U(23BB), JOIN_H }, /* horizontal scan 3 */
 {"\00\00\00\00\00\00\37\00\00", U(23BC), JOIN_H }, /* horizontal scan 7 */
 {"\00\00\00\00\00\00\00\00\37", U(23BD), JOIN_H }, /* horizontal scan 9 */
 {"\04\04\37\00\16\00\04\00\00", U(23DA) }, /* earth */
 {"\00\00\00\00\22\25\25\25\22", U(23E8) }, /* decimal exponent symbol */

 /* Control pictures */
 {"\32\26\22\22\00\11\11\11\06", U(2400) }, /* NUL */
 {"\10\20\10\20\05\05\07\05\05", U(2401) }, /* SOH */
 {"\10\20\10\20\05\05\02\05\05", U(2402) }, /* STX */
 {"\34\20\30\20\35\05\02\05\05", U(2403) }, /* ETX */
 {"\34\20\30\20\37\02\02\02\02", U(2404) }, /* EOT */
 {"\34\20\30\22\35\05\05\02\01", U(2405) }, /* ENQ */
 {"\10\24\34\24\25\05\06\05\05", U(2406) }, /* ACK */
 {"\30\24\30\24\30\02\02\02\03", U(2407) }, /* BEL */
 {"\30\24\30\24\30\01\02\01\02", U(2408) }, /* BS */
 {"\24\24\34\24\27\02\02\02\02", U(2409) }, /* HT */
 {"\20\20\20\20\37\04\06\04\04", U(240A) }, /* LF */
 {"\24\24\24\10\17\02\02\02\02", U(240B) }, /* VT */
 {"\34\20\30\20\27\04\06\04\04", U(240C) }, /* FF */
 {"\14\20\20\20\16\05\06\05\05", U(240D) }, /* CR */
 {"\14\20\10\04\30\02\05\05\02", U(240E) }, /* SO */
 {"\14\20\10\04\31\01\01\01\01", U(240F) }, /* SI */
 {"\30\24\24\24\30\02\02\02\03", U(2410) }, /* DLE */
 {"\30\24\24\24\31\01\01\01\01", U(2411) }, /* DC1 */
 {"\30\24\24\24\30\02\01\02\03", U(2412) }, /* DC2 */
 {"\30\24\24\30\02\01\02\01\02", U(2413) }, /* DC3 */
 {"\30\24\24\24\31\03\05\07\01", U(2414) }, /* DC4 */
 {"\32\26\22\22\00\05\06\05\05", U(2415) }, /* NAK */
 {"\10\20\10\20\05\05\02\02\02", U(2416) }, /* SYN */
 {"\34\20\30\20\36\05\06\05\06", U(2417) }, /* ETB */
 {"\14\20\20\14\00\15\13\11\11", U(2418) }, /* CAN */
 {"\34\20\30\20\34\00\33\25\21", U(2419) }, /* EOM */
 {"\10\20\10\20\06\05\06\05\06", U(241A) }, /* SUB */
 {"\30\20\30\20\30\03\04\04\03", U(241B) }, /* ESC */
 {"\34\20\30\20\23\04\02\01\06", U(241C) }, /* FS */
 {"\14\20\20\24\14\01\02\01\02", U(241D) }, /* GS */
 {"\30\24\30\24\24\01\02\01\02", U(241E) }, /* RS */
 {"\22\22\14\00\03\04\02\01\06", U(241F) }, /* US */
 {"\10\20\10\20\06\05\06\04\04", U(2420) }, /* SP */
 {"\30\24\24\30\00\07\02\02\02", U(2421) }, /* DEL */
 {"\00\00\00\00\00\21\37\00\00", U(2423) }, /* Visible space */
 {"\32\26\22\22\00\04\04\04\07", U(2424) }, /* NL */
 {"\04\11\22\04\11\22\04\00\00", U(2425) }, /* pictorial DEL */
 {"\16\21\10\04\04\00\04\00\00", U(2426) }, /* pictorial SUB */

 /* Box drawing */
 {"\00\00\00\00\37\00\00\00\00", 0x2500, "SF100000", JOIN },
 {"\00\00\00\37\37\37\00\00\00", U(2501), JOIN },
 {"\04\04\04\04\04\04\04\04\04", 0x2502, "SF110000", JOIN },
 {"\16\16\16\16\16\16\16\16\16", U(2503), JOIN },
 {"\00\00\00\00\25\00\00\00\00", U(2504) },
 {"\00\00\00\25\25\25\00\00\00", U(2505) },
 {"\04\04\00\04\04\04\00\04\04", U(2506) },
 {"\16\16\00\16\16\16\00\16\16", U(2507) },
 {"\04\00\04\04\00\04\00\04\04", U(250A) },
 {"\16\00\16\16\00\16\00\16\16", U(250B) },
 {"\00\00\00\00\07\04\04\04\04", 0x250c, "SF010000", JOIN },
 {"\00\00\00\07\07\07\04\04\04", U(250D), JOIN },
 {"\00\00\00\00\17\16\16\16\16", U(250E), JOIN },
 {"\00\00\00\17\17\17\16\16\16", U(250F), JOIN },
 {"\00\00\00\00\34\04\04\04\04", 0x2510, "SF030000", JOIN },
 {"\00\00\00\34\34\34\04\04\04", U(2511), JOIN },
 {"\00\00\00\00\36\16\16\16\16", U(2512), JOIN },
 {"\00\00\00\36\36\36\16\16\16", U(2513), JOIN },
 {"\04\04\04\04\07\00\00\00\00", 0x2514, "SF020000", JOIN },
 {"\04\04\04\07\07\07\00\00\00", U(2515), JOIN },
 {"\16\16\16\16\17\00\00\00\00", U(2516), JOIN },
 {"\16\16\16\17\17\17\00\00\00", U(2517), JOIN },
 {"\04\04\04\04\34\00\00\00\00", 0x2518, "SF040000", JOIN },
 {"\04\04\04\34\34\34\00\00\00", U(2519), JOIN },
 {"\16\16\16\16\36\00\00\00\00", U(251A), JOIN },
 {"\16\16\16\36\36\36\00\00\00", U(251B), JOIN },
 {"\04\04\04\04\07\04\04\04\04", 0x251c, "SF080000", JOIN },
 {"\04\04\04\07\07\07\04\04\04", U(251D), JOIN },
 {"\16\16\16\16\17\04\04\04\04", U(251E), JOIN },
 {"\04\04\04\04\17\16\16\16\16", U(251F), JOIN },
 {"\16\16\16\16\17\16\16\16\16", U(2520), JOIN },
 {"\16\16\16\17\17\17\04\04\04", U(2521), JOIN },
 {"\04\04\04\17\17\17\16\16\16", U(2522), JOIN },
 {"\16\16\16\17\17\17\16\16\16", U(2523), JOIN },
 {"\04\04\04\04\34\04\04\04\04", 0x2524, "SF090000", JOIN },
 {"\04\04\04\34\34\34\04\04\04", U(2525), JOIN },
 {"\16\16\16\16\36\04\04\04\04", U(2526), JOIN },
 {"\04\04\04\04\36\16\16\16\16", U(2527), JOIN },
 {"\16\16\16\16\36\16\16\16\16", U(2528), JOIN },
 {"\16\16\16\36\36\36\04\04\04", U(2529), JOIN },
 {"\04\04\04\36\36\36\16\16\16", U(252A), JOIN },
 {"\16\16\16\36\36\36\16\16\16", U(252B), JOIN },
 {"\00\00\00\00\37\04\04\04\04", 0x252c, "SF060000", JOIN },
 {"\00\00\00\34\37\34\04\04\04", U(252D), JOIN },
 {"\00\00\00\07\37\07\04\04\04", U(252E), JOIN },
 {"\00\00\00\37\37\37\04\04\04", U(252F), JOIN },
 {"\00\00\00\00\37\16\16\16\16", U(2530), JOIN },
 {"\00\00\00\36\37\36\16\16\16", U(2531), JOIN },
 {"\00\00\00\17\37\17\16\16\16", U(2532), JOIN },
 {"\00\00\00\37\37\37\16\16\16", U(2533), JOIN },
 {"\04\04\04\04\37\00\00\00\00", 0x2534, "SF070000", JOIN },
 {"\04\04\04\34\37\34\00\00\00", U(2535), JOIN },
 {"\04\04\04\07\37\07\00\00\00", U(2536), JOIN },
 {"\04\04\04\37\37\37\00\00\00", U(2537), JOIN },
 {"\16\16\16\16\37\00\00\00\00", U(2538), JOIN },
 {"\16\16\16\36\37\36\00\00\00", U(2539), JOIN },
 {"\16\16\16\17\37\17\00\00\00", U(253A), JOIN },
 {"\16\16\16\37\37\37\00\00\00", U(253B), JOIN },
 {"\04\04\04\04\37\04\04\04\04", 0x253c, "SF050000", JOIN },
 {"\04\04\04\34\37\34\04\04\04", U(253D), JOIN },
 {"\04\04\04\07\37\07\04\04\04", U(253E), JOIN },
 {"\04\04\04\37\37\37\04\04\04", U(253F), JOIN },
 {"\16\16\16\16\37\04\04\04\04", U(2540), JOIN },
 {"\04\04\04\04\37\16\16\16\16", U(2541), JOIN },
 {"\16\16\16\16\37\16\16\16\16", U(2542), JOIN },
 {"\16\16\16\36\37\36\04\04\04", U(2543), JOIN },
 {"\16\16\16\17\37\17\04\04\04", U(2544), JOIN },
 {"\04\04\04\36\37\36\16\16\16", U(2545), JOIN },
 {"\04\04\04\17\37\17\16\16\16", U(2546), JOIN },
 {"\16\16\16\37\37\37\04\04\04", U(2547), JOIN },
 {"\04\04\04\37\37\37\16\16\16", U(2548), JOIN },
 {"\16\16\16\36\37\36\16\16\16", U(2549), JOIN },
 {"\16\16\16\17\37\17\16\16\16", U(254A), JOIN },
 {"\16\16\16\37\37\37\16\16\16", U(254B), JOIN },
 {"\00\00\00\00\33\00\00\00\00", U(254C) },
 {"\00\00\00\33\33\33\00\00\00", U(254D) },
 {"\04\04\04\04\00\04\04\04\04", U(254E) },
 {"\16\16\16\16\00\16\16\16\16", U(254F) },
 {"\00\00\00\37\00\37\00\00\00", 0x2550, "SF430000", JOIN },
 {"\12\12\12\12\12\12\12\12\12", 0x2551, "SF240000", JOIN },
 {"\00\00\00\07\04\07\04\04\04", 0x2552, "SF510000", JOIN },
 {"\00\00\00\00\17\12\12\12\12", 0x2553, "SF520000", JOIN },
 {"\00\00\00\17\10\13\12\12\12", 0x2554, "SF390000", JOIN },
 {"\00\00\00\34\04\34\04\04\04", 0x2555, "SF220000", JOIN },
 {"\00\00\00\00\36\12\12\12\12", 0x2556, "SF210000", JOIN },
 {"\00\00\00\36\02\32\12\12\12", 0x2557, "SF250000", JOIN },
 {"\04\04\04\07\04\07\00\00\00", 0x2558, "SF500000", JOIN },
 {"\12\12\12\12\17\00\00\00\00", 0x2559, "SF490000", JOIN },
 {"\12\12\12\13\10\17\00\00\00", 0x255a, "SF380000", JOIN },
 {"\04\04\04\34\04\34\00\00\00", 0x255b, "SF280000", JOIN },
 {"\12\12\12\12\36\00\00\00\00", 0x255c, "SF270000", JOIN },
 {"\12\12\12\32\02\36\00\00\00", 0x255d, "SF260000", JOIN },
 {"\04\04\04\07\04\07\04\04\04", 0x255e, "SF360000", JOIN },
 {"\12\12\12\12\13\12\12\12\12", 0x255f, "SF370000", JOIN },
 {"\12\12\12\13\10\13\12\12\12", 0x2560, "SF420000", JOIN },
 {"\04\04\04\34\04\34\04\04\04", 0x2561, "SF190000", JOIN },
 {"\12\12\12\12\32\12\12\12\12", 0x2562, "SF200000", JOIN },
 {"\12\12\12\32\02\32\12\12\12", 0x2563, "SF230000", JOIN },
 {"\00\00\00\37\00\37\04\04\04", 0x2564, "SF470000", JOIN },
 {"\00\00\00\00\37\12\12\12\12", 0x2565, "SF480000", JOIN },
 {"\00\00\00\37\00\33\12\12\12", 0x2566, "SF410000", JOIN },
 {"\04\04\04\37\00\37\00\00\00", 0x2567, "SF450000", JOIN },
 {"\12\12\12\12\37\00\00\00\00", 0x2568, "SF460000", JOIN },
 {"\12\12\12\33\00\37\00\00\00", 0x2569, "SF400000", JOIN },
 {"\04\04\04\37\04\37\04\04\04", 0x256a, "SF540000", JOIN },
 {"\12\12\12\12\37\12\12\12\12", 0x256b, "SF530000", JOIN },
 {"\12\12\12\33\00\33\12\12\12", 0x256c, "SF440000", JOIN },
 {"\00\00\00\00\01\02\04\04\04", U(256D), JOIN },
 {"\00\00\00\00\20\10\04\04\04", U(256E), JOIN },
 {"\04\04\04\10\20\00\00\00\00", U(256F), JOIN },
 {"\04\04\04\02\01\00\00\00\00", U(2570), JOIN },
 {"\00\00\00\00\34\00\00\00\00", U(2574), JOIN },
 {"\04\04\04\04\04\00\00\00\00", U(2575), JOIN },
 {"\00\00\00\00\07\00\00\00\00", U(2576), JOIN },
 {"\00\00\00\00\04\04\04\04\04", U(2577), JOIN },
 {"\00\00\00\34\34\34\00\00\00", U(2578), JOIN },
 {"\16\16\16\16\16\00\00\00\00", U(2579), JOIN },
 {"\00\00\00\07\07\07\00\00\00", U(257A), JOIN },
 {"\00\00\00\00\16\16\16\16\16", U(257B), JOIN },
 {"\00\00\00\07\37\07\00\00\00", U(257C), JOIN },
 {"\04\04\04\04\16\16\16\16\16", U(257D), JOIN },
 {"\00\00\00\34\37\34\00\00\00", U(257E), JOIN },
 {"\16\16\16\16\16\04\04\04\04", U(257F), JOIN },

 /* Block elements */
 {{0x03}, 0x2580, "upblock", MOS4 },
 UALIAS(U(2582), "u1FB2D"), /* lower quarter block (octant-78) */
 {{0x0c}, 0x2584, "dnblock", MOS4 },
 UALIAS(U(2586), "u1FB39"), /* lower three-quarter block (octant-345678) */
 {"\25\00\12\00\25\00\12\00\25", 0x2591, "ltshade" },
 {"\22\11\04\22\11\04\22\11\04", 0x2592, "shade" },
 {"\25\37\12\37\25\37\12\37\25", 0x2593, "dkshade" },
#define M(x, u) { {x}, U(u), MOS4 }
 M( 4, 2596), M( 8, 2597), M( 1, 2598), M(13, 2599),
 M( 9, 259A), M( 7, 259B), M(11, 259C), M( 2, 259D),
 M( 6, 259E), M(14, 259F),
#undef M

 /* Geometric shapes */
 {"\37\21\21\21\21\21\37\00\00", 0x25a1, "H22073" },
 {"\00\00\16\16\16\00\00\00\00", 0x25aa, "H18543" },
 {"\00\00\16\12\16\00\00\00\00", 0x25ab, "H18551" },
 {"\00\00\37\37\37\00\00\00\00", 0x25ac, "filledrect" },
 {"\00\00\37\21\37\00\00\00\00", U(25AD) }, /* rect */
 {"\04\04\16\16\37\37\37\00\00", 0x25b2, "triagup" },
 {"\10\14\16\17\16\14\10\00\00", U(25B6) }, /* blacktriangleright */
 {"\30\24\22\21\22\24\30\00\00", U(25B7) }, /* whitetriangleright */
 {"\00\20\34\37\34\20\00\00\00", 0x25ba, "triagrt" },
 {"\37\37\37\16\16\04\04\00\00", 0x25bc, "triagdn" },
 {"\01\03\07\17\07\03\01\00\00", U(25C0) }, /* blacktriangleleft */
 {"\03\05\11\21\11\05\03\00\00", U(25C1) }, /* whitetriangleleft */
 {"\00\01\07\37\07\01\00\00\00", 0x25c4, "triaglf" },
 {"\00\04\16\37\16\04\00\00\00", U(25C6) }, /* black diamond shape */
 {"\00\04\12\21\12\04\00\00\00", U(25C7) }, /* white diamond shape */
 {"\04\12\12\21\12\12\04\00\00", 0x25ca, "lozenge" },
 {"\00\16\21\21\21\16\00\00\00", 0x25cb, "circle" },
 {"\00\04\21\00\21\04\00\00\00", U(25CC) }, /* dottedcircle */
 {"\00\16\37\37\37\16\00\00\00", 0x25cf, "H18533" },
 {"\37\37\21\21\21\37\37\00\00", 0x25d8, "invbullet" },
 {"\37\21\04\16\04\21\37\00\00", 0x25d9, "invcircle" },
 {"\00\00\16\12\16\00\00\00\00", 0x25e6, "openbullet" },
 {"\00\37\25\25\25\37\00\00\00", U(25EB) }, /* squared vertical line */
 {"\00\37\25\35\21\37\00\00\00", U(25F0) }, /* square UL quad (FS) */
 {"\00\37\21\35\25\37\00\00\00", U(25F1) }, /* square LL quad (GS) */
 {"\00\37\21\27\25\37\00\00\00", U(25F2) }, /* square LR quad (RS) */
 {"\00\37\25\27\21\37\00\00\00", U(25F3) }, /* square UR quad (US) */
 {"\00\16\25\35\21\16\00\00\00", U(25F4) }, /* circle UL quad (DC4) */
 {"\00\16\21\35\25\16\00\00\00", U(25F5) }, /* circle LL quad (DC3) */
 {"\00\16\21\27\25\16\00\00\00", U(25F6) }, /* circle LR quad (DC2) */
 {"\00\16\25\27\21\16\00\00\00", U(25F7) }, /* circle UR quad (DC1) */

 /* Miscellaneous symbols */
 {"\01\02\04\10\05\03\07\00\00", U(2607) }, /* lightning */
 {"\37\22\24\30\25\23\27\00\00", U(2608) }, /* thunderstorm */
 {"\00\16\21\25\21\16\00\00\00", U(2609) }, /* astrological sun */
 {"\00\37\00\37\00\37\00\00\00", U(2630) }, /* Yijing trigrams... */
 {"\00\33\00\37\00\37\00\00\00", U(2631) },
 {"\00\37\00\33\00\37\00\00\00", U(2632) },
 {"\00\33\00\33\00\37\00\00\00", U(2633) },
 {"\00\37\00\37\00\33\00\00\00", U(2634) },
 {"\00\33\00\37\00\33\00\00\00", U(2635) },
 {"\00\37\00\33\00\33\00\00\00", U(2636) },
 {"\00\33\00\33\00\33\00\00\00", U(2637) },
 {"\00\12\00\00\16\21\00\00\00", U(2639) }, /* frownface */
 {"\00\12\00\00\21\16\00\00\00", 0x263a, "smileface" },
 {"\16\37\25\37\37\21\16\00\00", 0x263b, "invsmileface" },
 {"\00\25\16\33\16\25\00\00\00", 0x263c, "sun" },
 {"\16\21\21\16\04\16\04\00\00", 0x2640, "female" },
 {"\07\03\05\14\22\22\14\00\00", 0x2642, "male" },
 {"\00\04\16\37\37\04\16\00\00", 0x2660, "spade" },
 {"\00\12\25\21\12\04\00\00\00", U(2661) }, /* white heart */
 {"\00\04\12\21\12\04\00\00\00", U(2662) }, /* white diamond */
 {"\00\16\16\37\37\04\16\00\00", 0x2663, "club" },
 {"\04\12\21\21\37\04\16\00\00", U(2664) }, /* white spade */
 {"\00\12\37\37\16\04\00\00\00", 0x2665, "heart" },
 {"\00\04\16\37\16\04\00\00\00", 0x2666, "diamond" },
 {"\16\12\33\21\37\04\16\00\00", U(2667) }, /* white club */
 {"\02\02\02\02\06\16\04\00\00", U(2669) }, /* crotchet */
 {"\04\06\05\05\14\34\10\00\00", 0x266a, "musicalnote" },
 {"\03\15\11\11\13\33\30\00\00", 0x266b, "musicalnotedbl" },
 {"\17\11\17\11\11\33\33\00\00", U(266C) }, /* semiquavers */
 {"\10\10\13\15\11\12\14\00\00", U(266D) }, /* flat */
 {"\10\13\15\11\13\15\01\00\00", U(266E) }, /* natural */
 {"\02\13\16\33\16\32\10\00\00", U(266F) }, /* sharp */
 {"\00\16\21\23\21\16\00\00\00", U(2686) }, /* white circle dot right */
 {"\00\16\21\33\21\16\00\00\00", U(2687) }, /* white circle two dots */
 {"\00\16\37\35\37\16\00\00\00", U(2688) }, /* black circle dot right */
 {"\00\16\37\25\37\16\00\00\00", U(2689) }, /* black circle two dots */
 {"\00\00\00\37\00\00\00\00\00", U(268A) }, /* Yijing monogram yang */
 {"\00\00\00\33\00\00\00\00\00", U(268B) }, /* Yijing monogram yin */
 {"\00\00\37\00\37\00\00\00\00", U(268C) }, /* Yijing digrams... */
 {"\00\00\33\00\37\00\00\00\00", U(268D) },
 {"\00\00\37\00\33\00\00\00\00", U(268E) },
 {"\00\00\33\00\33\00\00\00\00", U(268F) },

 /* Dingbats */
 {"\00\04\22\17\22\04\00\00\00", U(2708) }, /* airplane */
 {"\00\00\01\02\24\10\00\00\00", U(2713) }, /* tick (ACK) */
 {"\16\04\25\37\25\04\16\00\00", U(2720) }, /* maltese cross */

 /* Miscellaneous mathematical symbols-A */
 {"\04\04\04\16\04\04\04\00\00", U(27CA) }, /* vert bar horiz stroke */
 {"\00\25\25\25\25\16\00\00\00", U(27D2) }, /* upward element of */
 {"\04\04\04\04\04\04\04\04\37", U(27D8) }, /* large up tack */
 {"\37\04\04\04\04\04\04\04\04", U(27D9) }, /* large down tack */
 {"\00\00\10\27\10\00\00\00\00", U(27DC) }, /* left multimap */
 ALIAS("uni27DC.rtlm", "uni22B8"),
 {"\02\02\04\10\04\02\02\00\00", U(27E8) }, /* left angle bracket */
 {"\10\10\04\02\04\10\10\00\00", U(27E9) }, /* right angle bracket */
 {"\02\04\04\04\04\04\02\00\00", U(27EE) }, /* flattened parenleft */
 {"\10\04\04\04\04\04\10\00\00", U(27EF) }, /* flattened parenright */

 /* Supplemental arrows-B */
 {"\00\37\16\25\04\04\00\00\00", U(2912) }, /* arrowupbar */
 {"\00\04\04\25\16\37\00\00\00", U(2913) }, /* arrowdownbar */
 {"\00\04\16\25\04\30\00\00\00", U(2934) }, /* arrow right curve up */
 {"\00\30\04\25\16\04\00\00\00", U(2935) }, /* arrow right curve down */
 {"\01\05\11\36\10\04\00\00\00", U(2936) }, /* arrow down curve left */
 {"\20\24\22\17\02\04\00\00\00", U(2937) }, /* arrow down curve right */
 {"\00\04\10\37\02\04\00\00\00", U(294A) }, /* harpoon lf up rt dn */
 {"\00\04\02\37\10\04\00\00\00", U(294B) }, /* harpoon lf dn rt up */
 {"\04\06\05\04\24\14\04\00\00", U(294C) }, /* harpoon up rt dn lf */
 {"\04\14\24\04\05\06\04\00\00", U(294D) }, /* harpoon up lf dn rt */
 {"\00\12\21\37\00\00\00\00\00", U(294E) }, /* harpoon lf up rt up */
 {"\04\06\05\04\05\06\04\00\00", U(294F) }, /* harpoon up rt dn rt */
 {"\00\00\00\37\21\12\00\00\00", U(2950) }, /* harpoon lf dn rt dn */
 {"\04\14\24\04\24\14\04\00\00", U(2951) }, /* harpoon up lf dn lf */
 {"\00\24\30\37\20\20\00\00\00", U(2952) }, /* harpoons to bars... */
 {"\00\05\03\37\01\01\00\00\00", U(2953) },
 {"\00\37\06\05\04\04\00\00\00", U(2954) },
 {"\00\04\04\05\06\37\00\00\00", U(2955) },
 {"\00\20\20\37\30\24\00\00\00", U(2956) },
 {"\00\01\01\37\03\05\00\00\00", U(2957) },
 {"\00\37\14\24\04\04\00\00\00", U(2958) },
 {"\00\04\04\24\14\37\00\00\00", U(2959) },
 {"\00\05\11\37\01\01\00\00\00", U(295A) }, /* ... and from bars */
 {"\00\24\22\37\20\20\00\00\00", U(295B) },
 {"\00\04\06\05\04\37\00\00\00", U(295C) },
 {"\00\37\04\05\06\04\00\00\00", U(295D) },
 {"\00\01\01\37\11\05\00\00\00", U(295E) },
 {"\00\20\20\37\22\24\00\00\00", U(295F) },
 {"\00\04\14\24\04\37\00\00\00", U(2960) },
 {"\00\37\04\24\14\04\00\00\00", U(2961) },
 {"\04\10\37\00\37\10\04\00\00", U(2962) }, /* paired harpoons */
 {"\04\02\37\00\37\02\04\00\00", U(2964) },
 {"\04\10\37\00\04\02\37\00\00", U(2966) },
 {"\37\10\04\00\37\02\04\00\00", U(2967) },
 {"\04\02\37\00\04\10\37\00\00", U(2968) },
 {"\37\02\04\00\37\10\04\00\00", U(2969) },
 {"\04\10\37\00\37\00\00\00\00", U(296A) },
 {"\00\00\37\00\37\10\04\00\00", U(296B) },
 {"\04\02\37\00\37\00\00\00\00", U(296C) },
 {"\00\00\37\00\37\02\04\00\00", U(296D) },

 /* Miscellaneous mathematical symbols-B */
 {"\25\25\25\25\25\25\25\00\00", U(2980) }, /* triple vertical bar */
 {"\00\16\25\25\25\16\00\00\00", U(29B6) }, /* circled vert bar */
 {"\00\16\25\37\21\16\00\00\00", U(29BA) }, /* circled horiz up */

 /* Supplemental mathematical operators */
 {"\16\21\21\21\25\21\21\21\16", U(2A00) }, /* n-ary circled dot */
 {"\16\21\25\25\37\25\25\21\16", U(2A01) }, /* n-ary circled plus */
 {"\16\21\21\33\25\33\21\21\16", U(2A02) }, /* n-ary circled times */
 {"\21\21\21\21\25\21\21\21\16", U(2A03) }, /* n-ary union dot */
 {"\21\21\25\25\37\25\25\21\16", U(2A04) }, /* n-ary union plus */
 {"\02\05\04\04\37\04\04\24\10", U(2A0D) }, /* integral with bar */
 {"\10\24\04\04\37\04\04\05\02", 0xf113, "uni2A0D.rtlm" },
 {"\02\05\04\37\04\37\04\24\10", U(2A0E) }, /* integral w/dbl bar */
 {"\10\24\04\37\04\37\04\05\02", 0xf114, "uni2A0E.rtlm" },
 {"\02\05\04\37\25\37\04\24\10", U(2A16) }, /* integral w/square */
 {"\10\24\04\37\25\37\04\05\02", 0xf115, "uni2A16.rtlm" },
 {"\02\05\04\16\25\25\04\24\10", U(2A19) }, /* integral w/cap */
 {"\10\24\04\16\25\25\04\05\02", 0xf116, "uni2A19.rtlm" },
 {"\02\05\04\25\25\16\04\24\10", U(2A1A) }, /* integral w/cup */
 {"\10\24\04\25\25\16\04\05\02", 0xf117, "uni2A1A.rtlm" },
 {"\04\04\37\04\04\00\04\00\00", U(2A25) }, /* plus dot */
 {"\00\00\00\37\00\04\00\00\00", U(2A2A) }, /* minus dot */
 {"\00\20\00\37\00\01\00\00\00", U(2A2B) }, /* minus falling dots */
 {"\00\01\00\37\00\20\00\00\00", U(2A2C) }, /* minus rising dots */
 {"\04\21\12\04\12\21\00\00\00", U(2A30) }, /* dot times */
 {"\00\16\21\25\21\21\00\00\00", U(2A40) }, /* intersection with dot */
 {"\04\00\04\12\21\00\00\00\00", U(2A51) }, /* dot and */
 {"\04\00\21\12\04\00\00\00\00", U(2A52) }, /* dot or */
 {"\37\00\37\00\04\12\21\00\00", U(2A5E) }, /* double bar and */
 {"\00\04\12\21\00\37\00\00\00", U(2A5F) }, /* and bar */
 {"\04\12\21\00\37\00\37\00\00", U(2A60) }, /* and double bar */
 {"\37\00\37\00\21\12\04\00\00", U(2A62) }, /* double bar or */
 {"\21\12\04\00\37\00\37\00\00", U(2A63) }, /* or double bar */
 {"\00\00\37\00\37\00\04\00\00", U(2A66) }, /* equals dot */
 {"\04\00\37\00\37\00\37\00\00", U(2A67) }, /* dot identical */
 {"\04\00\10\25\02\00\00\00\00", U(2A6A) }, /* dot tilde */
 {"\04\00\02\25\10\00\00\00\00", 0xf118, "uni2A6A.rtlm" },
 {"\00\01\10\25\02\20\00\00\00", U(2A6B) }, /* tilde rising dots */
 {"\00\20\02\25\10\01\00\00\00", 0xf119, "uni2A6B.rtlm" },
 {"\37\00\37\00\10\25\02\00\00", U(2A73) }, /* equals tilde */
 {"\37\00\37\00\02\25\10\00\00", 0xf11a, "uni2A73.rtlm" },
 {"\12\00\37\00\37\00\12\00\00", U(2A77) }, /* equals with four dots */
 {"\04\33\00\37\00\37\00\00\00", U(2AAE) }, /* bumpy equals */
 {"\00\17\20\24\20\17\00\00\00", U(2ABD) }, /* subset with dot */
 {"\00\36\01\05\01\36\00\00\00", U(2ABE) }, /* superset with dot */
 {"\17\20\17\00\04\16\04\00\00", U(2ABF) }, /* subset plus */
 {"\36\01\36\00\04\16\04\00\00", U(2AC0) }, /* superset plus */
 {"\17\20\17\00\12\04\12\00\00", U(2AC1) }, /* subset times */
 {"\36\01\36\00\12\04\12\00\00", U(2AC2) }, /* superset times */
 {"\04\00\17\20\17\00\37\00\00", U(2AC3) }, /* dot subset or equal */
 {"\04\00\36\01\36\00\37\00\00", U(2AC4) }, /* dot superset or equal */
 {"\17\20\17\00\37\00\37\00\00", U(2AC5) }, /* subset equals */
 {"\36\01\36\00\37\00\37\00\00", U(2AC6) }, /* superset equals */
 {"\17\20\17\00\10\25\02\00\00", U(2AC7) }, /* subset tilde */
 {"\36\01\36\00\02\25\10\00\00", 0xf11b, "uni2AC7.rtlm" },
 {"\36\01\36\00\10\25\02\00\00", U(2AC8) }, /* superset tilde */
 {"\17\20\17\00\02\25\10\00\00", 0xf11c, "uni2AC8.rtlm" },
 {"\00\17\21\21\21\17\00\00\00", U(2ACF) }, /* closed subset */
 {"\00\36\21\21\21\36\00\00\00", U(2AD0) }, /* closed superset */
 {"\17\21\21\21\17\00\37\00\00", U(2AD1) }, /* closed subset or equal */
 {"\36\21\21\21\36\00\37\00\00", U(2AD2) }, /* closed superset or equal */
 {"\17\20\17\00\36\01\36\00\00", U(2AD3) }, /* subset superset */
 {"\36\01\36\00\17\20\17\00\00", U(2AD4) }, /* superset subset */
 {"\17\20\17\00\17\20\17\00\00", U(2AD5) }, /* subset subset */
 {"\36\01\36\00\36\01\36\00\00", U(2AD6) }, /* superset superset */
 {"\00\16\25\25\25\25\00\00\00", U(2AD9) }, /* downward element of */
 {"\16\04\16\25\25\25\25\00\00", U(2ADA) }, /* tee top pitchfork */
 {"\04\16\25\25\25\25\04\00\00", U(2ADB) }, /* transversal intersection */
 {"\00\02\02\16\02\02\00\00\00", U(2ADE) }, /* short left tack */
 {"\00\00\37\04\04\00\00\00\00", U(2ADF) }, /* short down tack */
 {"\00\00\04\04\37\00\00\00\00", U(2AE0) }, /* short up tack */
 {"\20\37\20\37\20\37\20\00\00", U(2AE2) }, /* triple right turnstile */
 {"\01\37\01\37\01\37\01\00\00", 0xf11d, "uni2AE2.rtlm" },
 {"\00\05\05\35\05\05\00\00\00", U(2AE3) }, /* double vert left turnstile */
 {"\00\01\37\01\37\01\00\00\00", U(2AE4) }, /* double left turnstile */
 {"\00\05\35\05\35\05\00\00\00", U(2AE5) }, /* double v dbl left turnstile */
 {"\00\24\24\37\24\24\00\00\00", U(2AE6) },
 {"\00\05\05\37\05\05\00\00\00", 0xf11e, "uni2AE6.rtlm" },
 {"\00\37\00\37\04\04\00\00\00", U(2AE7) }, /* short down tack overbar */
 {"\00\04\04\37\00\37\00\00\00", U(2AE8) }, /* short up tack underbar */
 {"\04\04\37\00\37\04\04\00\00", U(2AE9) }, /* short up down tacks */
 {"\00\37\12\12\12\12\00\00\00", U(2AEA) }, /* double down tack */
 {"\00\12\12\12\12\37\00\00\00", U(2AEB) }, /* double up tack */
 {"\00\37\01\37\01\01\00\00\00", U(2AEC) }, /* double not sign */
 {"\00\37\20\37\20\20\00\00\00", U(2AED) }, /* double reversed not */
 {"\12\12\12\37\12\12\12\00\00", U(2AF2) }, /* double vert bar horiz stroke */
 {"\25\25\25\25\25\25\25\00\00", U(2AF4) }, /* triple vert bar operator */
 {"\25\25\25\37\25\25\25\00\00", U(2AF5) }, /* triple vert bar horiz stroke */
 {"\25\25\25\25\25\25\25\25\25", U(2AFC) }, /* large triple vertical bar */
 {"\16\12\12\12\12\12\16\00\00", U(2AFE) }, /* white vertical bar */
 {"\16\12\12\12\12\12\12\12\16", U(2AFF) }, /* n-ary white vertical bar */

 /* Miscellaneous symbols and arrows */
 {"\00\25\00\21\00\25\00\00\00", U(2B1A) }, /* dottedsquare */

 /* Latin extended-C */
 {"\17\21\21\21\21\23\15\00\00", U(2C6D) }, /* Latin Alpha */
 ALIAS("uni2C6D.c2sc", "uni0251"),
 {"\21\21\37\21\21\12\04\00\00", U(2C6F) }, /* turned A */
 {"\00\00\21\21\37\21\16\00\00", 0xf1bf, "uni2C6F.c2sc" },
 {"\00\00\22\25\24\24\10\00\00", U(2C71) }, /* vhook */
 {"\00\00\37\01\17\01\37\00\00", U(2C7B) }, /* small cap turned E */
 {"\00\00\00\00\04\00\04\04\10", U(2C7C) }, /* jinferior */
 {"\16\21\20\16\01\21\16\04\03", U(2C7E) }, /* S with swash tail */
 ALIAS("uni2C7E.c2sc", "uni023F"),
 {"\37\01\02\04\10\20\30\04\03", U(2C7F) }, /* Z with swash tail */
 {"\00\00\37\01\16\20\30\04\03", 0xf1c3, "uni2C7F.c2sc" },

 /* Supplemental punctuation */
 {"\02\04\10\02\04\10\00\00\00", U(2E17) }, /* dbl oblique hyphen */
 {"\00\00\04\00\04\14\24\25\16", U(2E18) }, /* gnaborretni */
 {"\12\00\00\16\00\00\00\00\00", U(2E1A) }, /* hyphendieresis */
 {"\04\12\04\00\10\25\02\00\00", U(2E1B) }, /* tilde ring above */
 {"\04\00\10\25\02\00\00\00\00", U(2E1E) }, /* tilde dot above */
 {"\00\00\10\25\02\00\04\00\00", U(2E1F) }, /* tilde dot below */
 {"\17\10\10\10\00\00\00\00\00", U(2E22) }, /* top half bracketleft */
 {"\36\02\02\02\00\00\00\00\00", U(2E23) }, /* top half bracketright */
 {"\00\00\00\10\10\10\17\00\00", U(2E24) }, /* bot half bracketleft */
 {"\00\00\00\02\02\02\36\00\00", U(2E25) }, /* bot half bracketright */
 {"\11\22\22\22\22\22\11\00\00", U(2E28) }, /* left double paren */
 {"\22\11\11\11\11\11\22\00\00", U(2E29) }, /* right double paren */
 {"\00\00\21\00\00\00\04\00\00", U(2E2A) }, /* two dots over one */
 {"\00\00\04\00\00\00\21\00\00", U(2E2B) }, /* one dot over two */
 {"\00\00\21\00\00\00\21\00\00", U(2E2C) }, /* squared four dots */
 {"\00\00\04\00\25\00\04\00\00", U(2E2D) }, /* five dot mark */
 {"\16\21\10\04\04\00\04\00\00", U(2E2E) }, /* reversed question */
 {"\16\21\20\10\04\00\04\00\00", 0xf212, "uni2E2E.open" },
 {"\00\00\00\00\00\02\04\04\00", U(2E32) }, /* turned comma */
 {"\00\02\04\04\00\00\04\00\00", U(2E35) }, /* turned semicolon */
 {"\04\04\34\04\04\04\04\04\04", U(2E36) }, /* dagger w/left guard */
 {"\04\04\07\04\04\04\04\04\04", U(2E37) }, /* dagger w/right guard */
 {"\04\04\04\04\04\04\37\04\04", U(2E38) }, /* turned dagger */
 {"\16\21\20\16\21\16\00\00\00", U(2E39) }, /* top half section */
 {"\00\00\16\00\16\00\00\00\00", U(2E40) }, /* double hyphen */
 {"\00\00\00\00\00\04\04\02\00", U(2E41) }, /* reversed comma */
 {"\00\00\00\00\00\22\22\11\00", U(2E42) }, /* double low reversed-9 quote */
 {"\04\04\37\04\37\04\37\04\04", U(2E4B) }, /* triple dagger */
 {"\17\10\10\34\10\10\17\00\00", U(2E55) }, /* bracketleft stroke */
 ALIAS("uni2E55.rtlm", "uni2E56"),
 {"\36\02\02\07\02\02\36\00\00", U(2E56) }, /* bracketright stroke */
 ALIAS("uni2E56.rtlm", "uni2E55"),
 {"\17\10\34\10\34\10\17\00\00", U(2E57) }, /* bracketleft double stroke */
 ALIAS("uni2E57.rtlm", "uni2E58"),
 {"\36\02\07\02\07\02\36\00\00", U(2E58) }, /* bracketright double stroke */
 ALIAS("uni2E58.rtlm", "uni2E57"),
 {"\02\04\10\10\00\00\00\00\00", U(2E59) }, /* top half parenleft */
 ALIAS("uni2E59.rtlm", "uni2E5A"),
 {"\10\04\02\02\00\00\00\00\00", U(2E5A) }, /* top half parenright */
 ALIAS("uni2E5A.rtlm", "uni2E59"),
 {"\00\00\00\10\10\04\02\00\00", U(2E5B) }, /* bottom half parenleft */
 ALIAS("uni2E5B.rtlm", "uni2E5C"),
 {"\00\00\00\02\02\04\10\00\00", U(2E5C) }, /* bottom half parenright */
 ALIAS("uni2E5C.rtlm", "uni2E5B"),
 {"\00\00\02\04\10\00\00\00\00", U(2E5D) }, /* oblique hyphen */

 /* Latin extended-D */
 {"\21\21\21\37\21\21\21\01\16", U(A726) }, /* Heng */
 {"\00\00\21\21\37\21\21\01\16", 0xf1c4, "uniA726.c2sc" },
 {"\20\20\36\21\21\21\21\01\06", U(A727) }, /* heng */
 ALIAS("uniA727.sc", "uniA726.c2sc"),
 {"\00\00\37\20\36\20\20\00\00", U(A730) }, /* small cap F */
 {"\00\00\17\20\16\01\36\00\00", U(A731) }, /* small cap S */
 {"\37\01\01\01\01\01\01\00\00", U(A780) }, /* turned L */
 {"\00\00\37\01\01\01\01\00\00", 0xf186, "uniA780.c2sc" },
 {"\36\02\02\02\02\02\02\00\00", 0xf21a, "uniA780.narrow" },
 {"\00\00\36\02\02\02\02\00\00", 0xf21b, "uniA780.c2sc.narrow" },
 {"\16\04\04\04\04\04\06\00\00", U(A781) }, /* turned l */
 ALIAS("uniA781.sc", "uniA780.c2sc"),
 ALIAS("uniA781.sc.narrow", "uniA780.c2sc.narrow"),
 {"\21\21\21\37\01\01\01\00\00", U(A78D) }, /* cap turned h */
 {"\06\11\10\34\10\11\06\00\00", U(A792) }, /* C with bar */
 {"\00\00\06\11\34\11\06\00\00", 0xf1c5, "uniA792.c2sc" },
 {"\00\00\07\10\34\10\07\00\00", U(A793) }, /* c with bar */
 ALIAS("uniA793.sc", "uniA792.c2sc"),
 {"\11\31\11\17\11\11\11\00\00", U(A7AA) }, /* H with hook */
 {"\00\00\11\31\17\11\11\00\00", 0xf1c6, "uniA7AA.c2sc" },
 {"\16\21\01\06\01\21\16\00\00", U(A7AB) }, /* reversed Epsilon */
 {"\00\00\16\21\06\21\16\00\00", 0xf1c7, "uniA7AB.c2sc" },
 {"\04\04\14\25\16\04\07\00\00", U(A7AD) }, /* belted L */
 UALIAS(0xf1c8, "uniA7AD.c2sc", "u1DF04"),
 {"\37\25\04\04\04\25\37\00\00", U(A7AE) }, /* cap small-cap I */
 {"\00\00\37\25\04\25\37\00\00", 0xf1c9, "uniA7AE.c2sc" },
 {"\00\00\16\21\25\22\15\00\00", U(A7AF) }, /* small cap Q */
 {"\21\11\05\03\05\11\21\00\00", U(A7B0) }, /* turned K */
 UALIAS(0xf1ca, "uniA7B0.c2sc", "u1DF10"),
 {"\04\04\04\04\04\04\37\00\00", U(A7B1) }, /* turned T */
 {"\00\00\04\04\04\04\37\00\00", 0xf1cb, "uniA7B1.c2sc" },
 {"\01\01\01\11\25\25\16\04\00", U(A7B2) }, /* crossed-tail J */
 {"\00\00\01\01\11\25\16\04\00", 0xf1cc, "uniA7B2.c2sc" },
 {"\37\01\01\17\01\01\01\00\00", U(A7FB) }, /* reversed F */
 {"\17\21\21\17\01\01\01\00\00", U(A7FC) }, /* reversed P */
 {"\21\21\21\25\25\33\21\00\00", U(A7FD) }, /* inverted M */

 /* Private use area */
 /* U+EE00--U+EE7F: zvbi mosaic graphics */
#define A(f,t) { .alias_of = t, U(EE ## f), IS_ALIAS | COMPAT }
#define S(f,t) A(f, "u1CE" #t)
#define C(f,t) A(f, "u1FB" #t)
 /*    */ S(01,51),S(02,52),S(03,53),S(04,54),S(05,55),S(06,56),S(07,57),
 S(08,58),S(09,59),S(0A,5A),S(0B,5B),S(0C,5C),S(0D,5D),S(0E,5E),S(0F,5F),
 S(10,60),S(11,61),S(12,62),S(13,63),S(14,64),S(15,65),S(16,66),S(17,67),
 S(18,68),S(19,69),S(1A,6A),S(1B,6B),S(1C,6C),S(1D,6D),S(1E,6E),S(1F,6F),
 /*    */ C(21,00),C(22,01),C(23,02),C(24,03),C(25,04),C(26,05),C(27,06),
 C(28,07),C(29,08),C(2A,09),C(2B,0A),C(2C,0B),C(2D,0C),C(2E,0D),C(2F,0E),
 C(30,0F),C(31,10),C(32,11),C(33,12),C(34,13),         C(36,14),C(37,15),
 C(38,16),C(39,17),C(3A,18),C(3B,19),C(3C,1A),C(3D,1B),C(3E,1C),C(3F,1D),
 S(40,70),S(41,71),S(42,72),S(43,73),S(44,74),S(45,75),S(46,76),S(47,77),
 S(48,78),S(49,79),S(4A,7A),S(4B,7B),S(4C,7C),S(4D,7D),S(4E,7E),S(4F,7F),
 S(50,80),S(51,81),S(52,82),S(53,83),S(54,84),S(55,85),S(56,86),S(57,87),
 S(58,88),S(59,89),S(5A,8A),S(5B,8B),S(5C,8C),S(5D,8D),S(5E,8E),S(5F,8F),
 C(60,1E),C(61,1F),C(62,20),C(63,21),C(64,22),C(65,23),C(66,24),C(67,25),
 C(68,26),C(69,27),         C(6B,28),C(6C,29),C(6D,2A),C(6E,2B),C(6F,2C),
 C(70,2D),C(71,2E),C(72,2F),C(73,30),C(74,31),C(75,32),C(76,33),C(77,34),
 C(78,35),C(79,36),C(7A,37),C(7B,38),C(7C,39),C(7D,3A),C(7E,3B),
 A(00,"uni2003"),A(20,"uni2003"),A(35,"lfblock"),A(6A,"rtblock"),A(7F,"block"),
#undef A
#undef C
#undef S

 /*
  * Characters in the private use area are used for Bedstead glyphs
  * that would otherwise be unencoded so as to make them easier to
  * use. These encodings should be stable, but they do not use uniXXXX
  * glyph names. They appear scattered across the glyph list, and not
  * here.
  */

 /* Alphabetic presentation forms */
 {"\06\10\36\12\12\12\17\00\00", 0xfb01, "fi" },
 {"\06\12\12\36\12\12\17\00\00", 0xfb02, "fl" },

 /* Halfwidth and fullwidth forms */
 {"\04\04\04\04\04\04\04\04\04", U(FFE8) }, /* halfwidth U+2502 */
 {"\00\04\10\37\10\04\00\00\00", U(FFE9) }, /* halfwidth U+2190 */
 {"\00\04\16\25\04\04\00\00\00", U(FFEA) }, /* halfwidth U+2191 */
 {"\00\04\02\37\02\04\00\00\00", U(FFEB) }, /* halfwidth U+2192 */
 {"\00\04\04\25\16\04\00\00\00", U(FFEC) }, /* halfwidth U+2193 */
 {"\37\37\37\37\37\37\37\00\00", U(FFED) }, /* halfwidth U+25A0 */
 {"\00\16\21\21\21\16\00\00\00", U(FFEE) }, /* halfwidth U+25CB */

 /* Specials */
 {"\16\21\25\11\16\12\16\00\00", U(FFFD) }, /* replacement */

 /* Shavian */
 {"\30\04\04\04\04\04\04\00\00", U(10450) }, /* peep */
 {"\14\24\04\04\04\04\04\00\00", U(10451) }, /* tot */
 {"\04\04\04\14\20\20\14\00\00", U(10452) }, /* kick */
 {"\04\04\04\04\04\04\30\00\00", U(10453) }, /* fee */
 {"\14\02\01\17\21\21\16\00\00", U(10454) }, /* thigh */
 {"\17\20\16\01\01\01\36\00\00", U(10455) }, /* so */
 {"\01\02\04\10\20\20\16\00\00", U(10456) }, /* sure */
 {"\15\22\04\10\20\20\16\00\00", U(10457) }, /* church */
 {"\20\20\10\04\02\01\01\00\00", U(10458) }, /* yea */
 {"\04\12\21\12\04\12\21\00\00", U(10459) }, /* hung */
 {"\00\00\04\04\04\04\04\04\03", U(1045A) }, /* bib */
 {"\00\00\04\04\04\04\04\05\06", U(1045B) }, /* dead */
 {"\00\00\06\01\01\06\04\04\04", U(1045C) }, /* gag */
 {"\00\00\03\04\04\04\04\04\04", U(1045D) }, /* vow */
 {"\00\00\16\21\21\36\20\10\06", U(1045E) }, /* they */
 {"\00\00\36\01\01\01\16\20\17", U(1045F) }, /* zoo */
 {"\00\00\16\01\01\02\04\10\20", U(10460) }, /* measure */
 {"\00\00\16\01\01\02\04\11\26", U(10461) }, /* judge */
 {"\00\00\01\01\02\04\10\20\20", U(10462) }, /* woe */
 {"\00\00\21\12\04\12\21\12\04", U(10463) }, /* ha-ha */
 {"\00\00\17\20\20\20\17\00\00", U(10464) }, /* loll */
 {"\00\00\03\02\04\10\30\00\00", U(10465) }, /* mime */
 {"\00\00\04\04\04\04\04\00\00", U(10466) }, /* if */
 {"\00\00\04\04\04\04\03\00\00", U(10467) }, /* egg */
 {"\00\00\04\04\04\04\30\00\00", U(10468) }, /* ash */
 {"\00\00\03\04\04\04\04\00\00", U(10469) }, /* ado */
 {"\00\00\30\04\04\04\04\00\00", U(1046A) }, /* on */
 {"\00\00\21\21\12\12\04\00\00", U(1046B) }, /* wool */
 {"\00\00\04\10\34\04\02\00\00", U(1046C) }, /* out */
 {"\00\00\06\10\07\02\34\00\00", U(1046D) }, /* ah */
 {"\00\00\36\01\01\01\36\00\00", U(1046E) }, /* roar */
 {"\00\00\30\10\04\02\03\00\00", U(1046F) }, /* nun */
 {"\00\00\20\23\25\31\01\00\00", U(10470) }, /* eat */
 {"\00\00\37\20\10\04\03\00\00", U(10471) }, /* age */
 {"\00\00\37\01\02\04\30\00\00", U(10472) }, /* ice */
 {"\00\00\37\01\16\20\20\00\00", U(10473) }, /* up */
 {"\00\00\16\21\21\21\16\00\00", U(10474) }, /* oak */
 {"\00\00\04\12\12\21\21\00\00", U(10475) }, /* ooze */
 {"\00\00\30\07\02\04\10\00\00", U(10476) }, /* oil */
 {"\00\00\34\03\01\02\01\00\00", U(10477) }, /* awe */
 {"\00\00\16\22\21\11\32\00\00", U(10478) }, /* are */
 {"\00\00\13\22\21\11\16\00\00", U(10479) }, /* or */
 {"\00\00\16\21\11\21\22\00\00", U(1047A) }, /* air */
 {"\00\00\11\21\22\21\16\00\00", U(1047B) }, /* err */
 {"\00\00\06\11\21\21\26\00\00", U(1047C) }, /* array */
 {"\00\00\26\31\21\21\26\00\00", U(1047D) }, /* ear */
 {"\00\00\27\30\20\20\20\00\00", U(1047E) }, /* ian */
 {"\00\00\20\20\22\25\15\00\00", U(1047F) }, /* yew */

 /* Symbols for legacy computing supplement */
#define M(x) { {0x ## x}, U(1CC2 ## x), SEP4 }
 /* */ M(1), M(2), M(3), M(4), M(5), M(6), M(7),
 M(8), M(9), M(A), M(B), M(C), M(D), M(E), M(F),
#undef M
#define O(x,u) { {0x ## x}, U(1CD ## u), MOS8 }
 /*
  * Because of where we put the cell boundaries, some 8-cell mosaic
  * characters can be aliased to 6-cell ones.  To make this less
  * confusing (maybe), the A() macro has its arguments the opposite
  * way around from ALIAS().
  */
#define A(t,u) UALIAS(U(1CD ## u), "u1FB" #t)
 /*                               */ O(04,00),         O(06,01),O(07,02),
 O(08,03),O(09,04),         O(0B,05),O(0C,06),O(0D,07),O(0E,08),
 O(10,09),O(11,0A),O(12,0B),O(13,0C),         A(04,0D),A(05,0E),A(06,0F),
 O(18,10),O(19,11),O(1A,12),O(1B,13),O(1C,14),O(1D,15),O(1E,16),O(1F,17),
 O(20,18),O(21,19),O(22,1A),O(23,1B),O(24,1C),O(25,1D),O(26,1E),O(27,1F),
 /*    */ A(08,20),A(09,21),A(0A,22),O(2C,23),O(2D,24),O(2E,25),O(2F,26),
 O(30,27),O(31,28),O(32,29),O(33,2A),O(34,2B),O(35,2C),O(36,2D),O(37,2E),
 O(38,2F),O(39,30),O(3A,31),O(3B,32),A(0B,33),A(0C,34),A(0D,35),
 /*    */ A(10,36),A(11,37),A(12,38),O(44,39),O(45,3A),O(46,3B),O(47,3C),
 O(48,3D),O(49,3E),O(4A,3F),O(4B,40),O(4C,41),O(4D,42),O(4E,43),O(4F,44),
 /*    */ O(51,45),O(52,46),O(53,47),A(13,48),         A(14,49),A(15,4A),
 O(58,4B),O(59,4C),         O(5B,4D),O(5C,4E),O(5D,4F),O(5E,50),
 O(60,51),O(61,52),O(62,53),O(63,54),O(64,55),O(65,56),O(66,57),O(67,58),
 A(16,59),A(17,5A),A(18,5B),A(19,5C),O(6C,5D),O(6D,5E),O(6E,5F),O(6F,60),
 O(70,61),O(71,62),O(72,63),O(73,64),O(74,65),O(75,66),O(76,67),O(77,68),
 O(78,69),O(79,6A),O(7A,6B),O(7B,6C),A(1A,6D),A(1B,6E),A(1C,6F),A(1D,70),
 /*    */ A(1F,71),A(20,72),A(21,73),O(84,74),O(85,75),O(86,76),O(87,77),
 O(88,78),O(89,79),O(8A,7A),O(8B,7B),O(8C,7C),O(8D,7D),O(8E,7E),O(8F,7F),
 O(90,80),O(91,81),O(92,82),O(93,83),A(22,84),A(23,85),A(24,86),A(25,87),
 O(98,88),O(99,89),O(9A,8A),O(9B,8B),O(9C,8C),O(9D,8D),O(9E,8E),O(9F,8F),
 /*    */ O(A1,90),O(A2,91),O(A3,92),O(A4,93),         O(A6,94),O(A7,95),
 A(26,96),A(27,97),         A(28,98),O(AC,99),O(AD,9A),O(AE,9B),
 O(B0,9C),O(B1,9D),O(B2,9E),O(B3,9F),O(B4,A0),O(B5,A1),O(B6,A2),O(B7,A3),
 O(B8,A4),O(B9,A5),O(BA,A6),O(BB,A7),A(29,A8),A(2A,A9),A(2B,AA),A(2C,AB),
 /*    */ A(2E,AC),A(2F,AD),A(30,AE),O(C4,AF),O(C5,B0),O(C6,B1),O(C7,B2),
 O(C8,B3),O(C9,B4),O(CA,B5),O(CB,B6),O(CC,B7),O(CD,B8),O(CE,B9),O(CF,BA),
 O(D0,BB),O(D1,BC),O(D2,BD),O(D3,BE),A(31,BF),A(32,C0),A(33,C1),A(34,C2),
 O(D8,C3),O(D9,C4),O(DA,C5),O(DB,C6),O(DC,C7),O(DD,C8),O(DE,C9),O(DF,CA),
 O(E0,CB),O(E1,CC),O(E2,CD),O(E3,CE),O(E4,CF),O(E5,D0),O(E6,D1),O(E7,D2),
 A(35,D3),A(36,D4),A(37,D5),A(38,D6),O(EC,D7),O(ED,D8),O(EE,D9),O(EF,DA),
 /*    */ O(F1,DB),O(F2,DC),O(F3,DD),O(F4,DE),         O(F6,DF),O(F7,E0),
 O(F8,E1),O(F9,E2),         O(FB,E3),         A(3A,E4),A(3B,E5),
#undef O
#undef A
 UALIAS(U(1CE1A), "uni256D"),  /* HP large type upper left arc */
 UALIAS(U(1CE1B), "SF010000"), /* upper left corner */
 UALIAS(U(1CE1C), "uni2577"),  /* upper terminal (345)*/
 {"\00\00\00\00\04\06\05\04\04", U(1CE1D), JOIN }, /* upper left crotch */
 UALIAS(U(1CE1E), "uni2576"),  /* left arm */
 UALIAS(U(1CE1F), "SF100000"), /* crossbar */
 UALIAS(U(1CE20), "SF060000"), /* crossbar with lower stem */
 {"\00\00\00\00\00\00\21\12\04", U(1CE21), JOIN }, /* upper half vertex of M */
 {"\00\00\00\00\00\00\20\10\04", U(1CE22), JOIN }, /* diagonal lower left */
 {"\00\00\00\00\00\00\00\04\04", U(1CE23), JOIN }, /* short upper term (5) */
 UALIAS(U(1CE24), "uni256E"),  /* upper right arc */
 UALIAS(U(1CE25), "uni2574"),  /* right arm */
 {"\00\00\00\00\04\14\24\04\04", U(1CE26), JOIN }, /* upper right crotch */
 UALIAS(U(1CE27), "SF030000"), /* upper right corner */
 UALIAS(U(1CE28), "SF080000"), /* stem with right crossbar */
 UALIAS(U(1CE29), "SF110000"), /* stem (12345) */
 {"\04\02\01\00\00\00\01\02\04", U(1CE2A), JOIN }, /* diag upr+lwr right */
 {"\04\02\01\00\00\00\00\00\00", U(1CE2B), JOIN }, /* diagonal upper right */
 {"\00\00\00\00\00\00\01\02\04", U(1CE2C), JOIN }, /* diagonal lower right */
 {"\04\04\00\00\00\00\00\00\00", U(1CE2D), JOIN }, /* short lower term (1) */
 {"\04\04\04\02\01\02\04\04\04", U(1CE2E), JOIN }, /* top+btm left arc */
 {"\00\00\01\02\34\02\01\00\00", U(1CE2F), JOIN }, /* centre of K */
 {"\04\00\00\00\00\00\00\00\00", U(1CE30), JOIN }, /* lower half vertex of M */
 {"\00\00\00\00\00\00\00\00\04", U(1CE31), JOIN }, /* upper half vertex of W */
 {"\00\00\21\12\04\12\21\00\00", U(1CE32), JOIN }, /* centre of X */
 {"\00\00\21\12\04\04\04\04\04", U(1CE33), JOIN }, /* centre of Y */
 {"\00\00\01\02\37\10\20\00\00", U(1CE34), JOIN }, /* centre of Z w/crossbar */
 {"\00\00\01\02\04\04\04\04\04", U(1CE35), JOIN }, /* raised upper left arc */
 UALIAS(U(1CE36), "SF090000"), /* stem with left crossbar */
 {"\04\04\04\10\20\10\04\04\04", U(1CE37), JOIN }, /* top+btm right arc */
 {"\04\10\20\00\00\00\20\10\04", U(1CE38), JOIN }, /* diag upr+lwr left */
 {"\04\04\24\14\04\04\04\04\04", U(1CE39), JOIN }, /* stem with left joint */
 UALIAS(U(1CE3A), "SF050000"), /* stem with crossbar */
 {"\04\10\20\00\00\00\00\00\00", U(1CE3B), JOIN }, /* diagonal upper left */
 UALIAS(U(1CE3C), "uni2575"),  /* lower terminal (123) */
 UALIAS(U(1CE3D), "SF020000"), /* lower left corner */
 UALIAS(U(1CE3E), "uni2570"),  /* lower left arc */
 {"\04\04\05\06\04\00\00\00\00", U(1CE3F), JOIN }, /* lower left crotch */
 UALIAS(U(1CE40), "SF070000"), /* crossbar with upper stem */
 {"\00\00\21\12\04\00\00\00\00", U(1CE41), JOIN }, /* vertex of V */
 {"\04\12\21\00\00\00\00\00\00", U(1CE42), JOIN }, /* lower half vertex of W */
 UALIAS(U(1CE43), "uni256F"),  /* lower right arc */
 UALIAS(U(1CE44), "SF040000"), /* lower right corner */
 {"\04\04\24\10\26\00\00\00\00", U(1CE45), JOIN }, /* lwr rt arc with tail */
 {"\04\04\24\14\04\00\00\00\00", U(1CE46), JOIN }, /* lower right crotch */
 {"\00\00\00\00\00\04\04\04\04", U(1CE47), JOIN }, /* stem part (45) */
 {"\00\00\04\04\04\04\04\04\04", U(1CE48), JOIN }, /* stem part (2345) */
 {"\00\00\00\00\00\04\04\00\00", U(1CE49), JOIN }, /* stem part (4) */
 {"\00\00\00\00\04\04\04\00\00", U(1CE4A), JOIN }, /* stem part (34) */
 {"\00\00\04\04\04\04\04\00\00", U(1CE4B), JOIN }, /* stem part (234) */
 {"\04\04\04\04\04\04\04\00\00", U(1CE4C), JOIN }, /* stem part (1234) */
 {"\00\00\00\00\04\00\00\00\00", U(1CE4D), JOIN }, /* stem part (3) */
 {"\00\00\04\04\04\00\00\00\00", U(1CE4E), JOIN }, /* stem part (23) */
 {"\00\00\04\04\00\00\00\00\00", U(1CE4F), JOIN }, /* stem part (2) */
 {"\04\04\04\04\00\00\00\00\00", U(1CE50), JOIN }, /* stem part (12) */
 UALIAS(U(1CEA0), "u1FB1E"), /* right half lower quarter block (octant-8) */
 UALIAS(U(1CEA3), "u1FB0F"), /* left half lower quarter block (octant-7) */
 UALIAS(U(1CEA8), "u1FB00"), /* left half upper quarter block (octant-1) */
 UALIAS(U(1CEAB), "u1FB01"), /* right half upper quarter block (octant-2) */

 /* Musical symbols */
 {"\04\04\04\04\04\04\04\04\04", U(1D100) }, /* bar line */
 {"\12\12\12\12\12\12\12\12\12", U(1D101) }, /* double bar line */
 {"\13\13\13\13\13\13\13\13\13", U(1D102) }, /* final bar line */
 {"\32\32\32\32\32\32\32\32\32", U(1D103) }, /* reversed ditto */
 {"\04\04\00\04\04\04\00\04\04", U(1D104) }, /* dashed bar line */
 {"\00\00\04\04\04\04\04\00\00", U(1D105) }, /* short bar line */
 {"\24\24\24\25\24\25\24\24\24", U(1D106) }, /* start repeat */
 {"\05\05\05\25\05\25\05\05\05", U(1D107) }, /* end repeat */
 {"\00\00\00\04\00\04\00\00\00", U(1D108) }, /* repeat dots */
 {"\10\24\21\12\04\12\21\05\02", U(1D10B) }, /* segno */
 {"\16\21\25\00\00\00\00\00\00", U(1D110) }, /* fermata */
 {"\00\00\00\00\00\00\25\21\16", U(1D111) }, /* fermata below */
 {"\00\00\00\00\37\00\00\00\00", U(1D116), JOIN_H }, /* 1-line stave */
 {"\00\00\00\37\00\37\00\00\00", U(1D117), JOIN_H }, /* 2-line stave */
 {"\00\00\37\00\37\00\37\00\00", U(1D118), JOIN_H }, /* 3-line stave */
 {"\00\37\00\37\00\37\00\37\00", U(1D119), JOIN_H }, /* 4-line stave */
 {"\37\00\37\00\37\00\37\00\37", U(1D11A), JOIN_H }, /* 5-line stave */
 {"\04\12\12\12\14\26\25\16\04", U(1D11E) }, /* G clef */
 {"\25\25\25\26\24\26\25\25\25", U(1D121) }, /* C clef */
 {"\10\25\04\05\10\20\00\00\00", U(1D122) }, /* F clef */
 {"\00\33\33\04\33\33\00\00\00", U(1D12A) }, /* double sharp */
 {"\04\24\27\35\25\26\30\00\00", U(1D12B) }, /* double flat */
 {"\00\00\37\16\37\00\00\00\00", U(1D13A) }, /* breve rest */
 {"\00\00\37\16\00\00\00\00\00", U(1D13B) }, /* semibreve rest */
 {"\00\00\00\16\37\00\00\00\00", U(1D13C) }, /* minim rest */

 /* Mathematical alphanumeric symbols */
 {"\16\25\25\35\25\25\27\00\00", U(1D538) }, /* double-struck A */
 {"\36\25\25\26\25\25\36\00\00", U(1D539) }, /* double-struck B */
 {"\36\25\25\25\25\25\36\00\00", U(1D53B) }, /* double-struck D */
 {"\37\24\24\26\24\24\37\00\00", U(1D53C) }, /* double-struck E */
 {"\37\24\24\26\24\24\34\00\00", U(1D53D) }, /* double-struck F */
 {"\16\25\24\24\27\25\17\00\00", U(1D53E) }, /* double-struck G */
 {"\37\12\12\12\12\12\37\00\00", U(1D540) }, /* double-struck I */
 {"\07\05\05\05\05\25\16\00\00", U(1D541) }, /* double-struck J */
 {"\35\25\25\26\25\25\35\00\00", U(1D542) }, /* double-struck K */
 {"\34\24\24\24\24\24\37\00\00", U(1D543) }, /* double-struck L */
 {"\21\33\25\25\25\25\27\00\00", U(1D544) }, /* double-struck M */
 {"\16\25\25\25\25\25\16\00\00", U(1D546) }, /* double-struck O */
 {"\17\24\22\11\05\05\36\00\00", U(1D54A) }, /* double-struck S */
 {"\37\12\12\12\12\12\16\00\00", U(1D54B) }, /* double-struck T */
 {"\35\25\25\25\25\25\16\00\00", U(1D54C) }, /* double-struck U */
 {"\35\25\22\12\12\04\04\00\00", U(1D54D) }, /* double-struck V */
 {"\35\25\25\25\25\25\12\00\00", U(1D54E) }, /* double-struck W */
 {"\35\25\22\12\11\25\27\00\00", U(1D54F) }, /* double-struck X */
 {"\35\25\22\12\12\12\16\00\00", U(1D550) }, /* double-struck Y */
 {"\00\00\16\05\15\25\17\00\00", U(1D552) }, /* double-struck a */
 {"\34\24\26\25\25\25\36\00\00", U(1D553) }, /* double-struck b */
 {"\00\00\17\24\24\24\17\00\00", U(1D554) }, /* double-struck c */
 {"\07\05\15\25\25\25\17\00\00", U(1D555) }, /* double-struck d */
 {"\00\00\16\25\27\24\16\00\00", U(1D556) }, /* double-struck e */
 {"\07\12\12\33\12\12\16\00\00", U(1D557) }, /* double-struck f */
 {"\00\00\17\25\25\25\17\01\16", U(1D558) }, /* double-struck g */
 {"\34\24\26\25\25\25\35\00\00", U(1D559) }, /* double-struck h */
 {"\16\00\36\12\12\12\37\00\00", U(1D55A) }, /* double-struck i */
 {"\16\00\16\12\12\12\12\12\34", U(1D55B) }, /* double-struck j */
 {"\34\24\25\25\26\25\35\00\00", U(1D55C) }, /* double-struck k */
 {"\36\12\12\12\12\12\37\00\00", U(1D55D) }, /* double-struck l */
 {"\00\00\32\25\25\25\35\00\00", U(1D55E) }, /* double-struck m */
 {"\00\00\36\25\25\25\35\00\00", U(1D55F) }, /* double-struck n */
 {"\00\00\16\25\25\25\16\00\00", U(1D560) }, /* double-struck o */
 {"\00\00\36\25\25\25\26\24\34", U(1D561) }, /* double-struck p */
 {"\00\00\17\25\25\25\15\05\07", U(1D562) }, /* double-struck q */
 {"\00\00\35\26\24\24\34\00\00", U(1D563) }, /* double-struck r */
 {"\00\00\17\22\11\05\36\00\00", U(1D564) }, /* double-struck s */
 {"\16\12\33\12\12\12\07\00\00", U(1D565) }, /* double-struck t */
 {"\00\00\27\25\25\25\17\00\00", U(1D566) }, /* double-struck u */
 {"\00\00\35\25\22\12\04\00\00", U(1D567) }, /* double-struck v */
 {"\00\00\35\25\25\25\12\00\00", U(1D568) }, /* double-struck w */
 {"\00\00\35\22\12\11\27\00\00", U(1D569) }, /* double-struck x */
 {"\00\00\27\25\25\25\17\01\16", U(1D56A) }, /* double-struck y */
 {"\00\00\37\11\12\22\37\00\00", U(1D56B) }, /* double-struck z */

 /* Latin extended-G */
 {"\00\00\14\25\16\04\07\00\00", U(1DF04) }, /* small cap belted L */
 {"\00\00\21\11\07\11\21\00\00", U(1DF10) }, /* small cap turned K */

 /* Transport and map symbols */
 {"\04\12\12\12\33\33\25\00\00", U(1F680) }, /* rocket */
 {"\00\34\10\11\27\10\00\00\00", U(1F681) }, /* helicopter */
 {"\00\00\13\36\37\25\00\00\00", U(1F682) }, /* steamlocomotive */
 {"\00\07\17\27\37\11\00\00\00", U(1F69A) }, /* deliverytruck */

 /* Symbols for legacy computing */
 UALIAS(U(1FB82), "u1FB02"), /* upper quarter block (octant-12) */
 UALIAS(U(1FB85), "u1FB0E"), /* upper three-quarter block (octant-123456) */
 {"\30\24\22\21\27\30\00\00\00", U(1FBB0) }, /* arrowheadptr */
 {"\00\12\33\00\33\12\00\00\00", U(1FBBB) }, /* voided Greek cross */
 {"\00\37\01\05\01\37\00\00\00", U(1FBBC) }, /* right open square dot */
 {"\12\25\21\12\21\25\12\00\00", U(1FBC0) }, /* RISC OS close button */
 {"\37\21\35\31\37\33\37\00\00", U(1FBC4) }, /* invquestion */
 {"\04\12\12\04\37\04\04\12\21", U(1FBC5) }, /* stick figure */
 {"\04\12\12\04\25\16\04\12\21", U(1FBC6) }, /* ... with arms up */
 {"\04\12\12\04\15\26\04\12\11", U(1FBC7) }, /* ... leaning left */
 {"\04\12\12\04\26\15\04\12\22", U(1FBC8) }, /* ... leaning right */
 {"\04\12\12\04\37\12\21\37\12", U(1FBC9) }, /* ... wearing dress */
 {"\04\12\21\21\25\33\21\00\00", U(1FBCA) }, /* white chevron up */
 UALIAS(U(1FBE6), "u1FB03"), /* middle left quarter block (octant-35) */
 UALIAS(U(1FBE7), "u1FB07"), /* middle right quarter block (octant-46) */

 /*
  * Backward compatibility aliases.  These are glyphs whose name (and
  * sometimes Unicode mapping) has changed and where we want to keep
  * the old name working.  The first group were added in Bedstead 002.002.
  */
#define COMPAT_ALIAS(alias, canonical)			\
 {{.alias_of=canonical},NU,alias,IS_ALIAS|COMPAT}
 COMPAT_ALIAS("uni2126", "Omega"),
 COMPAT_ALIAS("uni2295", "circleplus"),
#define UA(u) COMPAT_ALIAS("uni" #u, "u" #u)
 UA(10450), UA(10451), UA(10452), UA(10453), UA(10454), UA(10455), UA(10456),
 UA(10457), UA(10458), UA(10459), UA(1045A), UA(1045B), UA(1045C), UA(1045D),
 UA(1045E), UA(1045F), UA(10460), UA(10461), UA(10462), UA(10463), UA(10464),
 UA(10465), UA(10466), UA(10467), UA(10468), UA(10469), UA(1046A), UA(1046B),
 UA(1046C), UA(1046D), UA(1046E), UA(1046F), UA(10470), UA(10471), UA(10472),
 UA(10473), UA(10474), UA(10475), UA(10476), UA(10477), UA(10478), UA(10479),
 UA(1047A), UA(1047B), UA(1047C), UA(1047D), UA(1047E), UA(1047F), UA(1F680),
 UA(1F681), UA(1F682), UA(1F69A), UA(1FBB0), UA(1FBBB), UA(1FBBC), UA(1FBC0),
 UA(1FBC4), UA(1FBC5), UA(1FBC6), UA(1FBC7), UA(1FBC8), UA(1FBC9), UA(1FBCA),
#undef UA
#define UA6(u)					\
 COMPAT_ALIAS("uni" #u, "u" #u),		\
 COMPAT_ALIAS("uni" #u ".sep6", "u" #u ".sep6")
 UA6(1FB00), UA6(1FB01), UA6(1FB02),
 UA6(1FB03), UA6(1FB04), UA6(1FB05), UA6(1FB06),
 UA6(1FB07), UA6(1FB08), UA6(1FB09), UA6(1FB0A),
 UA6(1FB0B), UA6(1FB0C), UA6(1FB0D), UA6(1FB0E),
 UA6(1FB0F), UA6(1FB10), UA6(1FB11), UA6(1FB12),
 UA6(1FB13), UA6(1FB14), UA6(1FB15),
 UA6(1FB16), UA6(1FB17), UA6(1FB18), UA6(1FB19),
 UA6(1FB1A), UA6(1FB1B), UA6(1FB1C), UA6(1FB1D),
 UA6(1FB1E), UA6(1FB1F), UA6(1FB20), UA6(1FB21),
 UA6(1FB22), UA6(1FB23), UA6(1FB24), UA6(1FB25),
 UA6(1FB26), UA6(1FB27), UA6(1FB28),
 UA6(1FB29), UA6(1FB2A), UA6(1FB2B), UA6(1FB2C),
 UA6(1FB2D), UA6(1FB2E), UA6(1FB2F), UA6(1FB30),
 UA6(1FB31), UA6(1FB32), UA6(1FB33), UA6(1FB34),
 UA6(1FB35), UA6(1FB36), UA6(1FB37), UA6(1FB38),
 UA6(1FB39), UA6(1FB3A), UA6(1FB3B),
#undef UA6
 
 /*
  * Further compatibility aliases, added in 002.007 when Unicode 16
  * gave proper code points for separated 6-cell mosaic graphics.
  */
#define A(f,t)								\
 { .alias_of = "u1CE" #t, NU, "u1FB" #f ".sep6", IS_ALIAS | COMPAT }
 /*    */ A(00,51),A(01,52),A(02,53),A(03,54),A(04,55),A(05,56),A(06,57),
 A(07,58),A(08,59),A(09,5A),A(0A,5B),A(0B,5C),A(0C,5D),A(0D,5E),A(0E,5F),
 A(0F,60),A(10,61),A(11,62),A(12,63),A(13,64),         A(14,66),A(15,67),
 A(16,68),A(17,69),A(18,6A),A(19,6B),A(1A,6C),A(1B,6D),A(1C,6E),A(1D,6F),
 A(1E,70),A(1F,71),A(20,72),A(21,73),A(22,74),A(23,75),A(24,76),A(25,77),
 A(26,78),A(27,79),         A(28,7B),A(29,7C),A(2A,7D),A(2B,7E),A(2C,7F),
 A(2D,80),A(2E,81),A(2F,82),A(30,83),A(31,84),A(32,85),A(33,86),A(34,87),
 A(35,88),A(36,89),A(37,8A),A(38,8B),A(39,8C),A(3A,8D),A(3B,8E),
#undef A
 COMPAT_ALIAS("lfblock.sep6", "u1CE65"),
 COMPAT_ALIAS("rtblock.sep6", "u1CE7A"),
 COMPAT_ALIAS("block.sep6", "u1CE8F"),

 /* Compatibility alias added in 3.246. */
 COMPAT_ALIAS("Wdieresis.sc", "Wdieresis"),

 /* Compatibility aliases for 4-cell separated graphics. */
#define M(x, n) \
 { .alias_of = "u1CC2" #x, 0xf1e ## x, n ".sep4", IS_ALIAS | COMPAT }
 /*            */ M(1, "uni2598"), M(2, "uni259D"), M(3, "upblock"),
 M(4, "uni2596"), M(5, "lfblock"), M(6, "uni259E"), M(7, "uni259B"),
 M(8, "uni2597"), M(9, "uni259A"), M(A, "rtblock"), M(B, "uni259C"),
 M(C, "dnblock"), M(D, "uni2599"), M(E, "uni259F"), M(F, "block"),
#undef M

 /* Added after 3.246. */
 COMPAT_ALIAS("comma.saa5051", "comma.left"),
 COMPAT_ALIAS("comma.saa5052", "comma.left"),
 COMPAT_ALIAS("period.saa5051", "period.large"),
 COMPAT_ALIAS("period.saa5052", "period.large"),
 COMPAT_ALIAS("colon.saa5051", "colon.leftsmall"),
 COMPAT_ALIAS("colon.saa5052", "colon.leftsmall"),
 COMPAT_ALIAS("semicolon.saa5051", "semicolon.left"),
 COMPAT_ALIAS("semicolon.saa5052", "semicolon.left"),
 COMPAT_ALIAS("question.saa5051", "question.open"),
 COMPAT_ALIAS("question.saa5052", "question.open"),
 COMPAT_ALIAS("D.saa5051", "D.serif"),
 COMPAT_ALIAS("D.saa5052", "D.narrow"),
 COMPAT_ALIAS("J.saa5051", "J.narrow"),
 COMPAT_ALIAS("J.saa5052", "J.narrow"),
 COMPAT_ALIAS("L.saa5051", "L.narrow"),
 COMPAT_ALIAS("L.saa5052", "L.narrow"),
 COMPAT_ALIAS("j.saa5051", "j.serif"),
 COMPAT_ALIAS("j.saa5052", "j.serif"),
 COMPAT_ALIAS("t.saa5051", "t.small"),
 COMPAT_ALIAS("t.saa5052", "t.small"),
 COMPAT_ALIAS("ocircumflex.saa5054", "ocircumflex.large"),
 COMPAT_ALIAS("ugrave.saa5054", "ugrave.roundjoined"),
 COMPAT_ALIAS("ccedilla.saa5054", "ccedilla.square"),

 /* and finally */
 {"\37\21\21\21\21\21\37\00\00", 0xf1ff, ".notdef" },
};

#undef U

static struct glyph const *glyphs_by_name[lenof(glyphs)];

/*
 * This array defines 'aalt' mappings that should not be automatically
 * generated from glyph names.  The OpenType specification says that
 * different fonts in the same family should order the results of the
 * 'aalt' lookup the same, which implies that applications might
 * reasonably record the index of an alternative.  To avoid breaking
 * such applications when Bedstead is upgraded, we want to ensure that
 * any such indexes in an old version of a font still work in newer
 * (minor) versions.  For most AlternateSets in the 'aalt' lookup,
 * this will happen naturally, but where it doesn't, this table
 * explicitly specifies the list of alternative glyphs so that they
 * match older versions.
 */
#define MAX_ALT_SUB_OVERRIDE 3
static struct alt_sub_override {
	char const *base;
	char const *alt[MAX_ALT_SUB_OVERRIDE];
} const aalt_overrides[] = {
	{ "D", { "D.c2sc", "D.serif", "D.narrow" } },
	{ "J", { "J.c2sc", "J.narrow", "J.narrow" } },
	{ "L", { "L.c2sc", "L.narrow", "L.narrow" } },
	{ "colon", { "colon.leftsmall", "colon.leftsmall" } },
	{ "ocircumflex", { "ocircumflex.large", "ocircumflex.sc" } },
	{ "question", { "question.open", "question.open" } },
	{ "semicolon", { "semicolon.left", "semicolon.left" } },
	{ "ugrave", { "ugrave.roundjoined", "ugrave.sc" } },
};

/*
 * Define all the Character Variant features using X macros.  These
 * are then used to create 'cvXX' features and grouped into 'ssXX'
 * features.
 *
 * Where possible, Character Variant features have numbers that are 32
 * less than the ASCII code of the base character.
 */
#define CV07(V) V("quotesingle", ".curly")
#define CV12(V) V("comma", ".left")
#define CV14(V) V("period", ".large")
#define CV26(V) V("colon", ".leftsmall")
#define CV27(V) V("semicolon", ".left")
#define CV31(V) V("question", ".open") V("questiondown", ".open")	\
	V("uni2E2E", ".open")
#define CV38(V) V("D", ".serif", ".narrow")				\
	V("D.c2sc", ".serif", ".narrow") V("d.sc", ".serif", ".narrow") \
	V("Eth", ".serif", "")						\
	V("Eth.c2sc", ".serif", "") V("eth.sc", ".serif", "")		\
	V("Dcaron", ".serif", ".narrow")				\
	V("dcaron.sc", ".serif", ".narrow") V("Dcroat", ".serif", "")	\
	V("Dcroat.c2sc", ".serif", "") V("dcroat.sc", ".serif", "")	\
	V("uni0189", ".serif", "") V("uni0189.c2sc", ".serif", "")	\
	V("uni0256.sc", ".serif", "")					\
	V("uni1D05", ".serif", ".narrow") V("uni1D06", ".serif", "")	\
	V("uni1E0A", ".serif", ".narrow")				\
	V("uni1E0B.sc", ".serif", ".narrow")				\
	V("uni1E0C", ".serif", ".narrow")				\
	V("uni1E0C.c2sc", ".serif", ".narrow")				\
	V("uni1E0D.sc", ".serif", ".narrow")				\
	V("uni1E0E", ".serif", ".narrow")				\
	V("uni1E0E.c2sc", ".serif", ".narrow")				\
	V("uni1E0F.sc", ".serif", ".narrow")				\
	V("uni1E10", ".serif", ".narrow")				\
	V("uni1E10.c2sc", ".serif", ".narrow")				\
	V("uni1E11.sc", ".serif", ".narrow")				\
	V("uni1E12", ".serif", ".narrow")				\
	V("uni1E12.c2sc", ".serif", ".narrow")				\
	V("uni1E13.sc", ".serif", ".narrow")
#define CV42(V) V("J", ".narrow") V("J.c2sc", ".narrow")	\
	V("j.sc", ".narrow") V("uni1D0A", ".narrow")
#define CV44(V) V("L", ".narrow") V("L.c2sc", ".narrow")	\
	V("l.sc", ".narrow") V("uni013B", ".narrow")		\
	V("uni013B.c2sc", ".narrow") V("uni013C.sc", ".narrow") \
	V("Lcaron", ".narrow")					\
	V("Lcaron.c2sc", ".narrow") V("lcaron.sc", ".narrow")	\
	V("Ldot", ".narrow") V("Ldot.c2sc", ".narrow")		\
	V("ldot.sc", ".narrow") V("uni029F", ".narrow")		\
	V("uniA780", ".narrow") V("uniA780.c2sc", ".narrow")	\
	V("uniA781.sc", ".narrow")
#define CV61(V) V("ugrave",".roundjoined")
#define CV64(V) V("grave", ".curly")
#define CV74(V) V("j", ".serif") V("ij", ".serif")		\
	V("jcircumflex", ".serif") V("uni01F0", ".serif")	\
	V("uni0237", ".serif") V("uni029D", ".serif")
#define CV79(V) V("ograve", ".large") V("oacute", ".large")	\
	V("ocircumflex", ".large") V("otilde", ".large")	\
	V("odieresis", ".large") V("omacron", ".large")
#define CV84(V) V("t", ".small") V("uni0163", ".small") \
	V("uni0163.square", ".small")			\
	V("tcaron", ".small") V("tbar", ".small")	\
	V("uni01AB", ".small") V("uni021B", ".small")	\
	V("uni0287", ".small")				\
	V("uni0288", ".small") V("uni02A6", ".small")	\
	V("uni02A7", ".small") V("uni1E6D", ".small")	\
	V("uni1E6F", ".small") V("uni1E71", ".small")	\
	V("uni1E97", ".small")
#define CV92(V) V("bar", ".broken")
#define CV96(V) V("cedilla", ".square")				\
	V("ccedilla", ".square") V("ccedilla.sc", ".square")		\
	V("Ccedilla", ".square") V("Ccedilla.c2sc", ".square")	\
	V("Scedilla", ".square") V("Scedilla.c2sc", ".square")	\
	V("scedilla", ".square")					\
	V("uni0162", ".square") V("uni0162.c2sc", ".square")		\
	V("uni0163", ".square") V("uni0163.small", ".square")		\
	V("uni0163.sc", ".square") V("uni0228", ".square")		\
	V("uni0228.c2sc", ".square") V("uni0229", ".square")		\
	V("uni0229.sc", ".square") V("uni1E08", ".square")		\
	V("uni1E09", ".square") V("uni1E09.sc", ".square")		\
	V("uni1E28", ".square") V("uni1E28.c2sc", ".square")		\
	V("uni1E29", ".square") V("uni1E29.sc", ".square") 

#define MAXSUBNAME 3
static struct gsub_feature {
	char const *tag;
#define SCRIPT_DFLT 0x01
#define SCRIPT_LATN 0x02
#define SCRIPT_ALL  0x03
	unsigned int scripts;
	char const *suffix; /* NULL for all alternative glyphs. */
	char const *xml; /* Individual character substitutions. */
	char const *name;
	char const *subnames[MAXSUBNAME];
	struct alt_sub_override const *overrides;
	int noverrides;
} const gsub_features[] = {
	{ "aalt", SCRIPT_ALL, .overrides = aalt_overrides,
	  .noverrides = lenof(aalt_overrides) },
	{ "smcp", SCRIPT_LATN, .suffix = ".sc" },
	{ "c2sc", SCRIPT_LATN, .suffix = ".c2sc" },
	{ "rtlm", SCRIPT_ALL, .suffix = ".rtlm" },
	{ "onum", SCRIPT_ALL, .suffix = ".onum" },
	{ "zero", SCRIPT_ALL, .suffix = ".zero" },
#define SUB(in, out) "<Substitution in='" in "' out='" out "'/>\n"
#define SUFFIXSUB1(base, suffix1, ...) SUB(base, base suffix1)
#define SUFFIXSUB2(base, suffix1, suffix2, ...) SUB(base, base suffix2)
#define SINGLESUB(x) "<SingleSubst>\n" x "</SingleSubst>\n"
	/*
	 * Each stylistic set is defined as a set of character variant
	 * feature values using the CVxx() macros defined above.
	 */
	{ "ss01", SCRIPT_ALL, .name = "SAA5051",
	  .xml = SINGLESUB(CV07(SUFFIXSUB1) CV12(SUFFIXSUB1)
			   CV14(SUFFIXSUB1) CV26(SUFFIXSUB1)
			   CV27(SUFFIXSUB1) CV31(SUFFIXSUB1)
			   CV38(SUFFIXSUB1) CV42(SUFFIXSUB1)
			   CV44(SUFFIXSUB1) CV74(SUFFIXSUB1)
			   CV84(SUFFIXSUB1)) },
	{ "ss02", SCRIPT_ALL, .name = "SAA5052",
	  .xml = SINGLESUB(CV07(SUFFIXSUB1) CV12(SUFFIXSUB1)
			   CV14(SUFFIXSUB1) CV26(SUFFIXSUB1)
			   CV27(SUFFIXSUB1) CV31(SUFFIXSUB1)
			   CV38(SUFFIXSUB2) CV42(SUFFIXSUB1)
			   CV44(SUFFIXSUB1) CV74(SUFFIXSUB1)
			   CV84(SUFFIXSUB1)) },
	{ "ss04", SCRIPT_ALL, .name = "SAA5054",
	  .xml = SINGLESUB(CV61(SUFFIXSUB1) CV79(SUFFIXSUB1)
			   CV96(SUFFIXSUB1)) },
	{ "ss05", SCRIPT_ALL, .name = "SAA5055",
	  .xml = SINGLESUB(CV07(SUFFIXSUB1) CV64(SUFFIXSUB1)
			   CV92(SUFFIXSUB1)) },
#define ALT1(g) "<Alternate glyph='" g "'/>"
#define ALT2(a, b) ALT1(a) ALT1(b)
#define ALT3(a, b, c) ALT1(a) ALT2(b, c)
#define ALTSET(g, a) "<AlternateSet glyph='" g "'>" a "</AlternateSet>\n"
#define ALTSUB(x) "<AlternateSubst>\n" x "</AlternateSubst>\n"
#define CROSSALT2(a, b) ALTSET(a, ALT2(a, b)) ALTSET(b, ALT2(a, b))
#define CROSSALT3(a, b, c) ALTSET(a, ALT3(a, b, c)) ALTSET(b, ALT3(a, b, c)) \
	ALTSET(c, ALT3(a, b, c))
#define CV2(base, a) CROSSALT2(base, base a)
#define CV3(base, a, b) CROSSALT3(base, base a, base b)
#define CV(...) GET_MACRO(__VA_ARGS__, CV3, CV2)(__VA_ARGS__)
	/*
	 * These construct the character-variant features out of the
	 * CVxx() macros above.
	 */
	{ "cv07", SCRIPT_ALL, .name = "apostrophe variants",
	  .subnames = { "straight", "curly" },
	  .xml = ALTSUB(CV07(CV)) },
	{ "cv12", SCRIPT_ALL, .name = "comma variants",
	  .subnames = { "centred", "left" },
	  .xml = ALTSUB(CV12(CV)) },
	{ "cv14", SCRIPT_ALL, .name = "full-stop variants",
	  .subnames = { "small", "large" },
	  .xml = ALTSUB(CV14(CV)) },
	{ "cv26", SCRIPT_ALL, .name = "colon variants",
	  .subnames = { "centred/large", "left/small" },
	  .xml = ALTSUB(CV26(CV)) },
	{ "cv27", SCRIPT_ALL, .name = "semicolon variants",
	  .subnames = { "centred", "left" },
	  .xml = ALTSUB(CV27(CV)) },
	{ "cv31", SCRIPT_ALL, .name = "question-mark variants",
	  .subnames = { "standard", "open" },
	  .xml = ALTSUB(CV31(CV)) },
	{ "cv38", SCRIPT_ALL, .name = "capital-D variants",
	  .subnames = { "standard", "with serifs", "narrow" },
	  .xml = ALTSUB(CV38(CV)) },
	{ "cv42", SCRIPT_ALL, .name = "capital-J variants",
	  .subnames = { "wide", "narrow" },
	  .xml = ALTSUB(CV42(CV)) },
	{ "cv44", SCRIPT_ALL, .name = "capital-L variants",
	  .subnames = { "wide", "narrow" },
	  .xml = ALTSUB(CV44(CV)) },
	{ "cv61", SCRIPT_ALL, .name = "small-u-grave variants",
	  .subnames = { "standard", "SAA5054" },
	  .xml = ALTSUB(CV61(CV)) },
	{ "cv64", SCRIPT_ALL, .name = "grave variants",
	  .subnames = { "diagonal", "curly" },
	  .xml = ALTSUB(CV64(CV)) },
	{ "cv74", SCRIPT_ALL, .name = "small-j variants",
	  .subnames = { "sans-serif", "with serif" },
	  .xml = ALTSUB(CV74(CV)) },
	{ "cv79", SCRIPT_ALL, .name = "accented small-o variants",
	  .subnames = { "small", "large" },
	  .xml = ALTSUB(CV79(CV)) },
	{ "cv84", SCRIPT_ALL, .name = "small-t variants",
	  .subnames = { "large", "small" },
	  .xml = ALTSUB(CV84(CV)) },
	{ "cv92", SCRIPT_ALL, .name = "vertical-bar variants",
	  .subnames = { "solid", "broken" },
	  .xml = ALTSUB(CV92(CV)) },
	{ "cv96", SCRIPT_ALL, .name = "cedilla variants",
	  .subnames = { "standard", "square" },
	  .xml = ALTSUB(CV96(CV)) },
	{ "ss14", SCRIPT_ALL,  ".sep4", .name = "4-cell separated graphics" },
	{ "ss16", SCRIPT_ALL,  ".sep6", .name = "6-cell separated graphics" },
};

static struct gsub_script {
	char const *tag;
	unsigned int flag;
} const gsub_scripts[] = {
	{ "DFLT", SCRIPT_DFLT },
	{ "latn", SCRIPT_LATN },
};

static void dochar(struct glyph *g);
static void dochar_plotter(struct glyph *g);
static void domosaic(struct glyph *g);
static void domosaic4(struct glyph *g);
static void domosaic8(struct glyph *g);
static void dopanose(void);
static void docmap(int pid, int eid, int format);
static void dogsub(void);
static void doaltsubs(int noverrides,
		      const struct alt_sub_override[noverrides]);
static void dosinglesubs(char const *suffix);
static void dogpos(void);
static void glyph_complement(void);
static void bdf_gen(int size);
static void doglyph(struct glyph *);

static bool
getpix(unsigned char const data[YSIZE - 1], int x, int y, unsigned int flags)
{

	/*
	 * Pixel co-ordinates count from (0,0) in the top left of the
	 * character cell.  In the glyph array, the top row isn't
	 * represented and the columns are right-aligned in a char.
	 * Like the top row, the left column of the character cell
	 * always reads as 0.
	 */
	/* Line-drawing characters repeat top row and/or left column. */
	if ((flags & JOIN_L) && x < 1) x = 1;
	if ((flags & JOIN_R) && x >= XSIZE) x = XSIZE - 1;
	if ((flags & JOIN_U) && y < 1) y = 1;
	if ((flags & JOIN_D) && y >= YSIZE) y = YSIZE - 1;
	if (x < 1 || x >= XSIZE || y < 1 || y >= YSIZE)
		return 0;
	else
		return (data[y - 1] >> (XSIZE - x - 1)) & 1;
}

static bool plottermode = false;

static char *
get_fullname(void)
{
#define FULLNAME_MAX 100
	static char fullname[FULLNAME_MAX];
	int len;

	len = snprintf(fullname, sizeof(fullname),
		       FAMILY_NAME "%s%s", weight->suffix, width->suffix);
	assert(len >= 0 && (unsigned int)len < sizeof(fullname));
	return fullname;
}

static char *
fullname_to_fontname(char const *fullname)
{
#define FONTNAME_MAX 29 /* Adobe-recommended limit */
	static char fontname[FONTNAME_MAX + 1];
	char *op = fontname;
	char const *p = fullname;
	bool gotfamily = false;

	while (*p != '\0') {
		assert(op - fontname <= FONTNAME_MAX);
		if (*p == ' ') {
			if (!gotfamily) {
				*op++ = '-';
				gotfamily = true;
			}
		} else {
			*op++ = *p;
		}
		++p;
	}
	*op++ = '\0';
	return fontname;
}

/* Get a suitable asctime-format time string for a TTX file. */
static char *
time_for_ttx(void)
{
	static char timestr[25];
	size_t timestrlen;
	struct tm *timeptr, tm;
	unsigned long long epochull;
	char *epochstr, *endptr;

	/* Work out what timestamp to use. */
	if ((epochstr = getenv("SOURCE_DATE_EPOCH")) != NULL) {
		epochull = strtoull(epochstr, &endptr, 10);
		if (!isdigit((unsigned char)epochstr[0]) ||
		    endptr == epochstr || *endptr != '\0' ||
		    /* Allow years up to 9999. */
		    epochull >= 253402300800) {
			fprintf(stderr, "Invalid SOURCE_DATE_EPOCH\n");
			return NULL;
		}
		tm.tm_isdst = -1;
		tm.tm_sec = epochull % 60;
		tm.tm_min = epochull / 60 % 60;
		tm.tm_hour = epochull / 3600 % 24;
		long day = epochull / 86400;
		tm.tm_wday = (day + 4) % 7; /* 1970-01-01 was a Thursday. */
		int y = 1970 + (day / 146097) * 400;
		day = day % 146097;
		bool ly;
		for (;;) {
			ly = y % 400 ? y % 100 ? y % 4 ? false:true:false:true;
			if (day < 365 + ly) break;
			day -= 365 + ly; y++;
		}
		tm.tm_year = y - 1900;
		tm.tm_yday = day;
		static int const mlc[] = {31,28,31,30,31,30,31,31,30,31,30,31};
		static int const mll[] = {31,29,31,30,31,30,31,31,30,31,30,31};
		tm.tm_mon = 0;
		for (;;) {
			int md = (ly ? mll : mlc)[tm.tm_mon];
			if (day < md) break;
			day -= md; tm.tm_mon++;
		}
		tm.tm_mday = day + 1;
		timeptr = &tm;
	} else {
		time_t now = time(NULL);
		if (now == (time_t)-1) {
			fprintf(stderr, "Can't get current time\n");
			return NULL;
		}
		timeptr = gmtime(&now);
		if (timeptr == NULL) {
			fprintf(stderr, "Can't convert time to UTC\n");
			return NULL;
		}
		if (timeptr->tm_year >= 8100) {
			fprintf(stderr, "Can't handle years past 9999\n");
			return NULL;
		}
	}
	timestrlen = strftime(timestr, lenof(timestr), "%c", timeptr);
	assert(timestrlen == 24);
	return timestr;
}

/*
 * Various parts of the code depend on the precise ordering of glyph
 * names produced and consumed by these functions.  ISO C doesn't make
 * many guarantees about the execution character set, so we put some
 * effort into getting the right ordering.  One useful thing that ISO
 * C does guarantee is that all characters in the basic execution
 * character set (which includes everything we use in glyph names) are
 * non-negative.
 *
 * The following table defines the sort order that we use.
 * Unspecified characters get mapped to zero, which we catch with an
 * assertion.
 */
unsigned char const glyphcharset[CHAR_MAX] = {
	['\0']=1,
	['.']=2,
	['0']=10,['1']=11,['2']=12,['3']=13,['4']=14,
	['5']=15,['6']=16,['7']=17,['8']=18,['9']=19,
	['A']=21,['B']=22,['C']=23,['D']=24,['E']=25,['F']=26,['G']=27,
	['H']=28,['I']=29,['J']=30,['K']=31,['L']=32,['M']=33,['N']=34,
	['O']=35,['P']=36,['Q']=37,['R']=38,['S']=39,['T']=40,['U']=41,
	['V']=42,['W']=43,['X']=44,['Y']=45,['Z']=46,
	['_']=47,
	['a']=51,['b']=52,['c']=53,['d']=54,['e']=55,['f']=56,['g']=57,
	['h']=58,['i']=59,['j']=60,['k']=61,['l']=62,['m']=63,['n']=64,
	['o']=65,['p']=66,['q']=67,['r']=68,['s']=69,['t']=70,['u']=71,
	['v']=72,['w']=73,['x']=74,['y']=75,['z']=76
};

static int namecmp(char const *ap, char const *bp)
{
	for (;; ap++, bp++) {
		unsigned int a = *ap, b = *bp;

		assert(a <= CHAR_MAX && b <= CHAR_MAX);
		assert(glyphcharset[a] != 0 && glyphcharset[b] != 0);
		if (glyphcharset[a] < glyphcharset[b]) return -1;
		if (glyphcharset[a] > glyphcharset[b]) return +1;
		assert(a == b);
		if (a == '\0') return 0;
	}
}

static int
compare_glyphs_by_name(const void *va, const void *vb)
{
	struct glyph const * const *ap = va, * const *bp = vb;
	struct glyph const *a = *ap, *b = *bp;

	return namecmp(a->name, b->name);
}

static int
compare_glyph_to_name(const void *vn, const void *vg)
{
	struct glyph const * const *gp = vg;
	struct glyph const *g = *gp;
	char const *name = vn;

	return namecmp(name, g->name);
}

static struct glyph *
get_glyph_by_name(char const *name)
{
	struct glyph * const *gp;

	gp = bsearch(name, glyphs_by_name, lenof(glyphs_by_name),
		     sizeof(glyphs_by_name[0]), &compare_glyph_to_name);
	assert(gp != NULL);
	return *gp;
}

static struct glyph *
realglyph(struct glyph *g)
{
	while (g->flags & IS_ALIAS) g = get_glyph_by_name(g->alias_of);
	return g;
}

/*
 * Compare glyphs in a way that will put them in an order that
 * FontForge wouldn't change.  We don't actually use FontForge any
 * more, but this sorting still makes the OTF file a few K shorter.
 */
static int
compare_glyphs_by_ffid(const void *va, const void *vb)
{
	struct glyph const *a = va, *b = vb;

	/* .notdef comes first. */
	if (strcmp(a->name, ".notdef") == 0) return -1;
	if (strcmp(b->name, ".notdef") == 0) return +1;
	/* Then characters with Unicode code-points in order. */
	if (a->unicode < b->unicode) return -1;
	if (a->unicode > b->unicode) return +1;
	/* Finally sort by glyph name for an arbitrary stable order. */
	return namecmp(a->name, b->name);
}

static int nsubrs;

int
main(int argc, char **argv)
{
	bool gen_bdf = false;
	int i;
	char *endptr, *timestr;

	if (argc == 2 && strcmp(argv[1], "--complement") == 0) {
		glyph_complement();
		return EXIT_SUCCESS;
	}

	while (argc > 1) {
		for (i = 0; i < lenof(widths); i++)
			if (strcmp(argv[1], widths[i].option) == 0) {
				width = &widths[i];
				argv++; argc--;
				goto next;
			}
		for (i = 0; i < lenof(weights); i++)
			if (strcmp(argv[1], weights[i].option) == 0) {
				weight = &weights[i];
				argv++; argc--;
				goto next;
			}
		if (strcmp(argv[1], "--plotter") == 0) {
			plottermode = true;
			argv++; argc--;
		} else if (strcmp(argv[1], "--bdfgen") == 0) {
			gen_bdf = true;
		} else if (strcmp(argv[1], "--") == 0) {
			argv++; argc--;
			break;
		} else if (argv[1][0] == '-') {
			fprintf(stderr, "unknown option '%s'\n", argv[1]);
			return EXIT_FAILURE;
		} else break;
		argv++; argc--;
	next:;
	}

	if (gen_bdf) {
		unsigned long u;

		if (argc != 2) {
			fprintf(stderr, "--bdfgen needs precisely one size\n");
			return EXIT_FAILURE;
		}
		u = strtoul(argv[1], &endptr, 10);
		if (*endptr != '\0') {
			fprintf(stderr, "--bdfgen needs a numeric size\n");
			return EXIT_FAILURE;
		}
		if (u > INT_MAX) {
			fprintf(stderr, "--bdfgen size can be at most %d\n",
				INT_MAX);
			return EXIT_FAILURE;
		}
		bdf_gen(u);
		return EXIT_SUCCESS;
	}

	if (argc > 1) {
		struct glyph g = { .flags = 0 };
		int y;
		unsigned long u;

		for (y = 0; y < YSIZE - 1; y++)
			g.data[y] = 0;

		y = 0;
		for (i = 1; i < argc; i++) {
			if (y >= YSIZE) {
				fprintf(stderr, "too many arguments\n");
				return EXIT_FAILURE;
			}
			u = strtoul(argv[i], &endptr, 0);
			if (u > 077 || !argv[i] || *endptr) {
				fprintf(stderr, "invalid argument \"%s\"\n",
					argv[i]);
				return EXIT_FAILURE;
			}
			g.data[y++] = u;
		}
		dochar(&g);
		return EXIT_SUCCESS;
	}

	/* Put glyphs into FontForge-compatible order. */
	qsort(glyphs, lenof(glyphs), sizeof(glyphs[0]),
	      &compare_glyphs_by_ffid);
	printf("<?xml version='1.0'?>\n");
	printf("<ttFont sfntVersion='OTTO'>\n");
/* Convenient macros for TTX values. */
#define TTXF(key, format, ...) \
    printf("<" key " value='" format "'/>\n", __VA_ARGS__)
#define TTXI(key, i) TTXF(key, "%ld", (long)(i))
#define TTXS(key, s) TTXF(key, "%s", (s))
	printf("<head>\n");
	TTXS("tableVersion", "1.0");
	TTXS("fontRevision", VERSION);
	TTXI("checkSumAdjustment", 0);
	TTXI("magicNumber", 0x5f0f3cf5);
	/*
	 * Flags:
	 *  Baseline and left sidebearing point at (0,0)
	 *  Force ppem to integer
	 */
	TTXS("flags", "00000000 00001011");
	TTXI("unitsPerEm", YSIZE * YPIX);
	if ((timestr = time_for_ttx()) == NULL) return 1;
	TTXS("created", timestr);
	TTXS("modified", timestr);
	TTXI("xMin", 0);
	TTXI("yMin", -DESCENT * YPIX);
	TTXI("xMax", XSIZE * XPIX);
	TTXI("yMax", ASCENT * YPIX);
	TTXF("macStyle", "00000000 0%c%c000%c",
	       width->ttfwidth > 5 ? '1' : '0', /* Expanded? */
	       width->ttfwidth < 5 ? '1' : '0', /* Condensed? */
	       weight->ttfweight > 500 ? '1' : '0'); /* Bold? */
	TTXI("lowestRecPPEM", YSIZE);
	TTXI("fontDirectionHint", 0); /* Fully bi-di. */
	TTXI("indexToLocFormat", 0);
	TTXI("glyphDataFormat", 0);
	printf("</head>\n");

	printf("<hhea>\n");
	TTXS("tableVersion", "0x00010000");
	TTXI("ascent", ASCENT * YPIX);
	TTXI("descent", -DESCENT * YPIX);
	TTXI("lineGap", 0);
	TTXI("advanceWidthMax", XSIZE * XPIX);
	TTXI("minLeftSideBearing", 0);
	TTXI("minRightSideBearing", 0);
	TTXI("xMaxExtent", XSIZE * XPIX);
	TTXI("caretSlopeRise", 1); TTXI("caretSlopeRun", 0);
	TTXI("caretOffset", 0);
	TTXI("reserved0", 0); TTXI("reserved1", 0);
	TTXI("reserved2", 0); TTXI("reserved3", 0);
	TTXI("metricDataFormat", 0);
	TTXI("numberOfHMetrics", 0); /* Will be calculated by TTX. */
	printf("</hhea>\n");

	printf("<maxp>\n");
	TTXS("tableVersion", "0x5000");
	TTXI("numGlyphs", lenof(glyphs));
	printf("</maxp>\n");

	printf("<OS_2>\n");
	TTXI("version", 4);
	TTXI("xAvgCharWidth", XSIZE * XPIX);
	TTXI("usWeightClass", weight->ttfweight);
	TTXI("usWidthClass", width->ttfwidth);
	TTXS("fsType", "00000000 00000000");
	/* Sub/Superscript are three by five pixels */
	TTXI("ySubscriptXSize", YSIZE * YPIX * 3 / (XSIZE - 1));
	TTXI("ySubscriptYSize", YSIZE * YPIX * 5 / (YSIZE - 3));
	TTXI("ySubscriptXOffset", 0);
	TTXI("ySubscriptYOffset", 2 * YPIX);
	TTXI("ySuperscriptXSize", YSIZE * YPIX * 3 / (XSIZE - 1));
	TTXI("ySuperscriptYSize", YSIZE * YPIX * 5 / (YSIZE - 3));
	TTXI("ySuperscriptXOffset", 0);
	TTXI("ySuperscriptYOffset", 2 * YPIX);
	TTXI("yStrikeoutSize", YPIX);
	TTXI("yStrikeoutPosition", 3 * YPIX);
	TTXI("sFamilyClass", 0x080a); /* Sans-serif / Matrix */
	dopanose();
	/*
	 * Unicode ranges that are considered "functional" in this
	 * font.
	 *
	 * The set bits correspond with:
	 * 0: Basic Latin
	 * 1: Latin-1 Supplement
	 * 2: Latin Extended-A
	 * 3: Latin Extended-B
	 * 4: IPA Extensions + Phonetic Extensions + Supplement
	 * 5: Spacing Modifier Letters
	 * 7: Greek and Coptic
	 * 9: Cyrillic + Supplement + Extended-A + -B
	 * 11: Hebrew
	 * 29: Latin Extended Additional + -C + -D
	 * 31: General Punctuation + Supplemental Punctuation
	 * 32: Superscripts and Subscripts
	 * 33: Currency Symbols
	 * 35: Letterlike Symbols
	 * 36: Number Forms
	 * 37: Arrows + Supplemental Arrows-A + -B + Misc Symbols and Arrows
	 * 38: Mathematical Operators + Supp + Misc -A + -B
	 * 39: Miscellaneous Technical
	 * 40: Control Pictures
	 * 43: Box Drawing
	 * 44: Block Elements
	 * 45: Geometric Shapes
	 * 46: Miscellaneous Symbols
	 * 57: Non-Plane 0 (any at all, need not be "functional")
	 * 60: Private Use Area (plane 0)
	 * 105: Shavian
	 */
	TTXS("ulUnicodeRange1", "10100000 00000000 00001010 10111111");
	TTXS("ulUnicodeRange2", "00010010 00000000 01111001 11111011");
	TTXS("ulUnicodeRange3", "00000000 00000000 00000000 00000000");
	TTXS("ulUnicodeRange4", "00000000 00000000 00000010 00000000");
	TTXS("achVendID", VENDOR_ID);
	TTXF("fsSelection", "00000001 1%c%c00000",
	       /* Fixed: use typo metrics; WWS */
	       weight->ttfweight == 500 && width->ttfwidth == 5 ? '1' : '0',
	       weight->ttfweight > 500 ? '1' : '0'); /* Bold? */
	TTXI("usFirstCharIndex", 32);
	TTXI("usLastCharIndex", 65535);
	TTXI("sTypoAscender", ASCENT * YPIX);
	TTXI("sTypoDescender", -DESCENT * YPIX);
	TTXI("sTypoLineGap", 0);
	TTXI("usWinAscent", ASCENT * YPIX);
	TTXI("usWinDescent", DESCENT * YPIX);
	/*
	 * Code pages that are considered "functional" in this font.
	 *
	 * The set bits correspond with:
	 * 0:  CP1252 Latin 1
	 * 2:  CP1251 Cyrillic
	 * 3:  CP1253 Greek
	 * 5:  CP1255 Hebrew
	 * 29: Macintosh Character Set (US Roman)
	 * 30: OEM Character Set
	 * 48: CP869 IBM Greek
	 * 49: CP866 MS-DOS Russian
	 * 50: CP865 MS-DOS Nordic
	 * 52: CP863 MS-DOS Canadian French
	 * 53: CP862 Hebrew
	 * 54: CP861 MS-DOS Icelandic
	 * 55: CP860 MS-DOS Portuguese
	 * 57: CP855 IBM Cyrillic; primarily Russian
	 * 60: CP737 Greek; former 437 G
	 * 62: CP850 WE/Latin 1
	 * 63: CP437 US
	 */
	TTXS("ulCodePageRange1", "01100000 00000000 00000000 00101101");
	TTXS("ulCodePageRange2", "11010010 11110111 00000000 00000000");
	TTXI("sxHeight", XHEIGHT * YPIX);
	TTXI("sCapHeight", CAPHEIGHT * YPIX);
	TTXI("usDefaultChar", 0);
	TTXI("usBreakChar", 32);
	TTXI("usMaxContext", 1); /* No pair subs. */
	printf("</OS_2>\n");

	printf("<name>\n");
	/* Encode each name in Mac Roman and Windows UTF-16. */
#define NAMEF(id, f, ...) \
	printf("<namerecord nameID='%d' " \
	       "platformID='1' platEncID='0' langID='0x0' unicode='True'>" \
	       f "</namerecord>\n" \
	       "<namerecord nameID='%d' " \
	       "platformID='3' platEncID='1' langID='0x409'>" \
	       f "</namerecord>\n", \
	       (int)(id), __VA_ARGS__, (int)(id), __VA_ARGS__)
#define NAME(id, s) NAMEF(id, "%s", s)
	NAME(8, "Ben Harris");
	NAME(11, "https://bjh21.me.uk/");
	NAME(14, "https://bjh21.me.uk/bedstead/#copyright");
	NAME(19, "BBC Computer 32K");
	if ((weight->ttfweight == 500 || weight->ttfweight == 700) &&
	    width->suffix[0] == '\0') {
		/* Normal width; regular or bold */
		NAME(1, FAMILY_NAME);
		NAME(2, weight->suffix[0] ? weight->suffix + 1 : "Regular");
	} else {
		/*
		 * Slightly unusual face, so we need to set the
		 * "Preferred" family and style.
		 */
		NAMEF(1, FAMILY_NAME "%s%s",
		      weight->ttfweight == 700 ? "" : weight->suffix,
		      width->suffix);
		NAME(2,
		     weight->ttfweight == 700 ? weight->suffix+1 : "Regular");
		NAME(16, FAMILY_NAME);
		NAMEF(17, "%s%s", weight->suffix[0] ? weight->suffix+1 : "",
		      weight->suffix[0] ? width->suffix :
		      width->suffix[0] ? width->suffix+1 : "");
	}
	NAMEF(3, "%s; %s; %s", get_fullname(), VERSION, timestr);
	NAME(4, get_fullname());
	NAME(5, "Version " VERSION);
	NAME(6, fullname_to_fontname(get_fullname()));
	/* Stylistic set names. */
#define NAMEBASE_GSUB 0x100
#define NAMEBASE_GSUB_SUB 0x200
	for (i = 0; i < lenof(gsub_features); i++) {
		if (gsub_features[i].name != NULL)
			NAME(NAMEBASE_GSUB + i, gsub_features[i].name);
		for (int j = 0; j < MAXSUBNAME; j++)
			if (gsub_features[i].subnames[j] != NULL)
				NAME(NAMEBASE_GSUB_SUB + MAXSUBNAME * i + j,
				     gsub_features[i].subnames[j]);
	}
	printf("</name>\n");

	printf("<post>\n");
	TTXS("formatType", "3.0");
	TTXS("italicAngle", "0.0");
	TTXI("underlinePosition", -YPIX);
	TTXI("underlineThickness", YPIX);
	TTXI("isFixedPitch", 1);
	TTXI("minMemType42", 0); TTXI("maxMemType42", 0);
	TTXI("minMemType1", 0); TTXI("maxMemType1", 0);
	printf("</post>\n");

	for (i = 0; i < lenof(glyphs); i++)
		glyphs_by_name[i] = glyphs + i;
	qsort(glyphs_by_name, lenof(glyphs_by_name), sizeof(glyphs_by_name[0]),
	      &compare_glyphs_by_name);

	printf("<GlyphOrder>\n");
	for (i = 0; i < lenof(glyphs); i++)
		printf("<GlyphID name='%s'/>\n", glyphs[i].name);
	printf("</GlyphOrder>\n");

	printf("<cmap><tableVersion version='0'/>\n");
	docmap(0, 3, 4);   /* Unicode 2.0+, BMP only */
	docmap(0, 4, 12);  /* Unicode 2.0+, full repertoire */
	docmap(3, 1, 4);   /* Windows Unicode, BMP only */
	docmap(3, 10, 12); /* Windows Unicode, full repertoire */
	/* Variation sequences. */
	printf("<cmap_format_14 platformID='0' platEncID='5'>\n");
	for (i = 0; i < lenof(glyphs); i++) {
		unsigned long u = glyphs[i].unicode;
		if (u != NU && (u & U_HASVS))
			printf("<map uv='0x%lx' uvs='0x%lx' name='%s'/>\n",
			       GET_UV(u), GET_UVS(u), glyphs[i].name);
	}
	printf("</cmap_format_14>\n");
	printf("</cmap>\n");

	dogpos(); /* Must be before 'CFF ' because it uses glyph bitmaps. */
	dogsub();

	printf("<CFF>\n");
	TTXI("major", 1); TTXI("minor", 0);
	printf("<CFFFont name='%s'>\n",
	       fullname_to_fontname(get_fullname()));
	TTXS("version", VERSION);
	TTXS("Notice", "Dedicated to the public domain");
	TTXS("FullName", get_fullname());
	TTXS("FamilyName", FAMILY_NAME);
	TTXS("Weight", *weight->suffix ? weight->suffix + 1 : "Medium");
	TTXI("isFixedPitch", 1);
	TTXI("UnderlinePosition", -3 * YPIX / 2);
	TTXI("UnderlineThickness", YPIX);
	TTXF("FontMatrix", "%g 0 0 %g 0 0",
	     1.0/(YSIZE * YPIX), 1.0/(YSIZE * YPIX));
	TTXF("FontBBox", "0 %d %d %d", (int)(-DESCENT * YPIX),
	     (int)(XSIZE * XPIX), (int)(ASCENT * YPIX));
	printf("<Private>\n");
	TTXF("BlueValues", "0 0 %4g %4g %4g %4g %4g %4g",
	     (double)(YPIX * XHEIGHT),    (double)(YPIX * XHEIGHT),
	     (double)(YPIX * ALEFHEIGHT), (double)(YPIX * ALEFHEIGHT),
	     (double)(YPIX * CAPHEIGHT),  (double)(YPIX * CAPHEIGHT));
	TTXF("OtherBlues", "%4g %4g",
	     (double)(YPIX * -DESCENT),   (double)(YPIX * -DESCENT));
	TTXI("BlueFuzz", 0);
	TTXF("StdHW", "%4g", (double)YPIX);
	TTXF("StdVW", "%4g", (double)(XPIX * (100 + weight->weight) / 100));
	if (weight->weight > 0)
		TTXI("ForceBold", 1);
	TTXI("defaultWidthX", XSIZE * XPIX);
	/* if (plottermode) { */
	/* 	printf("StrokedFont: 1\n"); */
	/* 	printf("StrokeWidth: 50\n"); */
	/* } */
	printf("<Subrs>\n");
	nsubrs = 0;
	for (i = 0; i < lenof(glyphs); i++) {
		struct glyph *g = &glyphs[i];
		if (g->flags & IS_ALIAS) {
			g = realglyph(g);
			if (g->flags & IS_SUBR) continue;
			printf("<CharString> <!-- %s -->", g->name);
			doglyph(g);
			g->flags = IS_SUBR;
			g->subr_idx = nsubrs++;
			printf("</CharString>\n");
		}
	}
	printf("</Subrs>\n");
	printf("</Private>\n");
	printf("<CharStrings>\n");
	for (i = 0; i < lenof(glyphs); i++) {
		printf("<CharString name='%s'>", glyphs[i].name);
		doglyph(&glyphs[i]);
		printf("</CharString>\n");
	}
	printf("</CharStrings>\n");
	printf("</CFFFont>\n");
	printf("</CFF>\n");
	printf("<hmtx>\n");
	for (i = 0; i < lenof(glyphs); i++)
		printf("<mtx name='%s' width='%d' lsb='%d'/>\n",
		       glyphs[i].name, (int)(XSIZE * XPIX),
		       (int)realglyph(&glyphs[i])->left_sidebearing);
	printf("</hmtx>\n");
	printf("</ttFont>\n");
	return EXIT_SUCCESS;
}

static void
dopanose(void)
{
	/*
	 * PANOSE is a complex font categorisation scheme.  It's not
	 * entirely unused, and is a mandatory part of the 'OS/2'
	 * table, so it would be nice to get it right.  This procedure
	 * calculates the PANOSE code for a Bedstead variant based on
	 * its design parameters.
	 *
	 * See <http://panose.com> for the full details.
	 *
	 * PANOSE has some rules that divert Latin Text fonts into the
	 * Latin Decorative category.  One applies if ConRat > 1.
	 * However, by definition ConRat is at most 1, so this never
	 * applies.  The other case is for very wide or narrow fonts,
	 * but this is checked after checking for monospaced fonts,
	 * and Bedstead is monospaced.  Thus all Bedstead variants can
	 * be treated as Latin Text fonts.
	 */
	int panose[10];
	int stdvw;
	assert(YPIX == 100); /* Make sums easier. */
	panose[0] = 2; /* Latin Text */
	/*
	 * WStem(I) = stdvw
	 * FootWid = 2 * XPIX + stdvw
	 * FootRat = 2 * XPIX / stdvw
	 * FootRat exceeds 1.6, but A, E, H and N are sans-serif
	 * So re-calculate FootRat based on H
	 * WStem(H) = stdvw
	 * FootWid(H) = stdvw
	 * FootRat(H) = 1
	 * So Bedstead is Sans Serif
	 * Flared excluded by FootRat
	 * StemCor = 0
	 * RonRat = 0
	 * Rounded excluded by RonRat
	 * FootPitch = 0 deg
	 * Perpendicular Sans Serif excluded by FootPitch
	 * EWid = 4 * XPIX + stdvw
	 * EOut = 4 * XPIX + stdvw
	 * SerOb = 1
	 * So Normal Sans Serif.
	 */
	panose[1] = 11; /* Normal Sans Serif */
	stdvw = XPIX * (100 + weight->weight) / 100;
	/* WeightRat == 700/StdVW */
	if (stdvw <= 20) /* WeightRat >= 35 */
		panose[2] = 2; /* Very Light */
	else if (stdvw < 39) /* WeightRat > 17.9 */
		panose[2] = 3; /* Light */
	else if (stdvw <= 70) /* WeightRat >= 10 */
		panose[2] = 4; /* Thin */
	else if (stdvw <= 94) /* WeightRat > 7.44 */
		panose[2] = 5; /* Book */
	else if (stdvw < 128) /* WeightRat > 5.5 */
		panose[2] = 6; /* Medium */
	else if (stdvw < 156) /* WeightRat > 4.5 */
		panose[2] = 7; /* Demi */
	else if (stdvw <= 200) /* WeightRat >= 3.5 */
		panose[2] = 8; /* Bold */
	else if (stdvw <= 280) /* WeightRat >= 2.5 */
		panose[2] = 9; /* Heavy */
	else if (stdvw <= 350) /* WeightRat >= 2.0 */
		panose[2] = 10; /* Black */
	else
		panose[2] = 11; /* Extra Black */
	/*
	 * To make life simpler, ignore diagonals when calculating
	 * ConRat.
	 */
	/* J and M are the same width. */
	panose[3] = 9; /* Monospaced */
	/* ConRat == min(StdVW / 100, 100 / StdVW) */
	if (stdvw > 80 && stdvw < 125)
		panose[4] = 2; /* None */
	else if (stdvw > 65 && stdvw < 154)
		panose[4] = 3; /* Very Low */
	else if (stdvw > 48 && stdvw < 209)
		panose[4] = 4; /* Low */
	else if (stdvw > 30 && stdvw < 334)
		panose[4] = 5; /* Medium Low */
	else if (stdvw > 20 && stdvw < 500)
		panose[4] = 6; /* Medium */
	else if (stdvw > 15 && stdvw < 667)
		panose[4] = 7; /* Medium High */
	else if (stdvw > 8 && stdvw < 1250)
		panose[4] = 8; /* High */
	else
		panose[4] = 9; /* Very High */
	/*
	 * Trying to work out Speed values by algebra made my head
	 * hurt, as did learning enough Maxima to get it to help.  So
	 * I sketched it in GeoGebra and played with the numbers.
	 * Speed varies with weight, but seems to be independent of
	 * XPIX.
	 */
	assert(XQTR_S == 29 && YQTR_S == 29); /* Only tested these values. */
	if (panose[4] == 2)
		panose[5] = 2; /* No Variation */
	else if (weight->weight < 12) { /* Speed < ~0.9602 */
		if (stdvw > YPIX)
			panose[5] = 7; /* Rapid/Vertical */
		else
			panose[5] = 8; /* Rapid/Horizontal */
	} else {
		if (stdvw > YPIX)
			panose[5] = 5; /* Gradual/Vertical */
		else
			panose[5] = 6; /* Gradual/Horizontal */
	}
	/* Unusually shaped 'A' means no fit here. */
	panose[6] = 1; /* No Fit */
	/*
	 * We declare that our O is not Off Center.
	 *
	 * Like Speed, OutCurv seems to be independent of XPIX.
	 */
	if (weight->weight <= -80)
		panose[7] = 3; /* Normal/Weighted */
	else if (weight->weight <= 90)
		panose[7] = 4; /* Normal/Boxed */
	else
		panose[7] = 5; /* Normal/Flattened */
	/*
	 * TrimRat is independent of XPIX.
	 * TrimRat = ((100-2*XQTR_S) + weight->weight) / (100 + weight->weight)
	 *         = (42 + weight->weight) / (100 + weight->weight)
	 * So between 0 and 0.613.
	 */
	if (weight->weight >= 45)
		panose[8] = 2; /* Standard/Trimmed */
	else
		panose[8] = 3; /* Standard/Pointed */
	/*
	 * DuckRat = XRat = 5/7
	 */
	panose[9] = 7; /* Ducking/Large */
	printf("<panose>\n"
	       "<bFamilyType value='%d'/><bSerifStyle value='%d'/>\n"
	       "<bWeight value='%d'/><bProportion value='%d'/>\n"
	       "<bContrast value='%d'/><bStrokeVariation value='%d'/>\n"
	       "<bArmStyle value='%d'/><bLetterForm value='%d'/>\n"
	       "<bMidline value='%d'/><bXHeight value='%d'/>\n"
	       "</panose>\n",
	       panose[0], panose[1], panose[2], panose[3], panose[4],
	       panose[5], panose[6], panose[7], panose[8], panose[9]);
}

static void
docmap(int pid, int eid, int format)
{
	int i;
	unsigned long limit = 0x10000;

	printf("<cmap_format_%d platformID='%d' platEncID='%d' language='0'",
	       format, pid, eid);
	if (format == 12) {
		limit = 0x110000;
		/* TTX will recalculate these, but insists we give them. */
		printf(" format='12' reserved='0' length='0' nGroups='0'");
	}
	printf(">\n");
	for (i = 0; i < lenof(glyphs); i++)
		if (glyphs[i].unicode < limit)
			printf("<map code='0x%lx' name='%s'/>\n",
			       (unsigned long)glyphs[i].unicode,
			       glyphs[i].name);
	printf("</cmap_format_%d>\n", format);
}

static int
compare_features_by_tag(const void *va, const void *vb)
{
	struct gsub_feature const * const *ap = va, * const *bp = vb;

	return namecmp((*ap)->tag, (*bp)->tag);
}

static void
dogsub(void)
{
	int i, j;
	struct gsub_feature const *sorted[lenof(gsub_features)];

	for (i = 0; i < lenof(gsub_features); i++)
		sorted[i] = &gsub_features[i];
	qsort(sorted, lenof(sorted), sizeof(sorted[0]),
	      &compare_features_by_tag);
	printf("<GSUB>\n");
	TTXS("Version", "0x00010000");
	printf("<ScriptList>\n");
	/*
	 * The FeatureList should be sorted alphabetically by tag, but
	 * the LookupList has to be in the order in which the lookups
	 * should be applied.  The FeatureIndices can be in any order
	 * we like.  This requires some care with the indexing.
	 */
	for (i = 0; i < lenof(gsub_scripts); i++) {
		printf("<ScriptRecord>\n");
		TTXS("ScriptTag", gsub_scripts[i].tag);
		printf("<Script><DefaultLangSys>\n");
		TTXI("ReqFeatureIndex", 0xffff); /* No required feature. */
		for (j = 0; j < lenof(sorted); j++)
			if (sorted[j]->scripts & gsub_scripts[i].flag) {
				printf("<!-- %s --> ", sorted[j]->tag);
				TTXI("FeatureIndex", j);
			}
		printf("</DefaultLangSys></Script>\n");
		printf("</ScriptRecord>\n");
	}
	printf("</ScriptList>\n");
	printf("<FeatureList>\n");
	for (i = 0; i < lenof(sorted); i++) {
		struct gsub_feature const *feat = sorted[i];
		int featidx = (feat - gsub_features);
		printf("<FeatureRecord>\n");
		TTXS("FeatureTag", feat->tag);
		printf("<Feature>\n");
		if (feat->name != NULL) {
			if (feat->tag[0] == 's') {
				printf("<FeatureParamsStylisticSet>\n");
				TTXI("Version", 0);
				TTXI("UINameID", NAMEBASE_GSUB + featidx);
				printf("</FeatureParamsStylisticSet>\n");
			} else {
				int nparam = 0;
				while (nparam < MAXSUBNAME &&
				       feat->subnames[nparam])
					nparam++;
				printf("<FeatureParamsCharacterVariants>\n");
				TTXI("Format", 0);
				TTXI("FeatUILabelNameID",
				     NAMEBASE_GSUB + featidx);
				TTXI("FeatUITooltipTextNameID", 0);
				TTXI("SampleTextNameID", 0);
				TTXI("NumNamedParameters", nparam);
				TTXI("FirstParamUILabelNameID",
				     NAMEBASE_GSUB_SUB + MAXSUBNAME * featidx);
				printf("</FeatureParamsCharacterVariants>\n");
			}
		}
		/* We only have one GSUB lookup per feature, thankfully. */
		TTXI("LookupListIndex", featidx);
		printf("</Feature>\n");
		printf("</FeatureRecord>\n");
	}
	printf("</FeatureList>\n");
	printf("<LookupList>\n");
	for (i = 0; i < lenof(gsub_features); i++) {
		printf("<Lookup> <!-- '%s' -->\n", gsub_features[i].tag);
		if (gsub_features[i].suffix != NULL) {
			/* Single lookup for all glyphs with a suffix. */
			TTXI("LookupFlag", 0);
			printf("<SingleSubst>\n");
			dosinglesubs(gsub_features[i].suffix);
			printf("</SingleSubst>\n");
		} else if (gsub_features[i].xml != NULL) {
			/* Raw XML for a substitution. */
			TTXI("LookupFlag", 0);
			printf("%s", gsub_features[i].xml);
		} else {
			/* All possible alternative glyphs. */
			TTXI("LookupFlag", 0);
			printf("<AlternateSubst>\n");
			doaltsubs(gsub_features[i].noverrides,
				  gsub_features[i].overrides);
			printf("</AlternateSubst>\n");
		}
		printf("</Lookup>\n");
	}
	printf("</LookupList>\n");
	printf("</GSUB>\n");

}

/* Find all the mappings from normal to alternative glyphs. */
static void
doaltsubs(int noverrides, const struct alt_sub_override overrides[noverrides])
{
	int i, j, next_override = 0;

	for (i = 1; i < lenof(glyphs_by_name); i++) {
#define HASDOT(x) (strchr(glyphs_by_name[x]->name, '.') != NULL)
		/*
		 * We want to map each glyph with a name that doesn't
		 * contain a '.' to all the glyphs whose names start
		 * with that name followed by a '.'.  By sorting them
		 * by name, we guarantee that each qualified glyph
		 * name comes immediately after the unqualified one.
		 */
		if (HASDOT(i)) {
			bool overridden = next_override < noverrides &&
				strcmp(glyphs_by_name[i-1]->name,
				       overrides[next_override].base) == 0;
			printf("%s<AlternateSet glyph='%s'>",
			       overridden ? "<!-- " : "",
			       glyphs_by_name[i-1]->name);
			for (; i < lenof(glyphs_by_name) && HASDOT(i); i++)
				printf("<Alternate glyph='%s'/>",
				       glyphs_by_name[i]->name);
			printf("</AlternateSet>%s\n",
			       overridden ? " -->" : "");
			next_override += overridden;
		}
	}

	printf("<!-- overrides -->\n");
	for (i = 0; i < noverrides; i++) {
		printf("<AlternateSet glyph='%s'>", overrides[i].base);
		for (j = 0; j < MAX_ALT_SUB_OVERRIDE &&
			     overrides[i].alt[j] != NULL; j++)
			printf("<Alternate glyph='%s'/>",
			       overrides[i].alt[j]);
		printf("</AlternateSet>\n");
	}	
}

/* Find all mappings for a single feature. */
static void
dosinglesubs(char const *suffix)
{
	int i;
	char *dot;

	for (i = 0; i < lenof(glyphs); i++)
		if ((dot = strchr(glyphs[i].name, '.')) != NULL &&
		    strcmp(dot, suffix) == 0)
			printf("<Substitution in='%.*s' out='%s'/>\n",
			       (int)(dot - glyphs[i].name),
			       glyphs[i].name, glyphs[i].name);
}

static int
glyph_footprint(unsigned char data[YSIZE - 1])
{
	int i;
	int footprint = 0;

	for (i = 0; i < YSIZE - 1; i++) footprint |= data[i];
	return footprint;
}

static void
dogpos(void)
{
	int dx, dh, i;

	/* We only support one 'GPOS' lookup, 'palt'. */
	printf("<GPOS>\n");
	TTXS("Version", "0x00010000");
	printf("<ScriptList>\n");
	printf("<ScriptRecord>\n");
	TTXS("ScriptTag", "DFLT");
	printf("<Script><DefaultLangSys>\n");
	TTXI("ReqFeatureIndex", 0xffff); /* No required feature. */
	TTXI("FeatureIndex", 0);
	printf("</DefaultLangSys></Script>\n");
	printf("</ScriptRecord>\n");
	printf("<ScriptRecord>\n");
	TTXS("ScriptTag", "latn");
	printf("<Script><DefaultLangSys>\n");
	TTXI("ReqFeatureIndex", 0xffff); /* No required feature. */
	TTXI("FeatureIndex", 0);
	printf("</DefaultLangSys></Script>\n");
	printf("</ScriptRecord>\n");
	printf("</ScriptList>\n");
	printf("<FeatureList>\n");
	printf("<FeatureRecord>\n");
	TTXS("FeatureTag", "palt");
	printf("<Feature>\n");
	TTXI("LookupListIndex", 0);
	printf("</Feature>\n");
	printf("</FeatureRecord>\n");
	printf("</FeatureList>\n");
	printf("<LookupList>\n");
	printf("<Lookup>\n");
	TTXI("LookupFlag", 0);
	/*
	 * We have only a few dx/dh combinations, so it makes sense to
	 * organise by that rather than spitting out a separate value
	 * for each glyph.
	 */
	for (dx = 0; dx < XSIZE - 1; dx++)
		for (dh = (dx == 0 ? 1 : dx); dh < XSIZE - 1; dh++) {
			printf("<SinglePos Format='1'>\n");
			printf("<!-- dx = %d; dh = %d -->\n", dx, dh);
			printf("<Coverage>\n");
			for (i = 0; i < lenof(glyphs); i++) {
				struct glyph *g = realglyph(&glyphs[i]);
				if (g->flags & (MOS6|MOS4)) continue;
				int fp = glyph_footprint(g->data);
				if (((fp & (0xff << (XSIZE-dx-1))) == 0 &&
				     (fp & (0xff << (XSIZE-dx-2))) != 0 &&
				     (fp & ((1 << (dh - dx)) - 1)) == 0 &&
				     (fp & ((1 << (dh - dx + 1)) - 1)) != 0) ||
				    (fp == 0 && dx == 0 && dh == 3))
					TTXS("Glyph", glyphs[i].name);
			}
			printf("</Coverage>\n");
			TTXI("ValueFormat", 5);
			printf("<Value XPlacement='%i' XAdvance='%i'/>\n",
			       (int)(-dx * XPIX), (int)(-dh * XPIX));
			printf("</SinglePos>\n");
		}
			
	printf("</Lookup>\n");
	printf("</LookupList>\n");
	printf("</GPOS>\n");
}

/* Emit a charstring for a glyph. */
static void
doglyph(struct glyph *g)
{

	g = realglyph(g);
	if (g->flags & IS_SUBR)
		printf(" %d callsubr",
		       g->subr_idx - (nsubrs < 1240 ? 107 : 1131));
	else if (g->flags & MOS6)
		domosaic(g);
	else if (g->flags & MOS4)
		domosaic4(g);
	else if (g->flags & MOS8)
		domosaic8(g);
	else if (plottermode)
		dochar_plotter(g);
	else
		dochar(g);
}

typedef struct vec {
	int x, y;
} vec;

typedef struct point {
	struct point *next, *prev;
	struct vec v;
} point;

#define MAXPOINTS (XSIZE * YSIZE * 16)

static point points[MAXPOINTS];

static int nextpoint;

static void
clearpath(void)
{

	nextpoint = 0;
}

static void
moveto(int x, int y)
{
	struct point *p = &points[nextpoint++];

	assert(nextpoint <= MAXPOINTS);
	p->v.x = x; p->v.y = y;
	p->next = p->prev = NULL;
}

static void
lineto(int x, int y)
{
	struct point *p = &points[nextpoint++];

	assert(nextpoint <= MAXPOINTS);
	p->v.x = x; p->v.y = y;
	p->next = NULL;
	p->prev = p - 1;
	p->prev->next = p;
}

static void
closepath(void)
{
	struct point *p = &points[nextpoint - 1];

	while (p->prev) p--;
	p->prev = points + nextpoint - 1;
	points[nextpoint - 1].next = p;
}

static void
killpoint(point *p)
{

	p->prev->next = p->next;
	p->next->prev = p->prev;
	p->next = p->prev = NULL;
}

static vec const zero = { 0, 0 };

static bool
vec_eqp(vec v1, vec v2)
{
	return v1.x == v2.x && v1.y == v2.y;
}

static vec
vec_sub(vec v1, vec v2)
{
	vec ret;
	ret.x = v1.x - v2.x; ret.y = v1.y - v2.y;
	return ret;
}

static int
gcd(int a, int b)
{
	int t;
	while (b != 0) {
		t = b;
		b = a % b;
		a = t;
	}
	return a;
}

/* Return the shortest representable vector parallel to the argument. */
static vec
vec_bearing(vec v)
{
	vec ret;
	int d = gcd(abs(v.x), abs(v.y));
	if (d != 0) {
		ret.x = v.x / d;
		ret.y = v.y / d;
	} else {
		ret.x = 0;
		ret.y = 0;
	}
	return ret;
}

/* If p is identical to its successor, remove p. */
static void
fix_identical(point *p)
{
	if (!p->next) return;
	if (vec_eqp(p->next->v, p->v))
		killpoint(p);
}

/* Are a, b, and c distinct collinear points in that order? */
static bool
vec_inline3(vec a, vec b, vec c)
{
	return
	    vec_eqp(vec_bearing(vec_sub(b, a)), vec_bearing(vec_sub(c, b))) &&
	    !vec_eqp(vec_bearing(vec_sub(b, a)), zero);
}

/* Are a, b, c, and d distinct collinear points in that order? */
static bool
vec_inline4(vec a, vec b, vec c, vec d)
{
	return vec_inline3(a, b, c) && vec_inline3(b, c, d);
}

/* If p is on the line between its predecessor and successor, remove p. */
static void
fix_collinear(point *p)
{
	if (!p->next) return;
	if (vec_inline3(p->prev->v, p->v, p->next->v))
		killpoint(p);
}

/* If p is the only point on its path, remove p. */
static void
fix_isolated(point *p)
{
	if (p->next == p)
		killpoint(p);
}

static bool done_anything;

static void
fix_edges(point *a0, point *b0)
{
	point *a1 = a0->next, *b1 = b0->next;

	assert(a1->prev == a0); assert(b1->prev == b0);
	assert(a0 != a1); assert(a0 != b0);
	assert(a1 != b1); assert(b0 != b1);
	if (vec_eqp(vec_bearing(vec_sub(a0->v, a1->v)),
		    vec_bearing(vec_sub(b1->v, b0->v))) &&
	    (vec_inline4(a0->v, b1->v, a1->v, b0->v) ||
	     vec_inline4(a0->v, b1->v, b0->v, a1->v) ||
	     vec_inline4(b1->v, a0->v, b0->v, a1->v) ||
	     vec_inline4(b1->v, a0->v, a1->v, b0->v) ||
	     vec_eqp(a0->v, b1->v) || vec_eqp(a1->v, b0->v))) {
		a0->next = b1; b1->prev = a0;
		b0->next = a1; a1->prev = b0;
		fix_isolated(a0);
		fix_identical(a0);
		fix_collinear(b1);
		fix_isolated(b0);
		fix_identical(b0);
		fix_collinear(a1);
		done_anything = true;
	}
}

static void
clean_path(void)
{
	int i, j;

	do {
		done_anything = false;
		for (i = 0; i < nextpoint; i++)
			for (j = i+1; points[i].next && j < nextpoint; j++)
				if (points[j].next)
					fix_edges(&points[i], &points[j]);
	} while (done_anything);
}

static void
emit_contour(point *p0, vec *cur)
{
	point *p = p0, *p1;
	int stacksize = -2; /* Allow for initial rmoveto. */

	if (p->prev) p->prev->next = NULL; /* Break the loop. */
	do {
		stacksize += 2;
		printf(" %g %g%s",
		       (double)(p->v.x - cur->x) / XSCALE,
		       (double)(p->v.y - cur->y) / YSCALE,
		       p == p0 ? " rmoveto" :
		       stacksize >= 48 ? " rlineto" : "");
		stacksize %= 48;
		*cur = p->v;
		p1 = p->next;
		p->prev = p->next = NULL;
		p = p1;
	} while (p);
	if (stacksize != 0) printf(" rlineto");
}

static void
emit_path(void)
{
	int i, pass;
	point *p;
	struct vec cur = { 0, DESCENT * YPIX };

	/*
	 * On the first pass, emit open contours (if there are any).
	 * On the second pass, emit all remaining contours.
	 */
	for (pass = 0; pass <= 1; pass++) {
		for (i = 0; i < nextpoint; i++) {
			p = &points[i];
			if (p->next && (!p->prev || pass == 1))
				emit_contour(p, &cur);
		}
	}
	printf(" endchar");
}

/* An "A" edge is top or left, a "Z" edge is bottom or right. */
enum hint_type {
	hint_none, hint_stem, hint_counter_stem, hint_aedge, hint_zedge
};

static void
select_hints(int nstems, int stems[nstems],
	     int aedges[nstems], int zedges[nstems],
	     enum hint_type hints[nstems])
{
	int most = 0;

	/* Find longest stems. */
	for (int i = 0; i < nstems; i++) {
		hints[i] = hint_none;
		if (stems[i] > most) most = stems[i];
	}
	for (;most > 0; most--) {
		/* Prefer long stems. */
		for (int i = 0; i < nstems; i++) {
			/* Prefer outer stems. */
			int ii = i % 2 ? nstems - 1 - i / 2 : i / 2;
			if (stems[ii] == most) {
				hints[ii] = hint_stem;
				stems[ii] = 0;
				/* Disallow overlapping stems. */
				if (ii >= 1) stems[ii-1] = 0;
				if (ii < nstems - 1) stems [ii+1] = 0;
			}
		}
	}
	/* Look for evenly spaced stems to attach counter masks to. */
	for (int step = 2; step < (nstems + 1) / 2; step++)
		for (int i = 0; i < nstems - 2 * step; i++)
			if (hints[i] && hints[i+step] && hints[i+step*2]) {
				for (int j = i; j < nstems; j += step)
					if (hints[j])
						hints[j] = hint_counter_stem;
					else break;
				goto counters_done;
			}
counters_done:
	/*
	 * Now look for edge hints, preferring ones closer to the edge
	 * of the character.
	 */
	for (int i = 0; i < nstems - 1; i++)
		if (aedges[i] > 0 && hints[i] == hint_none &&
		    (i == 0 || hints[i-1] == hint_none))
			hints[i] = hint_aedge;
	for (int i = nstems - 1; i >= 1; i--)
		if (zedges[i] > 0 && hints[i] == hint_none &&
		    (i == nstems - 1 || hints[i+1] == hint_none))
			hints[i] = hint_zedge;
	printf("<!-- HINTS ");
	for (int i = 0; i < nstems; i++) printf("%c", ".|I[]"[hints[i]]);
	printf(" -->");
}

/*
 * The purpose of this function is to adjust a few hints to improve
 * FreeType's rendering of certain characters at awkward sizes.  The
 * letters 'B' and 'e' at 11 pixels per em under FreeType 2.12.1 are a
 * good test case.
 *
 * It appears that when rendering these characters, FreeType fixes the
 * top and bottom strokes to the top and bottom alignment zones.  If
 * those are an even number of pixels apart, this means that the
 * middle stroke falls precisely on a pixel boundary.  FreeType then
 * has three choices: move the stem up to the pixel row above the
 * centre, move it down to the row below, or anti-alias it across the
 * two rows.  FreeType chooses to move the stem down, which makes the
 * characters look top-heavy.  We'd much prefer FreeType to move the
 * stem upwards.
 *
 * It turns out that this can be achieved just by making the middle
 * stem hint slightly closer to the stem above that to the stem below.
 * This just requires adjusting the width of the stem hint by one
 * design unit.  As far as I can see, this is allowed by the Type 1
 * font specification.  A stem hint must encompass the stem outline,
 * but it's allowed to be larger than the stem.  And it practice it
 * seems to work fine.
 *
 * To avoid surprising effects on other characters, this tweak is
 * applied only to two rows and only if the character looks like it
 * might benefit.  In particular, it's not applied to any stem hint
 * that might be captured by an alignment zone.
 */
static bool
middle_stem_tweak_wanted(enum hint_type hhints[YSIZE], int row)
{
	if (row == 4 &&
	    hhints[1] == hint_counter_stem &&
	    hhints[7] == hint_counter_stem) return true;
	if (row == 5 &&
	    hhints[3] == hint_counter_stem &&
	    hhints[7] == hint_counter_stem) return true;
	return false;
}

static void
emit_hints(int vstems[XSIZE], int ledges[XSIZE], int redges[XSIZE],
	   int hstems[YSIZE], int tedges[YSIZE], int bedges[YSIZE])
{
	int i;
	double start, size, cur;
	enum hint_type vhints[XSIZE], hhints[YSIZE];
	bool printed, need_cntrmask = false;

	/*
	 * We don't emit any counter hints for horizontal stems.  On
	 * Microsoft Windows 11, such counter hints are treated as
	 * more important than alignment zones, so when the cap height
	 * is an even number of pixels you end up with 'B' being one
	 * pixel smaller than 'C' to ensure that the two counters are
	 * the same size.  This is not actually what we want.
	 * FreeType 2.12.1 appears to entirely ignore those counter
	 * hints, so discarding them has no effect there.
	 */
	select_hints(YSIZE, hstems, tedges, bedges, hhints);
	cur = DESCENT * YPIX; printed = false;
	for (i = 0; i < YSIZE; i++) {
		switch (hhints[YSIZE - 1 - i]) {
		case hint_counter_stem:
			if (middle_stem_tweak_wanted(hhints, YSIZE - 1 - i)) {
				start = i * YPIX;
				size = YPIX + 1;
				break;
			} /* FALLTHROUGH */
		case hint_stem:
			start = i * YPIX;
			size = YPIX;
			break;
		case hint_aedge: /* Top edge. */
			start = (i + 1) * YPIX;
			size = -20;
			break;
		case hint_zedge: /* Bottom edge. */
			start = i * YPIX + 21;
			size = -21;
			break;
		default: continue;
		}
		printf(" %g %g", start - cur, size);
		cur = start + size;
		printed = true;
	}
	if (printed) printf(" hstem");

	select_hints(XSIZE, vstems, ledges, redges, vhints);
	cur = 0; printed = false;
	size = XPIX_S + weight->weight;
	for (i = 0; i < XSIZE; i++) {
		switch (vhints[i]) {
		case hint_counter_stem:
			need_cntrmask = true; /* FALLTHROUGH */
		case hint_stem:
			start = i * XPIX - weight->weight / XSCALE;
			size = XPIX + weight->weight / XSCALE;
			break;
		case hint_aedge: /* Left edge. */
			start = i * XPIX + 21;
			size = -21;
			break;
		case hint_zedge: /* Right edge. */
			start = (i + 1) * XPIX;
			size = -20;
			break;
		default: continue;
		}
		printf(" %g %g", start - cur, size);
		cur = start + size;
		printed = true;
	}
	if (printed) printf(" vstem");

	if (need_cntrmask) {
		int nhints = 0;
		printf(" cntrmask ");
		for (i = 0; i < YSIZE; i++)
			if (hhints[i]) {
				printf("0");
				nhints++;
			}
		for (i = 0; i < XSIZE; i++)
			if (vhints[i]) {
				printf("%c", "X0100"[vhints[i]]);
				nhints++;
			}
		printf("%s", &"0000000"[(nhints - 1) % 8]);
	}
}

/*
 * To vary the weight of Bedstead, we just vary the thickness of
 * vertical strokes.  More precisely, we pretend that the weight
 * variation is achieved by moving the rising edges of the output
 * waveform of the SAA5050.  This is implemented by moving all left
 * edges left and right.  The code below is not fully general: it can
 * handle cases where the slack in horizontal lines doesn't run out,
 * and one special case where it does.  This means it should work
 * correctly for the range (2 * XQTR_S - 1) < weight < (2 * XQTR_S).
 */
static void
adjust_weight(void)
{
	int i, X, Y, PX, PY, NX, NY, W = weight->weight;
	point *p;
	struct vec moves[MAXPOINTS];

	for (i = 0; i < nextpoint; i++) {
		p = &points[i];
		if (p->next == NULL) continue;
		assert(p->prev != NULL);
		moves[i].x = moves[i].y = 0;
		X = p->v.x; Y = p->v.y;
		PX = p->prev->v.x; PY = p->prev->v.y;
		NX = p->next->v.x; NY = p->next->v.y;
		/* Move left-edge points horizontally */
		if (NY <= Y && Y <= PY) {
			moves[i].x -= W;
			/*
			 * These two clauses deal in an ad-hoc way
			 * with the special cases (all four seen on
			 * the "asterisk" glyph) where the bevel of an
			 * inside corner gets completely consumed by
			 * boldening.
			 */
			if (W > XQTR_S && NY == Y && NX == X - XQTR_S) {
				moves[i].y += W - XQTR_S;
				if (PX != X) moves[i].x += W - XQTR_S;
				killpoint(p->next);
			}
			if (W > XQTR_S && PY == Y && PX == X - XQTR_S) {
				moves[i].y -= W - XQTR_S;
				if (NX != X) moves[i].x += W - XQTR_S;
				killpoint(p->prev);
			}
		}
		/* Move top inner corner points along NE/SW diagonal */
		if (NY < Y && Y > PY && NX > X && X > PX) {
			moves[i].x -= W/2;
			moves[i].y -= W/2;
		}
		/* Move bottom inner corner points along NW/SE diagonal */
		if (NY > Y && Y < PY && NX < X && X < PX) {
			moves[i].x -= W/2;
			moves[i].y += W/2;
		}
	}
	for (i = 0; i < nextpoint; i++) {
		p = &points[i];
		if (p->next == NULL) continue;
		assert(p->prev != NULL);
		p->v.x += moves[i].x;
		p->v.y += moves[i].y;
		/* Make sure line-drawing characters don't overflow. */
		if (p->v.x < 0) p->v.x = 0;
	}
}

static void
blackpixel(int x, int y, bool bl, bool br, bool tr, bool tl)
{
	x *= XPIX_S; y *= YPIX_S;

	if (bl)	moveto(x, y);
	else { moveto(x, y+YQTR_S); lineto(x+XQTR_S, y); }
	if (br) lineto(x+XPIX_S, y);
	else { lineto(x+XPIX_S-XQTR_S, y); lineto(x+XPIX_S, y+YQTR_S); }
	if (tr) lineto(x+XPIX_S, y+YPIX_S);
	else { lineto(x+XPIX_S, y+YPIX_S-YQTR_S);
	       lineto(x+XPIX_S-XQTR_S, y+YPIX_S); }
	if (tl) lineto(x, y+YPIX_S);
	else { lineto(x+XQTR_S, y+YPIX_S); lineto(x, y+YPIX_S-YQTR_S); }
	closepath();
}

static void
whitepixel(int x, int y, bool bl, bool br, bool tr, bool tl)
{
	x *= XPIX_S; y *= YPIX_S;

	if (bl) {
		moveto(x, y);
		if (br) { lineto(x+XQTR_S, y);
			  lineto(x+XPIX_S/2, y+YPIX_S/2-YQTR_S); }
		else lineto(x+XPIX_S-XQTR_S, y);
		lineto(x, y+YPIX_S-YQTR_S); closepath();
	}
	if (br) {
		moveto(x+XPIX_S, y);
		if (tr) { lineto(x+XPIX_S, y+YQTR_S);
			  lineto(x+XPIX_S/2+XQTR_S, y+YPIX_S/2); }
		else lineto(x+XPIX_S, y+YPIX_S-YQTR_S);
		lineto(x+XQTR_S, y); closepath();
	}
	if (tr) {
		moveto(x+XPIX_S, y+YPIX_S);
		if (tl) { lineto(x+XPIX_S-XQTR_S, y+YPIX_S);
			  lineto(x+XPIX_S/2, y+YPIX_S/2+YQTR_S); }
		else lineto(x+XQTR_S, y+YPIX_S);
		lineto(x+XPIX_S, y+YQTR_S); closepath();
	}
	if (tl) {
		moveto(x, y+YPIX_S);
		if (bl) { lineto(x, y+YPIX_S-YQTR_S);
			  lineto(x+XPIX_S/2-XQTR_S, y+YPIX_S/2); }
		else lineto(x, y+YQTR_S);
		lineto(x+XPIX_S-XQTR_S, y+YPIX_S); closepath();
	}
}

static void
record_lsb(struct glyph *g)
{
	int i;
	int lsb = XPIX_S * XSIZE;

	for (i = 0; i < nextpoint; i++) {
		if (points[i].next == NULL) continue;
		if (points[i].v.x < lsb) lsb = points[i].v.x;
	}
	/* A glyph with no contours has an lsb of zero. */
	if (lsb < XPIX_S * XSIZE)
		g->left_sidebearing = lsb / XSCALE;
}

static void
dochar(struct glyph *g)
{
	int x, y;
	int vstems[XSIZE], ledges[XSIZE], redges[XSIZE];
	int hstems[YSIZE], tedges[YSIZE], bedges[YSIZE];

#define GETPIX(x,y) (getpix(g->data, (x), (y), g->flags))
#define L GETPIX(x-1, y)
#define R GETPIX(x+1, y)
#define U GETPIX(x, y-1)
#define D GETPIX(x, y+1)
#define UL GETPIX(x-1, y-1)
#define UR GETPIX(x+1, y-1)
#define DL GETPIX(x-1, y+1)
#define DR GETPIX(x+1, y+1)

	for (x = 0; x < XSIZE; x++) vstems[x] = ledges[x] = redges[x] = 0;
	for (y = 0; y < YSIZE; y++) hstems[y] = tedges[y] = bedges[y] = 0;
	clearpath();
	for (y = 0; y < YSIZE; y++) {
		for (x = 0; x < XSIZE; x++) {
			if (GETPIX(x, y)) {
				bool tl, tr, bl, br;

				/* Assume filled in */
				tl = tr = bl = br = true;
				/* Check for diagonals */
				if ((UL && !U && !L) || (DR && !D && !R))
					tr = bl = false;
				if ((UR && !U && !R) || (DL && !D && !L))
					tl = br = false;
				/* Avoid odd gaps */
				if (L || UL || U) tl = true;
				if (R || UR || U) tr = true;
				if (L || DL || D) bl = true;
				if (R || DR || D) br = true;
				blackpixel(x, YSIZE - y - 1, bl, br, tr, tl);
				if (!L && !R && (U || D))
					vstems[x]++;
				if (!U && !D && (L || R))
					hstems[y]++;
				if (UL + U + UR + L + R + DL + D + DR == 0)
					vstems[x]++, hstems[y]++;
				if (UL + U + UR == 0) tedges[y]++;
				if (DL + D + DR == 0) bedges[y]++;
				if (UL + L + DL == 0) ledges[x]++;
				if (UR + R + DR == 0) redges[x]++;
			} else {
				bool tl, tr, bl, br;

				/* Assume clear */
				tl = tr = bl = br = false;
				/* white pixel -- just diagonals */
				if (L && U && !UL) tl = true;
				if (R && U && !UR) tr = true;
				if (L && D && !DL) bl = true;
				if (R && D && !DR) br = true;
				whitepixel(x, YSIZE - y - 1, bl, br, tr, tl);
			}
		}
	}
	clean_path();
	adjust_weight();
	record_lsb(g);
	emit_hints(vstems, ledges, redges, hstems, tedges, bedges);
	emit_path();
}

static void
reverse_contour(point *a)
{
	point *tmp;

	while (a->prev != NULL)
		a = a->prev;
	while (a != NULL) {
		tmp = a->next;
		a->next = a->prev;
		a->prev = tmp;
		a = tmp;
	}
}

/* Join together two points each at the end of a contour */
static void
join_ends(point *a, point *b)
{
	point *tmp;

	if (a->prev == NULL && b->next == NULL) {
		tmp = a; a = b; b = tmp;
	}
	if (a->prev == NULL)
		reverse_contour(a);
	if (b->next == NULL)
		reverse_contour(b);
	assert(a->next == NULL);
	assert(a->prev != NULL);
	assert(b->prev == NULL);
	assert(b->next != NULL);
	assert(vec_eqp(a->v, b->v));
	a->next = b;
	b->prev = a;
	fix_identical(a); /* Will delete a */
	fix_collinear(b); /* Might delete b */
}

static bool
point_endp(point *p)
{
	return p->next == NULL || p->prev == NULL;
}

static bool
point_deadp(point *p)
{
	return p->next == NULL && p->prev == NULL;
}

static void
clean_skeleton()
{
	int i, j;

	/* Pass 1: join collinear connected segments */
	for (i = 0; i < nextpoint; i++) {
		if (point_deadp(&points[i]))
			continue;
		for (j = 0; j < nextpoint; j++) {
			if (point_deadp(&points[j]))
				continue;
			if (vec_eqp(points[i].v, points[j].v) &&
			    points[i].next == NULL && points[j].prev == NULL &&
			    vec_inline3(points[i].prev->v, points[i].v,
					points[j].next->v))
				join_ends(&points[i], &points[j]);
			if (point_deadp(&points[i]))
				break;
		}
	}
	/* Pass 2: join any connected segments */
	for (i = 0; i < nextpoint; i++) {
		if (point_deadp(&points[i]))
			continue;
		for (j = i+1; j < nextpoint; j++) {
			if (point_deadp(&points[j]))
				continue;
			if (vec_eqp(points[i].v, points[j].v) &&
			    point_endp(&points[i]) && point_endp(&points[j]))
				join_ends(&points[i], &points[j]);
			if (point_deadp(&points[i]))
				break;
		}
	}
}

static void
dochar_plotter(struct glyph *g)
{
	int x, y;

#define CONNECT(dx, dy) do { \
		moveto(x * XPIX_S, (YSIZE-y-1) * YPIX_S); \
		lineto((x+dx) * XPIX_S, (YSIZE-y-1+dy) * YPIX_S); \
	} while (0)

	clearpath();
	for (y = 0; y < YSIZE; y++) {
		for (x = 0; x < XSIZE; x++) {
			if (GETPIX(x, y)) {
				if (R) CONNECT(1, 0);
				if (D) CONNECT(0, -1);
				if (DR && !D && !R) CONNECT(1, -1);
				if (DL && !D && !L) CONNECT(-1, -1);
				if (!U && !D && !L && !R &&
				    !UL && !UR && !DL && !DR)	{
					/* draw a dot */
					CONNECT(0, 0);
				}
			}
		}
	}
	clean_skeleton();
	emit_path();
}

static void
tile(int x0, int y0, int x1, int y1)
{
	x0 *= XPIX_S; y0 *= YPIX_S;
	x1 *= XPIX_S; y1 *= YPIX_S;
	moveto(x0, y0);  lineto(x1, y0); lineto(x1, y1); lineto(x0, y1);
	closepath();
}

static void
domosaic(struct glyph *g)
{
	unsigned int code = g->data[0];
	bool sep = (g->flags & SEP) != 0;

	clearpath();
	if (code & 1)  tile(0 + sep, 7 + sep, 3, 10);
	if (code & 2)  tile(3 + sep, 7 + sep, 6, 10);
	if (code & 4)  tile(0 + sep, 3 + sep, 3, 7);
	if (code & 8)  tile(3 + sep, 3 + sep, 6, 7);
	if (code & 16) tile(0 + sep, 0 + sep, 3, 3);
	if (code & 32) tile(3 + sep, 0 + sep, 6, 3);
	clean_path();
	record_lsb(g);
	emit_path();
}

static void
domosaic4(struct glyph *g)
{
	unsigned int code = g->data[0];
	bool sep = (g->flags & SEP) != 0;

	clearpath();
	if (code & 1) tile(0 + sep, 5 + sep, 3, 10);
	if (code & 2) tile(3 + sep, 5 + sep, 6, 10);
	if (code & 4) tile(0 + sep, 0 + sep, 3, 5);
	if (code & 8) tile(3 + sep, 0 + sep, 6, 5);
	clean_path();
	record_lsb(g);
	emit_path();
}

static void
domosaic8(struct glyph *g)
{
	unsigned int code = g->data[0];

	clearpath();
	if (code & 1)   tile(0, 7, 3, 10);
	if (code & 2)   tile(3, 7, 6, 10);
	if (code & 4)   tile(0, 5, 3, 7);
	if (code & 8)   tile(3, 5, 6, 7);
	if (code & 16)  tile(0, 3, 3, 5);
	if (code & 32)  tile(3, 3, 6, 5);
	if (code & 64)  tile(0, 0, 3, 3);
	if (code & 128) tile(3, 0, 6, 3);
	clean_path();
	record_lsb(g);
	emit_path();
}

static int
byunicode(const void *va, const void *vb)
{
	struct glyph const *a = *(struct glyph const **)va,
		*b = *(struct glyph const **)vb;

	if (a->unicode < b->unicode) return -1;
	if (a->unicode > b->unicode) return +1;
	return namecmp(a->name, b->name);
}

static void
glyph_complement()
{
	int const nrow = 16, ncol=12;
	unsigned long unicol = 32/nrow;
	int i, col = -1, row = 0;
	int npages = 0;
	bool newcol = false;
	struct glyph const *sorted[lenof(glyphs)], *g;

	for (i = 0; i < lenof(glyphs); i++)
		glyphs_by_name[i] = glyphs + i;
	qsort(glyphs_by_name, lenof(glyphs_by_name), sizeof(glyphs_by_name[0]),
	      &compare_glyphs_by_name);
	for (i = 0; i < lenof(glyphs); i++)
		sorted[i] = &glyphs[i];
	qsort(sorted, lenof(sorted), sizeof(sorted[0]), &byunicode);
	printf("%%!PS-Adobe\n");
	printf("%%%%Creator: bedstead %s\n", VERSION);
	printf("%%%%Title: %s %s Glyph Complement\n", FAMILY_NAME, VERSION);
	printf("/xfont /%s findfont 20 scalefont def\n", FAMILY_NAME);
	printf("/nfont /%s findfont 10 scalefont def\n", FAMILY_NAME);
	printf("/sfont /%s findfont 6 scalefont def\n", FAMILY_NAME);
	printf("/lfont /%s findfont 4 scalefont def\n", FAMILY_NAME);
	printf("/str 50 string def\n");
	printf("/centre {\n");
	printf(" dup stringwidth pop 2 div neg 0 rmoveto show\n");
	printf("} def\n");
	printf("/label {\n");
	printf(" lfont setfont centre\n");
	printf("} def\n");
	/* unicode glyphname  exemplify -- */
	printf("/exemplify {\n");
	printf(" xfont setfont dup 14 14 moveto glyphshow\n");
	printf(" 0 0 moveto 0 40 lineto 40 40 lineto 40 0 lineto closepath\n");
	printf("  1 setlinewidth gsave stroke grestore clip\n");
	printf(" 0.1 setlinewidth\n");
	printf("  14 10 moveto 14 30 lineto 26 30 lineto 26 10 lineto\n");
	printf("  closepath stroke\n");
	printf(" 20 36 moveto str cvs label\n");
	printf(" 20 2 moveto label\n");
	printf("} def\n");
	printf("/sash {\n");
	printf(" gsave 20 20 translate 45 rotate\n");
	printf(" 8 setlinewidth -30 0 moveto 30 0 lineto stroke\n");
	printf(" 1 setgray sfont setfont -20 -2.1 moveto show grestore\n");
	printf("} def\n");
	printf("/colnum {\n");
	printf(" nfont setfont 20 42 moveto centre\n");
	printf("} def\n");
	printf("/rownums {\n");
	printf(" nfont setfont 0 1 15 {\n");
	printf("  dup -40 mul 16.5 add -10 exch moveto\n");
	printf("  16 1 string cvrs show\n");
	printf(" } for\n");
	printf("} def\n");
	printf("%%%%EndProlog\n");
	for (i = 0; i < lenof(sorted); i++) {
		g = sorted[i];
		if (g->unicode / nrow != unicol ||
		    (g->unicode == NU && row == nrow)) {
			if (++col == ncol) {
				printf("grestore showpage\n");
				col = -1;
			}
			unicol = g->unicode / nrow;
			row = 0;
			newcol = true;
		}
		if (col == -1) {
			++npages;
			printf("%%%%Page: %d %d\n", npages, npages);
			printf("gsave 60 700 translate rownums\n");
			col = 0;
			newcol = true;
		}
		if (newcol && g->unicode != NU && !(g->unicode & U_HASVS)) {
			printf("gsave %d 0 translate (%03lX) colnum grestore\n",
			       col * 40, unicol);
			newcol = false;
		}
		printf("gsave %d %d translate ",
		       (col * 40),
		       (int)-((g->unicode == NU || (g->unicode & U_HASVS) ?
			       row++ : (int)(g->unicode % nrow)) * 40));
		if (g->unicode == NU)
			printf("()");
		else if (g->unicode & U_HASVS)
			printf("(<U+%04lX,U+%04lX>)",
			       (unsigned long)GET_UV(g->unicode),
			       (unsigned long)GET_UVS(g->unicode));
		else
			printf("(U+%04lX)", (unsigned long)g->unicode);
		printf("/%s ", g->name);
		printf("exemplify ");
		if (g->flags & COMPAT) {
			struct glyph const *target = g;
			while (target->flags & COMPAT)
				target = get_glyph_by_name(target->alias_of);
			if (target->unicode != NU)
				printf("(USE U+%04lX) sash ",
				       (unsigned long)target->unicode);
			else
				printf("(USE %s) sash ", g->alias_of);
		}
		printf(" grestore\n");
	}
	printf("showpage\n");
	printf("%%%%EOF\n");
}

static void
bdf_gen(int px_height)
{
	int px_width = (px_height * XPIX * XSIZE) / (YPIX * YSIZE);
	int pt_height = (long)(px_height * 7227 + 3750) / 7500;
	int dpt_height = (long)(px_height * 7227 + 375) / 750;
	int base = (px_height * DESCENT) / YSIZE;
	int i;

	printf("%%!\n");
	printf("/d [1 0 0 1 0 0] %d %d <ff 00> makeimagedevice def\n",
	       (px_width + 7) / 8 * 8, px_height);
	printf("d setdevice\n");
	printf("/%s findfont %d scalefont setfont\n",
	       fullname_to_fontname(get_fullname()), px_height);
	printf("/f ARGUMENTS 0 get (w) file def\n");
	printf("/buf %d string def\n", (px_width + 7) / 8);
	printf("(\\\n");
	printf("STARTFONT 2.1\n");
	printf("FONT -%s-%s-%s-R-%s--"
	       "%d-%d-75-75-C-%d-ISO10646-1\n", FOUNDRY, FAMILY_NAME,
	       *weight->suffix ? weight->suffix + 1 : "Medium",
	       *width->suffix ? width->suffix + 1 : "Normal",
	       dpt_height, px_height, px_width * 10);
	printf("SIZE %d 75 75\n", pt_height);
	printf("FONTBOUNDINGBOX %d %d 0 %d\n", px_width, px_height, -base);
	printf("STARTPROPERTIES 23\n");
	printf("FOUNDRY \"%s\"\n", FOUNDRY);
	printf("FAMILY_NAME \"%s\"\n", FAMILY_NAME);
	printf("WEIGHT_NAME \"%s\"\n",
	       *weight->suffix ? weight->suffix + 1 : "Medium");
	printf("SLANT \"R\"\n");
	printf("SETWIDTH_NAME \"%s\"\n",
	       *width->suffix ? width->suffix + 1 : "Normal");
	printf("ADD_STYLE_NAME \"\"\n");
	printf("PIXEL_SIZE %d\n", px_height);
	printf("POINT_SIZE %d\n", dpt_height);
	printf("RESOLUTION_X 75\n");
	printf("RESOLUTION_Y 75\n");
	printf("SPACING \"C\"\n");
	printf("AVERAGE_WIDTH %d\n", px_width * 10);
	printf("CHARSET_REGISTRY \"ISO10646\"\n");
	printf("CHARSET_ENCODING \"1\"\n");
	printf("UNDERLINE_POSITION %d\n", base / 2);
	printf("UNDERLINE_THICKNESS %d\n", base / 2);
	printf("RELATIVE_SETWIDTH %d\n", width->ttfwidth * 10);
	printf("RELATIVE_WEIGHT %d\n", weight->ttfweight / 10);
	printf("FACE_NAME \"%s\"\n", get_fullname());
	printf("FONT_VERSION \") f exch writestring\n");
	printf("currentfont /FontInfo get /version get f exch writestring\n");
	printf("(\"\n");
	printf("FONT_ASCENT %d\n", px_height - base);
	printf("FONT_DESCENT %d\n", base);
	printf("DEFAULT_CHAR %d\n", 0xf1ff);
	printf("ENDPROPERTIES\n");
	printf("CHARS %d\n", lenof(glyphs));
	for (i = 0; i < lenof(glyphs); i++) {
		struct glyph *g = &glyphs[i];
		printf("STARTCHAR %s\n", g->name);
		if (g->unicode == NU)
			printf("ENCODING -1\n");
		else
			printf("ENCODING %lu\n", (unsigned long)g->unicode);
		printf("SWIDTH %d 0\n", (int)(XPIX * XSIZE));
		printf("DWIDTH %d 0\n", px_width);
		printf("BBX %d %d 0 %d\n", px_width, px_height, -base);
		printf("BITMAP\n");
		printf(") f exch writestring\n");
		printf("erasepage\n");
		printf("0 %d moveto\n", base);
		printf("/%s glyphshow\n", g->name);
		printf("%d -1 0 {\n", px_height - 1);
		printf(" d exch buf copyscanlines\n");
		printf(" f exch writehexstring (\n) f exch writestring\n");
		printf("} for\n");
		printf("(\\\n");
		printf("ENDCHAR\n");
	}
	printf("ENDFONT\n");
	printf(") f exch writestring\n");
}
