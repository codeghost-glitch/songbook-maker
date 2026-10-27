package io.github.codeghost.songbook

import android.app.Activity
import android.app.AlertDialog
import android.content.ActivityNotFoundException
import android.content.ContentValues
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.MediaStore
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream

/**
 * One screen: pick a song list, tune the layout, get a printable PDF.
 * Every pixel of the PDF comes from the Rust engine through [Engine].
 */
class MainActivity : Activity() {

    private class Choice(val id: String, val label: String)

    /** A named look: the engine owns the styling, the app mirrors its layout. */
    private class Style(
        val id: String,
        val label: String,
        val paper: String,
        val columns: Int,
        val margin: Float
    )

    private val styles = listOf(
        Style("default", "Classic", "a4", 3, 0.25f),
        Style("platinum", "Platinum", "letter", 3, 0.30f),
        Style("tj", "TJ Media", "a4", 4, 0.25f)
    )
    private val papers = listOf(
        Choice("a4", "A4  8.3 x 11.7 in"),
        Choice("letter", "Letter  8.5 x 11 in"),
        Choice("long", "Long  8.5 x 13 in"),
        Choice("legal", "Legal  8.5 x 14 in")
    )
    private val margins = floatArrayOf(0.20f, 0.25f, 0.30f, 0.40f, 0.50f)
    private val columns = listOf(
        Choice("2", "2 columns"),
        Choice("3", "3 columns"),
        Choice("4", "4 columns")
    )
    private val sections = listOf(
        Choice("continuous", "Continuous"),
        Choice("newpage", "New page per letter"),
        Choice("none", "No letters")
    )
    private val sorts = listOf(
        Choice("title", "Title"),
        Choice("artist", "Artist"),
        Choice("number", "Number")
    )

    private var style = 0
    private var paper = 0
    private var margin = 1
    private var column = 1
    private var section = 0
    private var sort = 0

    private var text = ""
    private var fileName = ""
    private var shareUri: Uri? = null

    private class Saved(val uri: Uri, val location: String)

    private lateinit var tvFile: TextView
    private lateinit var tvStatus: TextView
    private lateinit var tvRejected: TextView
    private lateinit var tvResult: TextView
    private lateinit var tvStyle: TextView
    private lateinit var tvPaper: TextView
    private lateinit var tvColumns: TextView
    private lateinit var tvMargin: TextView
    private lateinit var tvSections: TextView
    private lateinit var tvSort: TextView
    private lateinit var bannerRejected: View
    private lateinit var rowStyle: View
    private lateinit var rowPaper: View
    private lateinit var rowColumns: View
    private lateinit var rowMargin: View
    private lateinit var rowSections: View
    private lateinit var rowSort: View
    private lateinit var etTitle: EditText
    private lateinit var etFooter: EditText
    private lateinit var swBold: Switch
    private lateinit var swNumbers: Switch
    private lateinit var swMirror: Switch
    private lateinit var swArtist: Switch
    private lateinit var btnPick: Button
    private lateinit var btnCreate: Button
    private lateinit var btnShare: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        tvFile = findViewById(R.id.tvFile)
        tvStatus = findViewById(R.id.tvStatus)
        tvRejected = findViewById(R.id.tvRejected)
        tvResult = findViewById(R.id.tvResult)
        tvStyle = findViewById(R.id.tvStyle)
        tvPaper = findViewById(R.id.tvPaper)
        tvColumns = findViewById(R.id.tvColumns)
        tvMargin = findViewById(R.id.tvMargin)
        tvSections = findViewById(R.id.tvSections)
        tvSort = findViewById(R.id.tvSort)
        bannerRejected = findViewById(R.id.bannerRejected)
        rowStyle = findViewById(R.id.rowStyle)
        rowPaper = findViewById(R.id.rowPaper)
        rowColumns = findViewById(R.id.rowColumns)
        rowMargin = findViewById(R.id.rowMargin)
        rowSections = findViewById(R.id.rowSections)
        rowSort = findViewById(R.id.rowSort)
        etTitle = findViewById(R.id.etTitle)
        etFooter = findViewById(R.id.etFooter)
        swBold = findViewById(R.id.swBold)
        swNumbers = findViewById(R.id.swNumbers)
        swMirror = findViewById(R.id.swMirror)
        swArtist = findViewById(R.id.swArtist)
        btnPick = findViewById(R.id.btnPick)
        btnCreate = findViewById(R.id.btnCreate)
        btnShare = findViewById(R.id.btnShare)

        btnPick.setOnClickListener { pickFile() }
        btnCreate.setOnClickListener { createPdf() }
        btnShare.setOnClickListener { sharePdf() }

