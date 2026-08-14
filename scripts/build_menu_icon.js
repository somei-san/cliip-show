// assets/peanut-template.svg の描画結果からテンプレート画像用の PNG を作る。
// build_menu_icon.sh から呼ばれる。引数は [描画済みPNG, 出力PNG]。
ObjC.import('AppKit');
ObjC.import('CoreImage');

const CANVAS = 256;
const SCAN = 128;

function alphaMask(path) {
  // 描画結果は白背景で不透明。輝度を反転して CIMaskToAlpha に通すと、
  // 図形が不透明・背景が透明のマスクになる。
  const src = $.CIImage.imageWithContentsOfURL($.NSURL.fileURLWithPath($(path)));
  const invert = $.CIFilter.filterWithName($('CIColorInvert'));
  invert.setValueForKey(src, $('inputImage'));
  const mask = $.CIFilter.filterWithName($('CIMaskToAlpha'));
  mask.setValueForKey(invert.outputImage, $('inputImage'));

  const rep = $.NSCIImageRep.imageRepWithCIImage(mask.outputImage);
  const img = $.NSImage.alloc.initWithSize(rep.size);
  img.addRepresentation(rep);
  return img;
}

/// 図形の外接矩形を正規化座標（左下原点）で返す。
function boundingBox(mask) {
  const probe = $.NSImage.alloc.initWithSize({ width: SCAN, height: SCAN });
  probe.lockFocus;
  mask.drawInRect({ origin: { x: 0, y: 0 }, size: { width: SCAN, height: SCAN } });
  probe.unlockFocus;

  const rep = $.NSBitmapImageRep.imageRepWithData(probe.TIFFRepresentation);
  const w = rep.pixelsWide;
  const h = rep.pixelsHigh;
  let minX = w;
  let maxX = -1;
  let minY = h;
  let maxY = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (rep.colorAtXY(x, y).alphaComponent > 0.1) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }
  if (maxX < 0) {
    throw new Error('図形が見つかりません（描画に失敗しています）');
  }
  // colorAtXY は左上原点、描画は左下原点なので y を反転する
  return {
    x0: minX / w,
    x1: (maxX + 1) / w,
    y0: 1 - (maxY + 1) / h,
    y1: 1 - minY / h,
  };
}

function run(argv) {
  const src = argv[0];
  const dst = argv[1];
  if (!$.NSFileManager.defaultManager.fileExistsAtPath($(src))) {
    throw new Error('描画結果が見つかりません: ' + src);
  }

  const mask = alphaMask(src);
  const box = boundingBox(mask);
  const cx = (box.x0 + box.x1) / 2;
  const cy = (box.y0 + box.y1) / 2;

  // 大きさは変えず、外接矩形の中心が canvas の中心に来るようずらす
  const dx = (0.5 - cx) * CANVAS;
  const dy = (0.5 - cy) * CANVAS;

  const out = $.NSImage.alloc.initWithSize({ width: CANVAS, height: CANVAS });
  out.lockFocus;
  mask.drawInRect({ origin: { x: dx, y: dy }, size: { width: CANVAS, height: CANVAS } });
  out.unlockFocus;

  const bitmap = $.NSBitmapImageRep.imageRepWithData(out.TIFFRepresentation);
  bitmap.representationUsingTypeProperties($.NSPNGFileType, $()).writeToFileAtomically($(dst), true);

  // 空の素材を書き出す事故を検出できるよう、結果を数値で残す
  let opaque = 0;
  for (let y = 0; y < bitmap.pixelsHigh; y += 4) {
    for (let x = 0; x < bitmap.pixelsWide; x += 4) {
      if (bitmap.colorAtXY(x, y).alphaComponent > 0.5) {
        opaque++;
      }
    }
  }
  const after = boundingBox($.NSImage.alloc.initWithContentsOfFile($(dst)));
  console.log(
    '出力: ' + bitmap.pixelsWide + 'x' + bitmap.pixelsHigh +
    ' 不透明サンプル数=' + opaque +
    ' 中心=(' + ((after.x0 + after.x1) / 2).toFixed(3) + ', ' + ((after.y0 + after.y1) / 2).toFixed(3) + ')'
  );
  if (opaque === 0) {
    throw new Error('不透明ピクセルがありません');
  }
}
