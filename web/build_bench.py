"""web/bench_template.html 에 펼친 장과 기준값을 넣어 onefield_bench.html 을 만든다.

먼저: cd flightbench && cargo run --release --bin field4 -- --dump
"""
import base64
import pathlib

here = pathlib.Path(__file__).parent
tpl = (here / "bench_template.html").read_text(encoding="utf-8")
field = base64.b64encode((here / "field_k4.bin").read_bytes()).decode()
truth = base64.b64encode((here / "truth.bin").read_bytes()).decode()
meta = (here / "field_meta.json").read_text(encoding="utf-8").strip()
out = tpl.replace("__META__", meta).replace("__FIELD_B64__", field).replace("__TRUTH_B64__", truth)
(here / "onefield_bench.html").write_text(out, encoding="utf-8")
print(f"onefield_bench.html {len(out.encode()) / 1e6:.2f} MB")
