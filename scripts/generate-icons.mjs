// Teamo · 图标生成脚本
// 从 docs/logo.svg 生成 Tauri 需要的全套多分辨率图标
//
// 用法：
//   pnpm icons        （生成所有平台图标到 src-tauri/icons/）
//
// 生成的文件不进版本控制（.gitignore 排除），CI 会在 build 前自动跑

import sharp from "sharp";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const SRC_SVG = path.join(ROOT, "docs/logo.svg");
const OUT_DIR = path.join(ROOT, "src-tauri/icons");

// Tauri 2.x 必需的图标清单
const SIZES = [
  { name: "32x32.png", size: 32 },
  { name: "128x128.png", size: 128 },
  { name: "128x128@2x.png", size: 256 },
  { name: "icon.png", size: 512 },
  // Microsoft Store 系列
  { name: "Square30x30Logo.png", size: 30 },
  { name: "Square44x44Logo.png", size: 44 },
  { name: "Square71x71Logo.png", size: 71 },
  { name: "Square89x89Logo.png", size: 89 },
  { name: "Square107x107Logo.png", size: 107 },
  { name: "Square142x142Logo.png", size: 142 },
  { name: "Square150x150Logo.png", size: 150 },
  { name: "Square284x284Logo.png", size: 284 },
  { name: "Square310x310Logo.png", size: 310 },
  { name: "StoreLogo.png", size: 50 },
];

async function generatePngs(svgBuffer) {
  await mkdir(OUT_DIR, { recursive: true });
  for (const { name, size } of SIZES) {
    await sharp(svgBuffer)
      .resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toFile(path.join(OUT_DIR, name));
    console.log(`✓ ${name} (${size}x${size})`);
  }
}

async function generateIco(svgBuffer) {
  // Windows .ico 多分辨率：16/24/32/48/64/256
  // sharp 不直接支持 ico；用 png-to-ico 库或调 sharp 多次输出 png 后用工具合并
  // 简化：生成一个 256x256 PNG 作为 .ico（Windows 接受单分辨率 PNG-as-ICO）
  // 严谨方案见 issue 卷子，CI 阶段可用 png-to-ico 包
  await sharp(svgBuffer)
    .resize(256, 256, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toFile(path.join(OUT_DIR, "icon.ico"));
  console.log(`✓ icon.ico (256x256, 简化版 — TODO: 多分辨率 ICO 用 png-to-ico)`);
}

async function generateIcns(svgBuffer) {
  // macOS .icns 需要专门的工具（如 iconutil 或 png2icns）
  // sharp 不支持，简化：生成 1024x1024 PNG 作为 .icns 占位
  // 严谨方案：CI 用 macos runner 的 iconutil 或 cargo-bundle 自动生成
  await sharp(svgBuffer)
    .resize(1024, 1024, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toFile(path.join(OUT_DIR, "icon.icns"));
  console.log(`✓ icon.icns (1024x1024, 简化版 — TODO: 真 ICNS 用 iconutil)`);
}

async function main() {
  console.log(`📦 生成 Teamo 图标到 ${OUT_DIR}`);
  const svgBuffer = await readFile(SRC_SVG);
  await generatePngs(svgBuffer);
  await generateIco(svgBuffer);
  await generateIcns(svgBuffer);
  console.log("\n✅ 完成");
}

main().catch((err) => {
  console.error("❌ 图标生成失败:", err);
  process.exit(1);
});
