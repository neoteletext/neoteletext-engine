/*
 * Test oracle for REQ-TST-021: decodes a T42 file with the system's zvbi
 * library and prints every cached page in the layout of `ttx text FILE
 * all`, so the two decoders can be compared. zvbi is run as a separate
 * program at test time (REQ-TST-001); none of its code is copied here.
 *
 * Build: cc -O2 zvbi_pages.c $(pkg-config --cflags --libs zvbi-0.2)
 * Usage: zvbi_pages FILE.t42 [GROUP]
 * GROUP is the default character set group of EN 300 706 Table 32, 0 to
 * 10, as given to `ttx --region`.
 */
#include <libzvbi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* A T42 packet: two address bytes and 40 data bytes (EN 300 706 clause 7). */
#define PACKET_BYTES 42
/* Packets handed to zvbi per call, about one field's worth of lines. */
#define LINES_PER_FIELD 16
/* Seconds per field at 50 fields per second. */
#define FIELD_SECONDS 0.02
/* Teletext rows 0 to 24 (EN 300 706 clause 9.3). */
#define ROWS 25
#define COLUMNS 40
/* zvbi's codes for block mosaics and DRCS are in the private use area;
 * they are printed as one marker, since only text decoding is compared. */
#define MOSAIC_MARKER '#'
/* National options per character set group (EN 300 706 Table 32). */
#define NATIONAL_OPTIONS 8

/* Writes one character as UTF-8. */
static void put_utf8(unsigned int code) {
    if (code < 0x80) {
        putchar((int) code);
    } else if (code < 0x800) {
        putchar((int) (0xC0 | (code >> 6)));
        putchar((int) (0x80 | (code & 0x3F)));
    } else {
        putchar((int) (0xE0 | (code >> 12)));
        putchar((int) (0x80 | ((code >> 6) & 0x3F)));
        putchar((int) (0x80 | (code & 0x3F)));
    }
}

/* Distinct pages and sub-pages remembered from zvbi's events. */
#define MAX_PAGES 8192

static struct {
    int pgno;
    int subno;
} seen[MAX_PAGES];
static int seen_count;

/* zvbi announces each page it has received; its cache does not always
 * answer for the exact sub-code it announced, so both that and "any" are
 * tried when printing. */
static void on_page(vbi_event *event, void *user) {
    (void) user;
    int pgno = event->ev.ttx_page.pgno;
    int subno = event->ev.ttx_page.subno;
    for (int i = 0; i < seen_count; i++) {
        if (seen[i].pgno == pgno && seen[i].subno == subno) {
            return;
        }
    }
    if (seen_count < MAX_PAGES) {
        seen[seen_count].pgno = pgno;
        seen[seen_count].subno = subno;
        seen_count++;
    }
}

int main(int argc, char **argv) {
    if (argc != 2 && argc != 3) {
        fprintf(stderr, "usage: %s FILE.t42 [GROUP]\n", argv[0]);
        return 2;
    }
    FILE *file = fopen(argv[1], "rb");
    if (!file) {
        perror(argv[1]);
        return 3;
    }
    vbi_decoder *decoder = vbi_decoder_new();
    vbi_teletext_set_level(decoder, VBI_WST_LEVEL_1p5);
    /* zvbi numbers regions as Table 32 rows: eight national options per
     * group. */
    int group = argc == 3 ? atoi(argv[2]) : 0;
    vbi_teletext_set_default_region(decoder, group * NATIONAL_OPTIONS);
    vbi_event_handler_register(decoder, VBI_EVENT_TTX_PAGE, on_page, NULL);
    vbi_sliced lines[LINES_PER_FIELD];
    double time = 0.0;
    int count = 0;
    unsigned char packet[PACKET_BYTES];
    while (fread(packet, 1, PACKET_BYTES, file) == PACKET_BYTES) {
        memset(&lines[count], 0, sizeof lines[count]);
        lines[count].id = VBI_SLICED_TELETEXT_B;
        lines[count].line = 7 + (unsigned) count;
        memcpy(lines[count].data, packet, PACKET_BYTES);
        if (++count == LINES_PER_FIELD) {
            vbi_decode(decoder, lines, count, time);
            time += FIELD_SECONDS;
            count = 0;
        }
    }
    if (count > 0) {
        vbi_decode(decoder, lines, count, time);
    }
    fclose(file);
    int printed = 0;
    for (int i = 0; i < seen_count; i++) {
        vbi_page page;
        if (!vbi_fetch_vt_page(decoder, &page, seen[i].pgno, seen[i].subno, VBI_WST_LEVEL_1p5, ROWS, FALSE)
            && !vbi_fetch_vt_page(decoder, &page, seen[i].pgno, VBI_ANY_SUBNO, VBI_WST_LEVEL_1p5, ROWS, FALSE)) {
            continue;
        }
        printf("=== %03x:%04x\n", page.pgno, page.subno & 0x3F7F);
        for (int row = 0; row < page.rows; row++) {
            for (int column = 0; column < page.columns; column++) {
                unsigned int code = page.text[row * page.columns + column].unicode;
                put_utf8(vbi_is_print(code) ? code : (unsigned int) (vbi_is_gfx(code) || vbi_is_drcs(code) ? MOSAIC_MARKER : ' '));
            }
            putchar('\n');
        }
        vbi_unref_page(&page);
        printed++;
    }
    fprintf(stderr, "zvbi announced %d pages and sub-pages; %d could be fetched\n", seen_count, printed);
    vbi_decoder_delete(decoder);
    return 0;
}
