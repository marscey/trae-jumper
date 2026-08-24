#!/usr/bin/env python3
"""从各平台的 .sig 文件生成 Tauri updater 的 latest.json。

Tauri v2 的 createUpdaterArtifacts 会为每个平台生成 updater 包和 .sig 签名文件，
但不会自动生成 latest.json。本脚本读取 .sig 文件内容作为签名，结合规范的
GitHub Release 下载 URL，生成符合 Tauri updater 格式的 latest.json。

用法:
  python3 scripts/merge-updater-json.py \
      --version <版本号> \
      --notes <发布说明> \
      --repo <owner/repo> \
      --out <输出路径> \
      <platform>=<asset-name> <path-to-.sig> ...

示例:
  python3 scripts/merge-updater-json.py \
      --version 0.9.10 \
      --notes "TraeJumper 0.9.10 发布" \
      --repo marscey/trae-jumper \
      --out latest.json \
      darwin-aarch64=TraeJumper-0.9.10-mac-arm64.app.tar.gz ./macos-arm64/TraeJumper.app.tar.gz.sig \
      darwin-x86_64=TraeJumper-0.9.10-mac-x64.app.tar.gz ./macos-x64/TraeJumper.app.tar.gz.sig \
      windows-x86_64=TraeJumper-0.9.10-win-x64-setup.exe ./windows-x64/TraeJumper_0.9.10_x64-setup.exe.sig
"""

import argparse
import json
import sys
from datetime import datetime, timezone


def main() -> int:
    parser = argparse.ArgumentParser(description="生成 Tauri updater latest.json")
    parser.add_argument("--version", required=True, help="目标版本号（如 0.9.10）")
    parser.add_argument("--notes", default="", help="发布说明")
    parser.add_argument("--repo", required=True, help="GitHub 仓库（owner/repo）")
    parser.add_argument("--out", required=True, help="输出 latest.json 路径")
    parser.add_argument(
        "inputs",
        nargs="+",
        help='格式为 <platform>=<assetName> <path-to-.sig> 的成对参数',
    )
    args = parser.parse_args()

    pairs: list[tuple[str, str, str]] = []
    i = 0
    while i < len(args.inputs):
        token = args.inputs[i]
        if "=" not in token:
            print(f"[merge-updater-json] 非法参数: {token!r}", file=sys.stderr)
            return 1
        platform, asset = token.split("=", 1)
        if i + 1 >= len(args.inputs):
            print("[merge-updater-json] 缺少 .sig 文件路径", file=sys.stderr)
            return 1
        sig_path = args.inputs[i + 1]
        pairs.append((platform, asset, sig_path))
        i += 2

    platforms = {}
    for platform, asset, sig_path in pairs:
        try:
            with open(sig_path, encoding="utf-8") as f:
                signature = f.read().strip()
        except FileNotFoundError:
            print(f"[merge-updater-json] 找不到 .sig 文件: {sig_path}", file=sys.stderr)
            continue
        if not signature:
            print(f"[merge-updater-json] .sig 文件为空: {sig_path}", file=sys.stderr)
            continue
        url = f"https://github.com/{args.repo}/releases/download/v{args.version}/{asset}"
        platforms[platform] = {
            "signature": signature,
            "url": url,
        }
        print(f"[merge-updater-json] {platform}: {asset} (sig: {len(signature)} chars)")

    if not platforms:
        print("[merge-updater-json] 没有任何可用的平台条目", file=sys.stderr)
        return 1

    output = {
        "version": args.version,
        "notes": args.notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)
    print(f"[merge-updater-json] 已生成 {args.out}，平台: {list(platforms.keys())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
