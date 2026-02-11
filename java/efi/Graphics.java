package efi;

public class Graphics {
    public static native int initGraphics();
    public static native int screenWidth();
    public static native int screenHeight();
    public static native int fontWidth();
    public static native int fontHeight();
    public static native void clearScreen(int color);
    public static native void fillRect(int x, int y, int w, int h, int color);
    public static native void drawText(String text, int x, int y, int fgColor, int scale);
    public static native void drawImage(String path, int x, int y);
    public static native int imageWidth(String path);
    public static native int imageHeight(String path);
}