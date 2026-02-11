package efi;

public class BootServices {
    public static native void chainload(String path);
    public static native void chainloadEntry(int index);
    public static native void stall(int milliseconds);
    public static native int discoverEntries();
    public static native String entryName(int index);
    public static native String entryPath(int index);
}