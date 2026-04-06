"""tswn_py 扩展模块公开 API 的汇总存根。"""

from ._types_engine import Storage, WorldState
from ._types_player import Player
from ._types_rc4 import RC4
from ._types_runner import PreparedRunner, Runner
from ._types_update import RunUpdate, RunUpdates, RunnerError

DEFAULT_EVAL_RQ: float
"""普通对局默认使用的名字评估 `rq`。"""

WIN_RATE_EVAL_RQ: float
"""胜率基准默认使用的名字评估 `rq`。"""

def wrapper_version_str() -> str:
    """返回 tswn_py 绑定层版本字符串。"""
    ...

def core_version_str() -> str:
    """返回底层 tswn_core 版本字符串。"""
    ...

def name_to_icon_rgba(name: str) -> bytes:
    """将名字渲染为 16x16 RGBA 原始像素字节。"""
    ...

def win_rate(raw: str, n: int, eval_rq: float | None = None) -> float:
    """按 CLI 默认语义计算第一组对其余组的胜率百分比。"""
    ...

def group_win_rate(
    target: str, against: list[str], n: int, eval_rq: float | None = None
) -> list[tuple[str, float]]:
    """按 CLI 默认语义批量计算 target 对多个 opponent 的胜率百分比。"""
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
    "DEFAULT_EVAL_RQ",
    "WIN_RATE_EVAL_RQ",
    "core_version_str",
    "group_win_rate",
    "name_to_icon_rgba",
    "name_to_png_base64",
    "name_to_png_bytes",
    "win_rate",
    "wrapper_version_str",
]
