package efi;

public class FileSystem {
    public static native byte[] readFile(String path);
    public static native String[] listDirectory(String path);
}