// assets/peanut-template.png から .app 用のアイコン画像を作る。
// build_app_icon.sh から呼ばれる。引数は [出力PNG]。
ObjC.import('AppKit');

// Apple のアイコングリッドに合わせる。1024 の canvas に対し角丸矩形は 824 角・半径 185
const CANVAS = 1024;
const PLATE = 824;
const RADIUS = 185;
// 角丸矩形の中でピーナッツを描く正方形。素材の余白ぶん、見た目はこれより一回り小さくなる
const GLYPH = 620;

const GLYPH_SOURCE = 'assets/peanut-template.png';

/// 指定ピクセル数の透明なビットマップを作る。lockFocus と違い、実行マシンの
/// backing scale factor に依存せず解像度が決まる。
function makeBitmap(size) {
  const bitmap = $.NSBitmapImageRep.alloc
    .initWithBitmapDataPlanesPixelsWidePixelsHighBitsPerSampleSamplesPerPixelHasAlphaIsPlanarColorSpaceNameBytesPerRowBitsPerPixel(
      $(), size, size, 8, 4, true, false, $.NSCalibratedRGBColorSpace, 0, 0);
  if (!bitmap.js) {
    throw new Error('ビットマップを確保できません: ' + size + 'x' + size);
  }
  return bitmap;
}

function drawInto(bitmap, body) {
  const context = $.NSGraphicsContext.graphicsContextWithBitmapImageRep(bitmap);
  if (!context.js) {
    throw new Error('描画コンテキストを作れません');
  }
  $.NSGraphicsContext.saveGraphicsState;
  $.NSGraphicsContext.setCurrentContext(context);
  body();
  $.NSGraphicsContext.restoreGraphicsState;
}

function rect(x, y, w, h) {
  return { origin: { x: x, y: y }, size: { width: w, height: h } };
}

/// 素材はアルファだけを持つマスクなので、白で塗り直してから角丸矩形に載せる。
function whiteGlyph() {
  const mask = $.NSImage.alloc.initWithContentsOfFile($(GLYPH_SOURCE));
  if (!mask.isValid) {
    throw new Error('素材を読み込めません: ' + GLYPH_SOURCE);
  }
  const bitmap = makeBitmap(GLYPH);
  drawInto(bitmap, function () {
    const area = rect(0, 0, GLYPH, GLYPH);
    mask.drawInRect(area);
    // NSCompositingOperationSourceAtop。JXA には定数が来ないので生値で指定する
    $.NSGraphicsContext.currentContext.compositingOperation = 5;
    $.NSColor.whiteColor.set;
    $.NSBezierPath.fillRect(area);
  });
  const image = $.NSImage.alloc.initWithSize({ width: GLYPH, height: GLYPH });
  image.addRepresentation(bitmap);
  return image;
}

function run(argv) {
  const dst = argv[0];
  const glyph = whiteGlyph();

  const canvas = makeBitmap(CANVAS);
  drawInto(canvas, function () {
    const inset = (CANVAS - PLATE) / 2;
    const plate = $.NSBezierPath.bezierPathWithRoundedRectXRadiusYRadius(
      rect(inset, inset, PLATE, PLATE), RADIUS, RADIUS);
    // HUD の既定の背景と同じ暗色系にして、アイコンと表示中の見た目を揃える
    const gradient = $.NSGradient.alloc.initWithStartingColorEndingColor(
      $.NSColor.colorWithSRGBRedGreenBlueAlpha(0.13, 0.13, 0.14, 1.0),
      $.NSColor.colorWithSRGBRedGreenBlueAlpha(0.29, 0.29, 0.31, 1.0));
    gradient.drawInBezierPathAngle(plate, 90);

    const offset = (CANVAS - GLYPH) / 2;
    glyph.drawInRect(rect(offset, offset, GLYPH, GLYPH));
  });

  const png = canvas.representationUsingTypeProperties($.NSPNGFileType, $());
  if (!png.writeToFileAtomically($(dst), true)) {
    throw new Error('書き出しに失敗しました: ' + dst);
  }

  // 真っ黒・真っ白な板だけを書き出す事故を検出できるよう、結果を数値で残す
  let white = 0;
  let total = 0;
  for (let y = 0; y < CANVAS; y += 8) {
    for (let x = 0; x < CANVAS; x += 8) {
      const color = canvas.colorAtXY(x, y);
      if (color.alphaComponent > 0.5) {
        total++;
        if (color.brightnessComponent > 0.8) {
          white++;
        }
      }
    }
  }
  console.log('出力: ' + CANVAS + 'x' + CANVAS + ' 不透明サンプル数=' + total + ' うち白=' + white);
  if (white === 0 || white === total) {
    throw new Error('ピーナッツと背景を判別できません（描画に失敗しています）');
  }
}
