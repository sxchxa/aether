# Génère src-tauri/icons/icon.ico (16 → 256 px) à partir de la géométrie exacte de app-icon.svg.
# Chaque taille est dessinée en vectoriel avec anticrénelage (pas de réduction d'une grande image),
# ce qui garantit des petites icônes nettes (Gestionnaire des tâches, barre des tâches, Explorateur).
param([string]$Out = "$PSScriptRoot\..\src-tauri\icons\icon.ico")

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing @"
using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;

public static class AetherIcon {
  static PointF P(float x, float y, float k) { return new PointF(x * k, y * k); }

  static PointF[] Pts(float k, float[] xy) {
    var pts = new PointF[xy.Length / 2];
    for (int i = 0; i < pts.Length; i++) pts[i] = P(xy[2 * i], xy[2 * i + 1], k);
    return pts;
  }

  // Polygone rempli d'un dégradé linéaire (de (x1,y1) à (x2,y2), coordonnées 1024).
  static void Grad(Graphics g, float k, Color c1, Color c2, float x1, float y1, float x2, float y2, params float[] xy) {
    using (var b = new LinearGradientBrush(P(x1, y1, k), P(x2, y2, k), c1, c2)) g.FillPolygon(b, Pts(k, xy));
  }

  /// Image d'icône : DIB 32 bits (BGRA + masque AND) sous 256 px, PNG pour 256 px.
  public static byte[] Encode(int size) {
    using (var bmp = Draw(size)) {
      if (size >= 256) {
        using (var ms = new MemoryStream()) { bmp.Save(ms, ImageFormat.Png); return ms.ToArray(); }
      }
      int maskStride = ((size + 31) / 32) * 4;
      using (var ms = new MemoryStream())
      using (var w = new BinaryWriter(ms)) {
        // BITMAPINFOHEADER (hauteur doublée : image XOR + masque AND)
        w.Write(40); w.Write(size); w.Write(size * 2);
        w.Write((ushort)1); w.Write((ushort)32);
        w.Write(0); w.Write(size * size * 4 + maskStride * size);
        w.Write(0); w.Write(0); w.Write(0); w.Write(0);
        for (int y = size - 1; y >= 0; y--)
          for (int x = 0; x < size; x++) {
            var c = bmp.GetPixel(x, y);
            w.Write(c.B); w.Write(c.G); w.Write(c.R); w.Write(c.A);
          }
        for (int y = size - 1; y >= 0; y--) {
          var row = new byte[maskStride];
          for (int x = 0; x < size; x++)
            if (bmp.GetPixel(x, y).A == 0) row[x / 8] |= (byte)(0x80 >> (x % 8));
          w.Write(row);
        }
        return ms.ToArray();
      }
    }
  }

  public static byte[] Render(int size) {
    using (var bmp = Draw(size))
    using (var ms = new MemoryStream()) { bmp.Save(ms, ImageFormat.Png); return ms.ToArray(); }
  }

  static Bitmap Draw(int size) {
    var bmp = new Bitmap(size, size, PixelFormat.Format32bppArgb);
    using (var g = Graphics.FromImage(bmp)) {
      g.SmoothingMode = SmoothingMode.AntiAlias;
      g.PixelOffsetMode = PixelOffsetMode.HighQuality;
      g.Clear(Color.Transparent);
      float k = size / 1024f;
      var bg = Color.FromArgb(255, 0x0C, 0x10, 0x18);
      // Squircle (mêmes courbes que app-icon.svg)
      using (var path = new GraphicsPath()) {
        path.AddBezier(P(512,32,k), P(843,32,k), P(992,181,k), P(992,512,k));
        path.AddBezier(P(992,512,k), P(992,843,k), P(843,992,k), P(512,992,k));
        path.AddBezier(P(512,992,k), P(181,992,k), P(32,843,k), P(32,512,k));
        path.AddBezier(P(32,512,k), P(32,181,k), P(181,32,k), P(512,32,k));
        path.CloseFigure();
        using (var b = new SolidBrush(bg)) g.FillPath(b, path);
      }
      // « A » : trait épais aux extrémités et sommet arrondis (dégradé vertical clair → profond)
      using (var grad = new LinearGradientBrush(P(512,170,k), P(512,880,k), Color.FromArgb(255, 0x6E, 0x9D, 0xEB), Color.FromArgb(255, 0x2F, 0x5F, 0xC4)))
      using (var pen = new Pen(grad, 124 * k)) {
        pen.StartCap = LineCap.Round; pen.EndCap = LineCap.Round; pen.LineJoin = LineJoin.Round;
        g.DrawLines(pen, new[] { P(222,806,k), P(512,236,k), P(802,806,k) });
      }
      // Lame courbe : du pied gauche jusqu'à la pointe centrale
      using (var blade = new GraphicsPath())
      using (var bb = new LinearGradientBrush(P(250,620,k), P(600,860,k), Color.FromArgb(255, 0x3D, 0x6E, 0xD2), Color.FromArgb(255, 0x28, 0x53, 0xB2))) {
        blade.AddBezier(P(206,784,k), P(276,668,k), P(420,612,k), P(572,618,k));
        blade.AddBezier(P(572,618,k), P(626,620,k), P(638,650,k), P(612,670,k));
        blade.AddBezier(P(612,670,k), P(510,716,k), P(384,764,k), P(302,818,k));
        blade.AddBezier(P(302,818,k), P(288,830,k), P(276,841,k), P(266,850,k));
        blade.AddLine(P(266,850,k), P(222,806,k));
        blade.AddLine(P(222,806,k), P(206,784,k));
        blade.CloseFigure();
        g.FillPath(bb, blade);
      }
    }
    return bmp;
  }

  public static void WriteIco(string path, int[] sizes) {
    var images = new byte[sizes.Length][];
    for (int i = 0; i < sizes.Length; i++) images[i] = Encode(sizes[i]);
    using (var fs = new FileStream(path, FileMode.Create))
    using (var w = new BinaryWriter(fs)) {
      w.Write((ushort)0); w.Write((ushort)1); w.Write((ushort)sizes.Length);
      int offset = 6 + 16 * sizes.Length;
      for (int i = 0; i < sizes.Length; i++) {
        int s = sizes[i];
        w.Write((byte)(s >= 256 ? 0 : s)); w.Write((byte)(s >= 256 ? 0 : s));
        w.Write((byte)0); w.Write((byte)0);
        w.Write((ushort)1); w.Write((ushort)32);
        w.Write(images[i].Length); w.Write(offset);
        offset += images[i].Length;
      }
      foreach (var img in images) w.Write(img);
    }
  }
}
"@
[AetherIcon]::WriteIco((Resolve-Path (Split-Path $Out)).Path + "\" + (Split-Path $Out -Leaf), @(16, 20, 24, 32, 40, 48, 64, 128, 256))
"icône écrite : $Out"
