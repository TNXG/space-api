#!/usr/bin/env python3
"""用 NVIDIA GPU 批量计算 blurhash，产出 space-api 使用的 JSON。

用法（GPU 版 CuPy，不是 numpy；`uv run` 会自动装好 CUDA 轮子）：

    uv run --with cupy-cuda13x --with pillow --with numpy \
        scripts/blurhash_gpu.py --root images --out images/blurhash.json

输出结构与 `src/data/blurhash.json` 一致：

    { "weight": { "<文件名>": "<blurhash>", ... },
      "height": { "<文件名>": "<blurhash>", ... } }

分组默认：`weight` ← `<root>/wallpaper`，`height` ← `<root>/wallpaper_height`，
可用 `--map height=wallpaper_height` 重复覆盖。

为什么在 GPU 上算
----------------
blurhash 的 DCT 系数可以整张图表示成两次矩阵乘：

    F[j, i] = norm(j, i) / (H * W) * (Ayᵀ · X · Ax)

其中 `Ay[y, j] = cos(pi * j * y / H)`、`Ax[x, i] = cos(pi * i * x / W)`，
X 是 sRGB→linear 之后的像素。用 CuPy 走 cuBLAS，492 张、3715 MP 的壁纸约 30 秒；
逐个分量做 Python 循环的参考实现要几十分钟。

算法与 halcy/blurhash（libjxl 之外的事实参考实现）逐字符一致：sRGB→linear、
DC 用 linear→sRGB 取整、AC 用 166/19 量化、base83 编码；默认 5×4 分量（44 字符）。
编码精度只受 float32 影响，量化边界翻转概率可忽略。
"""

import argparse
import json
import os
import sys

ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~"
IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".webp", ".gif"}


