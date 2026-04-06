"""
tswn_py — tswn_core 的 Python 绑定（PyO3 扩展模块）。

导出内容
--------
* 所有 PyO3 绑定符号（来自编译扩展模块 ``tswn_py.tswn_py``）：
  ``Runner``、``PyRunUpdates``、``PyRunUpdate``、``PyRunnerError``、
  ``wrapper_version_str``、``core_version_str``
* ``__version__``：与 ``Cargo.toml`` 保持同步的版本字符串。
"""

from ._version import __version__ as __version__
from .tswn_py import *  # type: ignore[reportWildcardImportFromLibrary]  # noqa: F403
