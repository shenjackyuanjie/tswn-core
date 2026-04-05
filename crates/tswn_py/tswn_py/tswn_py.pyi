"""tswn_py 扩展模块公开 API 的汇总存根。"""

from ._types_engine import Storage, WorldState
from ._types_player import Player
from ._types_rc4 import RC4
from ._types_runner import PreparedRunner, Runner
from ._types_update import RunUpdate, RunUpdates, RunnerError

def wrapper_version_str() -> str:
    """返回 tswn_py 绑定层版本字符串。"""
    ...

def core_version_str() -> str:
    """返回底层 tswn_core 版本字符串。"""
    ...

def name_to_png_base64(name: str) -> str:
    """将名字渲染为 PNG 并返回 Base64 字符串。"""
    ...

def name_to_png_bytes(name: str) -> bytes:
    """将名字渲染为 PNG 并返回原始字节。"""
    ...

__all__ = [
    "RunnerError",
    "PreparedRunner",
    "RunUpdate",
    "RunUpdates",
    "Runner",
    "WorldState",
    "Storage",
    "Player",
    "RC4",
    "core_version_str",
    "name_to_png_base64",
    "name_to_png_bytes",
    "wrapper_version_str",
]
