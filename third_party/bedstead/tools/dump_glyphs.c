/* Dumps the bitmap glyphs of Bedstead (bedstead.c, CC0 1.0 with the SAA5050
 * typeface note of CDPA 1988 s.55) as plain text, one glyph per line:
 *   U+XXXX name r0 r1 ... r8
 * where each r is a 5-bit row, most significant bit = leftmost dot.
 * Temporary development glyph source; see ../PROVENANCE.md. */
#define main bedstead_main
#include "../bedstead.c"
#undef main
static struct glyph const *by_name(char const *name, int depth)
{
	size_t i;
	if (depth > 8) return NULL;
	for (i = 0; i < lenof(glyphs); i++) {
		struct glyph const *g = &glyphs[i];
		if (g->name && strcmp(g->name, name) == 0) {
			if (g->flags & IS_ALIAS) return by_name(g->alias_of, depth + 1);
			if (g->flags & IS_SUBR) return NULL;
			return g;
		}
	}
	return NULL;
}
int main(void)
{
	size_t i; int r;
	for (i = 0; i < lenof(glyphs); i++) {
		struct glyph const *g = &glyphs[i], *src = g;
		if (g->flags & IS_SUBR) continue;
		if (g->unicode == NU || (g->unicode & U_HASVS)) continue;
		if (g->flags & IS_ALIAS) {
			src = by_name(g->alias_of, 0);
			if (src == NULL) continue;
		}
		if (src->flags & (MOS6 | MOS4 | MOS8)) continue; /* block mosaics are computed, not stored */
		printf("U+%04lX %s", (unsigned long)GET_UV(g->unicode), g->name ? g->name : "-");
		for (r = 0; r < YSIZE - 1; r++) printf(" %02X", src->data[r]);
		printf("\n");
	}
	return 0;
}
