package local.widnet;

import android.annotation.SuppressLint;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.location.Location;
import android.os.Binder;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;

import androidx.annotation.Nullable;

import com.google.android.gms.location.FusedLocationProviderClient;
import com.google.android.gms.location.LocationCallback;
import com.google.android.gms.location.LocationRequest;
import com.google.android.gms.location.LocationResult;
import com.google.android.gms.location.LocationServices;

class OverwritingLocationCallback extends LocationCallback {
  Location mLocation;
  Object mLock = new Object();

  Location getLocation() {
    synchronized (mLock) {
      return mLocation;
    }
  }

  @Override
  public void onLocationResult(LocationResult locationResult) {
    if (locationResult == null) {
      Log.i("widnet", "Got location update, but it's null.");
      return;
    }
    for (Location location : locationResult.getLocations()) {
      Log.i("widnet", "Got new location from system.");
      synchronized (mLock) {
        mLocation = location;
      }
    }
  }
}

public class LocationService extends Service {
  private static final String CHANNEL_ID = "widnet";
  private final IBinder binder = new LocalBinder();

  public class LocalBinder extends Binder {
    LocationService getService() {
      return LocationService.this;
    }
  }

  public OverwritingLocationCallback mLocationCallback = new OverwritingLocationCallback();

  @Nullable
  @Override
  public IBinder onBind(Intent intent) {
    return binder;
  }

  @SuppressLint("MissingPermission")
  @Override
  public int onStartCommand(Intent intent, int flags, int startId) {
    Log.w("widnet", "LocationService started. Going foreground.");

    final Intent startIntent = new Intent(getApplicationContext(), MainActivity.class);
    startIntent.setAction(Intent.ACTION_MAIN);
    startIntent.addCategory(Intent.CATEGORY_LAUNCHER);
    startIntent.addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT);
    PendingIntent contentIntent =
        PendingIntent.getActivity(
            getApplicationContext(), 1, startIntent, PendingIntent.FLAG_IMMUTABLE);

    NotificationChannel channel =
        new NotificationChannel(CHANNEL_ID, "Widawa", NotificationManager.IMPORTANCE_DEFAULT);

    NotificationManager notificationManager = getSystemService(NotificationManager.class);
    notificationManager.createNotificationChannel(channel);

    Notification notification =
        new Notification.Builder(this, CHANNEL_ID)
            .setContentText("Widnet is still running in the background.")
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(contentIntent)
            .build();

    startForeground(1, notification);

    LocationRequest locationRequest = new LocationRequest();
    locationRequest.setPriority(LocationRequest.PRIORITY_HIGH_ACCURACY);
    locationRequest.setWaitForAccurateLocation(true);
    locationRequest.setInterval(1000);

    Log.i("widnet", "Requesting location updates.");
    FusedLocationProviderClient fusedLocationClient =
        LocationServices.getFusedLocationProviderClient(this);

    fusedLocationClient.requestLocationUpdates(
        locationRequest, mLocationCallback, Looper.getMainLooper());

    Log.i("widnet", "Updates requested.");

    return START_NOT_STICKY;
  }
}
