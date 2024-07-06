package local.walkers;

import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;

import androidx.core.graphics.Insets;
import androidx.core.view.DisplayCutoutCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;

import com.google.androidgamesdk.GameActivity;

public class MainActivity extends GameActivity {
  static {
    System.loadLibrary("main");
  }

  @Override
          protected void onCreate(Bundle savedInstanceState) {

      Log.i("walkers", "setting listener.");

      View content = getWindow().getDecorView().findViewById(android.R.id.content);
    ViewCompat.setOnApplyWindowInsetsListener(content, (v, windowInsets) -> {


        Log.i("walkers", "apply window insets.");
        Insets insets = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
        // Apply the insets as a margin to the view. This solution sets only the
        // bottom, left, and right dimensions, but you can apply whichever insets are
        // appropriate to your layout. You can also update the view padding if that's
        // more appropriate.
        ViewGroup.MarginLayoutParams mlp = (ViewGroup.MarginLayoutParams) v.getLayoutParams();
        mlp.topMargin = 50;// insets.top;
        mlp.leftMargin = insets.left;
        mlp.bottomMargin = insets.bottom;
        mlp.rightMargin = insets.right;
        v.setLayoutParams(mlp);

        // Return CONSUMED if you don't want want the window insets to keep passing
        // down to descendant views.
        return WindowInsetsCompat.CONSUMED;


    //  Insets insets = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
    //  // Apply the insets as a margin to the view. This solution sets only the
    //  // bottom, left, and right dimensions, but you can apply whichever insets are
    //  // appropriate to your layout. You can also update the view padding if that's
    //  // more appropriate.
    //  MarginLayoutParams mlp = (MarginLayoutParams) v.getLayoutParams();
    //  mlp.leftMargin = insets.left;
    //  mlp.bottomMargin = insets.bottom;
    //  mlp.rightMargin = insets.right;
    //  v.setLayoutParams(mlp);

    //  // Return CONSUMED if you don't want want the window insets to keep passing
    //  // down to descendant views.
    //  return WindowInsetsCompat.CONSUMED;
    });

    WindowCompat.setDecorFitsSystemWindows(getWindow(), true);

      super.onCreate(savedInstanceState);
  }


  @Override
  public WindowInsetsCompat onApplyWindowInsets(View v, WindowInsetsCompat windowInsets) {
      Log.i("walkers", "apply window insets.");
      Insets insets = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars());
      // Apply the insets as a margin to the view. This solution sets only the
      // bottom, left, and right dimensions, but you can apply whichever insets are
      // appropriate to your layout. You can also update the view padding if that's
      // more appropriate.
      ViewGroup.MarginLayoutParams mlp = (ViewGroup.MarginLayoutParams) v.getLayoutParams();
      mlp.topMargin = insets.top;
      mlp.leftMargin = insets.left;
      mlp.bottomMargin = insets.bottom;
      mlp.rightMargin = insets.right;
      v.setLayoutParams(mlp);

      // Return CONSUMED if you don't want want the window insets to keep passing
      // down to descendant views.
      return WindowInsetsCompat.CONSUMED;
      }

  //          // Setup cutouts values.
  //          DisplayCutoutCompat dc = insets.getDisplayCutout();
  //          int cutoutTop = 0;
  //          int cutoutRight = 0;
  //          int cutoutBottom = 0;
  //          int cutoutLeft = 0;
  //          if (dc != null) {
  //              cutoutTop = dc.getSafeInsetTop();
  //              cutoutRight = dc.getSafeInsetRight();
  //              cutoutBottom = dc.getSafeInsetBottom();
  //              cutoutLeft = dc.getSafeInsetLeft();
  //          }

  //          // Get display insets.
  //          Insets systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars());

  //          // Setup values to pass into native code.
  //          int[] values = new int[]{0, 0, 0, 0};
  //          values[0] = Utils.pxToDp(Integer.max(cutoutTop, systemBars.top), this);
  //          values[1] = Utils.pxToDp(Integer.max(cutoutRight, systemBars.right), this);
  //          values[2] = Utils.pxToDp(Integer.max(cutoutBottom, systemBars.bottom), this);
  //          values[3] = Utils.pxToDp(Integer.max(cutoutLeft, systemBars.left), this);

  //          // Pass values into native code.
  //          onDisplayInsets(values);

  //          return insets;



  //    return WindowInsetsCompat.CONSUMED;
  //}
}

