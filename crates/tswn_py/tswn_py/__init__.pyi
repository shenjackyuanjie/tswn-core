"""
tswn_py 包顶层类型存根。

``__init__.py`` 通过 ``from .tswn_py import *`` 将扩展模块的所有符号
提升到顶层，本文件将这些符号显式地重新导出，使静态分析工具（pyright / mypy）
能正确解析 ``import tswn_py; tswn_py.Runner`` 等用法。
"""

from ._version import __version__ as __version__
from .tswn_py import (
    PreparedRunner as PreparedRunner,
    RunnerError as RunnerError,
    RunUpdate as RunUpdate,
    RunUpdates as RunUpdates,
    Runner as Runner,
    WorldState as WorldState,
    Storage as Storage,
    Player as Player,
    RC4 as RC4,
    core_version_str as core_version_str,
    name_to_png_base64 as name_to_png_base64,
    name_to_png_bytes as name_to_png_bytes,
    wrapper_version_str as wrapper_version_str,
)

__all__ = [
    "__version__",
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
