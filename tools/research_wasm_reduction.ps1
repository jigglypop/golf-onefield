# WASM 내보내기 축약 연구. 원본 코드는 읽기만 하고 사본을 flightbench/target 아래 빌드한다.
# 실행: powershell -ExecutionPolicy Bypass -File tools/research_wasm_reduction.ps1
# 필요: Rust wasm32-unknown-unknown 타깃, wasm-pack, Node 24 또는 동등한 ES 모듈 지원 런타임.
[CmdletBinding()]
param([string]$OutFile = 'docs/research/wasm-reduction-20261013.json')

$ErrorActionPreference = 'Stop'
# Windows PowerShell 5.1도 Node 표준 입력에 한글을 UTF-8로 넘기게 한다.
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$BuildRoot = Join-Path $RepoRoot 'flightbench/target/research-wasm-reduction'
$SourceRoot = Join-Path $RepoRoot 'golf-core'
$ResultPath = if ([System.IO.Path]::IsPathRooted($OutFile)) { $OutFile } else { Join-Path $RepoRoot $OutFile }
$Utf8 = [System.Text.UTF8Encoding]::new($false)
$FullDir = Join-Path $BuildRoot 'full'
$SlimDir = Join-Path $BuildRoot 'minimal'
New-Item -ItemType Directory -Path $BuildRoot -Force | Out-Null

foreach ($VariantDir in @($FullDir, $SlimDir)) {
    New-Item -ItemType Directory -Path (Join-Path $VariantDir 'src') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $SourceRoot 'Cargo.toml'), (Join-Path $SourceRoot 'Cargo.lock') -Destination $VariantDir
    Get-ChildItem -LiteralPath (Join-Path $SourceRoot 'src') -File -Filter '*.rs' | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $VariantDir 'src')
    }
}

$OriginalCode = [System.IO.File]::ReadAllText((Join-Path $SourceRoot 'src/lib.rs'), $Utf8)
$MinimalCode = [regex]::Replace($OriginalCode, '(?m)^[ \t]*#\[wasm_bindgen(?:\([^\r\n]*\))?\]\r?\n', '')
$MinimalCode += @'

#[wasm_bindgen]
pub fn mini_batch(shots: &[f64], h: f64) -> Vec<f64> {
    batch_simd(shots, h)
}
'@
[System.IO.File]::WriteAllText((Join-Path $SlimDir 'src/lib.rs'), $MinimalCode, $Utf8)

$SourceHashes = [ordered]@{}
foreach ($RelativeFile in @('Cargo.toml', 'Cargo.lock', 'src/lib.rs', 'src/lanes.rs', 'src/ground.rs')) {
    $SourceHashes["golf-core/$RelativeFile"] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $SourceRoot $RelativeFile)).Hash.ToLowerInvariant()
}
$RunInfo = [ordered]@{
    rust = ((& rustc --version) -join "`n")
    wasmPack = ((& wasm-pack --version) -join "`n")
    node = ((& node --version) -join "`n")
    flags = '-C target-feature=+simd128; wasm-pack --target web --release; opt-level=3, fat LTO, codegen-units=1'
    sourceHashes = $SourceHashes
    fullSourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $FullDir 'src/lib.rs')).Hash.ToLowerInvariant()
    minimalSourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $SlimDir 'src/lib.rs')).Hash.ToLowerInvariant()
}
$InfoPath = Join-Path $BuildRoot 'run-info.json'
[System.IO.File]::WriteAllText($InfoPath, ($RunInfo | ConvertTo-Json -Depth 10), $Utf8)

$OldRustFlags = $env:RUSTFLAGS
$OldTargetDir = $env:CARGO_TARGET_DIR
try {
    $env:RUSTFLAGS = '-C target-feature=+simd128'
    foreach ($VariantDir in @($FullDir, $SlimDir)) {
        # 독립 target을 써서 동일 crate 이름의 오래된 산출물을 재사용하지 않는다.
        $env:CARGO_TARGET_DIR = Join-Path $VariantDir 'target'
        & wasm-pack build --target web --release --out-dir pkg $VariantDir
        if ($LASTEXITCODE -ne 0) { throw "wasm-pack 실패: $VariantDir" }
    }
} finally {
    $env:RUSTFLAGS = $OldRustFlags
    $env:CARGO_TARGET_DIR = $OldTargetDir
}

