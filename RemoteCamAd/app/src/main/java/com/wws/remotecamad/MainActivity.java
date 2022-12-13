package com.wws.remotecamad;

import androidx.appcompat.app.AppCompatActivity;

import android.os.Bundle;
import android.widget.Toast;

public class MainActivity extends AppCompatActivity {

    private RemoteCamAgent agent;
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
        agent = new RemoteCamAgent(this);
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
}