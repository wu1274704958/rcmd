package com.wws.remotecamad;

public class HelloWorld {
    static {
        System.loadLibrary("remote_cam");
    }
    public native String hello(String str);
}
