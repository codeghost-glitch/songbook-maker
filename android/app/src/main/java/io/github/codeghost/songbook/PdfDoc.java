package io.github.codeghost.songbook;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Minimal PDF 1.4 writer: pages, filled/stroked rectangles, Helvetica text and
 * RGB images. Mirrors cli/src/pdf.rs so both apps produce the same pages.
 */
public final class PdfDoc {

    public static final int ALIGN_LEFT = 0;
    public static final int ALIGN_CENTER = 1;
    public static final int ALIGN_RIGHT = 2;

    public static final int BLACK = 0xFF000000;
    public static final int WHITE = 0xFFFFFFFF;

    private static final class Page {
        final float w;
        final float h;
        final StringBuilder ops = new StringBuilder();

        Page(float w, float h) {
            this.w = w;
            this.h = h;
        }
    }

    private static final class Image {
        final String name;
        final int width;
        final int height;
        final byte[] rgb;

        Image(String name, int width, int height, byte[] rgb) {
            this.name = name;
            this.width = width;
            this.height = height;
            this.rgb = rgb;
        }
    }

    private final List<Page> pages = new ArrayList<>();
    private final List<Image> images = new ArrayList<>();

    public int addImage(String name, int width, int height, byte[] rgb) {
        images.add(new Image(name, width, height, rgb));
        return images.size() - 1;
    }

    public void page(float w, float h) {
        pages.add(new Page(w, h));
    }

    public int pageCount() {
        return pages.size();
    }

    private float flip(float y) {
        Page p = pages.get(pages.size() - 1);
        return p.h - y;
    }

    public void fillRect(float x, float y, float w, float h, int argb) {
        Page p = pages.get(pages.size() - 1);
        float fy = p.h - (y + h);
        p.ops.append(num(red(argb))).append(' ').append(num(green(argb))).append(' ')
                .append(num(blue(argb))).append(" rg ")
                .append(num(x)).append(' ').append(num(fy)).append(' ')
                .append(num(w)).append(' ').append(num(h)).append(" re f\n");
    }

    public void strokeRect(float x, float y, float w, float h, int argb, float lineWidth) {
        Page p = pages.get(pages.size() - 1);
        float fy = p.h - (y + h);
        p.ops.append(num(red(argb))).append(' ').append(num(green(argb))).append(' ')
                .append(num(blue(argb))).append(" RG ")
                .append(num(lineWidth)).append(" w ")
                .append(num(x)).append(' ').append(num(fy)).append(' ')
                .append(num(w)).append(' ').append(num(h)).append(" re S\n");
    }

    public void text(String s, float x, float baseline, float size, int argb, boolean bold, int align) {
        if (s == null || s.isEmpty()) {
            return;
        }
        float width = Fonts.width(s, size, bold);
        float tx = x;
        if (align == ALIGN_CENTER) {
            tx = x - width / 2f;
        } else if (align == ALIGN_RIGHT) {
            tx = x - width;
        }
        Page p = pages.get(pages.size() - 1);
        p.ops.append("BT /").append(bold ? "F2" : "F1").append(' ').append(num(size)).append(" Tf ")
                .append(num(red(argb))).append(' ').append(num(green(argb))).append(' ')
                .append(num(blue(argb))).append(" rg 1 0 0 1 ")
                .append(num(tx)).append(' ').append(num(p.h - baseline)).append(" Tm (")
                .append(escape(s)).append(") Tj ET\n");
    }

    public void image(int index, float x, float y, float w, float h) {
        if (index < 0 || index >= images.size()) {
            return;
        }
        Image im = images.get(index);
        Page p = pages.get(pages.size() - 1);
        float fy = p.h - (y + h);
        p.ops.append("q ").append(num(w)).append(" 0 0 ").append(num(h)).append(' ')
                .append(num(x)).append(' ').append(num(fy)).append(" cm /")
                .append(im.name).append(" Do Q\n");
    }

