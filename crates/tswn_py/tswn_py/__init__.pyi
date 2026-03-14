"""
tswn_py 包顶层类型存根。

``__init__.py`` 通过 ``from .tswn_py import *`` 将扩展模块的所有符号
提升到顶层，本文件将这些符号显式地重新导出，使静态分析工具（pyright / mypy）
能正确解析 ``import tswn_py; tswn_py.Runner`` 等用法。
"""

from .tswn_py import (
    PyRunnerError as PyRunnerError,
    RunUpdate as RunUpdate,
    RunUpdates as RunUpdates,
    Runner as Runner,
    core_version_str as core_version_str,
    wrapper_version_str as wrapper_version_str,
)

__all__ = [
    "PyRunnerError",
    "RunUpdate",
    "RunUpdates",
    "Runner",
    "core_version_str",
    "wrapper_version_str",
]
