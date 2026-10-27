package io.github.codeghost.songbook;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.TextUtils;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.Switch;
import android.widget.TextView;
import android.widget.Toast;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/** Single screen: pick a song list, tune the layout, get a printable PDF. */
public final class MainActivity extends Activity {

    private static final int REQ_PICK = 1001;

    /** Callback for the single-choice dialogs (avoids java.util.function on API 23). */
    private interface Pick {
        void on(int which);
    }

    private final List<SongList.Song> songs = new ArrayList<>();
    private final SongList.Report report = new SongList.Report();
    private final Sheet.Options opt = new Sheet.Options();
    private final Handler ui = new Handler(Looper.getMainLooper());

    private String fileName = "";
    private File lastPdf;

    private TextView tvFile;
    private TextView tvStatus;
    private TextView tvRejected;
    private TextView tvResult;
    private TextView tvPaper;
    private TextView tvColumns;
    private TextView tvMargin;
    private TextView tvSections;
    private TextView tvSort;
    private View bannerRejected;
    private View rowPaper;
    private View rowColumns;
    private View rowMargin;
    private View rowSections;
    private View rowSort;
    private EditText etTitle;
    private EditText etFooter;
    private Switch swBold;
    private Switch swNumbers;
    private Switch swMirror;
    private Switch swArtist;
    private Button btnShare;
    private Button btnPick;
    private Button btnCreate;

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        setContentView(R.layout.activity_main);

        tvFile = findViewById(R.id.tvFile);
        tvStatus = findViewById(R.id.tvStatus);
        tvRejected = findViewById(R.id.tvRejected);
        tvResult = findViewById(R.id.tvResult);
        tvPaper = findViewById(R.id.tvPaper);
        tvColumns = findViewById(R.id.tvColumns);
        tvMargin = findViewById(R.id.tvMargin);
        tvSections = findViewById(R.id.tvSections);
        tvSort = findViewById(R.id.tvSort);
        bannerRejected = findViewById(R.id.bannerRejected);
        rowPaper = findViewById(R.id.rowPaper);
        rowColumns = findViewById(R.id.rowColumns);
        rowMargin = findViewById(R.id.rowMargin);
        rowSections = findViewById(R.id.rowSections);
        rowSort = findViewById(R.id.rowSort);
        etTitle = findViewById(R.id.etTitle);
        etFooter = findViewById(R.id.etFooter);
        swBold = findViewById(R.id.swBold);
        swNumbers = findViewById(R.id.swNumbers);
        swMirror = findViewById(R.id.swMirror);
        swArtist = findViewById(R.id.swArtist);
        btnShare = findViewById(R.id.btnShare);
        btnPick = findViewById(R.id.btnPick);
        btnCreate = findViewById(R.id.btnCreate);

