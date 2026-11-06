#!/usr/bin/env bash
# 为壁纸图片生成渐进式 JPEG XL 旁路文件：
#   images/wallpaper/150690403_p0.png -> images/wallpaper/150690403_p0.jxl
#
# 为什么用 cjxl 而不是 ffmpeg：ffmpeg 的 libjxl 封装只暴露 effort/distance/modular/xyb，
# 没有任何渐进参数；多 pass 渐进解码只能通过 cjxl -p 生成。
# -j 0 是必须的：JPEG 输入默认 -j 1（无损转码），那条路径不会生成渐进码流。
#
# 少数 JPEG 用了 libjxl 解码器不接受的采样因子（例如 1x2 垂直子采样），
# 此时退回 ImageMagick 解码成 PNG 再交给 cjxl。
#
# 用法: scripts/wallpaper_jxl.sh <目录> [目录...]
# 可调: JXL_QUALITY（默认 90，≈ 视觉无损）、JXL_EFFORT（默认 7）
set -uo pipefail

quality="${JXL_QUALITY:-90}"
effort="${JXL_EFFORT:-7}"
flags=(-j 0 -q "$quality" -e "$effort" -p --group_order=1 --progressive_ac --quiet)

if ! command -v cjxl >/dev/null 2>&1; then
    echo "cjxl (libjxl) 未安装" >&2
    exit 1
fi

if [ "$#" -eq 0 ]; then
    echo "用法: $0 <目录> [目录...]" >&2
    exit 1
fi

failed=0
converted=0
skipped=0

encode() {
    local src="$1" dst="$2"
    local tmp="${dst}.part"
    if cjxl "${flags[@]}" "$src" "$tmp" 2>/dev/null; then
        mv -f "$tmp" "$dst"
        return 0
    fi
    rm -f "$tmp"
    if command -v magick >/dev/null 2>&1; then
        local png
        png="$(mktemp --suffix=.png)"
        if magick "$src" -colorspace sRGB "$png" 2>/dev/null \
            && cjxl "${flags[@]}" "$png" "$tmp" 2>/dev/null; then
            mv -f "$tmp" "$dst"
            rm -f "$png"
            return 0
        fi
        rm -f "$png" "$tmp"
    fi
    return 1
}

for dir in "$@"; do
    [ -d "$dir" ] || { echo "跳过不存在的目录: $dir" >&2; continue; }
    for src in "$dir"/*.jpg "$dir"/*.jpeg "$dir"/*.png "$dir"/*.JPG "$dir"/*.JPEG "$dir"/*.PNG; do
        [ -f "$src" ] || continue
        dst="${src%.*}.jxl"
        # 已经比源文件新就跳过，方便增量补编码
        if [ -f "$dst" ] && [ "$dst" -nt "$src" ]; then
            skipped=$((skipped + 1))
            continue
        fi
        if encode "$src" "$dst"; then
            converted=$((converted + 1))
            printf 'ok   %s -> %s (%s)\n' "$src" "$dst" "$(stat -c%s "$dst")"
        else
            failed=$((failed + 1))
            printf 'FAIL %s\n' "$src" >&2
        fi
    done
done

printf 'converted=%d skipped=%d failed=%d\n' "$converted" "$skipped" "$failed"
[ "$failed" -eq 0 ]