def base83(value: int, length: int) -> str:
    out = []
    for i in range(1, length + 1):
        out.append(ALPHABET[(value // (83 ** (length - i))) % 83])
    return "".join(out)


def linear_to_srgb(value: float) -> int:
    value = min(1.0, max(0.0, value))
    if value <= 0.0031308:
        return int(value * 12.92 * 255 + 0.5)
    return int((1.055 * value ** (1 / 2.4) - 0.055) * 255 + 0.5)


def sign_pow(value: float, exponent: float) -> float:
    return (-1.0 if value < 0 else 1.0) * abs(value) ** exponent


def dct_matrices(cp, height: int, width: int, cx: int, cy: int):
    dtype = cp.float32
    ys = cp.arange(cy, dtype=dtype)
    xs = cp.arange(cx, dtype=dtype)
    ay = cp.cos(cp.pi * cp.outer(cp.arange(height, dtype=dtype), ys) / height)
    ax = cp.cos(cp.pi * cp.outer(cp.arange(width, dtype=dtype), xs) / width)
    return ay, ax


def factors(cp, rgb, cx: int, cy: int):
    """rgb: (H, W, 3) uint8 cupy 数组 -> (cy, cx, 3) float32 numpy 系数。"""
    height, width, _ = rgb.shape
    ay, ax = dct_matrices(cp, height, width, cx, cy)
    v = rgb.astype(cp.float32) / cp.float32(255.0)
    linear = cp.where(
        v <= 0.04045,
        v / cp.float32(12.92),
        ((v + cp.float32(0.055)) / cp.float32(1.055)) ** cp.float32(2.4),
    )
    # 三个通道一起做，右乘 kron(Ax, I3) 让 (x, channel) 保持同通道映射。
    s = ay.T @ linear.reshape(height, width * 3) @ cp.kron(ax, cp.eye(3, dtype=cp.float32))
    s = s.reshape(cy, cx, 3)
    norm = cp.full((cy, cx, 3), 2.0, dtype=cp.float32)
    norm[0, 0, :] = 1.0
    return cp.asnumpy(norm * s / cp.float32(height * width))


def encode(cp, np, rgb, cx: int, cy: int) -> str:
    components = factors(cp, cp.asarray(rgb), cx, cy).reshape(-1, 3)
    dc = components[0]
    dc_value = (
        (linear_to_srgb(float(dc[0])) << 16)
        + (linear_to_srgb(float(dc[1])) << 8)
        + linear_to_srgb(float(dc[2]))
    )
    ac = components[1:]
    peak = float(np.abs(ac).max()) if len(ac) else 0.0
    quant_peak = int(max(0, min(82, int(peak * 166 - 0.5))))
    scale = (quant_peak + 1) / 166.0

    out = base83((cx - 1) + (cy - 1) * 9, 1) + base83(quant_peak, 1) + base83(dc_value, 4)
    for r, g, b in ac:
        quantized = 0
        for channel in (r, g, b):
            value = int(max(0.0, min(18.0, int(sign_pow(float(channel) / scale, 0.5) * 9.0 + 9.5))))
            quantized = quantized * 19 + value
        out += base83(quantized, 2)
    return out


def natural_key(name: str):
    stem, ext = os.path.splitext(name)
    return (0, int(stem), ext) if stem.isdigit() else (1, stem, ext)


def main() -> int:
    parser = argparse.ArgumentParser(description="GPU 批量计算 blurhash")
    parser.add_argument("--root", default="images", help="图片根目录（默认 images）")
    parser.add_argument("--out", default=None, help="输出 JSON（默认 <root>/blurhash.json）")
    parser.add_argument(
        "--map",
        action="append",
        default=None,
        metavar="GROUP=DIR",
        help="分组映射，可重复；默认 weight=wallpaper,height=wallpaper_height",
    )
    parser.add_argument("--components", default="5x4", help="XxY 分量数，默认 5x4")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()

    try:
        import cupy as cp
        import numpy as np
        from PIL import Image
    except ImportError as exc:  # pragma: no cover - 依赖缺失直接给出可执行建议
        print(f"缺少依赖: {exc}\n请用 uv 运行：\n  uv run --with cupy-cuda13x --with pillow --with numpy {sys.argv[0]}", file=sys.stderr)
        return 2

    try:
        device = cp.cuda.runtime.getDeviceProperties(0)["name"].decode()
    except Exception as exc:  # pragma: no cover
        print(f"没有可用的 CUDA 设备: {exc}", file=sys.stderr)
        return 2

    try:
        cx, cy = (int(part) for part in args.components.lower().split("x"))
    except ValueError:
        print("--components 需要形如 5x4", file=sys.stderr)
        return 2

    groups = []
    for item in args.map or ["weight=wallpaper", "height=wallpaper_height"]:
        name, _, sub = item.partition("=")
        groups.append((name, os.path.join(args.root, sub)))

    out_path = args.out or os.path.join(args.root, "blurhash.json")
    result = {}
    total = 0
    for name, directory in groups:
        if not os.path.isdir(directory):
            print(f"目录不存在: {directory}", file=sys.stderr)
            return 1
        entries = {}
        for filename in sorted(os.listdir(directory), key=natural_key):
            if os.path.splitext(filename)[1].lower() not in IMAGE_EXTS:
                continue
            path = os.path.join(directory, filename)
            if not os.path.isfile(path):
                continue
            rgb = np.array(Image.open(path).convert("RGB"), dtype=np.uint8)
            entries[filename] = encode(cp, np, rgb, cx, cy)
            total += 1
            if not args.quiet and total % 50 == 0:
                print(f"  {total} 张…", file=sys.stderr)
        result[name] = entries
        if not args.quiet:
            print(f"{name}: {len(entries)} 条 <- {directory}", file=sys.stderr)

    # 与 src/data/blurhash.json 相同的格式：tab 缩进、文件末尾无换行。
    body = json.dumps(result, ensure_ascii=False, indent="\t")
    with open(out_path, "w", encoding="utf-8") as handle:
        handle.write(body)

    cp.get_default_memory_pool().free_all_blocks()
    if not args.quiet:
        print(f"GPU: {device}\n写入 {out_path}（{total} 条，{cx}x{cy} 分量）", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
