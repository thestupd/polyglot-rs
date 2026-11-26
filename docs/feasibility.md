# PNG/GIF + MPEG-TS feasibility

Date: 2026-11-26. Scope: research before production composition. No combination is enabled by these experiments.

## Findings

**PNG + MPEG-TS can decode through unmodified standard software when the transport interpretation permits leading/trailing non-TS bytes.** A private PNG ancillary chunk containing the unchanged transport fixture passed this library's PNG validation and FFmpeg 8.0 image decoding. Opening the entire same file with FFmpeg's MPEG-TS demuxer decoded all ten original video frames. This is a promising *conditional parser profile*, not strict byte-zero MPEG-TS conformance or general support for arbitrary inputs.

**GIF + MPEG-TS remains unproven for a structurally conforming composition.** Appending a TS stream decoded both ways in FFmpeg, but violates the current GIF validation policy and is only concatenation. Splitting TS bytes into ordinary GIF comment sub-blocks preserved the image but corrupted transport decoding. A viable structured layout would need coordinated packetization, not a generic byte insertion.

## Structural constraints

- PNG begins with `89 50 4e 47 0d 0a 1a 0a`; a byte-zero 188-byte MPEG-TS stream begins with `47`. Those two strict starting signatures cannot coincide. PNG provides private ancillary chunks with length/type/data/CRC envelopes that image decoders can skip. Such a chunk can preserve a contiguous TS stream, but the PNG prefix and suffix require an explicitly tolerant transport parser. PNG requires IEND to be last. See the [PNG specification, file structure and decoder behavior](https://www.w3.org/TR/png/).
- GIF starts with `GIF87a` or `GIF89a`. Its initial `G` is already the TS sync byte `47`, so a strict GIF/TS overlap cannot be ruled out solely by the first byte. The following fixed bytes constrain the first TS packet's PID, control flags and adaptation length; later GIF structure must also satisfy every 188-byte packet boundary. GIF comments/application data use length-prefixed sub-blocks of at most 255 data bytes. Inserting those length bytes into an existing TS stream changes its packet layout. See the [GIF89a specification, data sub-blocks, header and trailer](https://www.w3.org/Graphics/GIF/spec-gif89a.txt).
- Transport sync recovery is parser behavior, not a guarantee of strict conformance. FFmpeg scans for packet synchronization and has a bounded resynchronization mechanism; capture size, probing, initial offset, trailing data and demuxer selection can affect results. See the [FFmpeg demuxer implementation](https://ffmpeg.org/doxygen/trunk/mpegts_8c_source.html). This library's current MPEG-TS handler requires complete packets starting at byte zero and validates observed PAT/PMT CRCs and packet continuity.

## Reproducible experiments

Run `npm run build`, then `python3 scripts/feasibility.py`. Checked-in observations are in [feasibility-results.json](feasibility-results.json). Research candidates exist only in a temporary directory. The script does not call the production composition API.

Inputs: the existing 2 × 2 red PNG, 1 × 1 black GIF, and ten-frame 16 × 16 MPEG-2 TS fixture. Every FFmpeg command uses `-xerror -err_detect explode`, decodes the complete input to RGB24, preserves frame timing with `-fps_mode passthrough`, checks exit status and compares decoded content. Image interpretations use `-f png_pipe` / `-f gif`; transport uses `-f mpegts`. These select unmodified parsers without extracting or rewriting the input. Automatic probing is recorded separately.

| Research control | Native image / TS validity | Complete image decode | Complete TS decode |
| --- | --- | --- | --- |
| PNG followed by TS | invalid / invalid | error after initial image | ten matching video frames |
| GIF followed by TS | invalid / invalid | one matching image frame | ten matching video frames |
| TS in private PNG ancillary chunk before IEND | valid / invalid | four matching RGB pixels | ten matching video frames |
| TS split into GIF comment sub-blocks | valid / invalid | one matching image frame | decoding error; incomplete content |

Netpbm's `pngtopam` / `giftopnm`, when installed, also accepted the image interpretations. The script records their exit status and output size; pixel equality claims above come from the FFmpeg assertions. Automatic probing selected the image interpretation for these controls; it did not demonstrate simultaneous media discovery.

## Implementation decision

Keep the current strict validators unchanged. Append must report PNG/GIF combinations unsupported under those profiles. Generic append of opaque buffers is an explicitly unvalidated candidate, never proof of a polyglot.

The next viable production experiment is a named, opt-in PNG ancillary embedding strategy paired with a documented MPEG-TS resynchronization profile. It needs bounded offset discovery, full-image and full-video decoding tests across more fixtures/parser versions, and preservation checks before any support claim. GIF needs a separate packet-aware design that accounts for both the initial header and every sub-block length byte. JPEG + ZIP requires its own investigation after those handlers exist.
