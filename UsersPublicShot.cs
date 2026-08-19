using System; using System.Drawing; using System.Windows.Forms;
class P {
  static void Main(string[] a) {
    try {
      string p = a.Length>0 ? a[0] : @"C:UsersPublics.png";
      Screen s = Screen.PrimaryScreen;
      using (Bitmap b = new Bitmap(s.Bounds.Width, s.Bounds.Height)) {
        using (Graphics g = Graphics.FromImage(b)) {
          g.CopyFromScreen(s.Bounds.X, s.Bounds.Y, 0, 0, s.Bounds.Size);
        }
        b.Save(p, System.Drawing.Imaging.ImageFormat.Png);
        Console.WriteLine("OK:" + s.Bounds.Width + "x" + s.Bounds.Height + "->" + p);
      }
    } catch (Exception e) { Console.Error.WriteLine(e.Message); Environment.Exit(1); }
  }
}
