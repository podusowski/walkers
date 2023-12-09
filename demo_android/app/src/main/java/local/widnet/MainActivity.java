package local.walkers;

import android.Manifest;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.location.Location;
import android.os.Build.VERSION;
import android.os.Build.VERSION_CODES;
import android.os.Bundle;
import android.os.IBinder;
import android.util.Log;
import android.view.View;
import android.view.WindowManager;

import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.core.content.ContextCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.view.WindowInsetsControllerCompat;

import com.google.android.gms.location.LocationCallback;
import com.google.android.gms.location.LocationResult;
import com.google.androidgamesdk.GameActivity;

import java.util.Arrays;

public class MainActivity extends GameActivity implements SensorEventListener {

  static {
    System.loadLibrary("main");
  }

  private LocationService mService;

  ServiceConnection connection = new ServiceConnection() {
    public void onServiceConnected(ComponentName className, IBinder service) {
      Log.e("widnet", "Connected to the location service.");
      LocationService.LocalBinder binder = (LocationService.LocalBinder) service;
      mService = binder.getService();
    }

    public void onServiceDisconnected(ComponentName className) {
      Log.e("widnet", "onServiceDisconnected");
    }
  };

  private float[] accelerometerValues = new float[3];
  private float[] magnetometerValues = new float[3];

  private void hideSystemUI() {
    View decorView = getWindow().getDecorView();
    WindowInsetsControllerCompat controller = new WindowInsetsControllerCompat(getWindow(), decorView);
    controller.hide(WindowInsetsCompat.Type.systemBars());
    controller.hide(WindowInsetsCompat.Type.displayCutout());
    controller.setSystemBarsBehavior(
        WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
  }

  private ActivityResultLauncher<String> requestPermissionLauncher;
  private ActivityResultLauncher<String> requestPermissionLauncher2;

  @Override
  protected void onCreate(Bundle savedInstanceState) {
    Log.i("widnet", "Creating.");
    getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
    WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
    hideSystemUI();

    // Need to request permissions one by one.
    // https://stackoverflow.com/questions/66677217/android-requestmultiplepermissions-not-prompting-for-permissions-with-certain-pe#comment117868462_66677217

    requestPermissionLauncher2 = registerForActivityResult(
        new ActivityResultContracts.RequestPermission(),
        accessBackgroundLocationGranted -> {
          if (accessBackgroundLocationGranted) {
            Log.i("widnet", "Access background location granted.");
            startLocationUpdates();
          } else {
            Log.w("widnet", "Access background location denied.");
          }
        });

    requestPermissionLauncher = registerForActivityResult(
        new ActivityResultContracts.RequestPermission(),
        isGranted -> {
          if (isGranted) {
            Log.i("widnet", "Location granted.");
            requestPermissionLauncher2.launch(Manifest.permission.ACCESS_BACKGROUND_LOCATION);
          } else {
            Log.w("widnet", "Location denied.");
          }
        });

    String[] permissions = {
        Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_BACKGROUND_LOCATION
    };

    if (Arrays.stream(permissions)
        .allMatch(
            permission -> ContextCompat.checkSelfPermission(getApplicationContext(),
                permission) == PackageManager.PERMISSION_GRANTED)) {
      Log.i("widnet", "All permissions already granted.");
      startLocationUpdates();
    } else {
      Log.i("widnet", "Requesting permissions.");
      requestPermissionLauncher.launch(Manifest.permission.ACCESS_FINE_LOCATION);
    }

    super.onCreate(savedInstanceState);
  }

  @Override
  protected void onPause() {
    Log.i("widnet", "Pausing.");
    super.onPause();
  }

  @Override
  protected void onStop() {
    Log.i("widnet", "Stopping.");
    super.onStop();
  }

  @Override
  protected void onStopNative(long l) {
    Log.i("widnet", "Stop native.");
    super.onStopNative(l);
  }

  private Intent mLocationServiceIntent;

  @Override
  protected void onDestroy() {
    Log.i("widnet", "Destroying.");
    unbindService(connection);
    stopService(mLocationServiceIntent);

    super.onDestroy();
  }

  public void exit() {
    Log.i("widnet", "Gracefully exiting.");
    unbindService(connection);
    stopService(mLocationServiceIntent);
    finish();
  }

  private void startLocationUpdates() {
    Log.i("widnet", "Starting LocationService.");
    mLocationServiceIntent = new Intent(this, LocationService.class);
    bindService(mLocationServiceIntent, connection, Context.BIND_AUTO_CREATE);
    startForegroundService(mLocationServiceIntent);

    // Start compass.
    SensorManager sensorManager = (SensorManager) getSystemService(Context.SENSOR_SERVICE);
    Sensor magnetometer = sensorManager.getDefaultSensor(Sensor.TYPE_MAGNETIC_FIELD);
    Sensor accelerometer = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
    sensorManager.registerListener(this, magnetometer, SensorManager.SENSOR_DELAY_NORMAL);
    sensorManager.registerListener(this, accelerometer, SensorManager.SENSOR_DELAY_NORMAL);
  }

  public Location getLocation() {
    if (mService == null) {
      return null;
    }
    return mService.mLocationCallback.mLocation;
  }

  public String getClipboardText() {
    ClipboardManager clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);

    if (!clipboard.hasPrimaryClip()) {
      return null;
    }

    try {
      return clipboard.getPrimaryClip().getItemAt(0).getText().toString();
    } catch (Exception e) {
      return null;
    }
  }

  public void setClipboardText(String value) {
    ClipboardManager clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);

    try {
      clipboard.setPrimaryClip(ClipData.newPlainText(value, value));
    } catch (Exception e) {
      Log.e("widnet", "Could not put text into a clipboard: " + e.toString() + ".");
    }
  }

  public void storeValue(String key, String value) {
    Log.i("widnet", "Storing " + key + "=" + value + ".");
    SharedPreferences sharedPref = getPreferences(Context.MODE_PRIVATE);
    SharedPreferences.Editor editor = sharedPref.edit();
    editor.putString(key, value);
    editor.apply();
  }

  public String loadValue(String key) {
    SharedPreferences sharedPref = getPreferences(Context.MODE_PRIVATE);
    String value = sharedPref.getString(key, null);
    Log.i("widnet", "Getting " + key + "=" + value + ".");
    return value;
  }

  public float getAzimuth() {
    float[] rotationMatrix = new float[9];
    float[] orientationValues = new float[3];
    SensorManager.getRotationMatrix(rotationMatrix, null, accelerometerValues, magnetometerValues);
    SensorManager.getOrientation(rotationMatrix, orientationValues);
    return orientationValues[0];
  }

  @Override
  public void onSensorChanged(SensorEvent event) {
    if (event.sensor.getType() == Sensor.TYPE_ACCELEROMETER) {
      System.arraycopy(event.values, 0, accelerometerValues, 0, 3);
    } else if (event.sensor.getType() == Sensor.TYPE_MAGNETIC_FIELD) {
      System.arraycopy(event.values, 0, magnetometerValues, 0, 3);
    }
  }

  @Override
  public void onAccuracyChanged(Sensor sensor, int i) {

  }
}
