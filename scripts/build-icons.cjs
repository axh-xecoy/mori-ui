// Run with Node.js and sharp installed: node scripts/build-icons.cjs
// SVG is the source of truth; all bitmap sizes are rendered directly from it.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const assets = path.resolve(__dirname, '../assets');

async function main() {
  const source = fs.readFileSync(path.join(assets, 'mori.svg'));
  const sizes = [16, 24, 32, 48, 64, 128, 256, 512, 1024];
  const frames = [];
  for (const size of sizes) {
    const png = await sharp(source, { density: 192 * size / 512 })
      .resize(size, size).png().toBuffer();
    fs.writeFileSync(path.join(assets, `mori-${size}.png`), png);
    if (size <= 256) frames.push({ size, png });
  }
  const header = Buffer.alloc(6 + frames.length * 16);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(frames.length, 4);
  let offset = header.length;
  frames.forEach(({ size, png }, i) => {
    const entry = 6 + i * 16;
    header[entry] = header[entry + 1] = size === 256 ? 0 : size;
    header.writeUInt16LE(1, entry + 4);
    header.writeUInt16LE(32, entry + 6);
    header.writeUInt32LE(png.length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += png.length;
  });
  fs.writeFileSync(path.join(assets, 'mori.ico'), Buffer.concat([header, ...frames.map(f => f.png)]));
  console.log('Mori SVG rendered to 9 PNG sizes and a 7-size Windows ICO.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
