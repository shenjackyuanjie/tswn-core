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

def win_rate(raw: str, n: int, eval_rq: float | None = None, thread: int = 0) -> float:
    """按 CLI 默认语义计算第一组对其余组的胜率百分比。thread: 0=自动, 1=单线程, n=指定线程数。"""
    ...

def group_win_rate(
    target: str,
    against: list[str],
    n: int,
    eval_rq: float | None = None,
    thread: int = 0,
) -> list[tuple[str, float]]:
    """按 CLI 默认语义批量计算 target 对多个 opponent 的胜率百分比。thread: 0=自动, 1=单线程, n=指定线程数。"""
    ...

def prepared_win_rate(prepared: PreparedRunner, n: int, eval_rq: float | None = None, thread: int = 0) -> float:
    """基于 PreparedRunner 计算第一组对其余组的胜率百分比。thread: 0=自动, 1=单线程, n=指定线程数。"""
    ...

class WinRateResult:
    @property
    def wins(self) -> int: ...
    @property
    def total(self) -> int: ...
    @property
    def win_rate(self) -> float: ...
    @property
    def init_nanos(self) -> int: ...
    @property
    def fight_nanos(self) -> int: ...

class ScoreResult:
    @property
    def score(self) -> float: ...
    @property
    def wins(self) -> int: ...
    @property
    def total(self) -> int: ...
    @property
    def init_nanos(self) -> int: ...
    @property
    def fight_nanos(self) -> int: ...

class NamerPfResult:
    @property
    def group(self) -> list[str]: ...
    @property
    def modes(self) -> list[str]: ...
    @property
    def scores(self) -> list[float]: ...
    @property
    def total_score(self) -> float: ...
    def as_line(self, precision: int) -> str: ...

class BatchRateResult:
    @property
    def label(self) -> str: ...
    @property
    def avg_win_rate(self) -> float: ...
    @property
    def aggregate_win_rate(self) -> float: ...
    @property
    def wins(self) -> int: ...
    @property
    def total(self) -> int: ...
    @property
    def valid_matchups(self) -> int: ...
    @property
    def skipped_matchups(self) -> int: ...
    @property
    def init_nanos(self) -> int: ...
    @property
    def fight_nanos(self) -> int: ...

class PairRateResult:
    @property
    def label(self) -> str: ...
    @property
    def final_score(self) -> float: ...
    @property
    def head(self) -> int: ...
    @property
    def selected(self) -> int: ...
    @property
    def top_pairs(self) -> list[tuple[str, float]]: ...
    @property
    def aggregate_win_rate(self) -> float: ...
    @property
    def wins(self) -> int: ...
    @property
    def total(self) -> int: ...
    @property
    def valid_matchups(self) -> int: ...
    @property
    def skipped_matchups(self) -> int: ...
    @property
    def init_nanos(self) -> int: ...
    @property
    def fight_nanos(self) -> int: ...

class IconInfo:
    @property
    def border_style(self) -> int: ...
    @property
    def shapes(self) -> list[int]: ...
    @property
    def bg_color_idx(self) -> int: ...
    @property
    def bg_color(self) -> tuple[int, int, int]: ...
    @property
    def fg_color_indices(self) -> list[int]: ...
    @property
    def fg_colors(self) -> list[tuple[int, int, int]]: ...
    @property
    def colors_consumed(self) -> int: ...

def win_rate_summary(raw: str, n: int, eval_rq: float | None = None, thread: int = 0) -> WinRateResult:
    ...

def team_win_rate_summary(team1: str, team2: str, n: int, eval_rq: float | None = None, thread: int = 0) -> WinRateResult:
    ...

def group_win_rate_summary(
    target: str,
    against: list[str],
    n: int,
    eval_rq: float | None = None,
    thread: int = 0,
) -> list[tuple[str, WinRateResult]]:
    ...

def score(raw: str, n: int, mode: str = "normal", eval_rq: float | None = None, thread: int = 0) -> ScoreResult:
    ...

def namer_pf(
    raw: str,
    n: int,
    modes: list[str] | None = None,
    keep_rq: bool = False,
    thread: int = 0,
) -> list[NamerPfResult]:
    ...

def batch_rate(
    target_groups: list[str],
    player_groups: list[str],
    n: int,
    player_labels: list[str] | None = None,
    keep_rq: bool = False,
    thread: int = 0,
) -> list[BatchRateResult]:
    ...

def pair_rate(
    target_groups: list[str],
    players: list[str],
    teammates: list[str],
    head: int,
    n: int,
    keep_rq: bool = False,
    thread: int = 0,
) -> list[PairRateResult]:
    ...

def to_diy(name: str, old: bool = False, minions: bool = False) -> str:
    ...

def to_diy_batch(names: list[str], old: bool = False, minions: bool = False) -> list[str]:
    ...

def icon_info(name: str) -> IconInfo:
    ...

def parse_group_lines(content: str, double_plus: bool = False) -> list[str]:
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
    "prepared_win_rate",
    "WinRateResult",
    "ScoreResult",
    "NamerPfResult",
    "BatchRateResult",
    "PairRateResult",
    "IconInfo",
    "win_rate_summary",
    "team_win_rate_summary",
    "group_win_rate_summary",
    "score",
    "namer_pf",
    "batch_rate",
    "pair_rate",
    "to_diy",
    "to_diy_batch",
    "icon_info",
    "parse_group_lines",
    "name_to_icon_rgba",
    "name_to_png_base64",
    "name_to_png_bytes",
    "win_rate",
    "wrapper_version_str",
]
