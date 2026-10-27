package io.github.codeghost.songbook

/**
 * Thin wrapper around the Rust engine (cli/src/lib.rs, JNI in cli/src/android.rs).
 * All parsing, layout and PDF writing happens natively; Kotlin only passes the
 * file contents plus a key=value configuration and receives the PDF back.
 */
object Engine {

    init {
        System.loadLibrary("songbook")
    }

    data class Rendered(
        val pdf: ByteArray,
        val songs: Int,
        val pages: Int,
        val rejected: Int,
        val userContent: Int,
        val summary: String,
        val error: String?
    ) {
        val ok: Boolean get() = error == null && pdf.isNotEmpty()
    }

    data class Loaded(
        val songs: Int,
        val rejected: Int,
        val userContent: Int,
        val summary: String
    )

    private external fun nativeRender(input: String, config: String): ByteArray

    private external fun nativeInspect(input: String, order: String): String

    fun render(input: String, config: String): Rendered {
        val packet = nativeRender(input, config)
        if (packet.size < 4) {
            return Rendered(ByteArray(0), 0, 0, 0, 0, "", "engine returned nothing")
        }
        val metaLen = ((packet[0].toInt() and 0xFF) shl 24) or
                ((packet[1].toInt() and 0xFF) shl 16) or
                ((packet[2].toInt() and 0xFF) shl 8) or
                (packet[3].toInt() and 0xFF)
        val meta = String(packet, 4, metaLen, Charsets.UTF_8)
        val pdf = packet.copyOfRange(4 + metaLen, packet.size)
        val fields = parse(meta)
        return Rendered(
            pdf = pdf,
            songs = fields["songs"]?.toIntOrNull() ?: 0,
            pages = fields["pages"]?.toIntOrNull() ?: 0,
            rejected = fields["rejected"]?.toIntOrNull() ?: 0,
            userContent = fields["user_content"]?.toIntOrNull() ?: 0,
            summary = fields["summary"] ?: "",
            error = fields["error"]
        )
    }

    fun inspect(input: String, order: String): Loaded {
        val fields = parse(nativeInspect(input, order) ?: "")
        return Loaded(
            songs = fields["songs"]?.toIntOrNull() ?: 0,
            rejected = fields["rejected"]?.toIntOrNull() ?: 0,
            userContent = fields["user_content"]?.toIntOrNull() ?: 0,
            summary = fields["summary"] ?: ""
        )
    }

    private fun parse(meta: String): Map<String, String> {
        val out = HashMap<String, String>()
        for (line in meta.lineSequence()) {
            val i = line.indexOf('=')
            if (i > 0) out[line.substring(0, i)] = unescape(line.substring(i + 1))
        }
        return out
    }

    private fun unescape(s: String): String {
        val b = StringBuilder(s.length)
        var i = 0
        while (i < s.length) {
            val c = s[i]
            if (c == '\\' && i + 1 < s.length) {
                when (val n = s[i + 1]) {
                    'n' -> b.append('\n')
                    '\\' -> b.append('\\')
                    else -> b.append(n)
                }
                i += 2
            } else {
                b.append(c)
                i++
            }
        }
        return b.toString()
    }
}
