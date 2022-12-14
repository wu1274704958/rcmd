package com.wws.remotecamad;

import androidx.annotation.NonNull;
import androidx.appcompat.app.AppCompatActivity;

import android.os.Bundle;
import android.os.Handler;
import android.os.Message;
import android.widget.Toast;

import java.lang.ref.WeakReference;

public class MainActivity extends AppCompatActivity {

    private RemoteCamAgent agent;
    private MyHandle myHandle;
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
        myHandle = new MyHandle(this);
        agent = new RemoteCamAgent(this,myHandle);
        agent.setOnGetError(new OnGetError() {
            @Override
            public void OnError(int code) {
                Toast.makeText(MainActivity.this, "error:"+code, Toast.LENGTH_LONG).show();
            }
        });
        agent.registe();
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        agent.unregiste();
        agent = null;
    }

    private static class MyHandle extends Handler {
        WeakReference<MainActivity> mainActivityWeakReference;
        public MyHandle(MainActivity activity)
        {
            mainActivityWeakReference = new WeakReference<>(activity);
        }
        @Override
        public void handleMessage(@NonNull Message msg) {
            switch (msg.arg1)
            {
                case 1:
                    Runnable r = (Runnable) msg.obj;
                    if(r != null)
                        r.run();
                    break;
            }
        }
    }
}