#!/usr/bin/env python3
"""构建并启动 tswn_wasm 本地网页。"""

import argparse
import sys
import webbrowser
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from subprocess import CalledProcessError
from tempfile import TemporaryDirectory
from threading import Thread

from build_wasm import main as build_wasm


class LocalHTTPServer(ThreadingHTTPServer):
    # Windows 的 SO_REUSEADDR 会允许多个服务监听同一个地址和端口。
    allow_reuse_address = sys.platform != "win32"
    allow_reuse_port = False


class WasmRequestHandler(SimpleHTTPRequestHandler):
    extensions_map = {
        **SimpleHTTPRequestHandler.extensions_map,
        ".js": "text/javascript",
        ".wasm": "application/wasm",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8000, help="本地端口（默认 8000，传入 0 自动选择）")
    parser.add_argument("--no-open", action="store_true", help="启动后仅打印地址")
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error("--port 必须介于 0 和 65535 之间")

    try:
        with TemporaryDirectory(prefix="tswn-wasm-") as directory:
            handler = partial(WasmRequestHandler, directory=directory)
            with LocalHTTPServer(("127.0.0.1", args.port), handler) as server:
                build_wasm(["--release", "--output-dir", directory])
                url = f"http://127.0.0.1:{server.server_port}/examples/"
                print(f"完整回放：{url}")
                print(f"功能演示：{url}demo.html")
                print("按 Ctrl+C 停止服务。", flush=True)
                if not args.no_open:
                    Thread(target=webbrowser.open, args=(url,), daemon=True).start()
                server.serve_forever()
    except KeyboardInterrupt:
        return 130
    except (OSError, RuntimeError, CalledProcessError) as error:
        print(f"[error] 启动 tswn_wasm 网页失败：{error}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
