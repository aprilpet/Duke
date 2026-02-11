import efi.Console;
import efi.BootServices;
import efi.Graphics;

public class BootMenu {
    static final int BG = 0x0F0F12;
    static final int SURFACE = 0x17171C;
    static final int CARD = 0x212126;
    static final int BORDER = 0x2E2E36;
    static final int TEXT = 0xE0E0E6;
    static final int TEXT_DIM = 0x7A7A8A;
    static final int TEXT_DK = 0x4D4D59;
    static final int ACCENT = 0x8C8FA6;

    public static void main(String[] args) {
        int count = BootServices.discoverEntries();

        if (count == 0) {
            Console.println("No bootable entries found.");
            return;
        }

        int gfx = Graphics.initGraphics();
        if (gfx == 0) {
            textFallback(count);
            return;
        }

        int sw = Graphics.screenWidth();
        int sh = Graphics.screenHeight();
        int fw = Graphics.fontWidth();
        int fh = Graphics.fontHeight();

        Graphics.clearScreen(BG);

        int pad = 40;

        int titleScale = 2;
        int titleY = pad;
        Graphics.drawText("Duke", pad, titleY, TEXT, titleScale);

        int sepY = titleY + fh * titleScale + 12;
        Graphics.fillRect(pad, sepY, sw / 3, 1, BORDER);

        int itemH = fh + 10;
        int menuY = sepY + 16;

        int selected = 0;
        drawMenu(count, selected, pad, menuY, sw - pad * 2, itemH, fw, fh);

        int footerY = sh - pad;
        Graphics.drawText("Up/Down  Select    Enter  Boot", pad, footerY, TEXT_DK, 1);

        while (true) {
            int key = Console.readKey();
            if (key == Console.KEY_UP && selected > 0) {
                selected = selected - 1;
                drawMenu(count, selected, pad, menuY, sw - pad * 2, itemH, fw, fh);
            } else if (key == Console.KEY_DOWN && selected < count - 1) {
                selected = selected + 1;
                drawMenu(count, selected, pad, menuY, sw - pad * 2, itemH, fw, fh);
            } else if (key == Console.KEY_ENTER) {
                Graphics.clearScreen(0x000000);
                BootServices.chainloadEntry(selected);
                break;
            }
        }
    }

    static void drawMenu(int count, int selected, int x, int y, int w, int itemH, int fw, int fh) {
        for (int i = 0; i < count; i++) {
            int iy = y + i * itemH;
            String name = BootServices.entryName(i);

            if (i == selected) {
                Graphics.fillRect(x, iy, w, itemH - 2, CARD);
                Graphics.fillRect(x, iy, 2, itemH - 2, ACCENT);
                Graphics.drawText(name, x + 12, iy + 4, TEXT, 1);
            } else {
                Graphics.fillRect(x, iy, w, itemH - 2, BG);
                Graphics.drawText(name, x + 12, iy + 4, TEXT_DIM, 1);
            }
        }
    }

    static void textFallback(int count) {
        Console.println("Duke");
        Console.println("");

        for (int i = 0; i < count; i++) {
            Console.print("  ");
            Console.print(String.valueOf(i + 1));
            Console.print(". ");
            Console.println(BootServices.entryName(i));
        }

        Console.println("");
        Console.print("Select> ");

        while (true) {
            int key = Console.readKey();
            int choice = key - 49;
            if (choice >= 0 && choice < count) {
                Console.println("");
                BootServices.chainloadEntry(choice);
                break;
            }
        }
    }
}