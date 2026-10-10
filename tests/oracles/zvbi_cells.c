/*
 * Test oracle for REQ-TST-023: decodes a T42 file with the system's zvbi
 * library at Presentation Level 2.5 or 3.5 and prints every cached page
 * cell by cell: character, Colour Map indices, size and attributes. zvbi
 * is run as a separate program at test time (REQ-TST-001); none of its
 * code is copied here. compare_cells.py compares the output with
 * `ttx cells`.
 *
 * Colours are printed as Colour Map indices, never as zvbi's RGB values:
 * zvbi's default map differs from EN 300 706 Table 30 at CLUT 1 entries 5
 * and 6 (decision E.40), so only the indices are comparable.
 *
 * Build: cc -O2 zvbi_cells.c $(pkg-config --cflags --libs zvbi-0.2)
 * Usage: zvbi_cells FILE.t42 LEVEL [GROUP]
 * LEVEL is 2.5 or 3.5. GROUP is the default character set group of EN 300
 * 706 Table 32, 0 to 10.
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
/* National options per character set group (EN 300 706 Table 32). */
#define NATIONAL_OPTIONS 8
/* Distinct pages and sub-pages remembered from zvbi's events. */
#define MAX_PAGES 8192

static struct {
    int pgno;
    int subno;
} seen[MAX_PAGES];
static int seen_count;

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

/* One cell as "UUUU/F/B/S/flags": the Unicode code point, foreground and
 * background Colour Map indices, zvbi's size code and the attribute flags
 * (u underline, b bold, i italic, f flash, c conceal, p proportional). A
 * DRCS cell prints its code as zvbi gives it, which encodes the table and
 * character. */
static void put_cell(const vbi_char *cell) {
    printf("%04X/%u/%u/%u/", (unsigned) cell->unicode, (unsigned) cell->foreground,
           (unsigned) cell->background, (unsigned) cell->size);
    if (cell->underline) putchar('u');
    if (cell->bold) putchar('b');
    if (cell->italic) putchar('i');
    if (cell->flash) putchar('f');
    if (cell->conceal) putchar('c');
    if (cell->proportional) putchar('p');
}

int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) {
        fprintf(stderr, "usage: %s FILE.t42 2.5|3.5 [GROUP]\n", argv[0]);
        return 2;
    }
    vbi_wst_level level;
    if (strcmp(argv[2], "2.5") == 0) {
        level = VBI_WST_LEVEL_2p5;
    } else if (strcmp(argv[2], "3.5") == 0) {
        level = VBI_WST_LEVEL_3p5;
    } else {
        fprintf(stderr, "level must be 2.5 or 3.5\n");
        return 2;
    }
    FILE *file = fopen(argv[1], "rb");
    if (!file) {
        perror(argv[1]);
        return 3;
    }
    vbi_decoder *decoder = vbi_decoder_new();
    vbi_teletext_set_level(decoder, level);
    int group = argc == 4 ? atoi(argv[3]) : 0;
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
        if (!vbi_fetch_vt_page(decoder, &page, seen[i].pgno, seen[i].subno, level, ROWS, FALSE)
            && !vbi_fetch_vt_page(decoder, &page, seen[i].pgno, VBI_ANY_SUBNO, level, ROWS, FALSE)) {
            continue;
        }
        printf("=== %03x:%04x columns %d screen %u\n", page.pgno, page.subno & 0x3F7F, page.columns,
               (unsigned) page.screen_color);
        for (int row = 0; row < page.rows; row++) {
            printf("%02d", row);
            for (int column = 0; column < page.columns; column++) {
                putchar(' ');
                put_cell(&page.text[row * page.columns + column]);
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