    public byte[] toBytes(String title) {
        List<byte[]> objs = new ArrayList<>();
        int npages = pages.size();
        int nimg = images.size();
        int firstImg = 5;
        int firstPage = firstImg + nimg;
        int firstStream = firstPage + npages;

        objs.add(ascii("<< /Type /Catalog /Pages 2 0 R >>"));

        StringBuilder kids = new StringBuilder();
        for (int i = 0; i < npages; i++) {
            if (i > 0) {
                kids.append(' ');
            }
            kids.append(firstPage + i).append(" 0 R");
        }
        objs.add(ascii("<< /Type /Pages /Count " + npages + " /Kids [" + kids + "] >>"));
        objs.add(ascii("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"));
        objs.add(ascii("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"));

        for (Image im : images) {
            String head = "<< /Type /XObject /Subtype /Image /Width " + im.width + " /Height "
                    + im.height + " /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length "
                    + im.rgb.length + " >>\nstream\n";
            byte[] h = ascii(head);
            byte[] out = new byte[h.length + im.rgb.length + 10];
            System.arraycopy(h, 0, out, 0, h.length);
            System.arraycopy(im.rgb, 0, out, h.length, im.rgb.length);
            byte[] tail = ascii("\nendstream");
            System.arraycopy(tail, 0, out, h.length + im.rgb.length, tail.length);
            objs.add(out);
        }

        StringBuilder xobj = new StringBuilder();
        if (nimg > 0) {
            xobj.append(" /XObject << ");
            for (int i = 0; i < nimg; i++) {
                xobj.append('/').append(images.get(i).name).append(' ').append(firstImg + i).append(" 0 R ");
            }
            xobj.append(">>");
        }

        for (int i = 0; i < npages; i++) {
            Page p = pages.get(i);
            objs.add(ascii("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 " + num(p.w) + " " + num(p.h)
                    + "] /Resources << /Font << /F1 3 0 R /F2 4 0 R >>" + xobj
                    + " >> /Contents " + (firstStream + i) + " 0 R >>"));
        }

        for (Page p : pages) {
            byte[] body = p.ops.toString().getBytes(StandardCharsets.ISO_8859_1);
            byte[] head = ascii("<< /Length " + body.length + " >>\nstream\n");
            byte[] tail = ascii("endstream");
            byte[] out = new byte[head.length + body.length + tail.length];
            System.arraycopy(head, 0, out, 0, head.length);
            System.arraycopy(body, 0, out, head.length, body.length);
            System.arraycopy(tail, 0, out, head.length + body.length, tail.length);
            objs.add(out);
        }

        objs.add(ascii("<< /Title (" + escape(title) + ") /Producer (songbook) /CreationDate (D:20240101000000Z) >>"));

        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] header = ascii("%PDF-1.4\n");
        out.write(header, 0, header.length);
        byte[] binary = {'%', (byte) 0xE2, (byte) 0xE3, (byte) 0xCF, (byte) 0xD3, '\n'};
        out.write(binary, 0, binary.length);
        int[] offsets = new int[objs.size()];
        for (int i = 0; i < objs.size(); i++) {
            offsets[i] = out.size();
            byte[] pre = ascii((i + 1) + " 0 obj\n");
            out.write(pre, 0, pre.length);
            byte[] o = objs.get(i);
            out.write(o, 0, o.length);
            byte[] post = ascii("\nendobj\n");
            out.write(post, 0, post.length);
        }
        int xref = out.size();
        StringBuilder x = new StringBuilder();
        x.append("xref\n0 ").append(objs.size() + 1).append("\n");
        x.append("0000000000 65535 f \n");
        for (int off : offsets) {
            x.append(String.format(Locale.ROOT, "%010d 00000 n \n", off));
        }
        x.append("trailer\n<< /Size ").append(objs.size() + 1).append(" /Root 1 0 R /Info ")
                .append(objs.size()).append(" 0 R >>\nstartxref\n").append(xref).append("\n%%EOF\n");
        byte[] tail = ascii(x.toString());
        out.write(tail, 0, tail.length);
        return out.toByteArray();
    }

    // ---------------------------------------------------------------- helpers

    private static byte[] ascii(String s) {
        return s.getBytes(StandardCharsets.ISO_8859_1);
    }

    static String num(float v) {
        // half-to-even, exactly like Rust's {:.2} so both renderers agree
        String s = new java.math.BigDecimal(Float.toString(v))
                .setScale(2, java.math.RoundingMode.HALF_EVEN).toPlainString();
        while (s.indexOf('.') >= 0 && s.endsWith("0")) {
            s = s.substring(0, s.length() - 1);
        }
        if (s.endsWith(".")) {
            s = s.substring(0, s.length() - 1);
        }
        return s;
    }

    static float red(int argb) {
        return ((argb >> 16) & 0xFF) / 255f;
    }

    static float green(int argb) {
        return ((argb >> 8) & 0xFF) / 255f;
    }

    static float blue(int argb) {
        return (argb & 0xFF) / 255f;
    }

    /** Composite fg over bg with alpha 0..1; PDF opacity is avoided by pre-blending. */
    public static int blend(int fg, int bg, float alpha) {
        float a = Math.max(0f, Math.min(1f, alpha));
        int r = Math.round(red(fg) * a * 255 + red(bg) * (1 - a) * 255);
        int g = Math.round(green(fg) * a * 255 + green(bg) * (1 - a) * 255);
        int b = Math.round(blue(fg) * a * 255 + blue(bg) * (1 - a) * 255);
        return 0xFF000000 | (r << 16) | (g << 8) | b;
    }

    private static String escape(String s) {
        StringBuilder b = new StringBuilder(s.length() + 8);
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            int w = winAnsi(c);
            if (w == '(' || w == ')' || w == '\\') {
                b.append('\\').append((char) w);
            } else if (w >= 32 && w <= 126) {
                b.append((char) w);
            } else {
                b.append('\\').append(String.format(Locale.ROOT, "%03o", w));
            }
        }
        return b.toString();
    }

    private static int winAnsi(char c) {
        if (c < 0x100) {
            return c;
        }
        switch (c) {
            case '\u2018': return 0x91;
            case '\u2019': return 0x92;
            case '\u201c': return 0x93;
            case '\u201d': return 0x94;
            case '\u2022': return 0x95;
            case '\u2013': return 0x96;
            case '\u2014': return 0x97;
            case '\u2026': return 0x85;
            case '\u2122': return 0x99;
            default: return '?';
        }
    }
}
