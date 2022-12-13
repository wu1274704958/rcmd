package com.wws.remotecamad;

import android.content.Context;
import android.util.Log;
import android.widget.Toast;

public class RemoteCamAgent {
    static {
        System.loadLibrary("remote_cam");
    }
    public Context context;
    public void setOnGetError(OnGetError onGetError) {
        this.onGetError = onGetError;
    }
    public RemoteCamAgent(Context context) {
        this.context = context;
    }
    protected OnGetError onGetError;
    public void registe()
    {
        registe_low(context);
    }
    public void unregiste()
    {
        unregiste_low();
        context = null;
    }
    public native void registe_low(Context context);
    public native void unregiste_low();
    public void get_error(int code)
    {
        if(onGetError != null)onGetError.OnError(code);
    }
    public void toast(String str,int op)
    {
        Toast.makeText(context, str, op).show();
    }
}

