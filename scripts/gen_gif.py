#!/usr/bin/env python3
"""cliip-show HUDアニメーションGIF生成スクリプト"""

import subprocess
import os
import tempfile

SVG_PATH = "docs/assets/cliip-show-hud.svg"
OUT_GIF  = "docs/assets/cliip-show-hud.gif"
WIDTH, HEIGHT = 800, 500

# HUD部分の識別マーカー
SPLIT_MARKER = "  <!-- ━━━ HUD overlay ━━━ -->"

with open(SVG_PATH) as f:
    svg = f.read()

before, hud_tail = svg.split(SPLIT_MARKER, 1)
# hud_tail の末尾 "\n</svg>" を除いたHUD要素本体
last_close = hud_tail.rfind("\n</svg>")
hud_content = hud_tail[:last_close]


def make_frame(opacity: float) -> str:
    if opacity <= 0:
        return before + "\n</svg>"
    return (
        before
        + SPLIT_MARKER
        + f'\n  <g opacity="{opacity:.2f}">'
        + hud_content
        + "\n  </g>\n</svg>"
    )


# アニメーション定義: (opacity, フレーム数, delay_cs)
# delay_cs = GIF遅延（センチ秒 = 1/100秒）
steps = [
    (0.0, 8, 10),   # HUDなし 0.8s
    (0.3, 1,  4),   # フェードイン
    (0.6, 1,  4),
    (1.0, 1,  4),
    (1.0, 20, 8),   # 表示中 1.6s
    (0.6, 1,  4),   # フェードアウト
    (0.3, 1,  4),
    (0.0, 1,  4),
    (0.0, 8, 10),   # HUDなし 0.8s (ループ前の間)
]

tmp_dir = tempfile.mkdtemp()
png_files = []
delay_list = []

idx = 0
for opacity, count, delay_cs in steps:
    for _ in range(count):
        svg_file = os.path.join(tmp_dir, f"frame_{idx:03d}.svg")
        png_file = os.path.join(tmp_dir, f"frame_{idx:03d}.png")
        with open(svg_file, "w") as f:
            f.write(make_frame(opacity))
        # qlmanage で macOS ネイティブレンダリング（絵文字対応）
        subprocess.run(
            ["qlmanage", "-t", "-s", str(WIDTH), "-o", tmp_dir, svg_file],
            check=True, capture_output=True,
        )
        raw_png = svg_file + ".png"
        # qlmanage は正方形で出力するので 800x500 にクロップ
        subprocess.run(
            ["magick", raw_png, "-crop", f"{WIDTH}x{HEIGHT}+0+0", "+repage", png_file],
            check=True,
        )
        png_files.append(png_file)
        delay_list.append(delay_cs)
        idx += 1

print(f"{idx} frames rendered")

# ImageMagick でGIF合成
# フレームごとに delay が異なるので個別に指定
cmd = ["magick"]
for delay, png in zip(delay_list, png_files):
    cmd += ["-delay", str(delay), png]
cmd += ["-loop", "0", "-dither", "None", "-colors", "256", "-layers", "optimize", OUT_GIF]

subprocess.run(cmd, check=True)

size_kb = os.path.getsize(OUT_GIF) / 1024
print(f"Done: {OUT_GIF} ({size_kb:.0f} KB)")
