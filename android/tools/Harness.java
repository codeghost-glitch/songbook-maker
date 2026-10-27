import io.github.codeghost.songbook.PdfDoc;
import io.github.codeghost.songbook.Sheet;
import io.github.codeghost.songbook.SongList;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

/** Desktop smoke test for the android sources that do not need the framework. */
public final class Harness {
    public static void main(String[] args) throws Exception {
        String text = new String(Files.readAllBytes(Paths.get(args[0])), StandardCharsets.UTF_8);
        List<SongList.Song> songs = new ArrayList<>();
        SongList.Report rep = new SongList.Report();
        SongList.parse(text, SongList.Order.parse(args[1]), songs, rep);

        Sheet.Options o = new Sheet.Options();
        o.name = args[2];
        if (args.length > 4) o.columns = Integer.parseInt(args[4]);
        if (args.length > 5) o.mirror = Boolean.parseBoolean(args[5]);
        if (args.length > 6) o.footerCenter = args[6];
        if (args.length > 7) o.bold = Boolean.parseBoolean(args[7]);

        Sheet.Result r = Sheet.render(songs, o);
        Files.write(Paths.get(args[3]), r.pdf.toBytes(o.name));
        System.out.println("songs=" + songs.size() + " rejected=" + rep.rejected()
                + " pages=" + r.pages + " " + r.pageW + "x" + r.pageH + "pt"
                + " cells=" + r.cells[0] + "/" + r.cells[1] + "/" + r.cells[2]);
        if (rep.rejected() > 0) System.out.println(rep.summary());
    }
}
