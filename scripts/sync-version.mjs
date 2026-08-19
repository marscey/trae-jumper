#!/usr/bin/env node
/**
 * 版本号同步脚本
 *
 * 权威版本号来源：package.json 的 version 字段
 * 运行本脚本后，会自动将版本号同步到：
 *   1. src-tauri/tauri.conf.json（应用打包版本）
 *   2. src-tauri/Cargo.toml（Rust crate 版本）
 *
 * 用法：
 *   npm run version:sync            # 同步当前 package.json 版本到其他两处
 *   npm run version:set 0.9.9       # 先改 package.json 版本，再同步其他两处
 *
 * 或直接用 npm 标准流程（会自动触发 postversion 钩子同步）：
 *   npm version 0.9.9
 */
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const pkgPath = join(root, "package.json");
const confPath = join(root, "src-tauri", "tauri.conf.json");
const cargoPath = join(root, "src-tauri", "Cargo.toml");

// 允许通过参数直接设置新版本（如 npm run version:set 0.9.9）
const newVersion = process.argv[2];

const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
if (newVersion) {
  if (!/^\d+\.\d+\.\d+/.test(newVersion)) {
    console.error(`[version:sync] 非法版本号: ${newVersion}（应为 x.y.z 格式）`);
    process.exit(1);
  }
  pkg.version = newVersion;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n", "utf-8");
  console.log(`[version:sync] package.json -> ${newVersion}`);
}

const version = pkg.version;

// 同步 tauri.conf.json
const conf = JSON.parse(readFileSync(confPath, "utf-8"));
if (conf.version !== version) {
  conf.version = version;
  writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n", "utf-8");
  console.log(`[version:sync] tauri.conf.json -> ${version}`);
} else {
  console.log(`[version:sync] tauri.conf.json 已是最新 (${version})`);
}

// 同步 Cargo.toml
const cargo = readFileSync(cargoPath, "utf-8");
const updated = cargo.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
if (updated !== cargo) {
  writeFileSync(cargoPath, updated, "utf-8");
  console.log(`[version:sync] Cargo.toml -> ${version}`);
} else {
  console.log(`[version:sync] Cargo.toml 已是最新 (${version})`);
}

console.log(`\n[version:sync] 完成！当前版本：v${version}`);
console.log("前端显示版本号由 Vite 从 package.json 自动注入，无需手动修改。");
