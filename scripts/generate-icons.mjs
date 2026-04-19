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
const TRAY_SVG = path.join(ROOT, "docs/logo-tray.svg");
const OUT_DIR = path.join(ROOT, "src-tauri/icons");

// Tray 专用单色图标（融入系统任务栏，区别于彩色主 logo）
const TRAY_SIZES = [
  { name: "tray-icon.png", size: 32 },
  { name: "tray-icon@2x.png", size: 64 },
];

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
  // Windows 真多分辨率 ICO（PNG-in-ICO 格式，Vista+ 支持）
  // 结构：ICONDIR header(6B) + N 个 entry(16B each) + N 段 PNG 数据
  // 不能直接把 PNG 改扩展名——Tauri/Windows 的 ICO 解析器会读 ICONDIR.reserved，
  // PNG 魔数 0x8950 会被误读成 reserved=20617 导致 "Invalid reserved field" 编译失败
  const sizes = [16, 24, 32, 48, 64, 128, 256];

  const pngBuffers = await Promise.all(
    sizes.map((size) =>
      sharp(svgBuffer)
        .resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
        .png()
        .toBuffer(),
    ),
  );

  const HEADER_SIZE = 6;
  const ENTRY_SIZE = 16;
  const dirTotal = HEADER_SIZE + ENTRY_SIZE * sizes.length;

  const header = Buffer.alloc(HEADER_SIZE);
  header.writeUInt16LE(0, 0);             // reserved = 0
  header.writeUInt16LE(1, 2);             // type = 1 (icon)
  header.writeUInt16LE(sizes.length, 4);  // image count

  const entries = Buffer.alloc(ENTRY_SIZE * sizes.length);
  let dataOffset = dirTotal;
  for (let i = 0; i < sizes.length; i++) {
    const off = i * ENTRY_SIZE;
    const size = sizes[i];
    const pngSize = pngBuffers[i].length;
    // width/height：0 表示 256（标准约定）
    entries.writeUInt8(size === 256 ? 0 : size, off + 0);
    entries.writeUInt8(size === 256 ? 0 : size, off + 1);
    entries.writeUInt8(0, off + 2);          // color palette count (0 = no palette)
    entries.writeUInt8(0, off + 3);          // reserved
    entries.writeUInt16LE(1, off + 4);       // color planes
    entries.writeUInt16LE(32, off + 6);      // bits per pixel
    entries.writeUInt32LE(pngSize, off + 8); // image data size
    entries.writeUInt32LE(dataOffset, off + 12); // offset to data
    dataOffset += pngSize;
  }

  const icoBuffer = Buffer.concat([header, entries, ...pngBuffers]);
  await writeFile(path.join(OUT_DIR, "icon.ico"), icoBuffer);
  console.log(`✓ icon.ico (${sizes.length} 尺寸: ${sizes.join("/")}, 真 ICO)`);
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

async function generateTrayIcons() {
  const svgBuffer = await readFile(TRAY_SVG);
  for (const { name, size } of TRAY_SIZES) {
    await sharp(svgBuffer)
      .resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toFile(path.join(OUT_DIR, name));
    console.log(`✓ ${name} (${size}x${size}) [tray]`);
  }
}

async function main() {
  console.log(`📦 生成 Teamo 图标到 ${OUT_DIR}`);
  const svgBuffer = await readFile(SRC_SVG);
  await generatePngs(svgBuffer);
  await generateIco(svgBuffer);
  await generateIcns(svgBuffer);
  await generateTrayIcons();
  console.log("\n✅ 完成");
}

main().catch((err) => {
  console.error("❌ 图标生成失败:", err);
  process.exit(1);
});
