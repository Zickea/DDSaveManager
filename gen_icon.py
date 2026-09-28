# -*- coding: utf-8 -*-
"""生成 DDSaveManager 应用图标：深色背景 + 金色边框 + 暗红地牢拱门。"""
from PIL import Image, ImageDraw

SIZE = 1024

img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

# 背景圆角矩形（暗色）
d.rounded_rectangle([24, 24, SIZE - 24, SIZE - 24], radius=180, fill=(23, 19, 16, 255))

# 金色外框
d.rounded_rectangle([70, 70, SIZE - 70, SIZE - 70], radius=140, outline=(201, 168, 106, 255), width=28)

# 中央地牢拱门（暗红 + 深色门洞）
# 门柱
d.rectangle([300, 330, 400, 780], fill=(163, 59, 46, 255))
d.rectangle([624, 330, 724, 780], fill=(163, 59, 46, 255))
# 拱顶（半圆）
d.pieslice([300, 180, 724, 604], 180, 360, fill=(163, 59, 46, 255))
# 门洞（深色）
d.rectangle([430, 420, 594, 780], fill=(23, 19, 16, 255))
d.pieslice([430, 300, 594, 534], 180, 360, fill=(23, 19, 16, 255))
# 门内小门（暗红窄条）
d.rectangle([462, 430, 520, 780], fill=(70, 28, 22, 255))
d.pieslice([462, 330, 520, 468], 180, 360, fill=(70, 28, 22, 255))

# 顶部火焰/封印点（金色）
d.ellipse([482, 170, 542, 230], fill=(201, 168, 106, 255))

img.save(r"C:\Users\60143\DDSL\src-tauri\icons\icon.png")
# 多尺寸 ico
ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
img.resize((256, 256), Image.LANCZOS).save(
    r"C:\Users\60143\DDSL\src-tauri\icons\icon.ico", sizes=ico_sizes
)
print("icons generated")
