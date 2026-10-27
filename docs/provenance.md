# Provenance

The print layout implemented by this project was recovered from a commercial Android
songbook app that is written in a BASIC dialect and shipped fully obfuscated. No source, resources or
assets of that app are used here: what was studied was its observable behaviour - the
pages it prints - and the constants that produce them.

The work was done in three steps.

1. **Observation.** Print a reference songbook with a known list for every combination
   of paper, margin and column count and measure the result: page size, where each row
   starts, how wide each of the three columns is, at what size the text is drawn, and
   what happens when it does not fit.
2. **Constant recovery.** Decompile the APK, normalise the obfuscated identifiers into
   readable ones and read the layout tables directly, so the numbers are exact rather
   than measured - row counts per page are hard-coded per paper and margin, for
   instance, not computed.
3. **Reimplementation.** Write the engine from scratch, first as a single Python file to
   compare against the reference output, then as the Rust engine in `cli/`.

The first two steps produced the tables in [usage.md](usage.md); the third produced this
repository. Verification was done by rendering the same song list with both and
comparing pages at 150 dpi.

Deliberately not reproduced: advertising, the in-app song database, spreadsheet import,
per-songbook "user content" lists and the karaoke type icons. This project is only the
song list to PDF path.

Changes made on purpose:

* `,` and `;` are accepted as delimiters, with CSV quoting
* four columns work for any songbook, not only the artist-style one
* text that still does not fit is truncated instead of drawn over the next cell

The layout itself is a fact about a file format, not creative expression; the code here
is original and MIT licensed.
