package com.wws.remotecamad;

import androidx.annotation.NonNull;
import androidx.appcompat.app.AppCompatActivity;

import android.os.Bundle;
import android.os.Handler;
import android.os.Message;
import android.view.View;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;

import java.lang.ref.WeakReference;

public class MainActivity extends AppCompatActivity {

    private RemoteCamAgent agent;
    private MyHandle myHandle;
    private ScrollView m_Scroll;
    private TextView m_logTx;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
        init();
        myHandle = new MyHandle(this);
        agent = new RemoteCamAgent(this,myHandle);
        agent.setOnGetError(new OnGetError() {
            @Override
            public void OnError(int code) {
                agent.RunUIThread(new Runnable() {
                    @Override
                    public void run() {
                        log("error:"+code);
                    }
                });
            }
        });
        agent.registe();
    }

    private void init() {
        m_Scroll = (ScrollView)findViewById(R.id.scroll);
        m_logTx = (TextView)findViewById(R.id.log_tx);
    }
    public void log(String str)
    {
        if(m_logTx.getLineCount() >= m_logTx.getMaxLines())
            m_logTx.setText("");
        if(str.length() > 0 && str.charAt(str.length() - 1) != '\n')
            str += '\n';
        m_logTx.append(str);
        m_Scroll.fullScroll(View.FOCUS_DOWN);
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