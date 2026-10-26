/* The reference side of the Opus oracle (run_opus.sh, decision E.123): libopus's fixed-point
 * decoder, driven as the profile is decoded. opus_demo is not used for this, because it decodes a
 * lost packet late and from the next packet's in-band FEC data, which the profile does not decode.
 *
 * Reads an opus_demo stream (each packet a 4-byte big-endian length, a 4-byte big-endian final
 * range and the payload; a length of 0 is a lost packet) and writes what
 * neoteletext-opus's decode_bit writes: 16-bit little-endian samples, one 20 ms frame a packet,
 * and the decoder's final range after each packet as 8 hexadecimal digits a line.
 *
 *   opus_decode IN.bit OUT.pcm OUT.ranges
 */
#include <opus.h>
#include <stdio.h>

/* The profile: 48 kHz mono output, 20 ms frames (decision E.123). */
#define RATE 48000
#define FRAME 960
/* RFC 6716 section 3.4, R1 and R2: no packet exceeds 1275 bytes of frame and its header. */
#define MAX_PACKET 1500

static int read_be32(FILE *file, opus_uint32 *value) {
    unsigned char bytes[4];
    if (fread(bytes, 1, 4, file) != 4) return 0;
    *value = ((opus_uint32)bytes[0] << 24) | ((opus_uint32)bytes[1] << 16) | ((opus_uint32)bytes[2] << 8) | bytes[3];
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr, "usage: opus_decode IN.bit OUT.pcm OUT.ranges\n");
        return 2;
    }
    FILE *in = fopen(argv[1], "rb");
    FILE *pcm_out = fopen(argv[2], "wb");
    FILE *ranges_out = fopen(argv[3], "w");
    if (!in || !pcm_out || !ranges_out) {
        perror("opus_decode");
        return 2;
    }
    int error = 0;
    OpusDecoder *decoder = opus_decoder_create(RATE, 1, &error);
    if (error != OPUS_OK) {
        fprintf(stderr, "opus_decode: %s\n", opus_strerror(error));
        return 2;
    }
    unsigned char packet[MAX_PACKET];
    opus_int16 pcm[FRAME];
    opus_uint32 length = 0, encoder_range = 0;
    long index = 0;
    while (read_be32(in, &length) && read_be32(in, &encoder_range)) {
        if (length > MAX_PACKET || fread(packet, 1, length, in) != length) {
            fprintf(stderr, "opus_decode: packet %ld: truncated or too long\n", index);
            return 1;
        }
        int samples = length == 0 ? opus_decode(decoder, NULL, 0, pcm, FRAME, 0)
                                  : opus_decode(decoder, packet, (opus_int32)length, pcm, FRAME, 0);
        if (samples != FRAME) {
            fprintf(stderr, "opus_decode: packet %ld of %u bytes: %s\n", index, (unsigned)length,
                    samples < 0 ? opus_strerror(samples) : "short frame");
            return 1;
        }
        opus_uint32 range = 0;
        opus_decoder_ctl(decoder, OPUS_GET_FINAL_RANGE(&range));
        for (int i = 0; i < FRAME; i++) {
            unsigned char bytes[2] = {(unsigned char)(pcm[i] & 0xFF), (unsigned char)((pcm[i] >> 8) & 0xFF)};
            fwrite(bytes, 1, 2, pcm_out);
        }
        fprintf(ranges_out, "%08x\n", (unsigned)range);
        index++;
    }
    opus_decoder_destroy(decoder);
    if (fclose(pcm_out) != 0 || fclose(ranges_out) != 0) {
        perror("opus_decode");
        return 2;
    }
    fclose(in);
    return 0;
}
