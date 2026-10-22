// 版本号同步：把给定版本写入 package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml。
// CI 在 tag 触发出包时调用（版本跟随 tag），也可在本地发版前手动执行。
// 用法：node scripts/sync-version.mjs 0.2.0
import { readFileSync, writeFileSync } from 'node:fs'

const version = process.argv[2]
if (!version || !/^\d+\.\d+\.\d+([-+.][0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`用法: node scripts/sync-version.mjs <x.y.z>，收到: ${version ?? '(缺参)'}`)
  process.exit(1)
}

const editJson = (path) => {
  const json = JSON.parse(readFileSync(path, 'utf8'))
  json.version = version
  // 保留原缩进与结尾换行，避免无意义 diff
  writeFileSync(path, `${JSON.stringify(json, null, 2)}\n`)
  console.log(`${path} -> ${version}`)
}
editJson('package.json')
editJson('src-tauri/tauri.conf.json')

// Cargo.toml 依赖的 version 都带缩进或内联，行首 ^version 只命中 [package] 版本行
const cargoPath = 'src-tauri/Cargo.toml'
const cargo = readFileSync(cargoPath, 'utf8')
const versionLine = /^version\s*=\s*"[^"]*"/m
// 先探测再替换：同版本号替换后内容不变，不能用「结果相等」判断是否命中
if (!versionLine.test(cargo)) {
  console.error(`${cargoPath} 未找到 [package] version 行`)
  process.exit(1)
}
writeFileSync(cargoPath, cargo.replace(versionLine, (line) => line.replace(/"[^"]*"$/, `"${version}"`)))
console.log(`${cargoPath} -> ${version}`)
