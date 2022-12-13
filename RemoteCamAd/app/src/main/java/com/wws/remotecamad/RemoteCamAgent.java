package com.wws.remotecamad;

public class RemoteCamAgent {
    static {
        System.loadLibrary("remote_cam");
    }
    public native String hello(String str);
}