        rowStyle.setOnClickListener {
            choose(getString(R.string.choose_style), styles.map { it.label }, style) {
                style = it
                applyStyle()
            }
        }
        rowPaper.setOnClickListener {
            choose(getString(R.string.choose_paper), papers.map { it.label }, paper) {
                paper = it
            }
        }
        rowColumns.setOnClickListener {
            choose(getString(R.string.choose_columns), columns.map { it.label }, column) {
                column = it
            }
        }
        rowMargin.setOnClickListener {
            val labels = margins.map { String.format(java.util.Locale.ROOT, "%.2f in", it) }
            choose(getString(R.string.choose_margin), labels, margin) { margin = it }
        }
        rowSections.setOnClickListener {
            choose(getString(R.string.choose_sections), sections.map { it.label }, section) {
                section = it
            }
        }
        rowSort.setOnClickListener {
            choose(getString(R.string.choose_sort), sorts.map { it.label }, sort) { sort = it }
        }

        updateLabels()
        updateAccessibility()
    }

    // ------------------------------------------------------------ pickers

    /** Selecting a look also moves the layout pickers to its defaults. */
    private fun applyStyle() {
        val s = styles[style]
        papers.indexOfFirst { it.id == s.paper }.takeIf { it >= 0 }?.let { paper = it }
        column = (s.columns - 2).coerceIn(0, columns.size - 1)
        margins.indexOfFirst { kotlin.math.abs(it - s.margin) < 0.001f }
            .takeIf { it >= 0 }?.let { margin = it }
    }

    private fun choose(title: String, entries: List<String>, selected: Int, onPick: (Int) -> Unit) {
        AlertDialog.Builder(this)
            .setTitle(title)
            .setSingleChoiceItems(entries.toTypedArray(), selected) { dialog, which ->
                onPick(which)
                dialog.dismiss()
                updateLabels()
                updateAccessibility()
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    // -------------------------------------------------------------- import

    private fun pickFile() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
        intent.addCategory(Intent.CATEGORY_OPENABLE)
        intent.type = "*/*"
        try {
            @Suppress("DEPRECATION")
            startActivityForResult(intent, REQ_PICK)
        } catch (e: ActivityNotFoundException) {
            toast(getString(R.string.err_read))
        }
    }

    @Deprecated("Activity result API needs AndroidX; this app has no dependencies")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data ?: return
        if (requestCode == REQ_PICK && resultCode == RESULT_OK) load(uri)
    }

    private fun load(uri: Uri) {
        val bytes = try {
            contentResolver.openInputStream(uri)?.use { input ->
                val buffer = ByteArrayOutputStream()
                input.copyTo(buffer)
                buffer.toByteArray()
            }
        } catch (e: Exception) {
            null
        }
        if (bytes == null) {
            toast(getString(R.string.err_read))
            return
        }
        text = String(bytes, Charsets.UTF_8).removePrefix("﻿")
        fileName = uri.lastPathSegment?.substringAfterLast('/') ?: "song list"
        tvFile.text = fileName
        tvFile.contentDescription = getString(R.string.pick_file) + ": " + fileName
        hideResult()
        disableShare()

        btnPick.isEnabled = false
        Thread {
            val info = try {
                Engine.inspect(text, "auto")
            } catch (e: Throwable) {
                null
            }
            runOnUiThread {
                btnPick.isEnabled = true
                if (info == null) {
                    toast(getString(R.string.err_read))
                    return@runOnUiThread
                }
                showLoaded(info)
            }
        }.start()
    }

    private fun showLoaded(info: Engine.Loaded) {
        if (info.songs == 0) {
            tvStatus.visibility = View.GONE
            tvFile.text = fileName
            bannerRejected.visibility = if (info.rejected > 0) View.VISIBLE else View.GONE
            tvRejected.text = info.summary
            toast(getString(R.string.err_no_valid))
            return
        }
        var status = resources.getQuantityString(R.plurals.songs, info.songs, info.songs)
        if (info.rejected > 0) {
            status += "  -  " + resources.getQuantityString(
                R.plurals.skipped, info.rejected, info.rejected
            )
        }
        tvStatus.text = status
        tvStatus.visibility = View.VISIBLE
        bannerRejected.visibility = if (info.rejected > 0) View.VISIBLE else View.GONE
        tvRejected.text = info.summary
    }

    // -------------------------------------------------------------- export

    private fun createPdf() {
        if (text.isEmpty()) {
            toast(getString(R.string.err_no_songs))
            return
        }
        val cfg = config()
        btnCreate.isEnabled = false
        btnCreate.alpha = 0.6f

        Thread {
            val rendered = try {
                Engine.render(text, cfg)
            } catch (e: Throwable) {
                null
            }
            var saved: Saved? = null
            if (rendered != null && rendered.ok) {
                saved = try {
                    savePdf(fileNameFor(etTitle.text.toString()), rendered.pdf)
                } catch (e: Exception) {
                    null
                }
            }
            runOnUiThread {
                btnCreate.isEnabled = true
                btnCreate.alpha = 1f
                if (rendered == null) {
                    showResult(getString(R.string.err_write), false)
                } else if (!rendered.ok) {
                    showResult(rendered.error ?: getString(R.string.err_write), false)
                } else if (saved == null) {
                    showResult(getString(R.string.err_write), false)
                } else {
                    shareUri = saved.uri
                    val pages = resources.getQuantityString(R.plurals.pages, rendered.pages, rendered.pages)
                    val songs = resources.getQuantityString(R.plurals.songs, rendered.songs, rendered.songs)
                    showResult(
                        getString(R.string.created) + "  -  " + songs + ", " + pages +
                                "\n" + getString(R.string.saved_to) + " " + saved.location,
                        true
                    )
                    enableShare()
                    openPdf()
                }
            }
        }.start()
    }

    private fun config(): String = buildString {
        appendLine("style=" + styles[style].id)
        appendLine("paper=" + papers[paper].id)
        appendLine("margin=" + margins[margin])
        appendLine("columns=" + (column + 2))
        appendLine("order=auto")
        appendLine("sort=" + sorts[sort].id)
        appendLine("sections=" + sections[section].id)
        appendLine("artist=" + if (swArtist.isChecked) 1 else 0)
        appendLine("bold=" + if (swBold.isChecked) 1 else 0)
        appendLine("page_numbers=" + if (swNumbers.isChecked) 1 else 0)
        appendLine("mirror=" + if (swMirror.isChecked) 1 else 0)
        appendLine("name=" + etTitle.text.toString().trim())
        appendLine("footer_center=" + etFooter.text.toString().trim())
    }

    private fun pdfUri(): Uri = Uri.parse("content://" + packageName + ".pdfs/songbook.pdf")

    /** Filesystem-safe name for the saved PDF. */
    private fun fileNameFor(title: String): String {
        val clean = title.trim().replace(Regex("[\\\\/:*?\"<>|]"), " ")
            .replace(Regex("\\s+"), " ").trim()
        return (clean.ifEmpty { "songbook" }) + ".pdf"
    }

    /**
     * Writes to Downloads/Songbook on Android 10+, so the PDF shows up in any file
     * manager. Older releases fall back to the app cache plus the content provider.
     */
    private fun savePdf(name: String, bytes: ByteArray): Saved {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, name)
                put(MediaStore.Downloads.MIME_TYPE, "application/pdf")
                put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/Songbook")
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            val uri = contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            if (uri != null) {
                contentResolver.openOutputStream(uri)?.use { it.write(bytes) }
                values.clear()
                values.put(MediaStore.Downloads.IS_PENDING, 0)
                contentResolver.update(uri, values, null, null)
                return Saved(uri, "Downloads/Songbook/" + name)
            }
        }
        val file = File(cacheDir, "songbook.pdf")
        FileOutputStream(file).use { it.write(bytes) }
        return Saved(pdfUri(), "app storage/songbook.pdf")
    }

    private fun openPdf() {
        val uri = shareUri ?: return
        val view = Intent(Intent.ACTION_VIEW)
        view.setDataAndType(uri, "application/pdf")
        view.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        try {
            startActivity(view)
        } catch (e: ActivityNotFoundException) {
            toast(getString(R.string.err_no_viewer))
        }
    }

    private fun sharePdf() {
        val uri = shareUri ?: return
        val send = Intent(Intent.ACTION_SEND)
        send.type = "application/pdf"
        send.putExtra(Intent.EXTRA_STREAM, uri)
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        startActivity(Intent.createChooser(send, getString(R.string.share)))
    }

    // --------------------------------------------------------------- chrome

    private fun updateLabels() {
        tvStyle.text = styles[style].label
        tvPaper.text = papers[paper].label
        tvColumns.text = columns[column].label
        tvMargin.text = String.format(java.util.Locale.ROOT, "%.2f in", margins[margin])
        tvSections.text = sections[section].label
        tvSort.text = sorts[sort].label
    }

    /** Rows expose "label: value" so TalkBack reads the current setting. */
    private fun updateAccessibility() {
        rowStyle.contentDescription = getString(R.string.style) + ": " + styles[style].label
        rowPaper.contentDescription = getString(R.string.paper) + ": " + papers[paper].label
        rowColumns.contentDescription = getString(R.string.columns) + ": " + columns[column].label
        rowMargin.contentDescription = getString(R.string.margin) + ": " +
                String.format(java.util.Locale.ROOT, "%.2f in", margins[margin])
        rowSections.contentDescription = getString(R.string.sections) + ": " + sections[section].label
        rowSort.contentDescription = getString(R.string.sort) + ": " + sorts[sort].label
    }

    private fun showResult(message: String, ok: Boolean) {
        tvResult.text = message
        tvResult.visibility = View.VISIBLE
        tvResult.setBackgroundResource(if (ok) R.drawable.bg_banner_ok else R.drawable.bg_banner_error)
        tvResult.setTextColor(getColor(if (ok) R.color.color_ok else R.color.color_danger))
    }

    private fun hideResult() {
        tvResult.visibility = View.GONE
    }

    private fun enableShare() {
        btnShare.isEnabled = true
        btnShare.alpha = 1f
    }

    private fun disableShare() {
        shareUri = null
        btnShare.isEnabled = false
        btnShare.alpha = 0.45f
    }

    private fun toast(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_LONG).show()
    }

    private companion object {
        const val REQ_PICK = 1001
    }
}