        btnPick.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                pickFile();
            }
        });
        btnCreate.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                createPdf(false);
            }
        });
        btnShare.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                sharePdf();
            }
        });

        rowPaper.setOnClickListener(pickPaper());
        rowColumns.setOnClickListener(pickColumns());
        rowMargin.setOnClickListener(pickMargin());
        rowSections.setOnClickListener(pickSections());
        rowSort.setOnClickListener(pickSort());

        updateLabels();
        updateRowsAccessibility();
    }

    // ------------------------------------------------------------ pickers

    private CharSequence[] labels(String[] ids) {
        CharSequence[] out = new CharSequence[ids.length];
        for (int i = 0; i < ids.length; i++) out[i] = ids[i];
        return out;
    }

    private void choose(String title, String[] entries, int selected, final Pick onPick) {
        new AlertDialog.Builder(this)
                .setTitle(title)
                .setSingleChoiceItems(labels(entries), selected, (dialog, which) -> {
                    onPick.on(which);
                    dialog.dismiss();
                    updateLabels();
                    updateRowsAccessibility();
                })
                .setNegativeButton(R.string.cancel, null)
                .show();
    }

    private View.OnClickListener pickPaper() {
        return v -> choose(getString(R.string.choose_paper),
                new String[]{"A4  8.3 x 11.7 in", "Letter  8.5 x 11 in", "Long  8.5 x 13 in", "Legal  8.5 x 14 in"},
                opt.paper.ordinal(), which -> opt.paper = Sheet.Paper.values()[which]);
    }

    private View.OnClickListener pickColumns() {
        return v -> choose(getString(R.string.choose_columns),
                new String[]{"2 columns", "3 columns", "4 columns"},
                opt.columns - 2, which -> opt.columns = which + 2);
    }

    private View.OnClickListener pickMargin() {
        String[] entries = new String[Sheet.MARGINS.length];
        int selected = 0;
        for (int i = 0; i < entries.length; i++) {
            entries[i] = String.format(java.util.Locale.ROOT, "%.2f in", Sheet.MARGINS[i]);
            if (Math.abs(Sheet.MARGINS[i] - opt.margin) < 0.001f) selected = i;
        }
        final int current = selected;
        return v -> choose(getString(R.string.choose_margin), entries, current, which -> opt.margin = Sheet.MARGINS[which]);
    }

    private View.OnClickListener pickSections() {
        return v -> choose(getString(R.string.choose_sections),
                new String[]{Sheet.Sections.CONTINUOUS.label, Sheet.Sections.NEWPAGE.label, Sheet.Sections.NONE.label},
                opt.sections.ordinal(), which -> opt.sections = Sheet.Sections.values()[which]);
    }

    private View.OnClickListener pickSort() {
        return v -> choose(getString(R.string.choose_sort),
                new String[]{Sheet.SortKey.TITLE.label, Sheet.SortKey.ARTIST.label, Sheet.SortKey.NUMBER.label},
                opt.sort.ordinal(), which -> opt.sort = Sheet.SortKey.values()[which]);
    }

    // -------------------------------------------------------------- import

    private void pickFile() {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        try {
            startActivityForResult(intent, REQ_PICK);
        } catch (ActivityNotFoundException e) {
            toast(getString(R.string.err_read));
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQ_PICK || resultCode != RESULT_OK || data == null || data.getData() == null) {
            return;
        }
        load(data.getData());
    }

    private void load(Uri uri) {
        String text;
        try {
            InputStream in = getContentResolver().openInputStream(uri);
            if (in == null) throw new IllegalStateException("stream");
            ByteArrayOutputStream buffer = new ByteArrayOutputStream();
            byte[] chunk = new byte[16 * 1024];
            int n;
            while ((n = in.read(chunk)) > 0) buffer.write(chunk, 0, n);
            in.close();
            byte[] bytes = buffer.toByteArray();
            text = new String(bytes, StandardCharsets.UTF_8);
            if (!text.isEmpty() && text.charAt(0) == '\uFEFF') text = text.substring(1);
        } catch (Exception e) {
            toast(getString(R.string.err_read));
            return;
        }

        songs.clear();
        SongList.Report fresh = new SongList.Report();
        SongList.parse(text, opt.order, songs, fresh);
        copyReport(fresh);

        fileName = displayName(uri);
        tvFile.setText(fileName);
        tvFile.setContentDescription(getString(R.string.pick_file) + ": " + fileName);
        hideResult();

        if (songs.isEmpty()) {
            tvStatus.setVisibility(View.GONE);
            bannerRejected.setVisibility(report.rejected() > 0 ? View.VISIBLE : View.GONE);
            tvRejected.setText(report.summary());
            toast(getString(R.string.err_no_valid));
            return;
        }
        String status = songs.size() + (songs.size() == 1 ? " song" : " songs");
        if (report.rejected() > 0) status += "  -  " + report.rejected() + " skipped";
        tvStatus.setText(status);
        tvStatus.setVisibility(View.VISIBLE);
        if (report.rejected() > 0) {
            bannerRejected.setVisibility(View.VISIBLE);
            tvRejected.setText(report.summary());
        } else {
            bannerRejected.setVisibility(View.GONE);
        }
    }

    private void copyReport(SongList.Report fresh) {
        report.rejects.clear();
        report.rejects.addAll(fresh.rejects);
        report.userContent = fresh.userContent;
    }

    private String displayName(Uri uri) {
        String path = uri.getLastPathSegment();
        if (path == null) return "song list";
        int slash = path.lastIndexOf('/');
        return slash >= 0 ? path.substring(slash + 1) : path;
    }

    // -------------------------------------------------------------- export

    private void createPdf(final boolean thenShare) {
        if (songs.isEmpty()) {
            toast(getString(R.string.err_no_songs));
            return;
        }
        readOptions();
        btnCreate.setEnabled(false);
        btnCreate.setAlpha(0.6f);

        new Thread(() -> {
            try {
                Sheet.Result result = Sheet.render(songs, opt);
                String title = opt.name.isEmpty() ? "Songbook" : opt.name;
                byte[] pdf = result.pdf.toBytes(title);
                File file = new File(getCacheDir(), "songbook.pdf");
                FileOutputStream out = new FileOutputStream(file);
                out.write(pdf);
                out.close();
                final int pages = result.pages;
                ui.post(() -> {
                    lastPdf = file;
                    btnCreate.setEnabled(true);
                    btnCreate.setAlpha(1f);
                    String msg = getString(R.string.created) + "  -  " + songs.size()
                            + " songs, " + pages + (pages == 1 ? " page" : " pages");
                    showResult(msg, true);
                    enableShare();
                    if (thenShare) {
                        sharePdf();
                    } else {
                        openPdf();
                    }
                });
            } catch (final Exception e) {
                ui.post(() -> {
                    btnCreate.setEnabled(true);
                    btnCreate.setAlpha(1f);
                    String detail = e.getMessage() == null ? getString(R.string.err_write) : e.getMessage();
                    showResult(detail, false);
                });
            }
        }, "songbook-export").start();
    }

    private void readOptions() {
        opt.name = etTitle.getText().toString().trim();
        opt.footerCenter = etFooter.getText().toString().trim();
        opt.bold = swBold.isChecked();
        opt.pageNumbers = swNumbers.isChecked();
        opt.mirror = swMirror.isChecked();
        opt.noArtist = !swArtist.isChecked();
    }

    private Uri pdfUri() {
        return Uri.parse("content://" + getPackageName() + ".pdfs/songbook.pdf");
    }

    private void openPdf() {
        if (lastPdf == null) return;
        Intent view = new Intent(Intent.ACTION_VIEW);
        view.setDataAndType(pdfUri(), "application/pdf");
        view.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        try {
            startActivity(view);
        } catch (ActivityNotFoundException e) {
            toast(getString(R.string.err_no_viewer));
        }
    }

    private void sharePdf() {
        if (lastPdf == null) return;
        Intent send = new Intent(Intent.ACTION_SEND);
        send.setType("application/pdf");
        send.putExtra(Intent.EXTRA_STREAM, pdfUri());
        send.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        startActivity(Intent.createChooser(send, getString(R.string.share)));
    }

    // --------------------------------------------------------------- chrome

    private void updateLabels() {
        tvPaper.setText(opt.paper.label);
        tvColumns.setText(opt.columns + " columns");
        tvMargin.setText(String.format(java.util.Locale.ROOT, "%.2f in", opt.margin));
        tvSections.setText(opt.sections.label);
        tvSort.setText(opt.sort.label);
    }

    /** Rows expose "label: value" so TalkBack reads the current setting. */
    private void updateRowsAccessibility() {
        rowPaper.setContentDescription(getString(R.string.paper) + ": " + opt.paper.label);
        rowColumns.setContentDescription(getString(R.string.columns) + ": " + opt.columns);
        rowMargin.setContentDescription(getString(R.string.margin) + ": "
                + String.format(java.util.Locale.ROOT, "%.2f in", opt.margin));
        rowSections.setContentDescription(getString(R.string.sections) + ": " + opt.sections.label);
        rowSort.setContentDescription(getString(R.string.sort) + ": " + opt.sort.label);
    }

    private void showResult(String msg, boolean ok) {
        tvResult.setText(msg);
        tvResult.setVisibility(View.VISIBLE);
        tvResult.setBackgroundResource(ok ? R.drawable.bg_banner_ok : R.drawable.bg_banner_error);
        tvResult.setTextColor(getResources().getColor(ok ? R.color.color_ok : R.color.color_danger, getTheme()));
    }

    private void hideResult() {
        tvResult.setVisibility(View.GONE);
    }

    private void enableShare() {
        btnShare.setEnabled(true);
        btnShare.setAlpha(1f);
    }

    private void toast(String msg) {
        Toast.makeText(this, TextUtils.isEmpty(msg) ? getString(R.string.err_read) : msg, Toast.LENGTH_LONG).show();
    }
}
