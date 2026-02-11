package efi;

public class Console {
    public static final int KEY_UP = -1;
    public static final int KEY_DOWN = -2;
    public static final int KEY_ESCAPE = -3;
    public static final int KEY_HOME = -4;
    public static final int KEY_END = -5;
    public static final int KEY_RIGHT = -6;
    public static final int KEY_LEFT = -7;
    public static final int KEY_ENTER = 13;

    public static native void print(String text);
    public static native void println(String text);
    public static native void println();
    public static native int readKey();
}