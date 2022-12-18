package com.wws.remotecamad;

import android.content.Context;
import android.net.ConnectivityManager;
import android.net.NetworkInfo;
import android.net.wifi.WifiInfo;
import android.net.wifi.WifiManager;
import android.os.Message;
import android.util.Log;
import android.widget.Toast;
import android.os.Handler;

import java.net.Inet4Address;
import java.net.InetAddress;
import java.net.NetworkInterface;
import java.net.SocketException;
import java.util.Enumeration;

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
    public native void launch(String args);
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
    public String getIPAddress() {
        NetworkInfo info = ((ConnectivityManager) context.getSystemService(Context.CONNECTIVITY_SERVICE)).getActiveNetworkInfo();
        if (info != null && info.isConnected()) {
            if (info.getType() == ConnectivityManager.TYPE_MOBILE) {//当前使用2G/3G/4G网络
                try {
                    //Enumeration<NetworkInterface> en=NetworkInterface.getNetworkInterfaces();
                    for (Enumeration<NetworkInterface> en = NetworkInterface.getNetworkInterfaces(); en.hasMoreElements(); ) {
                        NetworkInterface intf = en.nextElement();
                        for (Enumeration<InetAddress> enumIpAddr = intf.getInetAddresses(); enumIpAddr.hasMoreElements(); ) {
                            InetAddress inetAddress = enumIpAddr.nextElement();
                            if (!inetAddress.isLoopbackAddress() && inetAddress instanceof Inet4Address) {
                                return inetAddress.getHostAddress();
                            }
                        }
                    }
                } catch (SocketException e) {
                    e.printStackTrace();
                }

            } else if (info.getType() == ConnectivityManager.TYPE_WIFI) {//当前使用无线网络
                WifiManager wifiManager = (WifiManager) context.getSystemService(Context.WIFI_SERVICE);
                WifiInfo wifiInfo = wifiManager.getConnectionInfo();
                String ipAddress = intIP2StringIP(wifiInfo.getIpAddress());//得到IPV4地址
                return ipAddress;
            }
        } else {
            //当前无网络连接,请在设置中打开网络
        }
        return null;
    }


    public static String intIP2StringIP(int ip) {
        return (ip & 0xFF) + "." +
                ((ip >> 8) & 0xFF) + "." +
                ((ip >> 16) & 0xFF) + "." +
                (ip >> 24 & 0xFF);
    }
}