$NodeScript = @'
const fs = require('fs'), zlib = require('zlib'), crypto = require('crypto');
const path = require('path'), { pathToFileURL } = require('url');
const [buildRoot, resultPath, infoPath] = process.argv.slice(2);
const sha = data => crypto.createHash('sha256').update(data).digest('hex');
(async () => {
  const provenance = JSON.parse(fs.readFileSync(infoPath, 'utf8'));
  const meta = [], modules = [];
  for (const variant of ['full', 'minimal']) {
    const dir = path.join(buildRoot, variant, 'pkg');
    const wasmPath = path.join(dir, 'golf_core_bg.wasm');
    const binary = fs.readFileSync(wasmPath), js = fs.readFileSync(path.join(dir, 'golf_core.js'));
    meta.push({ variant, path: wasmPath, bytes: binary.length, gzipBytes: zlib.gzipSync(binary).length,
      jsBytes: js.length, jsGzipBytes: zlib.gzipSync(js).length, sha256: sha(binary), jsSha256: sha(js),
      exports: WebAssembly.Module.exports(new WebAssembly.Module(binary)).map(e => e.name) });
    const mod = await import(pathToFileURL(path.join(dir, 'golf_core.js')).href);
    mod.initSync({ module: binary }); modules.push(mod);
  }
  const n = 4000, shots = new Float64Array(n * 4); let state = 42;
  const random = () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state / 4294967296; };
  for (let i = 0; i < n; i++) {
    shots[4*i] = 60 + 120 * random(); shots[4*i+1] = 5 + 35 * random();
    shots[4*i+2] = 500 + 10000 * random(); shots[4*i+3] = -40 + 80 * random();
  }
  const comparisons = [];
  for (const h of [0.05, 0.137]) {
    const a = modules[0].batch_simd(shots, h), b = modules[1].mini_batch(shots, h);
    if (a.length !== b.length) throw new Error('출력 길이가 다름');
    let mismatch = 0, maxAbs = 0;
    for (let i = 0; i < a.length; i++) if (!Object.is(a[i], b[i])) {
      mismatch++; maxAbs = Math.max(maxAbs, Math.abs(a[i] - b[i]));
    }
    comparisons.push({ h, n, outputElements: a.length, mismatch, maxAbs });
  }
  const result = { schema: 1, runUtc: new Date().toISOString(), recipe: 'tools/research_wasm_reduction.ps1', provenance,
    scope: '기본 SharedRe12, 무풍, SIMD f32 일괄 API. f64 경로, 환경 내보내기, field 전환, ground getter를 제외함.',
    sample: { seed: 42, mph: [60,180], launchDeg: [5,40], spinRpm: [500,10500], axisDeg: [-40,40],
      outputs: ['carry_m','lateral_m','apex_m','land_deg'] },
    meta, savings: { wasmRawPercent: 100*(1-meta[1].bytes/meta[0].bytes), wasmGzipPercent: 100*(1-meta[1].gzipBytes/meta[0].gzipBytes),
      wasmAndJsRawPercent: 100*(1-(meta[1].bytes+meta[1].jsBytes)/(meta[0].bytes+meta[0].jsBytes)),
      wasmAndJsGzipPercent: 100*(1-(meta[1].gzipBytes+meta[1].jsGzipBytes)/(meta[0].gzipBytes+meta[0].jsGzipBytes)) },
    comparisons, limitations: ['타 제품 API 구성은 별도 빌드 필요', 'CPU 시간이나 모바일 성능은 측정하지 않음', '엔진끼리의 수치 동치이며 실측 물리 검증은 아님'] };
  fs.mkdirSync(path.dirname(resultPath), { recursive: true }); fs.writeFileSync(resultPath, JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
  if (comparisons.some(c => c.mismatch)) process.exitCode = 1;
})().catch(error => { console.error(error); process.exitCode = 1; });
'@
$NodeScript | & node - $BuildRoot $ResultPath $InfoPath
if ($LASTEXITCODE -ne 0) { throw 'WASM 동치 검증 실패' }
