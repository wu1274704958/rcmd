package com.wws.remotecamad;

import android.content.Context;
import android.os.Message;
import android.util.Log;
import android.widget.Toast;
import android.os.Handler;

public class RemoteCamAgent {
    static {
        System.loadLibrary("remote_cam");
    }
    public Context context;
    private long mainThreadId;
    private Handler handler;
    public void setOnGetError(OnGetError onGetError) {
        this.onGetError = onGetError;
    }
    public RemoteCamAgent(Context context, Handler myHandle) {
        this.context = context;
        this.handler = myHandle;
    }
    protected OnGetError onGetError;
    public void registe()
    {
        mainThreadId = Thread.currentThread().getId();
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
        RunUIThread(new Runnable() {
            @Override
            public void run() {
                ((MainActivity)context).log(str);
                //Toast.makeText(context, str, op).show();
            }
        });
    }
    public void RunUIThread(Runnable r)
    {
        if(mainThreadId == Thread.currentThread().getId())
            r.run();
        else
        {
            Message msg = new Message();
            msg.arg1 = 1;
            msg.obj = r;
            handler.sendMessage(msg);
        }
    }
}

